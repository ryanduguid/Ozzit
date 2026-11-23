// Copyright 2026 Ryan Duguid
// SPDX-License-Identifier: Apache-2.0

//! A text baseline for the reviewed workbook. Capture is the only operation that reads an XLSX.
use crate::{
    afe,
    archive::{self, BOOK, Parts},
    common::{self, Output, Result},
    compiler, formula, workbook,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};

use std::{collections::BTreeMap, path::Path};

const AFE: &str = "customXml/item1.xml";
const STORE: &str = "{{ozzit-afe}}";

/// Keep only the layout Excel added to a compiled formula: spaces outside quoted
/// strings and local workbook qualifiers. Counts are UTF-8 bytes from the source.
fn layout(body: &str) -> String {
    let mut mask = String::new();
    let mut take = 0;
    let gaps = common::re(r"\[0\]!|\s+");
    for (literal, piece) in formula::split_literals(body) {
        if literal {
            take += piece.len();
            continue;
        }
        let mut last = 0;
        for gap in gaps.find_iter(piece) {
            let gap = gap.expect("fixed layout pattern");
            take += gap.start() - last;
            mask.push_str(&take.to_string());
            mask.push_str(gap.as_str());
            take = 0;
            last = gap.end();
        }
        take += piece.len() - last;
    }
    mask.push_str(&take.to_string());
    mask
}

fn render(mask: &str, compiled: &str) -> Result<String> {
    let mut body = String::new();
    let mut rest = compiled;
    let mut layout = mask;
    loop {
        let digits = layout.len()
            - layout
                .trim_start_matches(|c: char| c.is_ascii_digit())
                .len();
        let n = layout[..digits]
            .parse::<usize>()
            .map_err(|_| "invalid formula layout")?;
        let (chunk, tail) = rest
            .split_at_checked(n)
            .ok_or("formula layout exceeds source")?;
        body.push_str(chunk);
        rest = tail;
        layout = &layout[digits..];
        if layout.is_empty() {
            return rest
                .is_empty()
                .then_some(body)
                .ok_or_else(|| "incomplete formula layout".into());
        }
        let tail = layout
            .strip_prefix("[0]!")
            .unwrap_or_else(|| layout.trim_start_matches(char::is_whitespace));
        if tail.len() == layout.len() {
            return Err("unexpected formula layout character".into());
        }
        body.push_str(&layout[..layout.len() - tail.len()]);
        layout = tail;
    }
}

fn transform_names(book: &str, src: &Path, capture: bool) -> Result<String> {
    let mut definitions: BTreeMap<_, _> = compiler::compile_sources(src)?
        .into_iter()
        .map(|c| (c.name, (formula::tight(&c.stored), c.comment)))
        .collect();
    let mut result = String::with_capacity(book.len());
    let mut last = 0;
    for m in common::re(formula::NAME_RE).captures_iter(book) {
        let m = m.map_err(|e| e.to_string())?;
        let name = &m[1];
        let (source, comment) = definitions
            .remove(name)
            .ok_or_else(|| format!("unexpected or duplicate defined name {name}"))?;
        let attr = common::captures(r#" comment="([^"]*)""#, &m[2]);
        if attr.first().map(|a| common::unescape(&a[1])).transpose()? != Some(comment) {
            return Err(format!("{name} comment differs from the source header"));
        }
        let digest = common::sha256_hex(source.as_bytes());
        let span = m.get(3).expect("defined-name body");
        result.push_str(&book[last..span.start()]);
        if capture {
            let body = common::unescape(span.as_str())?;
            let mask = layout(&body);
            if common::escape(&render(&mask, &source)?) != span.as_str() {
                return Err(format!("{name} differs from the compiled source"));
            }
            result.push_str(&format!("{digest}:{mask}"));
        } else {
            let (hash, mask) = span
                .as_str()
                .split_once(':')
                .ok_or_else(|| format!("missing formula layout for {name}"))?;
            if hash != digest {
                return Err(format!(
                    "{name} changed: capture the verified workbook after native recalculation"
                ));
            }
            let body = render(mask, &source)?;
            if formula::tight(&body) != source {
                return Err(format!("layout changes the formula for {name}"));
            }
            result.push_str(&common::escape(&body));
        }
        last = span.end();
    }
    if let Some(name) = definitions.keys().next() {
        return Err(format!("source name {name} missing from workbook layout"));
    }
    result.push_str(&book[last..]);
    Ok(result)
}

fn swap_once(text: &str, from: &str, to: &str) -> Result<String> {
    if text.matches(from).count() != 1 {
        return Err("expected exactly one AFE payload".into());
    }
    Ok(text.replacen(from, to, 1))
}

fn push_tags<'a>(xml: &'a str, rows: &mut Vec<&'a str>) {
    let mut last = 0;
    for (at, _) in xml.match_indices("><") {
        rows.push(&xml[last..=at]);
        last = at + 1;
    }
    if last < xml.len() {
        rows.push(&xml[last..]);
    }
}

/// Keep a worksheet cell's address, formula and cached result on one diff line.
/// Slice the original XML so entity spellings and line endings survive capture.
fn xml_rows<'a>(name: &str, xml: &'a str) -> Result<Vec<&'a str>> {
    let mut rows = Vec::new();
    let mut last = 0;
    if name.starts_with("xl/worksheets/") && name.ends_with(".xml") {
        let doc = workbook::xml_part(name, xml.as_bytes())?;
        let cells = doc
            .root_element()
            .children()
            .filter(|n| n.has_tag_name("sheetData"))
            .flat_map(|data| data.children().filter(|n| n.has_tag_name("row")))
            .flat_map(|row| row.children().filter(|n| n.has_tag_name("c")));
        for cell in cells {
            let span = cell.range();
            push_tags(&xml[last..span.start], &mut rows);
            last = span.end;
            rows.push(&xml[span]);
        }
    }
    push_tags(&xml[last..], &mut rows);
    Ok(rows)
}

