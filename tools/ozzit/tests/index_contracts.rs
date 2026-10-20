//! Index gate contracts retained from the Python path and inventory tests.
use ozzit::{archive, common::*, compiler, formula, index};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
};

const FIELDS: &[&str] = &[
    "function",
    "module",
    "previous_name",
    "signature",
    "description",
];
const GOOD: &str = "OK: all 153 functions.csv rows match the workbook and source modules";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn newline() -> &'static str {
    if cfg!(windows) { "\r\n" } else { "\n" }
}

struct Fixture {
    dir: tempfile::TempDir,
    book: PathBuf,
    src: PathBuf,
    index: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let book = dir.path().join("ozzit.xlsx");
        let src = dir.path().join("src");
        let index = dir.path().join("functions.csv");
        std::fs::create_dir(&src).unwrap();
        Self {
            dir,
            book,
            src,
            index,
        }
    }
    fn published() -> Self {
        let f = Self::new();
        for path in [&f.book, &f.index] {
            std::fs::copy(root().join(path.file_name().unwrap()), path).unwrap();
        }
        for module in formula::MODULES {
            let name = format!("{module}.txt");
            std::fs::copy(root().join("src").join(&name), f.src.join(name)).unwrap();
        }
        f
    }
    fn small() -> Self {
        let f = Self::new();
        std::fs::write(f.src.join("Mod.txt"), "Aλ = LAMBDA(x, x);\n(1 + 2);\n").unwrap();
        let book = "<workbook><definedNames><definedName name=\"oz.Aλ\">LAMBDA(x, x)</definedName><definedName name=\"oz.Bλ\">LAMBDA(y, y)</definedName></definedNames></workbook>";
        archive::write_deterministic(
            &f.book,
            &[(archive::BOOK.into(), book.as_bytes().to_vec())].into(),
        )
        .unwrap();
        f.write_rows(
            &[
                "function".into(),
                "module".into(),
                "signature".into(),
                "description".into(),
            ],
            &[],
        );
        f
    }
    fn snapshot(&self) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
        fn visit(root: &Path, path: &Path, result: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
            for entry in std::fs::read_dir(path).unwrap() {
                let entry = entry.unwrap();
                assert!(!entry.file_type().unwrap().is_symlink());
                let path = entry.path();
                let data = if path.is_dir() {
                    visit(root, &path, result);
                    None
                } else {
                    Some(std::fs::read(&path).unwrap())
                };
                result.insert(path.strip_prefix(root).unwrap().into(), data);
            }
        }
        let mut result = BTreeMap::new();
        visit(self.dir.path(), self.dir.path(), &mut result);
        result
    }
    fn preserved<T>(&self, operation: impl FnOnce() -> T) -> T {
        let before = self.snapshot();
        let result = operation();
        assert_eq!(self.snapshot(), before);
        result
    }
    fn args(&self) -> Vec<String> {
        [&self.book, &self.src, &self.index]
            .map(|p| display(p))
            .to_vec()
    }
    fn run(&self) -> Output {
        self.preserved(|| index::verify(&self.args()))
    }
    fn cli(&self, args: &[String]) -> std::process::Output {
        self.preserved(|| {
            Command::new(env!("CARGO_BIN_EXE_ozzit"))
                .arg("verify_index")
                .args(args)
                .current_dir(self.dir.path())
                .output()
                .unwrap()
        })
    }
    fn write_rows(&self, columns: &[String], rows: &[Vec<String>]) {
        let text = std::iter::once(columns)
            .chain(rows.iter().map(Vec::as_slice))
            .map(csv_row)
            .collect::<String>();
        std::fs::write(&self.index, text).unwrap();
    }
    fn columns(
        &self,
        columns: &[&str],
        overrides: &[(usize, &str)],
        empty: bool,
        bom: bool,
        quoted: bool,
    ) {
        let csv = Csv::read(&root().join("functions.csv")).unwrap();
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
        let mut text = if bom {
            "\u{feff}".to_owned()
        } else {
            String::new()
        };
        text += &encode(columns.iter().map(|s| (*s).to_owned()).collect());
        if !empty {
            for row in &csv.rows {
                let mut values = columns
                    .iter()
                    .map(|field| csv.get(row, field).unwrap_or("extra,\nvalue").to_owned())
                    .collect::<Vec<_>>();
                for &(position, value) in overrides {
                    values[position] = value.to_owned();
                }
                text += &encode(values);
            }
        }
        std::fs::write(&self.index, text).unwrap();
    }
}

