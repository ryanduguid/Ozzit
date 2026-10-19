//! Native Excel's recalculated values compared with the values the workbook caches.
//!
//! Excel stays the formula engine: `tools/dump_values.ps1` drives it over COM and
//! writes every calculated value to a tab-separated dump. Nothing here evaluates a
//! formula or writes the workbook.
use crate::{archive::*, common::*, py};
use num_bigint::BigInt;
use num_traits::{Signed, ToPrimitive, Zero};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Below this the run proved nothing and the pass would be vacuous.
pub const FLOOR: usize = 15000;
/// Reading every value takes seconds; 10 minutes is a hang, not a slow run.
pub const DUMP_TIMEOUT_SECONDS: u64 = 600;
pub const DUMPER: &str = "tools/dump_values.ps1";

/// Tab position, row and column. Python's integers are unbounded, and so are these.
pub type Key = (BigInt, BigInt, BigInt);
/// Cached values by cell, and sheet names by tab position.
pub type Cached = (BTreeMap<Key, CellValue>, BTreeMap<usize, String>);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CellValue {
    pub text: String,
    pub is_error: bool,
}

/// How the Excel dumper finished.
#[derive(Clone, Debug)]
pub enum Dump {
    /// `powershell.exe` could not be found.
    NotFound,
    TimedOut,
    /// The exit status as Python reports it, with output decoded as text.
    Exited {
        code: i64,
        stdout: String,
        stderr: String,
    },
}

/// Why the cached values could not be read.
#[derive(Debug)]
pub enum CachedError {
    /// The workbook breaks the tab-position assumption; Python exits with this message.
    Refused(String),
    /// Python ends with a traceback here; this is its controlled replacement.
    Error(String),
}
impl From<String> for CachedError {
    fn from(message: String) -> Self {
        Self::Error(message)
    }
}

/// Excel hands an error cell back over COM as its error code, not its text.
fn error_text(code: &BigInt) -> Option<&'static str> {
    Some(match code.to_i64()? {
        -2146826288 => "#NULL!",
        -2146826281 => "#DIV/0!",
        -2146826273 => "#VALUE!",
        -2146826265 => "#REF!",
        -2146826259 => "#NAME?",
        -2146826252 => "#NUM!",
        -2146826246 => "#N/A",
        -2146826245 => "#GETTING_DATA",
        _ => return None,
    })
}

fn column(letters: &str) -> BigInt {
    letters.bytes().fold(BigInt::zero(), |n, c| {
        n * 26 + BigInt::from(i32::from(c) - 64)
    })
}

/// Every cached value in the workbook, keyed by (tab position, row, column).
///
/// Keyed by position rather than by part name because Excel renumbers the sheet
/// parts when it saves, so sheet4.xml is not the fourth sheet in a file it has written.
pub fn cached_values(path: &Path) -> std::result::Result<Cached, CachedError> {
    let mut zip = open_zip(path)?;
    let order = sheet_order(&mut zip)?;
    let mut shared = Vec::new();
    let has_shared = zip
        .file_names()
        .any(|name| name.is_ok_and(|n| n == "xl/sharedStrings.xml"));
    if has_shared {
        for si in captures(
            r"(?s)<si>(.*?)</si>",
            &member_text(&mut zip, "xl/sharedStrings.xml")?,
        ) {
            shared.push(unescape(&sub(r"<.*?>", &si[1], ""))?);
        }
    }

    // Excel's Worksheets collection skips chart sheets while workbook.xml lists them, so
    // a chart sheet would shift every position after it and compare 2 different sheets
    // against each other without saying so.
    let charts = order
        .iter()
        .filter(|(_, target)| !target.starts_with("xl/worksheets/"))
        .map(|(name, _)| name.as_str())
        .collect::<Vec<_>>();
    if !charts.is_empty() {
        return Err(CachedError::Refused(format!(
            "FAIL: {} are not worksheets, so tab position no longer lines up with Excel's Worksheets collection",
            charts.join(", ")
        )));
    }

    let mut out = BTreeMap::new();
    let mut names = BTreeMap::new();
    for (index, (name, target)) in order.iter().enumerate() {
        let pos = index + 1;
        names.insert(pos, name.clone());
        let text = member_text(&mut zip, target)?;
        sheet_values(&text, pos, &shared, &mut out)?;
    }
    Ok((out, names))
}

