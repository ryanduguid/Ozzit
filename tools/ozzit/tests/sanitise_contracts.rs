//! Replay the recorded CLI results of tools/sanitise_workbook.py and
//! tools/polish_workbook.py, and the refresh
//! command's handling of each Excel refresh result.
use base64::Engine;
use ozzit::{cache::Dump, polish, py::PyPath, refresh, sanitise};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn part_hashes(bytes: &[u8]) -> Option<serde_json::Map<String, Value>> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).ok()?;
    let mut out = serde_json::Map::new();
    for i in 0..zip.len() {
        let mut member = zip.by_index(i).ok()?;
        let mut data = Vec::new();
        member.read_to_end(&mut data).ok()?;
        out.insert(member.name().ok()?.into_owned(), Value::String(hex(&data)));
    }
    Some(out)
}

fn printed(lines: &[String]) -> String {
    lines.iter().map(|l| format!("{l}\n")).collect()
}

fn corpus(name: &str) -> Value {
    let bytes = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data")
            .join(name),
    )
    .unwrap();
    serde_json::from_reader(flate2::read::GzDecoder::new(&bytes[..])).unwrap()
}

/// Replay a writer's recorded CLI results: output text, result bytes and a second run.
fn replay(
    corpus_name: &str,
    cases: usize,
    tool: &str,
    command: fn(&[String]) -> ozzit::common::Output,
) {
    let data = corpus(corpus_name);
    let records = data["records"].as_array().unwrap();
    assert_eq!(records.len(), cases);
    let mut differences = Vec::new();
    for record in records {
        let name = record["name"].as_str().unwrap();
        let expected = &record["expected"];
        let dir = tempfile::tempdir().unwrap();
        let path_name = record["path"].as_str().unwrap_or("book.xlsx");
        let path = dir.path().join(path_name);
        if record["directory"].as_bool().unwrap() {
            std::fs::create_dir(&path).unwrap();
        } else if let Some(id) = record["input"].as_str() {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(data["blobs"][id].as_str().unwrap())
                .unwrap();
            std::fs::write(&path, bytes).unwrap();
        }
        let shown = path.display().to_string();
        let args = match record["args"].as_array() {
            Some(args) => args
                .iter()
                .map(|a| a.as_str().unwrap().to_string())
                .collect(),
            None => vec![shown.clone()],
        };
        let run = || {
            let output = command(&args);
            let hide = |lines: &[String]| {
                printed(lines)
                    .replace(&shown, path_name)
                    .replace(&format!("ozzit {tool}"), &format!("python tools/{tool}.py"))
            };
            (output.code, hide(&output.out), hide(&output.err))
        };
        let (code, out, err) = run();
        let mut same = i64::from(code) == expected["code"].as_i64().unwrap()
            && out == expected["stdout"].as_str().unwrap()
            && err == expected["stderr"].as_str().unwrap();
        // Only the native ZIP reason differs; the refusal, stream and exit are kept.
        if !same && let Some((prefix, _)) = err.split_once(": invalid ZIP: ") {
            same = code == 1
                && out.is_empty()
                && expected["stderr"].as_str().unwrap()
                    == format!("{prefix}: File is not a zip file\n");
        }
        if !same {
            differences.push(format!(
                "{name}: got {code} {out:?} {err:?}, expected {expected}"
            ));
            continue;
        }
        if let Some(sha) = expected.get("sha256").and_then(Value::as_str) {
            let bytes = std::fs::read(&path).unwrap();
            if hex(&bytes) != sha {
                differences.push(format!(
                    "{name}: result bytes differ (parts same: {})",
                    part_hashes(&bytes).map(Value::Object).as_ref() == Some(&expected["parts"])
                ));
            }
            if let Some(second) = expected["second"].as_str() {
                let (_, again, _) = run();
                if again != second {
                    differences.push(format!("{name}: second run {again:?} != {second:?}"));
                }
            }
        }
    }
    assert!(
        differences.is_empty(),
        "{} differences:\n{}",
        differences.len(),
        differences.join("\n")
    );
}

#[test]
fn sanitise_results_match_the_python_tool() {
    replay(
        "sanitise-contracts.json.gz",
        33,
        "sanitise_workbook",
        sanitise::command,
    );
}

