//! Pure layout math: reverse-map characters to key presses via xkbcommon.
//! No device or display access lives here; injectors consume the plans.

mod index;

pub use index::KeymapIndex;

/// XKB layout selection, detected from the session or overridden by config.
pub struct LayoutSpec {
    pub layout: String,
    pub variant: String,
    pub options: Option<String>,
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
