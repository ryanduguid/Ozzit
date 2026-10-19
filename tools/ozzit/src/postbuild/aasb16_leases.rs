//! AASB 16 lease pass: 4 lessee functions added to the Financial group.
//!
//! One spec per function generates both stores: the stored form is rendered
//! from the published source and checked with the comparison verify_sources
//! uses before anything is written.
use super::*;
use crate::formula;
use std::collections::BTreeSet;
use std::sync::LazyLock;

const MODULE: &str = "Financial";

struct Function {
    name: String,
    comment: String,
    block: String,
    about: String,
}

struct Tables {
    functions: Vec<Function>,
    heading: String,
    anchor: String,
}

static TABLES: LazyLock<Tables> = LazyLock::new(|| {
    let data = data(include_str!("../data/postbuild/aasb16_leases.json"));
    Tables {
        functions: data["functions"]
            .as_array()
            .expect("functions")
            .iter()
            .map(|f| {
                let f = strings(f);
                Function {
                    name: f[0].clone(),
                    comment: f[1].clone(),
                    block: f[2].clone(),
                    about: f[3].clone(),
                }
            })
            .collect(),
        heading: data["heading"].as_str().expect("heading").into(),
        anchor: data["anchor"].as_str().expect("anchor").into(),
    }
});

struct Definition {
    name: String,
    comment: String,
    stored: String,
}

fn qualified() -> Vec<String> {
    TABLES
        .functions
        .iter()
        .map(|f| format!("oz.{}", f.name))
        .collect()
}

fn build_definitions(library: &BTreeSet<String>) -> Result<Vec<Definition>> {
    let mut out = Vec::new();
    for f in &TABLES.functions {
        let statement = formula::statements(&f.block)
            .into_iter()
            .next()
            .ok_or_else(|| format!("{}: source block does not declare it", f.name))?;
        let found = captures(formula::NAME, &statement);
        let Some(m) = found.first().filter(|m| m[1] == f.name) else {
            return Err(format!("{}: source block does not declare it", f.name));
        };
        let body = py::strip(&m[2]);
        let stored = formula::render(body, library)?;
        let want = formula::canonical(&formula::qualify(body, "oz", library))
            .chars()
            .collect::<Vec<_>>();
        let got = formula::canonical(&stored).chars().collect::<Vec<_>>();
        if want != got {
            let cut = want
                .iter()
                .zip(&got)
                .position(|(a, b)| a != b)
                .unwrap_or(want.len().min(got.len()));
            let window = |s: &[char]| {
                s[cut.saturating_sub(40)..(cut + 60).min(s.len())]
                    .iter()
                    .collect::<String>()
            };
            return Err(format!(
                "{}: generated definition does not reproduce its source at character {cut}\n  src   : {}\n  stored: {}",
                f.name,
                window(&want),
                window(&got)
            ));
        }
        out.push(Definition {
            name: format!("oz.{}", f.name),
            comment: f.comment.clone(),
            stored,
        });
    }
    Ok(out)
}

fn about_row(f: &Function) -> String {
    format!("{}→{}", pad(&f.name, 19), f.about)
}

fn src_state(text: &str) -> Result<&'static str> {
    let names = &TABLES.functions;
    let present = names
        .iter()
        .filter(|f| text.contains(&format!("\n{} = LAMBDA(", f.name)))
        .count();
    let about = count(text, &TABLES.heading);
    if present == 0 && about == 0 {
        return Ok("absent");
    }
    if present == names.len() && about == 1 {
        return Ok("applied");
    }
    Err(format!(
        "src/{MODULE}.txt holds {present} of {} lease functions and {about} About heading(s); it is neither state this pass recognises",
        names.len()
    ))
}

