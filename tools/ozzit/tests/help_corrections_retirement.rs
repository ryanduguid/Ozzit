//! Apply, refusal, CLI, temporary siblings and documentation-order contracts.
#[path = "help_corrections_retirement/mod.rs"]
mod fixtures;
use fixtures::*;
use ozzit::{
    archive::{BOOK, read_parts},
    common::display,
    formula::MODULES,
    postbuild::help_corrections,
    py::PyPath,
};

#[test]
fn the_command_is_a_byte_noop_on_every_current_store() {
    let f = Fixture::new();
    let before = f.snapshot();
    for _ in 0..2 {
        assert_eq!(
            f.success(),
            format!(
                "help corrections already applied; no changes\nOK: {}\n",
                display(&f.book)
            )
        );
        assert_eq!(f.snapshot(), before);
    }
}

#[test]
fn all_static_literal_strings_are_corrected_without_formula_refreshes() {
    let f = Fixture::new();
    let expected = read_parts(&f.book).unwrap();
    let mut parts = expected.clone();
    let mut strings = text(&parts, "xl/sharedStrings.xml");
    for [what, old, new] in triples("string_swaps") {
        assert_eq!(strings.matches(&new).count(), 1, "{what}");
        assert!(!strings.contains(&old), "{what}");
        strings = strings.replace(&new, &old);
    }
    set(&mut parts, "xl/sharedStrings.xml", strings);
    f.write(&parts);
    let sources = f.library();
    assert!(
        f.success()
            .starts_with("applied help corrections to: workbook\n")
    );
    let after = read_parts(&f.book).unwrap();
    let strings = text(&after, "xl/sharedStrings.xml");
    for [_, old, new] in triples("string_swaps") {
        assert_eq!(strings.matches(&new).count(), 1);
        assert!(!strings.contains(&old));
    }
    assert_eq!(after, expected);
    assert_eq!(f.library(), sources);
}

#[test]
fn an_unrecognised_static_literal_is_refused() {
    let f = Fixture::new();
    let mut parts = read_parts(&f.book).unwrap();
    let [what, _, new] = &triples("string_swaps")[0];
    let strings = text(&parts, "xl/sharedStrings.xml");
    assert!(strings.contains(new));
    set(
        &mut parts,
        "xl/sharedStrings.xml",
        strings.replace(new, "<si><t>Removedλ</t></si>"),
    );
    f.write(&parts);
    f.refused_without_writing(&format!(
        "sharedStrings: {what} expected 1 recognised anchor, got 0 (before=0, after=0)"
    ));
}

#[test]
fn every_swap_repairs_both_reverted_stores_with_exact_hit_counts() {
    let swaps = swaps();
    assert_eq!(swaps.len(), 10);
    for s in swaps {
        let f = Fixture::new();
        let expected = read_parts(&f.book).unwrap();
        f.revert(&s);
        assert!(
            f.success()
                .starts_with("applied help corrections to: workbook, src/")
        );
        let after = read_parts(&f.book).unwrap();
        let book = text(&after, BOOK);
        assert_eq!(book.matches(&s.book[1]).count(), s.hits, "{}", s.what);
        assert!(!book.contains(&s.book[0]), "{}", s.what);
        let library = f.library();
        assert_eq!(library.matches(&s.source[1]).count(), s.hits, "{}", s.what);
        assert!(!library.contains(&s.source[0]), "{}", s.what);
        assert_eq!(after, expected);
        for module in MODULES {
            assert_eq!(
                std::fs::read(f.src.join(format!("{module}.txt"))).unwrap(),
                std::fs::read(root().join(format!("src/{module}.txt"))).unwrap()
            );
        }
        assert!(
            !paths(f.directory.path())
                .iter()
                .any(|p| p.extension().is_some_and(|e| e == "tmp"))
        );
    }
}

#[test]
fn all_demonstration_examples_are_corrected() {
    let f = Fixture::new();
    let expected = read_parts(&f.book).unwrap();
    let mut parts = expected.clone();
    for [what, old, new] in triples("cell_swaps") {
        let mut hits = 0;
        for (name, bytes) in &mut parts {
            if name.starts_with("xl/worksheets/") {
                let sheet = std::str::from_utf8(bytes).unwrap();
                hits += sheet.matches(&new).count();
                *bytes = sheet.replace(&new, &old).into_bytes();
            }
        }
        assert_eq!(hits, 1, "{what}");
    }
    f.write(&parts);
    let sources = f.library();
    f.success();
    let after = read_parts(&f.book).unwrap();
    let sheets = after
        .iter()
        .filter(|(n, _)| n.starts_with("xl/worksheets/"))
        .map(|(_, v)| std::str::from_utf8(v).unwrap())
        .collect::<String>();
    for [_, old, new] in triples("cell_swaps") {
        assert!(sheets.contains(&new));
        assert!(!sheets.contains(&old));
    }
    assert_eq!(after, expected);
    assert_eq!(f.library(), sources);
}

