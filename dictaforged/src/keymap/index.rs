use std::collections::HashMap;

use anyhow::anyhow;
use xkbcommon::xkb;
use xkbcommon::xkb::compose;

use super::{CharPlan, KeyPlan, LayoutSpec};

/// Reverse keymap: char -> key plan, built once per layout.
pub struct KeymapIndex {
    keymap: xkb::Keymap,
    compose: Option<compose::Table>,
    plans: HashMap<char, CharPlan>,
    group: u32,
    /// Modifier mask -> evdev keycode of a key that depresses exactly it.
    mod_keys: Vec<(xkb::ModMask, u32)>,
}

/// True when plan `a` beats plan `b`: prefer the requested group, then no
/// CapsLock (injecting Lock would leave it latched), then fewer modifiers,
/// then the lower keycode for determinism.
fn better(a: &KeyPlan, b: &KeyPlan, target_group: u32, caps_mask: xkb::ModMask) -> bool {
    let rank = |p: &KeyPlan| {
        (
            p.group != target_group,
            p.mods & caps_mask != 0,
            p.mods.count_ones(),
            p.keycode,
        )
    };
    rank(a) < rank(b)
}

impl KeymapIndex {
    pub fn from_spec(spec: &LayoutSpec) -> anyhow::Result<Self> {
        let ctx = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let keymap = xkb::Keymap::new_from_names(
            &ctx,
            "",
            "",
            &spec.layout,
            &spec.variant,
            spec.options.clone(),
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )
        .ok_or_else(|| {
            anyhow!(
                "xkb keymap failed to compile for layout {:?} variant {:?}",
                spec.layout,
                spec.variant
            )
        })?;
        Ok(Self::from_keymap(keymap, spec.group))
    }

