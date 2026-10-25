//! Original sanitiser fixtures, physical commands and independent ZIP checks.
use ozzit::{archive, common::display, sanitise};
use serde_json::Value;
use std::{ops::Range, path::Path, process::Command};

pub const STAMP: &str = "2026-01-01T00:00:00Z";
pub const WINDOW: &str = r#"xWindow="0" yWindow="0" windowWidth="28800" windowHeight="16000""#;
pub const STALE_DATE: &str = "2026-09-01T10:11:12Z";
pub const CORE: &str = "docProps/core.xml";

pub struct Fixture {
    pub directory: tempfile::TempDir,
    pub book: std::path::PathBuf,
}

impl Fixture {
    pub fn shipped() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let book = directory.path().join("dirty.xlsx");
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        std::fs::copy(root.join("ozzit.xlsx"), &book).unwrap();
        Self { directory, book }
    }

    pub fn residue() -> Self {
        let fixture = Self::shipped();
        fixture.write(&residue_parts(false));
        fixture
    }

    pub fn parts(&self) -> archive::Parts {
        archive::read_parts(&self.book).unwrap()
    }

    pub fn bytes(&self) -> Vec<u8> {
        std::fs::read(&self.book).unwrap()
    }

    pub fn write(&self, parts: &archive::Parts) {
        archive::write_deterministic(&self.book, parts).unwrap();
    }

    pub fn repeat(&self) {
        let before = self.bytes();
        assert_eq!(sanitise::sanitise(&self.book).unwrap(), ["already clean"]);
        assert_eq!(self.bytes(), before);
    }

    pub fn args(&self) -> [String; 1] {
        [display(&self.book)]
    }

    pub fn cli(&self, args: &[String]) -> (i32, String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_ozzit"))
            .arg("sanitise_workbook")
            .args(args)
            .current_dir(self.directory.path())
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

    pub fn status(&self) -> String {
        let mut size = self.bytes().len();
        let mut groups = Vec::new();
        while size >= 1000 {
            groups.push(format!("{:03}", size % 1000));
            size /= 1000;
        }
        groups.push(size.to_string());
        groups.reverse();
        format!(
            "OK: {} sanitised, {} bytes\n",
            display(&self.book),
            groups.join(",")
        )
    }

    pub fn noncanonical(&self, parts: &archive::Parts) {
        let bytes = archive::deterministic_bytes(parts).unwrap();
        let records = records(&bytes);
        let start = records.first().unwrap().start;
        let end = records.last().unwrap().end;
        let mut changed = bytes[..start].to_vec();
        for range in records.into_iter().rev() {
            let mut record = bytes[range].to_vec();
            // Deliberately change only central-directory date and entry order.
            record[14..16].copy_from_slice(&0u16.to_le_bytes());
            changed.extend(record);
        }
        changed.extend_from_slice(&bytes[end..]);
        std::fs::write(&self.book, changed).unwrap();
        assert_eq!(&self.parts(), parts);
    }
}

pub fn text(parts: &archive::Parts, name: &str) -> String {
    String::from_utf8(parts[name].clone()).unwrap()
}

pub fn residue_parts(clean: bool) -> archive::Parts {
    let data: Value = serde_json::from_str(include_str!("../data/sanitise-residue.json")).unwrap();
    data[if clean { "expected_parts" } else { "parts" }]
        .as_object()
        .unwrap()
        .iter()
        .map(|(p, v)| (p.clone(), v.as_str().unwrap().as_bytes().to_vec()))
        .collect()
}

pub fn residue_log() -> Vec<String> {
    vec![
        format!(
            "[Content_Types].xml {} -> {} bytes",
            residue_parts(false)["[Content_Types].xml"].len(),
            residue_parts(true)["[Content_Types].xml"].len()
        ),
        "removed 1 x15ac:absPath".into(),
        "cleared always-calculate flags on 2 cells".into(),
    ]
}

