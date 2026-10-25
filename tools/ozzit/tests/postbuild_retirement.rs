//! Contracts retained from the cover-label, sheet-name and revision-strip Python tests.
use ozzit::{
    archive::{Parts, read_parts, write_deterministic},
    common::display,
    formula::MODULES,
    postbuild::{cover_label, sheet_names, strip_revision_history},
};
use std::{path::Path, process::Command};

const STRINGS: &str = "xl/sharedStrings.xml";
const OLD_LABELS: [&str; 4] = [
    "Version 20 August 2026    -    130 functions    -    Microsoft 365 or Excel 2024 and later",
    "Version 16 September 2026    -    133 functions    -    Microsoft 365 or Excel 2024 and later",
    "Version 16 September 2026    -    137 functions    -    Microsoft 365 or Excel 2024 and later",
    "Version 19 October 2026    -    143 functions    -    Microsoft 365 or Excel 2024 and later",
];
const LABEL: &str =
    "Version 19 October 2026    -    148 functions    -    Microsoft 365 or Excel 2024 and later";
const OLD_CREATOR: &str = "<dc:creator>Ryan Duguid; Ozzit project</dc:creator>";
const CREATOR: &str = "<dc:creator>Ozzit project</dc:creator>";

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
}
fn strings(label: &str) -> Parts {
    [(
        STRINGS.into(),
        format!("<sst><si><t>{label}</t></si></sst>").into_bytes(),
    )]
    .into()
}
fn names(name: &str, links: usize, absolute: bool) -> Parts {
    let prefix = if absolute { "/xl/" } else { "" };
    [
        ("xl/workbook.xml", format!("<workbook><sheets><sheet name=\"TOC\" sheetId=\"1\" r:id=\"rId1\"/><sheet name=\"{name}\" sheetId=\"2\" r:id=\"rId2\"/></sheets></workbook>")),
        ("xl/_rels/workbook.xml.rels", format!("<Relationships><Relationship Id=\"rId1\" Type=\"ws\" Target=\"{prefix}worksheets/sheet1.xml\"/><Relationship Id=\"rId2\" Type=\"ws\" Target=\"worksheets/sheet2.xml\"/></Relationships>")),
        ("xl/worksheets/sheet1.xml", format!("<f>'{name}'!A1</f>").repeat(links)),
        ("xl/worksheets/sheet2.xml", format!("<v>{name}</v>")),
        ("docProps/app.xml", format!("<vt:lpstr>{name}</vt:lpstr>")),
        (STRINGS, format!("<t>{name}</t>")),
    ].into_iter().map(|(k,v)| (k.into(), v.into_bytes())).collect()
}

struct Fixture {
    directory: tempfile::TempDir,
}
impl Fixture {
    fn new() -> Self {
        Self {
            directory: tempfile::tempdir().unwrap(),
        }
    }
    fn book(&self) -> std::path::PathBuf {
        self.directory.path().join("ozzit.xlsx")
    }
    fn write(&self, parts: &Parts) {
        write_deterministic(&self.book(), parts).unwrap();
    }
    fn run(&self, pass: &str, args: &[String]) -> (i32, String, String) {
        let result = Command::new(env!("CARGO_BIN_EXE_ozzit"))
            .arg(pass)
            .args(args)
            .current_dir(self.directory.path())
            .output()
            .unwrap();
        (
            result.status.code().unwrap(),
            String::from_utf8(result.stdout)
                .unwrap()
                .replace("\r\n", "\n"),
            String::from_utf8(result.stderr)
                .unwrap()
                .replace("\r\n", "\n"),
        )
    }
    fn copy_revision_stores(&self, creator: &str) -> std::path::PathBuf {
        std::fs::copy(root().join("ozzit.xlsx"), self.book()).unwrap();
        let mut parts = read_parts(&self.book()).unwrap();
        let core = String::from_utf8(parts["docProps/core.xml"].clone()).unwrap();
        assert_eq!(core.matches(CREATOR).count(), 1);
        parts.insert(
            "docProps/core.xml".into(),
            core.replace(CREATOR, creator).into_bytes(),
        );
        self.write(&parts);
        let src = self.directory.path().join("src");
        std::fs::create_dir(&src).unwrap();
        for module in MODULES {
            std::fs::copy(
                root().join(format!("src/{module}.txt")),
                src.join(format!("{module}.txt")),
            )
            .unwrap();
        }
        src
    }
}

