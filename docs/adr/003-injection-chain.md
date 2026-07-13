# ADR 003: Injection backend chain with a layout-proof floor

Status: accepted (2026-07-13)

## Context

No single injection mechanism works across all compositors, and the obvious universal one (uinput) is layout-broken in every prior tool except dotool. Details and per-environment table: docs/research/injection-backends.md; prior-art findings: docs/research/injection-tools.md.

## Decision

An `Injector` trait with runtime capability probing, in this order: wlroots virtual-keyboard with our own uploaded keymap (the wtype technique), KWin fake_input, XDG RemoteDesktop portal with libei (keysyms preferred), XTEST with spare-keycode remap on X11, uinput with an xkbcommon reverse keymap, and clipboard paste as opt-in last resort. The reverse keymap autodetects the active layout from the session (not env vars), handles dead keys and AltGr/level-3, falls back per character to Unicode hex input or clipboard for unreachable symbols, and rebuilds on layout-change signals.

## Consequences

- Every backend must pass the same round-trip test suite across 20+ layouts; ydotool's QWERTZ/AZERTY garbage is a named regression test.
- The probe result is cached, user-overridable in config, and reported by `openflow-cli status`.
- The uinput path needs a udev rule or input-group membership; the first-run wizard owns that (guided, polkit-assisted where possible).
