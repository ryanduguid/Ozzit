//! Compare the predictable help tables with saved cells; never recalculate or write caches.
use crate::{archive::*, common::*, formula, py};
use num_bigint::BigInt;
use num_traits::{ToPrimitive, Zero};
use std::collections::BTreeMap;

const ANCHOR: &str = r#"<c r="([A-Z]+)(\d+)"([^>]*)><f\b(?=[^>]*\st="array")(?=[^>]*\sref="([A-Z]+\d+:[A-Z]+\d+)")([^>]*)>(oz\.[A-Za-z0-9_]+λ(?:DV)?)\(\)</f>(?:<v(?:/>|>[^<]*</v>))?</c>"#;
const CELL: &str = r#"(?s)<c r="([A-Z]+)(\d+)"([^>]*?)(?:/>|>(.*?)</c>)"#;

pub fn literal_text(code: &str) -> String {
    formula::split_literals(code)
        .into_iter()
        .filter(|(lit, _)| *lit)
        .map(|(_, s)| {
            let end = s.len().saturating_sub(1).max(1);
            s[1..end].replace("\"\"", "\"")
        })
        .collect()
}
pub fn trim(text: &str) -> String {
    sub(" {2,}", text, " ").trim_matches(' ').to_owned()
}
pub fn help_table(stored: &str, name: &str) -> Result<Vec<Vec<String>>> {
    let at = stored
        .find("TEXTSPLIT(")
        .ok_or_else(|| format!("{name}: the definition has no TEXTSPLIT help"))?;
    let args = formula::arguments(stored, at + "TEXTSPLIT(".len());
    if args.len() < 3 || literal_text(&args[1]) != "→" || literal_text(&args[2]) != "¶" {
        return Err(format!("{name}: the help is not split on → and ¶"));
    }
    literal_text(&args[0])
        .split('¶')
        .map(|raw| {
            let cells = raw.split('→').collect::<Vec<_>>();
            if cells.len() != 2 {
                return Err(format!(
                    "{name}: a help row has {} arrows, so Excel would pad it with #N/A: {}",
                    cells.len() - 1,
                    py::repr_str(prefix(raw, 60))
                ));
            }
            Ok(cells.into_iter().map(trim).collect())
        })
        .collect()
}
pub fn defined_names(book: &str) -> BTreeMap<String, String> {
    captures(
        r#"(?s)<definedName name="(oz\.[^"]+)"[^>]*>(.*?)</definedName>"#,
        book,
    )
    .into_iter()
    .map(|m| (m[1].clone(), unescape(&m[2])))
    .collect()
}
fn column_index(letters: &str) -> BigInt {
    letters
        .bytes()
        .fold(BigInt::zero(), |n, c| n * 26 + u32::from(c) - 64)
}
fn cached_values(
    sheet: &str,
    top: &BigInt,
    left: &BigInt,
    rows: &BigInt,
    cols: &BigInt,
) -> Result<BTreeMap<(BigInt, BigInt), String>> {
    let mut cells = BTreeMap::new();
    for cell in captures(CELL, sheet) {
        let column = column_index(&cell[1]);
        let row = py::parse_int(&cell[2], 10)?;
        if top <= &row && row < top + rows && left <= &column && column < left + cols {
            let value = captures(r"(?s)<v>(.*?)</v>", &cell[4]);
            cells.insert(
                (row, column),
                value.first().map_or_else(String::new, |m| unescape(&m[1])),
            );
        }
    }
    Ok(cells)
}
fn letters(mut number: BigInt) -> String {
    let mut chars = Vec::new();
    while number > BigInt::zero() {
        number -= 1;
        chars.push(char::from(
            65 + (&number % BigInt::from(26))
                .to_u8()
                .expect("column remainder"),
        ));
        number /= 26;
    }
    chars.into_iter().rev().collect()
}

