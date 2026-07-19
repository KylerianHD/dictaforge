//! System tray via StatusNotifierItem (ksni). Icon mirrors the daemon
//! state; the menu drives the same command channel as D-Bus and hotkey.

use std::sync::mpsc::Sender;

use crate::daemon::{Cmd, State};

pub struct Tray {
    pub state: State,
    tx: Sender<Cmd>,
}

impl ksni::Tray for Tray {
    fn id(&self) -> String {
        "dictaforge".into()
    }

    fn title(&self) -> String {
        "DictaForge".into()
    }

    fn icon_name(&self) -> String {
        match self.state {
            State::Idle => "audio-input-microphone".into(),
            _ => "media-record".into(),
        }
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::StandardItem;
        vec![
            StandardItem {
                label: "Toggle dictation".into(),
                icon_name: "audio-input-microphone".into(),
                activate: Box::new(|t: &mut Self| {
                    let _ = t.tx.send(Cmd::Toggle);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Quit".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|t: &mut Self| {
                    let _ = t.tx.send(Cmd::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// Start the tray service in the background. Fails when no StatusNotifier
/// host is running; the caller treats that as a warning, not a stop.
pub fn spawn(tx: Sender<Cmd>) -> Result<ksni::blocking::Handle<Tray>, ksni::Error> {
    use ksni::blocking::TrayMethods;
    Tray {
        state: State::Idle,
        tx,
    }
    .spawn()
}
