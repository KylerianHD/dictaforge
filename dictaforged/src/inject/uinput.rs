//! uinput injector: replays key plans as kernel input events through a
//! virtual keyboard device. Works on every compositor but needs write access
//! to /dev/uinput (input group or udev rule).

use std::fs::{File, OpenOptions};
use std::io;

use input_linux::sys::input_event;
use input_linux::{EventKind, InputId, Key, UInputHandle};

use super::{InjectError, Injector};
use crate::keymap::{CharPlan, KeyPlan, KeymapIndex};

const EV_SYN: u16 = 0;
const EV_KEY: u16 = 1;
const SYN_REPORT: u16 = 0;

/// Where events land: the real uinput device in production, a Vec in tests.
pub trait EventSink {
    fn ready(&mut self) -> Result<(), InjectError> {
        Ok(())
    }
    fn emit(&mut self, type_: u16, code: u16, value: i32) -> io::Result<()>;
}

pub struct UinputInjector<S = UinputSink> {
    index: KeymapIndex,
    sink: S,
}

impl UinputInjector {
    pub fn new(index: KeymapIndex) -> Self {
        UinputInjector {
            index,
            sink: UinputSink::default(),
        }
    }
}

impl<S: EventSink> UinputInjector<S> {
    #[cfg(test)]
    fn with_sink(index: KeymapIndex, sink: S) -> Self {
        UinputInjector { index, sink }
    }
}

impl<S: EventSink> Injector for UinputInjector<S> {
    fn name(&self) -> &'static str {
        "uinput"
    }

    fn probe(&mut self) -> Result<(), InjectError> {
        self.sink.ready()
    }

    fn inject(&mut self, text: &str) -> Result<(), InjectError> {
        // Resolve everything first so nothing is emitted on failure.
        let mut chords: Vec<(Vec<u32>, u32)> = Vec::new();
        let mut unreachable = Vec::new();
        for (plan, c) in self.index.plan(text).iter().zip(text.chars()) {
            let key_plans: &[KeyPlan] = match plan {
                CharPlan::Direct(kp) => std::slice::from_ref(kp),
                CharPlan::Sequence(kps) => kps,
                CharPlan::Unreachable(c) => {
                    unreachable.push(*c);
                    continue;
                }
            };
            for kp in key_plans {
                // uinput cannot switch layout groups, so a plan on another
                // group would type the wrong char
                match (kp.group == self.index.group())
                    .then(|| self.index.mod_keys(kp.mods))
                    .flatten()
                {
                    Some(mods) => chords.push((mods, kp.keycode)),
                    None => {
                        unreachable.push(c);
                        break;
                    }
                }
            }
        }
        if !unreachable.is_empty() {
            return Err(InjectError::Unreachable(unreachable));
        }
        self.sink.ready()?;
        for (mods, key) in &chords {
            emit_chord(&mut self.sink, mods, *key).map_err(InjectError::Io)?;
        }
        Ok(())
    }
}

/// Lazily created virtual keyboard device.
#[derive(Default)]
pub struct UinputSink {
    handle: Option<UInputHandle<File>>,
}

/// Modifier downs, key tap, modifier ups, one EV_SYN after every event.
fn emit_chord<S: EventSink>(sink: &mut S, mods: &[u32], key: u32) -> io::Result<()> {
    let mut tap = |code: u32, value: i32| -> io::Result<()> {
        sink.emit(EV_KEY, code as u16, value)?;
        sink.emit(EV_SYN, SYN_REPORT, 0)
    };
    for m in mods {
        tap(*m, 1)?;
    }
    tap(key, 1)?;
    tap(key, 0)?;
    for m in mods.iter().rev() {
        tap(*m, 0)?;
    }
    Ok(())
}

impl EventSink for UinputSink {
    fn ready(&mut self) -> Result<(), InjectError> {
        if self.handle.is_some() {
            return Ok(());
        }
        let file = OpenOptions::new()
            .write(true)
            .open("/dev/uinput")
            .map_err(|_| {
                InjectError::Unavailable(
                    "cannot open /dev/uinput; add your user to the input group or add a udev rule",
                )
            })?;
        let handle = UInputHandle::new(file);
        (|| -> io::Result<()> {
            handle.set_evbit(EventKind::Key)?;
            for code in 0..=input_linux::sys::KEY_MAX as u16 {
                if let Ok(key) = Key::from_code(code) {
                    handle.set_keybit(key)?;
                }
            }
            handle.create(&InputId::default(), b"dictaforged virtual keyboard", 0, &[])
        })()
        .map_err(InjectError::Io)?;
        // ponytail: fixed settle delay so the compositor binds the new
        // device before the first event; make it configurable if it bites
        std::thread::sleep(std::time::Duration::from_millis(500));
        self.handle = Some(handle);
        Ok(())
    }

