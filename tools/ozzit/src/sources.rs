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
fn known_defaults(collapsed: &Collapsed, failures: &mut Vec<String>) {
    let known = KNOWN
        .map(|(f, p)| (f.to_owned(), p.to_owned()))
        .into_iter()
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
    known_defaults(&collapsed, &mut failures);
    let every = raw.keys().cloned().collect::<BTreeSet<_>>();
    let parsed = raw
        .into_iter()
        .map(|(name, body)| (format!("oz.{name}"), qualify(&body, "oz", &every)))
        .collect::<IndexMap<_, _>>();
    let book = match read_parts(Path::new(workbook)).and_then(|p| read_book(&p)) {
        Ok(book) => book,
        Err(e) => return Output::failure(format!("FAIL: cannot read {workbook}: {e}")),
    };
    let shipped = compiler::shipped_names(&book);
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
            KNOWN.len()
        ),
    )
}
