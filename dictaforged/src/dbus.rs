//! D-Bus control surface: org.dictaforge.Daemon1 at /org/dictaforge/Daemon1.
//! Task 8 serves it over the stub pipeline (--no-hardware); Task 10 swaps the
//! stub actions for the real recorder, engine, and injectors.

use crate::config::Config;

/// What Start/Stop/InjectText actually do. Stub prints instead of touching
/// hardware so the integration test (and Task 10's state machine tests) can
/// run headless.
pub enum Pipeline {
    // ponytail: Real variant arrives in Task 10
    Stub,
}

impl Pipeline {
    fn backend(&self) -> &str {
        match self {
            Pipeline::Stub => "stub",
        }
    }

    fn inject(&mut self, text: &str) {
        match self {
            Pipeline::Stub => println!("stub inject: {text}"),
        }
    }
}

pub struct Daemon {
    cfg: Config,
    pipeline: Pipeline,
    recording: bool,
}

impl Daemon {
    pub fn new(cfg: Config, pipeline: Pipeline) -> Self {
        Self {
            cfg,
            pipeline,
            recording: false,
        }
    }
}

#[zbus::interface(name = "org.dictaforge.Daemon1")]
impl Daemon {
    fn toggle(&mut self) {
        self.recording = !self.recording;
    }

    fn start(&mut self) {
        self.recording = true;
    }

    fn stop(&mut self) {
        self.recording = false;
    }

    fn status(&self) -> String {
        let layout = match &self.cfg.layout_override {
            Some(spec) => spec.layout.clone(),
            // ponytail: Task 10 reports the actually detected layout here
            None => "auto".into(),
        };
        serde_json::json!({
            "state": if self.recording { "recording" } else { "idle" },
            "backend": self.pipeline.backend(),
            "model": self.cfg.model.display().to_string(),
            "layout": layout,
        })
        .to_string()
    }

    fn inject_text(&mut self, text: &str) {
        self.pipeline.inject(text);
    }
}

/// Claim the well-known name and serve until the process dies.
pub fn serve(daemon: Daemon) -> anyhow::Result<()> {
    let _conn = zbus::blocking::connection::Builder::session()?
        .name("org.dictaforge.Daemon1")?
        .serve_at("/org/dictaforge/Daemon1", daemon)?
        .build()?;
    loop {
        std::thread::park();
    }
}
