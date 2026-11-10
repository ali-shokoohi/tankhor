use std::collections::BTreeMap;
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

const COLOR_ROLES: [&str; 16] = [
    "background",
    "surface",
    "surfaceMuted",
    "textPrimary",
    "textSecondary",
    "brandVelvet",
    "curtainBase",
    "sky",
    "action",
    "onAction",
    "border",
    "borderControl",
    "onCurtain",
    "success",
    "warning",
    "danger",
];

const TYPE_ROLES: [&str; 7] = [
    "hero_desktop",
    "hero_mobile",
    "section_large",
    "section_small",
    "body",
    "control",
    "caption",
];

fn main() {
    println!("cargo:rerun-if-changed=design/tokens.toml");
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let tokens_path = manifest_dir.join("design/tokens.toml");
    let raw = fs::read_to_string(&tokens_path).expect("failed to read design/tokens.toml");

    let sections = parse_sections(&raw);
    let css = generate_css(&sections);

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    fs::write(out_dir.join("tokens.css"), css).expect("failed to write OUT_DIR/tokens.css");
}

fn parse_sections(raw: &str) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut sections: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut current_section = String::new();

    for raw_line in raw.lines() {
        let line = strip_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("[[") && line.ends_with("]]") {
            current_section = line.to_string();
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            current_section = line[1..line.len() - 1].trim().to_string();
            sections.entry(current_section.clone()).or_default();
            continue;
        }
        if current_section.starts_with("[[") {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            sections
                .entry(current_section.clone())
                .or_default()
                .insert(key.trim().to_string(), value.trim().to_string());
        }
    }

    sections
}

fn strip_comment(line: &str) -> &str {
    let mut in_quotes = false;
    for (idx, ch) in line.char_indices() {
        if ch == '"' {
            in_quotes = !in_quotes;
        } else if ch == '#' && !in_quotes {
            return &line[..idx];
        }
    }
    line
}

fn unquote(raw: &str) -> String {
    let trimmed = raw.trim();
    assert!(
        trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2,
        "expected quoted string value, got {trimmed}"
    );
    trimmed[1..trimmed.len() - 1].to_string()
}

fn parse_string_list(raw: &str) -> Vec<String> {
    let trimmed = raw.trim();
    assert!(
        trimmed.starts_with('[') && trimmed.ends_with(']'),
        "expected list value, got {trimmed}"
    );
    trimmed[1..trimmed.len() - 1]
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(unquote)
        .collect()
}

fn parse_u32_list(raw: &str) -> Vec<u32> {
    let trimmed = raw.trim();
    assert!(
        trimmed.starts_with('[') && trimmed.ends_with(']'),
        "expected integer list value, got {trimmed}"
    );
    trimmed[1..trimmed.len() - 1]
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(|item| item.parse::<u32>().expect("expected integer in list"))
        .collect()
}

