//! Source helper and command contracts retained from the Python tests.
use ozzit::{archive, common::*, formula, sources};
use std::{
    collections::{BTreeMap, BTreeSet},
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
    book: PathBuf,
    src: PathBuf,
}
impl Fixture {
    fn published() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let book = dir.path().join("ozzit.xlsx");
        let src = dir.path().join("src");
        std::fs::create_dir(&src).unwrap();
        std::fs::copy(root().join("ozzit.xlsx"), &book).unwrap();
        for module in formula::MODULES {
            let name = format!("{module}.txt");
            std::fs::copy(root().join("src").join(&name), src.join(name)).unwrap();
        }
        Self { dir, book, src }
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
    fn args(&self) -> Vec<String> {
        [&self.book, &self.src].map(|p| display(p)).to_vec()
    }
    fn run(&self, args: &[String]) -> Output {
        self.preserved(|| sources::verify(args))
    }
    fn cli(&self, args: &[String], encoding: &str) -> std::process::Output {
        self.preserved(|| {
            Command::new(env!("CARGO_BIN_EXE_ozzit"))
                .arg("verify_sources")
                .args(args)
                .env("PYTHONIOENCODING", encoding)
                .current_dir(self.dir.path())
                .output()
                .unwrap()
        })
    }
}

fn success(book: &str, src: &str) -> String {
    format!(
        "OK: {src}/ reproduces all 153 functions in {book}, every date conversion in them is read, and only the 5 listed argument defaults test a column inside OR() or AND()"
    )
}

#[test]
fn an_unterminated_literal_retains_its_text_and_end_offset() {
    assert_eq!(formula::read_string("\"abc", 0), ("\"abc", 4));
}

#[test]
fn an_unclosed_single_expression_is_left_as_written() {
    assert_eq!(formula::canonical("1+SINGLE(2"), "1+SINGLE(2");
}

#[test]
fn both_original_argument_lists_preserve_unclosed_and_nested_segments() {
    assert_eq!(
        formula::arguments("x, (y, z), w", 0),
        ["x", " (y, z)", " w"]
    );
    assert_eq!(
        formula::arguments("x, \"a,b\", [y,z], {1,2})tail", 0),
        ["x", " \"a,b\"", " [y,z]", " {1,2}"]
    );
}

#[test]
fn canonicalisation_preserves_all_five_literal_differences() {
    for (a, b) in [
        ("=LET(x, \"[0]!FY\", x)", "=LET(x, \"FY\", x)"),
        ("=LET(x, \"_xlfn.FY\", x)", "=LET(x, \"FY\", x)"),
        ("=LET(x, \"_xlop.Term\", x)", "=LET(x, \"[Term]\", x)"),
        ("=LET(x, \"SINGLE(1)\", x)", "=LET(x, \"@1\", x)"),
        ("=LET(x, \"XXFY\", x)", "=LET(x, \"FY\", x)"),
    ] {
        assert_ne!(
            formula::canonical(a),
            formula::canonical(b),
            "{a} versus {b}"
        );
    }
}

#[test]
fn canonicalisation_retains_all_five_original_equivalences_outside_literals() {
    for (a, b) in [
        ("=_xlfn.TEXTSPLIT(a)", "=TEXTSPLIT(a)"),
        ("=[0]!ozλ(1)", "=ozλ(1)"),
        ("=SINGLE(A1)", "=@A1"),
        ("=LAMBDA(_xlop.Term, 1)", "=LAMBDA([Term], 1)"),
        ("=SINGLE(IF(a,\"x\",b))", "=@IF(a,\"x\",b)"),
    ] {
        assert_eq!(
            formula::canonical(a),
            formula::canonical(b),
            "{a} versus {b}"
        );
    }
}

#[test]
fn the_known_collapsed_defaults_exactly_equal_all_published_source_occurrences() {
    let f = Fixture::published();
    let held = f.preserved(|| {
        let mut held = BTreeSet::new();
        for path in txt_files(&f.src).unwrap() {
            for statement in formula::statements(&read_text(&path).unwrap()) {
                for matched in captures(formula::NAME, &statement) {
                    held.extend(
                        formula::collapsed_defaults(&matched[2])
                            .into_iter()
                            .map(|parameter| (matched[1].clone(), parameter)),
                    );
                }
            }
        }
        held
    });
    assert_eq!(
        held,
        formula::KNOWN
            .map(|(f, p)| (f.to_owned(), p.to_owned()))
            .into()
    );
}

