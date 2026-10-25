use super::*;
use crate as tooling;
#[path = "../../../tests/working_capital_retirement/mod.rs"]
#[allow(dead_code)]
#[allow(clippy::duplicate_mod)] // Private pass suites share the same owned-copy fixture.
mod copies;
#[path = "../../../tests/leases_retirement/mod.rs"]
mod fixtures;
use fixtures::{Fixture, root};

fn synthetic(body: &str) -> Result<Vec<Definition>> {
    build_with(
        &[Function {
            name: "Fixtureλ".into(),
            comment: "Synthetic fixture".into(),
            block: format!("Fixtureλ = {body};"),
            about: String::new(),
        }],
        &["Fixtureλ".into()].into(),
        formula::render,
    )
}
fn dummy(names: &[String]) -> Vec<Definition> {
    names
        .iter()
        .map(|name| Definition {
            name: name.clone(),
            comment: "c".into(),
            stored: "s".into(),
        })
        .collect()
}
fn index(text: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("functions.csv");
    std::fs::write(&path, text).unwrap();
    (directory, path)
}
fn book(names: &[&str]) -> String {
    format!(
        "<workbook><definedNames>{}</definedNames></workbook>",
        names
            .iter()
            .map(|n| format!("<definedName name=\"{n}\">1</definedName>"))
            .collect::<String>()
    )
}
#[test]
fn a_generated_definition_that_drifts_from_its_source_is_refused() {
    let lib = library(&py::PyPath::new(&display(&root().join("src")))).unwrap();
    let error = build_with(&TABLES.functions, &lib, |body, library| {
        Ok(formula::render(body, library)? + " + 1")
    })
    .err()
    .unwrap();
    assert_eq!(
        error,
        "LeaseLiabilityλ: generated definition does not reproduce its source at character 2774\n  src   : EXPONENTS),CHOOSE(HELP?+1,RESULT,HELP)))\n  stored: EXPONENTS),CHOOSE(HELP?+1,RESULT,HELP)))+1"
    );
}
#[test]
fn src_state_reads_absent_and_refuses_a_partial_module() {
    assert_eq!(src_state("Otherλ = LAMBDA(1);").unwrap(), "absent");
    assert_eq!(
        src_state("\nLeaseLiabilityλ = LAMBDA(1);").unwrap_err(),
        "src/Financial.txt holds 1 of 4 lease functions and 0 About heading(s); it is neither state this pass recognises"
    );
}
#[test]
fn add_to_src_appends_after_a_final_newline_either_way() {
    let anchor = &TABLES.anchor;
    let rows = std::iter::once(format!("    \"{}¶\" &", TABLES.heading))
        .chain(
            TABLES
                .functions
                .iter()
                .map(|f| format!("    \"{}¶\" &", about_row(f))),
        )
        .collect::<Vec<_>>();
    assert_eq!(rows[..2],[format!("    \"{}¶\" &",TABLES.heading),"    \"LeaseLiabilityλ    →Present value of the lease payments that are not paid at the commencement date¶\" &".into()]);
    let blocks = TABLES
        .functions
        .iter()
        .map(|f| f.block.as_str())
        .collect::<Vec<_>>()
        .join("\n\n\n");
    let expected = format!("    {anchor} &\n{}\n\n\n\n{blocks}\n", rows.join("\n"));
    for suffix in ["", "\n"] {
        assert_eq!(
            add_to_src(&format!("    {anchor} &{suffix}")).unwrap(),
            expected
        );
    }
    let f = fixtures::absent();
    let original = std::fs::read_to_string(root().join("src/Financial.txt")).unwrap();
    assert_eq!(src_state(&fixtures::source(&f)).unwrap(), "absent");
    assert_eq!(src_state(&original).unwrap(), "applied");
    // Frozen from the original Python pass on this exact reverted fixture;
    // re-frozen from main's Python pass after main changed src/Financial.txt.
    // Later additions follow the shipped lease blocks, so reapply appends them anew.
    use sha2::{Digest, Sha256};
    assert_eq!(
        Sha256::digest(add_to_src(&fixtures::source(&f)).unwrap())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>(),
        "233bf27dedd66e8c28603a8122c8db5730eac1d3b18d0c7c16db402beef85ddf"
    );
}
#[test]
fn add_to_src_refuses_a_module_without_the_about_anchor() {
    assert_eq!(
        add_to_src("no anchor here\n").unwrap_err(),
        "src/Financial.txt: About anchor found 0 times, expected 1"
    );
}
#[test]
fn add_to_workbook_refuses_a_missing_anchor() {
    assert_eq!(
        add_to_workbook(&escape(&TABLES.anchor).repeat(2), &[]).unwrap_err(),
        "xl/workbook.xml: About anchor found 2 times, expected 1"
    );
}
#[test]
fn add_to_workbook_refuses_a_name_with_nothing_before_it() {
    assert_eq!(
        add_to_workbook(
            &escape(&TABLES.anchor),
            &dummy(&["oz.LeaseLiabilityλ".into()])
        )
        .unwrap_err(),
        "oz.LeaseLiabilityλ sorts before every shipped name, which is not expected"
    );
}
#[test]
fn update_index_refuses_unexpected_columns() {
    let (_directory, path) = index("function,module\noz.Aλ,Dates\n");
    assert_eq!(
        update_index(&path, &[]).unwrap_err(),
        format!(
            "{} has unexpected columns: ['function', 'module']",
            display(&path)
        )
    );
}
#[test]
fn update_index_leaves_a_complete_index_alone() {
    let f = Fixture::new();
    let before = std::fs::read(&f.index).unwrap();
    assert!(!update_index(&f.index, &dummy(&qualified())).unwrap());
    assert_eq!(std::fs::read(&f.index).unwrap(), before);
}
#[test]
fn update_index_refuses_a_partial_index() {
    let (_directory, path) = index(
        "function,module,previous_name,signature,description\noz.LeaseLiabilityλ,Financial,,LeaseLiabilityλ(),x\n",
    );
    assert_eq!(
        update_index(&path, &dummy(&qualified())).unwrap_err(),
        format!(
            "{} holds some of the lease functions but not all",
            display(&path)
        )
    );
}
#[test]
fn update_index_refuses_a_definition_without_a_function_line() {
    let (_directory, path) = index("function,module,previous_name,signature,description\n");
    assert_eq!(
        update_index(
            &path,
            &[Definition {
                name: "oz.Xλ".into(),
                comment: "comment".into(),
                stored: "no help".into()
            }]
        )
        .unwrap_err(),
        "oz.Xλ: stored definition has no FUNCTION line"
    );
}
#[test]
fn inline_nested_bindings_get_their_excel_markers() {
    assert_eq!(
        synthetic("LAMBDA([Values], LET(Result, MAP(Values, LAMBDA(v, v * 2)), Result))")
            .ok()
            .unwrap()[0]
            .stored,
        "_xlfn.LAMBDA(_xlop.Values, _xlfn.LET(_xlpm.Result, _xlfn.MAP(_xlpm.Values, _xlfn.LAMBDA(_xlpm.v, _xlpm.v * 2)), _xlpm.Result))"
    );
}
#[test]
fn comments_and_escaped_quotes_preserve_literal_text() {
    assert_eq!(
        synthetic("LAMBDA([Lease], /* comment */ Lease & \"a\"\"b//c\")")
            .ok()
            .unwrap()[0]
            .stored,
        "_xlfn.LAMBDA(_xlop.Lease, _xlpm.Lease & \"a\"\"b//c\")"
    );
}
#[test]
fn unknown_names_stop_definition_generation() {
    assert!(
        synthetic("LAMBDA([Lease], Lease + Undeclared)")
            .err()
            .unwrap()
            .contains("Undeclared")
    );
}
#[test]
fn lease_builder_preserves_shipped_rate_guidance_and_remeasurement() {
    let book = read_book(&read_parts(&root().join("ozzit.xlsx")).unwrap()).unwrap();
    let lib = library(&py::PyPath::new(&display(&root().join("src")))).unwrap();
    let rebuilt = build_definitions(&lib).ok().unwrap();
    for name in ["oz.LeaseLiabilityλ", "oz.LeaseRemeasureλ"] {
        let stored = captures(
            &format!(r#"(?s)<definedName name="{name}" comment="[^"]*">(.*?)<"#),
            &book,
        );
        assert_eq!(stored.len(), 1);
        assert_eq!(
            formula::canonical(&rebuilt.iter().find(|d| d.name == name).unwrap().stored),
            formula::canonical(&unescape(&stored[0][1]).unwrap())
        );
    }
}
#[test]
fn lease_pass_restores_source_if_workbook_write_fails() {
    let f = Fixture::new();
    let path = f.src.join("Financial.txt");
    std::fs::write(&path, b"Fixture = 1;\r\n").unwrap();
    let before = f.snapshot();
    let original = read_text(&path).unwrap();
    assert_eq!(original.as_bytes(), b"Fixture = 1;\r\n");
    let error = write_with_recovery(&path, &original, "changed", || {
        assert_eq!(std::fs::read(&path).unwrap(), b"changed");
        assert_eq!(
            std::fs::read(&f.book).unwrap(),
            std::fs::read(root().join("ozzit.xlsx")).unwrap()
        );
        assert_eq!(
            std::fs::read(&f.index).unwrap(),
            std::fs::read(root().join("functions.csv")).unwrap()
        );
        Err("synthetic failure".into())
    })
    .unwrap_err();
    assert_eq!(error, "synthetic failure");
    assert_eq!(f.snapshot(), before);
}
#[test]
fn none_of_the_names_is_absent() {
    assert_eq!(
        workbook_state(
            &book(&["oz.Other"]),
            &["oz.A".into(), "oz.B".into()],
            None,
            "defined names"
        )
        .unwrap(),
        "absent"
    );
}
#[test]
fn all_of_the_names_is_applied() {
    assert_eq!(
        workbook_state(
            &book(&["oz.A", "oz.B"]),
            &["oz.A".into(), "oz.B".into()],
            None,
            "defined names"
        )
        .unwrap(),
        "applied"
    );
}
#[test]
fn some_of_the_names_is_neither_state() {
    assert!(
        workbook_state(
            &book(&["oz.A"]),
            &["oz.A".into(), "oz.B".into()],
            None,
            "defined names"
        )
        .unwrap_err()
        .contains("1 of 2 defined names")
    );
}
#[test]
fn the_about_heading_is_counted_with_the_names() {
    let names = ["oz.A".into()];
    assert_eq!(
        workbook_state(
            &(book(&["oz.A"]) + "Leases"),
            &names,
            Some("Leases"),
            "defined names"
        )
        .unwrap(),
        "applied"
    );
    assert!(
        workbook_state(&book(&["oz.A"]), &names, Some("Leases"), "defined names")
            .unwrap_err()
            .contains("About heading")
    );
}
#[test]
fn only_the_prefixed_names_are_returned_in_document_order() {
    assert_eq!(
        oz_names(&book(&["oz.B", "Print_Area", "oz.A"])),
        ["oz.B", "oz.A"]
    );
}
#[test]
fn pairs_are_applied_in_the_order_given() {
    assert_eq!(
        apply_swaps("ab", &[("a".into(), "b".into()), ("bb".into(), "c".into())]),
        "c"
    );
}
#[test]
fn no_pairs_leaves_the_text_alone() {
    assert_eq!(apply_swaps("ab", &[]), "ab");
}
#[test]
fn applied_stores_accept_direct_none_and_index_without_writes() {
    let f = Fixture::new();
    let before = f.snapshot();
    let src = py::PyPath::new(&display(&f.src));
    let index = py::PyPath::new(&display(&f.index));
    for index in [Some(&index), None] {
        let mut notes = vec![];
        assert!(
            run_optional(&f.book, &src, index, &mut notes)
                .unwrap()
                .is_empty()
        );
        assert!(notes.is_empty());
        assert_eq!(f.snapshot(), before);
    }
}
#[test]
fn original_lease_source_guards_run_before_rendering() {
    assert!(
        build_definitions(&library(&py::PyPath::new(&display(&root().join("src")))).unwrap())
            .is_ok()
    );
    for (block, message) in [
        (
            "Fixtureλ = LAMBDA(\"&amp;\");",
            "&amp; is already escaped in the source",
        ),
        (
            "Fixtureλ = LAMBDA(\"&lt;\");",
            "&lt; is already escaped in the source",
        ),
        (
            "Fixtureλ = LAMBDA(\"&gt;\");",
            "&gt; is already escaped in the source",
        ),
        ("Fixtureλ =\tLAMBDA(1);", "tabs do not belong in the source"),
        ("Otherλ = LAMBDA(1);", "source block does not declare it"),
        (";", "source block does not declare it"),
    ] {
        let error = build_with(
            &[Function {
                name: "Fixtureλ".into(),
                comment: String::new(),
                block: block.into(),
                about: String::new(),
            }],
            &BTreeSet::new(),
            |_, _| panic!("must refuse before rendering"),
        )
        .err()
        .unwrap();
        assert_eq!(error, format!("Fixtureλ: {message}"));
    }
}