#[test]
fn polish_results_match_the_python_tool() {
    replay(
        "polish-contracts.json.gz",
        31,
        "polish_workbook",
        polish::command,
    );
}

#[test]
fn fold_longtext_joins_split_literals_and_keeps_escaped_quotes() {
    let (text, count) =
        sanitise::fold_longtext(r#"=_xlfn._LONGTEXT("ab","c""d")&_xlfn._LONGTEXT("x")"#);
    assert_eq!(count, 1);
    assert_eq!(text, r#"="abc""d"&_xlfn._LONGTEXT("x")"#);
}

#[test]
fn thousands_separators_match_python_format() {
    for (n, shown) in [
        (0, "0"),
        (999, "999"),
        (1000, "1,000"),
        (451_234, "451,234"),
        (1_234_567, "1,234,567"),
    ] {
        assert_eq!(sanitise::thousands(n), shown);
    }
}

fn excel_refresh(code: i64, stdout: &str, stderr: &str) -> Dump {
    Dump::Exited {
        code,
        stdout: stdout.into(),
        stderr: stderr.into(),
    }
}

#[test]
fn refresh_reports_each_excel_result_as_the_python_tool_does() {
    let book = PyPath::new("missing-dir/ozzit.xlsx");
    let lines = |dump| refresh::outcome(&book, Ok(dump));
    let output = lines(Dump::NotFound);
    assert_eq!(
        (output.code, output.out),
        (
            1,
            vec!["FAIL: powershell.exe was not found, so Excel could not be driven".to_string()]
        )
    );
    let output = lines(Dump::TimedOut);
    assert_eq!(output.code, 1);
    assert_eq!(output.out, vec!["FAIL: the Excel refresh did not finish within 600s; check for a modal dialog and close any Excel process before retrying".to_string()]);
    let output = lines(excel_refresh(
        2,
        "Excel is already running. Close it first.\n",
        "",
    ));
    assert_eq!(output.code, 1);
    assert_eq!(
        output.out,
        vec![
            "Excel is already running. Close it first.".to_string(),
            "FAIL: the Excel refresh did not complete (exit 2)".into()
        ]
    );
    let output = lines(excel_refresh(
        1,
        "  \n",
        &format!(" {}\n", "é".repeat(2500)),
    ));
    assert_eq!(output.out.len(), 2);
    assert_eq!(output.out[1].chars().count(), 2000);
    // A refresh that completed but left no readable workbook fails at the sanitiser.
    let output = lines(excel_refresh(0, "refreshed ozzit.xlsx: 1 -> 2 bytes\n", ""));
    assert_eq!(output.code, 1);
    assert_eq!(output.out[0], "refreshed ozzit.xlsx: 1 -> 2 bytes");
    assert!(output.out[1].starts_with(&format!("FAIL: cannot sanitise {book}: ")));
}

#[test]
fn refresh_sanitises_the_saved_workbook_and_reports_ok() {
    let data = corpus("sanitise-contracts.json.gz");
    let record = &data["records"][0];
    assert_eq!(record["name"], "full Excel save");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("book.xlsx");
    std::fs::write(
        &path,
        base64::engine::general_purpose::STANDARD
            .decode(
                data["blobs"][record["input"].as_str().unwrap()]
                    .as_str()
                    .unwrap(),
            )
            .unwrap(),
    )
    .unwrap();
    let book = PyPath::new(&path.display().to_string());
    let output = refresh::outcome(
        &book,
        Ok(excel_refresh(
            0,
            "refreshed book.xlsx: 1 -> 2 bytes\r\n",
            "",
        )),
    );
    assert_eq!(output.code, 0);
    let mut expected = vec!["refreshed book.xlsx: 1 -> 2 bytes".to_string()];
    let python = record["expected"]["stdout"].as_str().unwrap();
    expected.extend(
        python
            .lines()
            .filter(|l| !l.starts_with("OK: "))
            .map(String::from),
    );
    expected.push(format!(
        "OK: {book} holds the values its own formulas produce"
    ));
    assert_eq!(output.out, expected);
    assert_eq!(
        hex(&std::fs::read(&path).unwrap()),
        record["expected"]["sha256"].as_str().unwrap()
    );
}
