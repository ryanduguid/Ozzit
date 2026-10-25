//! Narrow aggregate-gate checks, preserving the reference helper's validation order.
use crate::{
    common::{Output, Result},
    py,
    pyjson::{self, Json},
};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

type Jobs = BTreeSet<String>;
const PREFIX: &str = ".github/workflows/";

fn job_key(text: &str) -> Option<&str> {
    let quote = if text.starts_with(['\'', '"']) {
        &text[..1]
    } else {
        ""
    };
    let rest = &text[quote.len()..];
    let length = rest
        .bytes()
        .take_while(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
        .count();
    let (name, rest) = rest.split_at(length);
    if !name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') {
        return None;
    }
    let rest = rest.strip_prefix(quote)?.trim_start_matches([' ', '\t']);
    let value = rest.strip_prefix(':')?;
    (value.is_empty() || value.starts_with([' ', '\t'])).then_some(name)
}

fn parse_jobs(text: &str) -> Option<(Jobs, Vec<String>)> {
    let (_, body) = text.split_once("\njobs:\n")?;
    let mut ids = Jobs::new();
    let mut unreadable = vec![];
    for line in py::splitlines(body) {
        if !line.is_empty() && !line.starts_with([' ', '#']) {
            break;
        }
        if let Some(rest) = line.strip_prefix("  ")
            && !rest.is_empty()
            && !rest.starts_with([' ', '#'])
        {
            match job_key(rest) {
                Some(name) => {
                    ids.insert(name.into());
                }
                None => unreadable.push(line.into()),
            }
        }
    }
    Some((ids, unreadable))
}

fn job_ids(text: &str) -> Result<Jobs> {
    let (ids, unreadable) =
        parse_jobs(text).ok_or("Cannot find the jobs block; the gate check needs updating")?;
    if let Some(line) = unreadable.first() {
        return Err(format!(
            "Cannot read the job key {}; the gate check needs updating",
            py::repr_str(line)
        ));
    }
    Ok(ids)
}

fn base_copy_with(
    workflow: &str,
    gate: &str,
    git: &mut impl FnMut(&[&str]) -> Result<String>,
) -> Result<Option<String>> {
    if !py::strip(&git(&["ls-tree", "--name-only", "HEAD^1", "--", workflow])?).is_empty() {
        return git(&["show", &format!("HEAD^1:{workflow}")]).map(Some);
    }
    let listing = git(&["ls-tree", "-r", "--name-only", "HEAD^1", "--", PREFIX])?;
    let mut candidates = vec![];
    for path in py::splitlines(&listing) {
        if path.matches('/').count() == 2 && (path.ends_with(".yml") || path.ends_with(".yaml")) {
            let text = git(&["show", &format!("HEAD^1:{path}")])?;
            if parse_jobs(&text).is_some_and(|(ids, _)| ids.contains(gate)) {
                candidates.push(text);
            }
        }
    }
    if candidates.len() > 1 {
        return Err(format!(
            "More than one base workflow has a job named {gate}; the gate check needs updating"
        ));
    }
    Ok(candidates.pop())
}

fn workflow_file_in(argument: &str, directory: &Path) -> Result<PathBuf> {
    // Select the path from directory entries, never by joining the untrusted operand.
    let mut listing = vec![];
    let io_error = |error| format!("OSError: {error}");
    for entry in fs::read_dir(directory).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        let path = entry.path();
        let file = match fs::metadata(&path) {
            Ok(metadata) => metadata.is_file(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => return Err(io_error(error)),
        };
        if file {
            listing.push((entry.file_name(), path));
        }
    }
    let name = argument.strip_prefix(PREFIX).unwrap_or("");
    listing
        .into_iter()
        .find(|(entry, _)| entry == name)
        .map(|(_, path)| path)
        .ok_or_else(|| format!("{argument} is not a file in .github/workflows"))
}

