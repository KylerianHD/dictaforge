# Roadmap

OpenFlow is a working name. It collides with the ONF OpenFlow SDN protocol; rename decision is tracked in TODO.md.

## M0: Foundations
- [x] Git repo (main + dev), license (GPL-3.0-or-later), .gitignore
- [x] ADR index (docs/adr/README.md)
- [x] Injection backend decision table (docs/research/injection-backends.md)
- [ ] Cargo workspace skeleton: openflowd, openflow, openflow-cli, crates/*
- [ ] CI: fmt check, clippy -D warnings, tests, cargo-deny, no-network-crates check (byok feature gate)
- [ ] Repo hygiene: README, PRIVACY.md, CONTRIBUTING.md, SECURITY.md, CODE_OF_CONDUCT.md, issue/PR templates
- [ ] Research notes: wtype/dotool/ydotool/kdotool techniques, whisper.cpp API (docs/research/)
- [ ] ADRs 001-007 written, GUI toolkit evaluated and decided (ADR 008)

## M1: Vertical slice (MVP)
- [ ] Audio capture via cpal, PipeWire first, 16 kHz mono resample
- [ ] Hotkeys: push-to-talk and toggle (portal GlobalShortcuts, compositor keybind docs, evdev fallback, XGrabKey)
- [ ] whisper.cpp transcription (whisper-rs) with model download helper
- [ ] Injector trait with runtime probing: virtual-keyboard (wlroots), fake_input (KDE), portal RemoteDesktop + libei, XTEST remap, uinput + xkbcommon reverse keymap
- [ ] Reverse keymap proven on QWERTZ and Dvorak (property tests)
- [ ] Verified end to end on Hyprland, KDE Plasma Wayland, and X11
- [ ] Tray icon (StatusNotifierItem), TOML config file
- [ ] Tag v0.1.0

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
- [ ] If the study says go: scaffold the separate openflow-ai repo, starting with the fine-tuned cleanup model

After each milestone: run the QA matrix, update these checkboxes, cut a tag from main.