/// Each sheet's name and part, in tab order.
fn sheet_order(zip: &mut ZipReader) -> std::result::Result<Vec<(String, String)>, CachedError> {
    let book = member_text(zip, BOOK)?;
    let mut rels = std::collections::HashMap::new();
    for m in captures(
        r#"Id="([^"]+)"[^>]*Target="([^"]+)""#,
        &member_text(zip, "xl/_rels/workbook.xml.rels")?,
    ) {
        rels.insert(m[1].clone(), m[2].clone());
    }
    let mut order = Vec::new();
    for m in captures(r#"<sheet name="([^"]+)"[^>]*r:id="([^"]+)""#, &book) {
        let target = rels.get(&m[2]).ok_or_else(|| py::repr_str(&m[2]))?;
        order.push((m[1].clone(), resolve_target(BOOK, target)));
    }
    Ok(order)
}

/// The cached values of the sheet at tab position `pos`, added to `out`.
fn sheet_values(
    text: &str,
    pos: usize,
    shared: &[String],
    out: &mut BTreeMap<Key, CellValue>,
) -> Result<()> {
    for m in captures(
        r#"(?s)<c r="([A-Z]+)(\d+)"([^>]*)>((?:(?!<c[ /]).)*?)</c>"#,
        text,
    ) {
        let (attrs, inner) = (&m[3], &m[4]);
        let kind = captures(r#"t="([^"]+)""#, attrs)
            .first()
            .map_or_else(|| "n".to_string(), |k| k[1].clone());
        let value = if kind == "inlineStr" {
            captures(r"(?s)<t[^>]*>(.*?)</t>", inner)
                .first()
                .map(|t| unescape(&t[1]))
                .transpose()?
        } else {
            let Some(got) = captures(r"(?s)<v>(.*?)</v>", inner).into_iter().next() else {
                continue;
            };
            let raw = unescape(&got[1])?;
            Some(match kind.as_str() {
                "s" => shared_string(shared, &raw)?,
                "b" => if raw == "1" { "True" } else { "False" }.into(),
                _ => raw,
            })
        };
        if let Some(text) = value {
            out.insert(
                (BigInt::from(pos), py::parse_int(&m[2], 10)?, column(&m[1])),
                CellValue {
                    text,
                    is_error: kind == "e",
                },
            );
        }
    }
    Ok(())
}

/// `shared[int(raw)]`, including Python's negative indices.
fn shared_string(shared: &[String], raw: &str) -> Result<String> {
    let index = py::parse_int(raw, 10)?;
    let length = BigInt::from(shared.len());
    let position = if index.is_negative() {
        &length + &index
    } else {
        index
    };
    position
        .to_usize()
        .filter(|_| position < length)
        .and_then(|p| shared.get(p).cloned())
        .ok_or_else(|| "list index out of range".into())
}

/// One cell must be one line, the way the dumper writes it.
pub fn fold(text: &str) -> String {
    text.replace("\r\n", "\\n").replace(['\n', '\r'], "\\n")
}

pub fn agree(cached: &CellValue, live: &CellValue) -> bool {
    if cached.is_error != live.is_error {
        return false;
    }
    if cached.is_error {
        return py::parse_int(&live.text, 10)
            .ok()
            .and_then(|code| error_text(&code))
            == Some(cached.text.as_str());
    }
    if fold(&cached.text) == live.text {
        return true;
    }
    match (py::parse_float(&cached.text), py::parse_float(&live.text)) {
        (Ok(a), Ok(b)) if a.is_finite() && b.is_finite() => {
            (a - b).abs() <= f64::max(1e-9, 1e-9 * a.abs().max(b.abs()))
        }
        _ => false,
    }
}

/// Parse the dumper's TSV: position, row, column, value kind, folded value.
///
/// Python decodes the dump in chunks as it reads rows; this decodes it whole first,
/// so in a dump over one read chunk a decoding error can be reported ahead of an
/// earlier malformed row, with its byte position counted from the start of the file.
pub fn read_dump(path: &Path) -> Result<BTreeMap<Key, CellValue>> {
    let bytes = std::fs::read(path)
        .map_err(|e| py::oserror_str(&e, &path.display().to_string(), py::OsStyle::Errno))?;
    let body = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&bytes);
    let text = py::universal_newlines(&py::decode_utf8(body).map_err(|mut e| {
        // The incremental decoder holds back an incomplete final sequence and
        // reports it at the end of the file, counted from the bytes it held.
        if e.reason == "unexpected end of data" {
            (e.start, e.end) = (0, e.end - e.start);
        }
        e.to_string()
    })?);
    let mut live = BTreeMap::new();
    for (index, line) in text.split_inclusive('\n').enumerate() {
        let row = index + 1;
        let bits = line
            .trim_end_matches('\n')
            .splitn(5, '\t')
            .collect::<Vec<_>>();
        if bits.len() != 5 || !matches!(bits[3], "value" | "error") {
            return Err(format!(
                "row {row} needs a cell position, value kind and value"
            ));
        }
        let key = (
            py::parse_int(bits[0], 10)?,
            py::parse_int(bits[1], 10)?,
            py::parse_int(bits[2], 10)?,
        );
        let one = BigInt::from(1);
        if key.0 < one || key.1 < one || key.2 < one || live.contains_key(&key) {
            return Err(format!(
                "row {row} has an invalid or repeated cell position"
            ));
        }
        live.insert(
            key,
            CellValue {
                text: bits[4].into(),
                is_error: bits[3] == "error",
            },
        );
    }
    Ok(live)
}

