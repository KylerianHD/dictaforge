//! Global hotkey via evdev. Reads /dev/input keyboards without grabbing,
//! matches only the configured chord, and drops every other event on the
//! same line it arrives (ADR 006: never buffer or log non-chord keys).

use std::sync::mpsc;

use evdev::KeyCode;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum HotkeyEvent {
    Pressed,
    Released,
}

/// A parsed hotkey like "ctrl+alt+d": modifier slots plus one main key.
/// Each modifier slot is satisfied by its left or right physical key.
#[derive(Clone, PartialEq, Debug)]
pub struct Chord {
    mods: Vec<[KeyCode; 2]>,
    key: KeyCode,
}

impl Chord {
    /// Parse "ctrl+alt+d" style: named modifiers (ctrl, alt, shift, super),
    /// last token is the key by evdev name (d, z, f5, space, ...).
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        let tokens: Vec<String> = s.split('+').map(|t| t.trim().to_lowercase()).collect();
        let (key, mod_names) = tokens
            .split_last()
            .filter(|(k, _)| !k.is_empty())
            .ok_or_else(|| anyhow::anyhow!("empty hotkey {s:?}; expected e.g. \"ctrl+alt+d\""))?;
        let mods = mod_names
            .iter()
            .map(|name| match name.as_str() {
                "ctrl" => Ok([KeyCode::KEY_LEFTCTRL, KeyCode::KEY_RIGHTCTRL]),
                "alt" => Ok([KeyCode::KEY_LEFTALT, KeyCode::KEY_RIGHTALT]),
                "shift" => Ok([KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_RIGHTSHIFT]),
                "super" | "meta" => Ok([KeyCode::KEY_LEFTMETA, KeyCode::KEY_RIGHTMETA]),
                other => Err(anyhow::anyhow!("unknown modifier {other:?} in {s:?}")),
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let key = format!("KEY_{}", key.to_uppercase())
            .parse::<KeyCode>()
            .map_err(|_| anyhow::anyhow!("unknown key {key:?} in {s:?}"))?;
        Ok(Self { mods, key })
    }
}

/// Pure chord state machine, one per input device. Feed it key events,
/// get Pressed when the full chord goes down, Released when it breaks.
#[derive(Clone, Debug)]
pub struct Matcher {
    chord: Chord,
    held: Vec<KeyCode>,
    active: bool,
}

impl Matcher {
    pub fn new(chord: Chord) -> Self {
        Self {
            chord,
            held: Vec::new(),
            active: false,
        }
    }

