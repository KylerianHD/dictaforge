//! XTEST injector for X11 sessions. Chars reachable on the server's actual
//! keymap go through plain fake key events; anything else temporarily maps
//! a spare keycode to the needed keysym (the xdotool technique). Under
//! Wayland this only reaches XWayland clients, so the manager selects it
//! only when XDG_SESSION_TYPE=x11.

use std::io;

use x11rb::connection::Connection as _;
use x11rb::protocol::xproto::{ConnectionExt as _, KEY_PRESS_EVENT, KEY_RELEASE_EVENT};
use x11rb::protocol::xtest::ConnectionExt as _;
use x11rb::xcb_ffi::XCBConnection;
use xkbcommon::xkb;

use super::{InjectError, Injector};
use crate::keymap::{CharPlan, KeyPlan, KeymapIndex};

/// Highest evdev keycode the core protocol can carry (255 - 8).
const MAX_CORE_EVDEV: u32 = 247;

/// One emission step for a single char.
#[derive(Debug, PartialEq)]
enum Action {
    /// Hold these evdev modifier keycodes, tap this evdev keycode.
    Chord { mods: Vec<u32>, key: u32 },
    /// Map a spare keycode to this keysym, tap it.
    Remap(xkb::Keysym),
}

/// Resolve text against the server keymap: chords where the plan fits the
/// core protocol (current group, producible mods, keycode + 8 within u8),
/// spare-keycode remaps for everything else.
fn actions(index: &KeymapIndex, text: &str) -> Result<Vec<Action>, InjectError> {
    let mut acts = Vec::new();
    let mut unreachable = Vec::new();
    for (plan, c) in index.plan(text).iter().zip(text.chars()) {
        let key_plans: Option<&[KeyPlan]> = match plan {
            CharPlan::Direct(kp) => Some(std::slice::from_ref(kp)),
            CharPlan::Sequence(kps) => Some(kps),
            CharPlan::Unreachable(_) => None,
        };
        // all key plans of the char must fit or the whole char is remapped
        let chords: Option<Vec<Action>> = key_plans.and_then(|kps| {
            kps.iter()
                .map(|kp| {
                    (kp.group == index.group() && kp.keycode <= MAX_CORE_EVDEV)
                        .then(|| index.mod_keys(kp.mods))
                        .flatten()
                        .map(|mods| Action::Chord {
                            mods,
                            key: kp.keycode,
                        })
                })
                .collect()
        });
        match chords {
            Some(cs) => acts.extend(cs),
            None => {
                let sym = xkb::utf32_to_keysym(c as u32);
                if sym.raw() == 0 {
                    unreachable.push(c);
                } else {
                    acts.push(Action::Remap(sym));
                }
            }
        }
    }
    if !unreachable.is_empty() {
        return Err(InjectError::Unreachable(unreachable));
    }
    Ok(acts)
}

pub struct XtestInjector {
    conn: Option<Conn>,
}

struct Conn {
    c: XCBConnection,
    index: KeymapIndex,
    /// Unused X keycodes (no symbols on the server map), highest first.
    spares: Vec<u8>,
    /// Keysym -> X keycode currently mapped; restored on drop.
    remaps: Vec<(xkb::Keysym, u8)>,
}

impl XtestInjector {
    pub fn new() -> Self {
        XtestInjector { conn: None }
    }
}

fn io_err(e: impl std::error::Error + Send + Sync + 'static) -> InjectError {
    InjectError::Io(io::Error::other(e))
}

