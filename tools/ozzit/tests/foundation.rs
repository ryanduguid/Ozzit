//! Native counterparts of the compiler, source, help and index contracts.
use ozzit::{archive, common::*, compiler, formula, help, index, sources};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn library() -> BTreeSet<String> {
    ["Otherλ", "Thisλ"].map(String::from).into_iter().collect()
}
fn render(body: &str) -> String {
    formula::render(body, &library()).unwrap()
}
struct Fixture {
    _dir: tempfile::TempDir,
    workbook: PathBuf,
    src: PathBuf,
    index: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let workbook = dir.path().join("ozzit.xlsx");
        let src = dir.path().join("src");
        let index = dir.path().join("functions.csv");
        std::fs::copy(root().join("ozzit.xlsx"), &workbook).unwrap();
        std::fs::copy(root().join("functions.csv"), &index).unwrap();
        std::fs::create_dir(&src).unwrap();
        for module in formula::MODULES {
            std::fs::copy(
                root().join(format!("src/{module}.txt")),
                src.join(format!("{module}.txt")),
            )
            .unwrap();
        }
        Self {
            _dir: dir,
            workbook,
            src,
            index,
        }
    }
    fn edit(&self, module: &str, before: &str, after: &str) {
        let path = self.src.join(format!("{module}.txt"));
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.matches(before).count(), 1);
        std::fs::write(path, text.replace(before, after)).unwrap();
    }
    fn args(&self) -> Vec<String> {
        vec![display(&self.workbook), display(&self.src)]
    }
}

