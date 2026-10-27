//! Narrow aggregate-gate checks, preserving the reference helper's validation order.
//!
//! On a pull request the base obligations come from the destination's base copy and
//! from the one base workflow whose job can report the gate's context, or, failing
//! both, the one other base workflow that held the gate id and no longer runs it.
use crate::{
    common::{self, Output, Result},
    py,
    pyjson::{self, Json},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

type Jobs = BTreeSet<String>;
/// Possible reported names of one job; None when the reader cannot tell.
type Names = Option<BTreeSet<String>>;
type Parsed = (BTreeMap<String, Names>, Vec<String>);
const PREFIX: &str = ".github/workflows/";
const EXPRESSION_STRING: &str = r"'((?:[^']|'')*)'";

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

/// `re.fullmatch` returning the groups of the match.
fn full_captures(pattern: &str, text: &str) -> Option<Vec<String>> {
    common::captures(&format!("^(?:{pattern})$"), text)
        .into_iter()
        .next()
}

/// The text from character `n` on, as Python's `text[n:]`.
fn from_char(text: &str, n: usize) -> &str {
    text.char_indices().nth(n).map_or("", |(i, _)| &text[i..])
}

fn lstrip(text: &str) -> &str {
    text.trim_start_matches(py::is_space)
}

/// A blank line or a comment, which never ends a block.
fn is_filler(line: &str) -> bool {
    py::strip(line).is_empty() || lstrip(line).starts_with('#')
}

/// Drop a trailing comment (a # after whitespace) in one linear pass, then strip.
fn without_comment(text: &str) -> &str {
    let mut previous = None;
    for (index, c) in text.char_indices() {
        if c == '#' && previous.is_some_and(py::is_space) {
            return py::strip(&text[..index]);
        }
        previous = Some(c);
    }
    py::strip(text)
}

/// Read a single-line literal name; unsupported YAML stays unknown.
fn literal_name(text: &str) -> Option<String> {
    if text
        .chars()
        .any(|c| py::is_space(c) && !matches!(c, ' ' | '\t'))
    {
        return None; // YAML separates with space and tab only, so other whitespace stays unknown
    }
    let text = py::strip(text);
    if text.starts_with('"') {
        let chars = text.chars().collect::<Vec<_>>();
        let (value, end) = pyjson::scanstring(&chars, 1).ok()?;
        if value.is_empty() || value.contains(['\r', '\n']) {
            return None;
        }
        let tail = chars[end..].iter().collect::<String>();
        let tail = py::strip(&tail);
        return (tail.is_empty() || tail.starts_with('#')).then_some(value);
    }
    if text.starts_with('\'') {
        let groups = full_captures(r"'((?:[^']|'')*)'\s*(?:#.*)?", text)?;
        return (!groups[1].is_empty()).then(|| groups[1].replace("''", "'"));
    }
    let text = without_comment(text);
    let first = text.chars().next()?;
    if "!&*[]{}>|%@`#.+-0123456789".contains(first)
        || common::matches(r":\s|[\r\n]", text)
        || ["null", "true", "false", "yes", "no", "on", "off", "~"]
            .contains(&text.to_lowercase().as_str())
        || common::fullmatch(r"[-+]?\d+(?:\.\d+)?(?:[eE][-+]?\d+)?", text)
    {
        return None;
    }
    Some(text.into())
}

/// Read one plain block property, retaining unsupported structures as unknown.
fn block<'a>(lines: &[&'a str], key: &str, indent: usize) -> Option<Vec<&'a str>> {
    let spaces = " ".repeat(indent);
    let positions = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.starts_with(&spaces) && job_key(&line[indent..]) == Some(key))
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    let [start] = positions[..] else {
        return None;
    };
    let tail = py::strip(lines[start].split_once(':')?.1);
    if !tail.is_empty() && !tail.starts_with('#') {
        return None;
    }
    let mut end = start + 1;
    while end < lines.len() {
        let line = lines[end];
        if !is_filler(line) && line.len() - line.trim_start_matches(' ').len() <= indent {
            break;
        }
        end += 1;
    }
    Some(lines[start + 1..end].to_vec())
}

