//! Revision-history pass: remove per-function REVISIONS blocks and the creator credit.
//!
//! Three stores hold the revision text and are rewritten together: `src/*.txt`,
//! the Advanced Formula Environment store (resynchronised from `src/`) and
//! `docProps/core.xml`. Every module is validated before any is written, and a
//! failure restores what was written.
use super::*;

const OLD_CREATOR: &[u8] = "<dc:creator>Ryan Duguid; Ozzit project</dc:creator>".as_bytes();
const NEW_CREATOR: &[u8] = b"<dc:creator>Ozzit project</dc:creator>";
const EXPECTED_BLOCKS: [(&str, usize); 6] = [
    ("Dates", 13),
    ("Essentials", 17),
    ("Financial", 39),
    ("Ratios", 39),
    ("Utilities", 17),
    ("Debt", 0),
];

/// Drop every REVISIONS block, keeping any other content in the same comment.
pub fn strip_module(text: &str) -> (String, usize) {
    let heading = re(r"(?m)^[ \t]*REVISIONS:.*");
    let mut out = String::new();
    let (mut index, mut blocks) = (0, 0);
    while index < text.len() {
        let Some(start) = text[index..].find("/*").map(|i| i + index) else {
            out.push_str(&text[index..]);
            break;
        };
        let Some(mut end) = text[start + 2..].find("*/").map(|i| i + start + 2) else {
            out.push_str(&text[index..]);
            break;
        };
        out.push_str(&text[index..start]);
        let body = &text[start + 2..end];
        match heading.find(body).ok().flatten() {
            None => {
                out.push_str("/*");
                out.push_str(body);
                out.push_str("*/");
            }
            Some(found) => {
                blocks += 1;
                let kept = &body[..found.start()];
                if !py::strip(kept).is_empty() {
                    let newline = if body.contains("\r\n") { "\r\n" } else { "\n" };
                    out.push_str("/*");
                    out.push_str(kept.trim_end_matches(py::is_space));
                    out.push_str(newline);
                    out.push_str("*/");
                } else if text[end + 2..].starts_with("\r\n") {
                    end += 2;
                } else if text[end + 2..].starts_with('\n') {
                    end += 1;
                }
            }
        }
        index = end + 2;
    }
    (out, blocks)
}

fn count_bytes(haystack: &[u8], needle: &[u8]) -> usize {
    let mut n = 0;
    let mut i = 0;
    while let Some(at) = haystack[i..]
        .windows(needle.len())
        .position(|w| w == needle)
    {
        n += 1;
        i += at + needle.len();
    }
    n
}

fn replace_bytes(haystack: &[u8], needle: &[u8], with: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(at) = haystack[i..]
        .windows(needle.len())
        .position(|w| w == needle)
    {
        out.extend(&haystack[i..i + at]);
        out.extend(with);
        i += at + needle.len();
    }
    out.extend(&haystack[i..]);
    out
}

type Stripped = (&'static str, py::PyPath, String, String, usize);

/// Each module that still carries REVISIONS blocks, before and after stripping.
fn stripped_modules(src: &py::PyPath) -> Result<Vec<Stripped>> {
    let mut stripped = Vec::new();
    for (module, expected) in EXPECTED_BLOCKS {
        let path = src.join(&format!("{module}.txt"));
        let before = read_text(&path.fs())?;
        let (after, blocks) = strip_module(&before);
        if blocks == 0 {
            continue;
        }
        if blocks != expected {
            return Err(format!(
                "{}: expected {expected} REVISIONS blocks, found {blocks}",
                path.name()
            ));
        }
        stripped.push((module, path, before, after, blocks));
    }
    Ok(stripped)
}

fn restore_sources(stripped: &[Stripped]) -> Result<()> {
    for (_, path, before, _, _) in stripped {
        write_text(&path.fs(), before)?;
    }
    Ok(())
}

pub fn apply(workbook: &Path, src: &py::PyPath) -> Result<Vec<String>> {
    let stripped = stripped_modules(src)?;

    let original = read_bytes(workbook)?;
    let mut parts = read_parts(workbook)?;
    let core = part(&parts, "docProps/core.xml")?.to_vec();
    let mut changes = Vec::new();
    for (module, path, _, after, blocks) in &stripped {
        if let Err(e) = write_text(&path.fs(), after) {
            restore_sources(&stripped)?;
            return Err(e);
        }
        changes.push(format!(
            "stripped {blocks} REVISIONS blocks from {module}.txt"
        ));
    }

    let mut touched = false;
    let mut workbook_phase = |changes: &mut Vec<String>| -> Result<()> {
        if count_bytes(&core, OLD_CREATOR) == 1 {
            parts.insert(
                "docProps/core.xml".into(),
                replace_bytes(&core, OLD_CREATOR, NEW_CREATOR),
            );
            write_deterministic(workbook, &parts)?;
            touched = true;
            changes.push("rewrote docProps/core.xml creator".into());
        } else if count_bytes(&core, NEW_CREATOR) == 0 {
            return Err("docProps/core.xml carries neither the old nor the new creator".into());
        }
        touched = true;
        changes.extend(
            crate::afe::sync(workbook, &src.fs())?
                .into_iter()
                .filter(|change| !change.contains("already")),
        );
        Ok(())
    };
    if let Err(e) = workbook_phase(&mut changes) {
        // Restore the workbook first, then the sources.
        let restored = if touched {
            replace_atomically(workbook, &original)
        } else {
            Ok(())
        };
        restore_sources(&stripped)?;
        restored?;
        return Err(e);
    }
    Ok(changes)
}

pub fn command(args: &[String]) -> Output {
    let args = match pass_args(
        args,
        "ozzit strip_revision_history [workbook] [src dir]",
        true,
    ) {
        Ok(args) => args,
        Err(output) => return output,
    };
    let workbook = args.workbook.fs();
    match apply(&workbook, &args.src) {
        Err(e) => Output::stderr(format!("FAIL: cannot strip revision history: {e}")),
        Ok(changes) if changes.is_empty() => Output::ok(vec![
            "already applied: no REVISIONS block, creator credit or AFE drift left".into(),
        ]),
        Ok(mut changes) => {
            let size = std::fs::metadata(&workbook).map_or(0, |m| m.len());
            changes.push(format!(
                "OK: {} stripped, {} bytes",
                args.workbook,
                crate::sanitise::thousands(size)
            ));
            Output::ok(changes)
        }
    }
}