pub fn stale_core(parts: &mut archive::Parts, creator: bool) {
    let mut core = text(parts, CORE);
    if !creator {
        let found = ozzit::common::re(r"<dc:creator>[^<]*</dc:creator>");
        let found = found.find(&core).unwrap().unwrap();
        core.replace_range(found.range(), "");
    }
    core = core
        .replace(
            "<cp:lastModifiedBy>Ozzit project</cp:lastModifiedBy>",
            "<cp:lastModifiedBy>Somebody Else</cp:lastModifiedBy>",
        )
        .replace(
            &format!("{STAMP}</dcterms:modified>"),
            &format!("{STALE_DATE}</dcterms:modified>"),
        );
    assert!(core.contains("Somebody Else"));
    assert!(core.contains(STALE_DATE));
    parts.insert(CORE.into(), core.into_bytes());
}

pub fn stale_book(parts: &mut archive::Parts) {
    let book = text(parts, archive::BOOK)
        .replace(&format!("<workbookView {WINDOW}"), "<workbookView xWindow=\"-120\" yWindow=\"-120\" windowWidth=\"51840\" windowHeight=\"21120\"")
        .replace("<fileVersion appName=\"xl\" lastEdited=\"7\" lowestEdited=\"7\"/>", concat!(
            "<fileVersion appName=\"xl\" lastEdited=\"7\" lowestEdited=\"7\" rupBuild=\"30326\"/>",
            "<workbookPr codeName=\"ThisWorkbook\"/><xr:revisionPtr revIDLastSave=\"0\" ",
            "documentId=\"13_ncr:1_{9C5E2E38-D768-43F5-8346-A39F06CA7C63}\" ",
            "xr6:coauthVersionLast=\"47\" xr6:coauthVersionMax=\"47\" ",
            "xr10:uidLastSave=\"{00000000-0000-0000-0000-000000000000}\"/>"
        ))
        .replacen("<workbookPr codeName=\"ThisWorkbook\"/><bookViews>", "<bookViews>", 1);
    assert!(book.contains("rupBuild"));
    assert!(book.contains("<xr:revisionPtr"));
    parts.insert(archive::BOOK.into(), book.into_bytes());
}

fn word(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn records(bytes: &[u8]) -> Vec<Range<usize>> {
    let end = bytes.len() - 22;
    assert_eq!(&bytes[end..end + 4], b"PK\x05\x06");
    let mut offset = u32::from_le_bytes(bytes[end + 16..end + 20].try_into().unwrap()) as usize;
    let mut records = Vec::new();
    for _ in 0..word(bytes, end + 10) {
        assert_eq!(&bytes[offset..offset + 4], b"PK\x01\x02");
        let next = offset
            + 46
            + usize::from(word(bytes, offset + 28))
            + usize::from(word(bytes, offset + 30))
            + usize::from(word(bytes, offset + 32));
        records.push(offset..next);
        offset = next;
    }
    assert_eq!(offset, end);
    records
}

pub fn canonical_metadata(bytes: &[u8]) {
    let mut names = Vec::new();
    for range in records(bytes) {
        let record = &bytes[range];
        assert_eq!(word(record, 4) >> 8, 3);
        assert_eq!(word(record, 12), 0);
        assert_eq!(word(record, 14), ((2026 - 1980) << 9) | (1 << 5) | 1);
        assert_eq!(
            u32::from_le_bytes(record[38..42].try_into().unwrap()) >> 16,
            0o644
        );
        let end = 46 + usize::from(word(record, 28));
        names.push(String::from_utf8(record[46..end].to_vec()).unwrap());
    }
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted);
}

pub fn missing(fixture: &Fixture) {
    let missing = fixture.directory.path().join("missing.xlsx");
    assert_eq!(
        fixture.cli(&[display(&missing)]),
        (
            1,
            String::new(),
            format!("FAIL: no such workbook: {}\n", display(&missing))
        )
    );
    assert!(!missing.exists());
}

pub fn broken(fixture: &Fixture) {
    let broken = fixture.directory.path().join("broken.xlsx");
    std::fs::write(&broken, b"not a zip").unwrap();
    let (code, out, err) = fixture.cli(&[display(&broken)]);
    assert_eq!(code, 1);
    assert_eq!(out, "");
    assert!(err.starts_with(&format!("FAIL: cannot sanitise {}: ", display(&broken))));
    assert!(!err.contains("Traceback"));
    assert_eq!(std::fs::read(&broken).unwrap(), b"not a zip");
}
