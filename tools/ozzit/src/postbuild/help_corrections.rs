//! Correction pass: argument defaults, help text and worksheet examples.
//!
//! Each correction is spelt out in both stores' serialisations with an asserted
//! hit count. Every changed output is staged before any destination is replaced.
use super::*;
use std::sync::LazyLock;

struct Swap {
    what: String,
    workbook: (String, String),
    src: (String, String),
    hits: usize,
}

struct Tables {
    swaps: Vec<Swap>,
    cells: Vec<(String, String, String)>,
    strings: Vec<(String, String, String)>,
}

fn pair(value: &Value) -> (String, String) {
    let items = strings(value);
    (items[0].clone(), items[1].clone())
}

fn triples(value: &Value) -> Vec<(String, String, String)> {
    value
        .as_array()
        .expect("triples")
        .iter()
        .map(|t| {
            let items = strings(t);
            (items[0].clone(), items[1].clone(), items[2].clone())
        })
        .collect()
}

static TABLES: LazyLock<Tables> = LazyLock::new(|| {
    let data = data(include_str!("../data/postbuild/help_corrections.json"));
    Tables {
        swaps: data["swaps"]
            .as_array()
            .expect("swaps")
            .iter()
            .map(|s| Swap {
                what: s[0].as_str().expect("what").into(),
                workbook: pair(&s[1]),
                src: pair(&s[2]),
                hits: s[3].as_u64().expect("hits") as usize,
            })
            .collect(),
        cells: triples(&data["cell_swaps"]),
        strings: triples(&data["string_swaps"]),
    }
});

fn states(before: usize, after: usize) -> String {
    format!("before={before}, after={after}")
}

fn validate_store(text: &str, store: &str, failures: &mut Vec<String>) {
    for swap in &TABLES.swaps {
        let (old, new) = if store == "workbook" {
            &swap.workbook
        } else {
            &swap.src
        };
        let (before, after) = (count(text, old), count(text, new));
        if before + after != swap.hits {
            failures.push(format!(
                "{store}: {} expected {} recognised anchor(s), got {} ({})",
                swap.what,
                swap.hits,
                before + after,
                states(before, after)
            ));
        }
    }
}

fn swap_pairs(store: &str) -> Vec<(String, String)> {
    TABLES
        .swaps
        .iter()
        .map(|s| {
            if store == "workbook" {
                s.workbook.clone()
            } else {
                s.src.clone()
            }
        })
        .collect()
}

fn pairs_of(table: &[(String, String, String)]) -> Vec<(String, String)> {
    table
        .iter()
        .map(|(_, o, n)| (o.clone(), n.clone()))
        .collect()
}

pub fn run(workbook: &Path, src: &py::PyPath) -> Result<Vec<String>> {
    let mut failures = Vec::new();
    let mut originals = Vec::new();
    for module in MODULES {
        let path = src.join(&format!("{module}.txt"));
        if !path.fs().is_file() {
            failures.push(format!("src: missing {path}"));
            continue;
        }
        let text = read_text(&path.fs())?;
        originals.push((module, path, text));
    }
    let mut parts = read_parts(workbook)?;
    let book = read_book(&parts)?;
    let mut sheets = Vec::new();
    for name in parts.keys().filter(|n| n.starts_with("xl/worksheets/")) {
        sheets.push((name.clone(), part_text(&parts, name)?));
    }
    if !parts.contains_key("xl/sharedStrings.xml") {
        return Err("workbook: xl/sharedStrings.xml is missing".into());
    }
    let strings = part_text(&parts, "xl/sharedStrings.xml")?;

    validate_store(&book, "workbook", &mut failures);
    let library = originals
        .iter()
        .map(|(_, _, t)| t.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    validate_store(&library, "src", &mut failures);
    for (what, old, new) in &TABLES.cells {
        let before = sheets.iter().map(|(_, t)| count(t, old)).sum::<usize>();
        let after = sheets.iter().map(|(_, t)| count(t, new)).sum::<usize>();
        if before + after != 1 {
            failures.push(format!(
                "worksheets: {what} expected 1 recognised anchor, got {} ({})",
                before + after,
                states(before, after)
            ));
        }
    }
    for (what, old, new) in &TABLES.strings {
        let (before, after) = (count(&strings, old), count(&strings, new));
        if before + after != 1 {
            failures.push(format!(
                "sharedStrings: {what} expected 1 recognised anchor, got {} ({})",
                before + after,
                states(before, after)
            ));
        }
    }
    if !failures.is_empty() {
        return Err(failures.join("; "));
    }

    let updated_book = apply_swaps(&book, &swap_pairs("workbook")).into_bytes();
    let mut rewritten = updated_book != part(&parts, BOOK)?;
    parts.insert(BOOK.into(), updated_book);
    let cell_pairs = pairs_of(&TABLES.cells);
    for (name, text) in &sheets {
        let updated = apply_swaps(text, &cell_pairs).into_bytes();
        if updated != part(&parts, name)? {
            parts.insert(name.clone(), updated);
            rewritten = true;
        }
    }
    let updated = apply_swaps(&strings, &pairs_of(&TABLES.strings)).into_bytes();
    if updated != part(&parts, "xl/sharedStrings.xml")? {
        parts.insert("xl/sharedStrings.xml".into(), updated);
        rewritten = true;
    }

    // Stage every changed output before any destination is replaced.
    let mut changed = Vec::new();
    let mut staged = Vec::new();
    if rewritten {
        staged.push((
            stage_bytes(workbook, &deterministic_bytes(&parts)?)?,
            workbook.to_path_buf(),
        ));
        changed.push("workbook".to_string());
    }
    let src_pairs = swap_pairs("src");
    for (module, path, original) in &originals {
        let text = apply_swaps(original, &src_pairs);
        if text == *original {
            continue;
        }
        staged.push((stage_bytes(&path.fs(), text.as_bytes())?, path.fs()));
        changed.push(format!("src/{module}.txt"));
    }
    for (temporary, destination) in staged {
        temporary
            .persist(&destination)
            .map_err(|e| io_error(&e.error, &destination))?;
    }
    Ok(changed)
}

pub fn command(args: &[String]) -> Output {
    let args = match pass_args(args, "ozzit help_corrections [workbook] [src dir]", true) {
        Ok(args) => args,
        Err(output) => return output,
    };
    report(
        run(&args.workbook.fs(), &args.src),
        &args.workbook,
        "help corrections",
        "help corrections",
    )
}
