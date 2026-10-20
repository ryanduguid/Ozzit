//! Cover-label pass: Cover!A3 states the current cut and function count.
use super::*;

const STRINGS: &str = "xl/sharedStrings.xml";
/// Earlier labels, oldest first; any one of them is swapped for [`NEW`].
const OLD: [&str; 4] = [
    "Version 20 August 2026    -    130 functions    -    Microsoft 365 or Excel 2024 and later",
    "Version 16 September 2026    -    133 functions    -    Microsoft 365 or Excel 2024 and later",
    "Version 16 September 2026    -    137 functions    -    Microsoft 365 or Excel 2024 and later",
    "Version 19 October 2026    -    143 functions    -    Microsoft 365 or Excel 2024 and later",
];
const NEW: &str =
    "Version 19 October 2026    -    148 functions    -    Microsoft 365 or Excel 2024 and later";

pub fn run(workbook: &Path) -> Result<Vec<String>> {
    let mut parts = read_parts(workbook)?;
    if !parts.contains_key(STRINGS) {
        return Err(format!("missing part {STRINGS}"));
    }
    let text = part_text(&parts, STRINGS)?;
    let counts = OLD
        .iter()
        .chain([&NEW])
        .map(|label| (*label, count(&text, &format!("<t>{label}</t>"))))
        .collect::<Vec<_>>();
    if counts.iter().map(|(_, n)| n).sum::<usize>() != 1 {
        let found = counts
            .iter()
            .filter(|(_, n)| *n > 0)
            .map(|(label, n)| format!("{n} of {}", py::repr_str(label)))
            .collect::<Vec<_>>()
            .join(", ");
        let found = if found.is_empty() {
            "none".into()
        } else {
            found
        };
        return Err(format!(
            "{STRINGS}: expected exactly one cover label, got {found}"
        ));
    }
    if counts[OLD.len()].1 > 0 {
        return Ok(vec![]);
    }
    let old = counts.iter().find(|(_, n)| *n > 0).expect("one label").0;
    parts.insert(
        STRINGS.into(),
        text.replace(&format!("<t>{old}</t>"), &format!("<t>{NEW}</t>"))
            .into_bytes(),
    );
    write_deterministic(workbook, &parts)?;
    Ok(vec!["workbook".into()])
}

pub fn command(args: &[String]) -> Output {
    let args = match pass_args(args, "ozzit cover_label [workbook]", false) {
        Ok(args) => args,
        Err(output) => return output,
    };
    report(
        run(&args.workbook.fs()),
        &args.workbook,
        "cover label",
        "cover label",
    )
}