// Keep this runner distinct from the release runner: inherited cwd/environment,
// captured streams and no timeout. Both streams decode before checking status.
fn git_with(
    args: &[&str],
    run: impl FnOnce(&mut Command) -> Result<(i32, Vec<u8>, Vec<u8>)>,
) -> Result<String> {
    let mut command = Command::new("git");
    command.args(args).stdin(Stdio::inherit());
    let (code, stdout, stderr) = run(&mut command)?;
    let decode = |bytes: &[u8]| {
        py::decode_utf8(bytes)
            .map(|s| py::universal_newlines(&s))
            .map_err(|e| format!("UnicodeDecodeError: {e}"))
    };
    let stdout = decode(&stdout)?;
    let _stderr = decode(&stderr)?;
    if code != 0 {
        let arguments = std::iter::once("git")
            .chain(args.iter().copied())
            .map(py::repr_str)
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "CalledProcessError: Command '[{arguments}]' returned non-zero exit status {code}."
        ));
    }
    Ok(stdout)
}
fn git(args: &[&str]) -> Result<String> {
    git_with(args, |command| {
        let output = command.output().map_err(|e| format!("OSError: {e}"))?;
        Ok((
            output.status.code().unwrap_or(-1),
            output.stdout,
            output.stderr,
        ))
    })
}

fn result_names(results: &Json) -> Result<Jobs> {
    match results {
        Json::Object(pairs) => Ok(pairs.iter().map(|(name, _)| name.clone()).collect()),
        Json::Str(text) => Ok(text.chars().map(|c| c.to_string()).collect()),
        Json::Array(items) => {
            let mut names = Jobs::new();
            for value in items {
                match value {
                    Json::Str(text) => {
                        names.insert(text.clone());
                    }
                    Json::Array(_) | Json::Object(_) => {
                        return Err(format!(
                            "TypeError: unhashable type: '{}'",
                            value.type_name()
                        ));
                    }
                    _ => (), // Hashable non-string values never equal a job id.
                }
            }
            Ok(names)
        }
        _ => Err(format!(
            "TypeError: '{}' object is not iterable",
            results.type_name()
        )),
    }
}

fn arguments(argv: &[String]) -> Result<(&str, &str, Jobs)> {
    let mut args = argv
        .iter()
        .filter(|a| *a != "--no-results")
        .map(String::as_str)
        .collect::<Vec<_>>();
    let exempt = if let Some(index) = args.iter().position(|a| *a == "--exempt") {
        let exempt = args[index + 1..]
            .iter()
            .map(|s| (*s).into())
            .collect::<Jobs>();
        args.truncate(index);
        exempt
    } else {
        Jobs::new()
    };
    let [workflow, gate] = args.as_slice() else {
        return Err("ValueError: expected WORKFLOW GATE [--exempt JOB ...] [--no-results]".into());
    };
    Ok((workflow, gate, exempt))
}

fn check_results(results: &Json) -> Result<()> {
    let Json::Object(pairs) = results else {
        return Err(format!(
            "AttributeError: '{}' object has no attribute 'items'",
            results.type_name()
        ));
    };
    let mut failed = Jobs::new();
    for (name, value) in pairs {
        let status = match value {
            Json::Object(_) => value.get("result").ok_or("KeyError: 'result'")?,
            Json::Array(_) => {
                return Err("TypeError: list indices must be integers or slices, not str".into());
            }
            Json::Str(_) => {
                return Err("TypeError: string indices must be integers, not 'str'".into());
            }
            _ => {
                return Err(format!(
                    "TypeError: '{}' object is not subscriptable",
                    value.type_name()
                ));
            }
        };
        if status.as_str() != Some("success") {
            failed.insert(name.clone());
        }
    }
    if !failed.is_empty() {
        return Err(format!("Jobs did not succeed: {}", joined(&failed)));
    }
    Ok(())
}

