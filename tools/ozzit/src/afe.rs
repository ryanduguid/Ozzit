//! The AFE JSON store is a publication view, not a formula engine.
use crate::{
    archive::*,
    common::*,
    compiler, formula, py,
    pyjson::{self, Json},
};
use base64::{
    Engine, alphabet,
    engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig},
};
use num_bigint::ToBigInt;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

pub const SCHEMA: &str =
    "http://schemas.advancedformulaenvironment.officeapps.live.com/afeprojects/0.2";
const PART: &str = "customXml/item1.xml";

pub fn find_blob(raw: &[u8]) -> Result<&[u8]> {
    let valid = |b: u8| b.is_ascii_alphanumeric() || b == b'+' || b == b'/';
    let (mut i, mut best) = (0, None);
    while i < raw.len() {
        while i < raw.len() && !valid(raw[i]) {
            i += 1;
        }
        let start = i;
        while i < raw.len() && valid(raw[i]) {
            i += 1;
        }
        if i - start >= 100 {
            for _ in 0..2 {
                if raw.get(i) == Some(&b'=') {
                    i += 1;
                }
            }
            let candidate = &raw[start..i];
            if best.is_none_or(|b: &[u8]| candidate.len() > b.len()) {
                best = Some(candidate);
            }
        }
    }
    best.ok_or_else(|| "AFE store has no base64 payload".into())
}

fn base64_decode(encoded: &[u8]) -> Result<Vec<u8>> {
    let data = encoded
        .iter()
        .rposition(|b| *b != b'=')
        .map_or(0, |i| i + 1);
    if data % 4 == 1 {
        return Err(format!(
            "Invalid base64-encoded string: number of data characters ({data}) cannot be 1 more than a multiple of 4"
        ));
    }
    let padding = encoded.len() - data;
    if data % 4 != 0 && padding < 4 - data % 4 {
        return Err("Incorrect padding".into());
    }
    GeneralPurpose::new(
        &alphabet::STANDARD,
        GeneralPurposeConfig::new()
            .with_decode_allow_trailing_bits(true)
            .with_decode_padding_mode(DecodePaddingMode::RequireNone),
    )
    .decode(&encoded[..data])
    .map_err(|e| e.to_string())
}

pub fn utf16le(bytes: &[u8]) -> Result<String> {
    let error = |start: usize, end: usize, reason: &str| {
        let location = if end == start + 1 {
            format!("byte 0x{:02x} in position {start}", bytes[start])
        } else {
            format!("bytes in position {start}-{}", end - 1)
        };
        format!("'utf-16-le' codec can't decode {location}: {reason}")
    };
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 == bytes.len() {
            return Err(error(i, i + 1, "truncated data"));
        }
        let first = u16::from_le_bytes([bytes[i], bytes[i + 1]]);
        if (0xd800..=0xdbff).contains(&first) {
            if i + 3 >= bytes.len() {
                return Err(error(i, bytes.len(), "unexpected end of data"));
            }
            let second = u16::from_le_bytes([bytes[i + 2], bytes[i + 3]]);
            if !(0xdc00..=0xdfff).contains(&second) {
                return Err(error(i, i + 2, "illegal UTF-16 surrogate"));
            }
            let scalar = 0x10000 + ((u32::from(first) - 0xd800) << 10) + u32::from(second) - 0xdc00;
            out.push(char::from_u32(scalar).expect("paired UTF-16 surrogate"));
            i += 4;
        } else if (0xdc00..=0xdfff).contains(&first) {
            return Err(error(i, i + 2, "illegal encoding"));
        } else {
            out.push(char::from_u32(u32::from(first)).expect("non-surrogate UTF-16 unit"));
            i += 2;
        }
    }
    Ok(out)
}

fn decode(encoded: &[u8]) -> Result<Json> {
    pyjson::loads(&utf16le(&base64_decode(encoded)?)?).map_err(|e| e.to_string())
}

