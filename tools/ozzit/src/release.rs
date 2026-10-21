//! Create and independently verify Ozzit's copy-only workbook release bundle.
//!
//! The tool copies the exact tracked workbook, proves it matches the reviewed
//! base manifest, runs the seven static gates and writes deterministic
//! verification evidence. It does not tag, upload or publish anything.
use crate::{
    cache::{self, Dump},
    common::*,
    native_evidence::sha256,
    py,
    pyjson::{self, Json, UniqueError},
};
use num_bigint::BigInt;
use num_traits::FromPrimitive;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

pub const ASSETS: [&str; 3] = ["ozzit.xlsx", "provenance.json", "SHA256SUMS"];
const CHECKSUMMED: [&str; 2] = ["ozzit.xlsx", "provenance.json"];
/// The provenance records each gate by the command its schema names.
pub const GATE_COMMANDS: [&str; 7] = [
    "python tools/verify_workbook.py ozzit.xlsx",
    "python tools/verify_sources.py ozzit.xlsx src",
    "python tools/verify_signatures.py src",
    "python tools/verify_previous_names.py functions.csv",
    "python tools/verify_index.py ozzit.xlsx src functions.csv",
    "python tools/verify_cover.py ozzit.xlsx functions.csv CHANGELOG.md",
    "python tools/verify_afe.py ozzit.xlsx src",
];
const LIMITATIONS: [&str; 2] = [
    "The bundle copies the reviewed tracked workbook; it does not regenerate it from the earlier workbook.",
    "The documented Mypy, unit, native Excel and cache gates remain required before publication.",
];
const BASE_KEYS: [&str; 6] = [
    "git_blob_sha1",
    "last_workbook_commit",
    "path",
    "schema",
    "sha256",
    "size",
];
pub const BASE_MANIFEST: &str = "release/workbook-base.json";
const GIT_TIMEOUT: Duration = Duration::from_secs(30);
const GATE_TIMEOUT: Duration = Duration::from_secs(120);

// ---- small Python-compatible helpers ---------------------------------------

/// Python's `==` between decoded JSON values: numbers compare by value across
/// bool, int and float; objects compare as unordered mappings.
pub fn py_eq(a: &Json, b: &Json) -> bool {
    fn number(value: &Json) -> Option<std::result::Result<BigInt, f64>> {
        match value {
            Json::Bool(b) => Some(Ok(BigInt::from(u8::from(*b)))),
            Json::Int(i) => Some(Ok(i.clone())),
            Json::Float(f) => Some(Err(*f)),
            _ => None,
        }
    }
    match (a, b) {
        (Json::Null, Json::Null) => true,
        (Json::Str(x), Json::Str(y)) => x == y,
        (Json::Array(x), Json::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(x, y)| py_eq(x, y))
        }
        (Json::Object(x), Json::Object(y)) => {
            let unique = |pairs: &Vec<(String, Json)>| {
                let mut keys = pairs.iter().map(|(k, _)| k).collect::<Vec<_>>();
                keys.sort();
                keys.dedup();
                keys.len()
            };
            unique(x) == unique(y)
                && x.iter().all(|(k, v)| {
                    let mine = a.get(k).unwrap_or(v);
                    b.get(k).is_some_and(|theirs| py_eq(mine, theirs))
                })
        }
        _ => match (number(a), number(b)) {
            (Some(Ok(x)), Some(Ok(y))) => x == y,
            (Some(Err(x)), Some(Err(y))) => x == y,
            (Some(Ok(i)), Some(Err(f))) | (Some(Err(f)), Some(Ok(i))) => {
                f.is_finite() && f.fract() == 0.0 && BigInt::from_f64(f).is_some_and(|f| f == i)
            }
            _ => false,
        },
    }
}

fn keys(value: &Json) -> Vec<&str> {
    match value {
        Json::Object(pairs) => {
            let mut keys = pairs.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>();
            keys.sort_unstable();
            keys.dedup();
            keys
        }
        _ => vec![],
    }
}

fn has_keys(value: &Json, expected: &[&str]) -> bool {
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    keys(value) == expected
}

fn int_is(value: Option<&Json>, n: i64) -> bool {
    matches!(value, Some(Json::Int(i)) if *i == BigInt::from(n))
}

fn strings(items: &[&str]) -> Json {
    Json::Array(items.iter().map(|s| Json::str_obj(s)).collect())
}

fn object(pairs: Vec<(&str, Json)>) -> Json {
    Json::Object(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

/// `Path.resolve(strict=False)`: links in the existing part are resolved and
/// the rest of the path is joined lexically.
pub fn resolve_lenient(path: &Path) -> PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut out = PathBuf::new();
    let mut existing = true;
    for component in absolute.components() {
        match component {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => {
                out.push(other);
                if existing {
                    if out.symlink_metadata().is_ok() {
                        out = py::resolve(&out);
                    } else {
                        existing = false;
                    }
                }
            }
        }
    }
    out
}

pub fn validate_version(value: &str) -> Result<String> {
    if !fullmatch(
        r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)",
        value,
    ) {
        return Err("version must be canonical X.Y.Z without a v prefix".into());
    }
    Ok(value.into())
}

pub fn validate_source_commit(value: &str) -> Result<String> {
    if !fullmatch("[0-9a-f]{40}", value) {
        return Err("source commit must be a full 40-character lowercase SHA".into());
    }
    Ok(value.into())
}

