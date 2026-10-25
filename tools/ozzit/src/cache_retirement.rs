//! Original cache/OPC contracts, with a fake dumper through the real gate sequence.
use super::*;

struct Fixture {
    _dir: tempfile::TempDir,
    book: PathBuf,
}

impl Fixture {
    fn parts(parts: Parts) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let book = dir.path().join("cache.xlsx");
        std::fs::write(&book, deterministic_bytes(&parts).unwrap()).unwrap();
        Self { _dir: dir, book }
    }

    fn original() -> Parts {
        fixture_parts("cache_parts")
    }

    fn sheet(cells: &str) -> Self {
        let mut parts = Self::original();
        parts.shift_remove("xl/sharedStrings.xml");
        parts.insert(
            "xl/worksheets/sheet1.xml".into(),
            format!("<worksheet><sheetData>{cells}</sheetData></worksheet>").into_bytes(),
        );
        Self::parts(parts)
    }

    fn numbers(count: usize, value: &str) -> Self {
        Self::sheet(
            &(1..=count)
                .map(|row| format!("<c r=\"A{row}\"><v>{value}</v></c>"))
                .collect::<String>(),
        )
    }
}

fn fixture_parts(name: &str) -> Parts {
    let data: serde_json::Value =
        serde_json::from_str(include_str!("../tests/data/cache-retirement.json")).unwrap();
    data[name]
        .as_object()
        .unwrap()
        .iter()
        .map(|(name, body)| (name.clone(), body.as_str().unwrap().as_bytes().to_vec()))
        .collect()
}

fn value(text: &str) -> CellValue {
    CellValue {
        text: text.into(),
        is_error: false,
    }
}

fn error(text: &str) -> CellValue {
    CellValue {
        text: text.into(),
        is_error: true,
    }
}

fn key(pos: usize, row: usize, col: usize) -> Key {
    (pos.into(), row.into(), col.into())
}

fn success() -> Result<Dump> {
    Ok(Dump::Exited {
        code: 0,
        stdout: String::new(),
        stderr: String::new(),
    })
}

fn expected_dumper() -> PathBuf {
    let checkout = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let exe = std::env::current_exe().unwrap();
    let ancestors = exe.parent().unwrap().ancestors();
    let script = Path::new("tools/dump_values.ps1");
    if exe.starts_with(checkout) {
        assert!(checkout.join(script).is_file());
        assert!(
            !ancestors
                .take_while(|dir| *dir != checkout)
                .any(|dir| dir.join(script).is_file())
        );
        checkout.join(script)
    } else {
        // An external Cargo target exercises the existing working-directory fallback.
        assert!(!ancestors.into_iter().any(|dir| dir.join(script).is_file()));
        std::env::current_dir().unwrap().join(script)
    }
}

fn check_launch(book: &Path, program: &str, args: &[String], timeout: Duration) -> PathBuf {
    assert_eq!(program, "powershell.exe");
    assert_eq!(timeout, Duration::from_secs(600));
    let path = PathBuf::from(args.last().unwrap());
    assert!(path.is_file());
    assert!(
        path.file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("ozzit-cache-")
    );
    assert_eq!(path.extension().unwrap(), "tsv");
    assert_eq!(std::fs::read(&path).unwrap(), b"");
    assert_eq!(
        args,
        &[
            "-NoProfile".to_string(),
            "-ExecutionPolicy".into(),
            "Bypass".into(),
            "-File".into(),
            expected_dumper().display().to_string(),
            "-Path".into(),
            std::path::absolute(book).unwrap().display().to_string(),
            "-Out".into(),
            path.display().to_string(),
        ]
    );
    path
}

/// Every launched test checks the complete process contract, cleanup and no writes.
fn gate(book: &Path, floor: usize, bytes: Option<&[u8]>, result: Result<Dump>) -> Output {
    let before = std::fs::read(book).unwrap();
    let mut dumps = Vec::new();
    let shown = book.display().to_string();
    let output = verify_with(
        std::slice::from_ref(&shown),
        floor,
        |program, args, timeout| {
            let path = check_launch(book, program, args, timeout);
            dumps.push(path.clone());
            if let Some(bytes) = bytes {
                std::fs::write(path, bytes).unwrap();
            } else {
                std::fs::remove_file(path).unwrap();
            }
            result
        },
    );
    assert_eq!(dumps.len(), 1);
    assert!(!dumps[0].exists(), "the gate left its value dump behind");
    assert_eq!(std::fs::read(book).unwrap(), before);
    output
}