pub fn check(parts: &Parts) -> Result<(Vec<String>, usize)> {
    let names = defined_names(&decode(parts.get(BOOK).ok_or_else(|| py::repr_str(BOOK))?)?);
    let mut stale = Vec::new();
    let mut anchors = 0;
    let mut sheets = parts
        .keys()
        .filter(|n| matches(r"^xl/worksheets/sheet\d+\.xml$", n))
        .collect::<Vec<_>>();
    sheets.sort();
    for part in sheets {
        let sheet = text(parts, part)?;
        for anchor in captures(ANCHOR, &sheet) {
            anchors += 1;
            let left = column_index(&anchor[1]);
            let top = py::parse_int(&anchor[2], 10)?;
            let last = anchor[4].split(':').nth(1).expect("matched range");
            let rows = py::parse_int(&sub("[A-Z]", last, ""), 10)? - &top + 1;
            let cols = column_index(&sub(r"\d", last, "")) - &left + 1;
            let name = &anchor[6];
            let body = names
                .get(name)
                .ok_or_else(|| format!("{part}: {name} is not a defined name"))?;
            if !anchor[0].contains("<v") || anchor[0].contains("<v/>") {
                stale.push(format!("{part}: the {name} help has no cached value at its anchor; Excel has not recalculated and saved this sheet"));
                continue;
            }
            let table = help_table(body, name)?;
            // Compare sparse cached cells only over a matching help shape. This
            // retains every cell's validation without allocating a corrupt range.
            let cached = cached_values(&sheet, &top, &left, &rows, &cols)?;
            let detail =
                if rows == BigInt::from(table.len()) && cols == BigInt::from(table[0].len()) {
                    let first = table.iter().enumerate().find_map(|(r, row)| {
                        row.iter().enumerate().find_map(|(c, value)| {
                            let key = (&top + r, &left + c);
                            (cached.get(&key).map_or("", String::as_str) != value).then_some((r, c))
                        })
                    });
                    let Some((row, col)) = first else { continue };
                    format!(
                        "first difference at {}{}",
                        letters(&left + col),
                        py::int_repr(&(&top + row))?
                    )
                } else {
                    let mut detail = format!(
                        "{} cached rows against {} in the definition",
                        py::int_repr(&rows)?,
                        table.len()
                    );
                    if cols != BigInt::from(table[0].len()) {
                        detail.push_str(&format!(
                            ", {} cached columns against {}",
                            py::int_repr(&cols)?,
                            table[0].len()
                        ));
                    }
                    detail
                };
            stale.push(format!("{part}: the {name} help is stale, {detail}"));
        }
    }
    Ok((stale, anchors))
}
pub fn verify(args: &[String]) -> Output {
    if args.len() > 1 {
        return Output::stderr("FAIL: usage: ozzit verify_help_spills [workbook]");
    }
    let path = py::PyPath::new(args.first().map_or("ozzit.xlsx", String::as_str)).fs();
    let workbook = path.as_path();
    if !workbook.is_file() {
        return Output::stderr(format!("FAIL: no such workbook: {}", display(workbook)));
    }
    let (stale, anchors) = match read_parts(workbook).and_then(|p| check(&p)) {
        Ok(result) => result,
        Err(e) => return Output::stderr(format!("FAIL: {e}")),
    };
    if anchors == 0 {
        return Output::stderr(format!(
            "FAIL: no help anchor found in {}, so no cached help was checked",
            display(workbook)
        ));
    }
    if stale.is_empty() {
        Output::ok(vec![format!(
            "OK: every cached help in {} matches its definition, {anchors} anchors",
            display(workbook)
        )])
    } else {
        Output {
            code: 1,
            err: vec![format!(
                "FAIL: {} of {anchors} cached helps in {} no longer match their definitions; run tools/refresh_cache.py in Excel, then tools/sanitise_workbook.py, and tools/verify_cache.py proves the result",
                stale.len(),
                display(workbook)
            )],
            out: stale,
        }
    }
}
