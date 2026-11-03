//! Bind the companion's saved native result to its workbook and test inputs.
//!
//! This checks retained evidence, not Excel calculation on this host. Run the
//! PowerShell test in Excel to refresh evidence after a material input changes.
use super::xml::{self, MAIN, REL};
use super::{json, names, part, parts, read_sig, sha256};
use ozzit::archive::Parts;
use roxmltree::Node;
use std::collections::BTreeSet;

const WORKBOOK: &str = "templates/practice-calculators.xlsx";

fn luminance(rgb: &str) -> f64 {
    let channel =
        |i: usize| f64::from(u8::from_str_radix(&rgb[i..i + 2], 16).expect("a hex colour")) / 255.0;
    let linear = |c: f64| {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    [0, 2, 4]
        .into_iter()
        .zip([0.2126, 0.7152, 0.0722])
        .map(|(i, weight)| linear(channel(i)) * weight)
        .sum()
}

fn contrast(foreground: &str, background: &str) -> f64 {
    let (a, b) = (luminance(foreground), luminance(background));
    let (light, dark) = if a >= b { (a, b) } else { (b, a) };
    (light + 0.05) / (dark + 0.05)
}

/// A conditional style's font and fill colour nodes, when it sets both.
fn conditional_colour_pairs<'a, 'i>(style: Node<'a, 'i>) -> Vec<(Node<'a, 'i>, Node<'a, 'i>)> {
    let font = xml::path(style, MAIN, &["font", "color"])
        .into_iter()
        .next();
    let fill = xml::path(style, MAIN, &["fill", "patternFill", "fgColor"])
        .into_iter()
        .next()
        .or_else(|| {
            xml::path(style, MAIN, &["fill", "patternFill", "bgColor"])
                .into_iter()
                .next()
        });
    match (font, fill) {
        (Some(font), Some(fill)) => vec![(font, fill)],
        _ => vec![],
    }
}

/// An explicit colour's last 6 hex digits. The companion deliberately exports
/// explicit colours; unknown theme or indexed colours need review rather than a
/// guessed pass.
fn colour(node: Option<Node<'_, '_>>, fallback: &str) -> String {
    let Some(node) = node else {
        return fallback.to_owned();
    };
    let rgb = node
        .attribute("rgb")
        .unwrap_or_else(|| panic!("<{}> has no rgb colour", node.tag_name().name()));
    rgb[rgb.len() - 6..].to_owned()
}

/// Each sheet's name and part, in workbook order.
fn sheets(parts: &Parts) -> Vec<(String, String)> {
    let rels = part(parts, "xl/_rels/workbook.xml.rels");
    let rels = xml::parse(&rels);
    let book = part(parts, "xl/workbook.xml");
    let book = xml::parse(&book);
    xml::path(book.root_element(), MAIN, &["sheets", "sheet"])
        .into_iter()
        .map(|sheet| {
            let id = sheet.attribute((REL, "id")).expect("r:id");
            let target = xml::elements(rels.root_element())
                .filter(|r| r.attribute("Id") == Some(id))
                .last()
                .and_then(|r| r.attribute("Target"))
                .unwrap_or_else(|| panic!("no relationship {id}"));
            let path = if target.starts_with('/') {
                target.trim_start_matches('/').to_owned()
            } else {
                format!("xl/{target}")
            };
            (sheet.attribute("name").expect("name").to_owned(), path)
        })
        .collect()
}

#[test]
fn native_receipt_matches_saved_inputs() {
    let receipt = json(&read_sig("docs/practice-native-evidence.json"));
    for (relative, field) in [
        (WORKBOOK, "workbook_sha256"),
        ("tools/excel_practice_selftest.ps1", "runner_sha256"),
        ("tools/build_practice_calculators.mjs", "builder_sha256"),
        ("tools/prepare_practice_template.ps1", "preparer_sha256"),
    ] {
        assert_eq!(
            sha256(relative),
            receipt[field],
            "path={relative}: rerun native Excel verification"
        );
    }
    assert_eq!(receipt["result"], "PASS");
    assert_eq!(receipt["baseline_errors"], 0);
    assert!(
        receipt["assertions"]
            .as_f64()
            .expect("a count of assertions")
            > 0.0
    );
    assert!(
        receipt["excel"]
            .as_str()
            .expect("an Excel version")
            .contains("build")
    );
    let names: BTreeSet<String> = sheets(&parts(WORKBOOK))
        .into_iter()
        .map(|(name, _)| name)
        .filter(|name| name != "Read me")
        .collect();
    let workflows: BTreeSet<String> = receipt["workflows"]
        .as_array()
        .expect("a list of workflows")
        .iter()
        .map(|w| w.as_str().expect("a workflow name").to_owned())
        .collect();
    assert_eq!(names, workflows);
}

#[test]
fn companion_has_no_executable_or_external_workbook_parts() {
    let parts = parts(WORKBOOK);
    let names = names(WORKBOOK);
    assert!(
        !names
            .iter()
            .any(|p| p.to_lowercase().contains("vbaproject"))
    );
    assert!(!names.iter().any(|p| p.starts_with("xl/externalLinks/")));
    assert!(!part(&parts, "[Content_Types].xml").contains("macroEnabled"));
    // Images and hyperlinks would be legitimate, but this companion
    // deliberately needs no external relationships at all.
    for name in names.iter().filter(|name| name.ends_with(".rels")) {
        let text = part(&parts, name);
        let relations = xml::parse(&text);
        assert!(
            !xml::elements(relations.root_element())
                .any(|r| r.attribute("TargetMode") == Some("External")),
            "{name}"
        );
    }
}

#[test]
fn native_navigation_targets_existing_calculators() {
    let parts = parts(WORKBOOK);
    let sheets = sheets(&parts);
    for (name, path) in &sheets {
        let text = part(&parts, path);
        let sheet = xml::parse(&text);
        let actual: BTreeSet<(String, String)> =
            xml::path(sheet.root_element(), MAIN, &["hyperlinks", "hyperlink"])
                .into_iter()
                .map(|link| {
                    let field = |key: &str| {
                        link.attribute(key)
                            .unwrap_or_else(|| panic!("a hyperlink without {key}"))
                    };
                    (field("ref").to_owned(), field("location").to_owned())
                })
                .collect();
        let expected: BTreeSet<(String, String)> = if name == "Read me" {
            sheets[1..]
                .iter()
                .enumerate()
                .map(|(i, (target, _))| (format!("B{}", 14 + i), format!("'{target}'!B2")))
                .collect()
        } else {
            [("B1".to_owned(), "'Read me'!B2".to_owned())].into()
        };
        assert_eq!(actual, expected, "{name}");
    }
}

#[test]
fn fbt_rate_note_names_its_fbt_years() {
    let parts = parts(WORKBOOK);
    let (_, path) = sheets(&parts)
        .into_iter()
        .find(|(name, _)| name == "Benefit totals")
        .expect("a Benefit totals sheet");
    let text = part(&parts, &path);
    let sheet = xml::parse(&text);
    let cell = xml::path(sheet.root_element(), MAIN, &["sheetData", "row", "c"])
        .into_iter()
        .find(|c| c.attribute("r") == Some("F7"))
        .expect("a cell F7");
    // Native Excel preparation stores the note as a shared string.
    assert_eq!(cell.attribute("t"), Some("s"));
    let strings = part(&parts, "xl/sharedStrings.xml");
    let strings = xml::parse(&strings);
    let index: usize = xml::child(cell, MAIN, "v")
        .and_then(|v| v.text())
        .and_then(|v| v.parse().ok())
        .expect("a shared string index");
    let item = xml::children(strings.root_element(), MAIN, "si")
        .nth(index)
        .unwrap_or_else(|| panic!("no shared string {index}"));
    let note = xml::text(item);
    assert!(
        note.contains("FBT years ending 31 March 2023 to 31 March 2027"),
        "{note}"
    );
}

/// A cell style's text contrast and its floor: 3 for large text, 4.5 otherwise.
fn cell_contrast(font: Node<'_, '_>, fill: Node<'_, '_>) -> (f64, f64) {
    let foreground = colour(xml::child(font, MAIN, "color"), "000000");
    let background = colour(
        xml::path(fill, MAIN, &["patternFill", "fgColor"])
            .into_iter()
            .next(),
        "FFFFFF",
    );
    let points = xml::child(font, MAIN, "sz").map_or(11.0, |size| {
        ozzit::py::parse_float(size.attribute("val").expect("a font size")).expect("a font size")
    });
    let large = points >= 18.0 || (points >= 14.0 && xml::child(font, MAIN, "b").is_some());
    let floor = if large { 3.0 } else { 4.5 };
    (contrast(&foreground, &background), floor)
}

#[test]
fn text_contrast_on_every_populated_cell_and_conditional_style() {
    let parts = parts(WORKBOOK);
    let styles = part(&parts, "xl/styles.xml");
    let styles = xml::parse(&styles);
    let list = |steps: &[&'static str]| xml::path(styles.root_element(), MAIN, steps);
    let (fonts, fills, formats) = (
        list(&["fonts", "font"]),
        list(&["fills", "fill"]),
        list(&["cellXfs", "xf"]),
    );
    for name in names(WORKBOOK) {
        if !name.starts_with("xl/worksheets/sheet") || !name.ends_with(".xml") {
            continue;
        }
        let text = part(&parts, &name);
        for cell in xml::descendants(xml::parse(&text).root_element(), MAIN, "c") {
            if xml::child(cell, MAIN, "v")
                .and_then(|v| v.text())
                .is_none_or(str::is_empty)
            {
                continue;
            }
            let style = formats[xml::index(cell, "s")];
            let (ratio, floor) = cell_contrast(
                fonts[xml::index(style, "fontId")],
                fills[xml::index(style, "fillId")],
            );
            let at = cell.attribute("r").unwrap_or_default();
            assert!(
                ratio >= floor,
                "cell={name}!{at}: contrast {ratio} below {floor}"
            );
        }
    }
    for (index, style) in list(&["dxfs", "dxf"]).into_iter().enumerate() {
        for (font, fill) in conditional_colour_pairs(style) {
            let ratio = contrast(&colour(Some(font), "000000"), &colour(Some(fill), "FFFFFF"));
            assert!(ratio >= 4.5, "conditional_style={index}: contrast {ratio}");
        }
    }
}

#[test]
fn conditional_colour_encodings_and_uncoloured_rules() {
    for tag in ["fgColor", "bgColor"] {
        let text = format!(
            r#"<dxf xmlns="{MAIN}"><font><color rgb="FF000000"/></font><fill><patternFill><{tag} rgb="FFFFFFFF"/></patternFill></fill></dxf>"#
        );
        let style = xml::parse(&text);
        let actual: Vec<(&str, &str)> = conditional_colour_pairs(style.root_element())
            .into_iter()
            .map(|(font, fill)| {
                (
                    font.attribute("rgb").unwrap(),
                    fill.attribute("rgb").unwrap(),
                )
            })
            .collect();
        assert_eq!(actual, [("FF000000", "FFFFFFFF")], "{tag}");
    }
    for content in ["", r#"<font><color rgb="FF000000"/></font>"#] {
        let text = format!(r#"<dxf xmlns="{MAIN}">{content}</dxf>"#);
        let style = xml::parse(&text);
        assert!(
            conditional_colour_pairs(style.root_element()).is_empty(),
            "{content}"
        );
    }
}
