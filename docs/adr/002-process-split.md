# ADR 002: Three processes, D-Bus between them

Status: accepted (2026-07-13)

## Context

Dictation must keep working when no window is open, and power users want to bind compositor keys directly to actions. GUI toolkits drag in dependencies the daemon must not pay for at idle.

## Decision

- `openflowd`: headless daemon owning hotkeys, audio, VAD, STT, cleanup, and injection. Exposes a D-Bus service on the session bus (via zbus).
- `openflow`: GUI for settings, history, and the first-run wizard. A pure D-Bus client; crashing it never interrupts dictation.
- `openflow-cli`: thin D-Bus client for scripting (start/stop/toggle/type/status), which is also how compositor-native keybinds integrate.

## Consequences

- The daemon has zero GUI dependencies, protecting the idle RSS bar.
- The D-Bus interface becomes a public API surface: document it, version it conservatively.
- Autostart is the daemon only (XDG autostart plus a systemd user unit).
