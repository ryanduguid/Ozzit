//! Table invariants, ordered example errors and an owned staging failure.
use super::*;
use std::{collections::BTreeMap, path::PathBuf};

#[test]
fn every_anchor_distinguishes_the_two_states() {
    assert_eq!(TABLES.swaps.len(), 10);
    assert_eq!(TABLES.cells.len(), 3);
    assert_eq!(TABLES.strings.len(), 5);
    for s in &TABLES.swaps {
        for (old, new) in [&s.workbook, &s.src] {
            assert!(!new.contains(old), "{}", s.what);
            assert!(!old.contains(new), "{}", s.what);
        }
    }
    for (what, old, new) in TABLES.cells.iter().chain(&TABLES.strings) {
        assert!(!new.contains(old), "{what}");
        assert!(!old.contains(new), "{what}");
    }
}

#[test]
fn all_missing_worksheet_examples_are_reported_in_table_order() {
    let book = TABLES
        .swaps
        .iter()
        .map(|s| s.workbook.1.repeat(s.hits))
        .collect::<String>();
    let source = TABLES
        .swaps
        .iter()
        .map(|s| s.src.1.repeat(s.hits))
        .collect::<String>();
    let originals = vec![("Dates", py::PyPath::new("Dates.txt"), source)];
    let strings = TABLES
        .strings
        .iter()
        .map(|(_, _, new)| new.as_str())
        .collect::<String>();
    let mut failures = vec![];
    anchor_failures(
        &book,
        &originals,
        &[("xl/worksheets/sheet1.xml".into(), "<worksheet/>".into())],
        &strings,
        &mut failures,
    );
    assert_eq!(
        failures,
        TABLES
            .cells
            .iter()
            .map(|(what, _, _)| format!(
                "worksheets: {what} expected 1 recognised anchor, got 0 (before=0, after=0)"
            ))
            .collect::<Vec<_>>()
    );
}

struct Fixture {
    directory: tempfile::TempDir,
    workbook: PathBuf,
    src: py::PyPath,
}
impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let workbook = directory.path().join("ozzit.xlsx");
        let source = directory.path().join("src");
        std::fs::create_dir(&source).unwrap();
        for module in MODULES {
            std::fs::copy(
                root.join(format!("src/{module}.txt")),
                source.join(format!("{module}.txt")),
            )
            .unwrap();
        }
        let swap = &TABLES.swaps[0];
        let mut parts = read_parts(&root.join("ozzit.xlsx")).unwrap();
        let book = read_book(&parts).unwrap();
        assert_eq!(book.matches(&swap.workbook.1).count(), swap.hits);
        parts.insert(
            BOOK.into(),
            book.replace(&swap.workbook.1, &swap.workbook.0)
                .into_bytes(),
        );
        write_deterministic(&workbook, &parts).unwrap();
        let mut hits = 0;
        for module in MODULES {
            let path = source.join(format!("{module}.txt"));
            let text = std::fs::read_to_string(&path).unwrap();
            hits += text.matches(&swap.src.1).count();
            std::fs::write(path, text.replace(&swap.src.1, &swap.src.0)).unwrap();
        }
        assert_eq!(hits, swap.hits);
        Self {
            directory,
            workbook,
            src: py::PyPath::new(&display(&source)),
        }
    }
    fn snapshot(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        std::iter::once(self.workbook.clone())
            .chain(MODULES.map(|m| self.src.join(&format!("{m}.txt")).fs()))
            .map(|p| {
                let bytes = std::fs::read(&p).unwrap();
                (p, bytes)
            })
            .collect()
    }
}

#[test]
fn a_partial_source_stage_failure_preserves_both_stores_and_cleans_all_stages() {
    let f = Fixture::new();
    let before = f.snapshot();
    let mut attempts = vec![];
    let mut stages = vec![];
    let result = run_with(&f.workbook, &f.src, &mut |path, bytes| {
        assert_eq!(
            f.snapshot(),
            before,
            "no destination may change whilst staging"
        );
        attempts.push(path.to_path_buf());
        if path == f.workbook {
            assert_eq!(attempts.len(), 1);
            let staged = stage_bytes(path, bytes)?;
            let book = read_book(&read_parts(staged.path())?).unwrap();
            assert_eq!(
                book.matches(&TABLES.swaps[0].workbook.1).count(),
                TABLES.swaps[0].hits
            );
            stages.push(staged.path().to_path_buf());
            return Ok(staged);
        }
        assert_eq!(attempts.len(), 2);
        assert_eq!(path.parent(), Some(f.src.fs().as_path()));
        assert!(
            stages[0].is_file(),
            "the complete workbook stage must still exist"
        );
        let half = &bytes[..bytes.len() / 2];
        assert!(!half.is_empty() && half.len() < bytes.len());
        let partial = stage_bytes(path, half)?;
        stages.push(partial.path().to_path_buf());
        assert_eq!(std::fs::read(partial.path()).unwrap(), half);
        Err("no space left on device".into())
    });
    assert_eq!(result, Err("no space left on device".into()));
    assert_eq!(attempts.len(), 2);
    assert_eq!(f.snapshot(), before);
    assert!(stages.iter().all(|p| !p.exists()));
    for directory in [f.directory.path().to_path_buf(), f.src.fs()] {
        assert!(
            std::fs::read_dir(directory).unwrap().all(|e| e
                .unwrap()
                .path()
                .extension()
                .is_none_or(|e| e != "tmp"))
        );
    }
}
