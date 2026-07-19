# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-07-19

The vertical slice: hold a hotkey, speak, release, and the words appear in
the focused window. 100% local.

### Added
- Dictation daemon (dictaforged): one event loop owning the whole pipeline; push-to-talk and toggle hotkey modes over evdev.
- Audio capture via cpal in the device's native format, mixed and resampled to 16 kHz mono with peak normalization, plus a `--dump-wav` debug flag.
- Transcription via whisper.cpp (whisper-rs): greedy sampling, language auto-detect or pinned, audio context capped to the take length (a 10 s utterance transcribes in ~1.5 s with the small model on 4 threads).
- Layout-aware reverse keymap: any typeable character resolves to key chords, dead-key sequences, or a clear unreachable error; round-trip tested over 21 layouts including non-Latin group fallback.
- Active layout detection: KDE, GNOME, Hyprland, sway, locale1, override.
- Three injection backends with runtime probing: wlroots virtual keyboard (own uploaded keymap, any Unicode), XTEST with spare-keycode remap for X11, uinput as the universal fallback.
- Tray icon (StatusNotifierItem), desktop notifications for every failure path, TOML config at ~/.config/dictaforge/config.toml.
- D-Bus control interface org.dictaforge.Daemon1 and dictaforge-cli (toggle, start, stop, status, type).
- CI integration jobs: full daemon loop on headless sway, XTEST under Xvfb across three layouts, model-backed STT round trip.
- Latency measurement script and research notes (docs/research/latency-m1.md).
- Project foundations: roadmap, ADR index, text injection backend research.
- Cargo workspace with dictaforged, dictaforge, and dictaforge-cli stubs.
- CI: format and clippy checks, test matrix (Ubuntu x86_64/aarch64, Arch, Fedora), cargo-deny, and a check that no networking crates enter the default build.
- README, PRIVACY, CONTRIBUTING, SECURITY, code of conduct, and issue/PR templates.
- Research notes on wtype/dotool/ydotool/kdotool internals and the whisper.cpp API.
- ADRs 001 through 008: language, process split, injection chain, STT abstraction, audio capture, hotkey tiers, cleanup LLM, GUI toolkit (GTK4 + libadwaita).