impl Injector for XtestInjector {
    fn name(&self) -> &'static str {
        "xtest"
    }

    fn probe(&mut self) -> Result<(), InjectError> {
        if self.conn.is_some() {
            return Ok(());
        }
        let (c, _screen) = XCBConnection::connect(None).map_err(|_| {
            InjectError::Unavailable("no X11 display ($DISPLAY unset or unreachable)")
        })?;
        c.xtest_get_version(2, 2)
            .map_err(io_err)?
            .reply()
            .map_err(|_| InjectError::Unavailable("X server lacks the XTEST extension"))?;
        let (mut maj, mut min, mut ev, mut err) = (0, 0, 0, 0);
        if !xkb::x11::setup_xkb_extension(
            &c,
            xkb::x11::MIN_MAJOR_XKB_VERSION,
            xkb::x11::MIN_MINOR_XKB_VERSION,
            xkb::x11::SetupXkbExtensionFlags::NoFlags,
            &mut maj,
            &mut min,
            &mut ev,
            &mut err,
        ) {
            return Err(InjectError::Unavailable("X server lacks the XKB extension"));
        }
        let device = xkb::x11::get_core_keyboard_device_id(&c);
        if device < 0 {
            return Err(InjectError::Unavailable("no core keyboard device"));
        }
        let ctx = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let keymap =
            xkb::x11::keymap_new_from_device(&ctx, &c, device, xkb::KEYMAP_COMPILE_NO_FLAGS);
        let state = xkb::x11::state_new_from_device(&keymap, &c, device);
        let group = state.serialize_layout(xkb::STATE_LAYOUT_EFFECTIVE);
        let index = KeymapIndex::from_keymap(keymap, group);

        // spare keycodes: no keysyms bound anywhere on the server map
        let setup = c.setup();
        let (min_kc, max_kc) = (setup.min_keycode, setup.max_keycode);
        let mapping = c
            .get_keyboard_mapping(min_kc, max_kc - min_kc + 1)
            .map_err(io_err)?
            .reply()
            .map_err(io_err)?;
        let per = mapping.keysyms_per_keycode as usize;
        let spares: Vec<u8> = (min_kc..=max_kc)
            .filter(|kc| {
                let start = (kc - min_kc) as usize * per;
                mapping.keysyms[start..start + per].iter().all(|&s| s == 0)
            })
            .rev()
            .collect();

        self.conn = Some(Conn {
            c,
            index,
            spares,
            remaps: Vec::new(),
        });
        Ok(())
    }

    fn inject(&mut self, text: &str) -> Result<(), InjectError> {
        self.probe()?;
        let conn = self.conn.as_mut().expect("probed");
        let acts = actions(&conn.index, text)?;

        // Upload any new remaps first, then give focused clients a moment
        // to process the MappingNotify before the fake events arrive.
        let mut uploaded = false;
        for act in &acts {
            let Action::Remap(sym) = act else { continue };
            if conn.remaps.iter().any(|(s, _)| s == sym) {
                continue;
            }
            let Some(kc) = conn.spares.pop() else {
                return Err(InjectError::Io(io::Error::other(
                    "out of spare X keycodes for remapping",
                )));
            };
            conn.c
                .change_keyboard_mapping(1, kc, 1, &[sym.raw()])
                .map_err(io_err)?;
            conn.remaps.push((*sym, kc));
            uploaded = true;
        }
        if uploaded {
            conn.sync()?;
            // ponytail: fixed delay for clients to reload the mapping;
            // make it configurable if it bites
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        for act in &acts {
            match act {
                Action::Chord { mods, key } => {
                    for m in mods {
                        conn.fake_key((m + 8) as u8, KEY_PRESS_EVENT)?;
                    }
                    conn.fake_key((key + 8) as u8, KEY_PRESS_EVENT)?;
                    conn.fake_key((key + 8) as u8, KEY_RELEASE_EVENT)?;
                    for m in mods.iter().rev() {
                        conn.fake_key((m + 8) as u8, KEY_RELEASE_EVENT)?;
                    }
                }
                Action::Remap(sym) => {
                    let kc = conn
                        .remaps
                        .iter()
                        .find(|(s, _)| s == sym)
                        .map(|(_, kc)| *kc)
                        .expect("uploaded above");
                    conn.fake_key(kc, KEY_PRESS_EVENT)?;
                    conn.fake_key(kc, KEY_RELEASE_EVENT)?;
                }
            }
        }
        conn.sync()
    }
}

impl Conn {
    fn fake_key(&self, keycode: u8, type_: u8) -> Result<(), InjectError> {
        self.c
            .xtest_fake_input(type_, keycode, x11rb::CURRENT_TIME, x11rb::NONE, 0, 0, 0)
            .map_err(io_err)?;
        Ok(())
    }

    /// Flush and wait until the server has processed everything.
    fn sync(&self) -> Result<(), InjectError> {
        self.c
            .get_input_focus()
            .map_err(io_err)?
            .reply()
            .map_err(io_err)?;
        Ok(())
    }
}

