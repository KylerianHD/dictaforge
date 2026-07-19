//! Active layout detection: probe the session for its XKB layout config.
//! Pure parsers per source (unit-tested against fixtures) plus thin runners
//! that shell out; detect() chains them by desktop priority.

use std::process::Command;

use super::LayoutSpec;

const EVDEV_XML: &str = "/usr/share/X11/xkb/rules/evdev.xml";

/// Detect the active layout: override, KDE, GNOME, Hyprland, sway,
/// systemd-locale1, then a `us` fallback.
pub fn detect(override_: Option<&LayoutSpec>) -> LayoutSpec {
    override_
        .cloned()
        .or_else(kde)
        .or_else(gnome)
        .or_else(hyprland)
        .or_else(sway)
        .or_else(locale1)
        .unwrap_or_else(|| LayoutSpec {
            layout: "us".into(),
            variant: String::new(),
            options: None,
            group: 0,
        })
}

/// `~/.config/kxkbrc` [Layout] section: full layout/variant lists, the active
/// group comes separately from D-Bus. None when KDE does not manage layouts
/// (file or LayoutList missing, or Use=false).
fn parse_kxkbrc(text: &str) -> Option<LayoutSpec> {
    let mut in_layout = false;
    let mut layout = None;
    let mut variant = String::new();
    let mut options = None;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_layout = line == "[Layout]";
            continue;
        }
        let Some((key, value)) = in_layout.then(|| line.split_once('=')).flatten() else {
            continue;
        };
        match key {
            "LayoutList" if !value.is_empty() => layout = Some(value.to_string()),
            "VariantList" => variant = value.to_string(),
            "Options" if !value.is_empty() => options = Some(value.to_string()),
            "Use" if value == "false" => return None,
            _ => {}
        }
    }
    Some(LayoutSpec {
        layout: layout?,
        variant,
        options,
        group: 0,
    })
}

/// Reply of `busctl --user call org.kde.keyboard /Layouts
/// org.kde.KeyboardLayouts getLayout`, e.g. "u 1".
fn parse_getlayout(reply: &str) -> Option<u32> {
    reply.trim().strip_prefix("u ")?.parse().ok()
}

/// GNOME `gsettings get org.gnome.desktop.input-sources sources` plus
/// `mru-sources`: all xkb sources become the group list, the mru head picks
/// the active group.
fn parse_gsettings(sources: &str, mru: &str) -> Option<LayoutSpec> {
    // entries look like ('xkb', 'de+nodeadkeys'); ibus entries are skipped
    fn xkb_entries(list: &str) -> Vec<&str> {
        list.split("('xkb', '")
            .skip(1)
            .filter_map(|rest| rest.split('\'').next())
            .collect()
    }
    let entries = xkb_entries(sources);
    if entries.is_empty() {
        return None;
    }
    let group = xkb_entries(mru)
        .first()
        .and_then(|active| entries.iter().position(|e| e == active))
        .unwrap_or(0) as u32;
    let mut layouts = Vec::new();
    let mut variants = Vec::new();
    for entry in &entries {
        let (layout, variant) = entry.split_once('+').unwrap_or((entry, ""));
        layouts.push(layout);
        variants.push(variant);
    }
    Some(LayoutSpec {
        layout: layouts.join(","),
        variant: variants.join(","),
        options: None,
        group,
    })
}

/// `hyprctl -j devices`: layout/variant/options of the main keyboard plus its
/// active_keymap name (resolved to a group index via resolve_group).
fn parse_hyprctl(json: &str) -> Option<(LayoutSpec, String)> {
    let devices: serde_json::Value = serde_json::from_str(json).ok()?;
    let keyboards = devices.get("keyboards")?.as_array()?;
    let kb = keyboards
        .iter()
        .find(|kb| kb.get("main").and_then(|m| m.as_bool()) == Some(true))
        .or_else(|| keyboards.first())?;
    let field = |name: &str| Some(kb.get(name)?.as_str()?.to_string());
    let layout = field("layout").filter(|l| !l.is_empty())?;
    let options = field("options").filter(|o| !o.is_empty());
    Some((
        LayoutSpec {
            layout,
            variant: field("variant").unwrap_or_default(),
            options,
            group: 0,
        },
        field("active_keymap").unwrap_or_default(),
    ))
}