/// Read static include rows with scalar values and no other matrix dimensions.
fn matrix_values(lines: &[&str], key: &str) -> Option<BTreeSet<String>> {
    let strategy = block(lines, "strategy", 4)?;
    let matrix = block(&strategy, "matrix", 6)?;
    let include = block(&matrix, "include", 8)?;
    for line in &matrix {
        if !is_filler(line)
            && line.chars().nth(8) != Some(' ')
            && job_key(from_char(line, 8)) != Some("include")
        {
            return None;
        }
    }
    let mut rows: Vec<BTreeMap<String, String>> = vec![];
    for line in include {
        if is_filler(line) {
            continue;
        }
        if line.starts_with("          - ") {
            rows.push(BTreeMap::new());
        } else if !line.starts_with("            ")
            || matches!(line.chars().nth(12), None | Some(' '))
        {
            return None;
        }
        let row = rows.last_mut()?;
        let field = job_key(&line[12..])?;
        let value = literal_name(line.split_once(':')?.1)?;
        if row.contains_key(field) || value.contains("${{") {
            return None;
        }
        row.insert(field.into(), value);
    }
    if rows.is_empty() {
        return None;
    }
    rows.into_iter().map(|mut row| row.remove(key)).collect()
}

/// Resolve the finite expression forms used here; keep all others conservative.
fn reported_names(name: Option<String>, lines: &[&str], job: &str) -> Names {
    let name = name?;
    let mut names = BTreeSet::from([name.clone()]);
    if let Some(groups) = full_captures(r"\$\{\{\s*matrix\.([A-Za-z_][A-Za-z_0-9]*)\s*\}\}", &name)
        && let Some(values) = matrix_values(lines, &groups[1])
    {
        names = values;
    }
    let conditional = format!(
        r"\$\{{\{{\s*matrix\.[A-Za-z_][A-Za-z_0-9]*\s*==\s*{EXPRESSION_STRING}\s*&&\s*{EXPRESSION_STRING}\s*\|\|\s*{EXPRESSION_STRING}\s*\}}\}}"
    );
    if let Some(groups) = full_captures(&conditional, &name)
        && !groups[2].is_empty()
        && !groups[3].is_empty()
    {
        names = BTreeSet::from([groups[2].replace("''", "'"), groups[3].replace("''", "'")]);
    }
    let properties = lines
        .iter()
        .filter(|line| line.starts_with("    ") && !matches!(line.chars().nth(4), None | Some(' ')))
        .map(|line| job_key(&line[4..]))
        .collect::<BTreeSet<_>>();
    if !properties.contains(&Some("name")) && properties.contains(&Some("strategy")) {
        // GitHub reports "job (values)", or the bare id when the strategy has no matrix.
        names = BTreeSet::from([job.into(), format!("{job} (${{{{ matrix }}}})")]);
    }
    if properties.contains(&Some("uses")) {
        names = names
            .into_iter()
            .map(|name| format!("{name} / ${{{{ called-job }}}}"))
            .collect();
    }
    Some(names)
}

/// Record a job-key line; return its id (None if unreadable) and whether its value is unknown.
fn start_job(
    line: &str,
    names: &mut BTreeMap<String, Option<String>>,
    bodies: &mut BTreeMap<String, Vec<String>>,
    unreadable: &mut Vec<String>,
) -> (Option<String>, bool) {
    let Some(job) = job_key(&line[2..]) else {
        unreadable.push(line.into());
        return (None, false);
    };
    if names.contains_key(job) {
        unreadable.push(line.into());
    }
    let tail = line.split_once(':').map_or("", |(_, tail)| py::strip(tail));
    let unknown = !tail.is_empty() && !tail.starts_with('#');
    names.insert(job.into(), (!unknown).then(|| job.into()));
    bodies.insert(job.into(), vec![]);
    (Some(job.into()), unknown)
}

