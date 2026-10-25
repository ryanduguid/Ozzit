use super::super::*;
use super::fixtures as copies;
use crate as tooling;
#[path = "../../../../tests/rate_date_retirement/mod.rs"]
mod fixtures;
use fixtures::MODULES;
use fixtures::*;

fn absent_copy() -> Fixture {
    let f = absent();
    let t = tables(&SPECS[1]);
    assert_eq!(
        t.about.iter().map(|(m, _)| m.as_str()).collect::<Vec<_>>(),
        MODULES
    );
    assert_eq!(t.of("Financial").len(), 3);
    assert_eq!(t.of("Dates").len(), 1);
    for module in MODULES {
        assert_eq!(
            add_to_src(&SPECS[1], &t, module, &source(&f, module)).unwrap(),
            std::fs::read_to_string(root().join(format!("src/{module}.txt"))).unwrap(),
            "src/{module}.txt does not round-trip through the pass"
        );
    }
    f
}

fn failure(second_source: bool, with_index: bool) -> Vec<String> {
    let f = absent_copy();
    let before = f.snapshot();
    let original_financial = source(&f, "Financial");
    let index = if with_index {
        f.index.clone()
    } else {
        f.directory.path().join("absent.csv")
    };
    let mut calls = vec![];
    let mut failed = false;
    let message = if second_source {
        "synthetic failure on the second source write"
    } else {
        "synthetic compilation failure"
    };
    let result = run_with_steps(
        &SPECS[1],
        &f.book,
        &py::PyPath::new(&display(&f.src)),
        Some(&py::PyPath::new(&display(&index))),
        &mut vec![],
        &mut |step, path| {
            if step == Step::Source {
                calls.push(path.file_name().unwrap().to_string_lossy().into_owned());
            }
            let inject = if second_source {
                step == Step::Source && calls.len() == 2
            } else {
                step == Step::Compile
            };
            if !failed && inject {
                assert_ne!(source(&f, "Financial"), original_financial);
                failed = true;
                return Err(message.into());
            }
            Ok(())
        },
    );
    assert_eq!(result.unwrap_err(), message);
    assert_eq!(f.snapshot(), before);
    if !with_index {
        assert!(!index.exists());
    }
    calls
}

#[test]
fn a_failed_source_write_leaves_no_module_applied() {
    assert_eq!(
        failure(true, true),
        ["Financial.txt", "Dates.txt", "Financial.txt", "Dates.txt"]
    );
}
#[test]
fn a_later_failure_still_restores_every_store() {
    assert_eq!(
        failure(false, true),
        ["Financial.txt", "Dates.txt", "Financial.txt", "Dates.txt"]
    );
}
#[test]
fn a_failure_without_an_index_restores_the_sources() {
    assert_eq!(
        failure(false, false),
        ["Financial.txt", "Dates.txt", "Financial.txt", "Dates.txt"]
    );
}
#[test]
fn the_import_guard_refuses_malformed_source_blocks() {
    check_source_blocks(&tables(&SPECS[1]).functions).unwrap();
    let good = "/* header */\nFλ = LAMBDA(x, x)";
    let function = |block: &str| Function {
        module: "Financial".into(),
        name: "Fλ".into(),
        about: "about".into(),
        block: block.into(),
    };
    check_source_blocks(&[function(good)]).unwrap();
    for (block, message) in [
        (
            format!("{good} &amp; more"),
            "Fλ: &amp; is already escaped in the source",
        ),
        (format!("{good}\t"), "Fλ: tabs do not belong in the source"),
        (
            "/* header */\nGλ = LAMBDA(x, x)".into(),
            "Fλ: the source block does not declare it",
        ),
    ] {
        assert_eq!(
            check_source_blocks(&[function(&block)]).unwrap_err(),
            message
        );
    }
}
#[test]
fn a_module_holding_part_of_the_change_is_refused() {
    let t = tables(&SPECS[1]);
    let functions = t.of("Financial");
    let rows = functions
        .iter()
        .map(|f| (f.name.clone(), t.about_row("Financial", f)))
        .collect::<Vec<_>>();
    assert_eq!(
        source_state(
            "src/Financial.txt",
            &format!("\n{} = LAMBDA(", functions[0].name),
            &rows
        )
        .unwrap_err(),
        "src/Financial.txt holds 1 of 3 functions and 0 About rows; it is neither state this pass recognises"
    );
}
#[test]
fn add_to_src_needs_exactly_one_anchor_and_ends_the_module_with_a_newline() {
    let t = tables(&SPECS[1]);
    assert_eq!(
        add_to_src(&SPECS[1], &t, "Dates", "no anchor here\n").unwrap_err(),
        "src/Dates.txt: About anchor found 0 times, expected 1"
    );
    let anchor = &lookup_about(&t.about, "Dates").anchor;
    let text = format!("Help, TEXTSPLIT(\n    {anchor} &\n    \"→\"");
    let after = add_to_src(&SPECS[1], &t, "Dates", &text).unwrap();
    assert_eq!(
        after,
        add_to_src(&SPECS[1], &t, "Dates", &format!("{text}\n")).unwrap()
    );
    assert!(after.ends_with('\n'));
}
#[test]
fn run_refuses_a_missing_source_module() {
    let f = Fixture::new();
    std::fs::remove_file(f.src.join("Financial.txt")).unwrap();
    assert_eq!(
        run(
            &SPECS[1],
            &f.directory.path().join("missing.xlsx"),
            &py::PyPath::new(&display(&f.src)),
            None,
            &mut vec![]
        )
        .unwrap_err(),
        format!("src: missing {}", display(&f.src.join("Financial.txt")))
    );
}
#[test]
fn a_missing_or_omitted_index_is_noted_and_left_unwritten() {
    for supplied in [true, false] {
        let f = absent_copy();
        let existing_index = std::fs::read(&f.index).unwrap();
        let missing = f.directory.path().join("absent.csv");
        let index = supplied.then(|| py::PyPath::new(&display(&missing)));
        let mut notes = vec![];
        assert_eq!(
            run(
                &SPECS[1],
                &f.book,
                &py::PyPath::new(&display(&f.src)),
                index.as_ref(),
                &mut notes
            )
            .unwrap(),
            ["src/Financial.txt", "src/Dates.txt", "workbook"]
        );
        assert_eq!(
            notes,
            if supplied {
                vec![format!(
                    "note: {} not found, index not updated",
                    display(&missing)
                )]
            } else {
                vec![]
            }
        );
        assert_applied(&f);
        assert!(!missing.exists());
        assert_eq!(std::fs::read(&f.index).unwrap(), existing_index);
    }
}
#[test]
fn unapply_source_leaves_an_absent_module_unchanged() {
    let text = format!(
        "{} &\n",
        lookup_about(&tables(&SPECS[1]).about, "Financial").anchor
    );
    assert_eq!(unapply_source("Financial", &text), text);
}
