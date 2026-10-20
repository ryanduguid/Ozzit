use super::*;
use crate as tooling;
#[path = "../../../tests/working_capital_retirement/mod.rs"]
mod fixtures;
use fixtures::*;

mod ato;

fn absent() -> Fixture {
    let f = Fixture::absent();
    assert_eq!(
        add_to_src(&SPECS[0], &tables(&SPECS[0]), "Ratios", &f.source()).unwrap(),
        std::fs::read_to_string(root().join("src/Ratios.txt")).unwrap(),
        "Ratios.txt does not round-trip through the pass"
    );
    f
}

fn failing(stage: Step, message: &str) -> Vec<Step> {
    let f = absent();
    let before = f.snapshot();
    let source_before = std::fs::read(f.src.join("Ratios.txt")).unwrap();
    let index_before = std::fs::read(&f.index).unwrap();
    let mut calls = Vec::new();
    let mut failed = false;
    let result = run_with_steps(
        &SPECS[0],
        &f.book,
        &py::PyPath::new(&display(&f.src)),
        Some(&py::PyPath::new(&display(&f.index))),
        &mut vec![],
        &mut |step, path| {
            calls.push(step);
            if step == Step::Source {
                assert_eq!(path.file_name().unwrap(), "Ratios.txt");
            }
            if !failed && step == Step::Compile {
                assert_ne!(
                    std::fs::read(f.src.join("Ratios.txt")).unwrap(),
                    source_before
                );
                assert_eq!(std::fs::read(&f.index).unwrap(), index_before);
            }
            if !failed && step == Step::Workbook {
                assert_ne!(
                    std::fs::read(f.src.join("Ratios.txt")).unwrap(),
                    source_before
                );
                assert_ne!(std::fs::read(&f.index).unwrap(), index_before);
                assert_eq!(
                    std::fs::read(&f.book).unwrap(),
                    before.iter().find(|(p, _)| p == &f.book).unwrap().1
                );
            }
            if !failed && step == stage {
                failed = true;
                return Err(message.into());
            }
            Ok(())
        },
    );
    assert_eq!(result.unwrap_err(), message);
    assert_eq!(f.snapshot(), before);
    calls
}

mod rate_date;

#[test]
fn a_failed_source_write_restores_every_store() {
    assert_eq!(
        failing(Step::Source, "synthetic failure on the source write"),
        [Step::Source, Step::Source]
    );
}
#[test]
fn a_failed_compile_restores_every_store() {
    assert_eq!(
        failing(Step::Compile, "synthetic compilation failure"),
        [Step::Source, Step::Compile, Step::Source]
    );
}
#[test]
fn a_failed_workbook_write_restores_every_store() {
    assert_eq!(
        failing(Step::Workbook, "synthetic workbook failure"),
        [Step::Source, Step::Compile, Step::Workbook, Step::Source]
    );
}
#[test]
fn the_source_guard_accepts_the_shipped_blocks() {
    check_source_blocks(&tables(&SPECS[0]).functions).unwrap();
}
#[test]
fn the_source_guard_refuses_each_malformed_block() {
    for (block, message) in [
        (
            "\nXλ = LAMBDA(a, a &amp; b)",
            "Xλ: &amp; is already escaped in the source",
        ),
        (
            "\nXλ = LAMBDA(a, a &lt; b)",
            "Xλ: &lt; is already escaped in the source",
        ),
        (
            "\nXλ = LAMBDA(a, a &gt; b)",
            "Xλ: &gt; is already escaped in the source",
        ),
        (
            "\nXλ = LAMBDA(a,\tb)",
            "Xλ: tabs do not belong in the source",
        ),
        (
            "\nYλ = LAMBDA(a, a)",
            "Xλ: the source block does not declare it",
        ),
    ] {
        let functions = [Function {
            module: "Ratios".into(),
            name: "Xλ".into(),
            about: "about".into(),
            block: block.into(),
        }];
        assert_eq!(check_source_blocks(&functions).unwrap_err(), message);
    }
}
#[test]
fn add_to_src_needs_exactly_one_about_anchor() {
    let t = tables(&SPECS[0]);
    for (text, hits) in [(String::new(), 0), (anchor().repeat(2), 2)] {
        assert_eq!(
            add_to_src(&SPECS[0], &t, "Ratios", &text).unwrap_err(),
            format!("src/Ratios.txt: About anchor found {hits} times, expected 1")
        );
    }
}
#[test]
fn unapply_source_leaves_an_absent_module_unchanged() {
    let text = format!("{} &\n", anchor());
    assert_eq!(unapply_source(&text), text);
}
#[test]
fn add_to_src_preserves_both_module_ending_states() {
    let t = tables(&SPECS[0]);
    for ending in ["", "\n"] {
        let text = format!("{} &\nbase{ending}", anchor());
        let after = add_to_src(&SPECS[0], &t, "Ratios", &text).unwrap();
        assert_eq!(after.ends_with('\n'), text.ends_with('\n'));
        assert!(after.contains(&format!("{} &\n", t.about_row("Ratios", &t.functions[0]))));
        assert!(after.ends_with(&format!("{}{ending}", t.functions[3].block)));
    }
}
