# ADR 007: Cleanup layer on llama.cpp, cloud strictly opt-in

Status: accepted (2026-07-13)

## Context

Filler removal, self-correction handling, and structure formatting need a small instruct model; regex passes cannot do "no wait, Wednesday". Wispr Flow's differentiation lives in exactly this layer. Privacy rules forbid any default network path.

## Decision

`CleanupEngine` trait with three implementations: none (verbatim), local llama.cpp (llama-cpp-2 bindings, 1-3B instruct model, quantized), and BYOK remote (OpenAI-compatible/Anthropic/Ollama endpoints). BYOK compiles only under the `byok` cargo feature, is off by default, and CI asserts no networking crates exist outside that feature. User-facing intensity: Off / Light / Full, implemented as prompt variants.

## Consequences

- Light mode must stay within the latency bar on 4-core CPUs; if the 3B model cannot, Light falls back to fast heuristics (filler stripping, punctuation) and only Full uses the LLM.
- Model download and selection go through the M2 model manager like STT models.
- The cleanup prompt set is a maintained artifact with its own regression fixtures (dictation in, expected text out).
