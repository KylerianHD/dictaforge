//! Pure layout math: reverse-map characters to key presses via xkbcommon.
//! No device or display access lives here; injectors consume the plans.

mod detect;
mod index;

pub use detect::detect;
pub use index::KeymapIndex;

/// XKB layout selection, detected from the session or overridden by config.
/// Serde lets the config file override it; only `layout` is required there.
#[derive(Clone, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub struct LayoutSpec {
    pub layout: String,
    #[serde(default)]
    pub variant: String,
    #[serde(default)]
    pub options: Option<String>,
    #[serde(default)]
    pub group: u32,
}

/// One key press: evdev keycode (xkb keycode minus 8), xkb modifier mask,
/// layout group index.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct KeyPlan {
    pub keycode: u32,
    pub mods: u32,
    pub group: u32,
}

/// How to produce one character on the active layout.
#[derive(Clone, PartialEq, Debug)]
pub enum CharPlan {
    Direct(KeyPlan),
    /// Dead-key composing: press each plan in order, the client composes.
    Sequence(Vec<KeyPlan>),
    Unreachable(char),
}
