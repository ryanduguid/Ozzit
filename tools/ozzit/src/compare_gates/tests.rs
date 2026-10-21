use super::*;

fn capture(code: i64, out: &str, err: &str) -> Capture {
    Capture {
        code,
        out: out.into(),
        err: err.into(),
    }
}
fn pair(rust: Capture, python: Capture) -> (bool, Output) {
    let comparator = Comparator {
        root: Path::new("/fixture"),
        native: "ozzit".into(),
        python: "python",
    };
    let mut captures = [rust, python].into_iter();
    let mut output = Output::default();
    let result = comparator
        .compare_with(
            "gate",
            &["ozzit".into(), "verify_workbook".into()],
            Path::new("/fixture"),
            &mut output,
            &mut |_, _| Ok(captures.next().unwrap()),
        )
        .unwrap();
    (result, output)
}
#[test]
fn identical_results_are_the_same() {
    let (same, output) = pair(capture(0, "OK\n", ""), capture(0, "OK\n", ""));
    assert!(same);
    assert_eq!(output.out, ["same: gate (exit 0)"]);
    assert!(output.err.is_empty());
}
#[test]
fn a_stderr_difference_is_reported() {
    let (same, output) = pair(
        capture(0, "OK\n", ""),
        capture(0, "OK\n", "different diagnostic\n"),
    );
    assert!(!same);
    assert!(output.out.is_empty());
    assert_eq!(
        output.err,
        [
            "DIFFERENT: gate",
            "--- ozzit: exit 0\nOK\n",
            "--- python: exit 0\nOK\ndifferent diagnostic\n"
        ]
    );
}
#[test]
fn a_different_zip_reason_is_reported() {
    let rust = "FAIL: invalid ZIP: invalid Zip archive: Could not find EOCD\n";
    let python = "FAIL: File is not a zip file\n";
    let (same, output) = pair(capture(1, rust, ""), capture(1, python, ""));
    assert!(!same);
    assert_eq!(
        output.err,
        [
            "DIFFERENT: gate".into(),
            format!("--- ozzit: exit 1\n{rust}"),
            format!("--- python: exit 1\n{python}")
        ]
    );
}
#[test]
fn nonzero_status_is_compared_and_matching_failures_succeed() {
    let (same, output) = pair(capture(-15, "x", "y"), capture(-15, "x", "y"));
    assert!(same);
    assert_eq!(output.out, ["same: gate (exit -15)"]);
    assert!(!pair(capture(0, "", ""), capture(1, "", "")).0);
    assert!(!pair(capture(0, "a", ""), capture(0, "b", "")).0);
}
#[test]
fn posix_shlex_preserves_quoting_escapes_comments_and_empty_words() {
    for (source, expected) in [
        (
            "a 'two words' \"three words\"",
            vec!["a", "two words", "three words"],
        ),
        ("'' \"\" a''b", vec!["", "", "ab"]),
        ("a\\ b x\\\ny", vec!["a b", "x\ny"]),
        (
            r#"'a\b' "a\$b\`c" "a\\b\"c""#,
            vec![r"a\b", r"a\$b\`c", "a\\b\"c"],
        ),
        ("#literal a#b\r\tx", vec!["#literal", "a#b", "x"]),
        ("a\u{a0}b\x0cc", vec!["a\u{a0}b\x0cc"]),
    ] {
        assert_eq!(split_command(source).unwrap(), expected, "{source:?}");
    }
    assert_eq!(
        split_command("a\\").unwrap_err(),
        "ValueError: No escaped character"
    );
    assert_eq!(
        split_command("'a").unwrap_err(),
        "ValueError: No closing quotation"
    );
}
#[test]
fn discovery_uses_exact_line_shape_python_unicode_classes_and_order() {
    let text = "run: echo ignored\n  run: tools/ozzit/target/release/ozzit verify_one 'a b'\nrun: |\n  tools/ozzit/target/release/ozzit verify_hidden\n\u{a0}run: tools/ozzit/target/release/ozzit verify_é x#y\nrun: tools/ozzit/target/release/ozzit verify_\n";
    assert_eq!(
        discover(text).unwrap(),
        [
            vec!["tools/ozzit/target/release/ozzit", "verify_one", "a b"],
            vec!["tools/ozzit/target/release/ozzit", "verify_é", "x#y"]
        ]
    );
    assert!(discover("run: tools/ozzit/target/release/ozzit verify_one 'bad\n").is_err());
}
#[test]
fn physical_lines_and_raw_csv_records_preserve_blank_and_unicode_boundaries() {
    let text = "a,b,c\r\n\n\"d,e\",x,y\u{2028}z,q,r";
    let lines = splitlines_keepends(text);
    assert_eq!(lines, ["a,b,c\r\n", "\n", "\"d,e\",x,y\u{2028}", "z,q,r"]);
    let rows = Csv::reader_lines(&lines).unwrap();
    assert_eq!(
        rows,
        [
            vec!["a", "b", "c"],
            vec![],
            vec!["d,e", "x", "y\u{2028}"],
            vec!["z", "q", "r"]
        ]
    );
    assert_eq!(splitlines_keepends(text).concat(), text);
    assert!(splitlines_keepends("").is_empty());
}
#[test]
fn removal_keeps_exact_physical_bytes_and_skips_about_rows() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("functions.csv");
    let original =
        "\u{feff}function,x,old\r\noz.Aboutλ,x,old\r\n\"oz.Realλ\",x,previous\r\noz.Nextλ,x,next";
    fs::write(&path, original).unwrap();
    drop_function_row(root.path()).unwrap();
    assert_eq!(
        fs::read_to_string(path).unwrap(),
        "\u{feff}function,x,old\r\noz.Aboutλ,x,old\r\noz.Nextλ,x,next"
    );
}
#[test]
fn multiline_short_blank_and_missing_rows_refuse_without_mutation() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("functions.csv");
    for (text, error) in [
        (
            "a,b,c\n\"a\nname\",b,c\n",
            "compare_gates: functions.csv has a multi-line row",
        ),
        ("a,b,c\n\nname,b,c\n", "IndexError: list index out of range"),
        ("a,b,c\nshort,b\n", "IndexError: list index out of range"),
        ("a,b,c\nname,b,\n", "StopIteration: no renamed function row"),
    ] {
        fs::write(&path, text).unwrap();
        assert_eq!(drop_function_row(root.path()).unwrap_err(), error);
        assert_eq!(fs::read_to_string(&path).unwrap(), text);
    }
    fs::write(&path, b"\xff").unwrap();
    assert!(drop_function_row(root.path()).is_err());
    assert_eq!(fs::read(path).unwrap(), b"\xff");
}
#[test]
fn parameter_replacement_and_truncation_preserve_reference_bytes() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("src")).unwrap();
    let path = root.path().join("src/Dates.txt");
    fs::write(&path, "CountDOWλ = LAMBDA(\r\n[Starts],[Starts],\r\n").unwrap();
    rename_parameter(root.path()).unwrap();
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "CountDOWλ = LAMBDA(\r\n[Begins],[Starts],\r\n"
    );
    fs::write(&path, "missing marker").unwrap();
    assert_eq!(
        rename_parameter(root.path()).unwrap_err(),
        "compare_gates: src/Dates.txt no longer declares CountDOWλ's [Starts]"
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), "missing marker");
    for bytes in [b"abcde".as_slice(), b""] {
        fs::write(root.path().join("ozzit.xlsx"), bytes).unwrap();
        truncate_workbook(root.path()).unwrap();
        assert_eq!(
            fs::read(root.path().join("ozzit.xlsx")).unwrap(),
            &bytes[..bytes.len() / 2]
        );
    }
}
fn fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("src/nested")).unwrap();
    for (path, bytes) in [
        ("functions.csv", "a,b,c\nname,b,old\n"),
        ("src/Dates.txt", "CountDOWλ = LAMBDA([Starts],"),
        ("src/nested/keep.txt", "unchanged"),
        ("ozzit.xlsx", "abcde"),
        ("CHANGELOG.md", "notes"),
    ] {
        fs::write(root.path().join(path), bytes).unwrap();
    }
    root
}
fn assert_broken_copy(cwd: &Path, target: &str) {
    assert_eq!(
        fs::read(cwd.join("src/nested/keep.txt")).unwrap(),
        b"unchanged"
    );
    if target == "functions.csv" {
        assert_eq!(fs::read(cwd.join(target)).unwrap(), b"a,b,c\n");
    }
    if target == "src" {
        assert!(
            fs::read_to_string(cwd.join("src/Dates.txt"))
                .unwrap()
                .contains("[Begins],")
        );
    }
    if target == "ozzit.xlsx" {
        assert_eq!(fs::read(cwd.join(target)).unwrap(), b"ab");
    }
}
#[test]
fn all_pairs_continue_after_mismatch_and_broken_cases_are_owned_and_ordered() {
    let root = fixture();
    let comparator = Comparator {
        root: root.path(),
        native: "native".into(),
        python: "selected python",
    };
    let gates = ["functions.csv", "src", "ozzit.xlsx"]
        .map(|target| vec!["native".into(), format!("verify_{target}"), target.into()]);
    let mut count = 0;
    let mut temporary = vec![];
    let mut output = Output::default();
    let success = comparator
        .all_with(&gates, &mut output, &mut |argv, cwd| {
            if count % 2 == 1 {
                assert_eq!(argv[0], "selected python");
                assert!(
                    argv[1].starts_with(&root.path().join("tools").to_string_lossy().to_string())
                );
            }
            if count >= 6 {
                assert_ne!(cwd, root.path());
                temporary.push(cwd.to_owned());
                assert_broken_copy(cwd, &argv[argv.len() - 1]);
            }
            count += 1;
            Ok(capture(
                0,
                if count == 2 { "different" } else { "same" },
                "",
            ))
        })
        .unwrap();
    assert!(!success);
    assert_eq!(count, 12);
    assert_eq!(output.out.len(), 5);
    assert_eq!(output.err.len(), 3);
    assert!(temporary.iter().all(|p| !p.exists()));
    assert_eq!(
        fs::read(root.path().join("functions.csv")).unwrap(),
        b"a,b,c\nname,b,old\n"
    );
    assert_eq!(fs::read(root.path().join("ozzit.xlsx")).unwrap(), b"abcde");
}
#[test]
fn cleanup_occurs_when_a_broken_case_process_fails() {
    let root = fixture();
    let comparator = Comparator {
        root: root.path(),
        native: "native".into(),
        python: "python",
    };
    let gates = [vec!["native".into(), "verify_x".into(), "src".into()]];
    let mut seen = None;
    let result = comparator.all_with(&gates, &mut Output::default(), &mut |_, cwd| {
        if cwd != root.path() {
            seen = Some(cwd.to_owned());
            return Err("process failed".into());
        }
        Ok(capture(0, "", ""))
    });
    assert_eq!(result.unwrap_err(), "process failed");
    assert!(!seen.unwrap().exists());
}
#[test]
fn missing_workflow_no_gates_and_argument_arity_refuse() {
    assert_eq!(command(&[]).code, 2);
    assert_eq!(command(&["a".into(), "b".into()]).code, 2);
    let root = fixture();
    assert!(check(root.path(), "python", &mut Output::default()).is_err());
    fs::create_dir_all(root.path().join(".github/workflows")).unwrap();
    fs::write(root.path().join(WORKFLOW), "run: echo nope\r\n").unwrap();
    assert_eq!(
        check(root.path(), "python", &mut Output::default()).unwrap_err(),
        format!("no native verify_* gates found in {WORKFLOW}")
    );
}

