//! Microphone capture producing 16 kHz mono f32 for the STT engine.
//!
//! The capture stream (cpal) collects whatever the device delivers; the pure
//! functions below convert it: interleaved any-rate any-channel-count f32 in,
//! 16 kHz mono out, peak-normalized to -3 dBFS so whispering works.

/// Target rate whisper.cpp expects.
pub const TARGET_RATE: u32 = 16_000;

/// -3 dBFS as linear amplitude.
const PEAK_TARGET: f32 = 0.708;
/// Peaks below this are treated as silence and not amplified (noise gate).
const NOISE_FLOOR: f32 = 0.01;

/// Mix interleaved multi-channel samples to mono and resample to 16 kHz.
pub fn to_mono_16k(samples: &[f32], channels: usize, rate: u32) -> anyhow::Result<Vec<f32>> {
    anyhow::ensure!(channels > 0, "zero channels");
    let mono: Vec<f32> = samples
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect();
    if rate == TARGET_RATE {
        return Ok(mono);
    }

    use audioadapter_buffers::direct::InterleavedSlice;
    use rubato::{Fft, FixedSync, Resampler};
    let mut resampler = Fft::<f32>::new(
        rate as usize,
        TARGET_RATE as usize,
        1024,
        1,
        FixedSync::Input,
    )?;
    let out_len = resampler.process_all_needed_output_len(mono.len());
    let mut out = vec![0f32; out_len];
    let input = InterleavedSlice::new(&mono, 1, mono.len()).map_err(anyhow::Error::msg)?;
    let mut output = InterleavedSlice::new_mut(&mut out, 1, out_len).map_err(anyhow::Error::msg)?;
    let (_, written) = resampler.process_all_into_buffer(&input, &mut output, mono.len(), None)?;
    out.truncate(written);
    Ok(out)
}

/// Peak-normalize to -3 dBFS in place; input quieter than the noise floor is
/// left untouched so silence does not get blown up into full-scale noise.
pub fn normalize(samples: &mut [f32]) {
    let peak = samples.iter().fold(0f32, |m, s| m.max(s.abs()));
    if peak < NOISE_FLOOR {
        return;
    }
    let gain = PEAK_TARGET / peak;
    for s in samples {
        *s *= gain;
    }
}

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use anyhow::{Context, anyhow, bail};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// How much audio the ring keeps. Long enough to cover a whole utterance and
/// short enough that idle RSS does not notice: 10 s of 48 kHz mono f32 is
/// under 2 MB.
const RING_SECONDS: u32 = 10;

/// How far back `mark` reaches. Covers both the frames still in flight when a
/// hotkey is pressed and the syllable the endpointer needed to hear before it
/// could call it speech.
const PREROLL_MS: u32 = 300;

/// A position in the capture stream. Only meaningful to the `Tap` that made it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Mark(u64);

/// Capped mono ring plus a monotonic count of everything ever written, which
/// is what makes marks survive samples being dropped off the front.
struct Ring {
    samples: VecDeque<f32>,
    written: u64,
    cap: usize,
}

impl Ring {
    fn new(cap: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(cap),
            written: 0,
            cap,
        }
    }

    fn push(&mut self, samples: impl Iterator<Item = f32>) {
        let before = self.samples.len();
        self.samples.extend(samples);
        self.written += (self.samples.len() - before) as u64;
        if let Some(excess) = self.samples.len().checked_sub(self.cap) {
            self.samples.drain(..excess);
        }
    }

    /// Everything captured since `mark`, plus a mark for the new end. A mark
    /// older than the ring is clamped to what is still there rather than
    /// failing: dropped audio is better than no audio.
    fn since(&self, mark: Mark) -> (Vec<f32>, Mark) {
        let oldest = self.written - self.samples.len() as u64;
        let start = mark.0.max(oldest) - oldest;
        let out = self.samples.range(start as usize..).copied().collect();
        (out, Mark(self.written))
    }
}

/// Reader side of a capture stream: cloneable and thread-safe, unlike the
/// cpal stream itself, so the hands-free watcher can share it with the
/// daemon's pipeline.
#[derive(Clone)]
pub struct Tap {
    ring: Arc<Mutex<Ring>>,
    rate: u32,
}

impl Tap {
    /// Device rate, not the 16 kHz whisper wants; `since` returns raw frames.
    pub fn rate(&self) -> u32 {
        self.rate
    }

