//! The published typed definitions must reproduce the workbook's names.
use crate::{archive::*, common::*, compiler, formula::*, py};
use indexmap::IndexMap;
use std::{collections::BTreeSet, path::Path};

type Collapsed = BTreeSet<(String, String)>;

/// Reads one source module's declarations into `raw` and its collapsed defaults into `collapsed`.
fn read_module(
    path: &Path,
    raw: &mut IndexMap<String, String>,
    collapsed: &mut Collapsed,
    failures: &mut Vec<String>,
) -> Result<()> {
    let text = read_text(path)?;
    for token in ["[0]!", "_xlfn.", "_xlpm.", "_xlop.", "_xlws."] {
        if text.contains(token) {
            failures.push(format!(
                "{} contains the stored-form token {}, which Excel will not accept as typed input",
                display(path),
                py::repr_str(token)
            ));
        }
    }
    for statement in statements(&text) {
        if let Some(m) = captures(NAME, &statement).first() {
            if raw.contains_key(&m[1]) {
                failures.push(format!("{} is declared in more than one module", m[1]));
            }
            raw.insert(m[1].clone(), py::strip(&m[2]).to_owned());
            for dead in dead_conversions(&m[2]) {
                failures.push(format!("{} binds {dead} and never reads it, so the argument it converts is used in its raw form", m[1]));
            }
            collapsed.extend(
                collapsed_defaults(&m[2])
                    .into_iter()
                    .map(|p| (m[1].clone(), p)),
            );
        } else if !py::strip(&statement).is_empty() {
            failures.push(format!(
                "{} has an unparsable statement: {}",
                display(path),
                py::repr_str(prefix(py::strip(&statement), 60))
            ));
        }
    }
    Ok(())
}

/// The collapsed defaults must be exactly the known ones.
fn known_defaults(collapsed: &Collapsed, known: &[(&str, &str)], failures: &mut Vec<String>) {
    let known = known
        .iter()
        .map(|&(f, p)| (f.to_owned(), p.to_owned()))
        .collect::<BTreeSet<_>>();
    for (function, parameter) in collapsed.difference(&known) {
        failures.push(format!("{function} defaults {parameter} with an IF that tests it inside OR() or AND(), which reduce a column to one value, so one blank cell gives every row the default. Test each row: IF(ISOMITTED(x), d, IF(TRIM(x & \"\")=\"\", d, x))"));
    }
    for (function, parameter) in known.difference(collapsed) {
        failures.push(format!("{function} no longer defaults {parameter} inside OR() or AND(); remove its entry from KNOWN_COLLAPSED_DEFAULTS"));
    }
}

/// Compares the parsed sources with the shipped names; returns how many match.
fn compare(
    parsed: &IndexMap<String, String>,
    shipped: &IndexMap<String, (String, String)>,
    failures: &mut Vec<String>,
) -> usize {
    let shipped_set = shipped.keys().cloned().collect::<BTreeSet<_>>();
    let parsed_set = parsed.keys().cloned().collect::<BTreeSet<_>>();
    for name in shipped_set.difference(&parsed_set) {
        failures.push(format!("{name} ships in the workbook but is not in src/"));
    }
    for name in parsed_set.difference(&shipped_set) {
        failures.push(format!(
            "{name} is in src/ but does not ship in the workbook"
        ));
    }
    let mut matched = 0;
    for name in parsed_set.intersection(&shipped_set) {
        let a = canonical(&parsed[name]);
        let b = canonical(&shipped[name].1);
        if a == b {
            matched += 1;
        } else {
            let cut = a
                .chars()
                .zip(b.chars())
                .position(|(a, b)| a != b)
                .unwrap_or(a.chars().count().min(b.chars().count()));
            failures.push(format!("{name} differs from its published source at character {cut}\n        src/ : {}\n        xlsx : {}", char_slice(&a, cut.saturating_sub(30), cut + 50), char_slice(&b, cut.saturating_sub(30), cut + 50)));
        }
    }
    matched
}

