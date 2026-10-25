//! Replay tools/verify_cache.py's recorded findings for generated workbooks, dumps
//! and dumper results. Excel is not run: the dumper's result is part of each case.
use base64::Engine;
use ozzit::cache::{self, CellValue, Dump};
use serde_json::Value;
use std::path::Path;

fn blob(data: &Value, id: &Value) -> Vec<u8> {
    base64::engine::general_purpose::STANDARD
        .decode(data["blobs"][id.as_str().unwrap()].as_str().unwrap())
        .unwrap()
}

fn dump_bytes(data: &Value, spec: &Value) -> Option<Vec<u8>> {
    if spec.is_null() {
        return None;
    }
    if let Some(id) = spec.get("blob") {
        return Some(blob(data, id));
    }
    let base = String::from_utf8(blob(data, &spec["base"])).unwrap();
    let mut rows = base
        .strip_suffix('\n')
        .unwrap()
        .split('\n')
        .map(|r| Some(r.to_string()))
        .collect::<Vec<_>>();
    for edit in spec["edits"].as_array().unwrap() {
        rows[edit[0].as_u64().unwrap() as usize] = edit[1].as_str().map(String::from);
    }
    let mut rows = rows.into_iter().flatten().collect::<Vec<_>>();
    rows.extend(
        spec["append"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r.as_str().unwrap().to_string()),
    );
    Some(format!("{}\n", rows.join("\n")).into_bytes())
}

fn dumper(result: &Value) -> Dump {
    match result["kind"].as_str().unwrap() {
        "notfound" => Dump::NotFound,
        "timeout" => Dump::TimedOut,
        _ => Dump::Exited {
            code: result["code"].as_i64().unwrap(),
            stdout: result["stdout"].as_str().unwrap().into(),
            stderr: result["stderr"].as_str().unwrap().into(),
        },
    }
}

fn printed(lines: &[String]) -> String {
    lines.iter().map(|l| format!("{l}\n")).collect()
}

/// Runs one recorded case; returns the output and its stdout and stderr
/// with the temporary paths hidden.
fn run_record(data: &Value, record: &Value) -> (ozzit::common::Output, String, String) {
    let dir = tempfile::tempdir().unwrap();
    let workbook = dir.path().join("book.xlsx");
    let dump_path = dir.path().join("dump.tsv");
    let shown = workbook.display().to_string();
    let output = if record["workbook"].is_null() {
        cache::verify(std::slice::from_ref(&shown))
    } else {
        std::fs::write(&workbook, blob(data, &record["workbook"])).unwrap();
        if let Some(dump) = dump_bytes(data, &record["dump"]) {
            std::fs::write(&dump_path, dump).unwrap();
        }
        cache::outcome(&shown, Ok(dumper(&record["result"])), &dump_path)
    };
    let dump_repr = ozzit::py::repr_str(&dump_path.display().to_string());
    let hide = |text: String| {
        text.replace(&dump_repr[1..dump_repr.len() - 1], "<dump>")
            .replace(&shown, "<workbook>")
    };
    let out = hide(printed(&output.out));
    let err = hide(printed(&output.err));
    (output, out, err)
}

#[test]
fn verify_cache_findings_match_the_python_tool() {
    let bytes = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/cache-contracts.json.gz"),
    )
    .unwrap();
    let data: Value = serde_json::from_reader(flate2::read::GzDecoder::new(&bytes[..])).unwrap();
    let records = cache_records(&data);
    let mut differences = Vec::new();
    let mut tracebacks = 0;
    for record in records {
        let name = record["name"].as_str().unwrap();
        let (output, out, err) = run_record(&data, record);
        let expected = &record["expected"];
        if expected["traceback"].is_string() {
            // The Python tool ends with a traceback; the port fails in a controlled way.
            tracebacks += 1;
            if output.code != 1
                || !err.is_empty()
                || output.out.len() != 1
                || !out.starts_with("FAIL: ")
            {
                differences.push(format!("{name}: traceback became {output:?}"));
            }
            continue;
        }
        // The frozen corpus predates retirement of the Python refresh command.
        let expected_stdout = expected["stdout"].as_str().unwrap().replace(
            "Run: python tools/refresh_cache.py\n",
            "Run: ozzit refresh_cache\n",
        );
        if i64::from(output.code) != expected["code"].as_i64().unwrap()
            || out != expected_stdout
            || err != expected["stderr"].as_str().unwrap()
        {
            differences.push(format!(
                "{name}: got {} {out:?} {err:?}, expected {} {:?} {:?}",
                output.code, expected["code"], expected_stdout, expected["stderr"]
            ));
        }
    }
    assert_eq!(tracebacks, 6);
    assert!(
        differences.is_empty(),
        "{} differences:\n{}",
        differences.len(),
        differences.join("\n")
    );
}

fn cache_records(data: &Value) -> &[Value] {
    let records = data["records"].as_array().unwrap();
    assert_eq!(records.len(), 76);
    assert_eq!(
        records
            .iter()
            .filter(|r| r["expected"]["stdout"]
                .as_str()
                .is_some_and(|s| s.contains("Run: python tools/refresh_cache.py\n")))
            .count(),
        16
    );
    records
}