fn location(names: &BTreeMap<usize, String>, (pos, row, col): &Key) -> String {
    let name = pos
        .to_usize()
        .and_then(|p| names.get(&p))
        .map_or("", String::as_str);
    format!("{name} row {row} col {col}")
}

/// The finding for `workbook` once the dumper has finished and written `dump_path`.
fn fail(lines: Vec<String>) -> Output {
    Output {
        code: 1,
        out: lines,
        err: vec![],
    }
}

/// The failure for a dumper that did not finish cleanly.
fn dump_failure(dump: Result<Dump>) -> Option<Output> {
    Some(match dump {
        Err(error) => Output::failure(format!("FAIL: {error}")),
        Ok(Dump::NotFound) => {
            Output::failure("FAIL: powershell.exe was not found, so Excel could not be driven")
        }
        Ok(Dump::TimedOut) => Output::failure(format!(
            "FAIL: Excel did not finish reading the workbook within {DUMP_TIMEOUT_SECONDS}s; check for a modal dialog and close any Excel process before retrying"
        )),
        Ok(Dump::Exited {
            code,
            stdout,
            stderr,
        }) if code != 0 => {
            let detail = if stdout.is_empty() { stderr } else { stdout };
            fail(vec![
                format!("FAIL: could not read the workbook in Excel (exit {code})"),
                py::prefix_chars(py::strip(&detail), 2000).into(),
            ])
        }
        Ok(Dump::Exited { .. }) => return None,
    })
}

fn missing_report(missing: &[&Key], names: &BTreeMap<usize, String>) -> Output {
    let mut out = vec![format!(
        "FAIL: {} cached value(s) are missing from the Excel dump",
        missing.len()
    )];
    out.extend(
        missing
            .iter()
            .take(20)
            .map(|k| format!("  - {}", location(names, k))),
    );
    if missing.len() > 20 {
        out.push(format!("  ... and {} more", missing.len() - 20));
    }
    fail(out)
}

fn stale_report(
    stale: &[&Key],
    both: usize,
    (cached, names): &Cached,
    live: &BTreeMap<Key, CellValue>,
) -> Output {
    let sheets = stale.iter().map(|k| &k.0).collect::<BTreeSet<_>>();
    let mut out = vec![format!(
        "FAIL: {} of {} cached values are not what the formula produces, on {} sheet(s)",
        stale.len(),
        both,
        sheets.len()
    )];
    let kind = |v: &CellValue| if v.is_error { "error" } else { "value" };
    for key in stale.iter().take(20) {
        let (was, now) = (&cached[*key], &live[*key]);
        out.push(format!(
            "  - {}: cached {} ({}), calculates to {} ({})",
            location(names, key),
            py::prefix_chars(&was.text, 40),
            kind(was),
            py::prefix_chars(&now.text, 40),
            kind(now)
        ));
    }
    if stale.len() > 20 {
        out.push(format!("  ... and {} more", stale.len() - 20));
    }
    out.push("Run: python tools/refresh_cache.py".into());
    fail(out)
}

pub fn outcome(workbook: &str, dump: Result<Dump>, dump_path: &Path) -> Output {
    if let Some(failure) = dump_failure(dump) {
        return failure;
    }
    let live = match read_dump(dump_path) {
        Ok(live) => live,
        Err(error) => return Output::failure(format!("FAIL: cannot read Excel dump: {error}")),
    };

    let values = match cached_values(Path::new(workbook)) {
        Ok(values) => values,
        Err(CachedError::Refused(message)) => return Output::stderr(message),
        Err(CachedError::Error(message)) => return Output::failure(format!("FAIL: {message}")),
    };
    let cached = &values.0;
    let both = cached
        .keys()
        .filter(|k| live.contains_key(*k))
        .collect::<Vec<_>>();
    let stale = both
        .iter()
        .copied()
        .filter(|k| !agree(&cached[*k], &live[*k]))
        .collect::<Vec<_>>();

    // A total or a per-sheet check can still pass when individual cells are missing.
    let missing = cached
        .keys()
        .filter(|k| !live.contains_key(*k))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return missing_report(&missing, &values.1);
    }

    if both.len() < FLOOR {
        return fail(vec![format!(
            "FAIL: only {} cells carried both a cached and a calculated value, under the floor of {FLOOR}. Something skipped most of the workbook rather than checking it.",
            both.len()
        )]);
    }

    if !stale.is_empty() {
        return stale_report(&stale, both.len(), &values, &live);
    }

    Output::ok(vec![format!(
        "OK: all {} cached values in {workbook} are the values their formulas produce",
        both.len()
    )])
}

