//! Assertion-level replacements for the retired Python module-builder tests.
use ozzit::{common::*, compiler, formula, module};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    process::Command,
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn published() -> Vec<u8> {
    std::fs::read(root().join("oz.txt")).unwrap()
}
fn findings(text: &str) -> Vec<String> {
    module::check(
        text,
        &root().join("ozzit.xlsx"),
        &root().join("functions.csv"),
    )
    .unwrap()
}
struct Capture {
    code: i32,
    out: String,
    err: String,
}
struct Fixture {
    directory: tempfile::TempDir,
    src: PathBuf,
    workbook: PathBuf,
    index: PathBuf,
    output: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let src = directory.path().join("src");
        std::fs::create_dir(&src).unwrap();
        for name in formula::MODULES {
            let path = format!("{name}.txt");
            std::fs::copy(root().join("src").join(&path), src.join(&path)).unwrap();
        }
        let workbook = directory.path().join("ozzit.xlsx");
        let index = directory.path().join("functions.csv");
        std::fs::copy(root().join("ozzit.xlsx"), &workbook).unwrap();
        std::fs::copy(root().join("functions.csv"), &index).unwrap();
        let output = directory.path().join("oz.txt");
        Self {
            directory,
            src,
            workbook,
            index,
            output,
        }
    }
    fn args(&self) -> Vec<String> {
        [&self.src, &self.output, &self.workbook, &self.index]
            .map(|p| display(p))
            .to_vec()
    }
    fn run(&self, args: &[String]) -> Capture {
        let result = Command::new(env!("CARGO_BIN_EXE_ozzit"))
            .arg("build_module")
            .args(args)
            .current_dir(self.directory.path())
            .output()
            .unwrap();
        Capture {
            code: result.status.code().unwrap(),
            // Python's subprocess text=True normalised platform line endings.
            out: String::from_utf8(result.stdout)
                .unwrap()
                .replace("\r\n", "\n"),
            err: String::from_utf8(result.stderr)
                .unwrap()
                .replace("\r\n", "\n"),
        }
    }
    fn columns(&self, columns: &[&str], overrides: &[(usize, &str)], header_only: bool, bom: bool) {
        let original = Csv::read(&root().join("functions.csv")).unwrap();
        let mut text = if bom {
            "\u{feff}".to_owned()
        } else {
            String::new()
        };
        text.push_str(&csv_row(
            &columns.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        ));
        if !header_only {
            for row in &original.rows {
                let mut values = columns
                    .iter()
                    .map(|c| original.get(row, c).unwrap_or("extra,\nvalue").to_owned())
                    .collect::<Vec<_>>();
                for &(position, value) in overrides {
                    values[position] = value.to_owned();
                }
                text.push_str(&csv_row(&values));
            }
        }
        std::fs::write(&self.index, text).unwrap();
    }
}

#[test]
fn repeated_function_columns_fail_before_destination_handling() {
    let fixture = Fixture::new();
    for identical in [false, true] {
        for checking in [false, true] {
            for destination in ["absent", "current", "stale"] {
                fixture.columns(
                    &["function", "function"],
                    if identical { &[] } else { &[(0, "")] },
                    false,
                    false,
                );
                let before = std::fs::read(&fixture.index).unwrap();
                let original = if destination == "current" {
                    published()
                } else {
                    b"/* keep temporary output */\n".to_vec()
                };
                if destination == "absent" {
                    if fixture.output.exists() {
                        std::fs::remove_file(&fixture.output).unwrap();
                    }
                } else {
                    std::fs::write(&fixture.output, &original).unwrap();
                }
                let mut args = fixture.args();
                if checking {
                    args.push("--check".into());
                }
                let result = fixture.run(&args);
                assert_eq!(result.code, 1, "{}{}", result.out, result.err);
                for message in ["duplicate CSV column", "function", "functions.csv"] {
                    assert!(result.out.contains(message), "{}", result.out);
                }
                assert!(!result.err.contains("Traceback"));
                assert!(!result.out.contains("wrote "));
                assert_eq!(std::fs::read(&fixture.index).unwrap(), before);
                if destination == "absent" {
                    assert!(!fixture.output.exists());
                } else {
                    assert_eq!(std::fs::read(&fixture.output).unwrap(), original);
                }
            }
        }
    }
}

