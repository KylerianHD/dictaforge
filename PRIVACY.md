# Privacy

Short version: your voice and your text stay on your machine.

- Speech recognition and text cleanup run locally on your hardware. No audio or text ever leaves the device by default.
- No telemetry, no analytics, no accounts, no crash reporting. Nothing phones home.
- The default build links no networking code; continuous integration enforces this on every change.
- The network is touched in exactly two cases, both started by you: downloading a speech or cleanup model you asked for, and cloud processing through a provider key you supplied (off by default, separate build feature, clearly labeled).
- Local-only mode hard-blocks all network access in the app, including model downloads.
- The microphone opens only while you dictate: on your hotkey, or on the wake word if you explicitly enable it (off by default).
- Dictation history is stored locally, can be encrypted at rest, and can be turned off entirely. Deleting an entry deletes it.
- The evdev hotkey fallback reads input devices only to match your configured shortcut. It never logs or stores keystrokes.
- The clipboard injection fallback is opt-in because clipboard managers may keep history; OpenFlow warns you before enabling it.

Questions or doubts: open an issue. Claims here are meant to be verifiable from the source.