    /// value: 1 press, 0 release, 2 autorepeat (suppressed).
    /// Non-chord keys return None without being stored anywhere.
    pub fn on_event(&mut self, code: KeyCode, value: i32) -> Option<HotkeyEvent> {
        let relevant = code == self.chord.key || self.chord.mods.iter().any(|m| m.contains(&code));
        if !relevant || value == 2 {
            return None;
        }
        match value {
            1 if !self.held.contains(&code) => self.held.push(code),
            0 => self.held.retain(|&c| c != code),
            _ => {}
        }
        let complete = self.held.contains(&self.chord.key)
            && self
                .chord
                .mods
                .iter()
                .all(|m| m.iter().any(|c| self.held.contains(c)));
        match (self.active, complete) {
            (false, true) => {
                self.active = true;
                Some(HotkeyEvent::Pressed)
            }
            (true, false) => {
                self.active = false;
                Some(HotkeyEvent::Released)
            }
            _ => None,
        }
    }
}

/// Scan /dev/input for keyboards and watch each on its own thread.
/// Returns the number of keyboards watched; zero is an error (usually
/// missing input-group permission).
pub fn spawn(chord: Chord, tx: mpsc::Sender<HotkeyEvent>) -> anyhow::Result<usize> {
    // ponytail: scanned once at startup; hotplug rescan when someone needs it
    let keyboards: Vec<_> = evdev::enumerate()
        .filter(|(_, d)| {
            d.supported_keys()
                .is_some_and(|keys| keys.contains(KeyCode::KEY_A))
        })
        .collect();
    if keyboards.is_empty() {
        anyhow::bail!("no readable keyboards under /dev/input; add the user to the input group");
    }
    let count = keyboards.len();
    for (path, mut device) in keyboards {
        let mut matcher = Matcher::new(chord.clone());
        let tx = tx.clone();
        std::thread::spawn(move || {
            loop {
                let events = match device.fetch_events() {
                    Ok(events) => events,
                    // device unplugged; the other threads keep watching
                    Err(e) => {
                        eprintln!("hotkey: dropping {}: {e}", path.display());
                        return;
                    }
                };
                for event in events {
                    if let evdev::EventSummary::Key(_, code, value) = event.destructure()
                        && let Some(hk) = matcher.on_event(code, value)
                        && tx.send(hk).is_err()
                    {
                        return; // receiver gone, daemon shutting down
                    }
                }
            }
        });
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    const D: KeyCode = KeyCode::KEY_D;
    const LCTRL: KeyCode = KeyCode::KEY_LEFTCTRL;
    const RCTRL: KeyCode = KeyCode::KEY_RIGHTCTRL;
    const LALT: KeyCode = KeyCode::KEY_LEFTALT;
    const RALT: KeyCode = KeyCode::KEY_RIGHTALT;

    fn feed(m: &mut Matcher, events: &[(KeyCode, i32)]) -> Vec<HotkeyEvent> {
        events
            .iter()
            .filter_map(|&(code, value)| m.on_event(code, value))
            .collect()
    }

    fn ctrl_alt_d() -> Matcher {
        Matcher::new(Chord::parse("ctrl+alt+d").unwrap())
    }

    #[test]
    fn parses_ctrl_alt_d() {
        let chord = Chord::parse("ctrl+alt+d").unwrap();
        assert_eq!(chord.key, D);
        assert_eq!(chord.mods, vec![[LCTRL, RCTRL], [LALT, RALT]]);
    }

    #[test]
    fn parses_super_z_case_insensitive() {
        let chord = Chord::parse("Super+Z").unwrap();
        assert_eq!(chord.key, KeyCode::KEY_Z);
        assert_eq!(
            chord.mods,
            vec![[KeyCode::KEY_LEFTMETA, KeyCode::KEY_RIGHTMETA]]
        );
    }

    #[test]
    fn rejects_trailing_plus_and_unknown_keys() {
        assert!(Chord::parse("ctrl+").is_err());
        assert!(Chord::parse("ctrl+bogus").is_err());
        assert!(Chord::parse("").is_err());
    }

    #[test]
    fn full_chord_presses_and_releases() {
        let mut m = ctrl_alt_d();
        let out = feed(&mut m, &[(LCTRL, 1), (LALT, 1), (D, 1), (D, 0)]);
        assert_eq!(out, vec![HotkeyEvent::Pressed, HotkeyEvent::Released]);
    }

    #[test]
    fn modifier_order_does_not_matter() {
        let mut m = ctrl_alt_d();
        let out = feed(&mut m, &[(LALT, 1), (LCTRL, 1), (D, 1)]);
        assert_eq!(out, vec![HotkeyEvent::Pressed]);
    }

    #[test]
    fn right_hand_modifiers_count() {
        let mut m = ctrl_alt_d();
        let out = feed(&mut m, &[(RCTRL, 1), (RALT, 1), (D, 1)]);
        assert_eq!(out, vec![HotkeyEvent::Pressed]);
    }

    #[test]
    fn autorepeat_is_suppressed() {
        let mut m = ctrl_alt_d();
        let out = feed(
            &mut m,
            &[(LCTRL, 1), (LALT, 1), (D, 1), (D, 2), (D, 2), (D, 0)],
        );
        assert_eq!(out, vec![HotkeyEvent::Pressed, HotkeyEvent::Released]);
    }

    #[test]
    fn partial_chord_emits_nothing() {
        let mut m = ctrl_alt_d();
        let out = feed(&mut m, &[(LCTRL, 1), (D, 1), (D, 0), (LCTRL, 0)]);
        assert_eq!(out, vec![]);
    }

    #[test]
    fn other_keys_do_not_break_an_active_chord() {
        let mut m = ctrl_alt_d();
        let out = feed(
            &mut m,
            &[
                (LCTRL, 1),
                (LALT, 1),
                (D, 1),
                (KeyCode::KEY_S, 1),
                (KeyCode::KEY_S, 0),
                (D, 0),
            ],
        );
        assert_eq!(out, vec![HotkeyEvent::Pressed, HotkeyEvent::Released]);
    }

    #[test]
    fn releasing_a_modifier_first_releases_once() {
        let mut m = ctrl_alt_d();
        let out = feed(&mut m, &[(LCTRL, 1), (LALT, 1), (D, 1), (LALT, 0), (D, 0)]);
        assert_eq!(out, vec![HotkeyEvent::Pressed, HotkeyEvent::Released]);
    }

    /// End-to-end through the kernel: a uinput virtual keyboard types the
    /// chord and spawn() must report it. Run with
    ///   cargo test -p dictaforged -- --ignored hotkey
    #[test]
    #[ignore = "creates a uinput virtual keyboard; needs /dev/uinput access"]
    fn spawn_sees_chord_from_a_virtual_keyboard() {
        use std::time::Duration;

        let mut keys = evdev::AttributeSet::<KeyCode>::new();
        for k in [KeyCode::KEY_A, LCTRL, LALT, D] {
            keys.insert(k);
        }
        let mut kbd = evdev::uinput::VirtualDevice::builder()
            .unwrap()
            .name("dictaforge-hotkey-test")
            .with_keys(&keys)
            .unwrap()
            .build()
            .unwrap();
        // give udev a moment to create the event node before the scan
        std::thread::sleep(Duration::from_millis(500));

        let (tx, rx) = mpsc::channel();
        let n = spawn(Chord::parse("ctrl+alt+d").unwrap(), tx).unwrap();
        assert!(n >= 1, "virtual keyboard not found by the scan");

        let press = |kbd: &mut evdev::uinput::VirtualDevice, code: KeyCode, value: i32| {
            kbd.emit(&[*evdev::KeyEvent::new(code, value)]).unwrap();
        };
        press(&mut kbd, LCTRL, 1);
        press(&mut kbd, LALT, 1);
        press(&mut kbd, D, 1);
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(2)).unwrap(),
            HotkeyEvent::Pressed
        );
        press(&mut kbd, D, 0);
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(2)).unwrap(),
            HotkeyEvent::Released
        );
    }

    #[test]
    fn chord_can_fire_again_after_release() {
        let mut m = ctrl_alt_d();
        let out = feed(&mut m, &[(LCTRL, 1), (LALT, 1), (D, 1), (D, 0), (D, 1)]);
        assert_eq!(
            out,
            vec![
                HotkeyEvent::Pressed,
                HotkeyEvent::Released,
                HotkeyEvent::Pressed
            ]
        );
    }
}
