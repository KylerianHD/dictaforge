# Text injection backend decision table

Every backend must produce correct text regardless of keyboard layout (QWERTZ, AZERTY, Dvorak, Colemak, bepo, non-Latin, dead keys, compose). Backends are probed at startup and per session, the working one is cached, user override lives in config, and `openflow-cli status` reports the active choice.

## Decision table per environment

| Environment | Primary backend | Fallback chain | Notes |
|---|---|---|---|
| Hyprland, Sway, river, Wayfire, LabWC (wlroots family) | zwp_virtual_keyboard_manager_v1 with our own generated XKB keymap uploaded (wtype technique) | uinput + reverse keymap, then clipboard (opt-in) | Layout-proof by construction, full Unicode/CJK. wlr-layer-shell available for the overlay. |
| KDE Plasma Wayland | org.kde.kwin.fake_input where KWin permits it | XDG RemoteDesktop portal + libei (keysyms), then uinput + reverse keymap, then clipboard (opt-in) | KWin does not offer zwp_virtual_keyboard to arbitrary clients. Probe fake_input, expect denial on hardened setups. Portal token is persisted so the dialog appears once. |
| GNOME Wayland | XDG RemoteDesktop portal + libei, keysyms preferred (layout-independent) | uinput + reverse keymap, then clipboard (opt-in) | No layer-shell: overlay falls back to a normal always-on-top window. If EIS only accepts keycodes, apply the reverse keymap before sending. |
| COSMIC | Probe zwp_virtual_keyboard_manager_v1 (Smithay-based, expected present) | Portal + libei, then uinput + reverse keymap, then clipboard | Verify during M1 and update this row with the result. |
| X11 sessions (KDE/GNOME on X11, XFCE, Cinnamon, bare WMs) | XTEST with temporary remap of a spare keycode to the needed keysym, map restored afterwards (xdotool technique) | uinput + reverse keymap, then clipboard (opt-in) | Never naive keycode injection. |
| XWayland apps under any Wayland compositor | Same as the host compositor row: compositor-level injection reaches XWayland clients too | XTEST via Xwayland as a targeted extra (reaches X11 clients only) | |
| IBus or Fcitx5 running (any environment) | Input-method commit-string path, inherently layout-independent (lands v1.0, opt-in at first) | Falls back to the environment row above | Also evaluate zwp_input_method_v2 where compositors expose it. |

## Runtime probe order

1. Wayland compositor advertises zwp_virtual_keyboard_manager_v1: use it with an uploaded custom keymap.
2. KDE: org.kde.kwin.fake_input permitted: use it.
3. XDG RemoteDesktop portal grants a session: libei, keysyms if the EIS implementation supports them.
4. X11 session: XTEST with spare-keycode remap.
5. /dev/uinput writable: uinput with the reverse keymap below.
6. Clipboard paste: only if the user opted in (privacy prompt, clipboard saved and restored, Ctrl+Shift+V for terminals via app profile).

## uinput reverse keymap (the ydotool problem, solved)

ydotool emits raw kernel keycodes assuming US QWERTY, which turns into garbage on any other layout. Our fallback instead:

1. Determine the actual active layout, in priority order: compositor/session query (KDE kxkbrc or D-Bus, GNOME gsettings org.gnome.desktop.input-sources including current index, swaymsg -t get_inputs, hyprctl devices), then org.freedesktop.locale1, then the wl_keyboard keymap if obtainable, then user config override.
2. Compile exactly that keymap (layout + variant + options + active group index) with libxkbcommon.
3. Build a reverse lookup: target character to (kernel keycode, required modifier set, group). Emit modifier press, keycode, release through uinput. Handles Shift, AltGr, level 3, and group switching.
4. Dead keys: emit the composing sequence.
5. Unreachable characters (emoji, CJK on a Latin layout): per-character fallback to IM Unicode hex input (Ctrl+Shift+U) where IBus/fcitx is present, otherwise the clipboard path.
6. Subscribe to layout-change signals and rebuild the cache live.

QA gate: property tests round-trip string to events to xkbcommon state machine and back across 20+ layouts including de, fr, es, dvorak, colemak, bepo, ru, ua, tr, jp.
