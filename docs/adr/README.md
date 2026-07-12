# Architecture Decision Records

One file per major decision: `NNN-slug.md`. Status: proposed, accepted, or superseded.

| NNN | Title | Status |
|-----|-------|--------|
| 001 | Language: Rust (C/C++ via FFI only per component when bindings fall short) | accepted, write-up pending |
| 002 | Process split: openflowd daemon + openflow GUI + openflow-cli, D-Bus API between them | accepted, write-up pending |
| 003 | Text injection backend chain and layout-proof strategy | accepted, table in docs/research/injection-backends.md, write-up pending |
| 004 | STT engine abstraction: whisper.cpp default, sherpa-onnx/Parakeet/Vosk pluggable | accepted, write-up pending |
| 005 | Audio capture: cpal, PipeWire native first, Pulse/ALSA fallback | accepted, write-up pending |
| 006 | Global hotkey tiers: portal GlobalShortcuts, compositor keybinds, evdev, XGrabKey | accepted, write-up pending |
| 007 | Cleanup layer: llama.cpp local default, BYOK remote behind byok feature flag, never on by default | accepted, write-up pending |
| 008 | GUI toolkit: GTK4/libadwaita vs Qt6/QML vs Slint | pending evaluation in M0 |
| 009 | Packaging: AppImage primary universal, AUR, deb/rpm, Nix flake; no Flatpak | accepted, write-up pending |
| 010 | Privacy enforcement: local-only mode hard block + CI check that no network crates link outside byok | accepted, write-up pending |
| 011 | Custom AI model contract and self-hostable registry | deferred to M5 |
