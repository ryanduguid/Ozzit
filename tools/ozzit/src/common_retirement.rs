//! Former Python writer contracts at the native staging and replacement boundary.
//!
//! Native stages remain open and use RAII. Write errors and panic unwinding are
//! checked here; neither file-close errors nor OS process termination are modelled.
use super::*;
use std::{collections::BTreeMap, io::Write};

struct Fixture {
    dir: tempfile::TempDir,
    book: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let book = dir.path().join("book.xlsx");
        std::fs::write(&book, b"original").unwrap();
        std::fs::write(dir.path().join("book.xlsx.tmp"), b"unrelated work").unwrap();
        Self { dir, book }
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
}

fn refuse(f: &Fixture, bytes: &[u8]) {
    let before = f.snapshot();
    let mut staged = None;
    let error = replace_atomically_with(&f.book, bytes, |file, destination| {
        assert_eq!(destination, f.book);
        assert_eq!(file.path().parent().unwrap(), f.dir.path());
        assert_eq!(std::fs::read(file.path()).unwrap(), bytes);
        assert_eq!(std::fs::read(destination).unwrap(), before[&f.book]);
        staged = Some(file.path().to_owned());
        Err(tempfile::PersistError {
            error: std::io::Error::other("blocked"),
            file,
        })
    })
    .unwrap_err();
    assert_eq!(error, "blocked");
    assert!(!staged.unwrap().exists());
    assert_eq!(f.snapshot(), before);
}

#[test]
fn test_atomic_writer_preserves_original_and_cleans_temp_if_replace_fails() {
    let f = Fixture::new();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let original = std::fs::read(root.join("ozzit.xlsx")).unwrap();
    std::fs::write(&f.book, &original).unwrap();
    let mut parts = crate::archive::read_parts(&f.book).unwrap();
    parts.get_mut("docProps/core.xml").unwrap().push(b' ');
    let changed = crate::archive::deterministic_bytes(&parts).unwrap();
    assert_ne!(changed, original);
    // The unchanged archive writer forwards these bytes to this shared boundary.
    refuse(&f, &changed);
}

#[test]
fn test_atomic_writer_preserves_unrelated_temporary_sibling() {
    let f = Fixture::new();
    let mut expected = f.snapshot();
    expected.insert(f.book.clone(), b"replacement".to_vec());
    replace_atomically(&f.book, b"replacement").unwrap();
    assert_eq!(f.snapshot(), expected);
}

#[test]
fn test_failed_atomic_writer_preserves_unrelated_temporary_sibling() {
    refuse(&Fixture::new(), b"replacement");
}

#[test]
fn staging_errors_and_unwinding_clean_real_partial_or_complete_files() {
    let f = Fixture::new();
    let before = f.snapshot();
    for count in [2, b"replacement".len()] {
        let mut staged = None;
        let error = stage_bytes_with(&f.book, b"replacement", |file, bytes| {
            staged = Some(file.path().to_owned());
            file.write_all(&bytes[..count])?;
            assert_eq!(std::fs::read(file.path()).unwrap(), &bytes[..count]);
            Err(std::io::Error::other("injected staging failure"))
        })
        .unwrap_err();
        assert!(error.contains("injected staging failure"));
        assert!(!staged.unwrap().exists());
        assert_eq!(f.snapshot(), before);
    }
    let mut staged = None;
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        stage_bytes_with(&f.book, b"replacement", |file, bytes| {
            staged = Some(file.path().to_owned());
            file.write_all(&bytes[..2])?;
            assert_eq!(std::fs::read(file.path()).unwrap(), b"re");
            panic!("injected staging failure");
        })
    }));
    assert_eq!(
        panic.unwrap_err().downcast_ref::<&str>(),
        Some(&"injected staging failure")
    );
    assert!(!staged.unwrap().exists());
    assert_eq!(f.snapshot(), before);
}