const COMPLETE: &str = "1\t1\t1\tvalue\tA&B\n1\t2\t2\tvalue\tTrue\n1\t3\t3\tvalue\tinline <value>\n1\t4\t4\tvalue\t123.5\n1\t6\t27\tvalue\tcached text\n";
const ERRORS: [(i64, &str); 8] = [
    (-2146826288, "#NULL!"),
    (-2146826281, "#DIV/0!"),
    (-2146826273, "#VALUE!"),
    (-2146826265, "#REF!"),
    (-2146826259, "#NAME?"),
    (-2146826252, "#NUM!"),
    (-2146826246, "#N/A"),
    (-2146826245, "#GETTING_DATA"),
];

#[test]
fn test_cached_values_reads_each_supported_cell_representation() {
    let fixture = Fixture::parts(Fixture::original());
    let (values, names) = cached_values(&fixture.book).unwrap();
    assert_eq!(names, BTreeMap::from([(1, "Data".into())]));
    assert_eq!(
        values,
        BTreeMap::from([
            (key(1, 1, 1), value("A&B")),
            (key(1, 2, 2), value("True")),
            (key(1, 3, 3), value("inline <value>")),
            (key(1, 4, 4), value("123.5")),
            (key(1, 6, 27), value("cached text")),
        ])
    );
}

#[test]
fn test_cached_values_rejects_chart_sheets_before_positions_drift() {
    let mut parts = Fixture::original();
    let rels = String::from_utf8(parts["xl/_rels/workbook.xml.rels"].clone()).unwrap();
    parts.insert(
        "xl/_rels/workbook.xml.rels".into(),
        rels.replace("worksheets/", "chartsheets/").into_bytes(),
    );
    let fixture = Fixture::parts(parts);
    let CachedError::Refused(message) = cached_values(&fixture.book).unwrap_err() else {
        panic!("expected chart refusal")
    };
    assert!(message.contains("are not worksheets"));
    let output = gate(&fixture.book, FLOOR, Some(b""), success());
    assert_eq!(output.code, 1);
    assert_eq!(output.err, [message]);
}

#[test]
fn test_cached_values_preserves_the_error_kind() {
    let fixture = Fixture::sheet(
        "<row r=\"1\"><c r=\"A1\" t=\"e\"><v>#DIV/0!</v></c><c r=\"B1\" t=\"str\"><v>#DIV/0!</v></c><c r=\"C1\"><v>-2146826281</v></c></row>",
    );
    assert_eq!(
        cached_values(&fixture.book).unwrap().0,
        BTreeMap::from([
            (key(1, 1, 1), error("#DIV/0!")),
            (key(1, 1, 2), value("#DIV/0!")),
            (key(1, 1, 3), value("-2146826281")),
        ])
    );
}

#[test]
fn test_cache_gate_requires_each_cached_cell_even_above_the_floor() {
    let fixture = Fixture::parts(Fixture::original());
    for (dump, expected) in [
        (COMPLETE.to_string(), 0),
        (COMPLETE.replace("1\t4\t4\tvalue\t123.5\n", ""), 1),
        (COMPLETE.replace("123.5", "999"), 1),
    ] {
        let output = gate(&fixture.book, 4, Some(dump.as_bytes()), success());
        assert_eq!(output.code, expected, "{output:?}");
        if !dump.contains("123.5") && !dump.contains("999") {
            assert!(output.out.iter().any(|l| l.contains("row 4 col 4")));
        }
    }
}

#[test]
fn test_cache_gate_fails_cleanly_when_excel_cannot_be_driven() {
    let fixture = Fixture::parts(Fixture::original());
    for (dump, expected) in [
        (Dump::TimedOut, "did not finish"),
        (Dump::NotFound, "was not found"),
    ] {
        let output = gate(&fixture.book, FLOOR, Some(b""), Ok(dump));
        assert_eq!(output.code, 1);
        assert!(output.out[0].contains(expected));
    }
}

