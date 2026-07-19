# ADR 006: Global hotkeys in four tiers

Status: accepted (2026-07-13)

## Context

No portable global-hotkey API exists on Linux. Wayland compositors gate input; the portal API covers only the two big desktops.

## Decision

Auto-detected tiers:

1. XDG Desktop Portal GlobalShortcuts (ashpd) on KDE Plasma and GNOME Wayland.
2. Generated/documented compositor keybind snippets calling `dictaforge-cli` (Hyprland, Sway, river and friends).
3. evdev listener as universal fallback: requires input-group membership, matches only the configured chord, never logs anything else. The privacy explanation ships in the first-run wizard, and push-to-talk (key-release detection) needs this tier or tier 1.
4. XGrabKey on X11 sessions.

## Consequences

- Tier 2 cannot do push-to-talk (no release events), only toggle; document that.
- The evdev listener is the most sensitive code in the project: it gets its own review focus and a hard rule against buffering or persisting events.
- Hotkey conflicts surface as desktop notifications with the wizard as the fix path.
