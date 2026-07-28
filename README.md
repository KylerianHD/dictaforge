# DictaForge

Privacy-first voice dictation for Linux. Hold a key, speak, and the text lands in whatever field has focus: terminal, browser, IDE, chat. Works across Wayland and X11, on every major desktop, on any keyboard layout.

**Status: early alpha (v0.1.0).** The full loop works: hotkey, speech, text in the focused window. No settings GUI yet, so setup means editing a TOML file and putting a whisper model on disk by hand. The plan lives in [ROADMAP.md](ROADMAP.md).


## Goals

- 100% local by default. No telemetry, no accounts, no network calls. See [PRIVACY.md](PRIVACY.md).
- Install to first dictation in under two minutes, without touching a config file.
- Works everywhere: KDE Plasma, GNOME, Hyprland, Sway, COSMIC, XFCE and friends, Wayland or X11, QWERTZ or Dvorak or bépo.
- Light on resources: a Rust daemon, not an 800 MB Electron app.

## Trying it

Prebuilt x86_64 and aarch64 tarballs hang off each
[release](https://github.com/KylerianHD/dictaforge/releases), or build from source:

```
cargo build --workspace --release
```

You need a whisper model and permission to type. The short version:

```
mkdir -p ~/.cache/dictaforge
curl -L -o ~/.cache/dictaforge/ggml-small.bin \
  https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin
sudo usermod -aG input "$USER"   # log out and back in
dictaforged
```

Then hold ctrl+alt+d, speak, and let go. Settings live in
`~/.config/dictaforge/config.toml`; every key is optional. A first-run wizard
that does all of the above for you is the next milestone.

Three ways to start an utterance, set with `mode`:

```
mode = "push_to_talk"   # hold the hotkey (default)
mode = "toggle"         # tap to start, tap to stop
mode = "hands_free"     # no hotkey, it stops when you stop talking
```

Hands-free keeps the microphone open and ends an utterance after
`vad_silence_ms` of quiet (700 by default). It listens for loudness, not for
speech, so a noisy room will trip it.

Known limits today: KDE and GNOME type through uinput, which is what the input
group is for. The large-v3-turbo model needs a GPU to be worth it; small is the
sensible default on CPU.

## License

GPL-3.0-or-later.