/// `swaymsg -t get_inputs`: first keyboard's layout description names and the
/// active index. Names still need resolve_description to become XKB codes.
fn parse_swaymsg(json: &str) -> Option<(Vec<String>, u32)> {
    let inputs: serde_json::Value = serde_json::from_str(json).ok()?;
    let kb = inputs
        .as_array()?
        .iter()
        .find(|input| input.get("type").and_then(|t| t.as_str()) == Some("keyboard"))?;
    let names: Vec<String> = kb
        .get("xkb_layout_names")?
        .as_array()?
        .iter()
        .filter_map(|n| Some(n.as_str()?.to_string()))
        .collect();
    if names.is_empty() {
        return None;
    }
    let group = kb
        .get("xkb_active_layout_index")
        .and_then(|i| i.as_u64())
        .unwrap_or(0) as u32;
    Some((names, group))
}

/// Map a layout description from evdev.xml ("German (no dead keys)") back to
/// XKB codes ("de", "nodeadkeys").
fn resolve_description(xml: &str, desc: &str) -> Option<(String, String)> {
    // ponytail: line scan instead of an XML parser; evdev.xml is stable,
    // machine-generated, one element per line where it matters
    fn tag_text<'a>(line: &'a str, tag: &str) -> Option<&'a str> {
        line.strip_prefix(&format!("<{tag}>"))?
            .strip_suffix(&format!("</{tag}>"))
    }
    let mut in_layouts = false;
    let mut in_variants = false;
    let mut layout = String::new();
    let mut variant = String::new();
    for line in xml.lines() {
        let line = line.trim();
        match line {
            "<layoutList>" => in_layouts = true,
            "</layoutList>" => return None,
            "<variantList>" => in_variants = true,
            "</variantList>" => in_variants = false,
            _ if !in_layouts => {}
            _ => {
                if let Some(name) = tag_text(line, "name") {
                    if in_variants {
                        variant = name.to_string();
                    } else {
                        layout = name.to_string();
                    }
                } else if tag_text(line, "description") == Some(desc) && !layout.is_empty() {
                    let variant = if in_variants { variant } else { String::new() };
                    return Some((layout, variant));
                }
            }
        }
    }
    None
}

/// Find the group index whose compiled layout name matches, 0 when unknown.
fn resolve_group(spec: &LayoutSpec, active_name: &str) -> u32 {
    use xkbcommon::xkb;
    let ctx = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
    let Some(keymap) = xkb::Keymap::new_from_names(
        &ctx,
        "",
        "",
        &spec.layout,
        &spec.variant,
        spec.options.clone(),
        xkb::KEYMAP_COMPILE_NO_FLAGS,
    ) else {
        return 0;
    };
    (0..keymap.num_layouts())
        .find(|&group| keymap.layout_get_name(group) == active_name)
        .unwrap_or(0)
}

/// `localectl status` text: X11 Layout/Variant/Options lines.
fn parse_localectl(text: &str) -> Option<LayoutSpec> {
    let value = |name: &str| {
        text.lines()
            .find_map(|line| {
                line.trim()
                    .strip_prefix(name)?
                    .strip_prefix(": ")
                    .map(str::trim)
            })
            .filter(|v| !v.is_empty() && *v != "(unset)")
            .map(String::from)
    };
    Some(LayoutSpec {
        layout: value("X11 Layout")?,
        variant: value("X11 Variant").unwrap_or_default(),
        options: value("X11 Options"),
        group: 0,
    })
}

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn kde() -> Option<LayoutSpec> {
    let home = std::env::var("HOME").ok()?;
    let text = std::fs::read_to_string(format!("{home}/.config/kxkbrc")).ok()?;
    let mut spec = parse_kxkbrc(&text)?;
    if let Some(group) = run(
        "busctl",
        &[
            "--user",
            "call",
            "org.kde.keyboard",
            "/Layouts",
            "org.kde.KeyboardLayouts",
            "getLayout",
        ],
    )
    .as_deref()
    .and_then(parse_getlayout)
    {
        spec.group = group;
    }
    Some(spec)
}

fn gnome() -> Option<LayoutSpec> {
    // ponytail: env guard, gsettings schemas exist on non-GNOME desktops too
    if !std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .contains("GNOME")
    {
        return None;
    }
    let sources = run(
        "gsettings",
        &["get", "org.gnome.desktop.input-sources", "sources"],
    )?;
    let mru = run(
        "gsettings",
        &["get", "org.gnome.desktop.input-sources", "mru-sources"],
    )
    .unwrap_or_default();
    parse_gsettings(&sources, &mru)
}

fn hyprland() -> Option<LayoutSpec> {
    std::env::var("HYPRLAND_INSTANCE_SIGNATURE").ok()?;
    let (mut spec, active) = parse_hyprctl(&run("hyprctl", &["-j", "devices"])?)?;
    spec.group = resolve_group(&spec, &active);
    Some(spec)
}

