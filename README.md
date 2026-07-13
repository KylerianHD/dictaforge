# DictaForge

Privacy-first voice dictation for Linux. Hold a key, speak, and the text lands in whatever field has focus: terminal, browser, IDE, chat. Works across Wayland and X11, on every major desktop, on any keyboard layout.

**Status: pre-alpha.** Nothing usable yet. The plan lives in [ROADMAP.md](ROADMAP.md).


## Goals

- 100% local by default. No telemetry, no accounts, no network calls. See [PRIVACY.md](PRIVACY.md).
- Install to first dictation in under two minutes, without touching a config file.
- Works everywhere: KDE Plasma, GNOME, Hyprland, Sway, COSMIC, XFCE and friends, Wayland or X11, QWERTZ or Dvorak or bépo.
- Light on resources: a Rust daemon, not an 800 MB Electron app.

## Building

```
cargo build --workspace
```

There is nothing to run yet beyond stubs.

## License

GPL-3.0-or-later.