    /// Build the index for an already-compiled keymap (e.g. fetched from the
    /// X server) and a target group.
    pub fn from_keymap(keymap: xkb::Keymap, group: u32) -> Self {
        let ctx = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        // ponytail: en_US.UTF-8 is the canonical superset Compose table that
        // every UTF-8 session maps to; user Compose override when someone asks
        let compose_table = compose::Table::new_from_locale(
            &ctx,
            std::ffi::OsStr::new("en_US.UTF-8"),
            compose::COMPILE_NO_FLAGS,
        )
        .ok();

        let caps_mask = match keymap.mod_get_index(xkb::MOD_NAME_CAPS) {
            xkb::MOD_INVALID => 0,
            idx => 1 << idx,
        };

        // Reverse pass: cheapest KeyPlan per keysym across keys x groups x levels.
        let target_group = group;
        let mut sym_plans: HashMap<xkb::Keysym, KeyPlan> = HashMap::new();
        keymap.key_for_each(|km, key| {
            if key.raw() < 8 {
                return;
            }
            for group in 0..km.num_layouts_for_key(key) {
                for level in 0..km.num_levels_for_key(key, group) {
                    // ponytail: multi-keysym levels are exotic, skipped
                    let [sym] = km.key_get_syms_by_level(key, group, level) else {
                        continue;
                    };
                    let mut masks = [0 as xkb::ModMask; 8];
                    let n = km.key_get_mods_for_level(key, group, level, &mut masks);
                    if n == 0 {
                        continue;
                    }
                    let mods = masks[..n.min(masks.len())]
                        .iter()
                        .copied()
                        .min_by_key(|m| (m & caps_mask != 0, m.count_ones()))
                        .unwrap();
                    let cand = KeyPlan {
                        keycode: key.raw() - 8,
                        mods,
                        group,
                    };
                    match sym_plans.get(sym) {
                        Some(cur) if !better(&cand, cur, target_group, caps_mask) => {}
                        _ => {
                            sym_plans.insert(*sym, cand);
                        }
                    }
                }
            }
        });

        // Direct plans: keysyms that map to a unicode char.
        let mut plans: HashMap<char, CharPlan> = HashMap::new();
        for (sym, kp) in &sym_plans {
            let cp = xkb::keysym_to_utf32(*sym);
            let Some(c) = (cp != 0).then(|| char::from_u32(cp)).flatten() else {
                continue;
            };
            match plans.get(&c) {
                Some(CharPlan::Direct(cur)) if !better(kp, cur, group, caps_mask) => {}
                _ => {
                    plans.insert(c, CharPlan::Direct(*kp));
                }
            }
        }

        // Dead-key pass: compose every reachable dead key with every reachable
        // base char through the real compose table; whatever comes out and is
        // not already directly typeable becomes a Sequence.
        // ponytail: dead+base pairs only; chained dead keys and Multi_key
        // sequences wait for a real-world report
        if let Some(table) = &compose_table {
            let dead: Vec<(xkb::Keysym, KeyPlan)> = sym_plans
                .iter()
                .filter(|(s, _)| xkb::keysym_get_name(**s).starts_with("dead_"))
                .map(|(s, p)| (*s, *p))
                .collect();
            let mut st = compose::State::new(table, compose::STATE_NO_FLAGS);
            for (dead_sym, dead_plan) in &dead {
                for (base_sym, base_plan) in &sym_plans {
                    if xkb::keysym_to_utf32(*base_sym) == 0 {
                        continue;
                    }
                    st.reset();
                    st.feed(*dead_sym);
                    st.feed(*base_sym);
                    if st.status() != compose::Status::Composed {
                        continue;
                    }
                    let Some(s) = st.utf8() else { continue };
                    let mut it = s.chars();
                    let (Some(c), None) = (it.next(), it.next()) else {
                        continue;
                    };
                    plans
                        .entry(c)
                        .or_insert_with(|| CharPlan::Sequence(vec![*dead_plan, *base_plan]));
                }
            }
        }

        // Which physical keys depress which modifier masks (Shift, AltGr, ...):
        // press each key on a fresh state and read the depressed mods.
        let mut mod_keys: Vec<(xkb::ModMask, u32)> = Vec::new();
        keymap.key_for_each(|km, key| {
            if key.raw() < 8 {
                return;
            }
            // ctrl, shift, alt, capslock, altgr, meta: left and right.
            // xkeyboard-config also puts modifiers on phantom keys no real
            // keyboard sends (<LVL3> at evdev 84); prefer the physical ones
            const REAL_MOD_KEYS: &[u32] = &[29, 42, 54, 56, 58, 97, 100, 125, 126];
            let code = key.raw() - 8;
            let mut st = xkb::State::new(km);
            st.update_key(key, xkb::KeyDirection::Down);
            let mask = st.serialize_mods(xkb::STATE_MODS_DEPRESSED);
            if mask == 0 {
                return;
            }
            let rank = |c: u32| (!REAL_MOD_KEYS.contains(&c), c);
            match mod_keys.iter_mut().find(|(m, _)| *m == mask) {
                Some((_, cur)) if rank(code) < rank(*cur) => *cur = code,
                None => mod_keys.push((mask, code)),
                _ => {}
            }
        });

        Self {
            keymap,
            compose: compose_table,
            plans,
            group,
            mod_keys,
        }
    }

    /// The group plans were ranked for; injectors that cannot switch groups
    /// must refuse plans on any other group.
    pub fn group(&self) -> u32 {
        self.group
    }

    /// Evdev keycodes to hold for a modifier mask, None when no combination
    /// of modifier keys produces exactly it.
    pub fn mod_keys(&self, mods: xkb::ModMask) -> Option<Vec<u32>> {
        let mut remaining = mods;
        let mut keys = Vec::new();
        while remaining != 0 {
            let (mask, key) = self
                .mod_keys
                .iter()
                .find(|(m, _)| m & remaining != 0 && m & !mods == 0)?;
            remaining &= !mask;
            keys.push(*key);
        }
        Some(keys)
    }

