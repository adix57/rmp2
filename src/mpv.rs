use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

pub enum MpvMsg {
    Event { name: String, data: Option<Value> },
    Shutdown,
}

type Pending = Arc<Mutex<HashMap<u32, Sender<Result<Value, String>>>>>;

pub struct Mpv {
    sock: UnixStream,
    next_id: AtomicU32,
    pending: Pending,
    child: Child,
    ipc_path: String,
}

impl Drop for Mpv {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_file(&self.ipc_path);
    }
}

pub const P_PAUSE: u32 = 1;
pub const P_TIME_POS: u32 = 2;

fn connect_retry(path: &Path, attempts: usize, delay: Duration) -> Result<UnixStream, String> {
    let mut last = "no attempt".to_string();
    for _ in 0..attempts {
        match UnixStream::connect(path) {
            Ok(s) => return Ok(s),
            Err(e) => {
                last = e.to_string();
                thread::sleep(delay);
            }
        }
    }
    Err(last)
}

pub type LiveInfo = (Option<String>, Option<String>, Option<f64>, Option<u64>);

impl Mpv {
    pub fn spawn(
        binary: &str,
        ipc_path: &Path,
        probe: bool,
    ) -> Result<(Mpv, Receiver<MpvMsg>), String> {
        let _ = fs::remove_file(ipc_path);
        let mut cmd = Command::new(binary);
        cmd.arg("--idle=yes")
            .arg("--no-terminal")
            .arg("--quiet")
            .arg("--no-video")
            .arg("--audio-display=no")
            .arg(format!("--input-ipc-server={}", ipc_path.to_string_lossy()))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if probe {
            cmd.arg("--no-config")
                .arg("--load-scripts=no")
                .arg("--ao=null");
        } else {
            cmd.arg("--volume=70");
        }
        let mut child = cmd.spawn().map_err(|e| format!("spawn mpv: {e}"))?;
        let sock = match connect_retry(ipc_path, 40, Duration::from_millis(50)) {
            Ok(s) => s,
            Err(e) => {
                let _ = child.kill();
                return Err(format!("connect mpv ipc: {e}"));
            }
        };
        let reader = BufReader::new(sock.try_clone().map_err(|e| e.to_string())?);
        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        let (tx, rx) = mpsc::channel();
        let td = pending.clone();
        thread::spawn(move || read_loop(reader, tx, td));
        Ok((
            Mpv {
                sock,
                next_id: AtomicU32::new(0),
                pending,
                child,
                ipc_path: ipc_path.to_string_lossy().into_owned(),
            },
            rx,
        ))
    }

    fn next_id(&self) -> u32 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    pub fn call(&mut self, cmd: Value, wait: Duration) -> Result<Value, String> {
        let id = self.next_id();
        let (tx, rx) = mpsc::channel();
        self.pending.lock().unwrap().insert(id, tx);
        let body = json!({ "command": cmd, "request_id": id }).to_string();
        let res = self
            .sock
            .write_all(body.as_bytes())
            .and_then(|_| self.sock.write_all(b"\n"))
            .and_then(|_| self.sock.flush());
        if res.is_err() {
            self.pending.lock().unwrap().remove(&id);
        }
        res.map_err(|e| e.to_string())?;
        match rx.recv_timeout(wait) {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(e)) => Err(e),
            Err(_) => Err("mpv timeout".into()),
        }
    }

    pub fn observe(&mut self, prop_id: u32, name: &str) {
        let _ = self.command(json!(["observe_property", prop_id, name]));
    }

    pub fn command(&mut self, cmd: Value) -> Result<Value, String> {
        self.call(cmd, Duration::from_secs(5))
    }

    pub fn loadfile(&mut self, uri: &str) -> Result<(), String> {
        self.command(json!(["loadfile", uri, "replace"]))
            .map(|_| ())
    }

    pub fn set_pause(&mut self, paused: bool) -> Result<(), String> {
        self.command(json!(["set_property", "pause", paused]))
            .map(|_| ())
    }

    pub fn seek(&mut self, seconds: f64, absolute: bool) -> Result<(), String> {
        if absolute {
            self.command(json!(["seek", seconds, "absolute"]))
                .map(|_| ())
        } else {
            self.command(json!(["seek", seconds, "relative"]))
                .map(|_| ())
        }
    }

    pub fn volume(&mut self, value: i32) -> Result<(), String> {
        self.command(json!(["set_property", "volume", value]))
            .map(|_| ())
    }

    pub fn property(&mut self, name: &str) -> Result<Value, String> {
        self.command(json!(["get_property", name]))
    }
}

fn read_loop(reader: BufReader<UnixStream>, tx: Sender<MpvMsg>, pending: Pending) {
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(rid) = msg.get("request_id").and_then(|v| v.as_u64()) {
            let data = match msg.get("data") {
                Some(d) if !d.is_null() => Ok(d.clone()),
                _ => match msg.get("error").and_then(|e| e.as_str()) {
                    Some(e) if e != "success" => Err(e.to_string()),
                    _ => Ok(Value::Null),
                },
            };
            if let Some(t) = pending.lock().unwrap().remove(&(rid as u32)) {
                let _ = t.send(data);
            }
        } else if let Some(ev) = msg.get("event").and_then(|v| v.as_str()) {
            let name = msg
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or(ev)
                .to_string();
            let data = if let Some(r) = msg.get("reason").and_then(|v| v.as_str()) {
                Some(json!({ "reason": r }))
            } else {
                msg.get("data").cloned().filter(|d| !d.is_null())
            };
            if tx.send(MpvMsg::Event { name, data }).is_err() {
                break;
            }
        }
    }
    let _ = tx.send(MpvMsg::Shutdown);
}

pub fn probe_metadata(binary: &str, ipc_path: &Path, uri: &str) -> Option<LiveInfo> {
    let (mut mpv, rx) = Mpv::spawn(binary, ipc_path, true).ok()?;
    mpv.loadfile(uri).ok()?;
    let deadline = Instant::now() + Duration::from_secs(6);
    let mut loaded = false;
    while Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(MpvMsg::Event { name, .. }) if name == "file-loaded" => {
                loaded = true;
                break;
            }
            Ok(_) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => break,
        }
    }
    if !loaded {
        return None;
    }
    thread::sleep(Duration::from_millis(300));
    Some(live_info(&mut mpv))
}

pub fn live_info(mpv: &mut Mpv) -> LiveInfo {
    let title =
        metadata_string(mpv, "media-title").or_else(|| metadata_string(mpv, "metadata/Title"));
    let artist = metadata_string(mpv, "metadata/Artist");
    let duration = mpv
        .property("duration")
        .ok()
        .and_then(|v| v.as_f64())
        .filter(|d| *d > 0.0);
    let bitrate = mpv
        .property("audio-params/bitrate")
        .ok()
        .and_then(|v| v.as_u64());
    (title, artist, duration, bitrate)
}

fn metadata_string(mpv: &mut Mpv, name: &str) -> Option<String> {
    mpv.property(name).ok().and_then(|v| match &v {
        Value::String(s) => Some(s.clone()),
        Value::Null => None,
        _ => v.as_str().map(|s| s.to_string()),
    })
}
