//! Physical help-cache contracts supplement the unchanged model fixture replay.
use base64::{Engine, engine::general_purpose::STANDARD};
use ozzit::{archive::*, common::*, spills};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn newline() -> &'static str {
    if cfg!(windows) { "\r\n" } else { "\n" }
}

struct Fixture {
    dir: tempfile::TempDir,
    book: PathBuf,
}
impl Fixture {
    fn published() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let book = dir.path().join("ozzit.xlsx");
        std::fs::copy(root().join("ozzit.xlsx"), &book).unwrap();
        Self { dir, book }
    }
    fn write(&self, parts: &Parts) {
        write_deterministic(&self.book, parts).unwrap();
    }
    fn snapshot(&self) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
        fn visit(root: &Path, path: &Path, result: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
            for entry in std::fs::read_dir(path).unwrap() {
                let entry = entry.unwrap();
                assert!(!entry.file_type().unwrap().is_symlink());
                let path = entry.path();
                let bytes = if path.is_dir() {
                    visit(root, &path, result);
                    None
                } else {
                    Some(std::fs::read(&path).unwrap())
                };
                result.insert(path.strip_prefix(root).unwrap().into(), bytes);
            }
        }
        let mut result = BTreeMap::new();
        visit(self.dir.path(), self.dir.path(), &mut result);
        result
    }
    fn cli(&self, args: &[String], encoding: &str) -> std::process::Output {
        let before = self.snapshot();
        let result = Command::new(env!("CARGO_BIN_EXE_ozzit"))
            .arg("verify_help_spills")
            .args(args)
            .env("PYTHONIOENCODING", encoding)
            .current_dir(self.dir.path())
            .output()
            .unwrap();
        assert_eq!(self.snapshot(), before);
        result
    }
    fn run(&self) -> std::process::Output {
        self.cli(&[display(&self.book)], "utf-8")
    }
}

fn frozen_parts(index: usize) -> Parts {
    let bytes = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/verifier-contracts.json.gz"),
    )
    .unwrap();
    let data: Value = serde_json::from_reader(flate2::read::GzDecoder::new(&bytes[..])).unwrap();
    let record = &data["records"][index];
    assert_eq!(record["kind"], "spills");
    record["parts"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(name, bytes)| {
            (
                name.clone(),
                STANDARD.decode(bytes.as_str().unwrap()).unwrap(),
            )
        })
        .collect()
}

#[test]
fn the_original_trim_collapses_ordinary_spaces() {
    assert_eq!(spills::trim("  a   b  "), "a b");
}

#[test]
fn both_original_stale_archives_and_the_current_archive_keep_their_channels() {
    for reordered in [false, true] {
        let f = Fixture::published();
        let mut parts = frozen_parts(8);
        assert_eq!(parts.len(), 3);
        if reordered {
            let name = "xl/worksheets/sheet2.xml";
            let sheet = std::str::from_utf8(&parts[name]).unwrap();
            let before = "<f t=\"array\" ref=\"A4:B5\">";
            assert_eq!(sheet.matches(before).count(), 1);
            let after = sheet.replace(before, "<f ref=\"A4:B5\" t=\"array\">");
            parts.insert(name.into(), after.into_bytes());
        }
        f.write(&parts);
        let output = f.run();
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(
            output.stdout,
            format!(
                "xl/worksheets/sheet2.xml: the oz.Gλ help is stale, first difference at B4{}",
                newline()
            )
            .into_bytes()
        );
        assert_eq!(
            output.stderr,
            format!("FAIL: 1 of 2 cached helps in {} no longer match their definitions; run ozzit refresh_cache in Excel, then ozzit sanitise_workbook, and ozzit verify_cache proves the result{}", display(&f.book), newline()).into_bytes()
        );
    }
    let f = Fixture::published();
    let parts = frozen_parts(6);
    assert_eq!(parts.len(), 2);
    f.write(&parts);
    let output = f.run();
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(
        output.stdout,
        format!(
            "OK: every cached help in {} matches its definition, 1 anchors{}",
            display(&f.book),
            newline()
        )
        .into_bytes()
    );
}

#[test]
fn the_original_zero_anchor_workbook_is_refused_without_a_writer() {
    let f = Fixture::published();
    let parts = frozen_parts(12);
    assert_eq!(spills::check(&parts).unwrap().1, 0);
    f.write(&parts);
    let output = f.run();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        format!(
            "FAIL: no help anchor found in {}, so no cached help was checked{}",
            display(&f.book),
            newline()
        )
        .into_bytes()
    );
}

#[test]
fn physical_defaults_and_explicit_unicode_paths_force_utf8_and_preserve_inputs() {
    let f = Fixture::published();
    let explicit = f.dir.path().join("help-λ.xlsx");
    std::fs::copy(&f.book, &explicit).unwrap();
    for encoding in ["cp1252", "utf-8"] {
        for (args, path) in [
            (vec![], "ozzit.xlsx".into()),
            (vec![display(&explicit)], display(&explicit)),
        ] {
            let output = f.cli(&args, encoding);
            assert_eq!(output.status.code(), Some(0));
            assert!(output.stderr.is_empty());
            assert_eq!(
                output.stdout,
                format!(
                    "OK: every cached help in {path} matches its definition, 43 anchors{}",
                    newline()
                )
                .into_bytes()
            );
        }
    }
}

#[test]
fn physical_surplus_arguments_fail_before_accessing_inputs() {
    let f = Fixture::published();
    let output = f.cli(&["a.xlsx".into(), "b.xlsx".into()], "cp1252");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        format!(
            "FAIL: usage: ozzit verify_help_spills [workbook]{}",
            newline()
        )
        .into_bytes()
    );
}

#[test]
fn both_original_unreadable_paths_keep_their_exact_errors_and_inputs() {
    let f = Fixture::published();
    let missing = f.dir.path().join("missing.xlsx");
    let not_zip = f.dir.path().join("not-zip.xlsx");
    std::fs::write(&not_zip, b"not a zip archive").unwrap();
    for (path, error) in [
        (
            &missing,
            format!("FAIL: no such workbook: {}", display(&missing)),
        ),
        (&not_zip, "FAIL: File is not a zip file".into()),
    ] {
        let output = f.cli(&[display(path)], "utf-8");
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, format!("{error}{}", newline()).into_bytes());
    }
}
