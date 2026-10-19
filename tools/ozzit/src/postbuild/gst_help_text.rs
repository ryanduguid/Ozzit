//! GST help-text pass: legislative scope NOTES! in GSTAddλ and GSTExtractλ.
//!
//! Recognises the pre-note text and the first published (legacy) note, and
//! brings both to the current note.
use super::*;
use std::sync::LazyLock;

struct Swap {
    old: String,
    new: String,
    legacy: String,
    workbook: usize,
    src: usize,
}

static SWAPS: LazyLock<Vec<Swap>> = LazyLock::new(|| {
    let data = data(include_str!("../data/postbuild/gst_help_text.json"));
    let legacy = strings(&data["legacy"]);
    data["swaps"]
        .as_array()
        .expect("swaps")
        .iter()
        .zip(legacy)
        .map(|(s, legacy)| Swap {
            old: s[0].as_str().expect("old").into(),
            new: s[1].as_str().expect("new").into(),
            legacy,
            workbook: s[2].as_u64().expect("hits") as usize,
            src: s[3].as_u64().expect("hits") as usize,
        })
        .collect()
});

fn validate(text: &str, label: &str, failures: &mut Vec<String>) {
    for swap in SWAPS.iter() {
        let expected = if label == "workbook" {
            swap.workbook
        } else {
            swap.src
        };
        let counts = [
            ("pre-note", count(text, &swap.old)),
            ("legacy-note", count(text, &swap.legacy)),
            ("current-note", count(text, &swap.new)),
        ];
        let hits = counts.iter().map(|(_, n)| n).sum::<usize>();
        if hits != expected {
            let states = counts
                .iter()
                .map(|(state, n)| format!("{state}={n}"))
                .collect::<Vec<_>>()
                .join(", ");
            failures.push(format!(
                "{label}: {} expected {expected} recognised anchor, got {hits} ({states})",
                head_repr(&swap.old, 50)
            ));
        }
    }
}

fn swap_pairs() -> Vec<(String, String)> {
    SWAPS
        .iter()
        .flat_map(|s| {
            [
                (s.old.clone(), s.new.clone()),
                (s.legacy.clone(), s.new.clone()),
            ]
        })
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
    validate(&book, "workbook", &mut failures);
    let joined = originals
        .iter()
        .map(|(_, _, t)| t.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    validate(&joined, "src", &mut failures);
    if !failures.is_empty() {
        return Err(failures.join("; "));
    }
    let pairs = swap_pairs();
    let new_book = apply_swaps(&book, &pairs);
    let mut changed = Vec::new();
    if new_book != book {
        parts.insert(BOOK.into(), new_book.into_bytes());
        write_deterministic(workbook, &parts)?;
        changed.push("workbook".into());
    }
    for (module, path, original) in originals {
        let text = apply_swaps(&original, &pairs);
        if read_text(&path.fs())? != text {
            write_text(&path.fs(), &text)?;
            changed.push(format!("src/{module}.txt"));
        }
    }
    Ok(changed)
}

pub fn command(args: &[String]) -> Output {
    let args = match pass_args(args, "ozzit gst_help_text [workbook] [src dir]", true) {
        Ok(args) => args,
        Err(output) => return output,
    };
    report(
        run(&args.workbook.fs(), &args.src),
        &args.workbook,
        "GST help text",
        "GST help text",
    )
}
