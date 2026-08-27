use crate::config::{Config, Paths};
use crate::db::{Library, classify_uri};
use crate::engine::{Lcg, filter_queue, next_index};
use crate::mpv::{Mpv, MpvMsg, P_EOF, P_IDLE, P_PAUSE, P_TIME_POS, live_info, probe_metadata};
use crate::proto::{Command, NowPlaying, RepeatMode, Snapshot};
use crate::state::{self, LastState};
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::Shutdown;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

enum LoopEvent {
    Command { _id: u64, cmd: Command },
    Drop { id: u64 },
    Mpv(MpvMsg),
}

pub struct Daemon {
    paths: Paths,
    cfg: Config,
    lib: Library,
    mpv: Option<Mpv>,
    mpv_rx: Option<Receiver<MpvMsg>>,
    listener: UnixListener,
    events_tx: Sender<LoopEvent>,
    events_rx: Receiver<LoopEvent>,
    clients: HashMap<u64, Sender<Value>>,
    next_client: u64,
    stop_flag: Arc<AtomicBool>,
    active_tags: Vec<String>,
    search: Option<String>,
    volume: i32,
    repeat: RepeatMode,
    shuffle: bool,
    selected: Option<i64>,
    now_id: Option<i64>,
    position: f64,
    paused: bool,
    notify: Option<String>,
    dirty: bool,
    last_push: Instant,
    eof_pending: bool,
    pending_resume: Option<(i64, f64, bool)>,
    rng: Lcg,
    stopping: bool,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

impl Daemon {
    pub fn run() -> Result<(), String> {
        let paths = Paths::resolve();
        let cfg = Config::load(&paths.config);
        let listener = match UnixListener::bind(&paths.sock) {
            Ok(l) => l,
            Err(_) => {
                if pid_alive(&paths.pid) {
                    return Err("daemon already running".into());
                }
                let _ = fs::remove_file(&paths.sock);
                UnixListener::bind(&paths.sock).map_err(|e| format!("bind socket: {e}"))?
            }
        };
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let last = state::load(&paths.state);
        let mut daemon = Self::new(paths, cfg, listener, last);
        daemon.write_pid();
        daemon.log("daemon started");
        daemon.event_loop();
        daemon.log("daemon stopped");
        Ok(())
    }

    fn new(paths: Paths, cfg: Config, listener: UnixListener, last: LastState) -> Self {
        let lib = Library::open(&paths.db).unwrap_or_else(|e| {
            eprintln!("db error: {e}");
            std::process::exit(1);
        });
        let (events_tx, events_rx) = mpsc::channel();
        let stop_flag = Arc::new(AtomicBool::new(false));
        let mut d = Daemon {
            paths,
            cfg,
            lib,
            mpv: None,
            mpv_rx: None,
            listener,
            events_tx,
            events_rx,
            clients: HashMap::new(),
            next_client: 0,
            stop_flag: stop_flag.clone(),
            active_tags: last.active_tags.iter().map(|s| s.to_lowercase()).collect(),
            search: None,
            volume: last.volume.clamp(0, 130),
            repeat: last.repeat,
            shuffle: last.shuffle,
            selected: None,
            now_id: last.media_id,
            position: last.position,
            paused: !last.playing,
            notify: None,
            dirty: false,
            last_push: Instant::now(),
            eof_pending: false,
            pending_resume: None,
            rng: Lcg::new(),
            stopping: false,
        };
        if let Some(id) = d.now_id {
            if d.lib.media(id).ok().flatten().is_some() {
                d.pending_resume = Some((id, d.position, !d.paused));
            } else {
                d.now_id = None;
            }
        }
        d.install_signals();
        match d.ensure_mpv() {
            Ok(()) => {}
            Err(e) => d.log(&format!("mpv unavailable: {e}")),
        }
        d.push();
        d
    }

    fn install_signals(&self) {
        let flag = self.stop_flag.clone();
        let _ = thread::Builder::new().name("sig".into()).spawn(move || {
            use signal_hook::consts::signal::{SIGINT, SIGTERM};
            if let Ok(mut signals) = signal_hook::iterator::Signals::new([SIGINT, SIGTERM]) {
                let _ = signals.wait();
                flag.store(true, Ordering::SeqCst);
            }
        });
    }

