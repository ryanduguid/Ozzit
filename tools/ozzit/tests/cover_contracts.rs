//! Native counterparts of the original cover gate fixtures and command contracts.
use ozzit::{archive::*, common::*, cover};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
};

const LABEL: &str = "Version 1 May 2026 - 3 functions - Excel";
const CELL: &str = "<c r=\"A3\" t=\"s\"><v>0</v></c>";
const GOOD: &str = "OK: cover label states 19 October 2026 and 148 functions";
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn newline() -> &'static str {
    if cfg!(windows) { "\r\n" } else { "\n" }
}

fn small_parts() -> Parts {
    [
        (BOOK, "<workbook><sheets><sheet name=\"Cover\" sheetId=\"1\" r:id=\"rId1\"/></sheets></workbook>".to_owned()),
        ("xl/_rels/workbook.xml.rels", "<Relationships><Relationship Id=\"rId1\" Type=\"t\" Target=\"worksheets/sheet1.xml\"/></Relationships>".to_owned()),
        ("xl/worksheets/sheet1.xml", format!("<worksheet><sheetData><row r=\"3\">{CELL}</row></sheetData></worksheet>")),
        ("xl/sharedStrings.xml", format!("<sst><si><t>{LABEL}</t></si></sst>")),
    ].into_iter().map(|(name, text)| (name.to_owned(), text.into_bytes())).collect()
}

struct Fixture {
    dir: tempfile::TempDir,
    book: PathBuf,
    index: PathBuf,
    changelog: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let book = dir.path().join("ozzit.xlsx");
        let index = dir.path().join("functions.csv");
        let changelog = dir.path().join("CHANGELOG.md");
        Self {
            dir,
            book,
            index,
            changelog,
        }
    }
    fn published() -> Self {
        let f = Self::new();
        for path in [&f.book, &f.index, &f.changelog] {
            std::fs::copy(root().join(path.file_name().unwrap()), path).unwrap();
        }
        f
    }
    fn small() -> Self {
        let f = Self::new();
        let parts = small_parts();
        assert_eq!(parts.len(), 4);
        write_deterministic(&f.book, &parts).unwrap();
        assert_eq!(read_parts(&f.book).unwrap(), parts);
        std::fs::write(&f.index, "function\noz.Aλ\noz.Bλ\noz.Cλ\noz.AboutDatesλ\n").unwrap();
        std::fs::write(
            &f.changelog,
            "# Changelog\n\n## v1.0.0, 1 May 2026, first cut\n",
        )
        .unwrap();
        f
    }
    fn snapshot(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        std::fs::read_dir(self.dir.path())
            .unwrap()
            .map(|entry| {
                let path = entry.unwrap().path();
                let bytes = std::fs::read(&path).unwrap();
                (path, bytes)
            })
            .collect()
    }
    fn args(&self) -> Vec<String> {
        [&self.book, &self.index, &self.changelog]
            .map(|p| display(p))
            .to_vec()
    }
    fn run_args(&self, args: &[String]) -> Output {
        let before = self.snapshot();
        let output = cover::verify(args);
        assert_eq!(self.snapshot(), before);
        output
    }
    fn run(&self) -> Output {
        self.run_args(&self.args())
    }
    fn cli(&self, args: &[&str]) -> std::process::Output {
        let before = self.snapshot();
        let output = Command::new(env!("CARGO_BIN_EXE_ozzit"))
            .current_dir(self.dir.path())
            .arg("verify_cover")
            .args(args)
            .output()
            .unwrap();
        assert_eq!(self.snapshot(), before);
        output
    }
    fn rewrite(&self, part: &str, old: &str, new: &str) {
        let mut parts = read_parts(&self.book).unwrap();
        let text = text(&parts, part).unwrap();
        assert_eq!(text.matches(old).count(), 1, "{part}: {old}");
        parts.insert(part.into(), text.replace(old, new).into_bytes());
        write_deterministic(&self.book, &parts).unwrap();
    }
    fn columns(&self, columns: &[&str], overrides: &[(usize, &str)], empty: bool, bom: bool) {
        let csv = Csv::read(&root().join("functions.csv")).unwrap();
        let mut text = if bom {
            "\u{feff}".to_owned()
        } else {
            String::new()
        };
        text += &csv_row(&columns.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>());
        if !empty {
            for row in &csv.rows {
                let mut values = columns
                    .iter()
                    .map(|field| csv.get(row, field).unwrap_or("extra,\nvalue").to_owned())
                    .collect::<Vec<_>>();
                for &(position, value) in overrides {
                    values[position] = value.to_owned();
                }
                text += &csv_row(&values);
            }
        }
        std::fs::write(&self.index, text).unwrap();
    }
}

fn good(output: Output, expected: &str) {
    assert_eq!(output.code, 0, "{:?} {:?}", output.out, output.err);
    assert_eq!(output.out, [expected]);
    assert!(output.err.is_empty(), "{:?}", output.err);
}
fn failure(output: Output, expected: &[&str]) {
    assert_eq!(output.code, 1);
    assert_eq!(output.out, expected);
    assert!(output.err.is_empty(), "{:?}", output.err);
}

