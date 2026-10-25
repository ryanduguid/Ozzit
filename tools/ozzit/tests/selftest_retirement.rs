//! Preserve the original Python generator assertions against the native tools.
use ozzit::{common::*, formula, selftest};
use std::{collections::BTreeSet, path::PathBuf, process::Command};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn fragment() -> String {
    let bytes = std::fs::read(root().join(selftest::OUTPUT)).unwrap();
    assert!(bytes.is_ascii());
    String::from_utf8(bytes).unwrap().replace("\r\n", "\n")
}
fn body(rows: &[&str]) -> String {
    let text = rows.join("¶").replace('"', "\"\"");
    format!("LAMBDA([x], LET(Help, TRIM(TEXTSPLIT(\"{text}\", \"→\", \"¶\")), x))")
}
fn examples(rows: &[&str]) -> (Vec<selftest::Example>, Vec<(String, String)>) {
    selftest::examples_of(&body(rows))
}
fn shape(examples: &[selftest::Example]) -> Vec<(&str, Vec<&str>)> {
    examples
        .iter()
        .map(|e| {
            (
                e.formula.as_str(),
                e.results.iter().map(String::as_str).collect(),
            )
        })
        .collect()
}
fn library() -> BTreeSet<String> {
    ["Thisλ", "Timelineλ"].map(str::to_owned).into()
}
fn checks(formula: &str, results: &[&str], bindings: &[(&str, &str)]) -> Vec<String> {
    let example = selftest::Example {
        formula: formula.to_owned(),
        results: results.iter().map(|s| s.to_string()).collect(),
    };
    let bindings = bindings
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect::<Vec<_>>();
    selftest::checks_for("Thisλ", &example, 1, &library(), &bindings)
}

#[test]
fn every_shipped_function_keeps_its_help_assertion() {
    let text = fragment();
    assert_eq!(
        captures(
            r"(?m)^(?:Same|Near) 'help: ([A-Za-z0-9_]+)\$\{L\}(DV)?'",
            &text
        )
        .len(),
        153
    );
    for expected in [
        r#"Same 'help: CountDOW${L}' "INDEX(oz.CountDOW${L}(),1,1)" 'FUNCTION:'"#,
        r#"Same 'help: AboutDates${L}' "INDEX(oz.AboutDates${L},1,1)" 'About:'"#,
        r#"Near 'help: Amortise${L}DV' "--OR(INDEX(oz.Amortise${L}DV(),1,1)=TRUE,ISTEXT(INDEX(oz.Amortise${L}DV(),1,1)))" '1'"#,
    ] {
        assert!(text.contains(expected), "{expected}");
    }
}

#[test]
fn native_selftest_dot_sources_the_fragment_after_its_helpers() {
    let bytes = std::fs::read(root().join("tools/excel_selftest.ps1")).unwrap();
    assert!(bytes.is_ascii());
    let text = String::from_utf8(bytes).unwrap();
    let helpers = text.find("function Same(").unwrap();
    let sourced = text
        .find(". (Join-Path $PSScriptRoot 'selftest_examples.ps1')")
        .unwrap();
    assert!(sourced > helpers);
}

#[test]
fn examples_requiring_undefined_names_are_excluded() {
    let text = fragment();
    for name in ["tblRates", "ModelPeriodCount", "AmortisationSchedule"] {
        assert!(!text.contains(name));
    }
}

#[test]
fn result_labels_before_formulas_remain_examples() {
    let (found, bindings) = examples(&[
        "FUNCTION:→Thisλ(x)",
        "EXAMPLES:→",
        "Result→Formula (oz is assumed to be the module's name)",
        "3→=oz.Thisλ(1)",
        "TRUE→=Thisλ(2)",
    ]);
    assert!(bindings.is_empty());
    assert_eq!(
        shape(&found),
        [("oz.Thisλ(1)", vec!["3"]), ("Thisλ(2)", vec!["TRUE"])]
    );
}

#[test]
fn formulas_collect_the_following_result_rows() {
    let (found, _) = examples(&[
        "EXAMPLES:→",
        "→Formula (oz is assumed to be the module's name)",
        "→=oz.Thisλ(1000, 100, 5)",
        "→Result",
        "→180,180",
        "→180,180",
    ]);
    assert_eq!(
        shape(&found),
        [("oz.Thisλ(1000, 100, 5)", vec!["180,180", "180,180"])]
    );
}

#[test]
fn named_inputs_remain_ordered_bindings() {
    let (found, bindings) = examples(&[
        "EXAMPLES:→",
        "Result→Formula",
        "Dates→=EDATE(1, SEQUENCE(,24, 0))",
        "Values→={1,2}",
        "→",
        "Result→Formula",
        "25%→=oz.Thisλ( Values, Dates)",
    ]);
    assert_eq!(
        bindings,
        vec![
            ("Dates".to_owned(), "EDATE(1, SEQUENCE(,24, 0))".to_owned()),
            ("Values".to_owned(), "{1,2}".to_owned())
        ]
    );
    assert_eq!(shape(&found), [("oz.Thisλ( Values, Dates)", vec!["25%"])]);
}