pub fn verify(args: &[String]) -> Output {
    verify_with_known(args, &KNOWN)
}

fn verify_with_known(args: &[String], known: &[(&str, &str)]) -> Output {
    if args.len() > 2 {
        return Output::stderr("FAIL: usage: ozzit verify_sources [workbook] [src dir]");
    }
    let workbook = args.first().map_or("ozzit.xlsx", String::as_str);
    let source = args.get(1).map_or("src", String::as_str);
    let src = Path::new(source);
    let mut failures = Vec::new();
    for module in MODULES {
        let path = src.join(format!("{module}.txt"));
        if !path.exists() {
            failures.push(format!("missing source module {}", display(&path)));
        }
    }
    let paths = match txt_files(src) {
        Ok(paths) => paths,
        Err(e) => return Output::failure(format!("FAIL: cannot read {source}: {e}")),
    };
    let mut raw = IndexMap::new();
    let mut collapsed = BTreeSet::new();
    for path in paths {
        if let Err(e) = read_module(&path, &mut raw, &mut collapsed, &mut failures) {
            return Output::failure(format!("FAIL: cannot read {}: {e}", display(&path)));
        }
    }
    known_defaults(&collapsed, known, &mut failures);
    let every = raw.keys().cloned().collect::<BTreeSet<_>>();
    let parsed = raw
        .into_iter()
        .map(|(name, body)| (format!("oz.{name}"), qualify(&body, "oz", &every)))
        .collect::<IndexMap<_, _>>();
    let shipped = match read_parts(Path::new(workbook))
        .and_then(|p| read_book(&p))
        .and_then(|book| compiler::shipped_names(&book))
    {
        Ok(shipped) => shipped,
        Err(e) => return Output::failure(format!("FAIL: cannot read {workbook}: {e}")),
    };
    let matched = compare(&parsed, &shipped, &mut failures);
    let bad = format!(
        "FAIL: {} problem(s) comparing {workbook} with {source}/",
        failures.len()
    );
    Output::report(
        failures,
        bad,
        format!(
            "OK: {source}/ reproduces all {matched} functions in {workbook}, every date conversion in them is read, and only the {} listed argument defaults test a column inside OR() or AND()",
            known.len()
        ),
    )
}

#[cfg(test)]
mod source_contract_tests {
    use super::*;
    use std::{collections::BTreeMap, path::PathBuf};