fn good(output: Output) {
    assert_eq!(output.code, 0, "{:?} {:?}", output.out, output.err);
    assert_eq!(output.out, [GOOD]);
    assert!(output.err.is_empty());
}
fn failure(output: Output) -> Vec<String> {
    assert_eq!(output.code, 1, "{:?} {:?}", output.out, output.err);
    assert!(output.err.is_empty(), "{:?}", output.err);
    assert!(output.out.first().unwrap().starts_with("FAIL:"));
    assert!(
        output
            .out
            .iter()
            .all(|s| !s.contains("Traceback") && !s.contains("OK:"))
    );
    output.out
}
fn duplicate(f: &Fixture, fields: &[&str]) {
    assert_eq!(
        failure(f.run()),
        [format!(
            "FAIL: functions.csv: duplicate CSV column(s): {}",
            fields.join(", ")
        )]
    );
}

#[test]
fn an_unclosed_signature_stops_at_the_last_help_row() {
    assert_eq!(
        compiler::index_fields("", "FUNCTION: →Fλ(a, b¶", "Fλ").unwrap(),
        ("Fλ(a, b".into(), String::new())
    );
}

#[test]
fn source_modules_ignore_the_original_non_declaration_statement() {
    let f = Fixture::small();
    assert_eq!(
        f.preserved(|| compiler::source_modules(&f.src)).unwrap(),
        [("oz.Aλ".into(), "Mod".into())].into()
    );
}

#[test]
fn header_only_index_reaches_missing_workbook_and_missing_part_diagnostics() {
    let f = Fixture::small();
    let missing = f.dir.path().join("missing.xlsx");
    let no_book = f.dir.path().join("no-book.xlsx");
    archive::write_deterministic(
        &no_book,
        &[("docProps/app.xml".into(), b"<Properties/>".to_vec())].into(),
    )
    .unwrap();
    for (book, detail) in [(missing, "No such file"), (no_book, "xl/workbook.xml")] {
        let mut args = f.args();
        args[0] = display(&book);
        let lines = failure(f.preserved(|| index::verify(&args)));
        assert_eq!(lines.len(), 1);
        assert!(lines[0].starts_with("FAIL: cannot read index inputs: "));
        assert!(lines[0].contains(detail), "{:?}", lines);
    }
}

#[test]
fn missing_names_on_each_side_have_the_exact_original_order() {
    let f = Fixture::small();
    f.write_rows(
        &[
            "function".into(),
            "module".into(),
            "signature".into(),
            "description".into(),
        ],
        &[
            vec!["oz.Aλ".into(), "Mod".into(), "Aλ".into(), "".into()],
            vec!["oz.Cλ".into(), "Mod".into(), "Cλ".into(), "".into()],
        ],
    );
    assert_eq!(
        failure(f.run()),
        [
            "FAIL: 2 functions.csv problem(s)",
            "  - oz.Bλ is in the workbook but missing from functions.csv",
            "  - oz.Cλ is in functions.csv but missing from the workbook"
        ]
    );
}

#[test]
fn contradictory_duplicate_cannot_hide_behind_the_later_valid_row() {
    let f = Fixture::published();
    let csv = Csv::read(&f.index).unwrap();
    let matches = csv
        .rows
        .iter()
        .filter(|row| csv.get(row, "function") == Some("oz.AnnualRateλ"))
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1);
    let original = matches[0].clone();
    let mut changed = original.clone();
    changed[csv.headers.iter().position(|s| s == "module").unwrap()] = "FabricatedModule".into();
    changed[csv.headers.iter().position(|s| s == "description").unwrap()] = "Fabricated".into();
    let mut rows = csv.rows.clone();
    rows.insert(0, changed);
    f.write_rows(&csv.headers, &rows);
    let actual = Csv::read(&f.index).unwrap();
    assert_eq!(actual.rows[0], rows[0]);
    assert_eq!(
        actual
            .rows
            .iter()
            .rfind(|row| csv.get(row, "function") == Some("oz.AnnualRateλ")),
        Some(&original)
    );
    assert_eq!(
        failure(f.run()),
        [
            "FAIL: 1 functions.csv problem(s)",
            "  - oz.AnnualRateλ appears 2 times in functions.csv"
        ]
    );
}

#[test]
fn invalid_utf8_and_directory_source_entries_fail_without_changing_inputs() {
    for directory in [false, true] {
        let f = Fixture::published();
        for entry in std::fs::read_dir(&f.src).unwrap() {
            std::fs::remove_file(entry.unwrap().path()).unwrap();
        }
        let source = f.src.join("Invalid.txt");
        if directory {
            std::fs::create_dir(&source).unwrap();
        } else {
            std::fs::write(&source, b"\xff").unwrap();
        }
        let lines = failure(f.run());
        assert_eq!(lines.len(), 1);
        assert!(lines[0].starts_with("FAIL: cannot read index inputs:"));
    }
}

fn changed_first_field(field: &str, value: &str, diagnostic: &str) {
    let f = Fixture::published();
    let csv = Csv::read(&f.index).unwrap();
    let mut rows = csv.rows.clone();
    rows[0][csv.headers.iter().position(|s| s == field).unwrap()] = value.into();
    f.write_rows(&csv.headers, &rows);
    let actual = Csv::read(&f.index).unwrap();
    assert_eq!(actual.rows, rows);
    assert_eq!(actual.rows[1..], csv.rows[1..]);
    let name = csv.get(&rows[0], "function").unwrap();
    assert_eq!(
        failure(f.run()),
        [
            "FAIL: 1 functions.csv problem(s)".to_owned(),
            format!("  - {name}: {diagnostic}")
        ]
    );
}