#[test]
fn test_cache_gate_distinguishes_errors_from_error_code_numbers() {
    for (code, label) in ERRORS {
        let code = code.to_string();
        for (cached, cached_error, kind, live, expected) in [
            (label, true, "error", code.clone(), 0),
            (label, true, "value", code.clone(), 1),
            (label, true, "value", format!("{code}.75"), 1),
            (code.as_str(), false, "error", code.clone(), 1),
            (label, false, "error", code.clone(), 1),
            (label, false, "value", label.to_string(), 0),
        ] {
            let kind_cached = if cached_error {
                "e"
            } else if cached.starts_with('#') {
                "str"
            } else {
                "n"
            };
            let fixture = Fixture::sheet(&format!(
                "<row r=\"1\"><c r=\"A1\" t=\"{kind_cached}\"><v>{cached}</v></c></row>"
            ));
            let dump = format!("1\t1\t1\t{kind}\t{live}\n");
            let output = gate(&fixture.book, 1, Some(dump.as_bytes()), success());
            assert_eq!(
                output.code, expected,
                "{cached:?}/{kind}/{live:?}: {output:?}"
            );
        }
    }
}

#[test]
fn test_cache_gate_refuses_malformed_or_ambiguous_dump_records() {
    let fixture = Fixture::parts(Fixture::original());
    for dump in [
        b"1\t1\t1\told-format\n".as_slice(),
        b"1\t1\t1\tunknown\t1\n",
        b"0\t1\t1\tvalue\t1\n",
        b"bad\t1\t1\tvalue\t1\n",
        b"1\t1\t1\tvalue\t1\n1\t1\t1\tvalue\t2\n",
        b"\xff",
    ] {
        let output = gate(&fixture.book, FLOOR, Some(dump), success());
        assert_eq!(output.code, 1);
        assert!(output.out[0].starts_with("FAIL: cannot read Excel dump:"));
    }
}

#[test]
fn test_cache_gate_reports_an_unreadable_value_dump() {
    let fixture = Fixture::parts(Fixture::original());
    let output = gate(&fixture.book, FLOOR, None, success());
    assert_eq!(output.code, 1);
    assert!(output.out[0].starts_with("FAIL: cannot read Excel dump:"));
}

#[test]
fn test_cache_gate_compares_nonfinite_values_at_the_real_floor() {
    assert_eq!(FLOOR, 15000);
    for (cached, live, expected, live_kind) in [
        ("1", "inf", 1, "value"),
        ("1", "-inf", 1, "value"),
        ("1", "1e309", 1, "value"),
        ("inf", "1", 1, "value"),
        ("inf", "-inf", 1, "value"),
        ("#NUM!", "inf", 1, "value"),
        ("#NUM!", "nan", 1, "value"),
        ("1", "nan", 1, "value"),
        ("inf", "inf", 0, "value"),
        ("1000000", "1000000.0005", 0, "value"),
        ("#NUM!", "-2146826252", 0, "error"),
        ("#NUM!", "-2146826252", 1, "value"),
    ] {
        let cells = (1..=FLOOR)
            .map(|row| {
                let kind = if row == 1 && cached.starts_with('#') {
                    " t=\"e\""
                } else {
                    ""
                };
                let text = if row == 1 { cached } else { "1" };
                format!("<row r=\"{row}\"><c r=\"A{row}\"{kind}><v>{text}</v></c></row>")
            })
            .collect::<String>();
        let fixture = Fixture::sheet(&cells);
        let dump = format!("1\t1\t1\t{live_kind}\t{live}\n")
            + &(2..=FLOOR)
                .map(|row| format!("1\t{row}\t1\tvalue\t1\n"))
                .collect::<String>();
        let output = gate(&fixture.book, FLOOR, Some(dump.as_bytes()), success());
        assert_eq!(output.code, expected, "{cached}/{live}: {output:?}");
        if expected == 1 {
            assert!(output.out[0].starts_with("FAIL: 1 of 15000 cached values"));
            assert!(output.out[1].contains("Data row 1 col 1"));
        } else {
            assert!(output.out[0].starts_with("OK: all 15000 cached values"));
        }
    }
}

