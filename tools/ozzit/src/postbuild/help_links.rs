//! Help-link pass: repoint 2 WEBPAGE rows copied from a neighbouring function.
use super::*;

#[cfg(test)]
mod tests;

/// (context prefix, old URL, new URL). The prefix is unique to the owning function.
const SWAPS: [(&str, &str, &str); 2] = [
    (
        "(Current Assets/Liabilities)¶\" {AMP} \"WEBPAGE:       →",
        "https://www.investopedia.com/terms/c/cash-ratio.asp",
        "https://www.investopedia.com/terms/c/currentratio.asp",
    ),
    (
        "(Net Return on Investment/Cost of Investment)¶\" {AMP} \"WEBPAGE:               →",
        "https://www.investopedia.com/terms/r/returnonequity.asp",
        "https://www.investopedia.com/terms/r/returnoninvestment.asp",
    ),
];

/// (old, new) anchor pairs in this store's concatenation form.
fn forms(store: &str) -> Vec<(String, String)> {
    let amp = if store == "workbook" { "&amp;" } else { "&" };
    SWAPS
        .iter()
        .map(|(prefix, old, new)| {
            let mut p = prefix.replace("{AMP}", amp);
            if store == "src" {
                // src wraps the concatenation across lines after the ampersand.
                p = p.replace("¶\" & \"", "¶\" & \n                        \"");
            }
            (format!("{p}{old}"), format!("{p}{new}"))
        })
        .collect()
}

fn validate(text: &str, store: &str, failures: &mut Vec<String>) {
    for (old, new) in forms(store) {
        let (wrong, fixed) = (count(text, &old), count(text, &new));
        if wrong + fixed != 1 {
            failures.push(format!(
                "{store}: {} expected exactly 1 recognised anchor, got wrong-link={wrong} fixed-link={fixed}",
                tail_repr(&old, 60)
            ));
        }
    }
}

pub fn run(workbook: &Path, src: &py::PyPath) -> Result<Vec<String>> {
    run_with(workbook, src, &mut write_text)
}

// Exercise failures in the production transaction with local callbacks.
fn run_with(
    workbook: &Path,
    src: &py::PyPath,
    source_write: &mut impl FnMut(&Path, &str) -> Result<()>,
) -> Result<Vec<String>> {
    let ratios_path = src.join("Ratios.txt");
    if !ratios_path.fs().is_file() {
        return Err(format!("src: missing {ratios_path}"));
    }
    let ratios = read_text(&ratios_path.fs())?;
    let mut parts = read_parts(workbook)?;
    let book = read_book(&parts)?;
    let mut failures = Vec::new();
    validate(&book, "workbook", &mut failures);
    validate(&ratios, "src", &mut failures);
    if !failures.is_empty() {
        return Err(failures.join("; "));
    }
    let new_book = apply_swaps(&book, &forms("workbook"));
    let new_ratios = apply_swaps(&ratios, &forms("src"));
    let original = read_bytes(workbook)?;
    let mut changed = Vec::new();
    let mut write = |changed: &mut Vec<String>, parts: &mut Parts| -> Result<()> {
        if new_book != book {
            parts.insert(BOOK.into(), new_book.clone().into_bytes());
            write_deterministic(workbook, parts)?;
            changed.push("workbook".into());
        }
        if new_ratios != ratios {
            source_write(&ratios_path.fs(), &new_ratios)?;
            changed.push("src/Ratios.txt".into());
        }
        Ok(())
    };
    if let Err(e) = write(&mut changed, &mut parts) {
        // Restore the published workbook first, then the source.
        let restored = replace_atomically(workbook, &original);
        source_write(&ratios_path.fs(), &ratios)?;
        restored?;
        return Err(e);
    }
    Ok(changed)
}

pub fn command(args: &[String]) -> Output {
    let args = match pass_args(args, "ozzit help_links [workbook] [src dir]", true) {
        Ok(args) => args,
        Err(output) => return output,
    };
    report(
        run(&args.workbook.fs(), &args.src),
        &args.workbook,
        "help links",
        "help links",
    )
}