/// SHA-1, for the Git blob object ID. Written out here rather than adding a
/// dependency for one digest of one file.
pub fn sha1_hex(data: &[u8]) -> String {
    let mut h: [u32; 5] = [
        0x6745_2301,
        0xEFCD_AB89,
        0x98BA_DCFE,
        0x1032_5476,
        0xC3D2_E1F0,
    ];
    let mut message = data.to_vec();
    let bits = (data.len() as u64).wrapping_mul(8);
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend(bits.to_be_bytes());
    for block in message.as_chunks::<64>().0 {
        let mut w = [0u32; 80];
        for (i, word) in block.as_chunks::<4>().0.iter().enumerate() {
            w[i] = u32::from_be_bytes(*word);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, word) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A82_7999),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6),
            };
            let t = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*word);
            (e, d, c, b, a) = (d, c, b.rotate_left(30), a, t);
        }
        for (state, value) in h.iter_mut().zip([a, b, c, d, e]) {
            *state = state.wrapping_add(value);
        }
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}

pub fn git_blob_sha1(path: &Path) -> Result<String> {
    let data = read_bytes(path)?;
    let mut object = format!("blob {}\0", data.len()).into_bytes();
    object.extend(data);
    Ok(sha1_hex(&object))
}

/// Read a UTF-8 JSON document whose root must be an object, refusing repeated members.
pub fn load_json_object(path: &Path) -> Result<Json> {
    let name = path
        .file_name()
        .map_or(String::new(), |n| n.to_string_lossy().into());
    let text = py::read_text(path).map_err(|e| {
        let reason = match e {
            py::ReadError::Os(e) => {
                py::oserror_str(&e, &path.display().to_string(), py::OsStyle::Errno)
            }
            py::ReadError::Decode(e) => e.to_string(),
        };
        format!("invalid JSON in {name}: {reason}")
    })?;
    let loaded = pyjson::loads_unique_keys(&text).map_err(|e| match e {
        UniqueError::Decode(e) => format!("invalid JSON in {name}: {e}"),
        UniqueError::Duplicate(key) => format!("duplicate JSON member: {key}"),
    })?;
    if !matches!(loaded, Json::Object(_)) {
        return Err(format!("{name} must contain a JSON object"));
    }
    Ok(loaded)
}

/// Deterministic UTF-8 bytes: sorted keys, compact separators, a final newline.
pub fn canonical_json(document: &Json) -> Vec<u8> {
    (pyjson::dumps_sorted_compact(document) + "\n").into_bytes()
}

fn regular_file(path: &Path) -> bool {
    path.is_file() && !path.is_symlink()
}

/// Prove a workbook is the exact byte sequence locked by the base manifest.
pub fn verify_base_workbook(workbook: &Path, manifest: &Path) -> Result<Json> {
    if !regular_file(workbook) {
        return Err("base workbook must be a regular non-symlink file".into());
    }
    if !regular_file(manifest) {
        return Err("base workbook manifest must be a regular non-symlink file".into());
    }
    let document = load_json_object(manifest)?;
    if !has_keys(&document, &BASE_KEYS) {
        return Err("base workbook manifest has an unexpected schema".into());
    }
    let field = |k| document.get(k).expect("checked keys");
    if !int_is(Some(field("schema")), 1) {
        return Err("base workbook manifest schema must be 1".into());
    }
    if field("path").as_str() != Some("ozzit.xlsx") {
        return Err("base workbook manifest path must be ozzit.xlsx".into());
    }
    if !matches!(field("size"), Json::Int(i) if *i >= BigInt::from(1)) {
        return Err("base workbook manifest size must be a positive integer".into());
    }
    if !field("sha256")
        .as_str()
        .is_some_and(|s| fullmatch("[0-9a-f]{64}", s))
    {
        return Err("base workbook manifest SHA-256 is invalid".into());
    }
    if !field("git_blob_sha1")
        .as_str()
        .is_some_and(|s| fullmatch("[0-9a-f]{40}", s))
    {
        return Err("base workbook manifest Git blob SHA-1 is invalid".into());
    }
    let Some(commit) = field("last_workbook_commit").as_str() else {
        return Err("base workbook manifest last commit is invalid".into());
    };
    validate_source_commit(commit)?;
    let blob = git_blob_sha1(workbook)?;
    let digest = sha256(workbook)?;
    let size = std::fs::metadata(workbook)
        .map_err(|e| io_error(&e, workbook))?
        .len();
    if field("git_blob_sha1").as_str() != Some(blob.as_str())
        || field("sha256").as_str() != Some(digest.as_str())
        || !matches!(field("size"), Json::Int(i) if *i == BigInt::from(size))
    {
        return Err("base workbook bytes do not match release/workbook-base.json".into());
    }
    Ok(document)
}

