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

## Open items for M1

- Verify whisper-rs exposes `carry_initial_prompt` and the VAD API; if it lags upstream, decide between a patched fork and waiting (bindings only need bumping, not rewriting).
- Benchmark small vs large-v3-turbo quantizations against the 1.5 s latency bar on a 4-core CPU.