#[test]
fn fabricated_description_is_rejected() {
    changed_first_field(
        "description",
        "Fabricated description",
        "description differs from workbook",
    );
}

#[test]
fn fabricated_module_is_rejected() {
    changed_first_field("module", "FabricatedModule", "module differs from src/");
}

#[test]
fn missing_and_explicitly_empty_final_function_fields_both_report_missing_names() {
    let f = Fixture::published();
    let csv = Csv::read(&f.index).unwrap();
    let mut columns = csv
        .headers
        .iter()
        .filter(|s| *s != "function")
        .cloned()
        .collect::<Vec<_>>();
    columns.push("function".into());
    let mut rows = csv
        .rows
        .iter()
        .map(|row| {
            columns
                .iter()
                .map(|field| csv.get(row, field).unwrap().to_owned())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    rows[0].pop();
    *rows[1].last_mut().unwrap() = String::new();
    f.write_rows(&columns, &rows);
    let actual = Csv::read(&f.index).unwrap();
    assert_eq!(actual.rows, rows);
    assert_eq!(actual.rows[0].len(), columns.len() - 1);
    assert_eq!(actual.rows[1].last().unwrap(), "");
    let lines = failure(f.run());
    for row in &csv.rows[..2] {
        assert!(lines.contains(&format!(
            "  - {} is in the workbook but missing from functions.csv",
            csv.get(row, "function").unwrap()
        )));
    }
}

#[test]
fn repeated_consumed_columns_reject_all_eight_matching_and_contradictory_cases() {
    let f = Fixture::published();
    for field in ["function", "module", "signature", "description"] {
        for identical in [false, true] {
            f.columns(
                &[&[field][..], FIELDS].concat(),
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
    reason = "Retains the original six-case parsed-header contract table."
)]
fn all_six_parsed_header_cases_fail_before_mapping_rows() {
    let f = Fixture::published();
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
        f.columns(&columns, &overrides, empty, bom, quoted);
        duplicate(&f, &repeated);
    }
}

#[test]
fn all_five_ignored_column_reordering_and_bom_cases_remain_valid() {
    let f = Fixture::published();
    for (columns, overrides, bom) in [
        (
            [&["previous_name"][..], FIELDS].concat(),
            vec![(0, "ignored contradiction")],
            false,
        ),
        (
            [&["extra"][..], FIELDS, &["extra"]].concat(),
            vec![(0, "ignored contradiction")],
            false,
        ),
        (
            FIELDS
                .iter()
                .copied()
                .filter(|s| *s != "previous_name")
                .collect(),
            vec![],
            false,
        ),
        (
            [
                FIELDS.iter().rev().copied().collect::<Vec<_>>(),
                vec!["Function", "extra"],
            ]
            .concat(),
            vec![],
            false,
        ),
        (
            [
                FIELDS.iter().rev().copied().collect::<Vec<_>>(),
                vec!["Function", "extra"],
            ]
            .concat(),
            vec![],
            true,
        ),
    ] {
        f.columns(&columns, &overrides, false, bom, false);
        good(f.run());
    }
}

#[test]
fn uppercase_function_is_not_the_required_lowercase_field() {
    let f = Fixture::published();
    f.columns(
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
    let lines = failure(f.run());
    assert!(lines.iter().all(|s| !s.contains("duplicate CSV column")));
}

#[test]
fn published_workbook_source_and_index_match() {
    good(Fixture::published().run());
}

#[test]
fn both_csv_field_limit_failures_are_controlled_and_preserve_all_inputs() {
    let f = Fixture::published();
    let huge = "x".repeat(131_073);
    for text in [
        format!("{huge},function\n"),
        format!("function,module,signature,description\n{huge},,,\n"),
    ] {
        std::fs::write(&f.index, text).unwrap();
        assert_eq!(
            failure(f.run()),
            ["FAIL: cannot read index inputs: field larger than field limit (131072)"]
        );
    }
}

#[test]
fn physical_defaults_and_explicit_arguments_preserve_inputs_and_output_channels() {
    let f = Fixture::published();
    for length in 0..=3 {
        let output = f.cli(&f.args()[..length]);
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(output.stdout, format!("{GOOD}{}", newline()).into_bytes());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn physical_surplus_arguments_are_rejected_before_reading_or_writing() {
    let f = Fixture::published();
    let mut args = f.args();
    args.push(display(&f.dir.path().join("unrequested.xlsx")));
    let output = f.cli(&args);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        format!(
            "FAIL: usage: ozzit verify_index [workbook] [src dir] [functions.csv]{}",
            newline()
        )
        .into_bytes()
    );
}