#[test]
fn values_agree_as_the_python_comparison_does() {
    let value = |text: &str| CellValue {
        text: text.into(),
        is_error: false,
    };
    let error = |text: &str| CellValue {
        text: text.into(),
        is_error: true,
    };
    assert!(cache::agree(
        &value("a\r\nb\rc\nd"),
        &value("a\\nb\\nc\\nd")
    ));
    assert!(cache::agree(&value("1"), &value("1.0000000000001")));
    assert!(!cache::agree(&value("1"), &value("1.001")));
    assert!(
        cache::agree(&value("nan"), &value("nan")),
        "equal text agrees before parsing"
    );
    assert!(!cache::agree(&value("nan"), &value("NaN")));
    assert!(
        cache::agree(&value("inf"), &value("inf")),
        "equal text agrees before parsing"
    );
    assert!(!cache::agree(&value("1e308"), &value("-1e308")));
    assert!(cache::agree(&error("#N/A"), &error("-2146826246")));
    assert!(!cache::agree(&error("#N/A"), &value("-2146826246")));
    assert!(!cache::agree(&error("#N/A"), &error("#N/A")));
    // float() strips ASCII whitespace and Unicode spaces, not U+001C to U+001F.
    for c in ['\x1c', '\x1d', '\x1e', '\x1f'] {
        assert!(!cache::agree(&value("100"), &value(&format!("100{c}"))));
        assert!(!cache::agree(&value(&format!("{c}100")), &value("100")));
    }
    assert!(cache::agree(&value("100"), &value("100\x0b")));
    assert!(cache::agree(&value("100"), &value("\u{a0}100")));
}

#[test]
fn captured_output_reads_as_text_mode_does() {
    assert_eq!(cache::captured_text(b"a\r\nb\rc\n").unwrap(), "a\nb\nc\n");
    assert_eq!(
        cache::captured_text(b"bad \xff byte").unwrap_err(),
        "'utf-8' codec can't decode byte 0xff in position 4: invalid start byte"
    );
}

#[cfg(windows)]
#[test]
fn the_dumper_runner_reports_exit_output_and_timeouts() {
    use std::time::Duration;
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("fake.cmd");
    std::fs::write(
        &script,
        "@echo off\r\necho out line\r\necho err line 1>&2\r\nexit /b 7\r\n",
    )
    .unwrap();
    let program = script.display().to_string();
    match cache::run_dumper(&program, &[], Duration::from_secs(60)).unwrap() {
        Dump::Exited {
            code,
            stdout,
            stderr,
        } => {
            assert_eq!(
                (code, stdout.as_str(), stderr.as_str()),
                (7, "out line\n", "err line \n")
            );
        }
        other => panic!("{other:?}"),
    }
    let slow = dir.path().join("slow.cmd");
    std::fs::write(&slow, "@ping -n 30 127.0.0.1 >nul\r\n").unwrap();
    let started = std::time::Instant::now();
    assert!(matches!(
        cache::run_dumper(&slow.display().to_string(), &[], Duration::from_millis(500)).unwrap(),
        Dump::TimedOut
    ));
    assert!(started.elapsed() < Duration::from_secs(20));
    assert!(matches!(
        cache::run_dumper("ozzit-no-such-dumper.exe", &[], Duration::from_secs(5)).unwrap(),
        Dump::NotFound
    ));
    // A clean exit with output Python cannot decode is not a clean dump.
    std::fs::write(dir.path().join("bad.bin"), b"\xff").unwrap();
    let bad = dir.path().join("bad.cmd");
    std::fs::write(&bad, "@type \"%~dp0bad.bin\"\r\n@exit /b 0\r\n").unwrap();
    assert_eq!(
        cache::run_dumper(&bad.display().to_string(), &[], Duration::from_secs(60)).unwrap_err(),
        "'utf-8' codec can't decode byte 0xff in position 0: invalid start byte"
    );
}

#[cfg(unix)]
#[test]
fn the_dumper_runner_reports_exit_output_and_timeouts() {
    use std::time::Duration;
    let run = |script: &str, timeout| {
        cache::run_dumper("sh", &["-c".into(), script.into()], timeout).unwrap()
    };
    match run(
        "echo out line; echo err line >&2; exit 7",
        Duration::from_secs(60),
    ) {
        Dump::Exited {
            code,
            stdout,
            stderr,
        } => {
            assert_eq!(
                (code, stdout.as_str(), stderr.as_str()),
                (7, "out line\n", "err line\n")
            );
        }
        other => panic!("{other:?}"),
    }
    match run("kill -9 $$", Duration::from_secs(60)) {
        Dump::Exited { code, .. } => assert_eq!(code, -9),
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        run("sleep 30", Duration::from_millis(500)),
        Dump::TimedOut
    ));
    assert!(matches!(
        cache::run_dumper("ozzit-no-such-dumper", &[], Duration::from_secs(5)).unwrap(),
        Dump::NotFound
    ));
    // A clean exit with output Python cannot decode is not a clean dump.
    for script in [r"printf '\377'", r"printf '\377' >&2"] {
        assert_eq!(
            run_err(script),
            "'utf-8' codec can't decode byte 0xff in position 0: invalid start byte"
        );
    }
}

#[cfg(unix)]
fn run_err(script: &str) -> String {
    cache::run_dumper(
        "sh",
        &["-c".into(), script.into()],
        std::time::Duration::from_secs(60),
    )
    .unwrap_err()
}
