//! Daemon core: one event loop owning the pipeline, driven by hotkey and
//! D-Bus commands over a channel. Idle -> Recording -> Transcribing ->
//! Injecting -> Idle; every failure becomes a notification, never a panic.

use std::path::PathBuf;
use std::sync::mpsc::Sender;

use crate::audio::{self, RecordingHandle};
use crate::config::{Config, Mode};
use crate::hotkey::HotkeyEvent;
use crate::inject::Injector;
use crate::stt::SttEngine;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum State {
    Idle,
    Recording,
    Transcribing,
    Injecting,
}

impl State {
    pub fn name(self) -> &'static str {
        match self {
            State::Idle => "idle",
            State::Recording => "recording",
            State::Transcribing => "transcribing",
            State::Injecting => "injecting",
        }
    }
}

/// Everything that can happen to the daemon, from any thread.
pub enum Cmd {
    Hotkey(HotkeyEvent),
    Toggle,
    Start,
    Stop,
    /// The ack lets D-Bus callers block until the text actually went out.
    InjectText(String, Sender<()>),
    Status(Sender<String>),
    Quit,
}

/// What the state machine drives. Stub records nothing and prints instead of
/// typing, so --no-hardware and the unit tests run headless.
pub enum Pipeline {
    Stub {
        transcript: String,
        injected: Vec<String>,
    },
    Real(Box<Real>),
}

pub struct Real {
    audio_device: Option<String>,
    language: Option<String>,
    model: PathBuf,
    layout: String,
    injector: Box<dyn Injector>,
    // ponytail: lazy so a missing model only bites when dictation is used
    stt: Option<SttEngine>,
    recording: Option<RecordingHandle>,
}

impl Pipeline {
    pub fn stub() -> Self {
        Pipeline::Stub {
            transcript: "stub transcript".into(),
            injected: Vec::new(),
        }
    }

    pub fn real(cfg: &Config, layout: String, injector: Box<dyn Injector>) -> Self {
        Pipeline::Real(Box::new(Real {
            audio_device: cfg.audio_device.clone(),
            language: cfg.language.clone(),
            model: cfg.model.clone(),
            layout,
            injector,
            stt: None,
            recording: None,
        }))
    }

    fn backend(&self) -> &str {
        match self {
            Pipeline::Stub { .. } => "stub",
            Pipeline::Real(r) => r.injector.name(),
        }
    }

    fn start_recording(&mut self) -> anyhow::Result<()> {
        match self {
            Pipeline::Stub { .. } => Ok(()),
            Pipeline::Real(r) => {
                r.recording = Some(audio::Recorder::start(r.audio_device.as_deref())?);
                Ok(())
            }
        }
    }

    fn stop_recording(&mut self) -> anyhow::Result<Vec<f32>> {
        match self {
            // one fake second so the too-short guard does not kick in
            Pipeline::Stub { .. } => Ok(vec![0.05; audio::TARGET_RATE as usize]),
            Pipeline::Real(r) => {
                // ponytail: grace period so frames still in flight at hotkey
                // release arrive; a persistent stream is the M2 fix if this
                // is not enough
                std::thread::sleep(std::time::Duration::from_millis(300));
                let handle = r
                    .recording
                    .take()
                    .ok_or_else(|| anyhow::anyhow!("not recording"))?;
                let mut pcm = handle.stop()?;
                audio::normalize(&mut pcm);
                // trailing silence keeps whisper from dropping a final word
                // that was cut off mid-breath
                pcm.extend(std::iter::repeat_n(0.0, audio::TARGET_RATE as usize / 2));
                Ok(pcm)
            }
        }
    }

    fn transcribe(&mut self, pcm: &[f32]) -> anyhow::Result<String> {
        match self {
            Pipeline::Stub { transcript, .. } => Ok(transcript.clone()),
            Pipeline::Real(r) => {
                let stt = match &mut r.stt {
                    Some(stt) => stt,
                    None => r.stt.insert(SttEngine::load(&r.model).map_err(|e| {
                        anyhow::anyhow!("cannot load STT model from {}: {e}", r.model.display())
                    })?),
                };
                stt.transcribe(pcm, r.language.as_deref())
            }
        }
    }

    fn inject(&mut self, text: &str) -> anyhow::Result<()> {
        match self {
            Pipeline::Stub { injected, .. } => {
                // stdout print is what the dbus integration test asserts on
                println!("stub inject: {text}");
                injected.push(text.to_string());
                Ok(())
            }
            Pipeline::Real(r) => Ok(r.injector.inject(text)?),
        }
    }
}

pub struct Daemon {
    cfg: Config,
    pipeline: Pipeline,
    state: State,
    tray: Option<ksni::blocking::Handle<crate::tray::Tray>>,
}

