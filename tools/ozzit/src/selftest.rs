//! Turn the help's worked examples and help tables into native self-test assertions.
//!
//! Every EXAMPLES block in `src/` is read; each example that stands on its own
//! becomes a PowerShell assertion that `tools/excel_selftest.ps1` dot-sources and
//! evaluates in Excel. Nothing here evaluates a formula.
use crate::{
    cache::repository_root,
    common::*,
    formula::{self, MODULES, PLAIN, XLFN},
    py,
};
use num_bigint::BigInt;
use num_traits::{Signed, Zero};
use std::collections::BTreeSet;
use std::path::Path;

const NAMESPACE: &str = "oz";
pub const OUTPUT: &str = "tools/selftest_examples.ps1";
const NUMBER: &str = r"^(-?\d*(?:\.\d+)?%?)(?:\s*[A-Za-z]+)?$";
const ISO_DATE: &str = r"^(\d{4})-(\d{2})-(\d{2})$";
const LIST: &str = r"^\s*-?\d+(?:\.\d+)?(?:\s*,\s*-?\d+(?:\.\d+)?)+\s*$";
const SHAPE: &str = r"^(\d+) rows?, (\d+) cols?$";
const WORD: &str = r"^[A-Za-z0-9:_\-]+$";
const IDENT: &str = r#"(?<![A-Za-z0-9_.λ"])([A-Za-z_][A-Za-z0-9_.λ]*\??)"#;

fn constant(word: &str) -> bool {
    matches!(word, "TRUE" | "FALSE")
}

