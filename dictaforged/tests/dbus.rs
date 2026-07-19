//! D-Bus round trip against the real daemon binary in stub mode.
//! Run inside a private bus:
//!   dbus-run-session -- cargo test -p dictaforged --test dbus -- --ignored

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use zbus::blocking::Connection;
use zbus::proxy;

#[proxy(
    interface = "org.dictaforge.Daemon1",
    default_service = "org.dictaforge.Daemon1",
    default_path = "/org/dictaforge/Daemon1"
)]
trait Daemon1 {
    fn toggle(&self) -> zbus::Result<()>;
    fn start(&self) -> zbus::Result<()>;
    fn stop(&self) -> zbus::Result<()>;
    fn status(&self) -> zbus::Result<String>;
    fn inject_text(&self, text: &str) -> zbus::Result<()>;
}

/// Kills the daemon even when an assert panics mid-test.
struct KillOnDrop(Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn wait_for_daemon(conn: &Connection) -> Daemon1ProxyBlocking<'_> {
    let proxy = Daemon1ProxyBlocking::new(conn).unwrap();
    for _ in 0..50 {
        if proxy.status().is_ok() {
            return proxy;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("daemon never claimed org.dictaforge.Daemon1");
}

#[test]
#[ignore = "needs a session bus; run under dbus-run-session"]
fn stub_daemon_round_trip() {
    let child = Command::new(env!("CARGO_BIN_EXE_dictaforged"))
        .arg("--no-hardware")
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut child = KillOnDrop(child);

    let conn = Connection::session().unwrap();
    let daemon = wait_for_daemon(&conn);

    let status: serde_json::Value = serde_json::from_str(&daemon.status().unwrap()).unwrap();
    assert_eq!(status["state"], "idle");
    assert_eq!(status["backend"], "stub");
    assert!(status["model"].is_string());
    assert!(status["layout"].is_string());

    daemon.toggle().unwrap();
    let status: serde_json::Value = serde_json::from_str(&daemon.status().unwrap()).unwrap();
    assert_eq!(status["state"], "recording");
    daemon.toggle().unwrap();
    let status: serde_json::Value = serde_json::from_str(&daemon.status().unwrap()).unwrap();
    assert_eq!(status["state"], "idle");

    daemon.start().unwrap();
    let status: serde_json::Value = serde_json::from_str(&daemon.status().unwrap()).unwrap();
    assert_eq!(status["state"], "recording");
    daemon.stop().unwrap();
    let status: serde_json::Value = serde_json::from_str(&daemon.status().unwrap()).unwrap();
    assert_eq!(status["state"], "idle");

    daemon.inject_text("Grüße vom Stub").unwrap();

    // The stub injector prints instead of typing; the text must have landed.
    let mut stdout = child.0.stdout.take().unwrap();
    let _ = child.0.kill();
    let _ = child.0.wait();
    let mut out = String::new();
    stdout.read_to_string(&mut out).unwrap();
    assert!(
        out.contains("Grüße vom Stub"),
        "stub injector output missing text: {out:?}"
    );
}
