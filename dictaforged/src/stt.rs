//! Speech-to-text over whisper.cpp (whisper-rs). The seam ADR 004 names;
//! no trait until a second engine exists (ponytail: trait when needed).
//! Lazy load and keep-alive live in the daemon (Task 10), not here.

use std::path::Path;

/// Below one second whisper.cpp produces garbage or asserts; treat as silence.
const MIN_SAMPLES: usize = crate::audio::TARGET_RATE as usize;

/// True when the take is too short to transcribe (returns "" instead).
fn too_short(pcm16k: &[f32]) -> bool {
    pcm16k.len() < MIN_SAMPLES
}

/// Encoder states to compute for a take. whisper.cpp always pads audio to a
/// 30 s window and runs the encoder over all 1500 states unless audio_ctx
/// caps it; 1500 states / 30 s = 50 per second, so cap at the real audio
/// length plus headroom. This is what brings a 5 s utterance from ~20 s to
/// ~1 s on CPU. Floor of 128 because tiny contexts degrade accuracy.
fn audio_ctx_for(samples: usize) -> i32 {
    let secs = samples as f32 / crate::audio::TARGET_RATE as f32;
    ((secs * 50.0).ceil() as i32 + 32).clamp(128, 1500)
}

pub struct SttEngine {
    state: whisper_rs::WhisperState,
}

impl SttEngine {
    pub fn load(model: &Path) -> anyhow::Result<Self> {
        // Route whisper.cpp's chatty stderr through the log crate (silent
        // without a logger); idempotent, cheap to call on every load.
        whisper_rs::install_logging_hooks();
        let ctx = whisper_rs::WhisperContext::new_with_params(
            model,
            whisper_rs::WhisperContextParameters::default(),
        )
        .map_err(|e| anyhow::anyhow!("loading whisper model {model:?}: {e}"))?;
        let state = ctx.create_state()?;
        Ok(Self { state })
    }

    /// Transcribe 16 kHz mono f32. `lang` is an ISO 639-1 code; None lets
    /// whisper auto-detect. Empty or sub-second input returns Ok("").
    pub fn transcribe(&mut self, pcm16k: &[f32], lang: Option<&str>) -> anyhow::Result<String> {
        if too_short(pcm16k) {
            return Ok(String::new());
        }
        let mut params =
            whisper_rs::FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 1 });
        params.set_no_context(true);
        // default is 4 threads; use what the machine has (latency bar, Task 11)
        params.set_n_threads(std::thread::available_parallelism().map_or(4, |n| n.get() as i32));
        params.set_language(lang.or(Some("auto")));
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_suppress_nst(true);
        params.set_audio_ctx(audio_ctx_for(pcm16k.len()));
        self.state.full(params, pcm16k)?;
        let mut text = String::new();
        for segment in self.state.as_iter() {
            let seg = segment.to_str_lossy()?.into_owned();
            // annotations like [BLANK_AUDIO] still slip past suppress_nst
            // on silence-only segments; never type them
            if seg.trim().starts_with('[') && seg.trim().ends_with(']') {
                continue;
            }
            text.push_str(&seg);
        }
        Ok(text.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Model-backed round trip; run with
    /// DICTAFORGE_TEST_MODEL=~/.cache/dictaforge/ggml-tiny.en.bin \
    ///   cargo test -p dictaforged -- --ignored stt
    #[test]
    #[ignore = "needs a whisper model; set DICTAFORGE_TEST_MODEL"]
    fn transcribes_jfk_fixture() {
        let model = std::env::var("DICTAFORGE_TEST_MODEL").expect("DICTAFORGE_TEST_MODEL");
        let mut engine = SttEngine::load(Path::new(&model)).unwrap();

        // Fixture is already 16 kHz mono 16-bit PCM; skip the 44-byte header.
        let bytes = include_bytes!("../tests/fixtures/jfk-16k.wav");
        let pcm: Vec<f32> = bytes[44..]
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / i16::MAX as f32)
            .collect();

        let text = engine.transcribe(&pcm, Some("en")).unwrap().to_lowercase();
        assert!(
            text.contains("ask not what your country can do for you"),
            "unexpected transcript: {text:?}"
        );

        // Short-input guard, checked here where a loaded engine exists.
        assert_eq!(engine.transcribe(&[], None).unwrap(), "");
        assert_eq!(engine.transcribe(&pcm[..8000], None).unwrap(), "");
    }

    /// Latency measurement for docs/research/latency-m1.md; prints timings.
    /// Run via scripts/latency.sh (release build, pinned to 4 cores).
    #[test]
    #[ignore = "needs a whisper model; set DICTAFORGE_TEST_MODEL"]
    fn latency_10s_utterance() {
        let model = std::env::var("DICTAFORGE_TEST_MODEL").expect("DICTAFORGE_TEST_MODEL");
        let mut engine = SttEngine::load(Path::new(&model)).unwrap();

        // 10 s utterance: the 6 s fixture plus its first 4 s again.
        let bytes = include_bytes!("../tests/fixtures/jfk-16k.wav");
        let mut pcm: Vec<f32> = bytes[44..]
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / i16::MAX as f32)
            .collect();
        let four_secs = pcm[..4 * crate::audio::TARGET_RATE as usize].to_vec();
        pcm.extend(four_secs);

        for run in 1..=2 {
            let t = std::time::Instant::now();
            let text = engine.transcribe(&pcm, Some("en")).unwrap();
            println!(
                "latency model={model} threads={} audio={:.1}s run{run}={:.2}s text={text:?}",
                std::thread::available_parallelism().map_or(4, |n| n.get()),
                pcm.len() as f32 / 16000.0,
                t.elapsed().as_secs_f32(),
            );
            assert!(!text.is_empty(), "empty transcript");
        }
    }

    #[test]
    fn audio_ctx_scales_with_take_length() {
        let rate = crate::audio::TARGET_RATE as usize;
        assert_eq!(audio_ctx_for(rate), 128); // 1 s hits the floor
        assert_eq!(audio_ctx_for(10 * rate), 532); // 10 s: 500 + headroom
        assert_eq!(audio_ctx_for(60 * rate), 1500); // never above the window
    }

    #[test]
    fn empty_and_sub_second_input_is_gated() {
        assert!(too_short(&[]));
        assert!(too_short(&vec![0.0; 15_999]));
        assert!(!too_short(&vec![0.0; 16_000]));
    }
}