/// Every double-quoted literal in order, `""` unescaped.
pub fn read_literals(body: &str) -> String {
    let chars = body.chars().collect::<Vec<_>>();
    let (mut out, mut i, n) = (String::new(), 0, chars.len());
    while i < n {
        if chars[i] != '"' {
            i += 1;
            continue;
        }
        i += 1;
        while i < n {
            if chars[i] == '"' {
                if i + 1 < n && chars[i + 1] == '"' {
                    out.push('"');
                    i += 2;
                    continue;
                }
                i += 1;
                break;
            }
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

pub fn help_rows(body: &str) -> Vec<(String, String)> {
    read_literals(body)
        .split('¶')
        .filter_map(|raw| {
            raw.split_once('→').map(|(label, text)| {
                (
                    py::strip(label).to_string(),
                    py::strip(text.trim_end_matches('→')).to_string(),
                )
            })
        })
        .collect()
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Example {
    pub formula: String,
    pub results: Vec<String>,
}

/// What `examples_of` has read so far from the rows after EXAMPLE.
#[derive(Default)]
struct ExampleReader {
    out: Vec<Example>,
    bindings: Vec<(String, String)>,
    current: Option<usize>,
    collecting: bool,
}

impl ExampleReader {
    /// A row whose text is a formula: a named input, or a new example.
    fn formula(&mut self, label: &str, formula: &str) {
        self.collecting = false;
        if !label.is_empty()
            && matches(r"^[A-Za-z_][A-Za-z0-9_]*$", label)
            && !constant(&label.to_uppercase())
            && !label.starts_with("Result")
            && !label.starts_with("Formula")
        {
            match self.bindings.iter_mut().find(|(k, _)| k == label) {
                Some(binding) => binding.1 = formula.into(),
                None => self.bindings.push((label.into(), formula.into())),
            }
            self.current = None;
            return;
        }
        let mut example = Example {
            formula: formula.into(),
            results: vec![],
        };
        if !label.is_empty() && !label.starts_with("Formula") {
            example.results.push(label.into());
        }
        self.out.push(example);
        self.current = Some(self.out.len() - 1);
    }

    /// Any other row: a wrapped formula, a result heading or result text.
    fn other(&mut self, label: &str, text: &str) {
        let count = |s: &str, c: char| s.chars().filter(|x| *x == c).count();
        if let Some(c) = self.current {
            let formula = &self.out[c].formula;
            if !text.is_empty() && label.is_empty() && count(formula, '(') > count(formula, ')') {
                // A formula wrapped onto a second row.
                self.out[c].formula = format!("{formula} {text}");
                return;
            }
        }
        if text.starts_with("Result") || text.starts_with("Formula") {
            self.collecting = text.starts_with("Result") && self.current.is_some();
            return;
        }
        if let Some(c) = self.current {
            let results = &mut self.out[c].results;
            if label.starts_with("Result") {
                if !text.is_empty() {
                    results.push(text.into());
                }
                self.collecting = true;
                return;
            }
            if self.collecting && !text.is_empty() {
                results.push(text.into());
                return;
            }
            if !label.is_empty() && text.is_empty() && !results.is_empty() {
                // A grid printed one row per label.
                results.push(label.into());
                return;
            }
        }
        if label.is_empty() && text.is_empty() {
            self.collecting = false;
        }
    }
}

/// The worked examples of one help, and the named inputs they rely on, in order.
pub fn examples_of(body: &str) -> (Vec<Example>, Vec<(String, String)>) {
    let rows = help_rows(body);
    let Some(start) = rows
        .iter()
        .position(|(label, _)| label.starts_with("EXAMPLE"))
    else {
        return (vec![], vec![]);
    };
    let mut reader = ExampleReader::default();
    for (label, text) in &rows[start..] {
        match text.strip_prefix('=') {
            Some(formula) => reader.formula(label, formula),
            None => reader.other(label, text),
        }
    }
    (reader.out, reader.bindings)
}

pub fn library_names(src: &Path) -> Result<BTreeSet<String>> {
    let mut names = BTreeSet::new();
    for module in MODULES {
        for statement in formula::statements(&module_text(src, module)?) {
            if let Some(m) = captures(formula::NAME, &statement).first() {
                names.insert(m[1].clone());
            }
        }
    }
    Ok(names)
}

fn module_text(src: &Path, module: &str) -> Result<String> {
    let path = src.join(format!("{module}.txt"));
    py::read_text(&path).map_err(|e| match e {
        py::ReadError::Os(e) => {
            py::oserror_str(&e, &path.display().to_string(), py::OsStyle::Errno)
        }
        py::ReadError::Decode(e) => e.to_string(),
    })
}

/// Prefix each library call with the namespace, longest names first.
pub fn qualify(formula: &str, library: &BTreeSet<String>) -> String {
    let mut names = library.iter().collect::<Vec<_>>();
    names.sort_by_key(|n| std::cmp::Reverse(n.chars().count()));
    let mut out = formula.to_string();
    for bare in names {
        let pattern = format!(r"(?<![A-Za-z0-9_.λ]){}(?=\s*\()", fancy_regex::escape(bare));
        out = sub(&pattern, &out, &format!("{NAMESPACE}.{bare}"));
    }
    out
}

/// True when every name in the formula is a function, a constant or a bound input.
pub fn stands_alone(formula: &str, library: &BTreeSet<String>, bound: &BTreeSet<&str>) -> bool {
    if formula.contains('[') {
        return false;
    }
    let code = sub(r#""(?:[^"]|"")*""#, formula, r#""""#);
    let found = re(IDENT);
    for m in found.captures_iter(code.as_str()) {
        let m = m.expect("identifier match");
        let word = &m[1];
        let after = code[m.get(0).expect("match").end()..].trim_start_matches(py::is_space);
        let upper = word.to_uppercase();
        if word
            .strip_prefix("oz.")
            .is_some_and(|bare| library.contains(bare))
        {
            continue;
        }
        if after.starts_with('(') {
            if formula::listed(XLFN, &upper)
                || formula::listed(PLAIN, &upper)
                || library.contains(word)
            {
                continue;
            }
            return false;
        }
        if constant(&upper) || bound.contains(word) {
            continue;
        }
        return false;
    }
    true
}

/// A formula inside a double-quoted PowerShell string, λ written as `${L}`.
pub fn powershell(text: &str) -> String {
    text.replace('`', "``")
        .replace('"', "`\"")
        .replace('$', "`$")
        .replace('λ', "${L}")
}

/// An exact decimal: `mantissa × 10^exponent`, with the sign of a negative zero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decimal {
    mantissa: BigInt,
    exponent: i64,
    negative_zero: bool,
}

/// Python's default decimal context keeps 28 significant digits, half to even.
const PRECISION: u32 = 28;

impl Decimal {
    fn parse(digits: &str) -> Option<Self> {
        let (negative, body) = match digits.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, digits),
        };
        let (whole, fraction) = body.split_once('.').unwrap_or((body, ""));
        let text = format!("{whole}{fraction}")
            .chars()
            .map(|c| py::decimal_value(c).and_then(|d| char::from_digit(d, 10)))
            .collect::<Option<String>>()?;
        let mut mantissa = if text.is_empty() {
            BigInt::zero()
        } else {
            text.parse::<BigInt>().ok()?
        };
        if negative {
            mantissa = -mantissa;
        }
        Some(Self {
            negative_zero: negative && mantissa.is_zero(),
            mantissa,
            exponent: -(fraction.chars().count() as i64),
        })
    }

    fn rounded(mut self) -> Self {
        let digits = self.mantissa.abs().to_string().len() as u32;
        if digits > PRECISION {
            let drop = digits - PRECISION;
            let divisor = BigInt::from(10).pow(drop);
            let negative = self.mantissa.is_negative();
            let magnitude = self.mantissa.abs();
            let (mut quotient, remainder) = (&magnitude / &divisor, &magnitude % &divisor);
            let twice = remainder * 2;
            if twice > divisor || twice == divisor && (&quotient % 2u8) == BigInt::from(1) {
                quotient += 1;
            }
            self.mantissa = if negative { -quotient } else { quotient };
            self.exponent += i64::from(drop);
            if self.mantissa.abs().to_string().len() as u32 > PRECISION {
                self.mantissa /= 10;
                self.exponent += 1;
            }
        }
        self
    }

    fn scaleb(self, places: i64) -> Self {
        Self {
            exponent: self.exponent + places,
            ..self
        }
        .rounded()
    }

    fn add(&self, other: &Self) -> Self {
        let exponent = self.exponent.min(other.exponent);
        let scale = |d: &Self| &d.mantissa * BigInt::from(10).pow((d.exponent - exponent) as u32);
        Self {
            mantissa: scale(self) + scale(other),
            exponent,
            negative_zero: self.negative_zero && other.negative_zero,
        }
        .rounded()
    }

    fn zero() -> Self {
        Self {
            mantissa: BigInt::zero(),
            exponent: 0,
            negative_zero: false,
        }
    }

    /// `repr(float(value))`: the shortest float literal that reads back as the value.
    pub fn shown(&self) -> String {
        let value = format!("{}e{}", self.mantissa, self.exponent)
            .parse::<f64>()
            .unwrap_or(f64::NAN);
        py::float_repr(if self.negative_zero { -0.0 } else { value })
    }
}

/// (value, tolerance) for a printed number.
pub fn number(text: &str) -> Option<(Decimal, Decimal)> {
    let found = captures(NUMBER, py::strip(text));
    let digits = found.first()?[1].clone();
    if matches!(digits.as_str(), "" | "-" | "%" | "-%") {
        return None;
    }
    let percent = digits.ends_with('%');
    let digits = digits.trim_end_matches('%');
    let mut decimals = digits
        .split_once('.')
        .map_or(0, |(_, f)| f.chars().count() as i64);
    let mut value = Decimal::parse(digits)?;
    if percent {
        value = value.scaleb(-2);
        decimals += 2;
    }
    let five = Decimal {
        mantissa: BigInt::from(5),
        exponent: 0,
        negative_zero: false,
    };
    Some((value, five.scaleb(-decimals - 1)))
}

/// The checks a list of numbers supports: its count and its sum.
fn list_checks(tag: &str, f: &str, rows: &[&String]) -> Vec<String> {
    let parsed = rows
        .iter()
        .flat_map(|r| r.split(','))
        .map(number)
        .collect::<Vec<_>>();
    let items = parsed.iter().flatten().collect::<Vec<_>>();
    if items.len() == parsed.len() {
        let total = items.iter().fold(Decimal::zero(), |sum, (v, _)| sum.add(v));
        let tolerance = items.iter().fold(Decimal::zero(), |sum, (_, t)| sum.add(t));
        vec![
            format!("Near {tag} \"COUNT({f})\" '{}'", items.len()),
            format!(
                "Near {tag} \"SUM({f})\" '{}' '{}'",
                total.shown(),
                tolerance.shown()
            ),
        ]
    } else {
        vec![]
    }
}

/// The self-test lines that compare formula `f` with its printed results.
fn result_checks(tag: &str, f: &str, results: &[String]) -> Vec<String> {
    let mut lines = Vec::new();
    let joined = results.join(" ");
    let printed = py::strip(&joined);
    let rows = results
        .iter()
        .filter(|r| !py::strip(r).is_empty())
        .collect::<Vec<_>>();
    let single = if rows.len() == 1 {
        number(printed)
    } else {
        None
    };
    let shape = captures(SHAPE, printed);
    let date = captures(ISO_DATE, printed);
    let int = |s: &str| py::parse_int(s, 10).map_or_else(|_| s.to_string(), |n| n.to_string());
    if let Some(d) = date.first() {
        let serial = format!("DATE({},{},{})", int(&d[1]), int(&d[2]), int(&d[3]));
        lines.push(format!("Near {tag} \"{f} - {serial}\" '0' '0.5'"));
    } else if let Some((value, tolerance)) = single {
        lines.push(format!(
            "Near {tag} \"{f}\" '{}' '{}'",
            value.shown(),
            tolerance.shown()
        ));
    } else if constant(&printed.to_uppercase()) {
        let truth = u8::from(printed.to_uppercase() == "TRUE");
        lines.push(format!("Near {tag} \"N({f})\" '{truth}'"));
    } else if let Some(s) = shape.first() {
        lines.push(format!("Near {tag} \"ROWS({f})\" '{}'", s[1]));
        lines.push(format!("Near {tag} \"COLUMNS({f})\" '{}'", s[2]));
    } else if !rows.is_empty() && rows.iter().all(|r| matches(LIST, r) || number(r).is_some()) {
        lines.extend(list_checks(tag, f, &rows));
    } else if rows.len() == 1 && matches(WORD, printed) {
        lines.push(format!("Same {tag} \"{f}\" '{printed}'"));
    }
    if lines.is_empty() {
        lines.push(format!("Near {tag} \"SUMPRODUCT(--ISERROR({f}))\" '0'"));
    }
    lines
}

pub fn checks_for(
    name: &str,
    example: &Example,
    index: usize,
    library: &BTreeSet<String>,
    bindings: &[(String, String)],
) -> Vec<String> {
    let bound = bindings
        .iter()
        .map(|(k, _)| k.as_str())
        .collect::<BTreeSet<_>>();
    if !stands_alone(&example.formula, library, &bound) {
        return vec![];
    }
    if bindings
        .iter()
        .any(|(_, v)| !stands_alone(v, library, &BTreeSet::new()))
    {
        return vec![];
    }
    let mut formula = qualify(&example.formula, library);
    if !bindings.is_empty() {
        let parts = bindings
            .iter()
            .map(|(k, v)| format!("{k}, {}", qualify(v, library)))
            .collect::<Vec<_>>()
            .join(", ");
        formula = format!("LET({parts}, {formula})");
    }
    let tag = format!("'{}'", powershell(&format!("example: {name} #{index}")));
    let f = powershell(&formula);
    result_checks(&tag, &f, &example.results)
}

pub fn help_check(name: &str, body: &str) -> String {
    let call = powershell(&format!("{NAMESPACE}.{name}"));
    let tag = format!("'{}'", powershell(&format!("help: {name}")));
    if !body.starts_with("LAMBDA(") {
        return format!("Same {tag} \"INDEX({call},1,1)\" 'About:'");
    }
    if name.ends_with("DV") {
        // A validator answers TRUE, or a column of messages; either passes, an error does not.
        let first = format!("INDEX({call}(),1,1)");
        return format!("Near {tag} \"--OR({first}=TRUE,ISTEXT({first}))\" '1'");
    }
    format!("Same {tag} \"INDEX({call}(),1,1)\" 'FUNCTION:'")
}

pub fn generate(src: &Path) -> Result<String> {
    let library = library_names(src)?;
    let mut lines = vec![
        "# Generated by tools/generate_selftest_examples.py from the help in src/. Do not edit;"
            .to_string(),
        "# rerun the generator after a help change. Dot-sourced by tools/excel_selftest.ps1."
            .into(),
        String::new(),
        "# --- every function answers a call with no arguments with its help".into(),
    ];
    let mut example_lines = Vec::new();
    let mut counted = 0;
    for module in MODULES {
        for statement in formula::statements(&module_text(src, module)?) {
            let Some(m) = captures(formula::NAME, &statement).into_iter().next() else {
                continue;
            };
            let (name, body) = (&m[1], py::strip(&m[2]));
            lines.push(help_check(name, body));
            let (examples, bindings) = examples_of(body);
            for (index, example) in examples.iter().enumerate() {
                let found = checks_for(name, example, index + 1, &library, &bindings);
                if !found.is_empty() {
                    counted += 1;
                }
                example_lines.extend(found);
            }
        }
    }
    lines.push(String::new());
    lines.push(format!(
        "# --- {counted} worked examples that stand on their own, as the help prints them"
    ));
    lines.extend(example_lines);
    Ok(lines.join("\n") + "\n")
}

pub fn command(args: &[String]) -> Output {
    if args.len() > 2 {
        return Output::stderr("FAIL: usage: ozzit generate_selftest_examples [src dir] [output]");
    }
    let src = py::PyPath::new(args.first().map_or("src", String::as_str));
    let output = match args.get(1) {
        Some(path) => py::PyPath::new(path),
        None => py::PyPath::new(
            &py::resolve(&repository_root(OUTPUT))
                .join(OUTPUT)
                .display()
                .to_string(),
        ),
    };
    let text = match generate(&src.fs()) {
        Ok(text) => text,
        Err(e) => return Output::failure(format!("FAIL: {e}")),
    };
    if !text.is_ascii() {
        return Output::stderr("FAIL: the generated fragment is not pure ASCII");
    }
    let path = output.fs();
    if path.is_file() {
        let before = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) => return Output::failure(format!("FAIL: {}", io_error(&e, &path))),
        };
        if !before.is_ascii() {
            return Output::failure(format!("FAIL: {} is not ASCII", output));
        }
        let before = py::universal_newlines(&String::from_utf8_lossy(&before));
        if before == text {
            return Output::ok(vec![format!("already current: {output}")]);
        }
    }
    if let Err(e) = write_bytes(&path, text.as_bytes()) {
        return Output::failure(format!("FAIL: {e}"));
    }
    Output::ok(vec![format!(
        "wrote {output}: {} lines",
        text.matches('\n').count()
    )])
}
