//! Original compiler refusal and reading contracts on complete owned sources.
use ozzit::{common::display, compiler, formula};
use std::collections::{BTreeMap, BTreeSet};

struct Sources {
    _directory: tempfile::TempDir,
    path: std::path::PathBuf,
}

impl Sources {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("src");
        std::fs::create_dir(&path).unwrap();
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("data/compiler-cli.json")).unwrap();
        for module in formula::MODULES {
            let name = format!("{module}.txt");
            std::fs::write(
                path.join(&name),
                fixture["sources"][&name].as_str().unwrap(),
            )
            .unwrap();
        }
        Self {
            _directory: directory,
            path,
        }
    }

    fn dates(&self, text: &str) {
        std::fs::write(self.path.join("Dates.txt"), text).unwrap();
    }
}

fn header(name: &str, description: &str) -> String {
    format!("/*  FUNCTION NAME:  {name}\n    DESCRIPTION:*//**{description}*/\n")
}

fn library() -> BTreeSet<String> {
    ["Otherλ".into()].into_iter().collect()
}

#[test]
fn comments_are_dropped_but_quoted_slashes_and_unclosed_block_tail_are_preserved() {
    assert_eq!(
        formula::strip_comments("a // note\nb /* x */ c \"//kept\" /* open"),
        "a \nb  c \"//kept\" "
    );
}

#[test]
fn a_qualified_library_call_is_kept_exactly() {
    assert_eq!(
        formula::render("LAMBDA([x], oz.Otherλ(x))", &library()).unwrap(),
        "_xlfn.LAMBDA(_xlop.x, oz.Otherλ(_xlpm.x))"
    );
}

#[test]
fn an_unknown_call_is_refused_before_a_stored_definition_is_returned() {
    assert!(
        formula::render("LAMBDA([x], Mystery(x))", &library())
            .unwrap_err()
            .starts_with("Mystery reached the stored form without a marker")
    );
}

#[test]
fn the_first_missing_source_module_is_named_exactly() {
    let directory = tempfile::tempdir().unwrap();
    assert_eq!(
        compiler::compile_sources(directory.path()).unwrap_err(),
        format!(
            "missing source module {}",
            display(&directory.path().join("Dates.txt"))
        )
    );
}

#[test]
fn nondeclarations_are_skipped_without_changing_module_order_or_stored_output() {
    let sources = Sources::new();
    sources
        .dates(&(header("Datesλ", "The Dates sample") + "Datesλ = LAMBDA([x], x + 1);\nBROKEN;\n"));
    let compiled = compiler::compile_sources(&sources.path).unwrap();
    assert_eq!(
        compiled.iter().map(|c| c.name.clone()).collect::<Vec<_>>(),
        formula::MODULES.map(|module| format!("oz.{module}λ"))
    );
    assert_eq!(compiled[0].stored, "_xlfn.LAMBDA(_xlop.x, _xlpm.x + 1)");
}

#[test]
fn a_declaration_without_its_header_is_refused_exactly() {
    let sources = Sources::new();
    sources.dates("Datesλ = LAMBDA([x], x);\n");
    assert_eq!(
        compiler::compile_sources(&sources.path).unwrap_err(),
        "Datesλ: no FUNCTION NAME / DESCRIPTION header in Dates.txt"
    );
}

#[test]
fn a_rendering_failure_names_the_function() {
    let sources = Sources::new();
    sources.dates(&(header("Datesλ", "d") + "Datesλ = LAMBDA([x], Mystery(x));\n"));
    assert!(
        compiler::compile_sources(&sources.path)
            .unwrap_err()
            .starts_with("Datesλ: Mystery reached the stored form")
    );
}

#[test]
fn an_oversized_definition_reports_its_exact_length_and_excel_limit() {
    let sources = Sources::new();
    let literal = "a".repeat(9000);
    sources.dates(&(header("Datesλ", "d") + &format!("Datesλ = LAMBDA([x], \"{literal}\");\n")));
    let stored = format!("_xlfn.LAMBDA(_xlop.x,\"{literal}\")");
    assert_eq!(
        compiler::compile_sources(&sources.path).unwrap_err(),
        format!(
            "Datesλ: the stored definition is {} characters and Excel refuses to open a workbook with a defined name over 8192",
            stored.chars().count()
        )
    );
}

#[test]
fn a_binding_shadowing_a_library_call_reports_the_round_trip_disagreement() {
    let sources = Sources::new();
    sources.dates(
        &(header("Datesλ", "d")
            + "Datesλ = LAMBDA([x], LET(Essentialsλ, LAMBDA(y, y), Essentialsλ(x)));\n"),
    );
    let message = compiler::compile_sources(&sources.path).unwrap_err();
    assert!(ozzit::common::re(
        r"^Datesλ: the rendering does not reproduce its source at character \d+\n  src   : .*OZ\.ESSENTIALSΛ\(X\)"
    ).is_match(&message).unwrap(), "{message}");
    assert!(message.contains("\n  stored: "));
}

#[test]
fn a_shipped_name_missing_from_sources_is_refused_exactly() {
    assert_eq!(
        compiler::apply("<definedName name=\"oz.Goneλ\">1</definedName>", &[]).unwrap_err(),
        "oz.Goneλ ship in the workbook but are not declared in src/"
    );
}

#[test]
fn unexpected_index_columns_are_reported_in_the_original_order() {
    let directory = tempfile::tempdir().unwrap();
    let index = directory.path().join("functions.csv");
    std::fs::write(&index, "function,module\noz.Datesλ,Dates\n").unwrap();
    let before = std::fs::read(&index).unwrap();
    assert_eq!(
        compiler::render_index(&index, "", &BTreeMap::new()).unwrap_err(),
        format!(
            "{} has unexpected columns: ['function', 'module']",
            display(&index)
        )
    );
    assert_eq!(std::fs::read(&index).unwrap(), before);
}