impl Drop for Conn {
    fn drop(&mut self) {
        // hand the borrowed keycodes back as unbound
        for (_, kc) in &self.remaps {
            let _ = self.c.change_keyboard_mapping(1, *kc, 1, &[0]);
        }
        let _ = self.sync();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::LayoutSpec;

    fn index(layout: &str) -> KeymapIndex {
        KeymapIndex::from_spec(&LayoutSpec {
            layout: layout.into(),
            variant: String::new(),
            options: None,
            group: 0,
        })
        .unwrap()
    }

    #[test]
    fn reachable_text_is_all_chords() {
        // '#' is deliberately absent: xkeyboard-config also binds numbersign
        // to a mods-free key above keycode 247, the index prefers it, and the
        // char correctly rides the remap path instead
        let acts = actions(&index("us"), "Test123 !@").unwrap();
        assert_eq!(acts.len(), 10);
        assert!(
            acts.iter()
                .all(|a| matches!(a, Action::Chord { key, .. } if *key <= 247)),
            "everything on us fits the core protocol: {acts:?}"
        );
    }

    #[test]
    fn unreachable_chars_become_remaps() {
        let acts = actions(&index("us"), "ä日").unwrap();
        assert_eq!(
            acts,
            vec![
                Action::Remap(xkb::utf32_to_keysym('ä' as u32)),
                Action::Remap(xkb::utf32_to_keysym('日' as u32)),
            ]
        );
    }

    #[test]
    fn euro_remaps_because_key_euro_exceeds_core_keycodes() {
        // '€' is Direct on every layout via KEY_EURO (evdev 435), but
        // 435 + 8 does not fit the core protocol's u8 keycode
        let acts = actions(&index("us"), "€").unwrap();
        assert_eq!(acts, vec![Action::Remap(xkb::utf32_to_keysym('€' as u32))]);
    }

    #[test]
    fn dead_key_sequence_stays_chords_on_de() {
        // 'â' on de is dead_circumflex + a, both plain keys
        let acts = actions(&index("de"), "â").unwrap();
        assert_eq!(acts.len(), 2, "two chords for dead key + base");
        assert!(acts.iter().all(|a| matches!(a, Action::Chord { .. })));
    }

    /// Full server round-trip: a window collects the fake key events and
    /// translates them through the server keymap. Run with:
    /// xvfb-run -a cargo test -p dictaforged -- --ignored xtest
    #[test]
    #[ignore = "needs an X server; run under xvfb-run"]
    fn xvfb_roundtrip() {
        use x11rb::connection::Connection as _;
        use x11rb::protocol::Event;
        use x11rb::protocol::xproto::{
            ConnectionExt as _, CreateWindowAux, EventMask, InputFocus, WindowClass,
        };

        let (lc, screen_num) = XCBConnection::connect(None).unwrap();
        // xkb must be set up per connection before device/keymap queries
        let (mut maj, mut min, mut ev, mut err) = (0, 0, 0, 0);
        assert!(xkb::x11::setup_xkb_extension(
            &lc,
            xkb::x11::MIN_MAJOR_XKB_VERSION,
            xkb::x11::MIN_MINOR_XKB_VERSION,
            xkb::x11::SetupXkbExtensionFlags::NoFlags,
            &mut maj,
            &mut min,
            &mut ev,
            &mut err,
        ));
        let screen = &lc.setup().roots[screen_num];
        let win = lc.generate_id().unwrap();
        lc.create_window(
            0,
            win,
            screen.root,
            0,
            0,
            1,
            1,
            0,
            WindowClass::INPUT_OUTPUT,
            screen.root_visual,
            &CreateWindowAux::new().event_mask(EventMask::KEY_PRESS | EventMask::KEY_RELEASE),
        )
        .unwrap();
        lc.map_window(win).unwrap();
        lc.set_input_focus(InputFocus::PARENT, win, x11rb::CURRENT_TIME)
            .unwrap();
        lc.get_input_focus().unwrap().reply().unwrap();

        // Translate queued events through a freshly fetched server keymap
        // (the injector's remaps are still in place, so spare keycodes
        // resolve; the injector is dropped only after draining).
        let drain = |lc: &XCBConnection| -> String {
            lc.get_input_focus().unwrap().reply().unwrap();
            let ctx = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
            let device = xkb::x11::get_core_keyboard_device_id(lc);
            let keymap =
                xkb::x11::keymap_new_from_device(&ctx, lc, device, xkb::KEYMAP_COMPILE_NO_FLAGS);
            let mut st = xkb::State::new(&keymap);
            let mut out = String::new();
            while let Some(event) = lc.poll_for_event().unwrap() {
                match event {
                    Event::KeyPress(e) => {
                        let key = xkb::Keycode::new(e.detail as u32);
                        out.push_str(&st.key_get_utf8(key));
                        st.update_key(key, xkb::KeyDirection::Down);
                    }
                    Event::KeyRelease(e) => {
                        st.update_key(xkb::Keycode::new(e.detail as u32), xkb::KeyDirection::Up);
                    }
                    _ => {}
                }
            }
            out
        };

        // phase 1: default us map, chords plus remaps (ä and € need spares)
        let mut inj = XtestInjector::new();
        inj.probe().unwrap();
        inj.inject("Test123 !@#ä€").unwrap();
        assert_eq!(drain(&lc), "Test123 !@#ä€");
        drop(inj);

        // phase 2: layout switch is picked up by a fresh injector
        assert!(
            std::process::Command::new("setxkbmap")
                .arg("de")
                .status()
                .unwrap()
                .success()
        );
        let mut inj = XtestInjector::new();
        inj.probe().unwrap();
        inj.inject("zäöü").unwrap();
        assert_eq!(drain(&lc), "zäöü");
    }
}