#[test]
fn repeated_function_headers_fail_before_counting_rows() {
    let f = Fixture::published();
    for (columns, overrides, empty) in [
        (vec!["function", "function"], vec![(0, "")], false),
        (vec!["function", "function"], vec![], false),
        (
            vec!["function", "signature", "function"],
            vec![(2, "")],
            false,
        ),
        (vec!["function", "function", "function"], vec![], false),
        (vec!["function", "function"], vec![], true),
    ] {
        f.columns(&columns, &overrides, empty, false);
        failure(
            f.run(),
            &["FAIL: functions.csv: duplicate CSV column(s): function"],
        );
    }
}

#[test]
fn ignored_columns_and_minimal_reordered_indexes_remain_usable() {
    let f = Fixture::published();
    for (columns, overrides, bom) in [
        (vec!["signature", "function"], vec![], false),
        (vec!["signature", "function"], vec![], true),
        (
            vec!["description", "function", "description"],
            vec![(0, "ignored contradiction")],
            false,
        ),
        (
            vec!["extra", "function", "extra"],
            vec![(0, "ignored contradiction")],
            false,
        ),
        (vec!["Function", "extra", "function"], vec![], true),
    ] {
        f.columns(&columns, &overrides, false, bom);
        good(f.run(), GOOD);
    }
}

#[test]
fn committed_cover_matches_index_and_changelog() {
    good(Fixture::published().run(), GOOD);
}

#[test]
fn stale_function_count_fails() {
    let f = Fixture::published();
    f.rewrite("xl/sharedStrings.xml", "148 functions", "133 functions");
    failure(
        f.run(),
        &["FAIL: cover says 133 functions; functions.csv publishes 148"],
    );
}

#[test]
fn current_looking_decoy_does_not_rescue_the_stale_displayed_cell() {
    let f = Fixture::published();
    let current = "Version 19 October 2026    -    148 functions    -    Microsoft 365 or Excel 2024 and later";
    let stale = current
        .replace("19 October 2026", "20 August 2026")
        .replace("148", "133");
    f.rewrite(
        "xl/sharedStrings.xml",
        &format!("<t>{current}</t>"),
        &format!("<t>{stale}</t>"),
    );
    f.rewrite(
        "xl/sharedStrings.xml",
        "</sst>",
        &format!("<si><t>{current}</t></si></sst>"),
    );
    failure(
        f.run(),
        &[
            "FAIL: cover says 133 functions; functions.csv publishes 148",
            "FAIL: cover is dated 20 August 2026; the changelog's current cut is 19 October 2026",
        ],
    );
}

#[test]
fn byte_order_marked_index_is_read() {
    let f = Fixture::published();
    let bytes = std::fs::read(&f.index).unwrap();
    std::fs::write(&f.index, [b"\xef\xbb\xbf".as_slice(), &bytes].concat()).unwrap();
    good(f.run(), GOOD);
}

#[test]
fn missing_function_names_fail_without_a_false_count() {
    let f = Fixture::published();
    let csv = Csv::read(&f.index).unwrap();
    let mut names = csv
        .rows
        .iter()
        .map(|row| csv.get(row, "function").unwrap())
        .collect::<Vec<_>>();
    let removed = names
        .iter()
        .position(|name| !name.starts_with("oz.About"))
        .unwrap();
    names.remove(removed);
    assert_eq!(names.len(), 152);
    for missing in [None, Some(""), Some("   ")] {
        let mut text = "signature,function\r\n".to_owned();
        for name in &names {
            text += &csv_row(&["LAMBDA()".into(), (*name).into()]);
        }
        text += &csv_row(&missing.map_or_else(
            || vec!["LAMBDA()".into()],
            |value| vec!["LAMBDA()".into(), value.into()],
        ));
        std::fs::write(&f.index, text).unwrap();
        failure(
            f.run(),
            &["FAIL: functions.csv contains an empty function name"],
        );
    }
}

#[test]
fn stale_changelog_date_fails() {
    let f = Fixture::published();
    let text = std::fs::read_to_string(&f.changelog).unwrap();
    assert!(text.contains("19 October 2026"));
    std::fs::write(
        &f.changelog,
        text.replacen("19 October 2026", "20 October 2026", 1),
    )
    .unwrap();
    failure(
        f.run(),
        &["FAIL: cover is dated 19 October 2026; the changelog's current cut is 20 October 2026"],
    );
}