#[test]
fn every_earlier_cover_label_moves_to_the_current_wording() {
    let f = Fixture::new();
    for old in OLD_LABELS {
        f.write(&strings(old));
        assert_eq!(cover_label::run(&f.book()).unwrap(), ["workbook"]);
        let parts = read_parts(&f.book()).unwrap();
        assert_eq!(parts, strings(LABEL));
        let text = std::str::from_utf8(&parts[STRINGS]).unwrap();
        assert!(OLD_LABELS.iter().all(|old| !text.contains(old)));
    }
}

#[test]
fn the_current_cover_label_is_a_byte_noop() {
    let f = Fixture::new();
    f.write(&strings(LABEL));
    let before = std::fs::read(f.book()).unwrap();
    assert!(cover_label::run(&f.book()).unwrap().is_empty());
    assert_eq!(std::fs::read(f.book()).unwrap(), before);
}

#[test]
fn ambiguous_and_unrecognised_cover_labels_are_refused_without_writing() {
    let f = Fixture::new();
    for text in [
        format!("{LABEL}</t><t>{}", OLD_LABELS[1]),
        LABEL.replace("148", "999"),
    ] {
        f.write(&strings(&text));
        let before = std::fs::read(f.book()).unwrap();
        assert!(
            cover_label::run(&f.book())
                .unwrap_err()
                .starts_with("xl/sharedStrings.xml: expected exactly one cover label, got ")
        );
        assert_eq!(std::fs::read(f.book()).unwrap(), before);
    }
}

#[test]
fn cover_label_cli_reports_the_write_and_subsequent_noop() {
    let f = Fixture::new();
    f.write(&strings(OLD_LABELS[0]));
    let args = [display(&f.book())];
    assert_eq!(
        f.run("cover_label", &args),
        (
            0,
            format!("applied cover label to: workbook\nOK: {}\n", args[0]),
            String::new()
        )
    );
    assert_eq!(read_parts(&f.book()).unwrap(), strings(LABEL));
    assert_eq!(
        f.run("cover_label", &args),
        (
            0,
            format!("cover label already applied; no changes\nOK: {}\n", args[0]),
            String::new()
        )
    );
}

#[test]
fn a_cover_without_shared_strings_has_a_controlled_failure() {
    let f = Fixture::new();
    f.write(&[("xl/workbook.xml".into(), b"<workbook/>".to_vec())].into());
    assert_eq!(
        f.run("cover_label", &[display(&f.book())]),
        (
            1,
            String::new(),
            "FAIL: missing part xl/sharedStrings.xml\n".into()
        )
    );
}

#[test]
fn sheet_names_renames_every_part_and_then_leaves_the_bytes_alone() {
    let f = Fixture::new();
    f.write(&names("oz.SumContains", 4, false));
    assert_eq!(sheet_names::run(&f.book()).unwrap(), ["workbook"]);
    assert_eq!(
        read_parts(&f.book()).unwrap(),
        names("oz.SumContainsλ", 4, false)
    );
    let before = std::fs::read(f.book()).unwrap();
    assert!(sheet_names::run(&f.book()).unwrap().is_empty());
    assert_eq!(std::fs::read(f.book()).unwrap(), before);
}

#[test]
fn sheet_names_resolves_absolute_and_relative_worksheet_targets() {
    let f = Fixture::new();
    f.write(&names("oz.SumContains", 4, true));
    assert_eq!(sheet_names::run(&f.book()).unwrap(), ["workbook"]);
    assert_eq!(
        read_parts(&f.book()).unwrap(),
        names("oz.SumContainsλ", 4, true)
    );
}

#[test]
fn sheet_names_requires_the_toc() {
    let f = Fixture::new();
    let mut parts = names("oz.SumContains", 4, false);
    let book = String::from_utf8(parts["xl/workbook.xml"].clone()).unwrap();
    parts.insert(
        "xl/workbook.xml".into(),
        book.replace("name=\"TOC\"", "name=\"Cover\"").into_bytes(),
    );
    f.write(&parts);
    let before = std::fs::read(f.book()).unwrap();
    assert_eq!(
        sheet_names::run(&f.book()).unwrap_err(),
        "the workbook lacks the TOC sheet or the oz.SumContains sheet"
    );
    assert_eq!(std::fs::read(f.book()).unwrap(), before);
}

#[test]
fn sheet_names_reports_every_missing_part_and_wrong_anchor_count_before_writing() {
    let f = Fixture::new();
    let mut parts = names("oz.SumContains", 3, false);
    parts.shift_remove("docProps/app.xml");
    f.write(&parts);
    let before = std::fs::read(f.book()).unwrap();
    assert_eq!(
        sheet_names::run(&f.book()).unwrap_err(),
        "xl/worksheets/sheet1.xml: \"'oz.SumContains'!\" expected 4 recognised anchors, got pre-rename=3 renamed=0; missing part docProps/app.xml"
    );
    assert_eq!(std::fs::read(f.book()).unwrap(), before);
}