fn sway() -> Option<LayoutSpec> {
    std::env::var("SWAYSOCK").ok()?;
    let (names, group) = parse_swaymsg(&run("swaymsg", &["-t", "get_inputs"])?)?;
    let xml = std::fs::read_to_string(EVDEV_XML).ok()?;
    let mut layouts = Vec::new();
    let mut variants = Vec::new();
    for name in &names {
        let (layout, variant) = resolve_description(&xml, name)?;
        layouts.push(layout);
        variants.push(variant);
    }
    Some(LayoutSpec {
        layout: layouts.join(","),
        variant: variants.join(","),
        options: None,
        group,
    })
}

fn locale1() -> Option<LayoutSpec> {
    parse_localectl(&run("localectl", &["status"])?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(layout: &str, variant: &str, options: Option<&str>, group: u32) -> LayoutSpec {
        LayoutSpec {
            layout: layout.into(),
            variant: variant.into(),
            options: options.map(String::from),
            group,
        }
    }

    #[test]
    fn kxkbrc_layout_section() {
        let parsed = parse_kxkbrc(include_str!("../../tests/fixtures/kxkbrc"));
        assert_eq!(
            parsed,
            Some(spec(
                "us,de",
                ",nodeadkeys",
                Some("grp:alt_shift_toggle,terminate:ctrl_alt_bksp"),
                0
            ))
        );
    }

    #[test]
    fn kxkbrc_unused_or_empty() {
        assert_eq!(parse_kxkbrc("[Layout]\nLayoutList=de\nUse=false\n"), None);
        assert_eq!(parse_kxkbrc("[Layout]\nUse=true\n"), None);
        assert_eq!(parse_kxkbrc(""), None);
    }

    #[test]
    fn getlayout_reply() {
        assert_eq!(parse_getlayout("u 1"), Some(1));
        assert_eq!(parse_getlayout("u 0\n"), Some(0));
        assert_eq!(parse_getlayout("Call failed"), None);
    }

    #[test]
    fn gsettings_sources() {
        let fixture = include_str!("../../tests/fixtures/gsettings-sources.txt");
        let (sources, mru) = fixture.split_once('\n').unwrap();
        assert_eq!(
            parse_gsettings(sources, mru),
            Some(spec("us,de", ",nodeadkeys", None, 1))
        );
        // no mru recorded yet: first source is active
        assert_eq!(
            parse_gsettings(sources, "@a(ss) []"),
            Some(spec("us,de", ",nodeadkeys", None, 0))
        );
        assert_eq!(parse_gsettings("[]", ""), None);
    }

    #[test]
    fn hyprctl_main_keyboard() {
        let fixture = include_str!("../../tests/fixtures/hyprctl-devices.json");
        let (parsed, active) = parse_hyprctl(fixture).unwrap();
        assert_eq!(
            parsed,
            spec("us,de", ",nodeadkeys", Some("grp:alt_shift_toggle"), 0)
        );
        assert_eq!(active, "German (no dead keys)");
        assert_eq!(resolve_group(&parsed, &active), 1);
        assert_eq!(resolve_group(&parsed, "Klingon"), 0);
    }

    #[test]
    fn swaymsg_keyboard() {
        let fixture = include_str!("../../tests/fixtures/swaymsg-inputs.json");
        assert_eq!(
            parse_swaymsg(fixture),
            Some((
                vec!["English (US)".into(), "German (no dead keys)".into()],
                1
            ))
        );
        assert_eq!(parse_swaymsg("[]"), None);
    }

    #[test]
    fn description_resolves_via_evdev_xml() {
        // real registry file; CI installs xkeyboard-config/xkb-data
        let xml = std::fs::read_to_string(EVDEV_XML).unwrap();
        assert_eq!(
            resolve_description(&xml, "German (no dead keys)"),
            Some(("de".into(), "nodeadkeys".into()))
        );
        assert_eq!(
            resolve_description(&xml, "English (US)"),
            Some(("us".into(), String::new()))
        );
        assert_eq!(resolve_description(&xml, "Klingon"), None);
    }

    #[test]
    fn localectl_status() {
        let text = "System Locale: LANG=en_US.UTF-8\n    VC Keymap: de\n   X11 Layout: de\n    X11 Model: pc105\n  X11 Options: terminate:ctrl_alt_bksp\n";
        assert_eq!(
            parse_localectl(text),
            Some(spec("de", "", Some("terminate:ctrl_alt_bksp"), 0))
        );
        assert_eq!(parse_localectl("X11 Layout: (unset)\n"), None);
        assert_eq!(parse_localectl(""), None);
    }

    #[test]
    fn override_wins() {
        let over = spec("de", "neo", None, 0);
        assert_eq!(detect(Some(&over)), over);
    }
}