#[test]
fn overlapping_open_stages_have_distinct_files_and_commit_in_order() {
    let f = Fixture::new();
    let mut expected = f.snapshot();
    let first = stage_bytes(&f.book, b"first").unwrap();
    let second = stage_bytes(&f.book, b"second").unwrap();
    assert_ne!(first.path(), second.path());
    for file in [&first, &second] {
        assert_eq!(file.path().parent().unwrap(), f.dir.path());
        assert!(file.as_file().metadata().unwrap().is_file());
    }
    assert_eq!(std::fs::read(&f.book).unwrap(), b"original");
    assert_eq!(std::fs::read(first.path()).unwrap(), b"first");
    assert_eq!(std::fs::read(second.path()).unwrap(), b"second");
    drop(first.persist(&f.book).unwrap());
    assert_eq!(std::fs::read(&f.book).unwrap(), b"first");
    assert_eq!(std::fs::read(second.path()).unwrap(), b"second");
    drop(second.persist(&f.book).unwrap());
    expected.insert(f.book.clone(), b"second".to_vec());
    assert_eq!(f.snapshot(), expected);
}

#[test]
fn test_atomic_text_writer_preserves_utf8_and_line_endings() {
    let f = Fixture::new();
    let path = f.dir.path().join("text.txt");
    let mut expected = f.snapshot();
    let text = "one\r\ntwo\nλ\rthree";
    replace_atomically(&path, text.as_bytes()).unwrap();
    expected.insert(path, text.as_bytes().to_vec());
    assert_eq!(f.snapshot(), expected);
}

#[test]
fn replacement_and_staging_preserve_the_platform_permission_policy() {
    let f = Fixture::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for mode in [0o640, 0o664] {
            std::fs::set_permissions(&f.book, std::fs::Permissions::from_mode(mode)).unwrap();
            let stage = stage_bytes(&f.book, b"new bytes").unwrap();
            assert_eq!(
                stage.as_file().metadata().unwrap().permissions().mode() & 0o7777,
                mode
            );
            assert_eq!(std::fs::read(&f.book).unwrap(), b"original");
            drop(stage);
            replace_atomically(&f.book, b"replacement").unwrap();
            assert_eq!(
                std::fs::metadata(&f.book).unwrap().permissions().mode() & 0o7777,
                mode
            );
            assert_eq!(std::fs::read(&f.book).unwrap(), b"replacement");
            std::fs::write(&f.book, b"original").unwrap();
        }
    }
    #[cfg(windows)]
    {
        let original = std::fs::metadata(&f.book).unwrap().permissions();
        let mut readonly = original.clone();
        readonly.set_readonly(true);
        std::fs::set_permissions(&f.book, readonly).unwrap();
        let stage = stage_bytes(&f.book, b"new bytes").unwrap();
        assert!(!stage.as_file().metadata().unwrap().permissions().readonly());
        assert!(std::fs::metadata(&f.book).unwrap().permissions().readonly());
        assert_eq!(std::fs::read(&f.book).unwrap(), b"original");
        drop(stage);
        std::fs::set_permissions(&f.book, original).unwrap();
    }
    assert_eq!(f.snapshot().len(), 2);
    assert_eq!(
        std::fs::read(f.dir.path().join("book.xlsx.tmp")).unwrap(),
        b"unrelated work"
    );
}

#[test]
fn staging_a_new_destination_keeps_default_permissions_and_owns_cleanup() {
    let f = Fixture::new();
    let before = f.snapshot();
    let missing = f.dir.path().join("new.xlsx");
    let stage = stage_bytes(&missing, b"new bytes").unwrap();
    assert_eq!(stage.path().parent().unwrap(), f.dir.path());
    assert_eq!(std::fs::read(stage.path()).unwrap(), b"new bytes");
    assert!(!missing.exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let default = tempfile::NamedTempFile::new_in(f.dir.path()).unwrap();
        assert_eq!(
            stage.as_file().metadata().unwrap().permissions().mode(),
            default.as_file().metadata().unwrap().permissions().mode()
        );
    }
    #[cfg(windows)]
    assert!(!stage.as_file().metadata().unwrap().permissions().readonly());
    let path = stage.path().to_owned();
    drop(stage);
    assert!(!path.exists());
    assert_eq!(f.snapshot(), before);
}

#[test]
fn test_raw_text_helper_preserves_utf8_and_physical_newlines() {
    let f = Fixture::new();
    let path = f.dir.path().join("module.txt");
    let text = "λ\r\nsecond\rthird\n";
    std::fs::write(&path, text.as_bytes()).unwrap();
    assert_eq!(read_raw(&path).unwrap(), text);
}
