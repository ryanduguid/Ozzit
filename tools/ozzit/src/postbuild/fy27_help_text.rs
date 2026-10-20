//! FY27 help-text pass: dated examples in defined names and `src/`.
//!
//! Every swap is length-preserving and carries an asserted hit count per store.
use super::*;
use std::sync::LazyLock;

/// (old, new, workbook hits, src hits).
static SWAPS: LazyLock<Vec<(String, String, usize, usize)>> = LazyLock::new(|| {
    let data = data(include_str!("../data/postbuild/fy27_help_text.json"));
    data["swaps"]
        .as_array()
        .expect("swaps")
        .iter()
        .map(|s| {
            (
                s[0].as_str().expect("old").into(),
                s[1].as_str().expect("new").into(),
                s[2].as_u64().expect("hits") as usize,
                s[3].as_u64().expect("hits") as usize,
            )
        })
        .collect()
});

/// The swaps this store earns, counting each against the text as it stands.
fn check_swaps(text: &str, label: &str, failures: &mut Vec<String>) -> Vec<(String, String)> {
    let mut text = text.to_string();
    let mut pairs = Vec::new();
    for (old, new, workbook, src) in SWAPS.iter() {
        assert_eq!(
            py::char_len(old),
            py::char_len(new),
            "length drift: {}",
            head_repr(old, 40)
        );
        let hits = count(&text, old);
        let expected = if label == "workbook" { *workbook } else { *src };
        if hits == 0 {
            continue;
        }
        if hits != expected {
            failures.push(format!(
                "{label}: {} expected {expected} hits, got {hits}",
                head_repr(old, 50)
            ));
            continue;
        }
        pairs.push((old.clone(), new.clone()));
        text = text.replace(old, new);
    }
    pairs
}

pub fn run(workbook: &Path, src: &py::PyPath) -> Result<Vec<String>> {
    let mut failures = Vec::new();
    let mut src_texts = Vec::new();
    let mut originals = Vec::new();
    for module in MODULES {
        let path = src.join(&format!("{module}.txt"));
        if !path.fs().is_file() {
            failures.push(format!("src: missing {path}"));
            continue;
        }
        let original = read_text(&path.fs())?;
        let pairs = check_swaps(&original, "src", &mut failures);
        src_texts.push((module, path, apply_swaps(&original, &pairs)));
        originals.push(original);
    }
    let mut parts = read_parts(workbook)?;
    let book = read_book(&parts)?;
    let pairs = check_swaps(&book, "workbook", &mut failures);
    let new_book = apply_swaps(&book, &pairs);

    // Every swap must be visible as its old anchor or its replacement somewhere.
    let aggregate = format!("{book}\n{}", originals.join("\n"));
    for (old, new, _, _) in SWAPS.iter() {
        if !aggregate.contains(old.as_str()) && !aggregate.contains(new.as_str()) {
            failures.push(format!(
                "library: {} absent and its replacement is absent too",
                head_repr(old, 50)
            ));
        }
    }
    if !failures.is_empty() {
        return Err(failures.join("; "));
    }

    let mut changed = Vec::new();
    if new_book != book {
        parts.insert(BOOK.into(), new_book.into_bytes());
        write_deterministic(workbook, &parts)?;
        changed.push("workbook".into());
    }
    for (module, path, text) in src_texts {
        if read_text(&path.fs())? != text {
            write_text(&path.fs(), &text)?;
            changed.push(format!("src/{module}.txt"));
        }
    }
    Ok(changed)
}

pub fn command(args: &[String]) -> Output {
    let args = match pass_args(args, "ozzit fy27_help_text [workbook] [src dir]", true) {
        Ok(args) => args,
        Err(output) => return output,
    };
    report(
        run(&args.workbook.fs(), &args.src),
        &args.workbook,
        "FY27 help text",
        "FY27 help text",
    )
}
