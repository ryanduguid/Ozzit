//! Signature command contracts retained from the Python tests.
use ozzit::{common::*, formula, help};
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
    src: PathBuf,
}
impl Fixture {
    fn published() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir(&src).unwrap();
        for module in formula::MODULES {
            let name = format!("{module}.txt");
            std::fs::copy(root().join("src").join(&name), src.join(name)).unwrap();
        }
        Self { dir, src }
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
    fn preserved<T>(&self, operation: impl FnOnce() -> T) -> T {
        let before = self.snapshot();
        let result = operation();
        assert_eq!(self.snapshot(), before);
        result
    }
    fn run(&self) -> Output {
        self.preserved(|| help::verify(&[display(&self.src)]))
    }
    fn cli(&self, args: &[String], encoding: &str) -> std::process::Output {
        self.preserved(|| {
            Command::new(env!("CARGO_BIN_EXE_ozzit"))
                .arg("verify_signatures")
                .args(args)
                .env("PYTHONIOENCODING", encoding)
                .current_dir(self.dir.path())
                .output()
                .unwrap()
        })
    }
    fn edit(&self, module: &str, before: &str, after: &str, matches: usize) {
        let path = self.src.join(format!("{module}.txt"));
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.matches(before).count(), matches);
        std::fs::write(path, text.replacen(before, after, 1)).unwrap();
    }
    fn refuses(&self, message: &str) {
        let result = self.run();
        assert_eq!(result.code, 1, "{result:?}");
        assert!(result.err.is_empty(), "{result:?}");
        assert!(
            result.out.iter().any(|line| line.contains(message)),
            "{result:?}"
        );
    }
}

fn published_output() -> Vec<String> {
    [
        "153 declarations read: 145 signatures, 145 parameter tables and 142 example blocks checked",
        "   no FUNCTION line (3): AmortiseλDV, CorkscrewλDV, DepreciateλDV",
        "   no parameter table (3): AmortiseλDV, CorkscrewλDV, DepreciateλDV",
        "   not a LAMBDA (5): AboutDatesλ, AboutEssentialsλ, AboutFinancialλ, AboutRatiosλ, AboutUtilitiesλ",
        "OK: all 145 signatures, 145 parameter tables and 142 example blocks describe their own function",
    ].map(String::from).to_vec()
}

#[test]
fn published_help_preserves_all_counts_and_accepts_non_name_headings() {
    let f = Fixture::published();
    assert!(
        std::fs::read_to_string(f.src.join("Financial.txt"))
            .unwrap()
            .contains("AMORTISATION SUITE")
    );
    let result = f.run();
    assert_eq!(result.code, 0, "{result:?}");
    assert!(result.err.is_empty());
    assert_eq!(result.out, published_output());
}

#[test]
fn the_first_coming_soon_webpage_is_rejected() {
    let f = Fixture::published();
    f.edit(
        "Financial",
        "\"WEBPAGE:       →https://github.com/ryanduguid/Ozzit¶\"",
        "\"WEBPAGE:       →<Coming Soon>¶\"",
        44,
    );
    f.refuses("Coming Soon");
}

#[test]
fn copied_about_descriptions_are_rejected() {
    let f = Fixture::published();
    f.edit("Financial", "\"RollingAvgλ        →Creates averages for preceding values of a set size moving from beginning to end over a row of values.¶\" &", "\"RollingAvgλ        →Finds the maximum value the n preceding values in a set moving from beginning to end over a row of values.¶\" &", 1);
    f.refuses("share the same About-table description");
}

#[test]
fn an_about_table_cannot_list_an_undeclared_name() {
    let f = Fixture::published();
    f.edit(
        "Dates",
        "\"CountDOWλ              →Count instances",
        "\"CountDOWλDV            →Count instances",
        1,
    );
    f.refuses("AboutDatesλ's table lists CountDOWλDV, which the library does not declare");
}

#[test]
fn an_about_table_cannot_call_an_undeclared_function() {
    let f = Fixture::published();
    f.edit(
        "Ratios",
        "\"Notes:                     →Sources include",
        "\"Notes:                     →Try DSIRatioλ( 1, 2). Sources include",
        1,
    );
    f.refuses("AboutRatiosλ's help calls DSIRatioλ, which the library does not declare");
}

#[test]
fn invalid_utf8_and_directory_source_entries_fail_without_writing() {
    for directory in [false, true] {
        let f = Fixture::published();
        for module in formula::MODULES {
            std::fs::remove_file(f.src.join(format!("{module}.txt"))).unwrap();
        }
        let invalid = f.src.join("Invalid.txt");
        if directory {
            std::fs::create_dir(&invalid).unwrap();
        } else {
            std::fs::write(&invalid, [0xff]).unwrap();
        }
        let result = f.cli(&[display(&f.src)], "utf-8");
        assert_eq!(result.status.code(), Some(1));
        assert!(result.stderr.is_empty());
        let text = String::from_utf8(result.stdout).unwrap();
        assert!(
            text.starts_with(&format!("FAIL: cannot read {}", display(&invalid))),
            "{text}"
        );
        assert!(!text.contains("Traceback") && !text.contains("OK:"));
    }
}

#[test]
fn physical_defaults_and_explicit_arguments_force_utf8_and_preserve_inputs() {
    let mut f = Fixture::published();
    let unicode = f.dir.path().join("sources-λ→");
    std::fs::create_dir(&unicode).unwrap();
    for module in formula::MODULES {
        let name = format!("{module}.txt");
        std::fs::copy(f.src.join(&name), unicode.join(name)).unwrap();
    }
    f.src = unicode;
    let expected = format!("{}{}", published_output().join(newline()), newline()).into_bytes();
    for encoding in ["cp1252", "utf-8"] {
        for args in [vec![], vec![display(&f.src)]] {
            let result = f.cli(&args, encoding);
            assert_eq!(result.status.code(), Some(0));
            assert!(result.stderr.is_empty());
            assert_eq!(result.stdout, expected);
        }
    }
}

#[test]
fn physical_surplus_arguments_fail_before_reading_or_writing() {
    let f = Fixture::published();
    let args = [
        display(&f.src),
        display(&f.dir.path().join("unrequested.xlsx")),
    ];
    let result = f.cli(&args, "utf-8");
    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty());
    assert_eq!(
        result.stderr,
        format!(
            "FAIL: usage: ozzit verify_signatures [src dir]{}",
            newline()
        )
        .into_bytes()
    );
}
