//! Only task-owned copies are written; source reversal keeps the original checks.
use super::tooling::{
    archive::{BOOK, read_parts, write_deterministic},
    common::{Csv, csv_row, display},
};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
pub fn data() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../src/data/postbuild/working_capital_days.json"
    ))
    .unwrap()
}
pub fn anchor() -> String {
    data()["about"]["Ratios"][0].as_str().unwrap().into()
}
pub fn functions() -> Vec<Vec<String>> {
    data()["functions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            v.as_array()
                .unwrap()
                .iter()
                .map(|s| s.as_str().unwrap().into())
                .collect()
        })
        .collect()
}
pub fn unapply_source(text: &str) -> String {
    let functions = functions();
    let blocks = format!(
        "\n\n\n{}",
        functions
            .iter()
            .map(|f| f[3].as_str())
            .collect::<Vec<_>>()
            .join("\n\n\n")
    );
    let text = text.strip_suffix(&blocks).unwrap_or(text);
    let anchor = anchor();
    let at = text.find(&anchor).unwrap();
    let start = text[..at].rfind('\n').map_or(0, |i| i + 1);
    let indent = &text[start..at];
    let rows = functions
        .iter()
        .map(|f| format!("{indent}\"{:<27}→{}¶\" &", f[1], f[2]))
        .collect::<Vec<_>>()
        .join("\n");
    text.replacen(&format!("{anchor} &\n{rows}"), &format!("{anchor} &"), 1)
}

pub struct Fixture {
    pub directory: tempfile::TempDir,
    pub book: PathBuf,
    pub src: PathBuf,
    pub index: PathBuf,
}
impl Fixture {
    pub fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let book = directory.path().join("ozzit.xlsx");
        let src = directory.path().join("src");
        let index = directory.path().join("functions.csv");
        std::fs::copy(root().join("ozzit.xlsx"), &book).unwrap();
        std::fs::copy(root().join("functions.csv"), &index).unwrap();
        std::fs::create_dir(&src).unwrap();
        for entry in std::fs::read_dir(root().join("src")).unwrap() {
            let entry = entry.unwrap();
            std::fs::copy(entry.path(), src.join(entry.file_name())).unwrap();
        }
        Self {
            directory,
            book,
            src,
            index,
        }
    }
    pub fn absent() -> Self {
        let f = Self::new();
        let original = f.source();
        assert!(!original.ends_with('\n'));
        let absent = unapply_source(&original);
        assert_ne!(absent, original);
        std::fs::write(f.src.join("Ratios.txt"), &absent).unwrap();
        let mut parts = read_parts(&f.book).unwrap();
        let mut book = String::from_utf8(parts[BOOK].clone()).unwrap();
        let names = functions()
            .iter()
            .map(|f| format!("oz.{}", f[1]))
            .collect::<Vec<_>>();
        for name in &names {
            let start = format!("<definedName name=\"{name}\"");
            assert_eq!(book.matches(&start).count(), 1, "{name}");
            let at = book.find(&start).unwrap();
            let end = at + book[at..].find("</definedName>").unwrap() + "</definedName>".len();
            book.replace_range(at..end, "");
        }
        parts.insert(BOOK.into(), book.into_bytes());
        write_deterministic(&f.book, &parts).unwrap();
        let csv = Csv::read(&f.index).unwrap();
        let removed = csv
            .rows
            .iter()
            .filter(|r| {
                names
                    .iter()
                    .any(|n| Some(n.as_str()) == csv.get(r, "function"))
            })
            .count();
        assert_eq!(removed, 4);
        f.remove_index_names(&names);
        f
    }
    pub fn remove_index_names(&self, names: &[String]) {
        let csv = Csv::read(&self.index).unwrap();
        let mut text = csv_row(&csv.headers);
        for row in &csv.rows {
            if !names
                .iter()
                .any(|n| Some(n.as_str()) == csv.get(row, "function"))
            {
                text.push_str(&csv_row(row));
            }
        }
        std::fs::write(&self.index, text).unwrap();
    }
    pub fn source(&self) -> String {
        std::fs::read_to_string(self.src.join("Ratios.txt")).unwrap()
    }
    pub fn snapshot(&self) -> Vec<(PathBuf, Vec<u8>)> {
        let mut paths = vec![self.book.clone(), self.index.clone()];
        paths.extend(
            std::fs::read_dir(&self.src)
                .unwrap()
                .map(|e| e.unwrap().path()),
        );
        paths.sort();
        paths
            .into_iter()
            .filter(|p| p.is_file())
            .map(|p| {
                let bytes = std::fs::read(&p).unwrap();
                (p, bytes)
            })
            .collect()
    }
    #[allow(dead_code)] // The private recovery tests use the same owned fixtures.
    pub fn cli(&self, binary: &str, args: &[String]) -> (i32, String, String) {
        let out = Command::new(binary)
            .current_dir(self.directory.path())
            .arg("working_capital_days")
            .args(args)
            .output()
            .unwrap();
        (
            out.status.code().unwrap(),
            String::from_utf8(out.stdout).unwrap().replace("\r\n", "\n"),
            String::from_utf8(out.stderr).unwrap().replace("\r\n", "\n"),
        )
    }
    #[allow(dead_code)]
    pub fn args(&self) -> Vec<String> {
        [&self.book, &self.src, &self.index]
            .map(|p| display(p))
            .to_vec()
    }
}