fn add_to_src(text: &str) -> Result<String> {
    let anchor = &TABLES.anchor;
    let hits = count(text, anchor);
    if hits != 1 {
        return Err(format!(
            "src/{MODULE}.txt: About anchor found {hits} times, expected 1"
        ));
    }
    let at = text.find(anchor.as_str()).expect("one anchor");
    let indent = &text[text[..at].rfind('\n').map_or(0, |i| i + 1)..at];
    let mut rows = vec![format!("{indent}\"{}¶\" &", TABLES.heading)];
    rows.extend(
        TABLES
            .functions
            .iter()
            .map(|f| format!("{indent}\"{}¶\" &", about_row(f))),
    );
    let text = text.replacen(
        &format!("{anchor} &"),
        &format!("{anchor} &\n{}", rows.join("\n")),
        1,
    );
    let blocks = TABLES
        .functions
        .iter()
        .map(|f| f.block.as_str())
        .collect::<Vec<_>>()
        .join("\n\n\n");
    let text = if text.ends_with('\n') {
        text
    } else {
        format!("{text}\n")
    };
    Ok(format!("{text}\n\n\n{blocks}\n"))
}

fn add_to_workbook(book: &str, definitions: &[Definition]) -> Result<String> {
    let stored_anchor = escape(&TABLES.anchor);
    let hits = count(book, &stored_anchor);
    if hits != 1 {
        return Err(format!(
            "xl/workbook.xml: About anchor found {hits} times, expected 1"
        ));
    }
    let mut addition = format!(" &amp; \"{}¶\"", escape(&TABLES.heading));
    for f in &TABLES.functions {
        addition.push_str(&format!(" &amp; \"{}¶\"", escape(&about_row(f))));
    }
    let mut book = book.replacen(&stored_anchor, &format!("{stored_anchor}{addition}"), 1);

    let mut existing = captures(r#"<definedName name="(oz\.[^"]+)""#, &book)
        .into_iter()
        .map(|m| m[1].clone())
        .collect::<Vec<_>>();
    let mut order = definitions.iter().collect::<Vec<_>>();
    order.sort_by_key(|d| d.name.to_lowercase());
    for d in order {
        let element = format!(
            "<definedName name=\"{}\" comment=\"{}\">{}</definedName>",
            escape(&d.name),
            escape(&d.comment),
            escape(&d.stored)
        );
        let lower = d.name.to_lowercase();
        let Some(previous) = existing.iter().rfind(|n| n.to_lowercase() < lower) else {
            return Err(format!(
                "{} sorts before every shipped name, which is not expected",
                d.name
            ));
        };
        let marker = format!("<definedName name=\"{}\"", escape(previous));
        let start = book.find(&marker).ok_or("substring not found")?;
        let end = book[start..]
            .find("</definedName>")
            .ok_or("substring not found")?
            + start
            + "</definedName>".len();
        book.insert_str(end, &element);
        existing.push(d.name.clone());
        existing.sort_by_key(|n| n.to_lowercase());
    }
    Ok(book)
}

fn update_index(path: &Path, definitions: &[Definition]) -> Result<bool> {
    let csv = Csv::read(path)?;
    let expected = [
        "function",
        "module",
        "previous_name",
        "signature",
        "description",
    ];
    let headers = csv
        .headers
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if headers != expected.into_iter().collect() {
        return Err(format!(
            "{} has unexpected columns: [{}]",
            display(path),
            csv.headers
                .iter()
                .map(|h| py::repr_str(h))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    // DictReader rows: the last column of a repeated name wins, short rows read empty.
    let rows = csv
        .rows
        .iter()
        .map(|row| {
            if row.len() > csv.headers.len() {
                return Err("dict contains fields not in fieldnames: None".to_string());
            }
            Ok(csv
                .headers
                .iter()
                .map(|h| (h.clone(), csv.get(row, h).unwrap_or_default().to_string()))
                .collect::<Vec<_>>())
        })
        .collect::<Result<Vec<_>>>();
    let names = definitions
        .iter()
        .map(|d| d.name.as_str())
        .collect::<BTreeSet<_>>();
    let have = csv
        .rows
        .iter()
        .map(|r| csv.get(r, "function").unwrap_or_default())
        .collect::<BTreeSet<_>>();
    if names.is_subset(&have) {
        return Ok(false);
    }
    if !names.is_disjoint(&have) {
        return Err(format!(
            "{} holds some of the lease functions but not all",
            display(path)
        ));
    }
    let mut rows = rows?;
    let signature = r"FUNCTION:(?>\s*)(?:→(?>\s*))?(.*?)¶";
    let mut sorted = definitions.iter().collect::<Vec<_>>();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    for d in sorted {
        let found = captures(signature, &d.stored);
        let Some(m) = found.first() else {
            return Err(format!(
                "{}: stored definition has no FUNCTION line",
                d.name
            ));
        };
        rows.push(vec![
            ("function".into(), d.name.clone()),
            ("module".into(), MODULE.into()),
            ("previous_name".into(), String::new()),
            ("signature".into(), py::strip(&m[1]).into()),
            ("description".into(), sub(r"\s{2,}", &d.comment, " ")),
        ]);
    }
    let field = |row: &Vec<(String, String)>, key: &str| {
        row.iter()
            .rev()
            .find(|(k, _)| k == key)
            .map_or(String::new(), |(_, v)| v.clone())
    };
    rows.sort_by_key(|row| field(row, "function"));
    let mut text = csv_row(&csv.headers);
    for row in &rows {
        let values = csv
            .headers
            .iter()
            .map(|h| field(row, h))
            .collect::<Vec<_>>();
        text.push_str(&csv_row(&values));
    }
    write_bytes(path, text.as_bytes())?;
    Ok(true)
}

pub fn run(
    workbook: &Path,
    src: &py::PyPath,
    index: &py::PyPath,
    notes: &mut Vec<String>,
) -> Result<Vec<String>> {
    let src_path = src.join(&format!("{MODULE}.txt"));
    if !src_path.fs().is_file() {
        return Err(format!("src: missing {src_path}"));
    }
    let src_text = read_text(&src_path.fs())?;
    let mut parts = read_parts(workbook)?;
    let book = read_book(&parts)?;
    let src_status = src_state(&src_text)?;
    let book_status = workbook_state(
        &book,
        &qualified(),
        Some(&TABLES.heading),
        "lease defined names",
    )?;
    if src_status != book_status {
        return Err(format!(
            "src/ is {src_status} but the workbook is {book_status}; the two views must not be applied separately"
        ));
    }

    let mut library = BTreeSet::new();
    for path in txt_files(&src.fs())? {
        for statement in formula::statements(&read_text(&path)?) {
            if let Some(m) = captures(formula::NAME, &statement).first() {
                library.insert(m[1].clone());
            }
        }
    }
    library.extend(TABLES.functions.iter().map(|f| f.name.clone()));
    let definitions = build_definitions(&library)?;

    let mut changed = Vec::new();
    if src_status == "absent" {
        write_text(&src_path.fs(), &add_to_src(&src_text)?)?;
        let written = add_to_workbook(&book, &definitions).and_then(|book| {
            parts.insert(BOOK.into(), book.into_bytes());
            write_deterministic(workbook, &parts)
        });
        if let Err(e) = written {
            write_text(&src_path.fs(), &src_text)?;
            return Err(e);
        }
        changed.push(format!("src/{MODULE}.txt"));
        changed.push("workbook".into());
    }
    if index.fs().is_file() {
        if update_index(&index.fs(), &definitions)? {
            changed.push(index.name().to_string());
        }
    } else {
        notes.push(format!("note: {index} not found, index not updated"));
    }
    Ok(changed)
}

pub fn command(args: &[String]) -> Output {
    if args.len() > 3 {
        return Output::stderr(
            "FAIL: usage: ozzit aasb16_leases [workbook] [src dir] [functions.csv]",
        );
    }
    let workbook = py::PyPath::new(args.first().map_or("ozzit.xlsx", String::as_str));
    let src = py::PyPath::new(args.get(1).map_or("src", String::as_str));
    let index = match args.get(2) {
        Some(path) => py::PyPath::new(path),
        None => workbook.parent().join("functions.csv"),
    };
    if !workbook.fs().is_file() {
        return Output::stderr(format!("FAIL: no such workbook: {workbook}"));
    }
    let mut notes = Vec::new();
    match run(&workbook.fs(), &src, &index, &mut notes) {
        Err(e) => Output {
            code: 1,
            out: notes,
            err: vec![format!("FAIL: {e}")],
        },
        Ok(changed) => {
            let mut out = notes;
            out.push(if changed.is_empty() {
                "AASB 16 lease functions already applied; no changes".into()
            } else {
                format!(
                    "added the AASB 16 lease functions to: {}",
                    changed.join(", ")
                )
            });
            out.push(format!("OK: {workbook}"));
            Output::ok(out)
        }
    }
}
