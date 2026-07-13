# ADR 004: STT behind a trait, whisper.cpp as the default engine

Status: accepted (2026-07-13)

## Context

whisper.cpp gives 100+ languages, GGUF model files from tiny to large-v3-turbo, vocabulary biasing via initial prompt, and now built-in Silero VAD (docs/research/whisper-cpp.md). Low-latency English-only engines (Parakeet via sherpa-onnx, Moonshine, Vosk) are attractive later but not required for MVP.

## Decision

An `SttEngine` trait (transcribe a finished utterance; optional streaming partials via callback). whisper-rs/whisper.cpp is the only implementation until v1.0. The trait is the seam where sherpa-onnx and, in M5, custom models plug in; the versioned on-disk model manifest arrives with the model manager in M2, not before.

## Consequences

- Audio pipeline commits to 16 kHz mono f32, whisper.cpp's fixed input format.
- Models lazy-load with a configurable keep-alive to protect the idle RSS bar.
- Dictionary biasing (M3) uses initial_prompt plus post-processing rules; no engine-specific hacks leak above the trait.
