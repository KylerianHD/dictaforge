//! User configuration, TOML at ~/.config/dictaforge/config.toml.
//! Missing file means defaults; a broken file is an error the user must see.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::keymap::LayoutSpec;

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub hotkey: String,
    pub mode: Mode,
    pub model: PathBuf,
    pub language: Option<String>,
    pub backend_override: Option<String>,
    pub audio_device: Option<String>,
    pub layout_override: Option<LayoutSpec>,
    /// Hands-free only: how much quiet ends an utterance.
    pub vad_silence_ms: u64,
    /// Hands-free only: hard stop, so a stuck endpointer cannot record until
    /// the disk fills.
    pub vad_max_utterance_s: u64,
}

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    PushToTalk,
    Toggle,
    /// No hotkey: the endpointer decides when an utterance starts and ends.
    HandsFree,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hotkey: "ctrl+alt+d".into(),
            mode: Mode::PushToTalk,
            model: home().join(".cache/dictaforge/ggml-small.bin"),
            language: None,
            backend_override: None,
            audio_device: None,
            layout_override: None,
            vad_silence_ms: 700,
            vad_max_utterance_s: 60,
        }
    }
}

fn home() -> PathBuf {
    // ponytail: env::home_dir is fine on Linux, the only supported OS
    #[allow(deprecated)]
    std::env::home_dir().unwrap_or_default()
}

impl Config {
    /// Default location: $XDG_CONFIG_HOME/dictaforge/config.toml.
    pub fn default_path() -> PathBuf {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home().join(".config"))
            .join("dictaforge/config.toml")
    }

    /// Load from `path`; a missing file yields the defaults.
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).map_err(|e| anyhow::anyhow!("parsing {path:?}: {e}")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(anyhow::anyhow!("reading {path:?}: {e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_toml() {
        let cfg = Config {
            hotkey: "super+d".into(),
            mode: Mode::Toggle,
            model: PathBuf::from("/models/ggml-small.bin"),
            language: Some("de".into()),
            backend_override: Some("uinput".into()),
            audio_device: Some("front-mic".into()),
            layout_override: Some(LayoutSpec {
                layout: "de".into(),
                variant: "neo".into(),
                options: None,
                group: 0,
            }),
            vad_silence_ms: 900,
            vad_max_utterance_s: 30,
        };
        let text = toml::to_string(&cfg).unwrap();
        assert_eq!(toml::from_str::<Config>(&text).unwrap(), cfg);
    }

    #[test]
    fn missing_file_yields_defaults() {
        let cfg = Config::load(std::path::Path::new("/nonexistent/config.toml")).unwrap();
        assert_eq!(cfg, Config::default());
        assert_eq!(cfg.hotkey, "ctrl+alt+d");
        assert_eq!(cfg.mode, Mode::PushToTalk);
        assert!(cfg.language.is_none());
        assert!(cfg.layout_override.is_none());
    }

    #[test]
    fn partial_file_keeps_defaults_for_the_rest() {
        let dir = std::env::temp_dir().join("dictaforge-config-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("partial.toml");
        std::fs::write(&path, "hotkey = \"super+space\"\nmode = \"toggle\"\n").unwrap();
        let cfg = Config::load(&path).unwrap();
        assert_eq!(cfg.hotkey, "super+space");
        assert_eq!(cfg.mode, Mode::Toggle);
        assert_eq!(cfg.model, Config::default().model);
    }

    #[test]
    fn broken_file_is_an_error_not_silent_defaults() {
        let dir = std::env::temp_dir().join("dictaforge-config-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("broken.toml");
        std::fs::write(&path, "mode = \"warp_speed\"\n").unwrap();
        assert!(Config::load(&path).is_err());
    }
}
