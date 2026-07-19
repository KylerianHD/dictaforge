//! Full loop on headless sway: the real daemon probes its backends, picks
//! the wlroots virtual keyboard, and types into a focused client (wev),
//! driven over D-Bus like a real session. Run inside a private bus:
//!   dbus-run-session -- cargo test -p dictaforged --test inject_sway -- --ignored
//! Needs sway and wev installed; no real GPU or input devices required.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use zbus::blocking::Connection;
use zbus::proxy;

#[proxy(
    interface = "org.dictaforge.Daemon1",
    default_service = "org.dictaforge.Daemon1",
    default_path = "/org/dictaforge/Daemon1"
)]
trait Daemon1 {
    fn status(&self) -> zbus::Result<String>;
    fn inject_text(&self, text: &str) -> zbus::Result<()>;
}

/// Kills the child even when an assert panics mid-test.
struct KillOnDrop(Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn wait_for(what: &str, mut cond: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !cond() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Chars a wl_keyboard client received, from wev's output: a pressed key
/// event line, then a continuation line carrying the utf8.
fn typed(wev_output: &str) -> String {
    let mut out = String::new();
    let mut pending = false;
    for line in wev_output.lines() {
        if line.contains("state: 1 (pressed)") {
            pending = true;
        } else if pending && let Some(i) = line.find("utf8: '") {
            let rest = &line[i + 7..];
            if let Some(j) = rest.rfind('\'') {
                out.push_str(&rest[..j]);
            }
            pending = false;
        }
    }
    out
}

/// Reads a child's output on a thread into a shared String.
fn tail(stream: impl std::io::Read + Send + 'static) -> Arc<Mutex<String>> {
    let sink = Arc::new(Mutex::new(String::new()));
    let out = sink.clone();
    std::thread::spawn(move || {
        let mut stream = stream;
        let mut buf = [0u8; 4096];
        while let Ok(n) = stream.read(&mut buf)
            && n > 0
        {
            out.lock()
                .unwrap()
                .push_str(&String::from_utf8_lossy(&buf[..n]));
        }
    });
    sink
}

const TEXT: &str = "Grüße vom Daemon! 42 €";

#[test]
#[ignore = "needs sway + wev and a session bus; run under dbus-run-session"]
fn sway_full_loop() {
    let dir = std::env::temp_dir().join(format!("dictaforge-sway-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let sway_cfg = dir.join("sway.cfg");
    // No bindings, no bars: the compositor only has to host one client.
    std::fs::write(&sway_cfg, "default_border none\n").unwrap();

    // sway picks its own socket name (it does not honor WAYLAND_DISPLAY,
    // that env is for clients) and announces it in its log; the announce
    // line is INFO level, hence -V.
    let mut sway = Command::new("sway")
        .args(["-V", "-c", sway_cfg.to_str().unwrap()])
        .env("WLR_BACKENDS", "headless")
        .env("WLR_LIBINPUT_NO_DEVICES", "1")
        .env_remove("WAYLAND_DISPLAY")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawning sway; is it installed?");
    let sway_log = tail(sway.stderr.take().unwrap());
    let _sway = KillOnDrop(sway);
    const ANNOUNCE: &str = "Running compositor on wayland display '";
    let mut display = String::new();
    wait_for("sway to announce its display", || {
        let log = sway_log.lock().unwrap();
        match log.find(ANNOUNCE).map(|i| &log[i + ANNOUNCE.len()..]) {
            Some(rest) if rest.contains('\'') => {
                display = rest[..rest.find('\'').unwrap()].to_string();
                true
            }
            _ => false,
        }
    });
    let socket =
        PathBuf::from(std::env::var("XDG_RUNTIME_DIR").expect("XDG_RUNTIME_DIR")).join(&display);
    wait_for("the sway socket", || socket.exists());

    // wev opens a toplevel, gets keyboard focus, and logs every key event;
    // a reader thread collects its output while the test drives the daemon.
    // stdbuf: wev block-buffers on a pipe and the events never arrive
    let mut wev = Command::new("stdbuf")
        .args(["-oL", "wev"])
        .env("WAYLAND_DISPLAY", &display)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawning wev; is it installed?");
    let wev_out = tail(wev.stdout.take().unwrap());
    let _wev = KillOnDrop(wev);
    // No wl_keyboard exists until the daemon creates its virtual one, so
    // keyboard enter cannot be the sync point; window activation can.
    wait_for("wev to gain focus", || {
        wev_out.lock().unwrap().contains("activated")
    });

    // Real daemon, default config (HOME points at the temp dir so the
    // user's config.toml cannot change the backend under the test).
    let _daemon = KillOnDrop(
        Command::new(env!("CARGO_BIN_EXE_dictaforged"))
            .env("WAYLAND_DISPLAY", &display)
            .env("HOME", &dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let conn = Connection::session().unwrap();
    let daemon = Daemon1ProxyBlocking::new(&conn).unwrap();
    wait_for("the daemon on the bus", || daemon.status().is_ok());

    let status: serde_json::Value = serde_json::from_str(&daemon.status().unwrap()).unwrap();
    assert_eq!(
        status["backend"], "virtual-keyboard",
        "daemon picked the wrong backend under sway: {status}"
    );

    daemon.inject_text(TEXT).unwrap();
    wait_for("the injected text at the client", || {
        typed(&wev_out.lock().unwrap()) == TEXT
    });
}
