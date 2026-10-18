//! Verify retained records and their inputs without executing Excel or pyxirr.
use crate::{
    common::*,
    py,
    pyjson::{self, Json},
};
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    io::Read,
    path::{Path, PathBuf},
};

pub const RELEASE_SHA256: &str = "0306793a7e473ce70e78149fea1e107fc0f714d61ab50960c16f6fd528878f6f";
pub const COMPARISON: &str = "docs/pyxirr-release-v3.4.2.json";
pub const INSTALLATION: &str = "docs/install-v3.4.2.json";
pub const RUNNERS: &[&str] = &[
    "tools/pyxirr_comparison.py",
    "tools/excel_eval_formulas.ps1",
];
pub const INSTALL_RUNNER: &str = "tools/excel_install_selftest.ps1";
const MONEY: f64 = 1e-6;
const RATE: f64 = 1e-7;
const ROOT_NOTE: &str = "Excel's XIRR returns #NUM! for this cash flow with any guess tried (0.05, 0.1, 0.25), and IRRλ passes that on. pyxirr returns one of the two valid rates. Neither is wrong; a model with sign changes like these needs a stated rule for which rate it uses.";

struct Case {
    id: &'static str,
    formula: &'static str,
    count: usize,
    tolerance: f64,
}
const CASES: &[Case] = &[
    Case {
        id: "irr-help-example",
        formula: "=oz.IRRλ({0,0,-150,0,-100,10,20,30,40,50,60,70}, EDATE(DATE(2026,7,1), SEQUENCE(1,24,0)))",
        count: 1,
        tolerance: RATE,
    },
    Case {
        id: "irr-irregular-dates",
        formula: "=oz.IRRλ({-10000,2750,4250,3250,2750}, DATE({2026,2026,2027,2027,2027},{7,9,3,7,9},{1,1,30,15,1}))",
        count: 1,
        tolerance: RATE,
    },
    Case {
        id: "irr-two-roots",
        formula: "=oz.IRRλ({-100,230,-132}, DATE({2026,2027,2028},7,1))",
        count: 1,
        tolerance: MONEY,
    },
    Case {
        id: "lease-arrears",
        formula: "=oz.LeaseLiabilityλ({100,100,100}, 0.05)",
        count: 1,
        tolerance: MONEY,
    },
    Case {
        id: "lease-advance",
        formula: "=oz.LeaseLiabilityλ({100,100,100}, 0.05, TRUE)",
        count: 1,
        tolerance: MONEY,
    },
    Case {
        id: "lease-60-months",
        formula: "=oz.LeaseLiabilityλ(SEQUENCE(1,60,1500,0), oz.PeriodRateλ(0.065))",
        count: 1,
        tolerance: MONEY,
    },
    Case {
        id: "amortise-monthly",
        formula: "=oz.AmortiseBλ(250000, 6.2%, 60, 60)",
        count: 300,
        tolerance: MONEY,
    },
    Case {
        id: "amortise-quarterly-deferred",
        formula: "=oz.AmortiseBλ(100000, 5%, 8, 12, 3, 3)",
        count: 60,
        tolerance: MONEY,
    },
];
struct InstallCase {
    id: &'static str,
    names: &'static [&'static str],
    source: &'static str,
    formula: &'static str,
    changed_formula: &'static str,
    rows: usize,
    columns: usize,
}
const INSTALL_CASES: &[InstallCase] = &[
    InstallCase {
        id: "rolling-sum",
        names: &["oz.RollingSumλ"],
        source: "oz.RollingSumλ!H21",
        formula: "=oz.RollingSumλ({1,2,3,4},2)",
        changed_formula: "=oz.RollingSumλ({2,4,6,8},2)",
        rows: 1,
        columns: 4,
    },
    InstallCase {
        id: "amortise-helpers",
        names: &["oz.Amortiseλ", "oz.TimelineOffsetλ", "oz.TimelinePositionλ"],
        source: "oz.Amortiseλ!I24",
        formula: "=oz.Amortiseλ(1000,0.06,4,DATE(2026,7,1))",
        changed_formula: "=oz.Amortiseλ(2000,0.06,4,DATE(2026,7,1))",
        rows: 6,
        columns: 4,
    },
];

