//! Hands-free endpointing: decide when the user started and stopped talking,
//! so no hotkey has to.
//!
//! ponytail: energy over frames, not Silero. whisper-rs does expose the
//! standalone VAD context, but it wants a second model file the downloader
//! (Task 3) cannot fetch yet, and its API is batch rather than streaming.
//! Ceiling: loud is not the same as speech, so a noisy room fools this.
//! Upgrade path is in docs/research/whisper-cpp.md.

use std::sync::mpsc::Sender;

use crate::audio::Tap;
use crate::daemon::Cmd;

/// Quiet audio at the start is treated as the room, not as speech.
const CALIBRATION_MS: f32 = 300.0;
/// Speech has to beat the room by this factor.
const MARGIN: f32 = 3.0;
/// A silent room would otherwise make its own hiss look like speech.
const ABS_THRESHOLD: f32 = 0.004;
/// A door slam is loud too; speech has to keep it up this long.
const MIN_SPEECH_MS: f32 = 120.0;
/// How fast the noise floor follows the room while nobody is talking.
const FLOOR_DRIFT: f32 = 0.02;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Endpoint {
    SpeechStart,
    SpeechEnd,
}

pub struct Endpointer {
    rate: f32,
    silence_ms: f32,
    max_utterance_ms: f32,
    floor: f32,
    calibrated_ms: f32,
    speaking: bool,
    /// Loud time accumulated before SpeechStart is worth emitting.
    speech_ms: f32,
    /// Quiet time accumulated since the last loud frame while speaking.
    quiet_ms: f32,
    utterance_ms: f32,
}

impl Endpointer {
    pub fn new(rate: u32, silence_ms: u64, max_utterance_s: u64) -> Self {
        Self {
            rate: rate as f32,
            silence_ms: silence_ms as f32,
            max_utterance_ms: (max_utterance_s * 1000) as f32,
            floor: 0.0,
            calibrated_ms: 0.0,
            speaking: false,
            speech_ms: 0.0,
            quiet_ms: 0.0,
            utterance_ms: 0.0,
        }
    }

    /// Feed the samples captured since the last call. Frame length is free:
    /// the duration comes from the sample count, so a caller polling every
    /// 50 ms and one polling every 20 ms both work.
    pub fn feed(&mut self, frame: &[f32]) -> Option<Endpoint> {
        if frame.is_empty() {
            return None;
        }
        let ms = frame.len() as f32 * 1000.0 / self.rate;
        let rms = rms(frame);

        if self.calibrated_ms < CALIBRATION_MS {
            self.calibrated_ms += ms;
            self.floor = self.floor.max(rms);
            return None;
        }

        let loud = rms > (self.floor * MARGIN).max(ABS_THRESHOLD);
        if self.speaking {
            self.utterance_ms += ms;
            self.quiet_ms = if loud { 0.0 } else { self.quiet_ms + ms };
            if self.quiet_ms >= self.silence_ms || self.utterance_ms >= self.max_utterance_ms {
                self.speaking = false;
                self.speech_ms = 0.0;
                self.quiet_ms = 0.0;
                self.utterance_ms = 0.0;
                return Some(Endpoint::SpeechEnd);
            }
        } else if loud {
            self.speech_ms += ms;
            if self.speech_ms >= MIN_SPEECH_MS {
                self.speaking = true;
                self.utterance_ms = self.speech_ms;
                self.quiet_ms = 0.0;
                return Some(Endpoint::SpeechStart);
            }
        } else {
            self.speech_ms = 0.0;
            // follow the room, so a fan switching on does not become an
            // utterance that never ends
            self.floor = self.floor * (1.0 - FLOOR_DRIFT) + rms * FLOOR_DRIFT;
        }
        None
    }
}

fn rms(frame: &[f32]) -> f32 {
    (frame.iter().map(|s| s * s).sum::<f32>() / frame.len() as f32).sqrt()
}

/// How often the watcher asks the capture stream for new audio. Well under
/// the silence threshold, so the endpointer sees the pause in time.
const POLL: std::time::Duration = std::time::Duration::from_millis(50);

