//! Published CSV and command contracts retained from the Python gate tests.
use ozzit::{common::*, index};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
const GOOD: &str = "OK: 153 functions, 130 of them replacing a name from released-names-v1.2.6.txt, 23 new since it";
const FIELDS: &[&str] = &[
    "function",
    "module",
    "previous_name",
    "signature",
    "description",
];

fn newline() -> &'static str {
    if cfg!(windows) { "\r\n" } else { "\n" }
}

struct Fixture {
    dir: tempfile::TempDir,
    index: PathBuf,
    csv: Csv,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let index = dir.path().join("functions.csv");
        let csv = Csv::read(&root().join("functions.csv")).unwrap();
        assert_eq!(csv.rows.len(), 153);
        Self { dir, index, csv }
    }
    fn write(
        &self,
        columns: &[&str],
        overrides: &[(usize, &str)],
        empty: bool,
        bom: bool,
        quoted: bool,
    ) {
        let mut text = if bom {
            "\u{feff}".to_owned()
        } else {
            String::new()
        };
        let encode = |row: Vec<String>| {
            if quoted {
                row.iter()
                    .map(|s| format!("\"{}\"", s.replace('"', "\"\"")))
                    .collect::<Vec<_>>()
                    .join(",")
                    + "\n"
            } else {
                csv_row(&row)
            }
        };
        text += &encode(columns.iter().map(|s| (*s).to_owned()).collect());
        if !empty {
            for row in &self.csv.rows {
                let mut values = columns
                    .iter()
                    .map(|field| {
                        self.csv
                            .get(row, field)
                            .unwrap_or("extra,\nvalue")
                            .to_owned()
                    })
                    .collect::<Vec<_>>();
                for &(position, value) in overrides {
                    values[position] = value.to_owned();
                }
                text += &encode(values);
            }
        }
        std::fs::write(&self.index, text).unwrap();
    }
    fn write_rows(&self, columns: &[String], rows: &[Vec<String>]) {
        let text = std::iter::once(columns)
            .chain(rows.iter().map(Vec::as_slice))
            .map(csv_row)
            .collect::<String>();
        std::fs::write(&self.index, text).unwrap();
    }
    fn run(&self) -> Output {
        let before = std::fs::read(&self.index).unwrap();
        let output = index::previous_names(&[display(&self.index)]);
        assert_eq!(std::fs::read(&self.index).unwrap(), before);
        output
    }
}
fn good(output: Output) {
    assert_eq!(output.code, 0, "{:?} {:?}", output.out, output.err);
    assert_eq!(output.out, [GOOD]);
    assert!(output.err.is_empty(), "{:?}", output.err);
}
fn duplicate(f: &Fixture, fields: &[&str]) {
    let output = f.run();
    assert_eq!(output.code, 1);
    assert!(output.out.is_empty(), "{:?}", output.out);
    assert_eq!(
        output.err,
        [format!(
            "{} has duplicate CSV column(s): {}",
            display(&f.index),
            fields.join(", ")
        )]
    );
}

#[test]
fn repeated_consumed_columns_reject_identical_and_contradictory_values() {
    let f = Fixture::new();
    for field in ["function", "previous_name", "signature", "description"] {
        for identical in [false, true] {
            let columns = [&[field][..], FIELDS].concat();
            f.write(
                &columns,
                if identical { &[] } else { &[(0, "")] },
                false,
                false,
                false,
            );
            duplicate(&f, &[field]);
        }
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "Retains the original six-case header contract table."
)]
fn parsed_headers_are_checked_before_mapping_rows() {
    let f = Fixture::new();
    for (columns, overrides, empty, bom, quoted, repeated) in [
        (
            [&["function"][..], FIELDS].concat(),
            vec![],
            false,
            false,
            false,
            vec!["function"],
        ),
        (
            [FIELDS, &["function"]].concat(),
            vec![(5, "")],
            false,
            false,
            false,
            vec!["function"],
        ),
        (
            [&["function", "function"][..], FIELDS].concat(),
            vec![],
            false,
            false,
            false,
            vec!["function"],
        ),
        (
            [&["signature", "function"][..], FIELDS].concat(),
            vec![],
            false,
            false,
            false,
            vec!["function", "signature"],
        ),
        (
            [FIELDS, &["function"]].concat(),
            vec![],
            true,
            false,
            false,
            vec!["function"],
        ),
        (
            [&["function"][..], FIELDS].concat(),
            vec![],
            false,
            true,
            true,
            vec!["function"],
        ),
    ] {
        f.write(&columns, &overrides, empty, bom, quoted);
        duplicate(&f, &repeated);
    }
}

