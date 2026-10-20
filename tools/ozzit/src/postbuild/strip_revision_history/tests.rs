//! Faults use the production transaction with local closures and owned copies.
use super::*;
use std::collections::BTreeMap;
use std::path::PathBuf;

const BLOCK: &str = "/*\r\nREVISIONS: synthetic test block\r\n*/\r\n";
const ONE_EACH: [(&str, usize); 6] = [
    ("Dates", 1),
    ("Essentials", 1),
    ("Financial", 1),
    ("Ratios", 1),
    ("Utilities", 1),
    ("Debt", 1),
];

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
        let mut original = std::fs::read(root.join("ozzit.xlsx")).unwrap();
        // A noncanonical ZIP comment makes a repack insufficient as a rollback.
        let comment = b"original archive";
        let end = original.len();
        assert_eq!(&original[end - 2..], &[0, 0]);
        original[end - 2..].copy_from_slice(&(comment.len() as u16).to_le_bytes());
        original.extend(comment);
        std::fs::write(&workbook, original).unwrap();
        let source = directory.path().join("src");
        std::fs::create_dir(&source).unwrap();
        for (module, _) in ONE_EACH {
            let mut bytes = std::fs::read(root.join(format!("src/{module}.txt"))).unwrap();
            bytes.extend(BLOCK.as_bytes());
            std::fs::write(source.join(format!("{module}.txt")), bytes).unwrap();
        }
        Self {
            _directory: directory,
            workbook,
            src: py::PyPath::new(&display(&source)),
        }
    }
    fn snapshot(&self) -> BTreeMap<String, Vec<u8>> {
        MODULES
            .iter()
            .map(|module| {
                let name = format!("{module}.txt");
                let bytes = std::fs::read(self.src.join(&name).fs()).unwrap();
                (name, bytes)
            })
            .collect()
    }
}

#[test]
fn every_module_is_stripped_before_afe_sync() {
    let f = Fixture::new();
    let changes = apply_with(
        &f.workbook,
        &f.src,
        &ONE_EACH,
        &mut write_text,
        crate::afe::sync,
    )
    .unwrap();
    let expected = MODULES.map(|module| format!("stripped 1 REVISIONS blocks from {module}.txt"));
    assert_eq!(&changes[..6], &expected);
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (name, bytes) in f.snapshot() {
        assert_eq!(bytes, std::fs::read(root.join("src").join(name)).unwrap());
    }
}

#[test]
fn a_failed_second_write_restores_every_module_in_order() {
    let f = Fixture::new();
    let before = f.snapshot();
    let before_book = std::fs::read(&f.workbook).unwrap();
    let mut calls = Vec::new();
    let mut write = |path: &Path, text: &str| {
        calls.push(basename(path));
        if calls.len() == 2 {
            return Err("synthetic failure on the second module write".into());
        }
        write_text(path, text)
    };
    assert_eq!(
        apply_with(&f.workbook, &f.src, &ONE_EACH, &mut write, |_, _| panic!(
            "sync must not run"
        )),
        Err("synthetic failure on the second module write".into())
    );
    let written = MODULES.map(|module| format!("{module}.txt"));
    assert_eq!(calls, [&written[..2], &written].concat());
    assert_eq!(f.snapshot(), before);
    assert_eq!(std::fs::read(&f.workbook).unwrap(), before_book);
}

#[test]
fn a_failed_sync_restores_source_and_noncanonical_workbook_bytes() {
    let f = Fixture::new();
    let before = f.snapshot();
    let before_book = std::fs::read(&f.workbook).unwrap();
    let result = apply_with(
        &f.workbook,
        &f.src,
        &ONE_EACH,
        &mut write_text,
        |workbook, _| {
            std::fs::write(workbook, b"partially written AFE store").unwrap();
            Err("synthetic AFE sync failure".into())
        },
    );
    assert_eq!(result, Err("synthetic AFE sync failure".into()));
    assert_eq!(f.snapshot(), before);
    assert_eq!(std::fs::read(&f.workbook).unwrap(), before_book);
}

#[test]
fn a_failed_source_restore_still_restores_the_workbook_first() {
    let f = Fixture::new();
    let before_book = std::fs::read(&f.workbook).unwrap();
    let mut calls = Vec::new();
    let mut write = |path: &Path, text: &str| {
        calls.push(basename(path));
        if calls.len() > MODULES.len() && basename(path) == "Dates.txt" {
            // The original workbook must already be restored at this point.
            assert_eq!(std::fs::read(&f.workbook).unwrap(), before_book);
            return Err("synthetic failure on the Dates restore".into());
        }
        write_text(path, text)
    };
    let result = apply_with(&f.workbook, &f.src, &ONE_EACH, &mut write, |workbook, _| {
        std::fs::write(workbook, b"partially written AFE store").unwrap();
        Err("synthetic AFE sync failure".into())
    });
    assert_eq!(result, Err("synthetic failure on the Dates restore".into()));
    assert_eq!(calls.len(), 7);
    assert_eq!(std::fs::read(&f.workbook).unwrap(), before_book);
}

#[test]
fn a_later_validation_failure_precedes_every_write() {
    let f = Fixture::new();
    let before = f.snapshot();
    let before_book = std::fs::read(&f.workbook).unwrap();
    let mut expected = ONE_EACH;
    expected[1].1 = 2;
    assert_eq!(
        apply_with(
            &f.workbook,
            &f.src,
            &expected,
            &mut |_, _| panic!("must not write"),
            |_, _| panic!("must not sync")
        ),
        Err("Essentials.txt: expected 2 REVISIONS blocks, found 1".into())
    );
    assert_eq!(f.snapshot(), before);
    assert_eq!(std::fs::read(&f.workbook).unwrap(), before_book);
}

#[test]
fn an_unreadable_archive_precedes_every_write() {
    let f = Fixture::new();
    let before = f.snapshot();
    std::fs::write(&f.workbook, b"invalid archive").unwrap();
    assert_eq!(
        apply_with(
            &f.workbook,
            &f.src,
            &ONE_EACH,
            &mut |_, _| panic!("must not write"),
            |_, _| panic!("must not sync")
        ),
        Err("File is not a zip file".into())
    );
    assert_eq!(f.snapshot(), before);
    assert_eq!(std::fs::read(&f.workbook).unwrap(), b"invalid archive");
}
