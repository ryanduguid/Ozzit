//! Workbook palette pass: theme, styles, sheets and drawings to the Mermaid palette.
//!
//! `--template` applies only the colour roles, for the workbooks in `templates/`.
//! A fixed-point guard refuses to write if any colour in the result is still
//! remappable. The hue remap reproduces Python's `colorsys` arithmetic.
use super::*;
use std::collections::BTreeMap;
use std::sync::LazyLock;

const BLACK: &str = "000000";
const IVORY: &str = "FFFFF0";
const PLATINUM: &str = "E5E4E2";
const CRIMSON: &str = "990024";
const GREEN: &str = "0F4B32";
const CRIMSON_TINT: &str = "F0D9DE";
const GREEN_TINT: &str = "DBE4E0";
/// Element shapes with or without a namespace prefix.
const P: &str = r"(?:\w+:)?";

struct Tables {
    explicit: BTreeMap<String, String>,
    keep: Vec<String>,
    tabs: Vec<(String, String)>,
    theme: Vec<(String, String)>,
    font_remap: Vec<(String, String)>,
    fill_remap: Vec<(String, String)>,
    font_families: Vec<(String, String)>,
    rfont_families: Vec<(String, String)>,
    input_fill: String,
}

fn pairs(value: &Value) -> Vec<(String, String)> {
    value
        .as_object()
        .expect("object")
        .iter()
        .map(|(k, v)| (k.clone(), v.as_str().expect("string").to_string()))
        .collect()
}

static TABLES: LazyLock<Tables> = LazyLock::new(|| {
    let data = data(include_str!("../data/postbuild/workbook_palette.json"));
    Tables {
        explicit: pairs(&data["EXPLICIT"]).into_iter().collect(),
        keep: strings(&data["KEEP"]),
        tabs: pairs(&data["TAB_COLOURS"]),
        theme: pairs(&data["THEME"]),
        font_remap: pairs(&data["FONT_REMAP"]),
        fill_remap: pairs(&data["FILL_REMAP"]),
        font_families: pairs(&data["FONT_FAMILIES"]),
        rfont_families: pairs(&data["RFONT_FAMILIES"]),
        input_fill: data["INPUT_FILL"].as_str().expect("fill").into(),
    }
});

/// Python's float `%`: the result takes the sign of the (positive) divisor.
fn py_mod(x: f64, m: f64) -> f64 {
    let r = x % m;
    if r != 0.0 && (r < 0.0) != (m < 0.0) {
        r + m
    } else if r == 0.0 {
        0.0_f64.copysign(m)
    } else {
        r
    }
}

fn rgb_to_hls(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let maxc = r.max(g).max(b);
    let minc = r.min(g).min(b);
    let sumc = maxc + minc;
    let rangec = maxc - minc;
    let l = sumc / 2.0;
    if minc == maxc {
        return (0.0, l, 0.0);
    }
    let s = if l <= 0.5 {
        rangec / sumc
    } else {
        rangec / (2.0 - maxc - minc)
    };
    let rc = (maxc - r) / rangec;
    let gc = (maxc - g) / rangec;
    let bc = (maxc - b) / rangec;
    let h = if r == maxc {
        bc - gc
    } else if g == maxc {
        2.0 + rc - bc
    } else {
        4.0 + gc - rc
    };
    (py_mod(h / 6.0, 1.0), l, s)
}

const ONE_THIRD: f64 = 1.0 / 3.0;
const ONE_SIXTH: f64 = 1.0 / 6.0;
const TWO_THIRD: f64 = 2.0 / 3.0;

fn v(m1: f64, m2: f64, hue: f64) -> f64 {
    let hue = py_mod(hue, 1.0);
    if hue < ONE_SIXTH {
        return m1 + (m2 - m1) * hue * 6.0;
    }
    if hue < 0.5 {
        return m2;
    }
    if hue < TWO_THIRD {
        return m1 + (m2 - m1) * (TWO_THIRD - hue) * 6.0;
    }
    m1
}

