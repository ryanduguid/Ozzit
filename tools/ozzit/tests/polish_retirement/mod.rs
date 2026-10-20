//! Reuse the frozen Python corpus input; keep expected presentation text explicit.
use base64::Engine;
use ozzit::{archive, common::display, polish};
use serde_json::Value;
use std::{path::Path, process::Command};

pub const STRAPLINES: [(&str, &str); 8] = [
    (
        "oz.CountCλ",
        "Count the number of times one or more characters appear in a string",
    ),
    (
        "oz.CountRowsλ",
        "Count the number of numbers in each row of an array",
    ),
    (
        "oz.CountColsλ",
        "Count the number of numbers in each column of an array",
    ),
    (
        "oz.CountARowsλ",
        "Count all non-empty cells in each row of an array",
    ),
    (
        "oz.CountAColsλ",
        "Count all non-empty cells in each column of an array",
    ),
    (
        "oz.IsBetweenEλ",
        "Determine if a value is between a lower and upper limit",
    ),
    (
        "oz.RangeToDAEλ",
        "Convert a static range into a dynamic array",
    ),
    ("oz.FinancialRatios", "Three dozen financial ratios"),
];
pub const COVER_OLD: &str = "Each function worksheet has green shaded cells. These cells contain the function whose name appears in the worksheet's title.";
pub const COVER_NEW: &str = "Each function worksheet has pale purple shaded cells. These cells contain the function whose name appears in the worksheet's title.";

pub struct Fixture {
    pub directory: tempfile::TempDir,
    pub book: std::path::PathBuf,
}

impl Fixture {
    pub fn new() -> Self {
        let data: Value = serde_json::from_reader(flate2::read::GzDecoder::new(
            &include_bytes!("../data/polish-contracts.json.gz")[..],
        ))
        .unwrap();
        assert_eq!(data["records"].as_array().unwrap().len(), 31);
        let record = data["records"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["name"] == "unpolished")
            .unwrap();
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(
                data["blobs"][record["input"].as_str().unwrap()]
                    .as_str()
                    .unwrap(),
            )
            .unwrap();
        let directory = tempfile::tempdir().unwrap();
        let book = directory.path().join("ozzit.xlsx");
        std::fs::write(&book, bytes).unwrap();
        Self { directory, book }
    }

    pub fn shipped() -> Self {
        let fixture = Self::new();
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        std::fs::copy(root.join("ozzit.xlsx"), &fixture.book).unwrap();
        fixture
    }

    pub fn args(&self) -> [String; 1] {
        [display(&self.book)]
    }

    pub fn cli(&self, args: &[String]) -> (i32, String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_ozzit"))
            .arg("polish_workbook")
            .args(args)
            .current_dir(self.directory.path())
            .output()
            .unwrap();
        // Python subprocess text mode normalised Windows newlines.
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

    pub fn status(&self, shown: &str) -> String {
        let mut size = std::fs::metadata(&self.book).unwrap().len();
        let mut groups = Vec::new();
        while size >= 1000 {
            groups.push(format!("{:03}", size % 1000));
            size /= 1000;
        }
        groups.push(size.to_string());
        groups.reverse();
        format!("OK: {shown} polished, {} bytes\n", groups.join(","))
    }

    pub fn parts(&self) -> archive::Parts {
        archive::read_parts(&self.book).unwrap()
    }
}

pub fn text(parts: &archive::Parts, name: &str) -> String {
    String::from_utf8(parts[name].clone()).unwrap()
}

pub fn sheet(with_a2: bool) -> String {
    let parts = Fixture::new().parts();
    text(
        &parts,
        if with_a2 {
            "xl/worksheets/sheet2.xml"
        } else {
            "xl/worksheets/sheet9.xml"
        },
    )
}

pub fn assert_polished_parts(parts: &archive::Parts) {
    let shared = text(parts, "xl/sharedStrings.xml");
    let mut expected = vec![COVER_NEW.to_string()];
    expected.extend(STRAPLINES.map(|(_, value)| value.to_string()));
    assert_eq!(polish::shared_values(&shared).unwrap(), expected);
    assert!(shared.contains("count=\"10\" uniqueCount=\"9\""));
    let paths = polish::sheet_paths(parts).unwrap();
    for (index, (name, _)) in STRAPLINES.iter().enumerate() {
        let path = &paths.iter().find(|(n, _)| n == name).unwrap().1;
        let style = if *name == "oz.FinancialRatios" {
            "288"
        } else {
            "1"
        };
        assert!(
            text(parts, path).contains(&format!(
                "<c r=\"A2\" s=\"{style}\" t=\"s\"><v>{}</v></c>",
                index + 1
            )),
            "{name}"
        );
    }
    let ratios = &paths
        .iter()
        .find(|(name, _)| name == "oz.FinancialRatios")
        .unwrap()
        .1;
    assert!(text(parts, ratios).contains(
        "<mergeCells count=\"2\"><mergeCell ref=\"A1:K1\"/><mergeCell ref=\"A2:K2\"/></mergeCells>"
    ));
    let styles = text(parts, "xl/styles.xml");
    for absent in ["indexed=\"42\"", "FF808080", "numFmtId=\"14\""] {
        assert!(!styles.contains(absent), "{absent}");
    }
}

pub fn refused(fixture: &Fixture, args: &[String], message: &str) {
    let before = std::fs::read(&fixture.book).unwrap();
    assert_eq!(
        fixture.cli(args),
        (1, String::new(), format!("{message}\n"))
    );
    assert_eq!(std::fs::read(&fixture.book).unwrap(), before);
}