    fn write_pid(&self) {
        let _ = fs::write(&self.paths.pid, std::process::id().to_string());
    }

    fn log(&self, msg: &str) {
        if let Ok(mut f) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.paths.log)
        {
            let _ = writeln!(f, "[{}] {msg}", now_ms());
        }
    }

    fn ensure_mpv(&mut self) -> Result<(), String> {
        let ipc = self.paths.root.join("mpv.sock");
        let (mut mpv, rx) = Mpv::spawn(&self.cfg.mpv_binary, &ipc, false)?;
        let _ = mpv.volume(self.volume);
        mpv.observe(P_PAUSE, "pause");
        mpv.observe(P_TIME_POS, "time-pos");
        mpv.observe(P_IDLE, "idle-active");
        mpv.observe(P_EOF, "eof-reached");
        if let Some(id) = self.now_id
            && let Some(m) = self.lib.media(id).ok().flatten()
        {
            let _ = mpv.loadfile(&m.uri);
            self.pending_resume = Some((id, self.position, !self.paused));
        }
        self.mpv = Some(mpv);
        self.mpv_rx = Some(rx);
        Ok(())
    }

    fn event_loop(&mut self) {
        while !self.stopping && !self.stop_flag.load(Ordering::SeqCst) {
            loop {
                match self.listener.accept() {
                    Ok((stream, _)) => {
                        let id = self.next_client;
                        self.next_client += 1;
                        self.register_client(id, stream);
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(e) => {
                        self.log(&format!("accept error: {e}"));
                        break;
                    }
                }
            }
            if let Ok(ev) = self.events_rx.recv_timeout(Duration::from_millis(30)) {
                self.handle_event(ev);
            }
            let mut mpv_events = Vec::new();
            if let Some(rx) = &self.mpv_rx {
                while let Ok(ev) = rx.try_recv() {
                    mpv_events.push(ev);
                }
            }
            for ev in mpv_events {
                self.handle_event(LoopEvent::Mpv(ev));
            }
            if self.dirty
                && self.now_id.is_some()
                && Instant::now().duration_since(self.last_push) >= Duration::from_millis(350)
            {
                self.push();
            }
        }
        self.shutdown();
    }

    fn register_client(&mut self, id: u64, stream: UnixStream) {
        let (tx, rx) = mpsc::channel();
        self.clients.insert(id, tx.clone());
        let reader = match stream.try_clone() {
            Ok(s) => s,
            Err(e) => {
                self.log(&format!("clone client: {e}"));
                self.clients.remove(&id);
                return;
            }
        };
        let e = self.events_tx.clone();
        thread::spawn(move || client_reader(reader, id, e));
        thread::spawn(move || client_writer(stream, rx));
        let snap = self.snapshot_value();
        let _ = tx.send(snap);
    }

    fn handle_event(&mut self, ev: LoopEvent) {
        match ev {
            LoopEvent::Command { cmd, .. } => self.handle_command(cmd),
            LoopEvent::Drop { id } => {
                self.clients.remove(&id);
            }
            LoopEvent::Mpv(m) => self.handle_mpv(m),
        }
    }

    fn handle_mpv(&mut self, msg: MpvMsg) {
        match msg {
            MpvMsg::Event { name, data } => {
                let got = data.as_ref().and_then(|v| v.as_bool());
                match name.as_str() {
                    "time-pos" => {
                        if let Some(v) = data.as_ref().and_then(|v| v.as_f64()) {
                            self.position = v;
                            self.dirty = true;
                        }
                    }
                    "pause" => {
                        if let Some(v) = got {
                            self.paused = v;
                            self.dirty = true;
                        }
                    }
                    "idle-active" => {
                        if !self.eof_pending {
                            self.eof_pending = true;
                            self.track_ended();
                        }
                    }
                    "eof-reached" => {
                        if !self.eof_pending {
                            self.eof_pending = true;
                            self.track_ended();
                        }
                    }
                    "file-loaded" => {
                        self.eof_pending = false;
                        self.on_file_loaded();
                    }
                    _ => {}
                }
            }
            MpvMsg::Shutdown => {
                self.log("mpv exited, restarting");
                self.mpv = None;
                self.mpv_rx = None;
                if !self.stopping {
                    if let Err(e) = self.ensure_mpv() {
                        self.log(&format!("mpv restart failed: {e}"));
                        self.notify = Some("mpv restart failed".into());
                    }
                    self.push();
                }
            }
        }
    }

    fn on_file_loaded(&mut self) {
        self.eof_pending = false;
        if let Some((id, pos, playing)) = self.pending_resume.take() {
            self.now_id = Some(id);
            self.selected = Some(id);
            if let Some(m) = self.mpv.as_mut() {
                if pos > 1.0 {
                    let _ = m.seek(pos, true);
                }
                let _ = m.set_pause(!playing);
            }
            self.paused = !playing;
            self.dirty = true;
        }
        let id = self.now_id;
        if let Some(id) = id
            && let Some(m) = self.mpv.as_mut()
        {
            let (title, artist, duration, bitrate) = live_info(m);
            if self.pending_resume.is_none() {
                let _ = self
                    .lib
                    .set_title_artist(id, title.as_deref(), artist.as_deref());
            }
            let _ = self.lib.update_playback_stats(id, duration, bitrate);
        }
        self.push();
    }

    fn track_ended(&mut self) {
        if self.now_id.is_none() {
            return;
        }
        if self.repeat == RepeatMode::One {
            let id = self.now_id;
            if let Some(id) = id
                && let Some(m) = self.lib.media(id).ok().flatten()
                && let Some(mv) = self.mpv.as_mut()
            {
                let _ = mv.loadfile(&m.uri);
            }
            return;
        }
        let queue = self.queue_ids();
        let next = next_index(
            &queue,
            self.now_id,
            1,
            self.shuffle,
            self.repeat,
            &mut |n| self.rng.next(n),
        );
        match next {
            Some(id) => self.play_id(id),
            None => {
                self.now_id = None;
                self.paused = false;
                self.position = 0.0;
                self.pending_resume = None;
                self.push();
            }
        }
    }

    fn play_id(&mut self, id: i64) {
        let info = match self.lib.media(id) {
            Ok(Some(m)) => m,
            Ok(None) => {
                self.notify = Some("media not found".into());
                return;
            }
            Err(e) => {
                self.log(&format!("db error: {e}"));
                return;
            }
        };
        let mpv = match self.mpv.as_mut() {
            Some(m) => m,
            None => {
                self.notify = Some("mpv unavailable".into());
                return;
            }
        };
        match mpv.loadfile(&info.uri) {
            Ok(()) => {
                self.now_id = Some(id);
                self.selected = Some(id);
                self.position = 0.0;
                self.paused = false;
                self.eof_pending = false;
                self.pending_resume = None;
                self.push();
            }
            Err(e) => {
                self.log(&format!("loadfile error: {e}"));
                self.notify = Some("failed to load media".into());
            }
        }
    }

    fn toggle_pause(&mut self) {
        if self.now_id.is_none() {
            let q = self.queue_ids();
            if let Some(first) = q.first() {
                self.play_id(*first);
            }
            return;
        }
        if let Some(m) = self.mpv.as_mut() {
            let _ = m.set_pause(!self.paused);
        }
        self.paused = !self.paused;
        self.push();
    }

    fn queue_ids(&mut self) -> Vec<i64> {
        let pat = self.search.clone();
        let re = match pat.as_deref() {
            Some(p) if !p.is_empty() => match Regex::new(p) {
                Ok(r) => Some(r),
                Err(_) => {
                    self.search = None;
                    self.notify = Some("invalid search pattern".into());
                    None
                }
            },
            _ => None,
        };
        let all = self.lib.all_media().unwrap_or_default();
        filter_queue(&all, &self.active_tags, re.as_ref())
    }

    fn handle_command(&mut self, cmd: Command) {
        let before = cmd.clone();
        match before {
            Command::PlayPause => self.toggle_pause(),
            Command::Next => {
                let q = self.queue_ids();
                if let Some(n) =
                    next_index(&q, self.now_id, 1, self.shuffle, self.repeat, &mut |n| {
                        self.rng.next(n)
                    })
                {
                    self.play_id(n);
                }
            }
            Command::Prev => {
                let q = self.queue_ids();
                if let Some(n) =
                    next_index(&q, self.now_id, -1, self.shuffle, self.repeat, &mut |n| {
                        self.rng.next(n)
                    })
                {
                    self.play_id(n);
                }
            }
            Command::Play { id } => self.play_id(id),
            Command::Select { id } => {
                self.selected = Some(id);
                self.push();
            }
            Command::Seek { delta } => {
                if let Some(m) = self.mpv.as_mut() {
                    let _ = m.seek(delta, false);
                }
                self.position = (self.position + delta).max(0.0);
                self.push();
            }
            Command::Volume { delta } => {
                self.volume = (self.volume + delta).clamp(0, 130);
                if let Some(m) = self.mpv.as_mut() {
                    let _ = m.volume(self.volume);
                }
                self.persist();
                self.push();
            }
            Command::SetVolume { volume } => {
                self.volume = volume.clamp(0, 130);
                if let Some(m) = self.mpv.as_mut() {
                    let _ = m.volume(self.volume);
                }
                self.persist();
                self.push();
            }
            Command::RepeatCycle => {
                self.repeat = self.repeat.cycle();
                self.persist();
                self.push();
            }
            Command::ShuffleToggle => {
                self.shuffle = !self.shuffle;
                self.persist();
                self.push();
            }
            Command::ToggleTag { tag } => {
                let tag = tag.to_lowercase();
                if self.active_tags.contains(&tag) {
                    self.active_tags.retain(|t| *t != tag);
                } else {
                    self.active_tags.push(tag);
                }
                self.persist();
                self.push();
            }
            Command::ClearTags => {
                self.active_tags.clear();
                self.persist();
                self.push();
            }
            Command::SetSearch { pattern } => {
                self.search = pattern.filter(|p| !p.is_empty());
                self.push();
            }
            Command::Add { uri, name, tags } => self.add_media(uri, name, tags),
            Command::Update {
                id,
                name,
                title,
                artist,
                tags,
            } => {
                if let Err(e) =
                    self.lib
                        .update_media(id, &name, title.as_deref(), artist.as_deref(), &tags)
                {
                    self.log(&format!("update error: {e}"));
                    self.notify = Some("update failed".into());
                }
                self.push();
            }
            Command::Delete { id } => {
                let _ = self.lib.delete_media(id);
                if self.now_id == Some(id) {
                    self.now_id = None;
                    self.selected = None;
                    self.pending_resume = None;
                    if let Some(m) = self.mpv.as_mut() {
                        let _ = m.command(serde_json::json!(["stop"]));
                    }
                }
                if self.selected == Some(id) {
                    self.selected = None;
                }
                self.push();
            }
            Command::ToggleFavorite { id } => {
                if let Ok(Some(m)) = self.lib.media(id) {
                    let _ = self.lib.set_favorite(id, !m.favorite);
                }
                self.push();
            }
            Command::Shutdown => {
                self.stopping = true;
            }
        }
    }

    fn add_media(&mut self, uri: String, name: Option<String>, tags: Vec<String>) {
        let uri = uri.trim().to_string();
        if uri.is_empty() {
            self.notify = Some("uri cannot be empty".into());
            self.push();
            return;
        }
        let (kind, source) = classify_uri(&uri);
        if kind == "offline" && !Path::new(&uri).exists() {
            self.notify = Some("file does not exist".into());
            self.push();
            return;
        }
        let name = name
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                Path::new(&uri)
                    .file_name()
                    .map(|f| f.to_string_lossy().into_owned())
                    .unwrap_or_else(|| uri.clone())
            });
        let tags: Vec<String> = {
            let mut seen = vec![];
            for t in tags {
                let t = t.trim().to_lowercase();
                if !t.is_empty() && !seen.contains(&t) {
                    seen.push(t);
                }
            }
            seen
        };
        let (title, artist, duration, bitrate) = if kind == "offline" {
            let probe = probe_path();
            let res = probe_metadata(&self.cfg.mpv_binary, &probe, &uri);
            let _ = fs::remove_file(&probe);
            res.unwrap_or((None, None, None, None))
        } else {
            (None, None, None, None)
        };
        match self.lib.add_media(
            crate::db::NewMedia {
                uri: &uri,
                name: &name,
                kind,
                title: title.as_deref(),
                artist: artist.as_deref(),
                duration,
                bitrate,
                source: source.as_deref(),
            },
            &tags,
        ) {
            Ok(id) => {
                self.log(&format!("added media {id}: {name}"));
                self.notify = Some(format!("added: {name}"));
                if self.selected.is_none() {
                    self.selected = Some(id);
                }
            }
            Err(e) => {
                self.log(&format!("add error: {e}"));
                self.notify = Some("add failed".into());
            }
        }
        self.push();
    }

    fn build_snapshot(&mut self) -> Snapshot {
        let all = self.lib.all_media().unwrap_or_default();
        let pat = self.search.clone();
        let re = match pat.as_deref() {
            Some(p) if !p.is_empty() => Regex::new(p).ok(),
            _ => None,
        };
        let queue = filter_queue(&all, &self.active_tags, re.as_ref());
        let tags = self.lib.tags(&self.active_tags).unwrap_or_default();
        let selected = self
            .selected
            .and_then(|id| all.iter().find(|m| m.id == id).cloned());
        let now = self.now_id.map(|id| NowPlaying {
            id,
            position: self.position,
            duration: all.iter().find(|m| m.id == id).and_then(|m| m.duration),
            paused: self.paused,
        });
        Snapshot {
            all_media: all,
            queue,
            tags,
            search: self.search.clone(),
            selected,
            now,
            volume: self.volume,
            repeat: self.repeat,
            shuffle: self.shuffle,
            notify: self.notify.take(),
        }
    }

    fn snapshot_value(&mut self) -> Value {
        match serde_json::to_value(self.build_snapshot()) {
            Ok(v) => v,
            Err(e) => {
                self.log(&format!("snapshot serialize: {e}"));
                Value::Null
            }
        }
    }

    fn push(&mut self) {
        let snap = self.snapshot_value();
        self.clients.retain(|_, tx| tx.send(snap.clone()).is_ok());
        self.dirty = false;
        self.last_push = Instant::now();
    }

    fn persist(&self) {
        let s = LastState {
            volume: self.volume,
            repeat: self.repeat,
            shuffle: self.shuffle,
            active_tags: self.active_tags.clone(),
            media_id: self.now_id,
            position: self.position,
            playing: self.now_id.is_some() && !self.paused,
        };
        state::save(&self.paths.state, &s);
    }

    fn shutdown(&mut self) {
        self.persist();
        if let Some(m) = self.mpv.as_mut() {
            let _ = m.command(serde_json::json!(["quit"]));
        }
        let _ = fs::remove_file(&self.paths.sock);
        let _ = fs::remove_file(&self.paths.pid);
    }
}

fn probe_path() -> std::path::PathBuf {
    Paths::resolve()
        .root
        .join(format!("probe-{}.sock", std::process::id()))
}

fn pid_alive(pid_path: &Path) -> bool {
    let Ok(raw) = fs::read_to_string(pid_path) else {
        return false;
    };
    let Ok(pid) = raw.trim().parse::<u32>() else {
        return false;
    };
    Path::new(&format!("/proc/{pid}")).exists() && pid != std::process::id()
}

fn client_reader(stream: UnixStream, id: u64, tx: Sender<LoopEvent>) {
    let reader = BufReader::new(stream);
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if let Ok(cmd) = serde_json::from_str::<Command>(&line)
            && tx.send(LoopEvent::Command { _id: id, cmd }).is_err()
        {
            break;
        }
    }
    let _ = tx.send(LoopEvent::Drop { id });
}

fn client_writer(mut stream: UnixStream, rx: Receiver<Value>) {
    for msg in rx {
        let mut body = msg.to_string();
        body.push('\n');
        if stream.write_all(body.as_bytes()).is_err() {
            break;
        }
    }
    let _ = stream.shutdown(Shutdown::Both);
}
