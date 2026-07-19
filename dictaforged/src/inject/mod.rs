//! Text injection backends. Each wraps the pure keymap plans in a transport
//! (uinput now; wlroots virtual keyboard and XTEST follow).

mod uinput;
mod virtual_keyboard;
mod xtest;

pub use uinput::UinputInjector;
pub use virtual_keyboard::VirtualKeyboardInjector;
pub use xtest::XtestInjector;

use std::fmt;

#[derive(Debug)]
pub enum InjectError {
    /// Backend cannot run here; the message says which permission or
    /// environment fixes it.
    Unavailable(&'static str),
    /// These characters cannot be produced on the active layout; nothing
    /// was injected.
    Unreachable(Vec<char>),
    Io(std::io::Error),
}

impl fmt::Display for InjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InjectError::Unavailable(why) => write!(f, "backend unavailable: {why}"),
            InjectError::Unreachable(chars) => {
                write!(f, "not typeable on the active layout: {chars:?}")
            }
            InjectError::Io(err) => write!(f, "injection failed: {err}"),
        }
    }
}

impl std::error::Error for InjectError {}

pub trait Injector {
    fn name(&self) -> &'static str;
    /// Cheap availability check; the manager caches the result.
    fn probe(&mut self) -> Result<(), InjectError>;
    fn inject(&mut self, text: &str) -> Result<(), InjectError>;
}

pub struct InjectorManager;

impl InjectorManager {
    /// Probe order per docs/research/injection-backends.md:
    /// virtual_keyboard, then xtest on X11 sessions, then uinput. An
    /// override probes only that backend so its real error surfaces.
    pub fn pick(
        override_: Option<&str>,
        spec: &crate::keymap::LayoutSpec,
    ) -> anyhow::Result<Box<dyn Injector>> {
        if let Some(name) = override_
            && !["virtual_keyboard", "xtest", "uinput"].contains(&name)
        {
            anyhow::bail!(
                "unknown backend_override {name:?}; valid: virtual_keyboard, xtest, uinput"
            );
        }
        let on_x11 = std::env::var("XDG_SESSION_TYPE").as_deref() == Ok("x11");
        let mut failures = Vec::new();
        for name in ["virtual_keyboard", "xtest", "uinput"] {
            match override_ {
                Some(wanted) if wanted != name => continue,
                None if name == "xtest" && !on_x11 => continue,
                _ => {}
            }
            let mut injector: Box<dyn Injector> = match name {
                "virtual_keyboard" => Box::new(VirtualKeyboardInjector::new()),
                "xtest" => Box::new(XtestInjector::new()),
                _ => match crate::keymap::KeymapIndex::from_spec(spec) {
                    Ok(index) => Box::new(UinputInjector::new(index)),
                    Err(e) => {
                        failures.push(format!("uinput: keymap: {e}"));
                        continue;
                    }
                },
            };
            match injector.probe() {
                Ok(()) => return Ok(injector),
                Err(e) => failures.push(format!("{name}: {e}")),
            }
        }
        anyhow::bail!("no injection backend available: {}", failures.join("; "))
    }
}
