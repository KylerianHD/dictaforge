# Prior art: wtype, dotool, ydotool, kdotool

Read from source on 2026-07-13 (shallow clones of upstream master). What each does, and what we take or fix.

## wtype (C, ~550 lines, wlroots virtual keyboard)

- Speaks `zwp_virtual_keyboard_manager_v1`. Builds its own keymap incrementally: each character is converted with `xkb_utf32_to_keysym`, appended to a keycode table (`main.c`, `get_key_code_by_wchar`), and the whole map is regenerated as `xkb_keymap` source text and uploaded via the protocol's keymap fd before each batch of key events (`upload_keymap`).
- Because the compositor interprets our keycodes against our uploaded map, the user's layout is irrelevant. Full Unicode works, including CJK.
- Also supports named keysyms and modifier press/release, which we need for Ctrl+V in the clipboard fallback.
- Take: the whole technique, as backend 1. Batch uploads instead of per-character re-uploads.

## dotool (Go, uinput + xkbcommon)

- The only prior tool that solves the layout problem on uinput. Compiles a keymap with xkbcommon from `DOTOOL_XKB_LAYOUT`/`XKB_DEFAULT_LAYOUT` env vars, then reverse-maps char to keysym to (keycode, modifiers) and emits through uinput (`dotool.go` around line 358).
- Handles dead keys: `getDeadChords` emits the two-chord composing sequence.
- Gaps we must close: layout comes only from env vars the user has to set by hand (we auto-detect from the session, see injection-backends.md); unreachable characters just print "impossible character for layout" and are dropped (we fall back to Unicode hex input or clipboard); no reaction to live layout switching.
- Take: reverse-map + dead-key approach. Fix: layout autodetection, unreachable-char fallback, layout-change signals.

## ydotool (C, uinput, the cautionary tale)

- `Client/tool_stdin.c` contains a hardcoded `ascii2keycode_map[128]`: ASCII to US-QWERTY kernel keycodes. Non-ASCII input is impossible and any non-US layout produces garbage. This is the exact failure mode our reverse keymap exists to prevent.
- Take: nothing but the daemon/socket split idea (uinput fd held by a privileged daemon). We already run a daemon, so ours holds the uinput handle itself.

## kdotool (Rust, KWin D-Bus scripting)

- Not an input tool at all: it generates KWin scripts and loads them over D-Bus (`org.kde.KWin` `/Scripting`, `src/main.rs` around line 699). Window management only; zero key injection code (`templates.rs` contains none).
- Take: the KWin scripting channel is our best route for focused-window detection on Plasma Wayland (per-app profiles, M3). Not part of the injection chain.

## Consequences for our Injector design

1. Backend 1 (wlroots) is a straight port of wtype's technique into a persistent daemon: keep the virtual keyboard object alive, cache the uploaded map, extend it lazily.
2. The uinput fallback is dotool plus the three fixes above.
3. ydotool's flaw is a regression test: typing "z" on a QWERTZ layout and "e" with an AZERTY layout must produce exactly those characters through every backend.
4. kdotool informs M3 app detection on KDE, nothing in M1.
