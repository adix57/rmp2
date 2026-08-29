mod config;
mod daemon;
mod db;
mod engine;
mod mpv;
mod proto;
mod state;
mod ui;

use config::{Config, Paths, config_dir};
use std::io::Write;
use std::os::unix::process::CommandExt;
use std::process::{Stdio, exit};
use std::thread;
use std::time::Duration;

fn main() {
    if std::env::args().any(|a| a == "--daemon") {
        if let Err(e) = daemon::Daemon::run() {
            eprintln!("rmp daemon: {e}");
            exit(1);
        }
        return;
    }
    let paths = Paths::resolve();
    let connection = match ui::Connection::connect(&paths.sock) {
        Ok(c) => c,
        Err(_) => {
            if let Err(e) = spawn_daemon() {
                eprintln!("rmp: {e}");
                exit(1);
            }
            let mut connected = None;
            for _ in 0..80 {
                thread::sleep(Duration::from_millis(50));
                if let Ok(c) = ui::Connection::connect(&paths.sock) {
                    connected = Some(c);
                    break;
                }
            }
            match connected {
                Some(c) => c,
                None => {
                    eprintln!("rmp: daemon did not start");
                    exit(1);
                }
            }
        }
    };
    let cfg = match Config::load(&paths.config) {
        Ok(c) => c,
        Err(e) => {
            let _ = writeln!(std::io::stderr(), "rmp: {e}");
            exit(1);
        }
    };
    if let Err(e) = ui::run(connection, cfg) {
        let _ = writeln!(std::io::stderr(), "rmp: {e}");
    }
}

fn spawn_daemon() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("--daemon")
        .env("RMP2_DIR", config_dir())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0);
    cmd.spawn().map_err(|e| format!("spawn daemon: {e}"))?;
    Ok(())
}