pub fn base64_decode_strict(encoded: &[u8]) -> Result<Vec<u8>> {
    if encoded.first() == Some(&b'=') {
        return Err("Leading padding not allowed".into());
    }
    let data = encoded
        .iter()
        .position(|b| *b == b'=')
        .unwrap_or(encoded.len());
    if encoded[..data]
        .iter()
        .any(|b| !b.is_ascii_alphanumeric() && *b != b'+' && *b != b'/')
    {
        return Err("Only base64 data is allowed".into());
    }
    if data % 4 == 1 {
        return Err(format!(
            "Invalid base64-encoded string: number of data characters ({data}) cannot be 1 more than a multiple of 4"
        ));
    }
    let padding = encoded[data..].iter().take_while(|b| **b == b'=').count();
    let required = (4 - data % 4) % 4;
    if padding > required {
        return Err("Excess padding not allowed".into());
    }
    if let Some(next) = encoded.get(data + padding)
        && !next.is_ascii_alphanumeric()
        && *next != b'+'
        && *next != b'/'
    {
        return Err("Only base64 data is allowed".into());
    }
    if data + padding != encoded.len() {
        return Err(if padding < required {
            "Discontinuous padding not allowed"
        } else {
            "Excess data after padding"
        }
        .into());
    }
    base64_decode(encoded)
}
pub fn decode_strict_blob(encoded: &[u8]) -> Result<Json> {
    pyjson::loads(&utf16le(&base64_decode_strict(encoded)?)?).map_err(|e| e.to_string())
}

pub fn decode_store(workbook: &Path) -> Result<Json> {
    let parts = read_parts(workbook).map_err(|e| format!("cannot read AFE store: {e}"))?;
    let raw = parts.get(PART).ok_or_else(|| {
        format!("cannot read AFE store: \"There is no item named '{PART}' in the archive\"")
    })?;
    let encoded = find_blob(raw)?;
    let store = decode(encoded).map_err(|e| format!("cannot decode AFE store: {e}"))?;
    if !matches!(store, Json::Object(_)) {
        return Err("cannot decode AFE store: root must be an object".into());
    }
    if store.get("schema").and_then(Json::as_str).is_none() {
        return Err("cannot decode AFE store: schema must be a string".into());
    }
    let Some(Json::Array(files)) = store.get("files") else {
        return Err("cannot decode AFE store: files must be a list".into());
    };
    for (i, file) in files.iter().enumerate() {
        if !matches!(file, Json::Object(_)) {
            return Err(format!(
                "cannot decode AFE store: files[{i}] must be an object"
            ));
        }
        if file.get("path").and_then(Json::as_str).is_none()
            || file.get("text").and_then(Json::as_str).is_none()
        {
            return Err(format!(
                "cannot decode AFE store: files[{i}] path and text must be strings"
            ));
        }
    }
    if !matches!(store.get("projectNames"), Some(Json::Array(names)) if names.iter().all(|n| matches!(n, Json::Str(_))))
    {
        return Err("cannot decode AFE store: projectNames must be a list of strings".into());
    }
    Ok(store)
}

fn duplicates(label: &str, values: Vec<&str>, failures: &mut Vec<String>) {
    let mut counts = BTreeMap::new();
    for value in values {
        *counts.entry(value).or_insert(0) += 1;
    }
    for (value, count) in counts {
        if count > 1 {
            failures.push(format!("AFE {label} {value} appears {count} times"));
        }
    }
}

fn compare_module(
    module: &str,
    files: &BTreeMap<&str, &str>,
    src: &Path,
    failures: &mut Vec<String>,
) {
    let path = format!("/projects/{module}");
    let expected_path = src.join(format!("{module}.txt"));
    let Some(actual) = files.get(path.as_str()) else {
        failures.push(format!("AFE module {path} missing"));
        return;
    };
    let expected = match py::read_text(&expected_path) {
        Ok(expected) => expected,
        Err(py::ReadError::Os(e)) if e.kind() == std::io::ErrorKind::NotFound => {
            failures.push(format!("source module {} missing", display(&expected_path)));
            return;
        }
        Err(e) => {
            let reason = match e {
                py::ReadError::Os(e) => io_error(&e, &expected_path),
                py::ReadError::Decode(e) => e.to_string(),
            };
            failures.push(format!(
                "cannot read source module {}: {reason}",
                display(&expected_path)
            ));
            return;
        }
    };
    if *actual != expected {
        let first = actual
            .chars()
            .zip(expected.chars())
            .position(|(a, b)| a != b)
            .unwrap_or(actual.chars().count().min(expected.chars().count()));
        let a = char_slice(actual, first.saturating_sub(30), first + 50).replace('\n', " ");
        let b = char_slice(&expected, first.saturating_sub(30), first + 50).replace('\n', " ");
        failures.push(format!(
            "AFE module {path} differs from src/{module}.txt at character {first}: {} != {}",
            py::repr_str(&a),
            py::repr_str(&b)
        ));
    }
}

/// The (path, text) files and the project names of a store `decode_store` validated.
fn store_entries(store: &Json) -> (Vec<(&str, &str)>, Vec<&str>) {
    let Some(Json::Array(files)) = store.get("files") else {
        unreachable!()
    };
    let Some(Json::Array(names)) = store.get("projectNames") else {
        unreachable!()
    };
    let names = names
        .iter()
        .map(|n| n.as_str().expect("validated project name"))
        .collect::<Vec<_>>();
    let files = files
        .iter()
        .map(|file| {
            (
                file.get("path")
                    .and_then(Json::as_str)
                    .expect("validated path"),
                file.get("text")
                    .and_then(Json::as_str)
                    .expect("validated text"),
            )
        })
        .collect::<Vec<_>>();
    (files, names)
}