fn hls_to_rgb(h: f64, l: f64, s: f64) -> (f64, f64, f64) {
    if s == 0.0 {
        return (l, l, l);
    }
    let m2 = if l <= 0.5 {
        l * (1.0 + s)
    } else {
        l + s - (l * s)
    };
    let m1 = 2.0 * l - m2;
    (
        v(m1, m2, h + ONE_THIRD),
        v(m1, m2, h),
        v(m1, m2, h - ONE_THIRD),
    )
}

pub fn hue_map(hex: &str) -> Option<String> {
    let channel =
        |i: usize| f64::from(u8::from_str_radix(&hex[i..i + 2], 16).expect("hex")) / 255.0;
    let (h, lightness, s) = rgb_to_hls(channel(0), channel(2), channel(4));
    let deg = h * 360.0;
    if s < 0.03 {
        return None;
    }
    let (h2, s2) = if (150.0..=225.0).contains(&deg) && s > 0.05 {
        (325.0 / 360.0, f64::min(0.35, s * 0.45))
    } else if (5.0..=40.0).contains(&deg) && s > 0.10 {
        (28.0 / 360.0, s * 0.22)
    } else {
        return None;
    };
    let (r, g, b) = hls_to_rgb(h2, lightness, s2);
    let byte = |x: f64| (x * 255.0).round_ties_even() as i64;
    Some(format!("{:02X}{:02X}{:02X}", byte(r), byte(g), byte(b)))
}

pub fn map_colour(hex: &str) -> Option<String> {
    let up = hex.to_uppercase();
    let t = &*TABLES;
    if t.keep.contains(&up) || t.explicit.values().any(|v| *v == up) {
        return None;
    }
    if let Some(new) = t.explicit.get(&up) {
        return Some(new.clone());
    }
    hue_map(&up)
}

