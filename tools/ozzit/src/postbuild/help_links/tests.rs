//! Recovery faults use local callbacks and owned copies of the published stores.
use super::*;
use std::{collections::BTreeMap, path::PathBuf};

struct Fixture {
    _directory: tempfile::TempDir,
    workbook: PathBuf,
    src: py::PyPath,
}
impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let workbook = directory.path().join("ozzit.xlsx");
        std::fs::copy(root.join("ozzit.xlsx"), &workbook).unwrap();
        let source = directory.path().join("src");
        std::fs::create_dir(&source).unwrap();
        for module in MODULES {
            std::fs::copy(
                root.join(format!("src/{module}.txt")),
                source.join(format!("{module}.txt")),
            )
            .unwrap();
        }
        let mut parts = read_parts(&workbook).unwrap();
        let mut book = read_book(&parts).unwrap();
        for (old, new) in forms("workbook") {
            assert_eq!(count(&book, &new), 1);
            book = book.replace(&new, &old);
        }
        parts.insert(BOOK.into(), book.into_bytes());
        write_deterministic(&workbook, &parts).unwrap();
        let ratios_path = source.join("Ratios.txt");
        let mut ratios = read_text(&ratios_path).unwrap();
        for (old, new) in forms("src") {
            assert_eq!(count(&ratios, &new), 1);
            ratios = ratios.replace(&new, &old);
        }
        std::fs::write(ratios_path, ratios).unwrap();

        // A repacked ZIP cannot satisfy the original-byte rollback assertion.
        let mut original = std::fs::read(&workbook).unwrap();
        let end = original.len();
        assert_eq!(&original[end - 2..], &[0, 0]);
        let comment = b"original archive";
        original[end - 2..].copy_from_slice(&(comment.len() as u16).to_le_bytes());
        original.extend(comment);
        std::fs::write(&workbook, original).unwrap();
        Self {
            _directory: directory,
            workbook,
            src: py::PyPath::new(&display(&source)),
        }
    }
    fn snapshot(&self) -> BTreeMap<String, Vec<u8>> {
        let mut files =
            BTreeMap::from([("ozzit.xlsx".into(), read_bytes(&self.workbook).unwrap())]);
        for module in MODULES {
            let name = format!("{module}.txt");
            files.insert(
                name.clone(),
                read_bytes(&self.src.join(&name).fs()).unwrap(),
            );
        }
        files
    }
}

fn check_source_failure(restore_fails: bool) {
    let f = Fixture::new();
    let before = f.snapshot();
    let mut calls = Vec::new();
    let mut write = |path: &Path, text: &str| {
        assert_eq!(path, f.src.join("Ratios.txt").fs());
        calls.push(basename(path));
        if calls.len() == 1 {
            assert_ne!(read_bytes(&f.workbook).unwrap(), before["ozzit.xlsx"]);
            return Err("synthetic failure on the Ratios write".into());
        }
        // Workbook recovery must precede even an unsuccessful source restore.
        assert_eq!(read_bytes(&f.workbook).unwrap(), before["ozzit.xlsx"]);
        assert_eq!(text.as_bytes(), before["Ratios.txt"]);
        if restore_fails {
            return Err("synthetic failure on the Ratios restore".into());
        }
        write_text(path, text)
    };
    let expected = if restore_fails {
        "synthetic failure on the Ratios restore"
    } else {
        "synthetic failure on the Ratios write"
    };
    assert_eq!(
        run_with(&f.workbook, &f.src, &mut write),
        Err(expected.into())
    );
    assert_eq!(calls, ["Ratios.txt", "Ratios.txt"]);
    assert_eq!(f.snapshot(), before);
}

#[test]
fn a_failed_ratios_restore_still_restores_original_workbook_bytes_first() {
    check_source_failure(true);
}

#[test]
fn a_failed_second_write_restores_both_stores() {
    check_source_failure(false);
}