pub fn check(workbook: &Path, src: &Path) -> Vec<String> {
    let store = match decode_store(workbook) {
        Ok(store) => store,
        Err(e) => return vec![e],
    };
    let mut failures = Vec::new();
    let schema = store
        .get("schema")
        .and_then(Json::as_str)
        .expect("validated schema");
    if schema != SCHEMA {
        failures.push(format!("AFE schema is {}", py::repr_str(schema)));
    }
    let (files, names) = store_entries(&store);
    duplicates(
        "file path",
        files.iter().map(|(path, _)| *path).collect(),
        &mut failures,
    );
    duplicates("projectNames entry", names.clone(), &mut failures);
    let files = files.into_iter().collect::<BTreeMap<_, _>>();
    for module in formula::MODULES {
        compare_module(module, &files, src, &mut failures);
    }
    let expected = match compiler::workbook_names(workbook) {
        Ok(names) => names,
        Err(e) => {
            failures.push(e);
            return failures;
        }
    };
    let actual = names
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    for name in expected.difference(&actual) {
        failures.push(format!("AFE projectNames is missing shipped name {name}"));
    }
    for name in actual.difference(&expected) {
        failures.push(format!("AFE projectNames has unknown name {name}"));
    }
    failures
}

pub fn verify(args: &[String]) -> Output {
    if args.len() > 2 {
        return Output::stderr("FAIL: usage: ozzit verify_afe [workbook] [src dir]");
    }
    let workbook = args.first().map_or("ozzit.xlsx", String::as_str);
    let src = args.get(1).map_or("src", String::as_str);
    let failures = check(Path::new(workbook), Path::new(src));
    let bad = format!("FAIL: {} AFE problem(s) in {workbook}", failures.len());
    Output::report(
        failures,
        bad,
        format!(
            "OK: AFE store in {workbook} matches all {} modules and every shipped name",
            formula::MODULES.len()
        ),
    )
}

pub fn index_order(
    current: &[String],
    expected: &BTreeSet<String>,
    owner: &BTreeMap<String, String>,
) -> Vec<String> {
    let mut kept = current
        .iter()
        .filter(|n| expected.contains(*n))
        .cloned()
        .collect::<Vec<_>>();
    let mut added = expected
        .iter()
        .filter(|n| !kept.contains(n))
        .cloned()
        .collect::<Vec<_>>();
    added.sort_by_key(|n| n.to_lowercase());
    for name in added {
        let at = kept
            .iter()
            .rposition(|held| owner.get(held) == owner.get(&name))
            .map_or(kept.len(), |i| i + 1);
        kept.insert(at, name);
    }
    kept
}

fn replace_bytes(raw: &[u8], old: &[u8], new: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut remaining = raw;
    while let Some(at) = remaining.windows(old.len()).position(|part| part == old) {
        out.extend_from_slice(&remaining[..at]);
        out.extend_from_slice(new);
        remaining = &remaining[at + old.len()..];
    }
    out.extend_from_slice(remaining);
    out
}

fn set_object(object: &mut Json, key: &str, value: Json) -> Result<()> {
    let Json::Object(pairs) = object else {
        return Err("AFE store root must be an object".into());
    };
    if let Some((_, held)) = pairs.iter_mut().find(|(k, _)| k == key) {
        *held = value;
    } else {
        pairs.push((key.into(), value));
    }
    Ok(())
}

fn hashable_key(value: &Json) -> Result<(&'static str, String)> {
    Ok(match value {
        Json::Null => ("none", String::new()),
        Json::Bool(b) => ("number", u8::from(*b).to_string()),
        Json::Int(i) => ("number", i.to_string()),
        Json::Float(f) if f.is_finite() && f.fract() == 0.0 => (
            "number",
            f.to_bigint().expect("finite integral float").to_string(),
        ),
        Json::Float(f) => ("float", py::float_repr(*f)),
        Json::Str(s) => ("str", s.clone()),
        other => return Err(format!("unhashable type: '{}'", other.type_name())),
    })
}