/// Return job ids mapped to possible reported names, plus unreadable job-level lines.
fn parse_jobs(text: &str) -> Option<Parsed> {
    let (_, body) = text.split_once("\njobs:\n")?;
    let mut names = BTreeMap::new();
    let mut bodies: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut unreadable = vec![];
    let (mut job, mut unknown, mut in_name, mut has_property) = (None, false, false, false);
    let mut properties = BTreeSet::new();
    for line in py::splitlines(body) {
        if py::strip(line).is_empty() || line.trim_start_matches([' ', '\t']).starts_with('#') {
            continue;
        }
        if !line.starts_with(' ') {
            break; // the next top-level key ends the jobs block
        }
        if line.starts_with("  ") && !matches!(line.chars().nth(2), None | Some(' ' | '#')) {
            (job, unknown) = start_job(line, &mut names, &mut bodies, &mut unreadable);
            (in_name, has_property) = (false, false);
            properties.clear();
        } else if let Some(current) = &job
            && !is_filler(line)
        {
            bodies
                .get_mut(current)
                .expect("started job")
                .push(line.into());
            let indent = line.len() - line.trim_start_matches(' ').len();
            if indent < 4 || (!has_property && indent != 4) {
                unknown = true;
            }
            if line.starts_with("    ") && !matches!(line.chars().nth(4), None | Some(' ')) {
                has_property = true;
                let key = job_key(&line[4..]);
                in_name = key == Some("name");
                if key.is_none_or(|key| !properties.insert(key.to_string())) {
                    unknown = true; // an unreadable or repeated property
                } else if in_name {
                    let value = line.split_once(':').map_or("", |(_, value)| value);
                    names.insert(current.clone(), literal_name(value));
                }
            } else if in_name {
                unknown = true; // a continued name is outside the single-line reader
            }
            if unknown {
                names.insert(current.clone(), None);
            }
        }
    }
    Some((resolve(names, &bodies), unreadable))
}

fn resolve(
    names: BTreeMap<String, Option<String>>,
    bodies: &BTreeMap<String, Vec<String>>,
) -> BTreeMap<String, Names> {
    names
        .into_iter()
        .map(|(job, name)| {
            let lines = bodies[&job].iter().map(String::as_str).collect::<Vec<_>>();
            let names = reported_names(name, &lines, &job);
            (job, names)
        })
        .collect()
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
    Ok(ids.into_keys().collect())
}

/// Keep unknown or expression-derived names unless their literal ends rule out a match.
fn possible_context(names: &Names, context: &str) -> bool {
    let Some(names) = names else {
        return true;
    };
    names.iter().any(|name| match name.split_once("${{") {
        None => name == context,
        Some((prefix, expression)) => match expression.rsplit_once("}}") {
            None => true,
            Some((_, suffix)) => {
                py::char_len(context) >= py::char_len(prefix) + py::char_len(suffix)
                    && context.starts_with(prefix)
                    && context.ends_with(suffix)
            }
        },
    })
}

/// Read event names from a scalar, flat list or plain block; aliases stay unknown.
fn workflow_modes(text: &str) -> Option<BTreeSet<&'static str>> {
    let lines = py::splitlines(text);
    let headers = lines
        .iter()
        .filter(|line| job_key(line) == Some("on"))
        .collect::<Vec<_>>();
    let [header] = headers[..] else {
        return None;
    };
    let tail = py::strip(header.split_once(':')?.1);
    let mut events = BTreeSet::new();
    if tail.is_empty() || tail.starts_with('#') {
        for line in block(&lines, "on", 0)? {
            if is_filler(line) {
                continue;
            }
            if line.starts_with("  ") && line.chars().nth(2) != Some(' ') {
                let event = job_key(&line[2..])?;
                if !events.insert(event.to_string()) {
                    return None;
                }
            } else if events.is_empty() {
                return None;
            }
        }
    } else {
        let tail = without_comment(tail);
        let values = match tail.strip_prefix('[').and_then(|t| t.strip_suffix(']')) {
            Some(inner) => inner.split(',').collect(),
            None => vec![tail],
        };
        for value in values {
            let name = literal_name(value)?;
            if job_key(&format!("{name}:")) != Some(name.as_str()) {
                return None;
            }
            events.insert(name);
        }
    }
    let modes = events
        .iter()
        .map(|event| {
            if event == "workflow_call" {
                "callable"
            } else {
                "standalone"
            }
        })
        .collect::<BTreeSet<_>>();
    (!modes.is_empty()).then_some(modes)
}

