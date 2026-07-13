# ADR 001: Language is Rust

Status: accepted (2026-07-13)

## Context

The core is a long-running daemon with hands on uinput, audio capture, and Wayland sockets. Memory bugs here mean corrupted input events or a crashed session component. The ecosystem coverage for this exact problem is unusually good in Rust: whisper-rs, cpal, xkbcommon, wayland-client/smithay-client-toolkit, zbus, ashpd, input-linux/evdev, x11rb, wl-clipboard-rs, llama-cpp-2.

## Decision

Rust for everything. Where a binding is genuinely insufficient, that one component may wrap C/C++ over FFI; the project language does not change.

## Consequences

- Single static-ish binaries, small idle footprint (the sub-50 MB RSS bar is realistic; an Electron equivalent ships hundreds of MB).
- unsafe blocks are expected at the uinput/FFI edges and get reviewed as such.
- Contributors need Rust, which is an acceptable filter for a systems tool.
