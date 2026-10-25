//! Owned workbook/source fixtures and the baseline-bound correction tables.
use ozzit::{
    archive::{BOOK, Parts, read_parts, write_deterministic},
    common::display,
    formula::MODULES,
};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    process::Command,
    sync::LazyLock,
};

static DATA: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!(
        "../../src/data/postbuild/help_corrections.json"
    ))
    .unwrap()
});

pub struct Swap {
    pub what: String,
    pub book: [String; 2],
    pub source: [String; 2],
    pub hits: usize,
}
pub fn swaps() -> Vec<Swap> {
    DATA["swaps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| Swap {
            what: s[0].as_str().unwrap().into(),
            book: [
                s[1][0].as_str().unwrap().into(),
                s[1][1].as_str().unwrap().into(),
            ],
            source: [
                s[2][0].as_str().unwrap().into(),
                s[2][1].as_str().unwrap().into(),
            ],
            hits: s[3].as_u64().unwrap() as usize,
        })
        .collect()
}
pub fn triples(key: &str) -> Vec<[String; 3]> {
    DATA[key]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| std::array::from_fn(|i| s[i].as_str().unwrap().into()))
        .collect()
}
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
pub fn text(parts: &Parts, name: &str) -> String {
    String::from_utf8(parts[name].clone()).unwrap()
}
pub fn set(parts: &mut Parts, name: &str, text: String) {
    parts.insert(name.into(), text.into_bytes());
}
pub fn paths(directory: &Path) -> BTreeSet<PathBuf> {
    let mut result = BTreeSet::new();
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            result.extend(paths(&path));
        }
        result.insert(path);
    }
    result
}

pub struct Fixture {
    pub directory: tempfile::TempDir,
    pub book: PathBuf,
    pub src: PathBuf,
}
impl Fixture {
    pub fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let book = directory.path().join("ozzit.xlsx");
        let src = directory.path().join("src");
        std::fs::create_dir(&src).unwrap();
        std::fs::copy(root().join("ozzit.xlsx"), &book).unwrap();
        for module in MODULES {
            std::fs::copy(
                root().join(format!("src/{module}.txt")),
                src.join(format!("{module}.txt")),
            )
            .unwrap();
        }
        Self {
            directory,
            book,
            src,
        }
    }
    pub fn source_files(&self) -> Vec<PathBuf> {
        let mut files = std::fs::read_dir(&self.src)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "txt"))
            .collect::<Vec<_>>();
        files.sort();
        files
    }
    pub fn snapshot(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        std::iter::once(self.book.clone())
            .chain(self.source_files())
            .map(|p| {
                let bytes = std::fs::read(&p).unwrap();
                (p, bytes)
            })
            .collect()
    }
    pub fn library(&self) -> String {
        self.source_files()
            .iter()
            .map(|p| std::fs::read_to_string(p).unwrap())
            .collect::<Vec<_>>()
            .join("\n")
    }
    pub fn write(&self, parts: &Parts) {
        write_deterministic(&self.book, parts).unwrap();
    }
    pub fn revert(&self, swap: &Swap) {
        let mut parts = read_parts(&self.book).unwrap();
        let book = text(&parts, BOOK);
        assert_eq!(book.matches(&swap.book[1]).count(), swap.hits);
        set(&mut parts, BOOK, book.replace(&swap.book[1], &swap.book[0]));
        self.write(&parts);
        let mut hits = 0;
        for path in self.source_files() {
            let source = std::fs::read_to_string(&path).unwrap();
            hits += source.matches(&swap.source[1]).count();
            std::fs::write(path, source.replace(&swap.source[1], &swap.source[0])).unwrap();
        }
        assert_eq!(hits, swap.hits);
    }
    pub fn cli(&self, args: &[String]) -> (i32, String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_ozzit"))
            .current_dir(self.directory.path())
            .arg("help_corrections")
            .args(args)
            .output()
            .unwrap();
        (
            output.status.code().unwrap(),
            String::from_utf8(output.stdout)
                .unwrap()
                .replace("\r\n", "\n"),
            String::from_utf8(output.stderr)
                .unwrap()
                .replace("\r\n", "\n"),
        )
    }
    pub fn run(&self) -> (i32, String, String) {
        self.cli(&[display(&self.book), display(&self.src)])
    }
    pub fn success(&self) -> String {
        let (code, out, err) = self.run();
        assert_eq!(code, 0, "{out}{err}");
        assert!(err.is_empty());
        assert!(out.ends_with(&format!("OK: {}\n", display(&self.book))));
        out
    }
    pub fn refused_without_writing(&self, expected: &str) {
        let before = self.snapshot();
        assert_eq!(
            self.run(),
            (1, String::new(), format!("FAIL: {expected}\n"))
        );
        assert_eq!(self.snapshot(), before);
    }
}