#[test]
fn ignored_columns_reordering_bom_and_quoted_multiline_values_are_preserved() {
    let f = Fixture::new();
    let reversed = FIELDS
        .iter()
        .rev()
        .copied()
        .chain(["Function", "extra"])
        .collect::<Vec<_>>();
    for (columns, overrides, bom) in [
        (
            [&["module"][..], FIELDS].concat(),
            vec![(0, "ignored contradiction")],
            false,
        ),
        (
            [&["extra"][..], FIELDS, &["extra"]].concat(),
            vec![(0, "ignored contradiction")],
            false,
        ),
        (
            FIELDS.iter().copied().filter(|f| *f != "module").collect(),
            vec![],
            false,
        ),
        (reversed.clone(), vec![], false),
        (reversed, vec![], true),
    ] {
        f.write(&columns, &overrides, false, bom, false);
        good(f.run());
    }
}

#[test]
fn uppercase_function_is_not_the_required_field() {
    let f = Fixture::new();
    f.write(
        &[
            "Function",
            "module",
            "previous_name",
            "signature",
            "description",
        ],
        &[],
        false,
        false,
        false,
    );
    let output = f.run();
    assert_eq!(output.code, 1);
    assert!(output.out.is_empty());
    assert_eq!(
        output.err,
        [format!("{} has no function column", display(&f.index))]
    );
}

#[test]
fn duplicate_new_function_with_blank_previous_name_is_reported() {
    let f = Fixture::new();
    let mut rows = f.csv.rows.clone();
    let mut duplicate = rows
        .iter()
        .find(|r| f.csv.get(r, "function") == Some("oz.AnnualRateλ"))
        .unwrap()
        .clone();
    assert_eq!(f.csv.get(&duplicate, "previous_name"), Some(""));
    let description = f
        .csv
        .headers
        .iter()
        .position(|h| h == "description")
        .unwrap();
    duplicate[description] = "Fabricated".into();
    rows.insert(0, duplicate);
    f.write_rows(&f.csv.headers, &rows);
    let output = f.run();
    assert_eq!(output.code, 1);
    assert!(output.err.is_empty());
    assert_eq!(
        output.out,
        [
            format!("FAIL: 1 problem(s) in {}", display(&f.index)),
            "  - oz.AnnualRateλ appears 2 times in functions.csv".into()
        ]
    );
}

#[test]
fn truncated_rows_report_missing_published_fields() {
    let f = Fixture::new();
    for columns in [1, 3, 4] {
        let mut rows = f.csv.rows.clone();
        rows[0].truncate(columns);
        f.write_rows(&f.csv.headers, &rows);
        let output = f.run();
        assert_eq!(output.code, 1);
        assert!(output.err.is_empty(), "{:?}", output.err);
        let field = if columns == 4 {
            "description"
        } else {
            "signature"
        };
        let function = f.csv.get(&rows[0], "function").unwrap();
        assert!(
            output
                .out
                .contains(&format!("  - {function} publishes an empty {field}")),
            "{:?}",
            output.out
        );
        assert!(
            !output
                .out
                .iter()
                .any(|s| s.contains("OK:") || s.contains("Traceback"))
        );
    }
}

#[test]
fn missing_trailing_previous_name_is_allowed_for_a_new_function() {
    let f = Fixture::new();
    let fields = f
        .csv
        .headers
        .iter()
        .filter(|h| *h != "previous_name")
        .cloned()
        .chain(["previous_name".into()])
        .collect::<Vec<_>>();
    let rows = f
        .csv
        .rows
        .iter()
        .map(|row| {
            let mut values = fields
                .iter()
                .map(|field| f.csv.get(row, field).unwrap().to_owned())
                .collect::<Vec<_>>();
            if f.csv.get(row, "function") == Some("oz.AnnualRateλ") {
                assert_eq!(values.pop(), Some(String::new()));
            }
            values
        })
        .collect::<Vec<_>>();
    assert!(rows.iter().any(|r| r.len() == fields.len() - 1));
    f.write_rows(&fields, &rows);
    good(f.run());
}