fn camel_to_kebab(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 4);
    for ch in input.chars() {
        if ch.is_ascii_uppercase() {
            out.push('-');
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

fn validate_hex(hex: &str) {
    assert!(
        hex.len() == 7 && hex.starts_with('#') && hex[1..].chars().all(|c| c.is_ascii_hexdigit()),
        "invalid hex colour token: {hex}"
    );
}

fn emit_theme_colors(out: &mut String, map: &BTreeMap<String, String>) {
    assert_eq!(
        map.len(),
        COLOR_ROLES.len(),
        "theme colour map must have exact 16 roles"
    );
    for role in COLOR_ROLES {
        let hex = unquote(
            map.get(role)
                .unwrap_or_else(|| panic!("missing colour role {role}")),
        );
        validate_hex(&hex);
        let css_name = camel_to_kebab(role);
        let _ = writeln!(out, "  --tk-color-{css_name}: {hex};");
    }
}

fn emit_typography_and_layout_vars(
    css: &mut String,
    sections: &BTreeMap<String, BTreeMap<String, String>>,
) {
    let typography = sections.get("typography").expect("missing [typography]");
    let spacing = sections.get("spacing").expect("missing [spacing]");
    let radii = sections.get("radii").expect("missing [radii]");
    let targets = sections.get("targets").expect("missing [targets]");
    let icons = sections.get("icons").expect("missing [icons]");
    let motion_feedback = sections
        .get("motion.control_feedback")
        .expect("missing [motion.control_feedback]");
    let motion_curtain = sections
        .get("motion.curtain_continuity")
        .expect("missing [motion.curtain_continuity]");

    let font_stack_css = parse_string_list(typography.get("font_stack").expect("font_stack"))
        .into_iter()
        .map(|f| {
            if f.contains(' ') || f == "Vazirmatn" {
                format!("'{f}'")
            } else {
                f
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    let _ = writeln!(css, "  --tk-font-sans: {font_stack_css};");

    for role in TYPE_ROLES {
        let sec_name = format!("typography.scale.{role}");
        let sec = sections
            .get(&sec_name)
            .unwrap_or_else(|| panic!("missing [{sec_name}]"));
        let size_px: u32 = sec.get("size_px").expect("size_px").parse().expect("u32");
        let weight: u32 = sec.get("weight").expect("weight").parse().expect("u32");
        let line_height: f64 = sec
            .get("line_height")
            .expect("line_height")
            .parse()
            .expect("f64");
        let css_role = role.replace('_', "-");
        let _ = writeln!(css, "  --tk-type-{css_role}-size: {size_px}px;");
        let _ = writeln!(css, "  --tk-type-{css_role}-weight: {weight};");
        let _ = writeln!(css, "  --tk-type-{css_role}-line-height: {line_height};");
    }

    for px in parse_u32_list(spacing.get("scale_px").expect("scale_px")) {
        let _ = writeln!(css, "  --tk-space-{px}: {px}px;");
    }

    let control_radius: u32 = radii.get("control_px").expect("c").parse().expect("u32");
    let panel_radius: u32 = radii.get("panel_px").expect("p").parse().expect("u32");
    let _ = writeln!(css, "  --tk-radius-control: {control_radius}px;");
    let _ = writeln!(css, "  --tk-radius-panel: {panel_radius}px;");

    let primary_btn_h: u32 = targets
        .get("primary_button_height_px")
        .expect("primary_button_height_px")
        .parse()
        .expect("u32");
    let touch_min: u32 = targets
        .get("touch_target_min_px")
        .expect("touch_target_min_px")
        .parse()
        .expect("u32");
    let _ = writeln!(css, "  --tk-target-primary-button: {primary_btn_h}px;");
    let _ = writeln!(css, "  --tk-target-touch-min: {touch_min}px;");

    let icon_canvas: u32 = icons.get("canvas_px").expect("c").parse().expect("u32");
    let stroke_min: f64 = icons.get("stroke_min_px").expect("s").parse().expect("f64");
    let stroke_max: f64 = icons.get("stroke_max_px").expect("s").parse().expect("f64");
    let _ = writeln!(css, "  --tk-icon-canvas: {icon_canvas}px;");
    let _ = writeln!(css, "  --tk-icon-stroke-min: {stroke_min}px;");
    let _ = writeln!(css, "  --tk-icon-stroke-max: {stroke_max}px;");

    let feedback_ms: u32 = motion_feedback
        .get("duration_ms")
        .expect("duration_ms")
        .parse()
        .expect("u32");
    let curtain_ms: u32 = motion_curtain
        .get("duration_ms")
        .expect("duration_ms")
        .parse()
        .expect("u32");
    let _ = writeln!(css, "  --tk-motion-control-feedback: {feedback_ms}ms;");
    let _ = writeln!(css, "  --tk-motion-curtain-continuity: {curtain_ms}ms;");
}

fn generate_css(sections: &BTreeMap<String, BTreeMap<String, String>>) -> String {
    let light = sections
        .get("colors.light")
        .expect("missing [colors.light]");
    let dark = sections.get("colors.dark").expect("missing [colors.dark]");

    let mut css = String::new();
    css.push_str(
        "/* Generated by crates/web/build.rs from crates/web/design/tokens.toml. Do not edit. */\n",
    );
    css.push_str(":root,\n:root[data-theme=\"light\"] {\n");
    emit_theme_colors(&mut css, light);
    emit_typography_and_layout_vars(&mut css, sections);
    css.push_str("}\n\n");

    css.push_str(":root[data-theme=\"dark\"] {\n");
    emit_theme_colors(&mut css, dark);
    css.push_str("}\n\n");

    css.push_str("@media (prefers-color-scheme: dark) {\n  :root:not([data-theme=\"light\"]) {\n");
    let mut dark_buf = String::new();
    emit_theme_colors(&mut dark_buf, dark);
    for line in dark_buf.lines() {
        let _ = writeln!(css, "  {line}");
    }
    css.push_str("  }\n}\n");

    css
}
