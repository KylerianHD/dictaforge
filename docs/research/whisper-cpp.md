# whisper.cpp API notes

Read from `include/whisper.h` (upstream master, 2026-07-13). Line numbers refer to that revision. We consume this through the whisper-rs crate; the C API is what matters for capability planning.

## Core flow

- `whisper_init_from_file_with_params` loads a GGUF/ggml model once; `whisper_full` (or `whisper_full_with_state` for thread separation) runs transcription on 16 kHz mono f32 PCM. `WHISPER_SAMPLE_RATE` is fixed at 16000 (line 33), which pins our audio pipeline's resample target.
- Results come back as segments: `whisper_full_n_segments` and `whisper_full_get_segment_text`.
- The `no_state` init variant plus per-job `whisper_state` lets one loaded model serve sequential jobs cheaply, which fits the lazy-load-with-keep-alive daemon design.

## Parameters that map to our features

- `initial_prompt` (line 527) and `carry_initial_prompt` (528): vocabulary biasing for the personal dictionary (M3). Prompting is the cheap first tier; post-processing replacement rules cover what prompting misses.
- `single_segment` (498) and `new_segment_callback` (562): the streaming/partial-results path for the optional live overlay.
- `no_context` (496): fresh decode per utterance, avoids contamination between dictations.
- `detect_language` (534) and `language = "auto"`: the 100+ language story including code switching between utterances.
- `token_timestamps` (505): available if per-word timing is ever needed; not needed for MVP.

## Built-in VAD (important find)

whisper.cpp now embeds Silero VAD:

- Inline: `whisper_full_params.vad`, `vad_model_path`, `vad_params` (lines 587-590) filter silence during transcription, and `whisper_full_n_vad_segments` etc. expose what was detected.
- Standalone: `whisper_vad_context` with `whisper_vad_init_from_file_with_params` (line 711) runs VAD without a transcription model loaded.

Consequence: hands-free end-of-utterance detection (M2) can use the standalone VAD context instead of pulling in an ONNX runtime for Silero. One inference stack fewer. The WebRTC-VAD option stays on the table only if the Silero model's footprint bothers the idle RSS bar; the VAD model is small, so it likely does not.

### Verdict, M2 Task 2 (2026-07-28)

whisper-rs 0.16 **does** expose the standalone API, in `src/whisper_vad.rs`: `WhisperVadContext::new(model_path, params)`, `detect_speech(&[f32])`, `probabilities() -> &[f32]`, `segments_from_samples(params, &[f32])`, all backed by `whisper_vad_init_from_file_with_params` in whisper-rs-sys 0.15. The bindings do not lag upstream, so no fork was ever on the table.

Hands-free still shipped on an energy endpointer (`dictaforged/src/vad.rs`). Two reasons the plan did not foresee:

1. **It needs a second model file the user cannot get yet.** `WhisperVadContext::new` takes a path to `ggml-silero-*.bin`. The downloader that would fetch it is Task 3, and the whole point of Task 2 is that hands-free works the moment the mode is set. Shipping a mode that errors out until a later milestone is worse than shipping a simpler detector.
2. **The API is batch, not streaming.** There is no "feed me the next 50 ms" entry point; `detect_speech` runs the model over a whole buffer and rewrites the probability array each call. Live endpointing means re-running it over a trailing window every tick, which works but is a strictly heavier way to answer "has the user stopped talking".

The energy endpointer is roughly forty lines of pure Rust, no model, no inference, and unit-testable against synthetic frames. Its ceiling is real: it loses to Silero in a noisy room, because loud is not the same as speech.

Upgrade path, when Task 3 can fetch models and someone reports the noisy-room failure: keep `Endpointer`'s shape, add a Silero implementation that pushes the trailing second through `detect_speech` and reads the tail of `probabilities()`, and pick between them on a config key. Nothing in the daemon or the audio stream has to move.

## Open items for M1

- Verify whisper-rs exposes `carry_initial_prompt` and the VAD API; if it lags upstream, decide between a patched fork and waiting (bindings only need bumping, not rewriting).
- Benchmark small vs large-v3-turbo quantizations against the 1.5 s latency bar on a 4-core CPU.