#[test]
fn all_seven_input_read_failures_keep_channels_status_and_input_bytes() {
    let f = Fixture::new();
    let missing = f.dir.path().join("missing");
    let directory = f.dir.path().join("directory");
    std::fs::create_dir(&directory).unwrap();
    let undecodable = f.dir.path().join("not-utf8");
    std::fs::write(&undecodable, [0xff]).unwrap();
    let oversized = f.dir.path().join("oversized.csv");
    std::fs::write(
        &oversized,
        format!(
            "function,previous_name,signature,description\n{},,,\n",
            "x".repeat(131_073)
        ),
    )
    .unwrap();
    let current = root().join("functions.csv");
    let baseline = root().join("tools/released-names-v1.2.6.txt");
    for (csv, names, failed) in [
        (&missing, &baseline, &missing),
        (&directory, &baseline, &directory),
        (&undecodable, &baseline, &undecodable),
        (&oversized, &baseline, &oversized),
        (&current, &missing, &missing),
        (&current, &directory, &directory),
        (&current, &undecodable, &undecodable),
    ] {
        let before = [csv, names].map(|p| p.is_file().then(|| std::fs::read(p).unwrap()));
        let output = index::previous_names(&[display(csv), display(names)]);
        assert_eq!(output.code, 1);
        assert!(output.err.is_empty(), "{:?}", output.err);
        assert_eq!(output.out.len(), 1);
        let prefix = format!("FAIL: cannot read {}: ", display(failed));
        assert!(
            output.out[0].starts_with(&prefix) && output.out[0].len() > prefix.len(),
            "{:?}",
            output.out
        );
        assert!(!output.out[0].contains("OK:") && !output.out[0].contains("Traceback"));
        if failed == &oversized {
            assert!(output.out[0].contains("field larger than field limit (131072)"));
        }
        for (path, bytes) in [csv, names].into_iter().zip(before) {
            if let Some(bytes) = bytes {
                assert_eq!(std::fs::read(path).unwrap(), bytes);
            }
        }
        assert!(!missing.exists());
        assert!(directory.is_dir());
    }
}

#[test]
fn physical_command_defaults_use_owned_csv_and_embedded_baseline() {
    let f = Fixture::new();
    std::fs::copy(root().join("functions.csv"), &f.index).unwrap();
    let before = std::fs::read(&f.index).unwrap();
    // No tools directory is present: default baseline resolution is independent of cwd.
    assert!(!f.dir.path().join("tools").exists());
    for args in [
        vec![],
        vec!["functions.csv"],
        vec!["functions.csv", "baseline.txt"],
    ] {
        if args.len() == 2 {
            std::fs::copy(
                root().join("tools/released-names-v1.2.6.txt"),
                f.dir.path().join("baseline.txt"),
            )
            .unwrap();
        }
        let result = Command::new(env!("CARGO_BIN_EXE_ozzit"))
            .current_dir(f.dir.path())
            .arg("verify_previous_names")
            .args(&args)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(0), "{:?}", result);
        assert!(result.stderr.is_empty(), "{:?}", result.stderr);
        let expected = if args.len() == 2 {
            GOOD.replace("released-names-v1.2.6.txt", "baseline.txt")
        } else {
            GOOD.into()
        };
        assert_eq!(
            String::from_utf8(result.stdout).unwrap(),
            expected + newline()
        );
        assert_eq!(std::fs::read(&f.index).unwrap(), before);
    }
    assert_eq!(
        std::fs::read(f.dir.path().join("baseline.txt")).unwrap(),
        std::fs::read(root().join("tools/released-names-v1.2.6.txt")).unwrap()
    );
}

#[test]
fn physical_command_rejects_surplus_inputs_before_reading_or_writing() {
    let f = Fixture::new();
    let result = Command::new(env!("CARGO_BIN_EXE_ozzit"))
        .current_dir(f.dir.path())
        .args([
            "verify_previous_names",
            "functions.csv",
            "baseline.txt",
            "unrequested.xlsx",
        ])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty());
    assert_eq!(
        String::from_utf8(result.stderr).unwrap(),
        format!(
            "FAIL: usage: ozzit verify_previous_names [functions.csv] [baseline]{}",
            newline()
        )
    );
    assert_eq!(std::fs::read_dir(f.dir.path()).unwrap().count(), 0);
}