/// Output captured in text mode: decoded, with `\r\n` and `\r` read as `\n`.
///
/// Python decodes strictly, so bytes that are not UTF-8 fail the run whatever
/// the exit status; its `UnicodeDecodeError` is the message.
pub fn captured_text(bytes: &[u8]) -> Result<String> {
    Ok(py::universal_newlines(&decode(bytes)?))
}

#[cfg(windows)]
fn exit_code(status: std::process::ExitStatus) -> i64 {
    // Python reports the process's DWORD exit code unsigned.
    status.code().map_or(-1, |c| i64::from(c.cast_unsigned()))
}
#[cfg(not(windows))]
fn exit_code(status: std::process::ExitStatus) -> i64 {
    use std::os::unix::process::ExitStatusExt;
    status
        .code()
        .map(i64::from)
        .or_else(|| status.signal().map(|s| -i64::from(s)))
        .unwrap_or(-1)
}

/// Run `program args`, capturing its output, and kill it once `timeout` passes.
pub fn run_dumper(program: &str, args: &[String], timeout: Duration) -> Result<Dump> {
    run_process(program, args, None, timeout)
}

/// [`run_dumper`] in a chosen working directory.
pub fn run_process(
    program: &str,
    args: &[String],
    cwd: Option<&Path>,
    timeout: Duration,
) -> Result<Dump> {
    let mut command = Command::new(program);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let mut child = match command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Dump::NotFound),
        Err(e) => return Err(e.to_string()),
    };
    let reader = |pipe: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            if let Some(mut pipe) = pipe {
                let _ = pipe.read_to_end(&mut bytes);
            }
            bytes
        })
    };
    let stdout = reader(child.stdout.take().map(|p| Box::new(p) as _));
    let stderr = reader(child.stderr.take().map(|p| Box::new(p) as _));
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(Dump::TimedOut);
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let text = |handle: std::thread::JoinHandle<Vec<u8>>| {
        captured_text(&handle.join().unwrap_or_default())
    };
    let (stdout, stderr) = (text(stdout), text(stderr));
    Ok(Dump::Exited {
        code: exit_code(status),
        stdout: stdout?,
        stderr: stderr?,
    })
}

/// The repository holding `script`: beside the executable or in one of its
/// ancestors, falling back to the working directory.
pub fn repository_root(script: &str) -> PathBuf {
    // The path only locates the checkout; the result must contain `script`
    // before it is used, and nothing is executed from it.
    // nosemgrep: rust.lang.security.current-exe.current-exe
    std::env::current_exe()
        .ok()
        .and_then(|exe| {
            exe.parent()
                .into_iter()
                .flat_map(Path::ancestors)
                .find(|d| d.join(script).is_file())
                .map(Path::to_owned)
        })
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default()
}

pub fn verify(args: &[String]) -> Output {
    let workbook = args.first().map_or("ozzit.xlsx", String::as_str);
    if !Path::new(workbook).exists() {
        return Output::failure(format!("FAIL: no such workbook: {workbook}"));
    }
    let dump_path = match tempfile::Builder::new()
        .prefix("ozzit-cache-")
        .suffix(".tsv")
        .tempfile()
    {
        Ok(file) => file.into_temp_path(),
        Err(e) => return Output::failure(format!("FAIL: {e}")),
    };
    let absolute = std::path::absolute(workbook).unwrap_or_else(|_| workbook.into());
    // Same boundary as refresh_cache.py: a modal dialog would otherwise hang the
    // gate forever, and a host with no powershell.exe must FAIL, not crash.
    let dump = run_dumper(
        "powershell.exe",
        &[
            "-NoProfile".into(),
            "-ExecutionPolicy".into(),
            "Bypass".into(),
            "-File".into(),
            repository_root(DUMPER).join(DUMPER).display().to_string(),
            "-Path".into(),
            absolute.display().to_string(),
            "-Out".into(),
            dump_path.display().to_string(),
        ],
        Duration::from_secs(DUMP_TIMEOUT_SECONDS),
    );
    outcome(workbook, dump, &dump_path)
}