#[test]
fn duplicate_source_anchors_are_refused() {
    let f = Fixture::new();
    let path = f.src.join("Financial.txt");
    let text = std::fs::read_to_string(&path).unwrap();
    let example = "\"0,-100,-110    →=oz.Reversalλ( , {100,110,130})\"";
    assert!(text.contains(example));
    std::fs::write(
        path,
        text.replace(example, &format!("{example} & {example}")),
    )
    .unwrap();
    f.refused_without_writing(&format!(
        "src: {} expected 1 recognised anchor(s), got 2 (before=0, after=2)",
        swaps()[2].what
    ));
}

#[test]
fn an_unrecognised_source_anchor_is_refused() {
    let f = Fixture::new();
    let path = f.src.join("Ratios.txt");
    let text = std::fs::read_to_string(&path).unwrap();
    let row = "\"DSIλ                       →Days Sales in Inventory Ratio";
    assert!(text.contains(row));
    std::fs::write(
        path,
        text.replace(row, "\"Removedλ                   →Days"),
    )
    .unwrap();
    f.refused_without_writing(&format!(
        "src: {} expected 1 recognised anchor(s), got 0 (before=0, after=0)",
        swaps()[4].what
    ));
}

#[test]
fn unrelated_temporary_siblings_and_all_paths_survive() {
    let f = Fixture::new();
    f.revert(&swaps()[0]);
    let destinations =
        std::iter::once(f.book.clone()).chain(MODULES.map(|m| f.src.join(format!("{m}.txt"))));
    let mut sentinels = vec![];
    for destination in destinations {
        for suffix in [".tmp", ".tmp.tmp"] {
            let path = destination.with_file_name(format!(
                "{}{suffix}",
                destination.file_name().unwrap().to_str().unwrap()
            ));
            std::fs::write(&path, b"unrelated work").unwrap();
            sentinels.push(path);
        }
    }
    let before = paths(f.directory.path());
    let changed = help_corrections::run(&f.book, &PyPath::new(&display(&f.src))).unwrap();
    assert!(changed.iter().any(|p| p == "workbook"));
    assert!(changed.iter().any(|p| p.starts_with("src/")));
    for path in sentinels {
        assert!(path.is_file());
        assert_eq!(std::fs::read(path).unwrap(), b"unrelated work");
    }
    assert_eq!(paths(f.directory.path()), before);
}

#[test]
fn the_documented_order_syncs_afe_then_sanitises_after_corrections() {
    let text = std::fs::read_to_string(root().join("tools/postbuild/README.md")).unwrap();
    let corrections = text
        .find("tools/ozzit/target/release/ozzit help_corrections ozzit.xlsx src")
        .unwrap();
    let afe = text
        .find("tools/ozzit/target/release/ozzit sync_afe_store ozzit.xlsx src")
        .unwrap();
    let sanitise = text
        .find("tools/ozzit/target/release/ozzit sanitise_workbook ozzit.xlsx")
        .unwrap();
    assert!(corrections < afe && afe < sanitise);
}

#[test]
fn a_workbook_without_shared_strings_is_refused() {
    let f = Fixture::new();
    f.write(&ozzit::archive::Parts::from([(
        BOOK.into(),
        b"<workbook/>".to_vec(),
    )]));
    f.refused_without_writing("workbook: xl/sharedStrings.xml is missing");
}

#[test]
fn all_missing_source_modules_are_reported_in_order_without_writing() {
    let f = Fixture::new();
    f.write(&ozzit::archive::Parts::from([
        (BOOK.into(), b"<workbook/>".to_vec()),
        ("xl/sharedStrings.xml".into(), b"<sst/>".to_vec()),
    ]));
    let empty = f.directory.path().join("empty");
    std::fs::create_dir(&empty).unwrap();
    let before = std::fs::read(&f.book).unwrap();
    let error = help_corrections::run(&f.book, &PyPath::new(&display(&empty))).unwrap_err();
    let failures = error.split("; ").take(MODULES.len()).collect::<Vec<_>>();
    let expected =
        MODULES.map(|m| format!("src: missing {}", display(&empty.join(format!("{m}.txt")))));
    assert_eq!(failures, expected);
    assert_eq!(std::fs::read(&f.book).unwrap(), before);
    assert_eq!(std::fs::read_dir(empty).unwrap().count(), 0);
}

#[test]
fn extra_command_arguments_report_native_usage() {
    let f = Fixture::new();
    let before = f.snapshot();
    assert_eq!(
        f.cli(&["a.xlsx".into(), "src".into(), "extra".into()]),
        (
            1,
            String::new(),
            "FAIL: usage: ozzit help_corrections [workbook] [src dir]\n".into()
        )
    );
    assert_eq!(f.snapshot(), before);
}

#[test]
fn a_missing_workbook_is_refused_by_the_command() {
    let f = Fixture::new();
    let missing = f.directory.path().join("missing.xlsx");
    assert_eq!(
        f.cli(&[display(&missing)]),
        (
            1,
            String::new(),
            format!("FAIL: no such workbook: {}\n", display(&missing))
        )
    );
    assert!(!missing.exists());
}