#[test]
fn test_fold_normalises_all_physical_newline_forms() {
    assert_eq!(fold("a\r\nb\rc\nd"), r"a\nb\nc\nd");
}

#[test]
fn test_agree_accepts_exact_text_and_negligible_numeric_rounding() {
    assert!(agree(&value("plain"), &value("plain")));
    assert!(agree(&value("1000000"), &value("1000000.0005")));
}

#[test]
fn test_agree_rejects_material_numeric_and_text_differences() {
    assert!(!agree(&value("1"), &value("1.01")));
    assert!(!agree(&value("old"), &value("new")));
}

#[test]
fn test_agree_maps_excel_com_error_codes() {
    assert!(agree(&error("#DIV/0!"), &error("-2146826281")));
    assert!(!agree(&error("#N/A"), &error("-2146826281")));
}

#[test]
fn test_agree_rejects_nonfinite_numeric_coercion() {
    for text in ["inf", "-inf", "nan", "NaN", "1e309", "-1e309"] {
        assert!(!agree(&value("1"), &value(text)));
        assert!(!agree(&value(text), &value("1")));
        assert!(!agree(&error("#NUM!"), &error(text)));
    }
    assert!(!agree(&value("inf"), &value("-inf")));
}

#[test]
fn test_agree_preserves_exact_nonfinite_text_and_newlines() {
    for text in ["inf", "-inf", "nan", "1e309"] {
        assert!(agree(&value(text), &value(text)));
    }
    assert!(agree(&value("a\r\nb"), &value(r"a\nb")));
}

#[test]
fn test_agree_accepts_only_integral_codes_from_real_errors() {
    for (code, label) in ERRORS {
        assert!(agree(&error(label), &error(&code.to_string())));
        assert!(!agree(&error(label), &error(&format!("{code}.75"))));
        assert!(!agree(&error(label), &error(&(-code).to_string())));
        assert!(!agree(&error(label), &value(&code.to_string())));
        assert!(!agree(&value(label), &error(&code.to_string())));
    }
}

#[test]
fn test_a_missing_workbook_fails_before_excel_is_driven() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("missing.xlsx").display().to_string();
    let output = verify_with(std::slice::from_ref(&missing), FLOOR, |_, _, _| {
        panic!("missing workbook launched a process")
    });
    assert_eq!(output.code, 1);
    assert_eq!(output.out, [format!("FAIL: no such workbook: {missing}")]);
    assert!(output.err.is_empty());
}

#[test]
fn test_an_excel_failure_reports_its_exit_code_and_output() {
    let fixture = Fixture::numbers(1, "1");
    let output = gate(
        &fixture.book,
        FLOOR,
        None,
        Ok(Dump::Exited {
            code: 3,
            stdout: String::new(),
            stderr: "  COM refused \n".into(),
        }),
    );
    assert_eq!(output.code, 1);
    assert_eq!(
        output.out,
        [
            "FAIL: could not read the workbook in Excel (exit 3)",
            "COM refused"
        ]
    );
    assert!(output.err.is_empty());
}

#[test]
fn test_cached_values_without_shared_strings_skip_empty_inline_cells() {
    let fixture = Fixture::sheet("<c r=\"A1\"><v>7</v></c><c r=\"B1\" t=\"inlineStr\"><is/></c>");
    assert_eq!(
        cached_values(&fixture.book).unwrap(),
        (
            BTreeMap::from([(key(1, 1, 1), value("7"))]),
            BTreeMap::from([(1, "Data".into())]),
        )
    );
}

#[test]
fn test_malformed_dump_lines_fail_with_a_diagnostic() {
    let fixture = Fixture::sheet("<c r=\"A1\"><v>7</v></c><c r=\"B1\" t=\"inlineStr\"><is/></c>");
    let output = gate(
        &fixture.book,
        FLOOR,
        Some(b"1\t1\t1\tvalue\t7\nnot a dump row\n1\t2\n"),
        success(),
    );
    assert_eq!(output.code, 1);
    assert!(output.out[0].starts_with("FAIL: cannot read Excel dump: row 2"));
}

