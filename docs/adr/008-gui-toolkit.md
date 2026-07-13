# ADR 008: GUI toolkit is GTK4 + libadwaita

Status: accepted (2026-07-13)

## Context

Requirements: Wayland-native, a path to a layer-shell overlay for the recording indicator, small footprint, accessible (keyboard navigation, screen readers), and workable pure-Rust bindings. Candidates: GTK4/libadwaita, Qt6/QML, Slint.

## Decision

GTK4 + libadwaita via gtk4-rs.

- gtk4-rs bindings are mature and first-party maintained; cxx-qt (Qt6) means living in a C++ FFI layer for the whole UI, and Slint is light but weakest on system tray integration, accessibility tooling, and layer-shell precedent.
- gtk4-layer-shell (with existing Rust bindings) covers the overlay on wlroots compositors; elsewhere the overlay degrades to an always-on-top window as planned.
- libadwaita gives a first-run wizard and settings UI that read as native on GNOME and acceptable everywhere, with GTK's screen-reader support (a stated project requirement) for free.

## Consequences

- The GUI process carries the GTK footprint; the daemon does not link any of it (ADR 002), so the idle RSS bar is unaffected.
- Theming on Qt-based desktops (KDE) is the known cosmetic cost; acceptable for a settings window that is rarely open.
- The overlay indicator is implemented against gtk4-layer-shell where available; if that pairing fights us on some compositor, a minimal smithay-client-toolkit surface is the escape hatch for the overlay only.
