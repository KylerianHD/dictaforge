//! Text injection backends. Each wraps the pure keymap plans in a transport
//! (uinput now; wlroots virtual keyboard and XTEST follow).

mod uinput;
mod virtual_keyboard;
mod xtest;

pub use uinput::{EventSink, UinputInjector};
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
