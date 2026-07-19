//! D-Bus control surface: org.dictaforge.Daemon1 at /org/dictaforge/Daemon1.
//! A thin shim that forwards every call onto the daemon's command channel;
//! the state lives in daemon::Daemon.

use std::sync::mpsc::{Sender, channel};

use crate::daemon::Cmd;

pub struct Api {
    tx: Sender<Cmd>,
}

#[zbus::interface(name = "org.dictaforge.Daemon1")]
impl Api {
    fn toggle(&self) {
        let _ = self.tx.send(Cmd::Toggle);
    }

    fn start(&self) {
        let _ = self.tx.send(Cmd::Start);
    }

    fn stop(&self) {
        let _ = self.tx.send(Cmd::Stop);
    }

    fn status(&self) -> String {
        let (reply, rx) = channel();
        if self.tx.send(Cmd::Status(reply)).is_err() {
            return "{}".into();
        }
        rx.recv().unwrap_or_else(|_| "{}".into())
    }

    fn inject_text(&self, text: &str) {
        // wait for the ack so callers (and the integration test) know the
        // text went out before this returns
        let (ack, rx) = channel();
        if self.tx.send(Cmd::InjectText(text.into(), ack)).is_ok() {
            let _ = rx.recv();
        }
    }
}

/// Claim the well-known name; the returned connection serves until dropped.
pub fn serve(tx: Sender<Cmd>) -> anyhow::Result<zbus::blocking::Connection> {
    Ok(zbus::blocking::connection::Builder::session()?
        .name("org.dictaforge.Daemon1")?
        .serve_at("/org/dictaforge/Daemon1", Api { tx })?
        .build()?)
}
