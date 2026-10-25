//! Owned real and synthetic workbook fixtures, plus physical command checks.
use ozzit::{
    archive::{Parts, write_deterministic},
    common::display,
    postbuild::workbook_palette,
};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

pub const DXFS: &str = concat!(
    r#"<dxfs><dxf><font><color rgb="FFC00000"/></font>"#,
    r#"<fill><patternFill><bgColor rgb="FFFFCCCC"/></patternFill></fill></dxf></dxfs>"#,
);
pub const INPUT_FILL: &str = r#"<fgColor theme="0" tint="-4.9989318521683403E-2"/>"#;

pub fn palette_parts(dxfs: &str) -> Parts {
    [
        ("xl/theme/theme1.xml", r#"<a:theme><a:accent1><a:srgbClr val="3F656F"/></a:accent1></a:theme>"#.into()),
        ("xl/styles.xml", format!(concat!(
            r#"<styleSheet><fonts><font><name val="Arial"/><color rgb="FF006600"/></font></fonts>"#,
            r#"<fills><fill><patternFill><fgColor rgb="FFCCFFCC"/></patternFill></fill></fills>"#,
            "{}</styleSheet>"
        ), dxfs)),
        ("xl/sharedStrings.xml", r#"<sst><si><r><rPr><rFont val="Arial"/></rPr><t>x</t></r></si></sst>"#.into()),
        ("xl/worksheets/sheet1.xml", "<worksheet/>".into()),
    ].into_iter().map(|(name, text): (&str, String)| (name.into(), text.into_bytes())).collect()
}
pub fn text(parts: &Parts, name: &str) -> String {
    String::from_utf8(parts[name].clone()).unwrap()
}
pub fn set(parts: &mut Parts, name: &str, text: String) {
    parts.insert(name.into(), text.into_bytes());
}
pub fn section(text: &str, tag: &str) -> String {
    text[text.find(&format!("<{tag}")).unwrap()..text.find(&format!("</{tag}>")).unwrap()].into()
}

pub struct Fixture {
    pub directory: tempfile::TempDir,
    pub book: PathBuf,
}
impl Fixture {
    pub fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let book = directory.path().join("ozzit.xlsx");
        std::fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ozzit.xlsx"),
            &book,
        )
        .unwrap();
        Self { directory, book }
    }
    pub fn bytes(&self) -> Vec<u8> {
        std::fs::read(&self.book).unwrap()
    }
    pub fn write(&self, parts: &Parts) {
        write_deterministic(&self.book, parts).unwrap();
    }
    pub fn cli(&self, args: &[String]) -> (i32, String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_ozzit"))
            .current_dir(self.directory.path())
            .arg("workbook_palette")
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
    pub fn success(&self, template: bool) -> String {
        let mut args = Vec::new();
        if template {
            args.push("--template".into());
        }
        args.push(display(&self.book));
        let (code, out, err) = self.cli(&args);
        assert_eq!(code, 0, "{out}{err}");
        assert!(err.is_empty(), "{err}");
        out
    }
    pub fn refused(&self, args: &[String], message: &str) {
        assert_eq!(
            self.cli(args),
            (1, String::new(), format!("FAIL: {message}\n"))
        );
    }
    pub fn assert_noop(&self, template: bool) {
        let before = self.bytes();
        assert_eq!(
            workbook_palette::run(&self.book, template).unwrap(),
            Vec::<String>::new()
        );
        assert_eq!(self.bytes(), before);
    }
}