impl Daemon {
    pub fn new(cfg: Config, pipeline: Pipeline) -> Self {
        Self {
            cfg,
            pipeline,
            state: State::Idle,
            tray: None,
        }
    }

    /// Process one command; returns false when the daemon should quit.
    pub fn handle(&mut self, cmd: Cmd) -> bool {
        match cmd {
            Cmd::Hotkey(HotkeyEvent::Pressed) => match self.cfg.mode {
                Mode::PushToTalk => self.begin(),
                Mode::Toggle => self.toggle(),
            },
            Cmd::Hotkey(HotkeyEvent::Released) => {
                if self.cfg.mode == Mode::PushToTalk {
                    self.finish();
                }
            }
            Cmd::Toggle => self.toggle(),
            Cmd::Start => self.begin(),
            Cmd::Stop => self.finish(),
            Cmd::InjectText(text, ack) => {
                if let Err(e) = self.pipeline.inject(&text) {
                    notify("Injection failed", &e.to_string());
                }
                let _ = ack.send(());
            }
            Cmd::Status(reply) => {
                let _ = reply.send(self.status_json());
            }
            Cmd::Quit => return false,
        }
        true
    }

    fn toggle(&mut self) {
        match self.state {
            State::Idle => self.begin(),
            State::Recording => self.finish(),
            _ => {}
        }
    }

    fn begin(&mut self) {
        if self.state != State::Idle {
            return;
        }
        match self.pipeline.start_recording() {
            Ok(()) => self.set_state(State::Recording),
            Err(e) => notify(
                "Microphone unavailable",
                &format!(
                    "{e}\navailable inputs: {}",
                    audio::input_devices().join(", ")
                ),
            ),
        }
    }

    /// Recording -> Transcribing -> Injecting -> Idle, or a notification.
    fn finish(&mut self) {
        if self.state != State::Recording {
            return;
        }
        self.set_state(State::Transcribing);
        let pcm = match self.pipeline.stop_recording() {
            Ok(pcm) => pcm,
            Err(e) => {
                notify("Dictation failed", &e.to_string());
                self.set_state(State::Idle);
                return;
            }
        };
        let text = match self.pipeline.transcribe(&pcm) {
            Ok(text) => text,
            Err(e) => {
                notify("Transcription failed", &e.to_string());
                self.set_state(State::Idle);
                return;
            }
        };
        if !text.is_empty() {
            self.set_state(State::Injecting);
            if let Err(e) = self.pipeline.inject(&text) {
                notify("Injection failed", &e.to_string());
            }
        }
        self.set_state(State::Idle);
    }

    fn set_state(&mut self, state: State) {
        self.state = state;
        if let Some(tray) = &self.tray {
            tray.update(|t| t.state = state);
        }
    }

    fn status_json(&self) -> String {
        let layout = match (&self.pipeline, &self.cfg.layout_override) {
            (Pipeline::Real(r), _) => r.layout.clone(),
            (Pipeline::Stub { .. }, Some(spec)) => spec.layout.clone(),
            (Pipeline::Stub { .. }, None) => "auto".into(),
        };
        serde_json::json!({
            "state": self.state.name(),
            "backend": self.pipeline.backend(),
            "model": self.cfg.model.display().to_string(),
            "layout": layout,
        })
        .to_string()
    }
}

/// Notify the user and log to stderr. Best effort: a missing notification
/// daemon must never stop dictation.
pub fn notify(summary: &str, body: &str) {
    eprintln!("dictaforged: {summary}: {body}");
    #[cfg(not(test))]
    let _ = notify_rust::Notification::new()
        .summary(summary)
        .body(body)
        .icon("audio-input-microphone")
        .show();
}