#[test]
fn sheet_names_cli_reports_the_rename() {
    let f = Fixture::new();
    f.write(&names("oz.SumContains", 4, false));
    assert_eq!(
        f.run("sheet_names", &[display(&f.book())]),
        (
            0,
            format!(
                "applied sheet names to: workbook\nOK: {}\n",
                display(&f.book())
            ),
            String::new()
        )
    );
}

#[test]
fn all_three_commands_reject_extra_arguments_and_missing_workbooks() {
    let f = Fixture::new();
    for (pass, usage, count) in [
        ("cover_label", "ozzit cover_label [workbook]", 2),
        ("sheet_names", "ozzit sheet_names [workbook]", 2),
        (
            "strip_revision_history",
            "ozzit strip_revision_history [workbook] [src dir]",
            3,
        ),
    ] {
        assert_eq!(
            f.run(pass, &vec!["extra".into(); count]),
            (1, String::new(), format!("FAIL: usage: {usage}\n"))
        );
        assert_eq!(
            f.run(pass, &[display(&f.book())]),
            (
                1,
                String::new(),
                format!("FAIL: no such workbook: {}\n", display(&f.book()))
            )
        );
    }
}

#[test]
fn invalid_archives_fail_without_a_traceback_or_a_write() {
    let f = Fixture::new();
    std::fs::write(f.book(), b"not a zip archive").unwrap();
    for pass in ["cover_label", "sheet_names"] {
        assert_eq!(
            f.run(pass, &[display(&f.book())]),
            (1, String::new(), "FAIL: File is not a zip file\n".into())
        );
        assert_eq!(std::fs::read(f.book()).unwrap(), b"not a zip archive");
    }
}

#[test]
fn revision_strip_cli_rewrites_the_old_creator_and_reports_the_size() {
    let f = Fixture::new();
    let src = f.copy_revision_stores(OLD_CREATOR);
    let result = f.run(
        "strip_revision_history",
        &[display(&f.book()), display(&src)],
    );
    assert_eq!(result.0, 0, "{result:?}");
    assert!(result.2.is_empty());
    let size = ozzit::sanitise::thousands(std::fs::metadata(f.book()).unwrap().len());
    assert_eq!(
        result.1,
        format!(
            "rewrote docProps/core.xml creator\nOK: {} stripped, {size} bytes\n",
            display(&f.book())
        )
    );
    assert_eq!(
        read_parts(&f.book()).unwrap()["docProps/core.xml"],
        read_parts(&root().join("ozzit.xlsx")).unwrap()["docProps/core.xml"]
    );
}

#[test]
fn an_unknown_creator_restores_sources_and_leaves_the_workbook_unchanged() {
    let f = Fixture::new();
    let src = f.copy_revision_stores("<dc:creator>Someone else</dc:creator>");
    let dates = src.join("Dates.txt");
    let mut before_dates = std::fs::read(&dates).unwrap();
    before_dates.extend(
        "/*\r\nREVISIONS: synthetic test block\r\n*/\r\n"
            .repeat(13)
            .as_bytes(),
    );
    std::fs::write(&dates, &before_dates).unwrap();
    let before_book = std::fs::read(f.book()).unwrap();
    assert_eq!(f.run("strip_revision_history", &[display(&f.book()), display(&src)]), (1, String::new(), "FAIL: cannot strip revision history: docProps/core.xml carries neither the old nor the new creator\n".into()));
    assert_eq!(std::fs::read(dates).unwrap(), before_dates);
    assert_eq!(std::fs::read(f.book()).unwrap(), before_book);
}

#[test]
fn revision_comment_removal_preserves_unterminated_text_and_line_endings() {
    for (source, expected, blocks) in [
        ("a /* REVISIONS: open", "a /* REVISIONS: open", 0),
        ("x\n/*\nREVISIONS: y\n*/\nz", "x\nz", 1),
        ("x\n/*\nREVISIONS: y\n*/", "x\n", 1),
        (
            "/* Keep this.\r\n REVISIONS:\r\n Old history.\r\n*/\r\nFixture = 1;\r\n",
            "/* Keep this.\r\n*/\r\nFixture = 1;\r\n",
            1,
        ),
    ] {
        assert_eq!(
            strip_revision_history::strip_module(source),
            (expected.into(), blocks)
        );
    }
}
