# Roadmap


## M0: Foundations
- [x] Git repo (main + dev), license (GPL-3.0-or-later), .gitignore
- [x] ADR index (docs/adr/README.md)
- [x] Injection backend decision table (docs/research/injection-backends.md)
- [x] Cargo workspace skeleton: dictaforged, dictaforge, dictaforge-cli (crates/* added when the first shared code appears)
- [x] CI: fmt check, clippy -D warnings, tests, cargo-deny, no-network-crates check (byok feature gate)
- [x] Repo hygiene: README, PRIVACY.md, CONTRIBUTING.md, SECURITY.md, CODE_OF_CONDUCT.md, issue/PR templates
- [x] Research notes: wtype/dotool/ydotool/kdotool techniques, whisper.cpp API (docs/research/)
- [x] ADRs 001-007 written, GUI toolkit evaluated and decided (ADR 008: GTK4 + libadwaita)

## M1: Vertical slice (MVP)
- [x] Audio capture via cpal, PipeWire first, 16 kHz mono resample
- [x] Hotkeys: push-to-talk and toggle (evdev tier; portal GlobalShortcuts and compositor keybinds follow per ADR 006)
- [x] whisper.cpp transcription (whisper-rs); model download helper lands with the M2 first-run wizard, until then config points at a model file
- [x] Injector trait with runtime probing: virtual-keyboard (wlroots), XTEST remap, uinput + xkbcommon reverse keymap (fake_input and portal + libei deferred per ADR 003, uinput covers KDE)
- [x] Reverse keymap proven on QWERTZ and Dvorak (round-trip matrix over 21 layouts incl. non-Latin groups)
- [x] Verified end to end on KDE Plasma Wayland (spoken acceptance), wlroots via headless sway in CI (Hyprland stand-in), and X11 under Xvfb
- [x] Tray icon (StatusNotifierItem), TOML config file
- [x] Tag v0.1.0

## M2: Cleanup and UX
- [ ] Local LLM cleanup via llama.cpp: Off / Light / Full
- [ ] VAD hands-free mode (Silero or WebRTC VAD)
- [ ] Recording overlay (wlr-layer-shell, always-on-top window fallback)
- [ ] Settings GUI, first-run wizard (model download, uinput permission, hotkey, mic test), model manager
- [ ] Tag v0.2.0

## M3: Personalization
- [ ] Personal dictionary with auto-learning and transcription biasing
- [ ] Voice snippets
- [ ] Per-app tone/formatting profiles (focused-app detection on Wayland and X11)
- [ ] Local history: search, re-insert, delete, optional encryption, off switch
- [ ] Tag v0.3.0

## M4: Command Mode and 1.0
- [ ] Voice editing of selected text
- [ ] Optional wake word (openWakeWord, off by default)
- [ ] BYOK cloud providers, opt-in only, behind the byok feature flag
- [ ] Packaging: AUR (PKGBUILD, -bin, -git), AppImage, deb/rpm, Nix flake
- [ ] Docs site, quality bars measured and documented
- [ ] Tag v1.0.0

## M5: Custom AI groundwork (post-1.0)
- [ ] Versioned model contract (trait + on-disk manifest), self-hostable model registry support
- [ ] docs/research/custom-ai-feasibility.md and docs/adr/NNN-custom-ai.md
- [ ] If the study says go: scaffold the separate dictaforge-ai repo, starting with the fine-tuned cleanup model

After each milestone: run the QA matrix, update these checkboxes, cut a tag from main.
