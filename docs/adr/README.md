# Architecture Decision Records

One file per major decision: `NNN-slug.md`. Status: proposed, accepted, or superseded.

| NNN | Title | Status |
|-----|-------|--------|
| [001](001-language-rust.md) | Language: Rust, C/C++ via FFI only per component when bindings fall short | accepted |
| [002](002-process-split.md) | Three processes: openflowd daemon, openflow GUI, openflow-cli, D-Bus between them | accepted |
| [003](003-injection-chain.md) | Text injection backend chain with a layout-proof floor | accepted |
| [004](004-stt-abstraction.md) | STT behind a trait, whisper.cpp default | accepted |
| [005](005-audio-capture.md) | Audio capture through cpal, PipeWire first | accepted |
| [006](006-hotkey-tiers.md) | Global hotkeys in four tiers | accepted |
| [007](007-cleanup-llm.md) | Cleanup layer on llama.cpp, BYOK behind feature flag | accepted |
| [008](008-gui-toolkit.md) | GUI toolkit: GTK4 + libadwaita | accepted |
| 009 | Packaging: AppImage primary, AUR, deb/rpm, Nix flake; no Flatpak | write-up due in M4, direction fixed |
| 010 | Privacy enforcement: local-only mode hard block | partially realized in CI (no-network check); write-up due with local-only mode in M2 |
| 011 | Custom AI model contract and self-hostable registry | deferred to M5 |