#[test]
fn published_sources_have_no_failures_and_retain_the_exact_known_count() {
    let f = Fixture::published();
    let output = f.run(&f.args());
    assert_eq!(output.code, 0, "{:?}", output.out);
    assert!(output.err.is_empty());
    assert_eq!(output.out, [success(&display(&f.book), &display(&f.src))]);
}

#[test]
fn all_four_unreadable_workbook_cases_fail_on_stdout_without_tracebacks() {
    let f = Fixture::published();
    let missing = f.dir.path().join("missing.xlsx");
    let not_zip = f.dir.path().join("not-a-zip.xlsx");
    std::fs::write(&not_zip, b"not a zip archive").unwrap();
    let no_book = f.dir.path().join("no-book.xlsx");
    archive::write_deterministic(
        &no_book,
        &[("docProps/app.xml".into(), b"<Properties/>".to_vec())].into(),
    )
    .unwrap();
    let undecodable = f.dir.path().join("undecodable.xlsx");
    archive::write_deterministic(
        &undecodable,
        &[(archive::BOOK.into(), b"<workbook>\xff</workbook>".to_vec())].into(),
    )
    .unwrap();
    for book in [not_zip, missing, no_book, undecodable] {
        let args = [display(&book), display(&f.src)];
        let output = f.cli(&args, "utf-8");
        assert_eq!(output.status.code(), Some(1));
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(
            stdout.starts_with(&format!("FAIL: cannot read {}", display(&book))),
            "{stdout}"
        );
        assert!(output.stderr.is_empty());
        assert!(!stdout.contains("Traceback") && !stdout.contains("OK:"));
    }
}

#[test]
fn undecodable_and_directory_source_modules_fail_without_writing_any_input() {
    for directory in [false, true] {
        let f = Fixture::published();
        for entry in std::fs::read_dir(&f.src).unwrap() {
            std::fs::remove_file(entry.unwrap().path()).unwrap();
        }
        let source = f.src.join("Invalid.txt");
        if directory {
            std::fs::create_dir(&source).unwrap();
        } else {
            std::fs::write(&source, b"\xff").unwrap();
        }
        let output = f.cli(&f.args(), "utf-8");
        assert_eq!(output.status.code(), Some(1));
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(
            stdout.starts_with(&format!("FAIL: cannot read {}", display(&source))),
            "{stdout}"
        );
        assert!(output.stderr.is_empty());
        assert!(!stdout.contains("Traceback") && !stdout.contains("OK:"));
    }
}

#[test]
fn physical_defaults_and_explicit_arguments_force_utf8_and_preserve_inputs() {
    let mut f = Fixture::published();
    let unicode = f.dir.path().join("valid-λ→.xlsx");
    std::fs::copy(&f.book, &unicode).unwrap();
    f.book = unicode;
    for encoding in ["cp1252", "utf-8"] {
        for length in 0..=2 {
            let args = f.args();
            let output = f.cli(&args[..length], encoding);
            assert_eq!(output.status.code(), Some(0));
            assert!(output.stderr.is_empty());
            let book = if length == 0 { "ozzit.xlsx" } else { &args[0] };
            let src = if length < 2 { "src" } else { &args[1] };
            assert_eq!(
                output.stdout,
                format!("{}{}", success(book, src), newline()).into_bytes()
            );
        }
    }
}

#[test]
fn physical_surplus_inputs_fail_before_reading_or_writing() {
    let f = Fixture::published();
    let mut args = f.args();
    args.push(display(&f.dir.path().join("unrequested.xlsx")));
    let output = f.cli(&args, "utf-8");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        format!(
            "FAIL: usage: ozzit verify_sources [workbook] [src dir]{}",
            newline()
        )
        .into_bytes()
    );
}