/// Whether HEAD still runs GATE from PATH, which then keeps its own obligations.
fn gate_kept(
    path: &str,
    gate: &str,
    git: &mut impl FnMut(&[&str]) -> Result<String>,
) -> Result<bool> {
    if py::strip(&git(&["ls-tree", "--name-only", "HEAD", "--", path])?).is_empty() {
        return Ok(false);
    }
    let text = git(&["show", &format!("HEAD:{path}")])?;
    Ok(parse_jobs(&text)
        .is_some_and(|(ids, unreadable)| unreadable.is_empty() && ids.contains_key(gate)))
}

/// Count the jobs in a base workflow that can report CONTEXT; unresolvable ones fail.
fn context_sources(
    path: &str,
    jobs: &BTreeMap<String, Names>,
    base_modes: Option<&BTreeSet<&str>>,
    context: &str,
) -> Result<usize> {
    let mut count = 0;
    for names in jobs.values() {
        if possible_context(names, context) {
            if base_modes.is_none() {
                return Err(format!(
                    "Cannot determine the invocation namespace in {path}; update the reader"
                ));
            }
            if names
                .as_ref()
                .is_none_or(|names| names.iter().any(|name| name.contains("${{")))
            {
                return Err(format!(
                    "Cannot resolve a possible gate context in {path}; update the reader"
                ));
            }
            count += 1;
        }
    }
    Ok(count)
}

/// The gate's single literal context and its workflow's single invocation namespace.
fn gate_identity(head: &str, gate: &str) -> Result<(String, BTreeSet<&'static str>)> {
    let context = match parse_jobs(head)
        .and_then(|(mut ids, _)| ids.remove(gate))
        .flatten()
    {
        Some(names) if names.len() == 1 && !names.iter().any(|name| name.contains("${{")) => {
            names.into_iter().next().expect("one name")
        }
        _ => {
            return Err("Cannot determine the gate context; use a single-line literal name".into());
        }
    };
    match workflow_modes(head) {
        Some(modes) if modes.len() == 1 => Ok((context, modes)),
        _ => Err("Cannot determine the workflow invocation namespace; update the reader".into()),
    }
}

/// The jobs of a base workflow, which must be readable to rule anything out.
fn base_workflow_jobs(path: &str, text: &str) -> Result<BTreeMap<String, Names>> {
    match parse_jobs(text) {
        Some((jobs, unreadable)) if !jobs.is_empty() && unreadable.is_empty() => Ok(jobs),
        _ => Err(format!(
            "Cannot read base workflow {path}; the gate check needs updating"
        )),
    }
}