/// Wire everything up and run the event loop until Quit. With `hardware`
/// the hotkey listener and tray come up too (both soft-fail to a warning);
/// without it (--no-hardware) only the D-Bus surface is served.
pub fn run(cfg: Config, pipeline: Pipeline, hardware: bool) -> anyhow::Result<()> {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut daemon = Daemon::new(cfg, pipeline);

    if hardware {
        let chord = crate::hotkey::Chord::parse(&daemon.cfg.hotkey)?;
        let (htx, hrx) = std::sync::mpsc::channel();
        match crate::hotkey::spawn(chord, htx) {
            Ok(n) => eprintln!("dictaforged: watching {n} keyboard(s)"),
            Err(e) => notify("Hotkey unavailable", &e.to_string()),
        }
        let hotkey_tx = tx.clone();
        std::thread::spawn(move || {
            for event in hrx {
                if hotkey_tx.send(Cmd::Hotkey(event)).is_err() {
                    return;
                }
            }
        });

        match crate::tray::spawn(tx.clone()) {
            Ok(handle) => daemon.tray = Some(handle),
            // headless or no StatusNotifier host; dictation works without it
            Err(e) => eprintln!("dictaforged: no tray: {e}"),
        }
    }

    let _conn = crate::dbus::serve(tx)?;
    while daemon.handle(rx.recv()?) {}
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn daemon(mode: Mode) -> Daemon {
        let cfg = Config {
            mode,
            ..Config::default()
        };
        Daemon::new(cfg, Pipeline::stub())
    }

    fn injected(d: &Daemon) -> &[String] {
        match &d.pipeline {
            Pipeline::Stub { injected, .. } => injected,
            Pipeline::Real(_) => unreachable!(),
        }
    }

    #[test]
    fn push_to_talk_records_then_dictates() {
        let mut d = daemon(Mode::PushToTalk);
        d.handle(Cmd::Hotkey(HotkeyEvent::Pressed));
        assert_eq!(d.state, State::Recording);
        d.handle(Cmd::Hotkey(HotkeyEvent::Released));
        assert_eq!(d.state, State::Idle);
        assert_eq!(injected(&d), ["stub transcript"]);
    }

    #[test]
    fn toggle_mode_ignores_release_and_stops_on_second_press() {
        let mut d = daemon(Mode::Toggle);
        d.handle(Cmd::Hotkey(HotkeyEvent::Pressed));
        d.handle(Cmd::Hotkey(HotkeyEvent::Released));
        assert_eq!(d.state, State::Recording);
        d.handle(Cmd::Hotkey(HotkeyEvent::Pressed));
        assert_eq!(d.state, State::Idle);
        assert_eq!(injected(&d), ["stub transcript"]);
    }

    #[test]
    fn dbus_start_stop_cycle() {
        let mut d = daemon(Mode::PushToTalk);
        d.handle(Cmd::Start);
        assert_eq!(d.state, State::Recording);
        d.handle(Cmd::Start); // already recording: no-op
        assert_eq!(d.state, State::Recording);
        d.handle(Cmd::Stop);
        assert_eq!(d.state, State::Idle);
        assert_eq!(injected(&d).len(), 1);
    }

    #[test]
    fn stop_when_idle_is_a_noop() {
        let mut d = daemon(Mode::PushToTalk);
        d.handle(Cmd::Stop);
        assert_eq!(d.state, State::Idle);
        assert!(injected(&d).is_empty());
    }

    #[test]
    fn toggle_cmd_flips_recording() {
        let mut d = daemon(Mode::PushToTalk);
        d.handle(Cmd::Toggle);
        assert_eq!(d.state, State::Recording);
        d.handle(Cmd::Toggle);
        assert_eq!(d.state, State::Idle);
    }

    #[test]
    fn empty_transcript_injects_nothing() {
        let mut d = daemon(Mode::PushToTalk);
        if let Pipeline::Stub { transcript, .. } = &mut d.pipeline {
            transcript.clear();
        }
        d.handle(Cmd::Start);
        d.handle(Cmd::Stop);
        assert_eq!(d.state, State::Idle);
        assert!(injected(&d).is_empty());
    }

    #[test]
    fn inject_text_reaches_the_pipeline_and_acks() {
        let mut d = daemon(Mode::PushToTalk);
        let (ack, rx) = std::sync::mpsc::channel();
        d.handle(Cmd::InjectText("Grüße".into(), ack));
        rx.recv().expect("acked");
        assert_eq!(injected(&d), ["Grüße"]);
    }

    #[test]
    fn status_reports_state_backend_model_layout() {
        let mut d = daemon(Mode::PushToTalk);
        let (tx, rx) = std::sync::mpsc::channel();
        d.handle(Cmd::Status(tx));
        let status: serde_json::Value = serde_json::from_str(&rx.recv().unwrap()).unwrap();
        assert_eq!(status["state"], "idle");
        assert_eq!(status["backend"], "stub");
        assert_eq!(status["layout"], "auto");
        assert!(status["model"].as_str().unwrap().contains("ggml"));

        d.handle(Cmd::Start);
        let (tx, rx) = std::sync::mpsc::channel();
        d.handle(Cmd::Status(tx));
        let status: serde_json::Value = serde_json::from_str(&rx.recv().unwrap()).unwrap();
        assert_eq!(status["state"], "recording");
    }

    #[test]
    fn quit_ends_the_loop() {
        let mut d = daemon(Mode::PushToTalk);
        assert!(d.handle(Cmd::Toggle));
        assert!(!d.handle(Cmd::Quit));
    }
}
