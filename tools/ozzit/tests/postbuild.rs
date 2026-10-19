//! Every postbuild pass reports "already applied" on the committed workbook,
//! `src/` and functions.csv, and leaves all three byte for byte unchanged.
use std::path::Path;

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        std::fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
    }
}

fn snapshot(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    for name in ["ozzit.xlsx", "functions.csv"] {
        out.push((name.to_string(), std::fs::read(dir.join(name)).unwrap()));
    }
    let mut src = std::fs::read_dir(dir.join("src"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect::<Vec<_>>();
    src.sort();
    for path in src {
        out.push((path.display().to_string(), std::fs::read(path).unwrap()));
    }
    out
}

#[test]
fn every_pass_is_already_applied_to_the_committed_workbook() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = tempfile::tempdir().unwrap();
    std::fs::copy(root.join("ozzit.xlsx"), dir.path().join("ozzit.xlsx")).unwrap();
    std::fs::copy(root.join("functions.csv"), dir.path().join("functions.csv")).unwrap();
    copy_dir(&root.join("src"), &dir.path().join("src"));
    let before = snapshot(dir.path());
    let book = dir.path().join("ozzit.xlsx").display().to_string();
    let src = dir.path().join("src").display().to_string();
    let index = dir.path().join("functions.csv").display().to_string();
    for (pass, args) in [
        ("cover_label", vec![book.clone()]),
        ("sheet_names", vec![book.clone()]),
        ("remove_residue", vec![book.clone()]),
        ("workbook_palette", vec![book.clone()]),
        ("help_links", vec![book.clone(), src.clone()]),
        ("fy27_help_text", vec![book.clone(), src.clone()]),
        ("gst_help_text", vec![book.clone(), src.clone()]),
        ("strip_revision_history", vec![book.clone(), src.clone()]),
        ("help_corrections", vec![book.clone(), src.clone()]),
        (
            "aasb16_leases",
            vec![book.clone(), src.clone(), index.clone()],
        ),
        (
            "rate_date_helpers",
            vec![book.clone(), src.clone(), index.clone()],
        ),
        (
            "working_capital_days",
            vec![book.clone(), src.clone(), index.clone()],
        ),
        (
            "ato_benchmarks_rate_lookup",
            vec![book.clone(), src.clone(), index.clone()],
        ),
    ] {
        let mut argv = vec![pass.to_string()];
        argv.extend(args);
        let output = ozzit::dispatch(&argv);
        assert_eq!(output.code, 0, "{pass}: {:?} {:?}", output.out, output.err);
        assert!(
            output.out.iter().any(|l| l.contains("already")),
            "{pass}: {:?}",
            output.out
        );
        assert!(before == snapshot(dir.path()), "{pass} changed a store");
    }
}

#[test]
fn index_rows_read_as_dictreader_does() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = tempfile::tempdir().unwrap();
    std::fs::copy(root.join("ozzit.xlsx"), dir.path().join("ozzit.xlsx")).unwrap();
    copy_dir(&root.join("src"), &dir.path().join("src"));
    let book = dir.path().join("ozzit.xlsx").display().to_string();
    let src = dir.path().join("src").display().to_string();
    let index = dir.path().join("index.csv");
    let run = |pass: &str| {
        let argv = [pass, &book, &src, &index.display().to_string()].map(String::from);
        ozzit::dispatch(&argv).code
    };
    let header = "module,previous_name,signature,description,function\n";
    // A short row leaves its function as None, which Python cannot sort: nothing is written.
    let short = format!("{header}Financial,,Sig,Desc\n");
    std::fs::write(&index, &short).unwrap();
    assert_eq!(run("aasb16_leases"), 1);
    assert_eq!(std::fs::read_to_string(&index).unwrap(), short);
    // An explicitly empty function sorts first and is written back.
    std::fs::write(&index, format!("{header}Financial,,Sig,Desc,\n")).unwrap();
    assert_eq!(run("aasb16_leases"), 0);
    // A blank first record is DictReader's (empty) header, so the stores disagree.
    let committed = std::fs::read(root.join("functions.csv")).unwrap();
    std::fs::write(&index, [b"\n".as_slice(), &committed].concat()).unwrap();
    assert_eq!(run("working_capital_days"), 1);
    std::fs::write(&index, [&committed, b"\n\n".as_slice()].concat()).unwrap();
    assert_eq!(run("working_capital_days"), 0);
}

#[test]
fn the_hue_remap_matches_python_colorsys() {
    use ozzit::postbuild::workbook_palette::hue_map;
    // Python: workbook_palette.hue_map on each input.
    for (input, expected) in [
        ("3F656F", Some("624C59")),
        ("66A1AF", Some("9B7A8D")),
        ("D7876B", Some("ADA095")),
        ("888888", None),
        ("FF0000", None),
    ] {
        assert_eq!(hue_map(input).as_deref(), expected, "{input}");
    }
}