#[test]
fn test_a_complete_but_thin_dump_still_fails_the_floor() {
    let fixture = Fixture::numbers(1, "1");
    let output = gate(
        &fixture.book,
        FLOOR,
        Some(b"1\t1\t1\tvalue\t1\n"),
        success(),
    );
    assert_eq!(output.code, 1);
    assert!(output.out[0].starts_with(
        "FAIL: only 1 cells carried both a cached and a calculated value, under the floor of 15000."
    ));
}

#[test]
fn test_value_tabs_remain_inside_the_final_dump_field() {
    let fixture = Fixture::sheet("<c r=\"A1\" t=\"str\"><v>a\tb</v></c>");
    let output = gate(&fixture.book, 1, Some(b"1\t1\t1\tvalue\ta\tb\n"), success());
    assert_eq!(output.code, 0, "{output:?}");
}

#[test]
fn test_more_than_twenty_missing_cells_are_summarised() {
    let fixture = Fixture::numbers(25, "1");
    let output = gate(&fixture.book, FLOOR, Some(b""), success());
    assert_eq!(output.code, 1);
    assert_eq!(
        output.out[0],
        "FAIL: 25 cached value(s) are missing from the Excel dump"
    );
    assert_eq!(output.out.len(), 22);
    assert_eq!(output.out.last().unwrap(), "  ... and 5 more");
}

#[test]
fn test_more_than_twenty_stale_cells_are_summarised() {
    let fixture = Fixture::numbers(25, "1");
    let dump = (1..=25)
        .map(|row| format!("1\t{row}\t1\tvalue\t2\n"))
        .collect::<String>();
    let output = gate(&fixture.book, 1, Some(dump.as_bytes()), success());
    assert_eq!(output.code, 1);
    assert_eq!(
        output.out[0],
        "FAIL: 25 of 25 cached values are not what the formula produces, on 1 sheet(s)"
    );
    assert_eq!(
        output.out[1],
        "  - Data row 1 col 1: cached 1 (value), calculates to 2 (value)"
    );
    assert_eq!(
        &output.out[output.out.len() - 2..],
        ["  ... and 5 more", "Run: ozzit refresh_cache"]
    );
}

#[test]
fn test_targets_resolve_per_opc() {
    for (source, target, expected) in [
        (
            "xl/workbook.xml",
            "/xl/worksheets/sheet1.xml",
            "xl/worksheets/sheet1.xml",
        ),
        (
            "xl/workbook.xml",
            "worksheets/sheet1.xml",
            "xl/worksheets/sheet1.xml",
        ),
        (
            "xl/worksheets/sheet1.xml",
            "../tables/table1.xml",
            "xl/tables/table1.xml",
        ),
        (
            "xl/worksheets/sheet1.xml",
            "/xl/tables/table1.xml",
            "xl/tables/table1.xml",
        ),
        ("", "xl/workbook.xml", "xl/workbook.xml"),
    ] {
        assert_eq!(resolve_target(source, target), expected);
    }
}

#[test]
fn test_verify_cache_reads_an_absolute_target() {
    let fixture = Fixture::parts(fixture_parts("absolute_parts"));
    assert_eq!(
        cached_values(&fixture.book).unwrap(),
        (
            BTreeMap::from([(key(1, 1, 1), value("1")), (key(2, 1, 1), value("2"))]),
            BTreeMap::from([(1, "Data".into()), (2, "Notes".into())]),
        )
    );
}

#[test]
fn a_runner_error_and_an_invalid_workbook_still_clean_the_dump() {
    let fixture = Fixture::numbers(1, "1");
    let output = gate(
        &fixture.book,
        FLOOR,
        Some(b""),
        Err("synthetic runner error".into()),
    );
    assert_eq!(output.out, ["FAIL: synthetic runner error"]);
    assert_eq!(output.code, 1);
    std::fs::write(&fixture.book, b"not an archive").unwrap();
    let output = gate(&fixture.book, FLOOR, Some(b""), success());
    assert_eq!(output.code, 1);
    assert!(output.out[0].starts_with("FAIL:"));
}