    pub fn plan(&self, text: &str) -> Vec<CharPlan> {
        text.chars()
            .map(|c| {
                self.plans
                    .get(&c)
                    .cloned()
                    .unwrap_or(CharPlan::Unreachable(c))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index(layout: &str, variant: &str) -> KeymapIndex {
        KeymapIndex::from_spec(&LayoutSpec {
            layout: layout.into(),
            variant: variant.into(),
            options: None,
            group: 0,
        })
        .unwrap()
    }

    fn state_for(index: &KeymapIndex, kp: KeyPlan) -> xkb::State {
        let mut st = xkb::State::new(&index.keymap);
        st.update_mask(kp.mods, 0, 0, 0, 0, kp.group);
        st
    }

    fn key_utf8(index: &KeymapIndex, kp: KeyPlan) -> String {
        state_for(index, kp).key_get_utf8(xkb::Keycode::new(kp.keycode + 8))
    }

    fn key_sym(index: &KeymapIndex, kp: KeyPlan) -> xkb::Keysym {
        state_for(index, kp).key_get_one_sym(xkb::Keycode::new(kp.keycode + 8))
    }

    /// Round-trip: plan the text, replay every plan through a fresh
    /// xkb_state on the same keymap (set the planned mod mask and group,
    /// read the utf8 a client would receive), assert identity.
    fn replay(index: &KeymapIndex, text: &str) -> String {
        let mut out = String::new();
        for plan in index.plan(text) {
            match plan {
                CharPlan::Direct(kp) => out.push_str(&key_utf8(index, kp)),
                CharPlan::Sequence(kps) => {
                    let table = index.compose.as_ref().expect("compose table missing");
                    let mut st = compose::State::new(table, compose::STATE_NO_FLAGS);
                    for kp in &kps {
                        st.feed(key_sym(index, *kp));
                    }
                    assert_eq!(
                        st.status(),
                        compose::Status::Composed,
                        "sequence did not compose"
                    );
                    out.push_str(&st.utf8().expect("composed utf8"));
                }
                CharPlan::Unreachable(c) => panic!("char {c:?} planned as unreachable"),
            }
        }
        out
    }

    #[test]
    fn roundtrip_us() {
        let idx = index("us", "");
        assert_eq!(replay(&idx, "Hello, World! 123"), "Hello, World! 123");
    }

    #[test]
    fn roundtrip_de() {
        let idx = index("de", "");
        assert_eq!(replay(&idx, "zäöüß Zylinder @{[]}"), "zäöüß Zylinder @{[]}");
    }

    #[test]
    fn roundtrip_dvorak() {
        let idx = index("us", "dvorak");
        assert_eq!(replay(&idx, "hello dvorak"), "hello dvorak");
    }

    #[test]
    fn roundtrip_fr() {
        let idx = index("fr", "");
        assert_eq!(replay(&idx, "azerty éèç"), "azerty éèç");
    }

    #[test]
    fn dead_key_sequence_de() {
        let idx = index("de", "");
        let plans = idx.plan("â");
        match &plans[0] {
            CharPlan::Sequence(kps) => assert_eq!(kps.len(), 2, "dead key + base key"),
            other => panic!("expected Sequence for 'â' on de, got {other:?}"),
        }
        assert_eq!(replay(&idx, "â"), "â");
    }

    #[test]
    fn unreachable_chars() {
        // '€' is NOT a valid case here: xkeyboard-config maps KEY_EURO
        // (evdev 435) to EuroSign in every layout, so it is reachable even
        // on plain us. 'ä' on us has no key, no dead key, no compose path.
        let us = index("us", "");
        assert_eq!(us.plan("ä"), vec![CharPlan::Unreachable('ä')]);
        assert_eq!(us.plan("日"), vec![CharPlan::Unreachable('日')]);
        let de = index("de", "");
        assert_eq!(de.plan("日"), vec![CharPlan::Unreachable('日')]);
    }
}
