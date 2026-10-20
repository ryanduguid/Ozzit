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
    let before_names = compiler::shipped_names(&archive::read_book(&before).unwrap()).unwrap();
    let after_names = compiler::shipped_names(&archive::read_book(&after).unwrap()).unwrap();
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
    assert!(result.out[0].contains("all 153 functions"));
    let result = help::verify(&[display(&fixture.src)]);
    assert_eq!(result.code, 0, "{result:?}");
    assert!(result.out[0].starts_with(
        "153 declarations read: 145 signatures, 145 parameter tables and 142 example blocks"
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
fn csv_reads_and_writes_as_dictreader_and_dictwriter_do() {
    let strings = |rows: &[Vec<String>]| rows.to_vec();
    // DictReader takes a blank first record as an empty header, then skips blank records.
    for text in ["\nfunction,module\nA,B\n", "\r\nfunction,module\r\nA,B\r\n"] {
        let csv = Csv::parse(text).unwrap();
        assert!(csv.headers.is_empty());
        assert_eq!(csv.rows.len(), 2);
        assert_eq!(csv.get(&csv.rows[1], "function"), None);
    }
    let csv = Csv::parse("function,module\n\nA,B\n\n").unwrap();
    assert_eq!(csv.headers, ["function", "module"]);
    assert_eq!(strings(&csv.rows), [vec!["A".to_string(), "B".into()]]);
    let csv = Csv::parse("\"\"\nA\n").unwrap();
    assert_eq!(csv.headers, [""]);
    assert!(Csv::parse("").unwrap().headers.is_empty());
    // A carriage return is quoted, as the csv writer does.
    assert_eq!(
        csv_row(&["tax\rnote".into(), "x".into()]),
        "\"tax\rnote\",x\n"
    );
}
#[test]
fn html_entities_follow_the_python_reader() {
    assert_eq!(
        unescape("&NegativeThinSpace;&caps;&nbsp;").unwrap(),
        "\u{200b}∩\u{fe00}\u{a0}"
    );
    assert_eq!(
        unescape("&#128; &#0; &NotEqualTilde; &notinX; &unknown;").unwrap(),
        "€ � ≂̸ ¬inX; &unknown;"
    );
    // int() reads a decimal reference, leading zeros included, so more than
    // 4,300 digits is refused; hexadecimal has no such limit (Python 3.15).
    assert_eq!(
        unescape(&format!("&#{}49;", "0".repeat(4301))).unwrap_err(),
        "Exceeds the limit (4300 digits) for integer string conversion: value has 4303 digits; \
         use sys.set_int_max_str_digits() to increase the limit"
    );
    assert_eq!(
        unescape(&format!("&#{}49;", "0".repeat(4298))).unwrap(),
        "1"
    );
    assert_eq!(
        unescape(&format!("&#x{}31;", "0".repeat(5000))).unwrap(),
        "1"
    );
    assert_eq!(unescape("&#99999999999;").unwrap(), "\u{fffd}");
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

#[test]
fn csv_decoding_removes_exactly_one_signature_and_keeps_later_boms() {
    let fixture = Fixture::new();
    let original = std::fs::read(&fixture.index).unwrap();
    for count in 0..=3 {
        let mut bytes = b"\xef\xbb\xbf".repeat(count);
        bytes.extend_from_slice(&original);
        std::fs::write(&fixture.index, bytes).unwrap();
        let csv = Csv::read(&fixture.index).unwrap();
        assert_eq!(
            csv.headers[0],
            format!("{}function", "\u{feff}".repeat(count.saturating_sub(1)))
        );
    }
}

#[test]
fn a_large_part_is_searched_without_a_backtrack_limit() {
    // Python's `re` has no limit. The engine counts each skipped start position, so the
    // sanitiser's pageSetup search panicked after 10 MB of sheet data.
    let text = format!(
        r#"{}<pageSetup scale="90" r:id="rId1"/>"#,
        "x".repeat(11_000_000)
    );
    let new = sub_fn(
        &bounded(r#"(<pageSetup\b[^>]*?)\s+r:id="[^"]*""#),
        &text,
        |m| m[1].to_string(),
    );
    assert!(new.ends_with(r#"<pageSetup scale="90"/>"#));
}

#[test]
fn a_block_comment_ends_after_its_opening_delimiter() {
    // Python searches for "*/" from after "/*", so "/*/" does not close a comment.
    assert_eq!(formula::strip_comments("LAMBDA(1/*/+2/**/)"), "LAMBDA(1)");
    assert_eq!(render("LAMBDA(1/*/+2/**/)"), "_xlfn.LAMBDA(1)");
}

#[test]
fn bracket_trimming_matches_python_for_let_and_optional_parameters() {
    // LET bindings keep their brackets; an optional parameter loses only its outer pair.
    assert!(formula::local_names("LAMBDA(LET([Bound],1,Bound))").is_empty());
    let refused =
        "Bound reached the stored form without a marker; Excel will not read the definition";
    for body in ["LAMBDA(LET([Bound],1,Bound))", "LAMBDA([Bound,Bound)"] {
        assert_eq!(
            formula::render(body, &BTreeSet::new()).unwrap_err(),
            refused
        );
    }
    assert_eq!(
        formula::render("LAMBDA([[Bound]],Bound)", &BTreeSet::new()).unwrap(),
        "_xlfn.LAMBDA(_xlop.[_xlpm.Bound],_xlpm.Bound)"
    );
}

#[test]
fn signature_discovery_skips_hidden_modules_as_glob_does() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".hidden.txt"),
        "Fλ=LAMBDA(X,\"FUNCTION:→Fλ(X)¶PARAMETERS:→¶X→input¶\");\n",
    )
    .unwrap();
    let src = display(dir.path());
    let result = help::verify(std::slice::from_ref(&src));
    assert_eq!(result.code, 1);
    assert_eq!(result.err, [format!("no source modules found in {src}/")]);
}

#[test]
fn a_local_header_naming_another_member_is_refused() {
    let parts: archive::Parts = [("xl/workbook.xml".to_string(), b"<workbook/>".to_vec())]
        .into_iter()
        .collect();
    let mut bytes = archive::deterministic_bytes(&parts).unwrap();
    // The first local header's name starts at byte 30; keep its length.
    bytes[30..45].copy_from_slice(b"xl/workbool.xml");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("book.xlsx");
    std::fs::write(&path, bytes).unwrap();
    assert_eq!(
        archive::read_parts(&path).unwrap_err(),
        "File name in directory 'xl/workbook.xml' and header b'xl/workbool.xml' differ."
    );
}

fn one_member_archive(name: &str) -> Vec<u8> {
    let parts: archive::Parts = [(name.to_string(), b"<probe/>".to_vec())]
        .into_iter()
        .collect();
    archive::deterministic_bytes(&parts).unwrap()
}

#[test]
fn a_retained_reader_checks_the_bytes_it_opened() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("book.xlsx");
    let good = one_member_archive("xl/workbook.xml");
    let mut bad = good.clone();
    bad[30..45].copy_from_slice(b"xl/workbool.xml");
    // A malformed archive stays malformed after its path is replaced with a good one.
    std::fs::write(&path, &bad).unwrap();
    let mut zip = archive::open_zip(&path).unwrap();
    std::fs::write(&path, &good).unwrap();
    assert!(archive::member_text(&mut zip, "xl/workbook.xml").is_err());
    // A good archive stays readable after its path is removed, as an open file does.
    let mut zip = archive::open_zip(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    assert_eq!(
        archive::member_text(&mut zip, "xl/workbook.xml").unwrap(),
        "<probe/>"
    );
}

#[test]
fn member_names_decode_by_their_own_utf8_flags_as_zipfile_does() {
    assert_eq!(ozzit::py::decode_cp437(b"a\x82\xc3\xa9\xff"), "aé├⌐\u{a0}");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("book.xlsx");
    // Both headers carry the UTF-8 bytes of "é.xml" and the UTF-8 flag (bit 11).
    let flagged = one_member_archive("é.xml");
    let central = flagged.windows(4).position(|w| w == b"PK\x01\x02").unwrap();
    let clear = |bytes: &mut Vec<u8>, at: usize| bytes[at + 1] &= !0x08;
    // Local flag cleared: the local name decodes as cp437 and no longer matches.
    let mut local_only = flagged.clone();
    clear(&mut local_only, 6);
    std::fs::write(&path, &local_only).unwrap();
    assert_eq!(
        archive::read_parts(&path).unwrap_err(),
        r"File name in directory 'é.xml' and header b'\xc3\xa9.xml' differ."
    );
    // Both flags cleared: both names decode as cp437 and match.
    let mut neither = local_only;
    clear(&mut neither, central + 8);
    std::fs::write(&path, &neither).unwrap();
    let parts = archive::read_parts(&path).unwrap();
    assert_eq!(parts.keys().collect::<Vec<_>>(), ["├⌐.xml"]);
}