pub fn sync(workbook: &Path, src: &Path) -> Result<Vec<String>> {
    let mut parts = read_parts(workbook)?;
    let raw = parts
        .get(PART)
        .ok_or_else(|| format!("\"There is no item named '{PART}' in the archive\""))?;
    let encoded = find_blob(raw)?;
    let mut store = decode(encoded)?;
    let mut changes = Vec::new();
    let Some(Json::Array(files)) = store.get_mut("files") else {
        return Err("'files'".into());
    };
    sync_modules(files, src, &mut changes)?;
    sync_project_names(&mut store, workbook, src, &mut changes)?;
    let bytes = pyjson::dumps_compact(&store)
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let replacement = base64::engine::general_purpose::STANDARD.encode(bytes);
    if replacement.as_bytes() == encoded && changes.is_empty() {
        return Ok(vec!["AFE store already matches src/".into()]);
    }
    let new = replace_bytes(raw, encoded, replacement.as_bytes());
    parts.insert(PART.into(), new);
    write_deterministic(workbook, &parts)?;
    Ok(changes)
}

fn sync_modules(files: &mut Vec<Json>, src: &Path, changes: &mut Vec<String>) -> Result<()> {
    for file in files.iter() {
        if !matches!(file, Json::Object(_)) {
            return Err(format!(
                "'{}' object has no attribute 'get'",
                file.type_name()
            ));
        }
        if let Some(path) = file.get("path") {
            hashable_key(path)?;
        }
    }
    for module in formula::MODULES {
        let expected = read_text(&src.join(format!("{module}.txt")))?;
        let path = format!("/projects/{module}");
        if let Some(i) = files
            .iter()
            .rposition(|file| file.get("path").and_then(Json::as_str) == Some(path.as_str()))
        {
            if files[i].get("text").and_then(Json::as_str) != Some(expected.as_str()) {
                set_object(&mut files[i], "text", Json::Str(expected))?;
                changes.push(format!("synced AFE module {path}"));
            }
        } else {
            files.push(Json::Object(vec![
                ("path".into(), Json::Str(path.clone())),
                ("text".into(), Json::Str(expected)),
            ]));
            changes.push(format!("added AFE module {path}"));
        }
    }
    Ok(())
}

fn sync_project_names(
    store: &mut Json,
    workbook: &Path,
    src: &Path,
    changes: &mut Vec<String>,
) -> Result<()> {
    let shipped = compiler::workbook_names(workbook)?;
    let current = match store.get("projectNames") {
        Some(Json::Array(names)) => names.clone(),
        Some(Json::Str(names)) => names.chars().map(|c| Json::Str(c.to_string())).collect(),
        Some(Json::Object(names)) => names.iter().map(|(k, _)| Json::Str(k.clone())).collect(),
        None => vec![],
        Some(other) => return Err(format!("'{}' object is not iterable", other.type_name())),
    };
    let before = current
        .iter()
        .map(hashable_key)
        .collect::<Result<BTreeSet<_>>>()?;
    let current_names = current
        .iter()
        .filter_map(Json::as_str)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let wanted = index_order(&current_names, &shipped, &compiler::source_modules(src)?);
    let wanted = wanted.into_iter().map(Json::Str).collect::<Vec<_>>();
    if wanted != current {
        let after = wanted
            .iter()
            .map(hashable_key)
            .collect::<Result<BTreeSet<_>>>()?;
        changes.push(format!(
            "synced AFE projectNames: {} added, {} dropped, {} listed",
            after.difference(&before).count(),
            before.difference(&after).count(),
            wanted.len()
        ));
        set_object(store, "projectNames", Json::Array(wanted))?;
    }
    Ok(())
}

pub fn sync_command(args: &[String]) -> Output {
    if args.len() > 2 {
        return Output::stderr("FAIL: usage: ozzit sync_afe_store [workbook] [src dir]");
    }
    let workbook = Path::new(args.first().map_or("ozzit.xlsx", String::as_str));
    let src = Path::new(args.get(1).map_or("src", String::as_str));
    if !workbook.is_file() {
        return Output::stderr(format!("FAIL: no such workbook: {}", display(workbook)));
    }
    let mut changes = match sync(workbook, src) {
        Ok(changes) => changes,
        Err(e) => return Output::stderr(format!("FAIL: cannot sync AFE store: {e}")),
    };
    let size = match std::fs::metadata(workbook) {
        Ok(meta) => meta.len(),
        Err(e) => return Output::stderr(io_error(&e, workbook)),
    };
    let size = size.to_string();
    let size = size
        .chars()
        .enumerate()
        .flat_map(|(i, c)| {
            if i > 0 && (size.len() - i).is_multiple_of(3) {
                vec![',', c]
            } else {
                vec![c]
            }
        })
        .collect::<String>();
    changes.push(format!(
        "OK: {} AFE store synced, {size} bytes",
        display(workbook)
    ));
    Output::ok(changes)
}
