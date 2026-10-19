//! Sheet-name pass: the oz.SumContains tab gains the λ every sibling carries.
use super::*;

const OLD: &str = "oz.SumContains";
const NEW: &str = "oz.SumContainsλ";

/// A part and its (old anchor, new anchor, expected count) edits.
type Planned = Vec<(String, Vec<(String, String, usize)>)>;

/// Part to (old anchor, new anchor, expected count). The worksheet parts are
/// found by sheet name, since Excel renumbers them when a sheet is lost.
fn edits(parts: &Parts) -> Result<Planned> {
    let by_name = sheet_targets(parts)?;
    let demo = lookup(&by_name, NEW).or_else(|| lookup(&by_name, OLD));
    let (Some(demo), Some(toc)) = (demo, lookup(&by_name, "TOC")) else {
        return Err("the workbook lacks the TOC sheet or the oz.SumContains sheet".into());
    };
    let edit = |o: String, n: String, c| vec![(o, n, c)];
    let mut planned: Planned = Vec::new();
    for (part, anchors) in [
        (
            BOOK.to_string(),
            edit(
                format!(r#"<sheet name="{OLD}""#),
                format!(r#"<sheet name="{NEW}""#),
                1,
            ),
        ),
        (
            toc.clone(),
            edit(format!("'{OLD}'!"), format!("'{NEW}'!"), 4),
        ),
        (
            demo.clone(),
            edit(format!("<v>{OLD}</v>"), format!("<v>{NEW}</v>"), 1),
        ),
        (
            "docProps/app.xml".into(),
            edit(
                format!("<vt:lpstr>{OLD}</vt:lpstr>"),
                format!("<vt:lpstr>{NEW}</vt:lpstr>"),
                1,
            ),
        ),
        (
            "xl/sharedStrings.xml".into(),
            edit(format!("<t>{OLD}</t>"), format!("<t>{NEW}</t>"), 1),
        ),
    ] {
        // A dictionary keeps one entry per part: a later key replaces the earlier value.
        match planned.iter_mut().find(|(p, _)| *p == part) {
            Some(entry) => entry.1 = anchors,
            None => planned.push((part, anchors)),
        }
    }
    Ok(planned)
}

pub fn run(workbook: &Path) -> Result<Vec<String>> {
    let mut parts = read_parts(workbook)?;
    let planned = edits(&parts)?;
    let mut failures = Vec::new();
    let mut texts = Vec::new();
    for (name, anchors) in &planned {
        if !parts.contains_key(name) {
            failures.push(format!("missing part {name}"));
            continue;
        }
        let text = part_text(&parts, name)?;
        for (old, new, expected) in anchors {
            let (before, after) = (count(&text, old), count(&text, new));
            if before + after != *expected {
                failures.push(format!(
                    "{name}: {} expected {expected} recognised anchors, got pre-rename={before} renamed={after}",
                    py::repr_str(old)
                ));
            }
        }
        texts.push((name.clone(), text));
    }
    if !failures.is_empty() {
        return Err(failures.join("; "));
    }
    let mut changed = false;
    for (name, anchors) in &planned {
        let original = &texts.iter().find(|(n, _)| n == name).expect("read").1;
        let text = anchors
            .iter()
            .fold(original.clone(), |t, (old, new, _)| t.replace(old, new));
        if text != *original {
            parts.insert(name.clone(), text.into_bytes());
            changed = true;
        }
    }
    if changed {
        write_deterministic(workbook, &parts)?;
        return Ok(vec!["workbook".into()]);
    }
    Ok(vec![])
}

pub fn command(args: &[String]) -> Output {
    let args = match pass_args(args, "ozzit sheet_names [workbook]", false) {
        Ok(args) => args,
        Err(output) => return output,
    };
    report(
        run(&args.workbook.fs()),
        &args.workbook,
        "sheet names",
        "sheet names",
    )
}
