use super::super::*;
use super::fixtures as copies;
use crate as tooling;
#[path = "../../../../tests/ato_retirement/mod.rs"]
mod fixtures;
use fixtures::MODULES;
use fixtures::*;

fn absent_copy() -> Fixture {
    let f = absent();
    let t = tables(&SPECS[2]);
    assert_eq!(
        t.about.iter().map(|(m, _)| m.as_str()).collect::<Vec<_>>(),
        MODULES
    );
    assert_eq!(t.of("Ratios").len(), 5);
    assert_eq!(t.of("Dates").len(), 1);
    for module in MODULES {
        assert_eq!(
            add_to_src(&SPECS[2], &t, module, &source(&f, module)).unwrap(),
            std::fs::read_to_string(root().join(format!("src/{module}.txt"))).unwrap(),
            "src/{module}.txt does not round-trip through the pass"
        );
    }
    f
}
fn failure(final_workbook: bool) {
    let f = absent_copy();
    let before = f.snapshot();
    let index_before = std::fs::read(&f.index).unwrap();
    let sources_before = MODULES.map(|m| source(&f, m));
    let book_before = std::fs::read(&f.book).unwrap();
    let mut calls = vec![];
    let mut failed = false;
    let message = "synthetic ATO write failure";
    let result = run_with_steps(
        &SPECS[2],
        &f.book,
        &py::PyPath::new(&display(&f.src)),
        Some(&py::PyPath::new(&display(&f.index))),
        &mut vec![],
        &mut |step, path| {
            if step == Step::Source {
                calls.push(path.file_name().unwrap().to_string_lossy().into_owned());
            }
            let inject = if final_workbook {
                step == Step::Workbook
            } else {
                step == Step::Source && calls.len() == 2
            };
            if !failed && inject {
                assert_ne!(source(&f, "Ratios"), sources_before[0]);
                if final_workbook {
                    assert_ne!(source(&f, "Dates"), sources_before[1]);
                    assert_ne!(std::fs::read(&f.index).unwrap(), index_before);
                    assert_eq!(std::fs::read(&f.book).unwrap(), book_before);
                }
                failed = true;
                return Err(message.into());
            }
            Ok(())
        },
    );
    assert_eq!(result.unwrap_err(), message);
    assert_eq!(
        calls,
        ["Ratios.txt", "Dates.txt", "Ratios.txt", "Dates.txt"]
    );
    assert_eq!(f.snapshot(), before);
}

#[test]
fn a_failed_source_write_restores_every_store() {
    failure(false);
}
#[test]
fn a_failed_workbook_write_restores_every_store() {
    failure(true);
}
#[test]
fn a_bare_workbook_and_src_confirm_the_applied_state() {
    let f = Fixture::new();
    std::fs::remove_file(&f.index).unwrap();
    let before = f.snapshot();
    let mut notes = vec![];
    assert!(
        run(
            &SPECS[2],
            &f.book,
            &py::PyPath::new(&display(&f.src)),
            None,
            &mut notes
        )
        .unwrap()
        .is_empty()
    );
    assert!(notes.is_empty());
    assert_eq!(f.snapshot(), before);
    assert!(!f.index.exists());
}
#[test]
fn run_refuses_a_missing_module() {
    let f = Fixture::new();
    std::fs::remove_file(f.src.join("Dates.txt")).unwrap();
    let before = f.snapshot();
    assert_eq!(
        run(
            &SPECS[2],
            &f.book,
            &py::PyPath::new(&display(&f.src)),
            None,
            &mut vec![]
        )
        .unwrap_err(),
        format!("src: missing {}", display(&f.src.join("Dates.txt")))
    );
    assert_eq!(f.snapshot(), before);
}
#[test]
fn add_to_src_needs_exactly_one_about_anchor() {
    let t = tables(&SPECS[2]);
    let anchor = &lookup_about(&t.about, "Dates").anchor;
    for (text, hits) in [(String::new(), 0), (anchor.repeat(2), 2)] {
        assert_eq!(
            add_to_src(&SPECS[2], &t, "Dates", &text).unwrap_err(),
            format!("src/Dates.txt: About anchor found {hits} times, expected 1")
        );
    }
}
#[test]
fn add_to_src_needs_exactly_one_following_block() {
    let t = tables(&SPECS[2]);
    assert_eq!(
        add_to_src(
            &SPECS[2],
            &t,
            "Dates",
            &format!("{} &", lookup_about(&t.about, "Dates").anchor)
        )
        .unwrap_err(),
        "src/Dates.txt: DateDifλ block found 0 times, expected 1"
    );
}
#[test]
fn the_source_guard_accepts_the_shipped_blocks() {
    check_blocks(&tables(&SPECS[2]).functions, true).unwrap();
}
#[test]
fn the_source_guard_refuses_each_malformed_block() {
    for (block, message) in [
        (
            "\nXλ = LAMBDA(a, a &amp; b)",
            "Xλ: &amp; does not belong in the source",
        ),
        (
            "\nXλ = LAMBDA(a, a &lt; b)",
            "Xλ: &lt; does not belong in the source",
        ),
        (
            "\nXλ = LAMBDA(a, a &gt; b)",
            "Xλ: &gt; does not belong in the source",
        ),
        (
            "\nXλ = LAMBDA(a, __ATO_PAGE__)",
            "Xλ: __ATO_PAGE__ does not belong in the source",
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
        let f = Function {
            module: "Ratios".into(),
            name: "Xλ".into(),
            about: "about".into(),
            block: block.into(),
        };
        assert_eq!(check_blocks(&[f], true).unwrap_err(), message);
    }
}