#[test]
fn label_is_resolved_through_absolute_target_and_rich_text() {
    let f = Fixture::small();
    f.rewrite(
        "xl/_rels/workbook.xml.rels",
        "worksheets/sheet1.xml",
        "/xl/worksheets/sheet1.xml",
    );
    f.rewrite("xl/sharedStrings.xml", &format!("<sst><si><t>{LABEL}</t></si></sst>"),
        "<sst><si><r><t>Version 1 May 2026</t></r><r><t xml:space=\"preserve\"> - 3 functions - Excel &amp; more</t></r></si></sst>");
    let before = f.snapshot();
    assert_eq!(
        cover::cover_label(&f.book).unwrap(),
        ("1 May 2026".into(), 3.into())
    );
    assert_eq!(f.snapshot(), before);
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "Retains all 7 exact original reference diagnostics."
)]
fn all_seven_unresolvable_cover_cells_have_the_original_exact_diagnostics() {
    let f = Fixture::small();
    for (part, old, new, expected) in [
        (
            BOOK,
            "name=\"Cover\"",
            "name=\"Data\"",
            "xl/workbook.xml declares no sheet named Cover",
        ),
        (
            "xl/_rels/workbook.xml.rels",
            "Id=\"rId1\"",
            "Id=\"rId9\"",
            "the Cover sheet has no relationship target",
        ),
        (
            "xl/worksheets/sheet1.xml",
            CELL,
            "<c r=\"A3\"><v>0</v></c>",
            "xl/worksheets/sheet1.xml: Cover!A3 is not a shared-string cell",
        ),
        (
            "xl/worksheets/sheet1.xml",
            CELL,
            "<c r=\"B3\" t=\"s\"><v>0</v></c>",
            "xl/worksheets/sheet1.xml: Cover!A3 is not a shared-string cell",
        ),
        (
            "xl/worksheets/sheet1.xml",
            CELL,
            "<c r=\"A3\" t=\"s\"></c>",
            "xl/worksheets/sheet1.xml: Cover!A3 carries no shared-string index",
        ),
        (
            "xl/worksheets/sheet1.xml",
            "<v>0</v>",
            "<v>5</v>",
            "Cover!A3 points at shared string 5, beyond the 1 stored",
        ),
        (
            "xl/sharedStrings.xml",
            LABEL,
            "Hello",
            "Cover!A3 is not a version label: 'Hello'",
        ),
    ] {
        write_deterministic(&f.book, &small_parts()).unwrap();
        f.rewrite(part, old, new);
        let before = f.snapshot();
        assert_eq!(cover::cover_label(&f.book).unwrap_err(), expected);
        assert_eq!(f.snapshot(), before);
        failure(f.run(), &[&format!("FAIL: {expected}")]);
    }
}

#[test]
fn empty_index_and_headingless_changelog_are_refused_exactly() {
    let f = Fixture::small();
    std::fs::write(&f.index, "function,signature\n").unwrap();
    std::fs::write(&f.changelog, "# Changelog\n\nNothing released yet.\n").unwrap();
    let before = f.snapshot();
    assert_eq!(
        cover::published_functions(&f.index).unwrap_err(),
        "functions.csv lists no functions"
    );
    assert_eq!(
        cover::current_cut_date(&f.changelog).unwrap_err(),
        "CHANGELOG.md has no '## vX.Y.Z, <date>,' heading"
    );
    assert_eq!(f.snapshot(), before);
}

#[test]
fn wrapper_reports_exact_small_success_and_an_unreadable_workbook() {
    let f = Fixture::small();
    good(f.run(), "OK: cover label states 1 May 2026 and 3 functions");
    let mut args = f.args();
    args[0] = display(&f.dir.path().join("missing.xlsx"));
    let output = f.run_args(&args);
    assert_eq!(output.code, 1);
    assert!(output.err.is_empty());
    assert_eq!(output.out.len(), 1);
    assert!(
        output.out[0].starts_with("FAIL: ") && output.out[0].contains("missing.xlsx"),
        "{:?}",
        output.out
    );
    assert!(!output.out[0].contains("Traceback") && !output.out[0].contains("OK:"));
}

#[test]
fn oversized_csv_headers_and_rows_fail_with_a_message_and_preserve_inputs() {
    let f = Fixture::published();
    let oversized = "x".repeat(131_073);
    for text in [
        format!("{oversized},function\n"),
        format!("function,module,signature,description\n{oversized},,,\n"),
    ] {
        std::fs::write(&f.index, text).unwrap();
        failure(f.run(), &["FAIL: field larger than field limit (131072)"]);
    }
}

#[test]
fn physical_command_defaults_and_explicit_inputs_preserve_path_policy() {
    let f = Fixture::published();
    for args in [
        vec![],
        vec!["ozzit.xlsx"],
        vec!["ozzit.xlsx", "functions.csv"],
        vec!["ozzit.xlsx", "functions.csv", "CHANGELOG.md"],
    ] {
        let output = f.cli(&args);
        assert_eq!(output.status.code(), Some(0), "{:?}", output);
        assert!(output.stderr.is_empty());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            GOOD.to_owned() + newline()
        );
    }
}

#[test]
fn physical_command_rejects_surplus_inputs_before_reading_or_writing() {
    let f = Fixture::new();
    let output = f.cli(&[
        "ozzit.xlsx",
        "functions.csv",
        "CHANGELOG.md",
        "unrequested.xlsx",
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        format!(
            "FAIL: usage: ozzit verify_cover [workbook] [functions.csv] [CHANGELOG.md]{}",
            newline()
        )
    );
    assert!(f.snapshot().is_empty());
}