fn require(condition: bool, message: &str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}
fn key<'a>(value: &'a Json, name: &str) -> Result<&'a Json> {
    match value {
        Json::Object(_) => value.get(name).ok_or_else(|| py::repr_str(name)),
        Json::Array(_) => Err("list indices must be integers or slices, not str".into()),
        Json::Str(_) => Err("string indices must be integers, not 'str'".into()),
        _ => Err(format!(
            "'{}' object is not subscriptable",
            value.type_name()
        )),
    }
}
fn number(value: &Json) -> Result<f64> {
    match value {
        Json::Float(f) => Ok(*f),
        Json::Int(i) => i
            .to_f64()
            .filter(|v| v.is_finite())
            .ok_or_else(|| "int too large to convert to float".into()),
        _ => Err(format!("must be real number, not {}", value.type_name())),
    }
}
fn finite(value: &Json) -> Result<bool> {
    Ok(matches!(value, Json::Int(_) | Json::Float(_)) && number(value)?.is_finite())
}
fn integer(value: &Json, expected: usize) -> bool {
    value == &Json::Int(expected.into())
}
fn is_true(value: &Json) -> bool {
    value == &Json::Bool(true)
}
fn is_text(value: &Json, expected: &str) -> bool {
    value.as_str() == Some(expected)
}
fn texts(value: &Json, expected: &[&str]) -> bool {
    matches!(value, Json::Array(items) if items.len() == expected.len()
        && items.iter().zip(expected).all(|(a,b)| is_text(a,b)))
}
fn digest_text(value: &Json, size: usize) -> bool {
    value.as_str().is_some_and(|s| {
        s.len() == size
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
fn runner_inventory(value: &Json) -> Result<bool> {
    let names = match value {
        Json::Object(pairs) => pairs
            .iter()
            .map(|(k, _)| k.as_str())
            .collect::<BTreeSet<_>>(),
        Json::Array(items) => {
            let mut names = BTreeSet::new();
            let mut strings = true;
            for item in items {
                match item {
                    Json::Str(s) => {
                        names.insert(s.as_str());
                    }
                    Json::Array(_) | Json::Object(_) => {
                        return Err(format!("unhashable type: '{}'", item.type_name()));
                    }
                    _ => strings = false,
                }
            }
            if !strings {
                return Ok(false);
            }
            names
        }
        Json::Str(_) => return Ok(false),
        _ => return Err(format!("'{}' object is not iterable", value.type_name())),
    };
    Ok(names == RUNNERS.iter().copied().collect())
}

pub fn sha256(path: &Path) -> Result<String> {
    let path = py::PyPath::new(&display(path)).fs();
    let mut file = std::fs::File::open(&path).map_err(|e| io_error(&e, &path))?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        match file.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => hash.update(&buffer[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(io_error(&e, &path)),
        }
    }
    Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}
pub fn read_record(path: &Path) -> Result<Json> {
    let path = py::PyPath::new(&display(path)).fs();
    let data = pyjson::loads_unique(&py::universal_newlines(&read_sig(&path)?))
        .map_err(|e| e.to_string())?;
    require(
        matches!(data, Json::Object(_)),
        "evidence must be an object",
    )?;
    Ok(data)
}
pub fn check_grid(grid: &Json) -> Result<()> {
    for name in ["rows", "columns"] {
        require(
            matches!(key(grid,name)?, Json::Int(i) if i > &BigInt::from(0)),
            "invalid installation spill shape",
        )?;
    }
    let values = key(grid, "values")?;
    let valid = if let Json::Array(values) = values {
        let (Json::Int(rows), Json::Int(columns)) = (key(grid, "rows")?, key(grid, "columns")?)
        else {
            unreachable!()
        };
        if rows * columns != BigInt::from(values.len()) {
            false
        } else {
            let mut valid = true;
            for value in values {
                if !finite(value)? {
                    valid = false;
                    break;
                }
            }
            valid
        }
    } else {
        false
    };
    require(valid, "invalid installation spill values")
}
pub fn installation_values(id: &str) -> Vec<f64> {
    if id == "rolling-sum" {
        return vec![1.0, 3.0, 5.0, 7.0];
    }
    let (mut balance, rate) = (1000.0, 0.06 / 12.0);
    let payment = balance * rate / (1.0_f64 - (1.0_f64 + rate).powf(-4.0));
    let (mut opening, mut interest, mut closing, mut principal) =
        ([0.0; 4], [0.0; 4], [0.0; 4], [0.0; 4]);
    for k in 0..4 {
        opening[k] = balance;
        interest[k] = balance * rate;
        principal[k] = payment - interest[k];
        balance -= principal[k];
        closing[k] = balance;
    }
    [
        [1000.0, 0.0, 0.0, 0.0],
        opening,
        interest,
        [-payment; 4],
        closing,
        principal,
    ]
    .into_iter()
    .flatten()
    .collect()
}

// These three finite terms fit f64. Preserve small partials and the final
// halfway rounding correction used by math.fsum instead of summing in order.
fn fsum3(terms: [f64; 3]) -> f64 {
    let mut partials = Vec::<f64>::new();
    for mut x in terms {
        let mut n = 0;
        for i in 0..partials.len() {
            let mut y = partials[i];
            if x.abs() < y.abs() {
                std::mem::swap(&mut x, &mut y);
            }
            let high = x + y;
            let low = y - (high - x);
            if low != 0.0 {
                partials[n] = low;
                n += 1;
            }
            x = high;
        }
        partials.truncate(n);
        partials.push(x);
    }
    let mut high = partials.pop().unwrap_or(0.0);
    let mut low = 0.0;
    while let Some(y) = partials.pop() {
        let x = high;
        high = x + y;
        low = y - (high - x);
        if low != 0.0 {
            break;
        }
    }
    if low != 0.0 && partials.last().is_some_and(|v| (*v > 0.0) == (low > 0.0)) {
        let y = low * 2.0;
        let x = high + y;
        if y == x - high {
            high = x;
        }
    }
    high
}
fn reference_npv(rate: f64) -> Result<f64> {
    let base = 1.0 + rate;
    let denominator = base.powf(731.0 / 365.0);
    if !denominator.is_finite() {
        return Err(if cfg!(target_os = "linux") {
            "(34, 'Numerical result out of range')"
        } else {
            "(34, 'Result too large')"
        }
        .into());
    }
    Ok(fsum3([-100.0, 230.0 / base, -132.0 / denominator]).abs())
}

pub fn verify(
    root: &Path,
    workbook: Option<&Path>,
    destination: Option<&Path>,
) -> Result<Vec<String>> {
    let trial = read_record(&root.join(COMPARISON))?;
    let install = read_record(&root.join(INSTALLATION))?;
    require(
        key(&trial, "workbook_sha256")? == key(&trial, "expected_sha256")?
            && key(&trial, "expected_sha256")? == key(&install, "source_sha256")?
            && is_text(key(&install, "source_sha256")?, RELEASE_SHA256),
        "evidence does not identify the retained v3.4.2 workbook",
    )?;
    require(
        is_text(key(&trial, "pyxirr")?, "0.10.8"),
        "pyxirr version changed",
    )?;
    require(
        digest_text(key(&trial, "runner_commit")?, 40),
        "invalid runner commit",
    )?;
    for item in [&trial, &install] {
        require(
            key(item, "excel")?
                .as_str()
                .is_some_and(|s| !py::strip(s).is_empty()),
            "missing Excel build",
        )?;
    }
    let runners = key(&trial, "runner_files")?;
    require(
        runner_inventory(runners)?,
        "comparison runner inventory changed",
    )?;
    for name in RUNNERS {
        require(
            is_text(key(runners, name)?, &sha256(&root.join(name))?),
            &format!("runner evidence is stale: {name}"),
        )?;
    }
    require(
        is_text(
            key(&install, "runner_sha256")?,
            &sha256(&root.join(INSTALL_RUNNER))?,
        ),
        &format!("runner evidence is stale: {INSTALL_RUNNER}"),
    )?;

    let Json::Array(cases) = key(&trial, "cases")? else {
        return Err("comparison cases are missing, duplicated or changed".into());
    };
    let inventory = cases
        .iter()
        .map(|c| Ok((key(c, "id")?, key(c, "formula")?)))
        .collect::<Result<Vec<_>>>()?;
    require(
        inventory.len() == CASES.len()
            && inventory
                .iter()
                .zip(CASES)
                .all(|((id, formula), d)| is_text(id, d.id) && is_text(formula, d.formula)),
        "comparison cases are missing, duplicated or changed",
    )?;
    for (case, defined) in cases.iter().zip(CASES) {
        require(
            integer(key(case, "values")?, defined.count),
            "invalid comparison value count",
        )?;
        require(
            matches!(key(case, "agrees")?, Json::Bool(_)),
            "comparison agreement must be boolean",
        )?;
        if defined.id == "irr-two-roots" {
            require(
                key(case, "agrees")? == &Json::Bool(false)
                    && is_text(key(case, "ozzit")?, "#NUM!")
                    && finite(key(case, "pyxirr")?)?
                    && finite(key(case, "pyxirr_npv")?)?
                    && number(key(case, "pyxirr_npv")?)? >= 0.0
                    && number(key(case, "pyxirr_npv")?)? <= MONEY
                    && key(case, "max_abs_diff")? == &Json::Null
                    && is_text(key(case, "note")?, ROOT_NOTE),
                "comparison has an unsupported difference",
            )?;
            let rate = number(key(case, "pyxirr")?)?;
            require(
                rate > -1.0,
                "reference rate is outside the real-valued XNPV domain",
            )?;
            let npv = reference_npv(rate)?;
            require(
                npv <= MONEY && (npv - number(key(case, "pyxirr_npv")?)?).abs() <= MONEY,
                "reference rate does not reproduce the recorded near-zero NPV",
            )?;
        } else {
            require(
                key(case, "agrees")?.truthy()
                    && finite(key(case, "max_abs_diff")?)?
                    && number(key(case, "max_abs_diff")?)? >= 0.0
                    && number(key(case, "max_abs_diff")?)? <= defined.tolerance
                    && matches!(key(case,"tolerance")?, Json::Float(f) if *f == defined.tolerance)
                    && key(case, "note")? == &Json::Null,
                "comparison difference exceeds its declared tolerance",
            )?;
            if defined.count == 1 {
                require(
                    finite(key(case, "ozzit")?)? && finite(key(case, "pyxirr")?)?,
                    "non-numeric comparison result",
                )?;
                require(
                    ((number(key(case, "ozzit")?)? - number(key(case, "pyxirr")?)?).abs()
                        - number(key(case, "max_abs_diff")?)?)
                    .abs()
                        <= 1e-15,
                    "comparison difference contradicts its results",
                )?;
            }
        }
    }
    require(
        is_true(key(&install, "passed")?)
            && is_true(key(&install, "source_unchanged")?)
            && integer(key(&install, "external_links")?, 0),
        "installation did not pass without source changes or external links",
    )?;
    require(
        digest_text(key(&install, "destination_sha256")?, 64),
        "invalid installed workbook digest",
    )?;
    let Json::Array(cases) = key(&install, "cases")? else {
        return Err("installation case inventory changed".into());
    };
    let inventory = cases
        .iter()
        .map(|c| key(c, "id"))
        .collect::<Result<Vec<_>>>()?;
    require(
        inventory.len() == INSTALL_CASES.len()
            && inventory
                .iter()
                .zip(INSTALL_CASES)
                .all(|(id, d)| is_text(id, d.id)),
        "installation case inventory changed",
    )?;
    for (case, defined) in cases.iter().zip(INSTALL_CASES) {
        require(
            is_true(key(case, "passed")?) && texts(key(case, "required_names")?, defined.names),
            "installation case failed or required names changed",
        )?;
        let inputs = [
            key(case, "copied_from")?,
            key(case, "formula")?,
            key(case, "changed_formula")?,
        ];
        require(
            inputs
                .iter()
                .zip([defined.source, defined.formula, defined.changed_formula])
                .all(|(a, b)| is_text(a, b)),
            "installation inputs changed",
        )?;
        let before = key(case, "reopened")?;
        let after = key(case, "changed_input")?;
        check_grid(before)?;
        check_grid(after)?;
        require(
            integer(key(before, "rows")?, defined.rows)
                && key(before, "rows")? == key(after, "rows")?
                && integer(key(before, "columns")?, defined.columns)
                && key(before, "columns")? == key(after, "columns")?,
            "installation spill shape changed",
        )?;
        let expected = installation_values(defined.id);
        for (grid, scale) in [(before, 1.0), (after, 2.0)] {
            let Json::Array(values) = key(grid, "values")? else {
                unreachable!()
            };
            require(
                values
                    .iter()
                    .zip(&expected)
                    .all(|(v, t)| number(v).is_ok_and(|n| (n - scale * t).abs() <= MONEY)),
                "installation values contradict the fixture inputs",
            )?;
        }
    }
    let mut messages = vec!["Retained v3.4.2 evidence: 8 comparison cases and 2 installation cases checked; runners match.".into()];
    messages.push(if sha256(&root.join("ozzit.xlsx"))? == RELEASE_SHA256 {"The retained workbook matches this checkout."}
        else {"Historical release evidence: the checkout workbook differs; these records do not validate it."}.into());
    for (label, path, digest) in [
        ("release workbook", workbook, RELEASE_SHA256),
        (
            "installed workbook",
            destination,
            key(&install, "destination_sha256")?.as_str().unwrap(),
        ),
    ] {
        if let Some(path) = path {
            require(
                sha256(path)? == digest,
                &format!("{label} hash does not match the retained evidence"),
            )?;
            messages.push(format!(
                "Supplied {label} bytes match the retained evidence."
            ));
        }
    }
    messages.push(
        "No native execution performed. Unprovided release or installed files are not verified."
            .into(),
    );
    Ok(messages)
}

fn default_root() -> Result<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        for directory in exe.parent().into_iter().flat_map(Path::ancestors) {
            if directory.join(COMPARISON).is_file()
                || directory.join("tools/pyxirr_comparison.py").is_file()
            {
                return Ok(directory.to_owned());
            }
        }
    }
    std::env::current_dir().map_err(|e| e.to_string())
}
const USAGE: &str = "usage: ozzit verify_native_evidence [-h] [--workbook WORKBOOK]\n                                    [--destination DESTINATION]";
pub fn command(args: &[String]) -> Output {
    let mut workbook = None;
    let mut destination = None;
    let mut unknown = Vec::new();
    let mut i = 0;
    let argument_error = |message: String| Output {
        code: 2,
        out: vec![],
        err: vec![format!(
            "{USAGE}\nozzit verify_native_evidence: error: {message}"
        )],
    };
    while i < args.len() {
        let arg = &args[i];
        if arg == "--" {
            unknown.extend(args[i..].iter().cloned());
            break;
        }
        let (name, inline) = arg
            .split_once('=')
            .map_or((arg.as_str(), None), |(n, v)| (n, Some(v)));
        if name == "--" {
            return argument_error(format!(
                "ambiguous option: {arg} could match --help, --workbook, --destination"
            ));
        }
        let option = ["--help", "--workbook", "--destination"]
            .into_iter()
            .find(|o| name.starts_with("--") && o.starts_with(name));
        if (name.starts_with("-h") && !name.starts_with("--")) || option == Some("--help") {
            if let Some(v) = inline.filter(|_| name == "-h" || option == Some("--help")) {
                return argument_error(format!(
                    "argument -h/--help: ignored explicit argument {}",
                    py::repr_str(v)
                ));
            }
            return Output::ok(vec![format!(
                "{USAGE}\n\nCheck the retained v3.4.2 native evidence without running Excel or pyxirr.\n\noptions:\n  -h, --help            show this help message and exit\n  --workbook WORKBOOK   optional exact v3.4.2 release workbook\n  --destination DESTINATION\n                        optional workbook from the recorded installation"
            )]);
        }
        if let Some(option) = option {
            let value = if let Some(value) = inline {
                value.to_owned()
            } else {
                i += 1;
                let Some(value) = args
                    .get(i)
                    .filter(|v| v.as_str() == "-" || !v.starts_with('-') || matches(r"^-\.?\d", v))
                else {
                    return argument_error(format!("argument {option}: expected one argument"));
                };
                value.clone()
            };
            let value = py::PyPath::new(&value).fs();
            if option == "--workbook" {
                workbook = Some(value);
            } else {
                destination = Some(value);
            }
        } else {
            unknown.push(arg.clone());
        }
        i += 1;
    }
    if !unknown.is_empty() {
        return argument_error(format!("unrecognized arguments: {}", unknown.join(" ")));
    }
    match default_root().and_then(|root| verify(&root, workbook.as_deref(), destination.as_deref()))
    {
        Ok(messages) => Output::ok(messages),
        Err(error) => Output::failure(format!("FAIL: native evidence: {error}")),
    }
}