#[test]
fn grids_accept_one_row_per_label_and_result_label_rows() {
    let (found, _) = examples(&[
        "EXAMPLES:→",
        "Result→Formula",
        "03, 07→=oz.Thisλ({1,2})",
        "07, 03→",
        "30, 70→",
    ]);
    assert_eq!(found[0].results, ["03, 07", "07, 03", "30, 70"]);
    let (found, _) = examples(&[
        "EXAMPLES:→NOTE! oz is assumed",
        "Formula:→=oz.Thisλ(0, SEQUENCE(1,5))",
        "Result:→00, 01",
        "→01, 02",
    ]);
    assert_eq!(found[0].results, ["00, 01", "01, 02"]);
}

#[test]
fn numbers_use_half_the_last_printed_digit_as_tolerance() {
    assert_eq!(
        checks("oz.Thisλ(1)", &["222.90"], &[]),
        [r#"Near 'example: This${L} #1' "oz.This${L}(1)" '222.9' '0.005'"#]
    );
    assert_eq!(
        checks("oz.Thisλ(1)", &["25%"], &[]),
        [r#"Near 'example: This${L} #1' "oz.This${L}(1)" '0.25' '0.005'"#]
    );
    let units = checks("oz.Thisλ(1)", &["40.89 days"], &[]);
    let pieces = units[0].split('\'').collect::<Vec<_>>();
    assert_eq!(pieces[pieces.len() - 2], "0.005");
}

#[test]
fn booleans_shapes_dates_words_and_lists_keep_their_exact_assertions() {
    for (formula, results, expected) in [
        (
            "Thisλ(1)",
            vec!["FALSE"],
            vec![r#"Near 'example: This${L} #1' "N(oz.This${L}(1))" '0'"#],
        ),
        (
            "oz.Thisλ(1)",
            vec!["5 rows, 60 cols"],
            vec![
                r#"Near 'example: This${L} #1' "ROWS(oz.This${L}(1))" '5'"#,
                r#"Near 'example: This${L} #1' "COLUMNS(oz.This${L}(1))" '60'"#,
            ],
        ),
        (
            "oz.Thisλ(1)",
            vec!["2026-10-01"],
            vec![r#"Near 'example: This${L} #1' "oz.This${L}(1) - DATE(2026,10,1)" '0' '0.5'"#],
        ),
        (
            "oz.Thisλ(1)",
            vec!["FY2027"],
            vec![r#"Same 'example: This${L} #1' "oz.This${L}(1)" 'FY2027'"#],
        ),
        (
            "oz.Thisλ(1)",
            vec!["1,3", "6,10"],
            vec![
                r#"Near 'example: This${L} #1' "COUNT(oz.This${L}(1))" '4'"#,
                r#"Near 'example: This${L} #1' "SUM(oz.This${L}(1))" '20.0' '2.0'"#,
            ],
        ),
    ] {
        assert_eq!(checks(formula, &results, &[]), expected);
    }
}

#[test]
fn totals_and_percentages_preserve_the_decimal_output() {
    assert_eq!(
        checks("oz.Thisλ(1)", &["0.1,0.2", "0.3"], &[]),
        [
            r#"Near 'example: This${L} #1' "COUNT(oz.This${L}(1))" '3'"#,
            r#"Near 'example: This${L} #1' "SUM(oz.This${L}(1))" '0.6' '0.15'"#
        ]
    );
    assert_eq!(
        checks("oz.Thisλ(1)", &["12.6122%"], &[]),
        [r#"Near 'example: This${L} #1' "oz.This${L}(1)" '0.126122' '5e-07'"#]
    );
    // Exact Decimal components from the original tuple assertions are checked
    // inside selftest.rs; shown() would hide rounding behind an f64 conversion.
}

#[test]
fn unrecognised_results_are_checked_for_excel_errors() {
    assert_eq!(
        checks("oz.Thisλ(TODAY())", &["12 mo, starts"], &[]),
        [r#"Near 'example: This${L} #1' "SUMPRODUCT(--ISERROR(oz.This${L}(TODAY())))" '0'"#]
    );
}

#[test]
fn bindings_use_let_and_escape_quotes_for_powershell() {
    assert_eq!(
        checks("oz.Thisλ(Values, \"a\")", &["3"], &[("Values", "{1,2}")]),
        [
            r#"Near 'example: This${L} #1' "LET(Values, {1,2}, oz.This${L}(Values, `"a`"))" '3.0' '0.5'"#
        ]
    );
}

#[test]
fn undefined_names_table_and_cell_references_are_skipped() {
    for formula in [
        "oz.Thisλ(Array, Labels)",
        "oz.Thisλ(tbl[Col])",
        "oz.Thisλ(A1, 2)",
    ] {
        assert!(checks(formula, &["3"], &[]).is_empty());
    }
    assert_eq!(
        checks("oz.Thisλ(Timelineλ(1), TRUE)", &["3"], &[])[0]
            .matches("oz.Timeline${L}(")
            .count(),
        1
    );
}

#[test]
fn an_unterminated_literal_runs_to_the_end() {
    assert_eq!(
        selftest::read_literals("x(\"ab\"\"c\", \"open"),
        "ab\"copen"
    );
}

#[test]
fn an_empty_result_label_collects_following_rows() {
    let (found, _) = examples(&["EXAMPLES:→", "→=oz.Thisλ(1)", "Result→", "→3"]);
    assert_eq!(shape(&found), [("oz.Thisλ(1)", vec!["3"])]);
}

#[test]
fn a_sign_and_percent_without_digits_is_not_numeric() {
    assert!(selftest::number("-%").is_none());
    assert_eq!(
        checks("oz.Thisλ(1)", &["-%"], &[]),
        [r#"Near 'example: This${L} #1' "SUMPRODUCT(--ISERROR(oz.This${L}(1)))" '0'"#]
    );
}

#[test]
fn unknown_functions_do_not_stand_alone() {
    assert!(!selftest::stands_alone(
        "Unknown(1)",
        &library(),
        &BTreeSet::new()
    ));
}

#[test]
fn a_binding_that_does_not_stand_alone_drops_the_example() {
    assert!(checks("oz.Thisλ(Values)", &["3"], &[("Values", "Unknown(1)")]).is_empty());
}

struct Fixture {
    directory: tempfile::TempDir,
    src: PathBuf,
    output: PathBuf,
}
impl Fixture {
    fn new(formula: &str) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let src = directory.path().join("src");
        std::fs::create_dir(&src).unwrap();
        for module in formula::MODULES {
            let text = if module == "Dates" {
                let help = body(&[
                    "FUNCTION:→Datesλ(x)",
                    "EXAMPLES:→",
                    "Result→Formula",
                    &format!("3→={formula}"),
                ]);
                format!("Datesλ = {help};\nBROKEN;\n")
            } else {
                format!("{module}λ = LAMBDA([x], x + 1);\n")
            };
            std::fs::write(src.join(format!("{module}.txt")), text).unwrap();
        }
        let output = directory.path().join("fragment.ps1");
        Self {
            directory,
            src,
            output,
        }
    }
    fn run(&self, args: &[&str]) -> (i32, String, String) {
        let result = Command::new(env!("CARGO_BIN_EXE_ozzit"))
            .arg("generate_selftest_examples")
            .args(args)
            .current_dir(self.directory.path())
            .output()
            .unwrap();
        (
            result.status.code().unwrap(),
            String::from_utf8(result.stdout)
                .unwrap()
                .replace("\r\n", "\n"),
            String::from_utf8(result.stderr)
                .unwrap()
                .replace("\r\n", "\n"),
        )
    }
}

#[test]
fn too_many_arguments_print_native_usage() {
    let fixture = Fixture::new("oz.Datesλ(1)");
    let (code, out, err) = fixture.run(&["a", "b", "c"]);
    assert_eq!(code, 1);
    assert_eq!(out, "");
    assert_eq!(
        err,
        "FAIL: usage: ozzit generate_selftest_examples [src dir] [output]\n"
    );
    assert!(!fixture.output.exists());
}

#[test]
fn the_fragment_is_written_once_then_reported_current() {
    let fixture = Fixture::new("oz.Datesλ(1)");
    let text = selftest::generate(&fixture.src).unwrap();
    assert!(text.contains(r#"Same 'help: Dates${L}' "INDEX(oz.Dates${L}(),1,1)" 'FUNCTION:'"#));
    assert!(text.contains("# --- 1 worked examples that stand on their own"));
    assert!(text.contains(r#"Near 'example: Dates${L} #1' "oz.Dates${L}(1)" '3.0' '0.5'"#));
    assert!(!text.contains("BROKEN"));
    let src = display(&fixture.src);
    let output = display(&fixture.output);
    let (code, out, err) = fixture.run(&[&src, &output]);
    assert_eq!((code, err.as_str()), (0, ""));
    assert_eq!(
        out,
        format!("wrote {output}: {} lines\n", text.matches('\n').count())
    );
    assert_eq!(std::fs::read(&fixture.output).unwrap(), text.as_bytes());
    let (code, out, err) = fixture.run(&[&src, &output]);
    assert_eq!((code, err.as_str()), (0, ""));
    assert_eq!(out, format!("already current: {output}\n"));
    assert_eq!(std::fs::read(&fixture.output).unwrap(), text.as_bytes());
}

#[test]
fn non_ascii_generation_is_refused_without_writing() {
    let fixture = Fixture::new("oz.Datesλ(\"café\")");
    let src = display(&fixture.src);
    let output = display(&fixture.output);
    let (code, out, err) = fixture.run(&[&src, &output]);
    assert_eq!((code, out.as_str()), (1, ""));
    assert_eq!(err, "FAIL: the generated fragment is not pure ASCII\n");
    assert!(!fixture.output.exists());
    std::fs::write(&fixture.output, b"sentinel").unwrap();
    let (code, _, _) = fixture.run(&[&src, &output]);
    assert_eq!(code, 1);
    assert_eq!(std::fs::read(&fixture.output).unwrap(), b"sentinel");
}
