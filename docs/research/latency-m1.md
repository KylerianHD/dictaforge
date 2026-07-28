# M1 latency measurement

Bar (ROADMAP): under 1.5 s from hotkey release to injected text for a 10 s
utterance with small or large-v3-turbo on a 4-core CPU.

## The fix that made it possible

whisper.cpp pads every take to a 30 s window and, by default, runs its
encoder over all 1500 audio-context states regardless of how much of the
window is real audio. That made every dictation cost the same as 30 s of
speech: about 20 s wall time with small during the Task 10 acceptance.

1500 states cover 30 s, so 50 states describe one second. Capping
`audio_ctx` at `ceil(seconds * 50) + 32` (floor 128, tiny contexts degrade
accuracy) makes the encoder cost proportional to the actual take. See
`audio_ctx_for` in `dictaforged/src/stt.rs`. Transcript quality on the
fixture is unchanged; the spoken acceptance below confirms real dictation.

## Numbers

10 s utterance (the 6 s jfk fixture plus its first 4 s again), release
build, warm engine (second run of two; first runs match within noise unless
noted). Machine: 20-core desktop CPU; the 4-thread rows are pinned with
`taskset -c 0-3` to model the bar's 4-core machine. Reproduce with
`scripts/latency.sh` (2026-07-19).

| Model          | Threads | Transcription  |
|----------------|---------|----------------|
| tiny.en        | 4       | 0.35 s         |
| small          | 4       | **1.45-1.63 s** (run-to-run spread) |
| small          | 20      | 1.25 s         |
| large-v3-turbo | 4       | 7.0-7.3 s      |
| large-v3-turbo | 20      | 3.84 s (8.25 s cold) |

The bar asks for small OR turbo: **small on 4 threads sits at the bar,
1.45-1.63 s depending on run** (background load on the shared desktop
moves it; a dedicated 4-core box should sit at the low end). Down from
~20 s before the audio_ctx cap, so the order-of-magnitude problem is
solved; the remaining margin is tuning, tracked below. large-v3-turbo does
not fit 4 CPU cores; it needs GPU offload (M2+ scope) or a bigger machine.

## End-to-end budget

Hotkey release to injected text is transcription plus two fixed costs:

- ~~300 ms release grace~~ removed in M2 Task 2, see below.
- uinput typing pace, 10 ms per chord (KWin drops events when flooded):
  a 100-char transcript takes ~1 s to type, but that is visible progress
  on screen, not dead waiting, so it is not counted against the bar.

## M2 update, 2026-07-28: the grace is gone

The 300 ms grace existed because the capture stream started cold at hotkey
press and was dropped at release, so whatever the device had not delivered
yet was lost. M2 Task 2 replaced it with a stream held open for the
daemon's lifetime writing into a ring buffer: `stop_recording` now reads
what is already there instead of waiting for it. Residual tail loss is one
cpal buffer period, tens of milliseconds, and the take still ends with the
half second of appended silence whisper likes.

Re-measured on the same machine and method after the rework, and the
transcription path itself is untouched, so the numbers only confirm no
regression:

| Model          | Threads | Transcription  |
|----------------|---------|----------------|
| tiny.en        | 4       | 0.32-0.33 s    |
| small          | 4       | **1.38-1.41 s** |
| large-v3-turbo | 4       | 5.94-5.96 s    |

**small on 4 threads is now under the 1.5 s bar end to end**, 1.4 s rather
than the 1.7-1.9 s of M1, because the only fixed cost left is negligible.
That is the first time the bar holds without an asterisk. turbo still needs
GPU offload.

The same rework also fixed the opposite end: `Tap::mark` reaches 300 ms
backwards into the ring, so the syllable spoken while the hotkey was still
travelling is in the take rather than lost.

## Method notes

- Measured by the `#[ignore]`d test `stt::tests::latency_10s_utterance`
  (prints model, threads, audio length, per-run wall time).
- Always measure release builds: whisper.cpp compiled without optimization
  is 10x+ slower and produced the misleading ~30 s numbers first seen in
  Task 10.
- tiny.en hallucinates extra repetitions on the looped fixture; its row is
  a speed reference, not a quality claim.