#[test]
fn compiler_marks_parameters_bindings_and_new_excel_functions() {
    let stored = render(
        "LAMBDA([Values], [Size], LET(Help, TRIM(TEXTSPLIT(\"FUNCTION: →Thisλ(Values)¶\", \"→\", \"¶\")), Rows, ROWS(Values), Kept, FILTER(Values, Values > Size), CHOOSE(Rows, Kept, Help)))",
    );
    for marker in [
        "_xlfn.LAMBDA(_xlop.Values, _xlop.Size, _xlfn.LET(",
        "_xlpm.Help, TRIM(_xlfn.TEXTSPLIT(",
        "_xlpm.Rows, ROWS(_xlpm.Values)",
        "_xlfn._xlws.FILTER(_xlpm.Values, _xlpm.Values > _xlpm.Size)",
        "CHOOSE(_xlpm.Rows, _xlpm.Kept, _xlpm.Help)",
        "\"FUNCTION: →Thisλ(Values)¶\"",
    ] {
        assert!(stored.contains(marker), "{marker}: {stored}");
    }
}
#[test]
fn compiler_qualifies_calls_and_normalises_binding_case() {
    assert!(
        render("LAMBDA([x], LET(Mpp, 12, Switch(x, 1, Otherλ(X / mpp), 0)))")
            .contains("_xlfn.SWITCH(_xlpm.x, 1, oz.Otherλ(_xlpm.x / _xlpm.Mpp), 0)")
    );
}
#[test]
fn compiler_marks_implicit_intersection() {
    let stored = render("LAMBDA([Items], LET(First, @INDEX(Items, 1), @First))");
    assert!(stored.contains("_xlpm.First, _xlfn.SINGLE(INDEX(_xlpm.Items, 1))"));
    assert!(stored.contains("_xlfn.SINGLE(_xlpm.First)"));
}
#[test]
fn compiler_preserves_error_literals_and_booleans() {
    assert!(
        render("LAMBDA([x], IF(x = true, #Value!, #N/A))")
            .contains("IF(_xlpm.x = TRUE, #VALUE!, #N/A)")
    );
}
#[test]
fn unknown_identifiers_are_refused() {
    assert!(
        formula::render("LAMBDA([x], x + Undeclared)", &library())
            .unwrap_err()
            .contains("Undeclared reached the stored form")
    );
}
#[test]
fn nested_lambdas_and_lets_keep_their_binding_markers() {
    assert!(render("LAMBDA([Values], MAP(Values, LAMBDA(v, LET(Twice, v * 2, Twice))))").contains("_xlfn.MAP(_xlpm.Values, _xlfn.LAMBDA(_xlpm.v, _xlfn.LET(_xlpm.Twice, _xlpm.v * 2, _xlpm.Twice)))"));
}
#[test]
fn function_named_bindings_are_functions_when_called() {
    assert!(
        render("LAMBDA([Values], LET(Rows, ROWS(Values), Rows + 1))")
            .contains("_xlpm.Rows, ROWS(_xlpm.Values), _xlpm.Rows + 1")
    );
}
#[test]
fn header_comments_encode_line_breaks_and_limit_characters() {
    let text = format!(
        "/* FUNCTION NAME: Thisλ\n DESCRIPTION:*//**First line \r\n    second line*/\nThisλ = LAMBDA([x], x);\n/* FUNCTION NAME: Longλ\n DESCRIPTION:*//**{}*/",
        "λ".repeat(300)
    );
    let comments = formula::header_comments(&text);
    assert_eq!(comments["Thisλ"], "First line _x000a_    second line");
    assert_eq!(comments["Longλ"], "λ".repeat(255));
}
fn compiled(name: &str) -> compiler::Compiled {
    compiler::Compiled {
        name: name.into(),
        module: "Utilities".into(),
        body: "3".into(),
        stored: "3".into(),
        comment: String::new(),
    }
}
#[test]
fn inserted_names_are_sorted_without_rewriting_existing_definitions() {
    let book = "<definedNames><definedName name=\"oz.Alpha\"> 1 </definedName><definedName name=\"oz.Zeta\"> 2 </definedName></definedNames>";
    let result =
        compiler::insert_names(book, &[compiled("oz.charlie"), compiled("oz.Beta")]).unwrap();
    let doc = roxmltree::Document::parse(&result).unwrap();
    assert_eq!(
        doc.root_element()
            .children()
            .filter(|n| n.is_element())
            .map(|n| n.attribute("name").unwrap())
            .collect::<Vec<_>>(),
        ["oz.Alpha", "oz.Beta", "oz.charlie", "oz.Zeta"]
    );
    assert!(result.contains("<definedName name=\"oz.Alpha\"> 1 </definedName>"));
}
#[test]
fn inserted_names_comments_and_formulae_stay_xml_text() {
    let mut name = compiled("oz.B&<λ");
    name.stored = "\"<&>\"".into();
    name.comment = "A \"quote\" & <tag>".into();
    let result = compiler::insert_names(
        "<definedNames><definedName name=\"oz.Alpha\">1</definedName></definedNames>",
        &[name.clone()],
    )
    .unwrap();
    let doc = roxmltree::Document::parse(&result).unwrap();
    let inserted = doc
        .root_element()
        .children()
        .filter(|n| n.is_element())
        .nth(1)
        .unwrap();
    assert_eq!(inserted.attribute("name"), Some(name.name.as_str()));
    assert_eq!(inserted.attribute("comment"), Some(name.comment.as_str()));
    assert_eq!(inserted.text(), Some(name.stored.as_str()));
}
#[test]
fn an_insertion_before_every_existing_name_is_refused() {
    assert!(
        compiler::insert_names(
            "<definedNames><definedName name=\"oz.Anchor\">1</definedName></definedNames>",
            &[compiled("oz.Aardvark")]
        )
        .unwrap_err()
        .contains("sorts before every shipped name")
    );
}
#[test]
fn the_archive_writer_reproduces_the_frozen_workbook_byte_for_byte() {
    let path = root().join("ozzit.xlsx");
    assert_eq!(
        archive::deterministic_bytes(&archive::read_parts(&path).unwrap()).unwrap(),
        std::fs::read(path).unwrap()
    );
}
#[test]
fn tracked_sources_need_no_compilation_or_index_rewrite() {
    let fixture = Fixture::new();
    let before = std::fs::read(&fixture.workbook).unwrap();
    assert!(
        compiler::run(&fixture.workbook, &fixture.src, true, None)
            .unwrap()
            .is_empty()
    );
    assert!(
        compiler::run(&fixture.workbook, &fixture.src, false, Some(&fixture.index))
            .unwrap()
            .is_empty()
    );
    assert_eq!(std::fs::read(&fixture.workbook).unwrap(), before);
    assert_eq!(
        std::fs::read(&fixture.index).unwrap(),
        std::fs::read(root().join("functions.csv")).unwrap()
    );
}
#[test]
fn a_single_recompiled_definition_preserves_every_other_part_and_is_idempotent() {
    let fixture = Fixture::new();
    let before = archive::read_parts(&fixture.workbook).unwrap();
    fixture.edit(
        "Essentials",
        "        Result,         ISNUMBER( XMATCH( Value, TOCOL( List), 0)),",
        "        Result,         ISNUMBER( XMATCH( Value, TOCOL( List), 0)) + 0,",
    );
    assert_eq!(
        compiler::run(&fixture.workbook, &fixture.src, false, None).unwrap(),
        ["oz.IsInListλ"]
    );
    let after = archive::read_parts(&fixture.workbook).unwrap();
    for (name, bytes) in &before {
        if name != archive::BOOK {
            assert_eq!(after.get(name), Some(bytes), "{name}");
        }
    }
    let before_names = compiler::shipped_names(&archive::read_book(&before).unwrap());
    let after_names = compiler::shipped_names(&archive::read_book(&after).unwrap());
    for (name, content) in &before_names {
        if name != "oz.IsInListλ" {
            assert_eq!(after_names.get(name), Some(content), "{name}");
        }
    }
    assert!(
        compiler::run(&fixture.workbook, &fixture.src, false, None)
            .unwrap()
            .is_empty()
    );
}
#[test]
fn whitespace_inside_literals_changes_the_definition() {
    let fixture = Fixture::new();
    fixture.edit(
        "Financial",
        "EXPAND({\" \"}, 1, COUNTA(Timeline), \" \")",
        "EXPAND({\"  \"}, 1, COUNTA(Timeline), \"  \")",
    );
    assert_eq!(
        compiler::run(&fixture.workbook, &fixture.src, true, None).unwrap(),
        ["oz.Amortiseλ"]
    );
    assert_eq!(
        compiler::run(&fixture.workbook, &fixture.src, false, None).unwrap(),
        ["oz.Amortiseλ"]
    );
    assert!(
        formula::tight(
            &archive::read_book(&archive::read_parts(&fixture.workbook).unwrap()).unwrap()
        )
        .contains("_xlfn.EXPAND({\"  \"},1,COUNTA(_xlpm.Timeline),\"  \")")
    );
}
#[test]
fn header_changes_keep_the_name_manager_comment_and_index_together() {
    let fixture = Fixture::new();
    fixture.edit(
        "Ratios",
        "/**Measures the extent of a company’s gearing */",
        "/**Leverage, the short way*/",
    );
    assert_eq!(
        compiler::run(&fixture.workbook, &fixture.src, false, Some(&fixture.index)).unwrap(),
        ["oz.DebtRatioλ", "functions.csv"]
    );
    let csv = Csv::read(&fixture.index).unwrap();
    let row = csv
        .rows
        .iter()
        .find(|r| csv.get(r, "function") == Some("oz.DebtRatioλ"))
        .unwrap();
    assert_eq!(csv.get(row, "description"), Some("Leverage, the short way"));
    assert_eq!(csv.get(row, "previous_name"), Some("nabla.r.DebtRatioλ"));
}
#[test]
fn malformed_index_is_refused_before_the_workbook_changes() {
    let fixture = Fixture::new();
    fixture.edit(
        "Ratios",
        "/**Measures the extent of a company’s gearing */",
        "/**Leverage, the short way*/",
    );
    std::fs::write(&fixture.index, "function,module\noz.DebtRatioλ,Ratios\n").unwrap();
    let before = std::fs::read(&fixture.workbook).unwrap();
    assert!(
        compiler::run(&fixture.workbook, &fixture.src, false, Some(&fixture.index))
            .unwrap_err()
            .contains("unexpected columns")
    );
    assert_eq!(std::fs::read(&fixture.workbook).unwrap(), before);
}
#[test]
fn new_source_names_must_be_added_to_the_workbook_before_compiling() {
    let fixture = Fixture::new();
    let path = fixture.src.join("Debt.txt");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str(
        "\n/* FUNCTION NAME: Extraλ\n DESCRIPTION:*//**Extra*/\nExtraλ = LAMBDA([x], x);\n",
    );
    std::fs::write(path, text).unwrap();
    assert!(
        compiler::run(&fixture.workbook, &fixture.src, true, None)
            .unwrap_err()
            .contains("oz.Extraλ is declared in src/ but does not ship")
    );
}
#[test]
fn source_and_help_checks_read_the_whole_shipped_library() {
    let fixture = Fixture::new();
    let result = sources::verify(&fixture.args());
    assert_eq!(result.code, 0, "{result:?}");
    assert!(result.out[0].contains("all 142 functions"));
    let result = help::verify(&[display(&fixture.src)]);
    assert_eq!(result.code, 0, "{result:?}");
    assert!(result.out[0].starts_with(
        "142 declarations read: 134 signatures, 134 parameter tables and 131 example blocks"
    ));
}
#[test]
fn source_verification_includes_additional_txt_modules() {
    let fixture = Fixture::new();
    std::fs::write(
        fixture.src.join("Payroll.txt"),
        "PayrollTaxλ = LAMBDA(Income, LET(Result, Income, Result));\n",
    )
    .unwrap();
    let result = sources::verify(&fixture.args());
    assert_eq!(result.code, 1);
    assert!(
        result
            .out
            .iter()
            .any(|s| s.contains("oz.PayrollTaxλ is in src/ but does not ship in the workbook"))
    );
}
#[test]
fn source_read_errors_are_refusals_with_no_writer() {
    let fixture = Fixture::new();
    std::fs::write(fixture.src.join("Bad.txt"), [0xff]).unwrap();
    let before = std::fs::read(&fixture.workbook).unwrap();
    let result = sources::verify(&fixture.args());
    assert_eq!(result.code, 1);
    assert!(result.out[0].starts_with("FAIL: cannot read "));
    assert_eq!(std::fs::read(&fixture.workbook).unwrap(), before);
}
#[test]
fn help_parameter_lists_ignore_nested_bodies_and_keep_parameters() {
    for body in [
        "LAMBDA(Cost, Life, Cost/Life)",
        "LAMBDA(Cost, Life, LET(Result, Cost/Life, Result))",
    ] {
        assert_eq!(help::declared(body), ["Cost", "Life"]);
    }
    assert_eq!(help::declared("LAMBDA(x, IF(x>0, LET(a, x, a), 0))"), ["x"]);
    assert!(help::declared("LAMBDA(5)").is_empty());
}
#[test]
fn wrapped_help_rows_and_worked_examples_are_read_in_order() {
    let body = "LAMBDA(x, \"FUNCTION:→Fλ(x,¶→ y)¶PARAMETERS:→¶x→An input¶NOTE!→Aside¶EXAMPLES:→=Fλ(1)¶ →=oz.Fλ(2)¶RESULT:→Otherλ(2)\")";
    assert_eq!(help::rows(body)[0], ("FUNCTION:".into(), "Fλ(x, y)".into()));
    assert_eq!(help::parameter_table(body), Some(vec!["x".into()]));
    assert_eq!(help::worked_examples(body), ["Fλ", "Fλ"]);
    assert_eq!(help::literals("head\"a\"\"b"), "a\"b");
}
#[test]
fn help_floor_refuses_an_empty_or_incomplete_library() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        help::verify(&[display(dir.path())]).err,
        [format!(
            "no source modules found in {}/",
            display(dir.path())
        )]
    );
    std::fs::write(dir.path().join("One.txt"), "Fλ = LAMBDA(x, x);").unwrap();
    let result = help::verify(&[display(dir.path())]);
    assert_eq!(result.code, 1);
    assert!(result.err[0].contains("the parser is not reading them"));
}
#[test]
fn index_gates_reject_duplicates_before_collapsing_rows() {
    let fixture = Fixture::new();
    let mut csv = Csv::read(&fixture.index).unwrap();
    let row = csv
        .rows
        .iter()
        .find(|r| csv.get(r, "function") == Some("oz.AnnualRateλ"))
        .unwrap()
        .clone();
    csv.rows.insert(0, row);
    let text = std::iter::once(&csv.headers)
        .chain(&csv.rows)
        .map(|r| csv_row(r))
        .collect::<String>();
    std::fs::write(&fixture.index, text).unwrap();
    let mut args = fixture.args();
    args.push(display(&fixture.index));
    for result in [
        index::verify(&args),
        index::previous_names(&[display(&fixture.index)]),
    ] {
        assert_eq!(result.code, 1);
        assert!(
            result
                .out
                .iter()
                .any(|s| s.contains("oz.AnnualRateλ appears 2 times"))
        );
    }
}
#[test]
fn repeated_consumed_headers_and_truncated_rows_cannot_pass() {
    let fixture = Fixture::new();
    let csv = Csv::read(&fixture.index).unwrap();
    let mut duplicate = csv.headers.clone();
    duplicate.push("function".into());
    std::fs::write(&fixture.index, csv_row(&duplicate)).unwrap();
    let mut args = fixture.args();
    args.push(display(&fixture.index));
    assert!(index::verify(&args).out[0].contains("duplicate CSV column(s): function"));
    assert!(
        index::previous_names(&[display(&fixture.index)]).err[0]
            .contains("duplicate CSV column(s): function")
    );
    let mut rows = csv.rows.clone();
    rows[0].truncate(1);
    std::fs::write(
        &fixture.index,
        std::iter::once(&csv.headers)
            .chain(&rows)
            .map(|r| csv_row(r))
            .collect::<String>(),
    )
    .unwrap();
    let result = index::previous_names(&[display(&fixture.index)]);
    assert_eq!(result.code, 1);
    assert!(
        result
            .out
            .iter()
            .any(|s| s.contains("publishes an empty signature"))
    );
}
#[test]
fn html_entities_follow_the_python_reader() {
    assert_eq!(
        unescape("&NegativeThinSpace;&caps;&nbsp;"),
        "\u{200b}∩\u{fe00}\u{a0}"
    );
    assert_eq!(
        unescape("&#128; &#0; &NotEqualTilde; &notinX; &unknown;"),
        "€ � ≂̸ ¬inX; &unknown;"
    );
}
#[test]
fn failed_atomic_replacement_preserves_destinations_and_unrelated_temp_files() {
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("book.xlsx");
    std::fs::create_dir(&destination).unwrap();
    let preexisting = dir.path().join("book.xlsx.tmp");
    std::fs::write(&preexisting, b"existing").unwrap();
    assert!(replace_atomically(&destination, b"replacement").is_err());
    assert!(destination.is_dir());
    assert_eq!(std::fs::read(preexisting).unwrap(), b"existing");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
}

#[test]
fn compiler_missing_function_column_preserves_python_error_precedence() {
    let fixture = Fixture::new();
    let original = std::fs::read(&fixture.workbook).unwrap();
    std::fs::write(&fixture.index, "module,signature\nx,y\n").unwrap();
    let mut args = fixture.args();
    args.push(format!("--index={}", display(&fixture.index)));
    let result = compiler::command(&args);
    assert_eq!(result.code, 1);
    assert_eq!(result.err, vec!["FAIL: 'function'"]);
    assert_eq!(std::fs::read(&fixture.workbook).unwrap(), original);
}
