//! Run independent Python references against the workflow's native gates.
use crate::{
    common::{self, Csv, Output, Result},
    py,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
};

const WORKFLOW: &str = ".github/workflows/verify.yml";
const INPUTS: [&str; 4] = ["ozzit.xlsx", "src", "functions.csv", "CHANGELOG.md"];
type Breaker = fn(&Path) -> Result<()>;
const BROKEN: [(&str, &str, Breaker); 3] = [
    (
        "functions.csv without a renamed function's row",
        "functions.csv",
        drop_function_row,
    ),
    (
        "src/Dates.txt with CountDOWλ's Starts renamed",
        "src",
        rename_parameter,
    ),
    (
        "ozzit.xlsx cut to half its length",
        "ozzit.xlsx",
        truncate_workbook,
    ),
];

#[derive(Debug, PartialEq, Eq)]
struct Capture {
    code: i64,
    out: String,
    err: String,
}

/// POSIX `shlex.split` with comments disabled, as in the reference helper.
fn split_command(text: &str) -> Result<Vec<String>> {
    let (mut words, mut word, mut quote, mut started) = (vec![], String::new(), None, false);
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\\' && quote != Some('\'') {
            let next = chars.next().ok_or("ValueError: No escaped character")?;
            if quote == Some('"') && !matches!(next, '\\' | '"') {
                word.push('\\');
            }
            word.push(next);
            started = true;
        } else if quote == Some(c) {
            quote = None;
        } else if quote.is_none() && matches!(c, '\'' | '"') {
            quote = Some(c);
            started = true;
        } else if quote.is_none() && matches!(c, ' ' | '\t' | '\r' | '\n') {
            if started {
                words.push(std::mem::take(&mut word));
                started = false;
            }
        } else {
            word.push(c);
            started = true;
        }
    }
    if quote.is_some() {
        return Err("ValueError: No closing quotation".into());
    }
    if started {
        words.push(word);
    }
    Ok(words)
}

fn discover(text: &str) -> Result<Vec<Vec<String>>> {
    let pattern = common::re(r"^\s*run:\s*(tools/ozzit/target/release/ozzit\s+verify_\w+.*?)\s*$");
    text.split_inclusive('\n')
        .filter_map(|line| {
            pattern
                .captures(line)
                .expect("constant regex")
                .map(|m| split_command(&m[1]))
        })
        .collect()
}

fn splitlines_keepends(text: &str) -> Vec<&str> {
    let mut offset = 0;
    py::splitlines(text)
        .into_iter()
        .map(|line| {
            let start = offset;
            offset += line.len();
            let tail = &text[offset..];
            offset += if tail.starts_with("\r\n") {
                2
            } else {
                tail.chars().next().map_or(0, char::len_utf8)
            };
            &text[start..offset]
        })
        .collect()
}

fn drop_function_row(root: &Path) -> Result<()> {
    let path = root.join("functions.csv");
    let text = common::read_raw(&path)?;
    let lines = splitlines_keepends(&text);
    let rows = Csv::reader_lines(&lines)?;
    if rows.len() != lines.len() {
        return Err("compare_gates: functions.csv has a multi-line row".into());
    }
    let mut selected = None;
    for (index, row) in rows.iter().enumerate().skip(1) {
        let previous = row.get(2).ok_or("IndexError: list index out of range")?;
        if !previous.is_empty() && !row[0].starts_with("oz.About") {
            selected = Some(index);
            break;
        }
    }
    let selected = selected.ok_or("StopIteration: no renamed function row")?;
    let bytes = lines
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != selected)
        .map(|(_, line)| *line)
        .collect::<String>();
    common::write_bytes(&path, bytes.as_bytes())
}

fn rename_parameter(root: &Path) -> Result<()> {
    let path = root.join("src/Dates.txt");
    let text = common::read_raw(&path)?;
    if !text.contains("CountDOWλ = LAMBDA(") || !text.contains("[Starts],") {
        return Err("compare_gates: src/Dates.txt no longer declares CountDOWλ's [Starts]".into());
    }
    common::write_bytes(&path, text.replacen("[Starts],", "[Begins],", 1).as_bytes())
}
fn truncate_workbook(root: &Path) -> Result<()> {
    let path = root.join("ozzit.xlsx");
    let bytes = common::read_bytes(&path)?;
    common::write_bytes(&path, &bytes[..bytes.len() / 2])
}

fn copy_metadata(source: &Path, destination: &Path) -> Result<()> {
    let apply = || -> std::io::Result<()> {
        let metadata = fs::metadata(source)?;
        #[cfg(windows)]
        let target = {
            use std::os::windows::fs::OpenOptionsExt;
            // FILE_WRITE_ATTRIBUTES and FILE_FLAG_BACKUP_SEMANTICS allow directory times.
            fs::OpenOptions::new()
                .access_mode(0x100)
                .custom_flags(0x02000000)
                .open(destination)?
        };
        #[cfg(unix)]
        let target = fs::File::open(destination)?;
        target.set_times(
            fs::FileTimes::new()
                .set_accessed(metadata.accessed()?)
                .set_modified(metadata.modified()?),
        )?;
        fs::set_permissions(destination, metadata.permissions())
    };
    apply().map_err(|e| common::io_error(&e, destination))
}
fn copy_input(source: &Path, destination: &Path, preserve: bool) -> Result<()> {
    if source.is_dir() {
        fs::create_dir(destination).map_err(|e| common::io_error(&e, destination))?;
        for entry in fs::read_dir(source).map_err(|e| common::io_error(&e, source))? {
            let entry = entry.map_err(|e| e.to_string())?;
            copy_input(&entry.path(), &destination.join(entry.file_name()), true)?;
        }
        copy_metadata(source, destination)
    } else {
        if preserve {
            fs::copy(source, destination)
                .map(|_| ())
                .map_err(|e| common::io_error(&e, source))?;
            copy_metadata(source, destination)
        } else {
            common::write_bytes(destination, &common::read_bytes(source)?)
        }
    }
}