#[test]
fn recursive_copy_keeps_times_permissions_and_plain_copy_keeps_default_mode() {
    let root = fixture();
    let source = root.path().join("src/nested/keep.txt");
    let time = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
    fs::File::options()
        .write(true)
        .open(&source)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(time))
        .unwrap();
    let copy = root.path().join("copied");
    copy_input(&root.path().join("src"), &copy, false).unwrap();
    assert_eq!(
        fs::metadata(copy.join("nested/keep.txt"))
            .unwrap()
            .modified()
            .unwrap(),
        time
    );
    let mut permissions = fs::metadata(&source).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&source, permissions).unwrap();
    let plain = root.path().join("plain.txt");
    copy_input(&source, &plain, false).unwrap();
    assert!(!fs::metadata(plain).unwrap().permissions().readonly());
}

#[test]
fn platform_status_codes_preserve_python_process_semantics() {
    #[cfg(windows)]
    {
        use std::os::windows::process::ExitStatusExt;
        assert_eq!(exit_code(ExitStatus::from_raw(u32::MAX)), 4_294_967_295);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(exit_code(ExitStatus::from_raw(15)), -15);
    }
}

#[cfg(unix)]
#[test]
fn recursive_copy_follows_owned_file_and_directory_symlinks() {
    use std::os::unix::fs::symlink;
    let root = fixture();
    symlink(
        root.path().join("src/nested/keep.txt"),
        root.path().join("src/link.txt"),
    )
    .unwrap();
    symlink(
        root.path().join("src/nested"),
        root.path().join("src/link-dir"),
    )
    .unwrap();
    let copy = root.path().join("copy");
    copy_input(&root.path().join("src"), &copy, false).unwrap();
    assert_eq!(fs::read(copy.join("link.txt")).unwrap(), b"unchanged");
    assert_eq!(
        fs::read(copy.join("link-dir/keep.txt")).unwrap(),
        b"unchanged"
    );
    assert!(
        !copy
            .join("link.txt")
            .symlink_metadata()
            .unwrap()
            .is_symlink()
    );
}
