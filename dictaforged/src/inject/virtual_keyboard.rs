//! wlroots virtual-keyboard injector (zwp_virtual_keyboard_v1), the wtype
//! technique: we upload our own keymap where each needed char gets a fresh
//! keycode from 8 up, so every Unicode char is reachable regardless of the
//! user's layout. Unavailable on compositors without the protocol (KDE,
//! GNOME).

use std::fmt::Write as _;
use std::io::Write as _;
use std::os::fd::AsFd;
use std::time::Instant;

use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, delegate_noop};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1;
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1;
use xkbcommon::xkb;

use super::{InjectError, Injector};

/// wl_keyboard keymap_format XKB_V1.
const KEYMAP_FORMAT_XKB_V1: u32 = 1;

pub struct VirtualKeyboardInjector {
    /// Chars seen so far; index = evdev keycode, xkb keycode = index + 8.
    chars: Vec<char>,
    conn: Option<Conn>,
    started: Instant,
}

struct Conn {
    queue: EventQueue<Globals>,
    globals: Globals,
    keyboard: ZwpVirtualKeyboardV1,
}

#[derive(Default)]
struct Globals {
    seat: Option<wl_seat::WlSeat>,
    manager: Option<ZwpVirtualKeyboardManagerV1>,
}

impl VirtualKeyboardInjector {
    pub fn new() -> Self {
        VirtualKeyboardInjector {
            chars: Vec::new(),
            conn: None,
            started: Instant::now(),
        }
    }

    fn upload_keymap(&mut self) -> Result<(), InjectError> {
        let conn = self.conn.as_mut().expect("probed");
        // keymap wants an fd; a deleted temp file keeps the fd alive
        let src = keymap_source(&self.chars);
        let path = std::env::temp_dir().join(format!("dictaforged-keymap-{}", std::process::id()));
        let mut file = std::fs::File::create(&path).map_err(InjectError::Io)?;
        let _ = std::fs::remove_file(&path);
        file.write_all(src.as_bytes()).map_err(InjectError::Io)?;
        file.write_all(b"\0").map_err(InjectError::Io)?;
        conn.keyboard
            .keymap(KEYMAP_FORMAT_XKB_V1, file.as_fd(), (src.len() + 1) as u32);
        conn.queue
            .roundtrip(&mut conn.globals)
            .map_err(|e| InjectError::Io(std::io::Error::other(e)))?;
        Ok(())
    }
}

impl Injector for VirtualKeyboardInjector {
    fn name(&self) -> &'static str {
        "virtual-keyboard"
    }

    fn probe(&mut self) -> Result<(), InjectError> {
        if self.conn.is_some() {
            return Ok(());
        }
        let connection = Connection::connect_to_env().map_err(|_| {
            InjectError::Unavailable("no wayland display (WAYLAND_DISPLAY unset or unreachable)")
        })?;
        let mut queue = connection.new_event_queue();
        let qh = queue.handle();
        connection.display().get_registry(&qh, ());
        let mut globals = Globals::default();
        queue
            .roundtrip(&mut globals)
            .map_err(|e| InjectError::Io(std::io::Error::other(e)))?;
        let (Some(seat), Some(manager)) = (&globals.seat, &globals.manager) else {
            return Err(InjectError::Unavailable(
                "compositor does not offer zwp_virtual_keyboard_v1 (KDE and GNOME do not)",
            ));
        };
        let keyboard = manager.create_virtual_keyboard(seat, &qh, ());
        self.conn = Some(Conn {
            queue,
            globals,
            keyboard,
        });
        Ok(())
    }

    fn inject(&mut self, text: &str) -> Result<(), InjectError> {
        self.probe()?;
        let mut grew = false;
        for c in text.chars() {
            if !self.chars.contains(&c) {
                self.chars.push(c);
                grew = true;
            }
        }
        if grew {
            self.upload_keymap()?;
        }
        let conn = self.conn.as_mut().expect("probed");
        for c in text.chars() {
            let code = self.chars.iter().position(|&k| k == c).unwrap() as u32;
            let time = self.started.elapsed().as_millis() as u32;
            conn.keyboard.key(time, code, 1);
            conn.keyboard.key(time, code, 0);
        }
        conn.queue
            .roundtrip(&mut conn.globals)
            .map_err(|e| InjectError::Io(std::io::Error::other(e)))?;
        Ok(())
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for Globals {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name, interface, ..
        } = event
        {
            match interface.as_str() {
                "wl_seat" => state.seat = Some(registry.bind(name, 1, qh, ())),
                "zwp_virtual_keyboard_manager_v1" => {
                    state.manager = Some(registry.bind(name, 1, qh, ()));
                }
                _ => {}
            }
        }
    }
}

delegate_noop!(Globals: ignore wl_seat::WlSeat);
delegate_noop!(Globals: ZwpVirtualKeyboardManagerV1);
delegate_noop!(Globals: ZwpVirtualKeyboardV1);

/// xkb keymap source with one keycode per char, starting at 8.
pub fn keymap_source(chars: &[char]) -> String {
    let mut keycodes = String::new();
    let mut symbols = String::new();
    for (i, c) in chars.iter().enumerate() {
        let name = xkb::keysym_get_name(xkb::utf32_to_keysym(*c as u32));
        writeln!(keycodes, "        <K{i}> = {};", i + 8).unwrap();
        writeln!(symbols, "        key <K{i}> {{ [ {name} ] }};").unwrap();
    }
    format!(
        "xkb_keymap {{\n\
         \x20   xkb_keycodes \"dictaforge\" {{\n\
         \x20       minimum = 8;\n\
         \x20       maximum = {max};\n\
         {keycodes}\
         \x20   }};\n\
         \x20   xkb_types \"dictaforge\" {{ include \"complete\" }};\n\
         \x20   xkb_compatibility \"dictaforge\" {{ include \"complete\" }};\n\
         \x20   xkb_symbols \"dictaforge\" {{\n\
         {symbols}\
         \x20   }};\n\
         }};\n",
        max = chars.len() + 8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keymap_source_names_keysyms_per_char() {
        let src = keymap_source(&['a', 'ä', '€', '日']);
        assert!(src.contains("minimum = 8"), "keycodes start at 8:\n{src}");
        for name in ["adiaeresis", "EuroSign", "U65E5"] {
            assert!(src.contains(name), "missing keysym {name}:\n{src}");
        }
        assert_eq!(
            src.matches("key <").count(),
            4,
            "one symbols entry per char"
        );
    }

    #[test]
    fn keymap_source_compiles_with_xkbcommon() {
        let src = keymap_source(&['a', 'ä', '€', '日']);
        let ctx = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let keymap = xkb::Keymap::new_from_string(
            &ctx,
            src.clone(),
            xkb::KEYMAP_FORMAT_TEXT_V1,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )
        .expect("generated keymap compiles");
        // first char sits at xkb keycode 8 and yields its keysym
        let state = xkb::State::new(&keymap);
        assert_eq!(state.key_get_utf8(xkb::Keycode::new(8)), "a");
        assert_eq!(state.key_get_utf8(xkb::Keycode::new(11)), "日");
    }
}