#[test]
fn direct_check_keeps_text_findings_when_csv_headers_repeat() {
    let fixture = Fixture::new();
    let text = module::build(&fixture.src).unwrap() + "BROKEN;\n";
    for (columns, overrides, empty) in [
        (vec!["function", "function"], vec![], false),
        (vec!["function", "module", "function"], vec![(2, "")], false),
        (vec!["function", "function", "function"], vec![], false),
        (vec!["function", "function"], vec![], true),
    ] {
        fixture.columns(&columns, &overrides, empty, false);
        let found = module::check(&text, &fixture.workbook, &fixture.index).unwrap();
        assert!(
            found.iter().any(|f| f.contains("duplicate CSV column")),
            "{found:?}"
        );
        assert!(
            found
                .iter()
                .any(|f| f.contains("not a declaration") && f.contains("BROKEN")),
            "{found:?}"
        );
    }
}

#[test]
fn ignored_columns_reordered_indexes_and_boms_remain_usable() {
    let fixture = Fixture::new();
    let text = module::build(&fixture.src).unwrap();
    for (columns, overrides, bom) in [
        (vec!["module", "function"], vec![], false),
        (vec!["module", "function"], vec![], true),
        (
            vec!["description", "function", "description"],
            vec![(0, "ignored contradiction")],
            false,
        ),
        (
            vec!["extra", "function", "extra"],
            vec![(0, "ignored contradiction")],
            false,
        ),
        (vec!["Function", "extra", "function"], vec![], true),
    ] {
        fixture.columns(&columns, &overrides, false, bom);
        assert_eq!(
            module::check(&text, &fixture.workbook, &fixture.index).unwrap(),
            Vec::<String>::new()
        );
    }
}

#[test]
fn tracked_module_is_the_current_join_and_passes_its_inventory_checks() {
    let text = module::build(&root().join("src")).unwrap();
    assert_eq!(published(), text.as_bytes());
    assert!(findings(&text).is_empty());
}