fn native_path(root: &Path, name: &str) -> PathBuf {
    let path = root.join(name);
    #[cfg(windows)]
    {
        let extensions = std::env::var("PATHEXT")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD".into());
        for extension in extensions
            .split(';')
            .filter(|s| !s.is_empty())
            .map(|s| s.trim_end_matches('.'))
        {
            let candidate = PathBuf::from(format!("{}{extension}", path.display()));
            if candidate.is_file() {
                return candidate.canonicalize().unwrap_or(candidate);
            }
        }
    }
    path.canonicalize().unwrap_or(path)
}

fn exit_code(status: ExitStatus) -> i64 {
    #[cfg(windows)]
    {
        i64::from(status.code().unwrap_or(-1) as u32)
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        status
            .code()
            .map_or_else(|| -i64::from(status.signal().unwrap_or(1)), i64::from)
    }
}
fn run(argv: &[String], cwd: &Path) -> Result<Capture> {
    let process = Command::new(&argv[0])
        .args(&argv[1..])
        .current_dir(cwd)
        .env("PYTHONUTF8", "1")
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("OSError: {e}"))?;
    let out = py::universal_newlines(&common::decode(&process.stdout)?);
    let err = py::universal_newlines(&common::decode(&process.stderr)?);
    Ok(Capture {
        code: exit_code(process.status),
        out,
        err,
    })
}

struct Comparator<'a> {
    root: &'a Path,
    native: PathBuf,
    python: &'a str,
}
impl Comparator<'_> {
    fn compare_with(
        &self,
        label: &str,
        gate: &[String],
        cwd: &Path,
        output: &mut Output,
        run: &mut impl FnMut(&[String], &Path) -> Result<Capture>,
    ) -> Result<bool> {
        let mut argv = vec![self.native.to_string_lossy().into_owned()];
        argv.extend_from_slice(&gate[1..]);
        let rust = run(&argv, cwd)?;
        let mut argv = vec![
            self.python.to_owned(),
            self.root
                .join("tools")
                .join(format!("{}.py", gate[1]))
                .to_string_lossy()
                .into_owned(),
        ];
        argv.extend_from_slice(&gate[2..]);
        let python = run(&argv, cwd)?;
        if rust == python {
            output
                .out
                .push(format!("same: {label} (exit {})", rust.code));
            return Ok(true);
        }
        output.err.push(format!("DIFFERENT: {label}"));
        for (name, capture) in [("ozzit", rust), ("python", python)] {
            output.err.push(format!(
                "--- {name}: exit {}\n{}{}",
                capture.code, capture.out, capture.err
            ));
        }
        Ok(false)
    }
    fn all_with(
        &self,
        gates: &[Vec<String>],
        output: &mut Output,
        run: &mut impl FnMut(&[String], &Path) -> Result<Capture>,
    ) -> Result<bool> {
        let mut success = true;
        for gate in gates {
            success &= self.compare_with(&gate[1], gate, self.root, output, run)?;
        }
        for (case, target, breaker) in BROKEN {
            let directory = tempfile::tempdir().map_err(|e| e.to_string())?;
            let result = (|| {
                for name in INPUTS {
                    copy_input(&self.root.join(name), &directory.path().join(name), false)?;
                }
                breaker(directory.path())?;
                for gate in gates {
                    if gate[2..].iter().any(|arg| arg == target) {
                        success &= self.compare_with(
                            &format!("{} on {case}", gate[1]),
                            gate,
                            directory.path(),
                            output,
                            run,
                        )?;
                    }
                }
                Result::Ok(())
            })();
            directory.close().map_err(|e| e.to_string())?;
            result?;
        }
        Ok(success)
    }
}

fn check(root: &Path, python: &str, output: &mut Output) -> Result<bool> {
    let gates = discover(&common::read_text(&root.join(WORKFLOW))?)?;
    let Some(first) = gates.first() else {
        return Err(format!("no native verify_* gates found in {WORKFLOW}"));
    };
    Comparator {
        root,
        native: native_path(root, &first[0]),
        python,
    }
    .all_with(&gates, output, &mut run)
}
pub fn command(args: &[String]) -> Output {
    let [python] = args else {
        return Output {
            code: 2,
            out: vec![],
            err: vec!["usage: ozzit compare_gates PYTHON".into()],
        };
    };
    let mut output = Output::default();
    let result = std::env::current_dir()
        .map_err(|e| e.to_string())
        .and_then(|root| check(&root, python, &mut output));
    match result {
        Ok(success) => output.code = u8::from(!success),
        Err(error) => {
            output.code = 1;
            output.err.push(error);
        }
    }
    output
}

#[cfg(test)]
mod tests;
