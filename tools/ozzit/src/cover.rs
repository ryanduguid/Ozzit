//! The displayed cover label must agree with the published inventory and cut date.
use crate::{archive::*, common::*, py};
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use std::path::Path;

/// The worksheet part behind the sheet named Cover.
fn cover_part(archive: &mut ZipReader) -> Result<String> {
    let book = member_text(archive, BOOK)?;
    let sheet = captures(r#"<sheet [^>]*name="Cover"[^>]*r:id="(rId\d+)""#, &book);
    let sheet = sheet
        .first()
        .ok_or("xl/workbook.xml declares no sheet named Cover")?;
    let rels = member_text(archive, "xl/_rels/workbook.xml.rels")?;
    let target = captures(
        &format!(
            r#"<Relationship [^>]*Id="{}"[^>]*Target="([^"]+)""#,
            sheet[1]
        ),
        &rels,
    );
    let target = target
        .first()
        .ok_or("the Cover sheet has no relationship target")?;
    let target = target[1].trim_start_matches('/');
    let part = format!("xl/{}", target.strip_prefix("xl/").unwrap_or(target));
    Ok(part)
}

pub fn cover_label(workbook: &Path) -> Result<(String, BigInt)> {
    let mut archive = open_zip(workbook)?;
    let part = cover_part(&mut archive)?;
    let sheet = member_text(&mut archive, &part)?;
    let cell = captures(r#"(?s)<c r="A3"([^>]*)>(.*?)</c>"#, &sheet);
    let cell = cell
        .first()
        .filter(|c| c[1].contains("t=\"s\""))
        .ok_or_else(|| format!("{part}: Cover!A3 is not a shared-string cell"))?;
    let index = captures(r"<v>(\d+)</v>", &cell[2]);
    let index = index
        .first()
        .ok_or_else(|| format!("{part}: Cover!A3 carries no shared-string index"))?;
    let position = py::parse_int(&index[1], 10)?;
    let items = captures(
        r"(?s)<si>(.*?)</si>",
        &member_text(&mut archive, "xl/sharedStrings.xml")?,
    );
    let item = position
        .to_usize()
        .and_then(|p| items.get(p))
        .ok_or_else(|| {
            format!(
                "Cover!A3 points at shared string {position}, beyond the {} stored",
                items.len()
            )
        })?;
    let value = unescape(
        &captures(r"(?s)<t[^>]*>(.*?)</t>", &item[1])
            .into_iter()
            .map(|m| m[1].clone())
            .collect::<String>(),
    )?;
    let value = py::strip(&value);
    let label = captures(r"^Version (.+?)\s+-\s+(\d+) functions\s+-\s+.+$", value);
    let label = label
        .first()
        .ok_or_else(|| format!("Cover!A3 is not a version label: {}", py::repr_str(value)))?;
    Ok((py::strip(&label[1]).into(), py::parse_int(&label[2], 10)?))
}

pub fn published_functions(index: &Path) -> Result<usize> {
    let csv = Csv::read(index)?;
    if csv
        .headers
        .iter()
        .filter(|h| h.as_str() == "function")
        .count()
        > 1
    {
        return Err(format!(
            "{}: duplicate CSV column(s): function",
            basename(index)
        ));
    }
    if csv.rows.is_empty() {
        return Err("functions.csv lists no functions".into());
    }
    if !csv.headers.iter().any(|h| h == "function") {
        return Err("'function'".into());
    }
    let names = csv
        .rows
        .iter()
        .map(|r| csv.get(r, "function"))
        .collect::<Vec<_>>();
    if names
        .iter()
        .any(|s| s.is_none_or(|s| py::strip(s).is_empty()))
    {
        return Err("functions.csv contains an empty function name".into());
    }
    Ok(names
        .iter()
        .filter(|s| !s.expect("checked name").starts_with("oz.About"))
        .count())
}

pub fn current_cut_date(changelog: &Path) -> Result<String> {
    let text = py::universal_newlines(&read_sig(changelog)?);
    captures(
        r"(?m)^## v\d+\.\d+\.\d+, (\d{1,2} [A-Z][a-z]+ \d{4}),",
        &text,
    )
    .first()
    .map(|m| m[1].clone())
    .ok_or_else(|| "CHANGELOG.md has no '## vX.Y.Z, <date>,' heading".into())
}

pub fn verify(args: &[String]) -> Output {
    if args.len() > 3 {
        return Output::stderr(
            "FAIL: usage: ozzit verify_cover [workbook] [functions.csv] [CHANGELOG.md]",
        );
    }
    let run = || -> Result<Output> {
        let workbook = py::PyPath::new(args.first().map_or("ozzit.xlsx", String::as_str)).fs();
        let index = py::PyPath::new(args.get(1).map_or("functions.csv", String::as_str)).fs();
        let changelog = py::PyPath::new(args.get(2).map_or("CHANGELOG.md", String::as_str)).fs();
        let (date, count) = cover_label(&workbook)?;
        let functions = published_functions(&index)?;
        let cut = current_cut_date(&changelog)?;
        let mut failures = Vec::new();
        if count != BigInt::from(functions) {
            failures.push(format!(
                "FAIL: cover says {count} functions; functions.csv publishes {functions}"
            ));
        }
        if date != cut {
            failures.push(format!(
                "FAIL: cover is dated {date}; the changelog's current cut is {cut}"
            ));
        }
        if failures.is_empty() {
            Ok(Output::ok(vec![format!(
                "OK: cover label states {cut} and {functions} functions"
            )]))
        } else {
            Ok(Output {
                code: 1,
                out: failures,
                err: vec![],
            })
        }
    };
    run().unwrap_or_else(|e| Output::failure(format!("FAIL: {e}")))
}