    fn ring(&self) -> std::sync::MutexGuard<'_, Ring> {
        // a panicking cpal callback must not take dictation down with it
        self.ring.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The exact write position, for callers that only want new audio.
    pub fn now(&self) -> Mark {
        Mark(self.ring().written)
    }

    /// The start of an utterance beginning now, which reaches slightly
    /// backwards so the first syllable is not lost.
    pub fn mark(&self) -> Mark {
        let preroll = (self.rate * PREROLL_MS / 1000) as u64;
        Mark(self.ring().written.saturating_sub(preroll))
    }

    /// Raw mono frames at the device rate since `mark`, and the new position.
    pub fn since(&self, mark: Mark) -> (Vec<f32>, Mark) {
        self.ring().since(mark)
    }

    /// The take since `mark`, ready for the STT engine: 16 kHz mono,
    /// peak-normalized.
    pub fn take_since(&self, mark: Mark) -> anyhow::Result<Vec<f32>> {
        let (raw, _) = self.since(mark);
        let mut pcm = to_mono_16k(&raw, 1, self.rate)?;
        normalize(&mut pcm);
        Ok(pcm)
    }
}

/// A microphone held open for the daemon's lifetime. Audio is always
/// flowing, so nothing has to warm up when the user starts talking.
pub struct Stream {
    // dropping this stops capture; nothing else touches it
    _stream: cpal::Stream,
    tap: Tap,
}

impl Stream {
    pub fn tap(&self) -> Tap {
        self.tap.clone()
    }

    /// Open the named input device (default device when None) and start
    /// capturing in its native format.
    pub fn open(device: Option<&str>) -> anyhow::Result<Self> {
        let host = cpal::default_host();
        let device = match device {
            Some(name) => host
                .input_devices()?
                .find(|d| d.to_string() == name)
                .ok_or_else(|| anyhow!("audio input device {name:?} not found"))?,
            None => host
                .default_input_device()
                .ok_or_else(|| anyhow!("no default audio input device"))?,
        };
        let config = device
            .default_input_config()
            .context("no input config; is the microphone busy or unplugged?")?;
        let channels = config.channels() as usize;
        let rate = config.sample_rate();
        let ring = Arc::new(Mutex::new(Ring::new((rate * RING_SECONDS) as usize)));
        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => build_stream::<f32>(&device, config.into(), &ring, channels),
            cpal::SampleFormat::I16 => build_stream::<i16>(&device, config.into(), &ring, channels),
            cpal::SampleFormat::U16 => build_stream::<u16>(&device, config.into(), &ring, channels),
            other => bail!("unsupported sample format {other:?}"),
        }?;
        stream.play()?;
        Ok(Stream {
            _stream: stream,
            tap: Tap { ring, rate },
        })
    }
}

fn build_stream<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    ring: &Arc<Mutex<Ring>>,
    channels: usize,
) -> anyhow::Result<cpal::Stream>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    use cpal::Sample;
    let ring = ring.clone();
    Ok(device.build_input_stream(
        config,
        move |data: &[T], _: &cpal::InputCallbackInfo| {
            // mixed to mono here so the ring holds one stream of samples and
            // marks mean the same thing whatever the device does
            let mono = data.chunks_exact(channels).map(|frame| {
                frame.iter().map(|s| f32::from_sample(*s)).sum::<f32>() / channels as f32
            });
            ring.lock().unwrap_or_else(|e| e.into_inner()).push(mono);
        },
        // ponytail: capture errors just log; the daemon surfaces the ones
        // that matter as notifications when a take comes back empty
        |err| eprintln!("audio stream error: {err}"),
        None,
    )?)
}

/// Names of the available input devices, for the mic-unavailable
/// notification.
pub fn input_devices() -> Vec<String> {
    cpal::default_host()
        .input_devices()
        .map(|devices| devices.map(|d| d.to_string()).collect())
        .unwrap_or_default()
}