    fn snapshot(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
        fn visit(root: &Path, path: &Path, result: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
            for entry in std::fs::read_dir(path).unwrap() {
                let entry = entry.unwrap();
                assert!(!entry.file_type().unwrap().is_symlink());
                let path = entry.path();
                let bytes = if path.is_dir() {
                    visit(root, &path, result);
                    None
                } else {
                    Some(std::fs::read(&path).unwrap())
                };
                result.insert(path.strip_prefix(root).unwrap().into(), bytes);
            }
        }
        let mut result = BTreeMap::new();
        visit(root, root, &mut result);
        result
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "Retains the original fixture and all 11 ordered source defects together."
    )]
    fn every_original_source_defect_has_the_exact_order_with_an_empty_known_list() {
        let directory = tempfile::tempdir().unwrap();
        let src = directory.path().join("src");
        std::fs::create_dir(&src).unwrap();
        std::fs::write(src.join("Dates.txt"), "Sameλ = LAMBDA(x, x + 1);\nDiffλ = LAMBDA(x, x + 2);\nDeadλ = LAMBDA(d, LET(CvtD, DATEVALUE(d), d));\nnot a declaration;\n   ;\n").unwrap();
        std::fs::write(
            src.join("Extra.txt"),
            "Sameλ = LAMBDA(x, x + 1);\nStoredλ = LAMBDA(x, _xlfn.SINGLE(x));\n",
        )
        .unwrap();
        let names = [
            ("oz.Sameλ", "LAMBDA(_xlpm.x, _xlpm.x + 1)"),
            ("oz.Diffλ", "LAMBDA(x, x + 3)"),
            ("oz.Deadλ", "LAMBDA(d, LET(CvtD, DATEVALUE(d), d))"),
            ("oz.Storedλ", "LAMBDA(x, SINGLE(x))"),
            ("oz.ShippedOnlyλ", "1"),
        ];
        let body = names
            .iter()
            .map(|(n, b)| format!("<definedName name=\"{n}\">{b}</definedName>"))
            .collect::<String>();
        let workbook = directory.path().join("ozzit.xlsx");
        write_deterministic(
            &workbook,
            &[(
                BOOK.into(),
                format!("<workbook><definedNames>{body}</definedNames></workbook>").into_bytes(),
            )]
            .into(),
        )
        .unwrap();
        let before = snapshot(directory.path());
        let output = verify_with_known(&[display(&workbook), display(&src)], &[]);
        assert_eq!(snapshot(directory.path()), before);
        let mut expected = ["Essentials", "Financial", "Ratios", "Utilities", "Debt"]
            .map(|m| {
                format!(
                    "missing source module {}",
                    display(&src.join(format!("{m}.txt")))
                )
            })
            .to_vec();
        expected.extend([
            "Deadλ binds CvtD and never reads it, so the argument it converts is used in its raw form".into(),
            format!("{} has an unparsable statement: 'not a declaration'", display(&src.join("Dates.txt"))),
            format!("{} contains the stored-form token '_xlfn.', which Excel will not accept as typed input", display(&src.join("Extra.txt"))),
            "Sameλ is declared in more than one module".into(),
            "oz.ShippedOnlyλ ships in the workbook but is not in src/".into(),
            "oz.Diffλ differs from its published source at character 11\n        src/ : LAMBDA(X,X+2)\n        xlsx : LAMBDA(X,X+3)".into(),
        ]);
        assert_eq!(expected.len(), 11);
        assert_eq!(output.code, 1);
        assert!(output.err.is_empty());
        assert_eq!(
            output.out[0],
            format!(
                "FAIL: 11 problem(s) comparing {} with {}/",
                display(&workbook),
                display(&src)
            )
        );
        assert_eq!(
            output.out[1..],
            expected
                .iter()
                .map(|s| format!("  - {s}"))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn changed_known_list_reports_both_the_new_occurrence_and_the_stale_entry() {
        let directory = tempfile::tempdir().unwrap();
        let src = directory.path().join("src");
        std::fs::create_dir(&src).unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let workbook = directory.path().join("ozzit.xlsx");
        std::fs::copy(root.join("ozzit.xlsx"), &workbook).unwrap();
        for module in MODULES {
            let name = format!("{module}.txt");
            std::fs::copy(root.join("src").join(&name), src.join(name)).unwrap();
        }
        let mut known = KNOWN.to_vec();
        let removed = known
            .iter()
            .position(|pair| *pair == ("DayCountRateλ", "Convention"))
            .unwrap();
        known.remove(removed);
        known.push(("Fixtureλ", "Rate"));
        let before = snapshot(directory.path());
        let output = verify_with_known(&[display(&workbook), display(&src)], &known);
        assert_eq!(snapshot(directory.path()), before);
        assert_eq!(output.code, 1);
        assert!(output.err.is_empty());
        assert_eq!(output.out.len(), 3);
        assert_eq!(
            output.out[0],
            format!(
                "FAIL: 2 problem(s) comparing {} with {}/",
                display(&workbook),
                display(&src)
            )
        );
        assert!(output.out[1].starts_with(
            "  - DayCountRateλ defaults Convention with an IF that tests it inside OR() or AND()"
        ));
        assert_eq!(
            output.out[2],
            "  - Fixtureλ no longer defaults Rate inside OR() or AND(); remove its entry from KNOWN_COLLAPSED_DEFAULTS"
        );
    }
}