#[test]
fn joined_names_match_workbook_and_index_including_all_five_help_tables() {
    let text = String::from_utf8(published()).unwrap();
    let declared = module::declared_names(&text);
    let unique = declared.iter().collect::<BTreeSet<_>>();
    assert_eq!(declared.len(), unique.len());
    let qualified = declared
        .iter()
        .map(|n| format!("oz.{n}"))
        .collect::<BTreeSet<_>>();
    let csv = Csv::read(&root().join("functions.csv")).unwrap();
    let indexed = csv
        .rows
        .iter()
        .map(|r| csv.get(r, "function").unwrap().to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(qualified, indexed);
    assert_eq!(
        qualified,
        compiler::workbook_names(&root().join("ozzit.xlsx")).unwrap()
    );
    let mut help = declared
        .iter()
        .filter(|n| n.starts_with("About"))
        .map(String::as_str)
        .collect::<Vec<_>>();
    help.sort();
    assert_eq!(
        help,
        [
            "AboutDatesλ",
            "AboutEssentialsλ",
            "AboutFinancialλ",
            "AboutRatiosλ",
            "AboutUtilitiesλ"
        ]
    );
    assert!(declared.len() >= 130);
}

#[test]
fn join_is_lf_only_with_final_newline_and_single_namespace_prefix() {
    let raw = published();
    assert!(!raw.contains(&b'\r'));
    assert!(raw.ends_with(b"\n"));
    assert!(!String::from_utf8(raw.clone()).unwrap().contains("oz.oz."));
    assert!(raw.starts_with(b"/*  Ozzit:"));
}

#[test]
fn check_rejects_duplicate_case_changed_missing_and_stray_declarations() {
    let text = module::build(&root().join("src")).unwrap();
    let first = text.split(";\n").find(|s| s.contains("= LAMBDA(")).unwrap();
    let duplicated = text.replacen(&format!("{first};\n"), &format!("{first};\n{first};\n"), 1);
    assert!(
        findings(&duplicated)
            .iter()
            .any(|f| f.contains("more than once"))
    );
    let name = module::declared_names(&text).pop().unwrap();
    let folded = text.replacen(
        &format!("\n{name} ="),
        &format!("\n{} =", name.to_uppercase()),
        1,
    );
    assert!(
        findings(&folded)
            .iter()
            .any(|f| f.contains(&name) && f.contains("does not declare"))
    );
    assert!(
        findings(&text.replacen('\n', "\r\n", 1))
            .iter()
            .any(|f| f.contains("carriage return"))
    );
    assert!(
        findings(&(text + "BROKEN;\n"))
            .iter()
            .any(|f| f.contains("not a declaration") && f.contains("BROKEN"))
    );
}

#[test]
fn cli_check_reports_stale_then_write_and_recheck_pass_without_further_changes() {
    let fixture = Fixture::new();
    std::fs::write(&fixture.output, b"/* stale */\n").unwrap();
    let args = fixture.args();
    let mut checking = args.clone();
    checking.push("--check".into());
    let result = fixture.run(&checking);
    assert_eq!(result.code, 1);
    assert!(result.out.contains("stale"));
    assert!(result.out.contains("run ozzit build_module"));
    assert_eq!(std::fs::read(&fixture.output).unwrap(), b"/* stale */\n");
    let result = fixture.run(&args);
    assert_eq!(result.code, 0, "{}{}", result.out, result.err);
    assert_eq!(std::fs::read(&fixture.output).unwrap(), published());
    let result = fixture.run(&checking);
    assert_eq!(result.code, 0);
    assert!(result.out.contains("OK"));
    assert_eq!(std::fs::read(&fixture.output).unwrap(), published());
}

#[test]
fn cli_rejects_unknown_options_and_extra_paths_without_writing() {
    let fixture = Fixture::new();
    for extra in ["--chek", "--chec", "--unknown", "extra.txt"] {
        std::fs::write(&fixture.output, b"sentinel").unwrap();
        let mut args = fixture.args();
        args.push(extra.into());
        let result = fixture.run(&args);
        assert_eq!(result.code, 2);
        assert!(result.err.contains("unrecognized arguments"));
        assert_eq!(std::fs::read(&fixture.output).unwrap(), b"sentinel");
    }
}

#[test]
fn cli_rejects_invalid_index_names_without_writing() {
    let fixture = Fixture::new();
    let original = Csv::read(&fixture.index).unwrap();
    let names = original
        .rows
        .iter()
        .map(|r| original.get(r, "function").unwrap().to_owned())
        .collect::<Vec<_>>();
    for name in [
        None,
        Some("".into()),
        Some(" \t".into()),
        Some(format!("xx.{}", &names[0][3..])),
        Some(format!("ozx{}", &names[0][3..])),
    ] {
        for checking in [false, true] {
            let mut text = "\u{feff}module,function\n".to_owned();
            text.push_str(&csv_row(&match &name {
                Some(n) => vec!["fixture".into(), n.clone()],
                None => vec!["fixture".into()],
            }));
            for name in &names[1..] {
                text.push_str(&csv_row(&["fixture".into(), name.clone()]));
            }
            std::fs::write(&fixture.index, text.replace('\n', "\r\n")).unwrap();
            let before = if checking {
                published()
            } else {
                b"sentinel".to_vec()
            };
            std::fs::write(&fixture.output, &before).unwrap();
            let mut args = fixture.args();
            if checking {
                args.push("--check".into());
            }
            let result = fixture.run(&args);
            assert_eq!(result.code, 1);
            assert!(result.out.contains("FAIL:") && result.out.contains("functions.csv"));
            assert!(!result.err.contains("Traceback"));
            assert_eq!(std::fs::read(&fixture.output).unwrap(), before);
        }
    }
}

#[test]
fn cli_rejects_missing_function_column_without_writing() {
    let fixture = Fixture::new();
    std::fs::write(&fixture.index, "module\nfixture\n").unwrap();
    std::fs::write(&fixture.output, b"sentinel").unwrap();
    let result = fixture.run(&fixture.args());
    assert_eq!(result.code, 1);
    assert!(result.out.contains("FAIL:") && result.out.contains("function"));
    assert!(!result.err.contains("Traceback"));
    assert_eq!(std::fs::read(&fixture.output).unwrap(), b"sentinel");
}

#[test]
fn cli_help_lists_arguments_without_reading_inputs_or_writing_output() {
    let fixture = Fixture::new();
    std::fs::write(&fixture.output, b"sentinel").unwrap();
    let args = vec![
        "missing-src".into(),
        display(&fixture.output),
        "missing.xlsx".into(),
        "missing.csv".into(),
        "--help".into(),
    ];
    let result = fixture.run(&args);
    assert_eq!(result.code, 0);
    for argument in ["src", "output", "workbook", "index", "--check"] {
        assert!(result.out.contains(argument));
    }
    assert_eq!(std::fs::read(&fixture.output).unwrap(), b"sentinel");
}

#[test]
fn cli_check_preserves_all_optional_path_defaults() {
    let fixture = Fixture::new();
    std::fs::write(&fixture.output, published()).unwrap();
    let paths = ["src", "oz.txt", "ozzit.xlsx", "functions.csv"].map(str::to_owned);
    for count in 0..=4 {
        let mut args = paths[..count].to_vec();
        args.push("--check".into());
        let result = fixture.run(&args);
        assert_eq!(result.code, 0, "{}{}", result.out, result.err);
        assert!(result.out.contains("OK"));
        assert_eq!(std::fs::read(&fixture.output).unwrap(), published());
    }
}

#[test]
fn cli_check_accepts_options_between_paths_without_writing() {
    let fixture = Fixture::new();
    for position in 0..=4 {
        std::fs::write(&fixture.output, b"sentinel").unwrap();
        let mut args = fixture.args();
        args.insert(position, "--check".into());
        let result = fixture.run(&args);
        assert_eq!(result.code, 1);
        assert!(result.out.contains("stale"));
        assert_eq!(std::fs::read(&fixture.output).unwrap(), b"sentinel");
    }
}

#[test]
fn text_only_failures_retain_exact_diagnostics() {
    let text = module::build(&root().join("src")).unwrap();
    assert_eq!(
        findings(text.trim_end_matches('\n')),
        ["module text does not end with a newline"]
    );
    assert_eq!(
        findings(&(text + "// oz.oz.Thing\n")),
        ["module text contains a doubled oz. prefix"]
    );
}

#[test]
fn extra_declaration_is_reported_against_both_inventories_before_writing() {
    let expected = vec![
        "module text declares Extraλ, which the functions.csv does not ship".to_owned(),
        "module text declares Extraλ, which the workbook does not ship".to_owned(),
    ];
    let fixture = Fixture::new();
    let path = fixture.src.join("Debt.txt");
    let text = std::fs::read_to_string(&path).unwrap() + "\nExtraλ = LAMBDA(x, x);\n";
    std::fs::write(path, text).unwrap();
    assert_eq!(
        module::check(
            &module::build(&fixture.src).unwrap(),
            &fixture.workbook,
            &fixture.index
        )
        .unwrap(),
        expected
    );
    let result = fixture.run(&fixture.args());
    assert_eq!(result.code, 1);
    assert_eq!(
        result.out,
        expected
            .iter()
            .map(|s| format!("FAIL: {s}\n"))
            .collect::<String>()
    );
    assert!(!fixture.output.exists());
}