fn module_texts(store: &mut Value, src: &Path, capture: bool) -> Result<()> {
    let files = store
        .get_mut("files")
        .and_then(Value::as_array_mut)
        .ok_or("missing AFE files")?;
    for module in formula::MODULES {
        let path = format!("/projects/{module}");
        let mut matching = files.iter_mut().filter(|f| f["path"] == path);
        let file = matching
            .next()
            .ok_or_else(|| format!("missing AFE module {module}"))?;
        if matching.next().is_some() {
            return Err(format!("duplicate AFE module {module}"));
        }
        let text = common::read_text(&src.join(format!("{module}.txt")))?;
        if capture {
            if file["text"] != text {
                return Err(format!("AFE module {module} differs from src/"));
            }
            file.as_object_mut()
                .ok_or("invalid AFE file")?
                .shift_remove("text");
        } else {
            if file.get("text").is_some() {
                return Err(format!("duplicate source text for {module}"));
            }
            file["text"] = Value::String(text);
        }
    }
    Ok(())
}

fn capture(workbook: &Path, src: &Path) -> Result<Vec<u8>> {
    let (parts, names) = archive::read_inventory(workbook)?;
    if names.len() != parts.len() {
        return Err("duplicate ZIP member".into());
    }
    let mut document = json!({"afe": null, "parts": {}});
    // The text layout has a stable order even when the input ZIP order varies.
    let sorted: BTreeMap<_, _> = parts.iter().collect();
    for (name, data) in sorted {
        let mut xml = common::decode(data)?;
        if name == BOOK {
            xml = transform_names(&xml, src, true)?;
        } else if name == AFE {
            let encoded = afe::find_blob(data)?;
            let decoded = afe::base64_decode_strict(encoded)?;
            let store = &mut document["afe"];
            *store = serde_json::from_str(&afe::utf16le(&decoded)?).map_err(|e| e.to_string())?;
            module_texts(store, src, true)?;
            xml = swap_once(
                &xml,
                std::str::from_utf8(encoded).map_err(|e| e.to_string())?,
                STORE,
            )?;
        }
        document["parts"][name] = json!(xml_rows(name, &xml)?);
    }
    if build(&document, src)? != common::read_bytes(workbook)? {
        return Err("capture did not regenerate the input workbook byte for byte".into());
    }
    let mut bytes = serde_json::to_vec_pretty(&document).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn build(document: &Value, src: &Path) -> Result<Vec<u8>> {
    let rows = document["parts"]
        .as_object()
        .ok_or("missing workbook parts")?;
    let mut store = document["afe"].clone();
    module_texts(&mut store, src, false)?;
    let json = serde_json::to_string(&store).map_err(|e| e.to_string())?;
    let encoded = STANDARD.encode(
        json.encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>(),
    );
    let mut parts = Parts::new();
    for (name, rows) in rows {
        if !(name.ends_with(".xml") || name.ends_with(".rels"))
            || name.split('/').any(|segment| {
                segment.is_empty()
                    || segment == "."
                    || segment == ".."
                    || !segment
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._-[]".contains(&b))
            })
        {
            return Err(format!("invalid XML part name: {name}"));
        }
        let mut xml = String::new();
        for row in rows.as_array().ok_or("invalid XML rows")? {
            xml.push_str(row.as_str().ok_or("expected XML text")?);
        }
        if name == BOOK {
            xml = transform_names(&xml, src, false)?;
        } else if name == AFE {
            xml = swap_once(&xml, STORE, &encoded)?;
        }
        parts.insert(name.clone(), xml.into_bytes());
    }
    if !parts.contains_key(BOOK) || !parts.contains_key(AFE) {
        return Err("missing workbook or AFE part".into());
    }
    archive::deterministic_bytes(&parts)
}

pub fn command(args: &[String]) -> Output {
    let [mode, input, src, output] = args else {
        return Output::stderr(
            "usage: ozzit workbook_source <build|capture> <input> <src dir> <new output>",
        );
    };
    let run = || -> Result<()> {
        let input = Path::new(input);
        let src = Path::new(src);
        let output = Path::new(output);
        let bytes = match mode.as_str() {
            "build" => {
                let document = serde_json::from_slice(&common::read_bytes(input)?)
                    .map_err(|e| e.to_string())?;
                build(&document, src)?
            }
            "capture" => capture(input, src)?,
            _ => return Err("expected build or capture".into()),
        };
        common::stage_bytes(output, &bytes)?
            .persist_noclobber(output)
            .map_err(|e| e.to_string())?;
        Ok(())
    };
    match run() {
        Ok(()) => Output::ok(vec![format!("Wrote {output}")]),
        Err(error) => Output::stderr(format!("FAIL: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worksheet_cells_keep_their_original_xml_on_one_diff_line() {
        let formula = r#"<c r="A1"><f>IF(1&lt;2,3,4)</f><v>3</v></c>"#;
        let inline = "<c r=\"B1\" t=\"inlineStr\"><is><t>λ\r\n&amp;</t></is></c>";
        let empty = r#"<c r="C1"/>"#;
        let xml = format!(
            "<worksheet><sheetData><row>{formula}{inline}{empty}</row></sheetData></worksheet>"
        );
        let rows = xml_rows("xl/worksheets/sheet1.xml", &xml).unwrap();
        assert_eq!(
            rows,
            [
                "<worksheet>",
                "<sheetData>",
                "<row>",
                formula,
                inline,
                empty,
                "</row>",
                "</sheetData>",
                "</worksheet>"
            ]
        );
        assert_eq!(rows.concat(), xml);
    }
}