fn declarations(text: &str, jobs: &Jobs) -> Result<Jobs> {
    let mut declared = Jobs::new();
    for line in py::splitlines(text) {
        if let Some(names) = py::strip(line).strip_prefix("# removed-jobs:") {
            declared.extend(names.split(',').map(|name| py::strip(name).to_string()));
        }
    }
    let live = declared.intersection(jobs).cloned().collect::<Jobs>();
    if !live.is_empty() {
        return Err(format!(
            "These removed-jobs declarations name jobs that still exist: {}",
            joined(&live)
        ));
    }
    Ok(declared)
}

fn check_removals(
    text: &str,
    workflow: &str,
    jobs: &Jobs,
    declared: &Jobs,
    out: &mut Vec<String>,
) -> Result<()> {
    let removed = job_ids(text)?.difference(jobs).cloned().collect::<Jobs>();
    let claimed = removed.intersection(declared).cloned().collect::<Jobs>();
    if !claimed.is_empty() {
        out.push(format!(
            "Declared removals from {workflow}: {}",
            joined(&claimed)
        ));
    }
    let undeclared = removed.difference(declared).cloned().collect::<Jobs>();
    if !undeclared.is_empty() {
        let names = joined(&undeclared);
        return Err(format!(
            "These jobs were removed from {workflow}: {names}. If that is intended, add the comment '# removed-jobs: {names}' to that file."
        ));
    }
    Ok(())
}

fn check_with(
    argv: &[String],
    mut environment: impl FnMut(&str) -> Result<Option<String>>,
    resolve: impl FnOnce(&str) -> Result<PathBuf>,
    read: impl FnOnce(&Path) -> Result<String>,
    base: impl FnOnce(&str, &str) -> Result<Option<String>>,
) -> Output {
    let mut out = vec![];
    let result = (|| -> Result<()> {
        let (workflow, gate, exempt) = arguments(argv)?;
        let raw = environment("RESULTS")?.ok_or("KeyError: 'RESULTS'")?;
        let results = pyjson::loads(&raw).map_err(|e| format!("JSONDecodeError: {e}"))?;
        if !argv.iter().any(|a| a == "--no-results") {
            check_results(&results)?;
        }
        let text = read(&resolve(workflow)?)?;
        let jobs = job_ids(&text)?;
        if !jobs.contains(gate) {
            return Err(format!(
                "Cannot find {gate} in {workflow}; the gate check needs updating"
            ));
        }
        let known = result_names(&results)?;
        let missing = jobs
            .iter()
            .filter(|name| {
                name.as_str() != gate && !exempt.contains(*name) && !known.contains(*name)
            })
            .cloned()
            .collect::<Jobs>();
        if !missing.is_empty() {
            return Err(format!("Add these jobs to needs: {}", joined(&missing)));
        }
        let declared = declarations(&text, &jobs)?;
        if environment("GITHUB_EVENT_NAME")?.as_deref() == Some("pull_request")
            && let Some(text) = base(workflow, gate)?
        {
            check_removals(&text, workflow, &jobs, &declared, &mut out)?;
        }
        Ok(())
    })();
    match result {
        Ok(()) => Output::ok(out),
        Err(error) => Output {
            code: 1,
            out,
            err: vec![error],
        },
    }
}
fn joined(jobs: &Jobs) -> String {
    jobs.iter().cloned().collect::<Vec<_>>().join(", ")
}

pub(crate) fn command(args: &[String]) -> Output {
    check_with(
        args,
        |name| match std::env::var(name) {
            Ok(value) => Ok(Some(value)),
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(_) => Err(format!("UnicodeError: {name} is not Unicode")),
        },
        |argument| workflow_file_in(argument, Path::new(PREFIX)),
        |path| {
            let bytes = fs::read(path).map_err(|e| format!("OSError: {e}"))?;
            py::decode_utf8(&bytes)
                .map(|s| py::universal_newlines(&s))
                .map_err(|e| format!("UnicodeDecodeError: {e}"))
        },
        |workflow, gate| base_copy_with(workflow, gate, &mut git),
    )
}

#[cfg(test)]
mod tests;