/// Watch the microphone and drive the daemon's normal Start/Stop commands.
/// Hands-free is exactly toggle mode with the endpointer pressing the key,
/// so the state machine needs no new states.
pub fn spawn(tap: Tap, silence_ms: u64, max_utterance_s: u64, tx: Sender<Cmd>) {
    std::thread::spawn(move || {
        let mut endpointer = Endpointer::new(tap.rate(), silence_ms, max_utterance_s);
        let mut mark = tap.now();
        loop {
            std::thread::sleep(POLL);
            let (frame, next) = tap.since(mark);
            mark = next;
            let cmd = match endpointer.feed(&frame) {
                Some(Endpoint::SpeechStart) => Cmd::Start,
                Some(Endpoint::SpeechEnd) => Cmd::Stop,
                None => continue,
            };
            if tx.send(cmd).is_err() {
                return; // daemon gone
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 16_000;
    const FRAME_MS: f32 = 20.0;

    /// One 20 ms frame of a 200 Hz tone at `amp` (0 for room silence, which
    /// still carries a little hiss).
    fn frame(amp: f32, phase: &mut f32) -> Vec<f32> {
        let n = (RATE as f32 * FRAME_MS / 1000.0) as usize;
        (0..n)
            .map(|_| {
                *phase += 2.0 * std::f32::consts::PI * 200.0 / RATE as f32;
                amp * phase.sin()
            })
            .collect()
    }

    /// Feed `ms` of audio at `amp`, collecting endpoints with the ms offset
    /// at which they fired.
    fn feed(
        ep: &mut Endpointer,
        phase: &mut f32,
        ms: f32,
        amp: f32,
        clock: &mut f32,
    ) -> Vec<(f32, Endpoint)> {
        let mut out = Vec::new();
        let frames = (ms / FRAME_MS) as usize;
        for _ in 0..frames {
            *clock += FRAME_MS;
            if let Some(e) = ep.feed(&frame(amp, phase)) {
                out.push((*clock, e));
            }
        }
        out
    }

    const HISS: f32 = 0.002;
    const SPEECH: f32 = 0.3;

    fn endpointer() -> (Endpointer, f32, f32) {
        (Endpointer::new(RATE, 700, 60), 0.0, 0.0)
    }

    #[test]
    fn speech_between_silences_starts_and_ends() {
        let (mut ep, mut phase, mut clock) = endpointer();
        let mut seen = feed(&mut ep, &mut phase, 500.0, HISS, &mut clock);
        seen.extend(feed(&mut ep, &mut phase, 1000.0, SPEECH, &mut clock));
        seen.extend(feed(&mut ep, &mut phase, 1000.0, HISS, &mut clock));

        assert_eq!(
            seen.len(),
            2,
            "expected one start and one end, got {seen:?}"
        );
        assert_eq!(seen[0].1, Endpoint::SpeechStart);
        assert_eq!(seen[1].1, Endpoint::SpeechEnd);
        // start once MIN_SPEECH_MS of the burst is in, end 700 ms into the
        // trailing silence
        assert!(
            (seen[0].0 - (500.0 + MIN_SPEECH_MS)).abs() <= FRAME_MS,
            "start at {} ms",
            seen[0].0
        );
        assert!(
            (seen[1].0 - (1500.0 + 700.0)).abs() <= FRAME_MS,
            "end at {} ms",
            seen[1].0
        );
    }

    /// The regression that would make hands-free unusable: thinking mid
    /// sentence must not cut the utterance in two.
    #[test]
    fn a_pause_shorter_than_the_threshold_does_not_end_the_utterance() {
        let (mut ep, mut phase, mut clock) = endpointer();
        let mut seen = feed(&mut ep, &mut phase, 500.0, HISS, &mut clock);
        seen.extend(feed(&mut ep, &mut phase, 500.0, SPEECH, &mut clock));
        seen.extend(feed(&mut ep, &mut phase, 300.0, HISS, &mut clock));
        seen.extend(feed(&mut ep, &mut phase, 500.0, SPEECH, &mut clock));
        seen.extend(feed(&mut ep, &mut phase, 1000.0, HISS, &mut clock));

        let kinds: Vec<Endpoint> = seen.iter().map(|(_, e)| *e).collect();
        assert_eq!(kinds, [Endpoint::SpeechStart, Endpoint::SpeechEnd]);
        // the end belongs to the second burst, not the 300 ms gap
        assert!(seen[1].0 > 1800.0, "ended at {} ms, too early", seen[1].0);
    }

    #[test]
    fn silence_threshold_is_honored() {
        let mut ep = Endpointer::new(RATE, 200, 60);
        let (mut phase, mut clock) = (0.0, 0.0);
        let mut seen = feed(&mut ep, &mut phase, 500.0, HISS, &mut clock);
        seen.extend(feed(&mut ep, &mut phase, 500.0, SPEECH, &mut clock));
        seen.extend(feed(&mut ep, &mut phase, 500.0, HISS, &mut clock));

        let end = seen
            .iter()
            .find(|(_, e)| *e == Endpoint::SpeechEnd)
            .unwrap();
        assert!(
            (end.0 - (1000.0 + 200.0)).abs() <= FRAME_MS,
            "end at {} ms",
            end.0
        );
    }

    /// A stuck endpointer must not record forever.
    #[test]
    fn max_utterance_forces_an_end() {
        let mut ep = Endpointer::new(RATE, 700, 1);
        let (mut phase, mut clock) = (0.0, 0.0);
        let mut seen = feed(&mut ep, &mut phase, 500.0, HISS, &mut clock);
        seen.extend(feed(&mut ep, &mut phase, 3000.0, SPEECH, &mut clock));

        let end = seen
            .iter()
            .find(|(_, e)| *e == Endpoint::SpeechEnd)
            .unwrap();
        assert!((end.0 - 1500.0).abs() <= FRAME_MS, "end at {} ms", end.0);
    }

    #[test]
    fn a_quiet_room_produces_nothing() {
        let (mut ep, mut phase, mut clock) = endpointer();
        let seen = feed(&mut ep, &mut phase, 5000.0, HISS, &mut clock);
        assert!(seen.is_empty(), "silence produced {seen:?}");
    }

    /// Sine waves are not speech. Real speech has gaps between words that a
    /// too-eager endpointer would call the end of the sentence.
    #[test]
    fn real_speech_is_one_utterance() {
        let bytes = include_bytes!("../tests/fixtures/jfk-16k.wav");
        let speech: Vec<f32> = bytes[44..]
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / i16::MAX as f32)
            .collect();
        let quiet = vec![0.0f32; RATE as usize]; // a second of room at each end
        let clip: Vec<f32> = quiet.iter().chain(&speech).chain(&quiet).copied().collect();

        let mut ep = Endpointer::new(RATE, 700, 60);
        let frame = (RATE as f32 * FRAME_MS / 1000.0) as usize;
        let seen: Vec<Endpoint> = clip
            .chunks(frame)
            .filter_map(|frame| ep.feed(frame))
            .collect();

        assert_eq!(
            seen,
            [Endpoint::SpeechStart, Endpoint::SpeechEnd],
            "the clip should be exactly one utterance"
        );
    }

    /// Acceptance harness for hands-free: opens the real default microphone,
    /// waits for one utterance, and cuts the take exactly the way the daemon
    /// does. Say a sentence and stop talking. Set DICTAFORGE_TEST_MODEL to
    /// see the transcript too.
    ///   cargo test -p dictaforged -- --ignored --nocapture hands_free
    #[test]
    #[ignore = "needs a microphone and someone talking into it"]
    fn hands_free_on_a_live_microphone() {
        let stream = crate::audio::Stream::open(None).expect("microphone");
        let tap = stream.tap();
        let mut ep = Endpointer::new(tap.rate(), 700, 60);
        let mut mark = tap.now();
        let mut utterance = None;
        let start = std::time::Instant::now();
        let mut seen = Vec::new();
        eprintln!("listening on a {} Hz stream, speak", tap.rate());
        while start.elapsed() < std::time::Duration::from_secs(15) && seen.len() < 2 {
            std::thread::sleep(POLL);
            let (frame, next) = tap.since(mark);
            mark = next;
            let Some(endpoint) = ep.feed(&frame) else {
                continue;
            };
            eprintln!("{endpoint:?} at {:.2} s", start.elapsed().as_secs_f32());
            seen.push(endpoint);
            match endpoint {
                // the same reach-backwards the daemon's begin() does
                Endpoint::SpeechStart => utterance = Some(tap.mark()),
                Endpoint::SpeechEnd => {
                    let pcm = tap
                        .take_since(utterance.expect("start came first"))
                        .unwrap();
                    eprintln!("took {:.2} s of audio", pcm.len() as f32 / 16_000.0);
                    if let Ok(model) = std::env::var("DICTAFORGE_TEST_MODEL") {
                        let text = crate::stt::SttEngine::load(std::path::Path::new(&model))
                            .unwrap()
                            .transcribe(&pcm, None)
                            .unwrap();
                        eprintln!("transcript: {text:?}");
                        assert!(!text.is_empty(), "captured audio transcribed to nothing");
                    }
                }
            }
        }
        assert_eq!(seen, [Endpoint::SpeechStart, Endpoint::SpeechEnd]);
    }

    /// Calibration must not treat a loud room as speech forever after.
    #[test]
    fn a_loud_room_raises_the_floor() {
        let (mut ep, mut phase, mut clock) = endpointer();
        let seen = feed(&mut ep, &mut phase, 3000.0, 0.05, &mut clock);
        assert!(seen.is_empty(), "steady room noise produced {seen:?}");
    }
}