/// Write 16 kHz mono f32 as a 16-bit PCM WAV. Debug aid for --dump-wav,
/// kept forever for support.
pub fn write_wav(path: &std::path::Path, samples: &[f32]) -> std::io::Result<()> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend(b"RIFF");
    out.extend((36 + data_len).to_le_bytes());
    out.extend(b"WAVEfmt ");
    out.extend(16u32.to_le_bytes()); // fmt chunk size
    out.extend(1u16.to_le_bytes()); // PCM
    out.extend(1u16.to_le_bytes()); // mono
    out.extend(TARGET_RATE.to_le_bytes());
    out.extend((TARGET_RATE * 2).to_le_bytes()); // byte rate
    out.extend(2u16.to_le_bytes()); // block align
    out.extend(16u16.to_le_bytes()); // bits per sample
    out.extend(b"data");
    out.extend(data_len.to_le_bytes());
    for s in samples {
        out.extend(((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).to_le_bytes());
    }
    std::fs::write(path, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Interleaved stereo sine, both channels identical.
    fn stereo_sine(rate: u32, secs: f32, freq: f32, amp: f32) -> Vec<f32> {
        let frames = (rate as f32 * secs) as usize;
        (0..frames)
            .flat_map(|i| {
                let s = amp * (2.0 * std::f32::consts::PI * freq * i as f32 / rate as f32).sin();
                [s, s]
            })
            .collect()
    }

    #[test]
    fn resamples_48k_stereo_to_16k_mono() {
        let input = stereo_sine(48_000, 1.0, 440.0, 0.5);
        let out = to_mono_16k(&input, 2, 48_000).unwrap();
        // 48000 frames at a 3:1 ratio: allow the resampler a little slack
        // at the edges but the length must be within 1% of 16000.
        let expected = 16_000usize;
        assert!(
            out.len().abs_diff(expected) < expected / 100,
            "expected ~{expected} samples, got {}",
            out.len()
        );
        // A sine stays a sine: peak survives the resample roughly intact.
        let peak = out.iter().fold(0f32, |m, s| m.max(s.abs()));
        assert!((peak - 0.5).abs() < 0.05, "peak {peak} strayed from 0.5");
    }

    #[test]
    fn mono_mix_averages_channels() {
        // L = 0.8, R = -0.4 every frame: average is 0.2.
        let input: Vec<f32> = [0.8f32, -0.4].repeat(1600);
        let out = to_mono_16k(&input, 2, 16_000).unwrap();
        assert_eq!(out.len(), 1600);
        for s in &out {
            assert!((s - 0.2).abs() < 1e-6, "expected 0.2, got {s}");
        }
    }

    #[test]
    fn passthrough_when_already_16k_mono() {
        let input = vec![0.1f32; 800];
        let out = to_mono_16k(&input, 1, 16_000).unwrap();
        assert_eq!(out, input);
    }

    #[test]
    fn whisper_level_input_normalizes_to_minus_3_dbfs() {
        let mut samples: Vec<f32> = (0..16_000)
            .map(|i| 0.05 * (2.0 * std::f32::consts::PI * 220.0 * i as f32 / 16_000.0).sin())
            .collect();
        normalize(&mut samples);
        let peak = samples.iter().fold(0f32, |m, s| m.max(s.abs()));
        assert!((peak - PEAK_TARGET).abs() < 1e-3, "peak {peak}");
    }

    #[test]
    fn ring_returns_exactly_what_arrived_since_the_mark() {
        let mut ring = Ring::new(100);
        ring.push([0.1f32; 10].into_iter());
        let (_, mark) = ring.since(Mark(0));
        ring.push([0.2f32; 25].into_iter());
        let (out, next) = ring.since(mark);
        assert_eq!(out.len(), 25);
        assert!(out.iter().all(|s| *s == 0.2));
        assert_eq!(next, Mark(35));
        // asking again with the new mark yields nothing
        assert!(ring.since(next).0.is_empty());
    }

    #[test]
    fn ring_caps_instead_of_growing() {
        let mut ring = Ring::new(100);
        for _ in 0..50 {
            ring.push([0.1f32; 10].into_iter());
        }
        assert_eq!(ring.samples.len(), 100);
        assert_eq!(ring.written, 500);
    }

    /// A mark older than the ring must yield the surviving audio, not panic
    /// and not an empty take.
    #[test]
    fn a_mark_that_fell_off_the_ring_is_clamped() {
        let mut ring = Ring::new(100);
        ring.push([0.1f32; 400].into_iter());
        let (out, next) = ring.since(Mark(0));
        assert_eq!(out.len(), 100);
        assert_eq!(next, Mark(400));
    }

    #[test]
    fn near_silence_is_not_amplified() {
        let quiet = vec![0.001f32; 1600];
        let mut samples = quiet.clone();
        normalize(&mut samples);
        assert_eq!(samples, quiet);
    }
}