/// Keep both destination and uniquely established context-source removal obligations.
fn base_jobs_with(
    workflow: &str,
    gate: &str,
    head: &str,
    git: &mut impl FnMut(&[&str]) -> Result<String>,
    paths: impl FnOnce() -> Result<Vec<String>>,
) -> Result<Jobs> {
    let mut required = Jobs::new();
    let mut destination = Jobs::new();
    if !py::strip(&git(&["ls-tree", "--name-only", "HEAD^1", "--", workflow])?).is_empty() {
        destination = job_ids(&git(&["show", &format!("HEAD^1:{workflow}")])?)?;
        required.extend(destination.iter().cloned());
    }
    let (context, modes) = gate_identity(head, gate)?;
    // shortcut: only static include rows and literal-result conditionals resolve;
    // keep other expressions possible and use a maintained parser for broader YAML.
    let mut candidates = vec![];
    let mut same_id = vec![];
    for path in paths()? {
        // GitHub reads workflows only from the top level of .github/workflows.
        if path.matches('/').count() != 2 || !(path.ends_with(".yml") || path.ends_with(".yaml")) {
            continue;
        }
        let text = git(&["show", &format!("HEAD^1:{path}")])?;
        let base_modes = workflow_modes(&text);
        if base_modes
            .as_ref()
            .is_some_and(|base| base.is_disjoint(&modes))
        {
            continue;
        }
        let jobs = base_workflow_jobs(&path, &text)?;
        let ids = jobs.keys().cloned().collect::<Jobs>();
        if ids.contains(gate) && path != workflow && !gate_kept(&path, gate, git)? {
            same_id.push(ids.clone());
        }
        let count = context_sources(&path, &jobs, base_modes.as_ref(), &context)?;
        candidates.extend(std::iter::repeat_n(ids, count));
    }
    if candidates.len() > 1 {
        return Err(format!(
            "More than one base job can precede {gate}; the gate check needs updating"
        ));
    }
    if let Some(source) = candidates.pop() {
        required.extend(source);
    } else if !destination.contains(gate) {
        // A gate that kept its id but changed file and context still owes the base jobs.
        if same_id.len() > 1 {
            return Err(format!(
                "More than one base workflow can precede {gate}; the gate check needs updating"
            ));
        }
        required.extend(same_id.into_iter().flatten());
    }
    Ok(required)
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

fn run_git(command: &mut Command) -> Result<(i32, Vec<u8>, Vec<u8>)> {
    let output = command.output().map_err(|e| format!("OSError: {e}"))?;
    Ok((
        output.status.code().unwrap_or(-1),
        output.stdout,
        output.stderr,
    ))
}

fn called_process_error(args: &[&str], code: i32) -> String {
    let arguments = std::iter::once("git")
        .chain(args.iter().copied())
        .map(py::repr_str)
        .collect::<Vec<_>>()
        .join(", ");
    format!("CalledProcessError: Command '[{arguments}]' returned non-zero exit status {code}.")
}

fn utf8(bytes: &[u8]) -> Result<String> {
    py::decode_utf8(bytes).map_err(|e| format!("UnicodeDecodeError: {e}"))
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
    let stdout = py::universal_newlines(&utf8(&stdout)?);
    utf8(&stderr)?;
    if code != 0 {
        return Err(called_process_error(args, code));
    }
    Ok(stdout)
}
fn git(args: &[&str]) -> Result<String> {
    git_with(args, run_git)
}

/// Base workflow paths: status first, then strict UTF-8 on the raw NUL-delimited
/// listing, so CR and LF in names survive.
fn base_workflow_paths_with(
    run: impl FnOnce(&mut Command) -> Result<(i32, Vec<u8>, Vec<u8>)>,
) -> Result<Vec<String>> {
    let args = ["ls-tree", "-r", "--name-only", "-z", "HEAD^1", "--", PREFIX];
    let mut command = Command::new("git");
    command.args(args).stdin(Stdio::inherit());
    let (code, stdout, _) = run(&mut command)?;
    if code != 0 {
        return Err(called_process_error(&args, code));
    }
    Ok(utf8(&stdout)?.split('\0').map(String::from).collect())
}
fn base_workflow_paths() -> Result<Vec<String>> {
    base_workflow_paths_with(run_git)
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
    // A declaration counts only at the start of a line, where it cannot be part of a
    // YAML value; only "\n" ends a YAML line, so NEL and U+2028 never split one.
    for line in text.split('\n') {
        if let Some(names) = line.strip_prefix("# removed-jobs:") {
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
    base: &Jobs,
    workflow: &str,
    jobs: &Jobs,
    declared: &Jobs,
    out: &mut Vec<String>,
) -> Result<()> {
    let removed = base.difference(jobs).cloned().collect::<Jobs>();
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
    base: impl FnOnce(&str, &str, &str) -> Result<Jobs>,
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
        if environment("GITHUB_EVENT_NAME")?.as_deref() == Some("pull_request") {
            let required = base(workflow, gate, &text)?;
            check_removals(&required, workflow, &jobs, &declared, &mut out)?;
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
        |workflow, gate, head| base_jobs_with(workflow, gate, head, &mut git, base_workflow_paths),
    )
}

#[cfg(test)]
mod tests;