/// `repr` of a command list, as `subprocess` errors show it.
fn command_repr(command: &[String]) -> String {
    format!(
        "[{}]",
        command
            .iter()
            .map(|a| py::repr_str(a))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn not_found(program: &str) -> String {
    if cfg!(windows) {
        "[WinError 2] The system cannot find the file specified".into()
    } else {
        format!(
            "[Errno 2] No such file or directory: {}",
            py::repr_str(program)
        )
    }
}

/// Run one closed read-only Git query and return its trimmed output.
pub fn git_output(root: &Path, arguments: &[&str]) -> Result<String> {
    let args = arguments.iter().map(|a| a.to_string()).collect::<Vec<_>>();
    let mut command = vec!["git".to_string()];
    command.extend(args.iter().cloned());
    match cache::run_process("git", &args, Some(root), GIT_TIMEOUT)? {
        Dump::NotFound => Err(not_found("git")),
        Dump::TimedOut => Err(format!(
            "Command '{}' timed out after 30 seconds",
            command_repr(&command)
        )),
        Dump::Exited {
            code: 0, stdout, ..
        } => Ok(py::strip(&stdout).into()),
        Dump::Exited { code, .. } => Err(format!(
            "Command '{}' returned non-zero exit status {code}.",
            command_repr(&command)
        )),
    }
}

/// Bind the base manifest's Git claims to the candidate checkout.
pub fn verify_repository_base(root: &Path, base: &Json) -> Result<()> {
    let tracked = git_output(root, &["ls-files", "--error-unmatch", "--", "ozzit.xlsx"])?;
    if tracked != "ozzit.xlsx" {
        return Err("base workbook must be tracked as ozzit.xlsx".into());
    }
    let blob = git_output(root, &["hash-object", "--", "ozzit.xlsx"])?;
    if base.get("git_blob_sha1").and_then(Json::as_str) != Some(blob.as_str()) {
        return Err("base workbook Git blob does not match the manifest".into());
    }
    let last = git_output(root, &["log", "-1", "--format=%H", "--", "ozzit.xlsx"])?;
    if base.get("last_workbook_commit").and_then(Json::as_str) != Some(last.as_str()) {
        return Err("base workbook last-change commit does not match Git history".into());
    }
    Ok(())
}

/// Remove checkout-specific paths and line-ending differences from gate output.
pub fn normalise_gate_output(text: &str, root: &Path, workbook: &Path) -> String {
    let shown = |p: PathBuf| py::resolve(&p).display().to_string();
    let mut replacements = vec![
        (shown(workbook.to_path_buf()), "ozzit.xlsx"),
        (shown(root.join("functions.csv")), "functions.csv"),
        (shown(root.join("src")), "src"),
        (shown(root.to_path_buf()), "."),
    ];
    // Longest first; equal lengths keep their insertion order, as Python's stable sort does.
    let mut unique: Vec<(String, &str)> = Vec::new();
    for (source, target) in replacements.drain(..) {
        match unique.iter_mut().find(|(s, _)| *s == source) {
            Some(entry) => entry.1 = target,
            None => unique.push((source, target)),
        }
    }
    unique.sort_by_key(|(s, _)| std::cmp::Reverse(s.chars().count()));
    let mut out = text.to_string();
    for (source, target) in unique {
        out = out.replace(&source, target);
        out = out.replace(&source.replace('\\', "/"), target);
    }
    out.replace('\\', "/")
        .replace("\r\n", "\n")
        .trim_end_matches('\n')
        .into()
}

/// Run the seven static gates against the staged workbook, index and changelog.
/// One gate's exit code and combined output.
fn run_gate(exe: &str, args: Vec<String>, root: &Path) -> Result<(i64, String)> {
    match cache::run_process(exe, &args, Some(root), GATE_TIMEOUT)? {
        Dump::NotFound => Err(not_found(exe)),
        Dump::TimedOut => {
            let mut command = vec![exe.to_string()];
            command.extend(args);
            Err(format!(
                "Command '{}' timed out after 120 seconds",
                command_repr(&command)
            ))
        }
        // The Python gates wrote stderr unbuffered ahead of their buffered stdout.
        Dump::Exited {
            code,
            stdout,
            stderr,
        } => Ok((code, format!("{stderr}{stdout}"))),
    }
}

pub fn run_semantic_gates(root: &Path, workbook: &Path) -> Result<Vec<(String, String)>> {
    let workbook = py::resolve(workbook);
    let w = workbook.display().to_string();
    let r = |part: &str| root.join(part).display().to_string();
    let specifications: [(&str, Vec<String>); 7] = [
        ("verify_workbook", vec![w.clone()]),
        ("verify_sources", vec![w.clone(), r("src")]),
        ("verify_signatures", vec![r("src")]),
        ("verify_previous_names", vec![r("functions.csv")]),
        (
            "verify_index",
            vec![w.clone(), r("src"), r("functions.csv")],
        ),
        (
            "verify_cover",
            vec![w.clone(), r("functions.csv"), r("CHANGELOG.md")],
        ),
        ("verify_afe", vec![w.clone(), r("src")]),
    ];
    // The gates run as this same binary in child processes, as the Python
    // tool ran each gate script; no path from input chooses the program.
    // nosemgrep: rust.lang.security.current-exe.current-exe
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe = exe.display().to_string();
    let mut results = Vec::new();
    for ((gate, arguments), display) in specifications.into_iter().zip(GATE_COMMANDS) {
        let mut args = vec![gate.to_string()];
        args.extend(arguments);
        let (code, output) = run_gate(&exe, args, root)?;
        let output = normalise_gate_output(&output, root, &workbook);
        if code != 0 {
            let detail = if output.is_empty() {
                String::new()
            } else {
                format!("\n{output}")
            };
            return Err(format!("{display} failed with exit code {code}{detail}"));
        }
        results.push((display.to_string(), output));
    }
    Ok(results)
}

fn gates_json(gates: &[(String, String)]) -> Json {
    Json::Array(
        gates
            .iter()
            .map(|(command, output)| {
                object(vec![
                    ("command", Json::str_obj(command)),
                    ("output", Json::str_obj(output)),
                ])
            })
            .collect(),
    )
}

fn native_gates() -> Json {
    object(vec![
        ("included", Json::Bool(false)),
        ("required_before_publication", Json::Bool(true)),
    ])
}

fn workbook_evidence(base: &Json) -> Json {
    let field = |k| base.get(k).cloned().unwrap_or(Json::Null);
    object(vec![
        ("base_manifest", Json::str_obj(BASE_MANIFEST)),
        ("copy_only", Json::Bool(true)),
        ("git_blob_sha1", field("git_blob_sha1")),
        ("last_workbook_commit", field("last_workbook_commit")),
        ("sha256", field("sha256")),
        ("size", field("size")),
    ])
}

/// The deterministic provenance document for a staged workbook.
pub fn provenance(base: &Json, version: &str, commit: &str, gates: &[(String, String)]) -> Json {
    object(vec![
        ("assets", strings(&ASSETS)),
        ("project", Json::str_obj("Ozzit")),
        ("schema", Json::Int(BigInt::from(1))),
        (
            "source",
            object(vec![
                ("commit", Json::str_obj(commit)),
                ("workbook_path", Json::str_obj("ozzit.xlsx")),
            ]),
        ),
        (
            "verification",
            object(vec![
                ("limitations", strings(&LIMITATIONS)),
                ("native_gates", native_gates()),
                ("static_gates", gates_json(gates)),
            ]),
        ),
        ("version", Json::str_obj(version)),
        ("workbook", workbook_evidence(base)),
    ])
}

fn checksum_bytes(checksums: &[(String, String)]) -> Vec<u8> {
    let mut sorted = checksums.to_vec();
    sorted.sort();
    sorted
        .iter()
        .map(|(name, digest)| format!("{digest}  {name}\n"))
        .collect::<String>()
        .into_bytes()
}

/// Parse a canonical SHA256SUMS file and reject ambiguous entries.
pub fn parse_checksums(path: &Path) -> Result<Vec<(String, String)>> {
    let data = std::fs::read(path).map_err(|e| {
        format!(
            "invalid SHA256SUMS: {}",
            py::oserror_str(&e, &path.display().to_string(), py::OsStyle::Errno)
        )
    })?;
    let Some(position) = data.iter().position(|b| !b.is_ascii()) else {
        return parse_ascii_checksums(&data);
    };
    Err(format!(
        "invalid SHA256SUMS: 'ascii' codec can't decode byte 0x{:02x} in position {position}: ordinal not in range(128)",
        data[position]
    ))
}

fn parse_ascii_checksums(data: &[u8]) -> Result<Vec<(String, String)>> {
    let text = std::str::from_utf8(data).expect("ASCII");
    if !text.ends_with('\n') || text.contains('\r') {
        return Err("SHA256SUMS must use canonical LF-terminated lines".into());
    }
    let mut checksums: Vec<(String, String)> = Vec::new();
    for line in py::splitlines(text) {
        let found = captures(r"^([0-9a-f]{64})  ([A-Za-z0-9._-]+)$", line);
        let Some(m) = found.first() else {
            return Err("SHA256SUMS contains a malformed line".into());
        };
        let (digest, name) = (m[1].clone(), m[2].clone());
        if checksums.iter().any(|(n, _)| *n == name) {
            return Err(format!("SHA256SUMS repeats {name}"));
        }
        checksums.push((name, digest));
    }
    let mut names = checksums
        .iter()
        .map(|(n, _)| n.as_str())
        .collect::<Vec<_>>();
    names.sort_unstable();
    if names != CHECKSUMMED {
        return Err("SHA256SUMS has an unexpected inventory".into());
    }
    if data != checksum_bytes(&checksums) {
        return Err("SHA256SUMS is not canonically ordered".into());
    }
    Ok(checksums)
}

/// The bundle must hold exactly the three reviewed assets as regular files.
fn check_inventory(bundle: &Path) -> Result<()> {
    if !bundle.is_dir() || bundle.is_symlink() {
        return Err("bundle must be a regular non-symlink directory".into());
    }
    let entries = std::fs::read_dir(bundle)
        .map_err(|e| py::oserror_str(&e, &bundle.display().to_string(), py::OsStyle::Native))?
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(|e| py::oserror_str(&e, &bundle.display().to_string(), py::OsStyle::Native))?;
    if entries.iter().any(|e| !regular_file(&e.path())) {
        return Err("bundle inventory may contain only regular non-symlink files".into());
    }
    let mut names = entries
        .iter()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    names.sort();
    let mut assets = ASSETS.to_vec();
    assets.sort_unstable();
    if names != assets {
        return Err("bundle inventory does not match the three reviewed assets".into());
    }
    Ok(())
}

/// The provenance identity, assets, version and source.
fn check_provenance_source(document: &Json) -> Result<()> {
    if !has_keys(
        document,
        &[
            "assets",
            "project",
            "schema",
            "source",
            "verification",
            "version",
            "workbook",
        ],
    ) {
        return Err("provenance.json has an unexpected top-level schema".into());
    }
    let get = |k| document.get(k);
    if !int_is(get("schema"), 1) || get("project").and_then(Json::as_str) != Some("Ozzit") {
        return Err("provenance.json identifies an unsupported bundle".into());
    }
    if !get("assets").is_some_and(|a| py_eq(a, &strings(&ASSETS))) {
        return Err("provenance asset inventory is invalid".into());
    }
    let Some(version) = get("version").and_then(Json::as_str) else {
        return Err("provenance version is invalid".into());
    };
    validate_version(version)?;
    let source = get("source").expect("checked keys");
    if !has_keys(source, &["commit", "workbook_path"]) {
        return Err("provenance source is invalid".into());
    }
    let Some(commit) = source.get("commit").and_then(Json::as_str) else {
        return Err("provenance source commit is invalid".into());
    };
    validate_source_commit(commit)?;
    if source.get("workbook_path").and_then(Json::as_str) != Some("ozzit.xlsx") {
        return Err("provenance workbook path is invalid".into());
    }
    Ok(())
}

/// The recorded static gates, as (command, output) pairs in the expected order.
fn static_gates(verification: &Json) -> Result<Vec<(String, String)>> {
    if !matches!(verification, Json::Object(_)) {
        return Err("provenance verification evidence is invalid".into());
    }
    let gates = match verification.get("static_gates") {
        Some(Json::Array(items)) => items
            .iter()
            .map(
                |item| match (item, item.get("command"), item.get("output")) {
                    (Json::Object(_), Some(Json::Str(c)), Some(Json::Str(o)))
                        if has_keys(item, &["command", "output"]) =>
                    {
                        Some((c.clone(), o.clone()))
                    }
                    _ => None,
                },
            )
            .collect::<Option<Vec<_>>>(),
        _ => None,
    };
    let Some(gates) = gates else {
        return Err("provenance static gate evidence is invalid".into());
    };
    if gates.iter().map(|(c, _)| c.as_str()).ne(GATE_COMMANDS) {
        return Err("provenance static gate sequence is invalid".into());
    }
    Ok(gates)
}

/// The limitations and native-gate claims are exactly the reviewed ones.
fn check_claims(verification: &Json) -> Result<()> {
    if !has_keys(
        verification,
        &["limitations", "native_gates", "static_gates"],
    ) || !verification
        .get("limitations")
        .is_some_and(|l| py_eq(l, &strings(&LIMITATIONS)))
        || !verification
            .get("native_gates")
            .is_some_and(|n| py_eq(n, &native_gates()))
    {
        return Err("provenance verification claims are invalid".into());
    }
    let native = verification.get("native_gates").expect("checked keys");
    if ["included", "required_before_publication"]
        .iter()
        .any(|k| !matches!(native.get(k), Some(Json::Bool(_))))
    {
        return Err("provenance native gate claims must be Boolean".into());
    }
    Ok(())
}

/// Verify exact inventory, hashes, provenance and optionally every workbook gate.
pub fn verify_bundle(bundle: &Path, root: &Path, check_semantics: bool) -> Result<Json> {
    check_inventory(bundle)?;
    for (name, expected) in parse_checksums(&bundle.join("SHA256SUMS"))? {
        if sha256(&bundle.join(&name))? != expected {
            return Err(format!("checksum mismatch for {name}"));
        }
    }

    let path = bundle.join("provenance.json");
    let document = load_json_object(&path)?;
    if read_bytes(&path)? != canonical_json(&document) {
        return Err("provenance.json is not canonical JSON".into());
    }
    check_provenance_source(&document)?;

    let base = verify_base_workbook(&bundle.join("ozzit.xlsx"), &root.join(BASE_MANIFEST))?;
    let workbook = document.get("workbook").expect("checked keys");
    if !py_eq(workbook, &workbook_evidence(&base)) {
        return Err("provenance workbook evidence does not match the base workbook".into());
    }
    if !matches!(workbook.get("copy_only"), Some(Json::Bool(_))) {
        return Err("provenance workbook copy-only claim is invalid".into());
    }

    let verification = document.get("verification").expect("checked keys");
    let gates = static_gates(verification)?;
    check_claims(verification)?;
    if check_semantics && run_semantic_gates(root, &bundle.join("ozzit.xlsx"))? != gates {
        return Err("semantic gate evidence does not match an independent run".into());
    }
    Ok(document)
}

/// The absolute bundle destination and its parent directory, refusing one
/// inside the repository, one that exists, or a parent that is not a directory.
fn bundle_destination(output: &py::PyPath, root: &Path) -> Result<(py::PyPath, PathBuf)> {
    let output = if output.is_absolute() {
        output.clone()
    } else {
        let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
        py::PyPath::new(&cwd.display().to_string()).join(&output.display())
    };
    let output_path = output.fs();
    if resolve_lenient(&output_path).starts_with(root) {
        return Err("release bundle output must be outside the source repository".into());
    }
    if output_path.symlink_metadata().is_ok() {
        return Err(format!(
            "release bundle destination already exists: {output}"
        ));
    }
    let parent = output.parent().fs();
    if !parent.is_dir() || parent.is_symlink() {
        return Err("release bundle parent must be an existing non-symlink directory".into());
    }
    Ok((output, parent))
}

/// Stage, verify and atomically name one deterministic no-overwrite bundle.
pub fn create_bundle(
    root: &Path,
    workbook: &Path,
    base_manifest: &Path,
    output: &py::PyPath,
    version: &str,
    source_commit: &str,
) -> Result<Json> {
    create_bundle_with(
        root,
        workbook,
        base_manifest,
        output,
        version,
        source_commit,
        (run_semantic_gates, verify_bundle),
    )
}

fn create_bundle_with<G, V>(
    root: &Path,
    workbook: &Path,
    base_manifest: &Path,
    output: &py::PyPath,
    version: &str,
    source_commit: &str,
    checks: (G, V),
) -> Result<Json>
where
    G: FnOnce(&Path, &Path) -> Result<Vec<(String, String)>>,
    V: FnOnce(&Path, &Path, bool) -> Result<Json>,
{
    validate_version(version)?;
    validate_source_commit(source_commit)?;
    let root = py::resolve(root);
    let workbook = py::resolve(workbook);
    let base_manifest = py::resolve(base_manifest);
    let (output, parent) = bundle_destination(output, &root)?;
    let output_path = output.fs();

    let base = verify_base_workbook(&workbook, &base_manifest)?;
    let staging = tempfile::Builder::new()
        .prefix(&format!(".{}-", output.name()))
        .suffix("")
        .tempdir_in(&parent)
        .map_err(|e| io_error(&e, &parent))?;
    let stage = || -> Result<Json> {
        let dir = staging.path();
        let staged = dir.join("ozzit.xlsx");
        write_bytes(&staged, &read_bytes(&workbook)?)?;
        verify_base_workbook(&staged, &base_manifest)?;
        let gates = checks.0(&root, &staged)?;
        let document = provenance(&base, version, source_commit, &gates);
        write_bytes(&dir.join("provenance.json"), &canonical_json(&document))?;
        let checksums = CHECKSUMMED
            .iter()
            .map(|name| Ok((name.to_string(), sha256(&dir.join(name))?)))
            .collect::<Result<Vec<_>>>()?;
        write_bytes(&dir.join("SHA256SUMS"), &checksum_bytes(&checksums))?;
        checks.1(dir, &root, false)?;
        if output_path.symlink_metadata().is_ok() {
            return Err(format!(
                "release bundle destination already exists: {output}"
            ));
        }
        Ok(document)
    };
    let document = stage()?;
    let dir = staging.keep();
    if let Err(e) = std::fs::rename(&dir, &output_path) {
        let _ = std::fs::remove_dir_all(&dir);
        return Err(py::oserror_str(
            &e,
            &dir.display().to_string(),
            py::OsStyle::Native,
        ));
    }
    Ok(document)
}

#[cfg(test)]
mod staging_contracts;

// ---- command line -------------------------------------------------------------

const PROG: &str = "ozzit prepare_release_bundle";
const TOP_USAGE: &str = "usage: ozzit prepare_release_bundle [-h] {create,verify} ...";
const TOP_HELP: &str = "usage: ozzit prepare_release_bundle [-h] {create,verify} ...

Create and independently verify Ozzit's copy-only workbook release bundle. The
current workbook cannot be regenerated byte-for-byte from the disclosed
earlier input. This tool therefore copies the exact tracked workbook, proves
it matches the reviewed base manifest, and writes deterministic verification
evidence. It does not tag, upload or publish anything.

positional arguments:
  {create,verify}
    create         create a new deterministic bundle
    verify         independently verify an existing bundle

options:
  -h, --help       show this help message and exit";
const CREATE_USAGE: &str = "usage: ozzit prepare_release_bundle create [-h] --version VERSION
                                           [--source-commit SOURCE_COMMIT]
                                           --output OUTPUT";
const CREATE_OPTIONS: &str = "options:
  -h, --help            show this help message and exit
  --version VERSION
  --source-commit SOURCE_COMMIT
  --output OUTPUT";
const VERIFY_USAGE: &str = "usage: ozzit prepare_release_bundle verify [-h] --bundle BUNDLE
                                           [--version VERSION]
                                           [--source-commit SOURCE_COMMIT]";
const VERIFY_OPTIONS: &str = "options:
  -h, --help            show this help message and exit
  --bundle BUNDLE
  --version VERSION
  --source-commit SOURCE_COMMIT";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Arguments {
    pub command: String,
    pub version: Option<String>,
    pub source_commit: Option<String>,
    pub output: Option<String>,
    pub bundle: Option<String>,
}

fn usage_error(usage: &str, prog: &str, message: &str) -> Output {
    Output {
        code: 2,
        out: vec![],
        err: vec![format!("{usage}\n{prog}: error: {message}")],
    }
}

/// An argument string argparse reads as an option rather than a value.
fn is_option(arg: &str, options: &[&str]) -> bool {
    if !arg.starts_with('-') || arg == "-" {
        return false;
    }
    let name = arg.split_once('=').map_or(arg, |(n, _)| n);
    if options.contains(&name)
        || options
            .iter()
            .any(|o| o.starts_with("--") && arg.starts_with("--") && o.starts_with(name))
    {
        return true;
    }
    // Python 3.14 reads anything starting with a digit after the dash as a negative number.
    if matches(r"^-\.?\d", arg) {
        return false;
    }
    !arg.contains(' ')
}

/// Match one option string as argparse does, including unique `--` prefixes.
fn match_option(
    arg: &str,
    options: &[&str],
) -> std::result::Result<Option<(String, Option<String>)>, String> {
    if options.contains(&arg) {
        return Ok(Some((arg.into(), None)));
    }
    if let Some((name, value)) = arg.split_once('=')
        && options.contains(&name)
    {
        return Ok(Some((name.into(), Some(value.into()))));
    }
    if arg.starts_with("--") {
        let (prefix, value) = match arg.split_once('=') {
            Some((p, v)) => (p, Some(v.to_string())),
            None => (arg, None),
        };
        let found = options
            .iter()
            .filter(|o| o.starts_with("--") && o.starts_with(prefix))
            .collect::<Vec<_>>();
        return match found.as_slice() {
            [] => Ok(None),
            [one] => Ok(Some((one.to_string(), value))),
            many => Err(format!(
                "ambiguous option: {prefix} could match {}",
                many.iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        };
    }
    if arg.len() > 2 && options.contains(&&arg[..2]) {
        return Ok(Some((arg[..2].into(), Some(arg[2..].into()))));
    }
    Ok(None)
}

fn help_output(text: String) -> Output {
    Output::ok(vec![text])
}

/// Whether `-h` given with an attached value is refused rather than shown.
///
/// `--help=x`, `-h=x` and `-h-x` are refused; `-hx` combines short flags, so
/// help runs first and exits.
fn help_refused(arg: &str, value: &str) -> bool {
    arg.starts_with("--") || arg.as_bytes().get(2) == Some(&b'=') || value.starts_with('-')
}

/// The subcommand, read past any top-level options, and the index after it.
/// Unrecognised top-level options are added to `extras`.
fn parse_command(
    args: &[String],
    extras: &mut Vec<String>,
) -> std::result::Result<(String, usize), Output> {
    let top_options = ["-h", "--help"];
    let mut i = 0;
    let mut literal = false;
    loop {
        let Some(arg) = args.get(i) else {
            return Err(usage_error(
                TOP_USAGE,
                PROG,
                "the following arguments are required: command",
            ));
        };
        i += 1;
        if !literal && arg == "--" {
            literal = true;
            continue;
        }
        if !literal && is_option(arg, &top_options) {
            match match_option(arg, &top_options) {
                Err(message) => return Err(usage_error(TOP_USAGE, PROG, &message)),
                Ok(Some((_, Some(value)))) if help_refused(arg, &value) => {
                    return Err(usage_error(
                        TOP_USAGE,
                        PROG,
                        &format!(
                            "argument -h/--help: ignored explicit argument {}",
                            py::repr_str(&value)
                        ),
                    ));
                }
                Ok(Some(_)) => return Err(help_output(TOP_HELP.into())),
                Ok(None) => extras.push(arg.clone()),
            }
            continue;
        }
        return Ok((arg.clone(), i));
    }
}

/// The subcommand being parsed: its name, usage, options and program name.
struct Sub {
    command: String,
    usage: &'static str,
    options: &'static [&'static str],
    prog: String,
}

impl Sub {
    fn new(command: String) -> std::result::Result<Self, Output> {
        let (usage, options): (&str, &[&str]) = match command.as_str() {
            "create" => (
                CREATE_USAGE,
                &["-h", "--help", "--version", "--source-commit", "--output"],
            ),
            "verify" => (
                VERIFY_USAGE,
                &["-h", "--help", "--bundle", "--version", "--source-commit"],
            ),
            _ => {
                return Err(usage_error(
                    TOP_USAGE,
                    PROG,
                    &format!(
                        "argument command: invalid choice: {} (choose from 'create', 'verify')",
                        py::repr_str(&command)
                    ),
                ));
            }
        };
        let prog = format!("{PROG} {command}");
        Ok(Self {
            command,
            usage,
            options,
            prog,
        })
    }

    fn error(&self, message: &str) -> Output {
        usage_error(self.usage, &self.prog, message)
    }

    /// The help text, or the error for a help option given a refused value.
    fn help(&self, arg: &str, explicit: Option<String>) -> Output {
        if let Some(value) = explicit.filter(|v| help_refused(arg, v)) {
            return self.error(&format!(
                "argument -h/--help: ignored explicit argument {}",
                py::repr_str(&value)
            ));
        }
        let options_text = if self.command == "create" {
            CREATE_OPTIONS
        } else {
            VERIFY_OPTIONS
        };
        help_output(format!("{}\n\n{options_text}", self.usage))
    }

    /// The value of option `name`: attached, or the next argument, advancing `i`.
    fn value(
        &self,
        args: &[String],
        i: &mut usize,
        name: &str,
        explicit: Option<String>,
    ) -> std::result::Result<String, Output> {
        if let Some(value) = explicit {
            return Ok(value);
        }
        match args.get(*i) {
            Some(next) if next != "--" && !is_option(next, self.options) => {
                *i += 1;
                Ok(next.clone())
            }
            _ => Err(self.error(&format!("argument {name}: expected one argument"))),
        }
    }

    /// Reads the subcommand's options from `args[i..]` into `parsed`;
    /// returns the arguments it does not recognise.
    fn options(
        &self,
        args: &[String],
        mut i: usize,
        parsed: &mut Arguments,
    ) -> std::result::Result<Vec<String>, Output> {
        // A `--` before the subcommand is consumed with it.
        let mut literal = false;
        let mut sub_extras = Vec::new();
        while let Some(arg) = args.get(i) {
            i += 1;
            if literal || arg == "--" {
                // argparse keeps the first `--` among unrecognised arguments.
                literal = true;
                sub_extras.push(arg.clone());
                continue;
            }
            if !is_option(arg, self.options) {
                sub_extras.push(arg.clone());
                continue;
            }
            let (name, explicit) = match match_option(arg, self.options) {
                Err(message) => return Err(self.error(&message)),
                Ok(None) => {
                    sub_extras.push(arg.clone());
                    continue;
                }
                Ok(Some(found)) => found,
            };
            if name == "-h" || name == "--help" {
                return Err(self.help(arg, explicit));
            }
            let value = self.value(args, &mut i, &name, explicit)?;
            let slot = match name.as_str() {
                "--version" => &mut parsed.version,
                "--source-commit" => &mut parsed.source_commit,
                "--output" => &mut parsed.output,
                _ => &mut parsed.bundle,
            };
            *slot = Some(value);
        }
        Ok(sub_extras)
    }

    fn check_required(&self, parsed: &Arguments) -> std::result::Result<(), Output> {
        let required: &[(&str, bool)] = if self.command == "create" {
            &[
                ("--version", parsed.version.is_some()),
                ("--output", parsed.output.is_some()),
            ]
        } else {
            &[("--bundle", parsed.bundle.is_some())]
        };
        let missing = required
            .iter()
            .filter(|(_, present)| !present)
            .map(|(name, _)| *name)
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(self.error(&format!(
                "the following arguments are required: {}",
                missing.join(", ")
            )));
        }
        Ok(())
    }
}

pub fn parse(args: &[String]) -> std::result::Result<Arguments, Output> {
    let mut extras: Vec<String> = Vec::new();
    let (command, i) = parse_command(args, &mut extras)?;
    let sub = Sub::new(command)?;
    let mut parsed = Arguments {
        command: sub.command.clone(),
        ..Arguments::default()
    };
    let sub_extras = sub.options(args, i, &mut parsed)?;
    sub.check_required(&parsed)?;
    extras.extend(sub_extras);
    if !extras.is_empty() {
        return Err(usage_error(
            TOP_USAGE,
            PROG,
            &format!("unrecognized arguments: {}", extras.join(" ")),
        ));
    }
    Ok(parsed)
}

fn ensure_clean_checkout(root: &Path) -> Result<()> {
    if !git_output(root, &["status", "--porcelain", "--untracked-files=all"])?.is_empty() {
        return Err("candidate checkout must be clean before staging a release bundle".into());
    }
    Ok(())
}

fn current_commit(root: &Path) -> Result<String> {
    validate_source_commit(&git_output(root, &["rev-parse", "--verify", "HEAD"])?)
}

fn run_create(arguments: &Arguments, root: &Path, head: String) -> Result<String> {
    let source_commit = match &arguments.source_commit {
        None => head.clone(),
        Some(requested) => validate_source_commit(requested)?,
    };
    if source_commit != head {
        return Err("source commit must equal the candidate checkout HEAD".into());
    }
    let output = py::PyPath::new(arguments.output.as_deref().expect("required"));
    let version = arguments.version.as_deref().expect("required");
    let document = create_bundle(
        root,
        &root.join("ozzit.xlsx"),
        &root.join(BASE_MANIFEST),
        &output,
        version,
        &source_commit,
    )?;
    let version = document
        .get("version")
        .and_then(Json::as_str)
        .unwrap_or_default();
    Ok(format!("OK: created Ozzit {version} bundle at {output}"))
}

fn run_verify(arguments: &Arguments, root: &Path, head: &str) -> Result<String> {
    let bundle = py::PyPath::new(arguments.bundle.as_deref().expect("required"));
    let document = verify_bundle(&bundle.fs(), root, true)?;
    let source = document
        .get("source")
        .and_then(|s| s.get("commit"))
        .and_then(Json::as_str);
    if source != Some(head) {
        return Err("bundle source commit does not match the candidate checkout HEAD".into());
    }
    let version = document
        .get("version")
        .and_then(Json::as_str)
        .unwrap_or_default();
    if let Some(requested) = &arguments.version
        && version != validate_version(requested)?
    {
        return Err("bundle version does not match --version".into());
    }
    if let Some(requested) = &arguments.source_commit
        && source != Some(validate_source_commit(requested)?.as_str())
    {
        return Err("bundle source commit does not match --source-commit".into());
    }
    Ok(format!("OK: verified Ozzit {version} bundle at {bundle}"))
}

fn run(arguments: &Arguments) -> Result<String> {
    let root = py::resolve(&cache::repository_root(BASE_MANIFEST));
    ensure_clean_checkout(&root)?;
    let head = current_commit(&root)?;
    let base = verify_base_workbook(&root.join("ozzit.xlsx"), &root.join(BASE_MANIFEST))?;
    verify_repository_base(&root, &base)?;
    if arguments.command == "create" {
        run_create(arguments, &root, head)
    } else {
        run_verify(arguments, &root, &head)
    }
}

pub fn command(args: &[String]) -> Output {
    let arguments = match parse(args) {
        Ok(arguments) => arguments,
        Err(output) => return output,
    };
    match run(&arguments) {
        Ok(line) => Output::ok(vec![line]),
        Err(e) => Output::stderr(format!("ERROR: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha1_matches_the_standard_vectors() {
        assert_eq!(sha1_hex(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(sha1_hex(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(
            sha1_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
        );
        assert_eq!(
            sha1_hex(&vec![b'a'; 1_000_000]),
            "34aa973cd4c4daa4f61eeb2bdbad27316534016f"
        );
    }

    #[test]
    fn json_equality_follows_python() {
        let j = |t: &str| pyjson::loads(t).unwrap();
        assert!(py_eq(&j("1"), &j("1.0")));
        assert!(py_eq(&j("true"), &j("1")));
        assert!(py_eq(
            &j("{\"a\":1,\"b\":[false]}"),
            &j("{\"b\":[0],\"a\":1.0}")
        ));
        assert!(!py_eq(&j("9007199254740993"), &j("9007199254740992.0")));
        assert!(!py_eq(&j("NaN"), &j("NaN")));
        assert!(!py_eq(&j("\"1\""), &j("1")));
        assert!(!py_eq(&j("{\"a\":1}"), &j("{\"a\":1,\"b\":2}")));
    }
}