fn recolour(text: &str) -> Result<(String, usize)> {
    let mut count = 0;
    let text = sub_fn(r#"(rgb=)"([0-9A-Fa-f]{8})""#, text, |m| {
        let (alpha, rgb) = m[2].split_at(2);
        match map_colour(rgb) {
            None => m[0].to_string(),
            Some(new) => {
                count += 1;
                format!("{}\"{alpha}{new}\"", &m[1])
            }
        }
    });
    let text = sub_fn(
        r#"(val=)"([0-9A-Fa-f]{6})"(?=/>|")"#,
        &text,
        |m| match map_colour(&m[2]) {
            None => m[0].to_string(),
            Some(new) => {
                count += 1;
                format!("{}\"{new}\"", &m[1])
            }
        },
    );
    // Fixed-point guard: nothing in the result may still be remappable.
    let residual = captures(r#"rgb="[0-9A-Fa-f]{8}""#, &text)
        .into_iter()
        .chain(captures(r#"val="[0-9A-Fa-f]{6}"(?=/>|")"#, &text));
    for token in residual {
        let hex = token[0].split('"').nth(1).expect("quoted");
        let rgb = if hex.len() == 8 { &hex[2..] } else { hex };
        if map_colour(rgb).is_some() {
            return Err(format!(
                "remap is not a fixed point: {rgb} is still remappable after one pass"
            ));
        }
    }
    Ok((text, count))
}

/// `text[:start], text[start:end], text[end:]` around one element, or `ValueError`.
fn section(text: &str, tag: &str) -> Result<(String, String, String)> {
    let start = text.find(&format!("<{tag}")).ok_or("substring not found")?;
    let close = format!("</{tag}>");
    let end = text.find(&close).ok_or("substring not found")? + close.len();
    if end < start {
        return Ok((text[..start].into(), String::new(), text[end..].into()));
    }
    Ok((
        text[..start].into(),
        text[start..end].into(),
        text[end..].into(),
    ))
}

fn theme_slots(text: &str) -> String {
    let mut text = text.to_string();
    for (slot, rgb) in &TABLES.theme {
        let new = format!("<a:{slot}><a:srgbClr val=\"{rgb}\"/></a:{slot}>");
        text = sub(&format!(r"(?s)<a:{slot}>.*?</a:{slot}>"), &text, &new);
    }
    sub(
        r#"<a:dk1><a:srgbClr val="[0-9A-Fa-f]{6}"/></a:dk1>"#,
        &text,
        &format!("<a:dk1><a:srgbClr val=\"{BLACK}\"/></a:dk1>"),
    )
}

fn tab_colours(text: &str) -> (String, usize) {
    let alternatives = TABLES
        .tabs
        .iter()
        .map(|(k, _)| k.as_str())
        .collect::<Vec<_>>()
        .join("|");
    let pattern = format!(r#"(?i)(<(?:\w+:)?tabColor rgb="FF)({alternatives})""#);
    let mut count = 0;
    let text = sub_fn(&pattern, text, |m| {
        count += 1;
        let key = m[2].to_uppercase();
        let new = lookup(&TABLES.tabs, &key).expect("tab colour");
        format!("{}{new}\"", &m[1])
    });
    (text, count)
}

/// White text only ever sits on a dark fill; make it ivory.
fn ivory_text(styles: &str) -> (String, usize) {
    let mut count = 0;
    let text = sub_fn(&format!(r"(?s)<{P}font>.*?</{P}font>"), styles, |m| {
        count += m[0].matches(r#"rgb="FFFFFFFF""#).count();
        m[0].replace(r#"rgb="FFFFFFFF""#, &format!("rgb=\"FF{IVORY}\""))
    });
    (text, count)
}

const PASSED_OLD: &str = r#"rgb="FF008000""#;

/// An xf's `fontId="…"` attribute text and its font and fill indices.
fn xf_ids(element: &str) -> Option<(String, usize, usize)> {
    let font = captures(r#"fontId="(\d+)""#, element).into_iter().next();
    let fill = captures(r#"fillId="(\d+)""#, element).into_iter().next();
    let (font, fill) = (font?, fill?);
    let index = |s: &str| {
        py::parse_int(s, 10)
            .ok()
            .and_then(|n| usize::try_from(n).ok())
    };
    let (fill_id, font_id) = (index(&fill[1])?, index(&font[1])?);
    Some((font[0].clone(), font_id, fill_id))
}

/// Points each xf with a passed fill and green font at a recoloured copy of
/// that font, appended to `fonts`. Returns the xfs, the count and whether
/// any copy was made.
fn passed_xfs(
    xfs_text: &str,
    passed: &[usize],
    fonts: &mut Vec<String>,
    new: &str,
) -> Result<(String, usize, bool)> {
    let wb = py::word_boundary();
    let mut count = 0;
    let mut copies: Vec<(usize, usize)> = Vec::new();
    let mut failure = None;
    let xfs = sub_fn(
        &format!(r"(?s)<{P}xf{wb}[^>]*?(?:/>|>.*?</{P}xf>)"),
        xfs_text,
        |m| {
            let element = &m[0];
            let Some((font_attr, font_id, fill_id)) = xf_ids(element) else {
                return element.to_string();
            };
            if !passed.contains(&fill_id) {
                return element.to_string();
            }
            let Some(font_text) = fonts.get(font_id) else {
                failure.get_or_insert_with(|| "list index out of range".to_string());
                return element.to_string();
            };
            if !font_text.contains(PASSED_OLD) {
                return element.to_string();
            }
            let copy = match copies.iter().find(|(f, _)| *f == font_id) {
                Some((_, copy)) => *copy,
                None => {
                    let copy = fonts.len();
                    fonts.push(font_text.replace(PASSED_OLD, new));
                    copies.push((font_id, copy));
                    copy
                }
            };
            count += 1;
            element.replace(&font_attr, &format!("fontId=\"{copy}\""))
        },
    );
    match failure {
        Some(failure) => Err(failure),
        None => Ok((xfs, count, !copies.is_empty())),
    }
}

/// The fonts element rebuilt around `fonts`, keeping its opening tag but its count.
fn rebuilt_fonts(fonts_text: &str, fonts: &[String]) -> Result<String> {
    let wb = py::word_boundary();
    let head = captures(&format!(r"^<{P}fonts{wb}[^>]*>"), fonts_text)
        .into_iter()
        .next()
        .ok_or("fonts element has no opening tag")?;
    let tag = head[0][1..]
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .trim_end_matches('>');
    let opening = sub(
        r#"count="\d+""#,
        &head[0],
        &format!("count=\"{}\"", fonts.len()),
    );
    Ok(format!("{opening}{}</{tag}>", fonts.concat()))
}

/// The index of each fill carrying the passed tint.
fn passed_fills(fills_text: &str, tint: &str) -> Vec<usize> {
    captures(
        &format!(r"(?s)<{P}fill\s*/>|<{P}fill>.*?</{P}fill>"),
        fills_text,
    )
    .iter()
    .enumerate()
    .filter(|(_, f)| f[0].contains(tint))
    .map(|(i, _)| i)
    .collect()
}

/// Green text on the passed tint is a passed check: recolour it, giving a
/// cell style whose green font is shared with links its own copy.
fn passed_green(styles: &str) -> Result<(String, usize)> {
    let mut count = 0;
    let new = format!("rgb=\"FF{GREEN}\"");
    let tint = format!("FF{GREEN_TINT}");
    let styles = sub_fn(&format!(r"(?s)<{P}dxf>.*?</{P}dxf>"), styles, |m| {
        if !m[0].contains(&tint) {
            return m[0].to_string();
        }
        count += m[0].matches(PASSED_OLD).count();
        m[0].replace(PASSED_OLD, &new)
    });
    let wb = py::word_boundary();
    let span = |pattern: String| {
        re(&pattern)
            .find(&styles)
            .ok()
            .flatten()
            .map(|m| (m.start(), m.end()))
    };
    let fonts_at = span(format!(r"(?s)<{P}fonts{wb}.*?</{P}fonts>"));
    let fills_at = span(format!(r"(?s)<{P}fills{wb}.*?</{P}fills>"));
    let xfs_at = span(format!(r"(?s)<{P}cellXfs{wb}.*?</{P}cellXfs>"));
    let (Some(fonts_at), Some(fills_at), Some(xfs_at)) = (fonts_at, fills_at, xfs_at) else {
        return Ok((styles, count));
    };
    let fonts_text = &styles[fonts_at.0..fonts_at.1];
    let mut fonts = captures(
        &format!(r"(?s)<{P}font\s*/>|<{P}font>.*?</{P}font>"),
        fonts_text,
    )
    .into_iter()
    .map(|m| m[0].clone())
    .collect::<Vec<_>>();
    let passed = passed_fills(&styles[fills_at.0..fills_at.1], &tint);
    let (xfs, recoloured, copied) =
        passed_xfs(&styles[xfs_at.0..xfs_at.1], &passed, &mut fonts, &new)?;
    count += recoloured;
    if !copied {
        return Ok((styles, count));
    }
    let new_fonts = rebuilt_fonts(fonts_text, &fonts)?;
    Ok((
        format!(
            "{}{new_fonts}{}{xfs}{}",
            &styles[..fonts_at.0],
            &styles[fonts_at.1..xfs_at.0],
            &styles[xfs_at.1..]
        ),
        count,
    ))
}

fn sheet_part(name: &str) -> bool {
    matches(r"^xl/worksheets/sheet\d+\.xml$", name)
}

/// Tab colours, theme slots and the palette remap shared by the workbook and templates.
fn colour_parts(parts: &mut Parts, order: &[String], changes: &mut Vec<String>) -> Result<()> {
    let mut tabs = 0;
    for name in order.iter().filter(|n| sheet_part(n)) {
        let (text, found) = tab_colours(&part_text(parts, name)?);
        if found > 0 {
            parts.insert(name.clone(), text.into_bytes());
            tabs += found;
        }
    }
    if tabs > 0 {
        changes.push(format!("tab colours set ({tabs} sheets)"));
    }
    for name in order
        .iter()
        .filter(|n| matches(r"^xl/theme/theme\d+\.xml$", n))
    {
        let text = part_text(parts, name)?;
        let slotted = theme_slots(&text);
        if slotted != text {
            parts.insert(name.clone(), slotted.into_bytes());
            changes.push(format!("{name}: theme colours set"));
        }
    }
    let mut palette = vec!["xl/styles.xml".to_string(), "xl/sharedStrings.xml".into()];
    palette.extend(
        order
            .iter()
            .filter(|n| {
                matches(
                    r"^xl/(worksheets/sheet|drawings/drawing|(drawings/)?charts/chart)\d+\.xml$",
                    n,
                )
            })
            .cloned(),
    );
    for name in palette {
        if !parts.contains_key(&name) {
            continue;
        }
        let text = part_text(parts, &name)?;
        let (recoloured, found) = recolour(&text)?;
        if recoloured != text {
            parts.insert(name.clone(), recoloured.into_bytes());
            changes.push(format!("{name}: {found} colours remapped"));
        }
    }
    Ok(())
}

/// Ivory white text and green passed checks in styles.xml, which is returned.
fn text_colours(parts: &mut Parts, changes: &mut Vec<String>) -> Result<String> {
    let styles = part_text(parts, "xl/styles.xml")?;
    let (styles, found) = ivory_text(&styles);
    if found > 0 {
        changes.push(format!("white text made ivory ({found} entries)"));
    }
    let (styles, found) = passed_green(&styles)?;
    if found > 0 {
        changes.push(format!("passed checks made green ({found} entries)"));
    }
    parts.insert("xl/styles.xml".into(), styles.clone().into_bytes());
    Ok(styles)
}

fn drawing_outlines(parts: &mut Parts, order: &[String], changes: &mut Vec<String>) -> Result<()> {
    for name in order
        .iter()
        .filter(|n| matches(r"^xl/drawings/drawing\d+\.xml$", n))
    {
        let text = part_text(parts, name)?;
        if text.contains(r#"<a:prstClr val="green"/>"#) {
            parts.insert(
                name.clone(),
                text.replace(
                    r#"<a:prstClr val="green"/>"#,
                    r#"<a:schemeClr val="accent3"/>"#,
                )
                .into_bytes(),
            );
            changes.push(format!("{name}: green outline made platinum"));
        }
    }
    Ok(())
}

fn font_families(parts: &mut Parts, styles: &mut String, changes: &mut Vec<String>) -> Result<()> {
    for (old, new) in &TABLES.font_families {
        if styles.contains(old.as_str()) {
            *styles = styles.replace(old, new);
            changes.push(format!("font family {old} -> {new}"));
        }
    }
    let mut shared = part_text(parts, "xl/sharedStrings.xml")?;
    for (old, new) in &TABLES.rfont_families {
        if shared.contains(old.as_str()) {
            shared = shared.replace(old, new);
            changes.push(format!("rich-text font {old} -> {new}"));
        }
    }
    parts.insert("xl/sharedStrings.xml".into(), shared.into_bytes());
    Ok(())
}

/// Replaces each `rgb="old"` in `text` with its `rgb="new"`; returns the hits.
fn remap_rgb(text: &mut String, remap: &[(String, String)]) -> usize {
    let mut folded = 0;
    for (old, new) in remap {
        let hits = count(text, &format!("rgb=\"{old}\""));
        if hits > 0 {
            *text = text.replace(&format!("rgb=\"{old}\""), &format!("rgb=\"{new}\""));
            folded += hits;
        }
    }
    folded
}

fn fold_fonts(styles: &mut String, changes: &mut Vec<String>) -> Result<()> {
    let (pre, mut fonts, post) = section(styles, "fonts")?;
    let folded = remap_rgb(&mut fonts, &TABLES.font_remap);
    if folded > 0 {
        changes.push(format!(
            "help-label fonts folded to palette ({folded} entries)"
        ));
    }
    *styles = format!("{pre}{fonts}{post}");
    Ok(())
}

fn fold_fills(styles: &mut String, changes: &mut Vec<String>) -> Result<()> {
    let (pre, mut fills, post) = section(styles, "fills")?;
    let mut folded = remap_rgb(&mut fills, &TABLES.fill_remap);
    let hits = count(&fills, r#"<fgColor indexed="42"/>"#);
    if hits > 0 {
        fills = fills.replace(
            r#"<fgColor indexed="42"/>"#,
            &format!("<fgColor rgb=\"FF{IVORY}\"/>"),
        );
        folded += hits;
    }
    if folded > 0 {
        changes.push(format!(
            "help-block fills folded to ivory ({folded} entries)"
        ));
    }
    let hits = count(&fills, &TABLES.input_fill);
    if hits > 0 {
        fills = fills.replace(
            &TABLES.input_fill,
            &format!("<fgColor rgb=\"FF{PLATINUM}\"/>"),
        );
        changes.push(format!("input fills made platinum ({hits} entries)"));
    }
    *styles = format!("{pre}{fills}{post}");
    Ok(())
}

pub fn run(workbook: &Path, template: bool) -> Result<Vec<String>> {
    let mut parts = read_parts(workbook)?;
    let order = parts.keys().cloned().collect::<Vec<_>>();
    let mut changes = Vec::new();
    colour_parts(&mut parts, &order, &mut changes)?;
    let mut styles = text_colours(&mut parts, &mut changes)?;

    if template {
        if changes.is_empty() {
            return Ok(vec![]);
        }
        write_deterministic(workbook, &parts)?;
        return Ok(changes);
    }

    drawing_outlines(&mut parts, &order, &mut changes)?;
    font_families(&mut parts, &mut styles, &mut changes)?;
    fold_fonts(&mut styles, &mut changes)?;
    fold_fills(&mut styles, &mut changes)?;
    parts.insert("xl/styles.xml".into(), styles.clone().into_bytes());

    // Guard: the TOC length warning keeps its warning role, now in crimson.
    let start = styles.find("<dxfs").ok_or("substring not found")?;
    let end = styles.find("</dxfs>").ok_or("substring not found")?;
    let dxfs = styles.get(start..end).unwrap_or_default();
    if !(dxfs.contains(&format!("FF{CRIMSON_TINT}")) && dxfs.contains(&format!("FF{CRIMSON}"))) {
        return Err("TOC warning colours are missing".into());
    }

    if changes.is_empty() {
        return Ok(vec![]);
    }
    write_deterministic(workbook, &parts)?;
    Ok(changes)
}

pub fn command(args: &[String]) -> Output {
    let template = args.first().is_some_and(|a| a == "--template");
    let args = if template { &args[1..] } else { args };
    if args.len() > 1 {
        return Output::stderr("FAIL: usage: ozzit workbook_palette [--template] [workbook]");
    }
    let workbook = py::PyPath::new(args.first().map_or("ozzit.xlsx", String::as_str));
    if !workbook.fs().is_file() {
        return Output::stderr(format!("FAIL: no such workbook: {workbook}"));
    }
    match run(&workbook.fs(), template) {
        Err(e) => Output::stderr(format!("FAIL: {e}")),
        Ok(mut changes) => {
            if changes.is_empty() {
                changes.push("Palette already applied; no changes".into());
            }
            changes.push(format!("OK: {workbook}"));
            Output::ok(changes)
        }
    }
}