    fn emit(&mut self, type_: u16, code: u16, value: i32) -> io::Result<()> {
        let Some(handle) = &self.handle else {
            return Err(io::Error::other("uinput device not created; probe first"));
        };
        let event = input_event {
            time: input_linux::sys::timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
            type_,
            code,
            value,
        };
        handle.write(&[event])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::LayoutSpec;

    #[derive(Default)]
    struct TestSink(Vec<(u16, u16, i32)>);

    impl EventSink for TestSink {
        fn emit(&mut self, type_: u16, code: u16, value: i32) -> io::Result<()> {
            self.0.push((type_, code, value));
            Ok(())
        }
    }

    fn injector(layout: &str) -> UinputInjector<TestSink> {
        let index = KeymapIndex::from_spec(&LayoutSpec {
            layout: layout.into(),
            variant: String::new(),
            options: None,
            group: 0,
        })
        .unwrap();
        UinputInjector::with_sink(index, TestSink::default())
    }

    /// The Direct KeyPlan for one char, from the injector's own index so
    /// keycodes are never hardcoded.
    fn direct(inj: &UinputInjector<TestSink>, c: char) -> KeyPlan {
        match &inj.index.plan(&c.to_string())[0] {
            CharPlan::Direct(kp) => *kp,
            other => panic!("expected Direct plan for {c:?}, got {other:?}"),
        }
    }

    const SYN: (u16, u16, i32) = (EV_SYN, SYN_REPORT, 0);

    #[test]
    fn plain_key_taps_without_mods() {
        let mut inj = injector("de");
        let kp = direct(&inj, "z".chars().next().unwrap());
        assert_eq!(kp.mods, 0);
        inj.inject("z").unwrap();
        let key = kp.keycode as u16;
        assert_eq!(
            inj.sink.0,
            vec![(EV_KEY, key, 1), SYN, (EV_KEY, key, 0), SYN]
        );
    }

    #[test]
    fn shift_chord_on_de() {
        let mut inj = injector("de");
        let kp = direct(&inj, 'Z');
        let mods = inj.index.mod_keys(kp.mods).unwrap();
        assert_eq!(mods.len(), 1, "one shift key");
        inj.inject("Z").unwrap();
        let (shift, key) = (mods[0] as u16, kp.keycode as u16);
        assert_eq!(
            inj.sink.0,
            vec![
                (EV_KEY, shift, 1),
                SYN,
                (EV_KEY, key, 1),
                SYN,
                (EV_KEY, key, 0),
                SYN,
                (EV_KEY, shift, 0),
                SYN,
            ]
        );
    }

    #[test]
    fn altgr_chord_on_de() {
        let mut inj = injector("de");
        let kp = direct(&inj, '@');
        let mods = inj.index.mod_keys(kp.mods).unwrap();
        // AltGr on de is ISO_Level3_Shift on the right alt key, evdev 100
        assert_eq!(mods, vec![100]);
        inj.inject("@").unwrap();
        let (altgr, key) = (mods[0] as u16, kp.keycode as u16);
        assert_eq!(
            inj.sink.0,
            vec![
                (EV_KEY, altgr, 1),
                SYN,
                (EV_KEY, key, 1),
                SYN,
                (EV_KEY, key, 0),
                SYN,
                (EV_KEY, altgr, 0),
                SYN,
            ]
        );
    }

    #[test]
    fn dead_key_sequence_replays_both_chords() {
        let mut inj = injector("de");
        let plans = inj.index.plan("â");
        let CharPlan::Sequence(kps) = &plans[0] else {
            panic!("expected Sequence for 'â' on de");
        };
        let taps: usize = kps
            .iter()
            .map(|kp| 2 + 2 * inj.index.mod_keys(kp.mods).unwrap().len())
            .sum();
        inj.inject("â").unwrap();
        // every press/release is followed by one EV_SYN
        assert_eq!(inj.sink.0.len(), 2 * taps);
    }

    #[test]
    fn unreachable_char_emits_nothing() {
        let mut inj = injector("us");
        match inj.inject("a日b") {
            Err(InjectError::Unreachable(chars)) => assert_eq!(chars, vec!['日']),
            other => panic!("expected Unreachable, got {other:?}"),
        }
        assert_eq!(inj.sink.0, vec![], "no events before the error");
    }
}
