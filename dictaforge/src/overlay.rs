//! Recording overlay: a small indicator driven by the daemon's StateChanged
//! signal.
//!
//! Its own process on purpose (ADR 002): the daemon links no GUI toolkit, so
//! a GTK crash here can never interrupt dictation. The bus watcher runs on a
//! plain thread and hands states to the GTK main loop over an async channel.

use gtk4::prelude::*;
use gtk4::{Align, Application, ApplicationWindow, CssProvider, Label, glib};

const APP_ID: &str = "org.dictaforge.Overlay";

const CSS: &str = "
window.dictaforge-overlay {
    background: alpha(black, 0.82);
    border-radius: 18px;
}
label.dictaforge-overlay {
    color: white;
    font-size: 15px;
    padding: 10px 20px;
}
label.recording { color: #ff6b6b; }
";

pub fn run() -> glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build);
    // our argv is ours, not GTK's
    app.run_with_args::<&str>(&[])
}

fn build(app: &Application) {
    let label = Label::new(None);
    label.add_css_class("dictaforge-overlay");
    label.set_halign(Align::Center);

    let window = ApplicationWindow::builder()
        .application(app)
        .title("DictaForge")
        .decorated(false)
        .resizable(false)
        .child(&label)
        .build();
    window.add_css_class("dictaforge-overlay");

    let css = CssProvider::new();
    // load_from_string would pull in the v4_12 feature and with it a newer
    // minimum GTK, for one call that does the same thing
    css.load_from_data(CSS);
    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &css,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    let (tx, rx) = async_channel::unbounded();
    std::thread::spawn(move || watch_daemon(&tx));

    glib::spawn_future_local(async move {
        while let Ok(state) = rx.recv().await {
            match state.as_str() {
                "recording" => show(&window, &label, "Recording", true),
                "transcribing" => show(&window, &label, "Transcribing", false),
                // injecting is over in milliseconds; flashing a third label
                // just makes the overlay strobe
                _ => window.set_visible(false),
            }
        }
    });
}

fn show(window: &ApplicationWindow, label: &Label, text: &str, recording: bool) {
    label.set_text(text);
    label.set_css_classes(if recording {
        &["dictaforge-overlay", "recording"]
    } else {
        &["dictaforge-overlay"]
    });
    window.present();
}

/// Follow the daemon forever: reconnect when it restarts, and hide the
/// overlay whenever the connection drops so it cannot get stuck on screen.
fn watch_daemon(tx: &async_channel::Sender<String>) {
    loop {
        if let Err(e) = follow(tx) {
            eprintln!("dictaforge: daemon watch ended: {e}");
        }
        if tx.send_blocking("idle".into()).is_err() {
            return; // UI gone, so are we
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}

fn follow(tx: &async_channel::Sender<String>) -> zbus::Result<()> {
    let conn = zbus::blocking::Connection::session()?;
    let daemon = Daemon1ProxyBlocking::new(&conn)?;

    // subscribe before asking for the current state, otherwise a transition
    // landing between the two is lost
    let states = daemon.receive_state_changed()?;
    if let Some(state) = current_state(&daemon)
        && tx.send_blocking(state).is_err()
    {
        return Ok(());
    }

    for signal in states {
        if tx.send_blocking(signal.args()?.state.to_string()).is_err() {
            return Ok(());
        }
    }
    Ok(())
}

/// The daemon may already be recording when the overlay starts.
fn current_state(daemon: &Daemon1ProxyBlocking<'_>) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(&daemon.status().ok()?).ok()?;
    Some(json.get("state")?.as_str()?.to_string())
}

#[zbus::proxy(
    interface = "org.dictaforge.Daemon1",
    default_service = "org.dictaforge.Daemon1",
    default_path = "/org/dictaforge/Daemon1"
)]
trait Daemon1 {
    fn status(&self) -> zbus::Result<String>;

    #[zbus(signal)]
    fn state_changed(&self, state: &str) -> zbus::Result<()>;
}
