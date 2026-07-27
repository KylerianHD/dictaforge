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

    /// Every state transition: idle, recording, transcribing, injecting.
    /// Declared here so introspection advertises it; see `emit_state` for why
    /// nothing calls this generated function.
    #[zbus(signal)]
    async fn state_changed(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        state: &str,
    ) -> zbus::Result<()>;
}

pub const PATH: &str = "/org/dictaforge/Daemon1";
pub const IFACE: &str = "org.dictaforge.Daemon1";

/// Claim the well-known name; the returned connection serves until dropped.
pub fn serve(tx: Sender<Cmd>) -> anyhow::Result<zbus::blocking::Connection> {
    Ok(zbus::blocking::connection::Builder::session()?
        .name(IFACE)?
        .serve_at(PATH, Api { tx })?
        .build()?)
}

/// Broadcast a state transition to whoever is listening, typically the
/// overlay. Best effort: nobody listening, or a bus hiccup, must never
/// interrupt dictation.
///
// ponytail: emitted through the connection rather than the generated
// Api::state_changed, because that one is async and reaching it from the
// blocking event loop needs zbus's doc(hidden) block_on. This is the same
// message on the wire.
pub fn emit_state(conn: &zbus::blocking::Connection, state: &str) {
    let _ = conn.emit_signal(None::<&str>, PATH, IFACE, "StateChanged", &(state,));
}
