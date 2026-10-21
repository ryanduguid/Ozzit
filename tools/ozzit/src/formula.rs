//! Typed Excel Labs sources and the stored OOXML form.
use crate::{common::*, py};
use std::collections::{BTreeSet, HashSet};
pub const MODULES: [&str; 6] = [
    "Dates",
    "Essentials",
    "Financial",
    "Ratios",
    "Utilities",
    "Debt",
];
pub const NAME: &str = r"(?s)^\s*([A-Za-z_][A-Za-z0-9_.λ]*)\s*=\s*(.+)$";
pub const NAME_RE: &str = r#"(?s)<definedName name="(oz\.[^"]+)"([^>]*)>(.*?)</definedName>"#;
pub const XLFN: &str = "ANCHORARRAY ARRAYTOTEXT BYCOL BYROW CHOOSECOLS CHOOSEROWS CONCAT DAYS DROP EXPAND FORMULATEXT HSTACK IFNA IFS ISOMITTED ISOWEEKNUM LAMBDA LET MAKEARRAY MAP MAXIFS MINIFS NUMBERVALUE RANDARRAY REDUCE SCAN SEQUENCE SINGLE SORTBY SWITCH TAKE TEXTAFTER TEXTBEFORE TEXTJOIN TEXTSPLIT TOCOL TOROW UNIQUE VALUETOTEXT VSTACK WRAPCOLS WRAPROWS XLOOKUP XMATCH XOR SORT FILTER";
pub const PLAIN: &str = "ABS AND AVERAGE AVERAGEIF CEILING CELL CHOOSE COLUMN COLUMNS COUNT COUNTA COUNTBLANK COUNTIF COUNTIFS CUMIPMT CUMPRINC DATE DATEDIF DATEVALUE DAY DB DDB EDATE EOMONTH EXACT EXP FIND FLOOR FV HLOOKUP IF IFERROR INDEX INDIRECT INT IPMT IRR ISBLANK ISERR ISERROR ISLOGICAL ISNA ISNONTEXT ISNUMBER ISTEXT LARGE LEFT LEN LN LOG LOOKUP LOWER MATCH MAX MEDIAN MID MIN MMULT MOD MONTH MROUND N NA NETWORKDAYS NOT NOW NPER NPV OFFSET OR PMT POWER PPMT PRODUCT PROPER PV QUOTIENT RATE REPLACE REPT RIGHT ROUND ROUNDDOWN ROUNDUP ROW ROWS SEARCH SIGN SLN SMALL SQRT SUBSTITUTE SUM SUMIF SUMIFS SUMPRODUCT SYD TEXT TIME TODAY TRANSPOSE TRIM TRUNC UPPER VALUE VDB VLOOKUP WEEKDAY WEEKNUM WORKDAY XIRR XNPV YEAR YEARFRAC";
pub const ERRORS: [&str; 9] = [
    "#DIV/0!", "#N/A", "#NAME?", "#NULL!", "#NUM!", "#REF!", "#VALUE!", "#CALC!", "#SPILL!",
];
pub const KNOWN: [(&str, &str); 5] = [
    ("CorkscrewλDV", "Opening"),
    ("DayCountRateλ", "Convention"),
    ("LeaseLiabilityλ", "InAdvance"),
    ("LeaseRemeasureλ", "InAdvance"),
    ("LeaseScheduleλ", "InAdvance"),
];
const SCALAR: &str = "AND AREAS AVERAGE COLUMNS COUNT COUNTA COUNTBLANK ISOMITTED ISREF MAX MIN OR ROWS SUM SUMPRODUCT XOR BYCOL BYROW LAMBDA MAKEARRAY MAP REDUCE SCAN";
const SHAPE: &str = "AREAS COLUMNS ISOMITTED ISREF ROWS";
pub fn listed(words: &str, word: &str) -> bool {
    words.split_whitespace().any(|w| w == word)
}

/// Byte offsets are used internally; quoted text is copied without normalisation.
pub fn read_string(text: &str, start: usize) -> (&str, usize) {
    let bytes = text.as_bytes();
    let mut i = start + 1;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            i += 1;
            if i < bytes.len() && bytes[i] == b'"' {
                i += 1;
                continue;
            }
            return (&text[start..i], i);
        }
        i += 1;
    }
    (&text[start..], text.len())
}
pub fn strip_comments(text: &str) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        if rest.starts_with('"') {
            let (s, end) = read_string(text, i);
            out.push_str(s);
            i = end;
        } else if rest.starts_with("//") {
            i += rest.find('\n').unwrap_or(rest.len());
        } else if let Some(body) = rest.strip_prefix("/*") {
            // As Python: the terminator is searched for after the opening "/*".
            i += body.find("*/").map_or(rest.len(), |n| n + 4);
        } else {
            let c = rest.chars().next().expect("nonempty");
            out.push(c);
            i += c.len_utf8();
        }
    }
    out
}
pub fn statements(text: &str) -> Vec<String> {
    let text = strip_comments(text);
    let mut out = Vec::new();
    let mut i = 0;
    let mut start = 0;
    let mut depth = 0i64;
    while i < text.len() {
        let c = text[i..].chars().next().expect("nonempty");
        if c == '"' {
            i = read_string(&text, i).1;
            continue;
        }
        if "({[".contains(c) {
            depth += 1;
        } else if ")}]".contains(c) {
            depth -= 1;
        } else if c == ';' && depth == 0 {
            out.push(text[start..i].into());
            start = i + 1;
        }
        i += c.len_utf8();
    }
    if !py::strip(&text[start..]).is_empty() {
        out.push(text[start..].into());
    }
    out
}
pub fn split_literals(text: &str) -> Vec<(bool, &str)> {
    let mut out = Vec::new();
    let mut i = 0;
    let mut start = 0;
    while i < text.len() {
        if text.as_bytes()[i] == b'"' {
            if start < i {
                out.push((false, &text[start..i]));
            }
            let (lit, end) = read_string(text, i);
            out.push((true, lit));
            i = end;
            start = i;
        } else {
            i += 1;
        }
    }
    if start < i {
        out.push((false, &text[start..]));
    }
    out
}
pub fn arguments(text: &str, start: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = start;
    let mut begin = start;
    let mut depth = 0i64;
    while i < text.len() {
        let c = text[i..].chars().next().expect("nonempty");
        if c == '"' {
            i = read_string(text, i).1;
            continue;
        }
        if "({[".contains(c) {
            depth += 1;
        } else if ")}]".contains(c) {
            if depth == 0 {
                break;
            }
            depth -= 1;
        } else if c == ',' && depth == 0 {
            out.push(text[begin..i].into());
            begin = i + 1;
        }
        i += c.len_utf8();
    }
    out.push(text[begin..i].into());
    out
}
pub fn canonical(formula: &str) -> String {
    let mut parts = String::new();
    let mut literals = Vec::new();
    for (literal, chunk) in split_literals(formula) {
        if literal {
            parts.push_str(&format!("\0{}\0", literals.len()));
            literals.push(chunk);
        } else {
            let chunk = sub_fn(r"_xlop\.([A-Za-z_][A-Za-z0-9_]*\??)", chunk, |m| {
                format!("[{}]", &m[1])
            });
            let chunk = sub(r"(?i)_xl[a-z]+\.", &chunk, "").replace("[0]!", "");
            parts.extend(
                chunk
                    .chars()
                    .filter(|&c| !py::is_space(c))
                    .flat_map(char::to_uppercase),
            );
        }
    }
    while let Some(at) = parts.find("SINGLE(") {
        let mut depth = 1;
        let mut end = at + 7;
        while end < parts.len() {
            match parts.as_bytes()[end] {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            end += 1;
        }
        if end == parts.len() {
            break;
        }
        parts = format!(
            "{}@{}{}",
            &parts[..at],
            &parts[at + 7..end],
            &parts[end + 1..]
        );
    }
    sub_fn(r"\x00([0-9]+)\x00", &parts, |m| {
        literals[m[1].parse::<usize>().expect("literal index")].into()
    })
}
pub fn tight(formula: &str) -> String {
    split_literals(formula)
        .into_iter()
        .map(|(literal, s)| {
            if literal {
                s.into()
            } else {
                s.chars()
                    .filter(|&c| !py::is_space(c))
                    .collect::<String>()
                    .replace("[0]!", "")
            }
        })
        .collect()
}
pub fn qualify(formula: &str, module: &str, names: &BTreeSet<String>) -> String {
    if names.is_empty() {
        return formula.into();
    }
    let mut names = names.iter().collect::<Vec<_>>();
    names.sort_by_key(|s| std::cmp::Reverse(s.chars().count()));
    let alternatives = names
        .iter()
        .map(|s| fancy_regex::escape(s))
        .collect::<Vec<_>>()
        .join("|");
    let pattern = format!(r"(?<![A-Za-z0-9_.!])(?:{alternatives})(?=\s*\()");
    split_literals(formula)
        .into_iter()
        .map(|(literal, s)| {
            if literal {
                s.into()
            } else {
                sub_fn(&pattern, s, |m| format!("{module}.{}", &m[0]))
            }
        })
        .collect()
}
pub fn dead_conversions(body: &str) -> BTreeSet<String> {
    let code = split_literals(body)
        .into_iter()
        .filter(|(lit, _)| !*lit)
        .map(|(_, s)| s)
        .collect::<String>();
    let seen = captures(r"\bCvt\w*\b", &code)
        .into_iter()
        .map(|m| m[0].clone())
        .collect::<Vec<_>>();
    seen.iter()
        .filter(|s| seen.iter().filter(|v| v == s).count() == 1)
        .cloned()
        .collect()
}
/// verify_sources' `call_arguments`: the top-level arguments of the call whose "("
/// ends at `start`. Only parentheses and braces nest (an unmatched "]" is text), and
/// a call left open loses its last argument. String literals must already be empty.
fn call_arguments(code: &str, start: usize) -> Vec<String> {
    let (mut out, mut depth, mut begin) = (Vec::new(), 1usize, start);
    for (i, c) in code[start..].char_indices() {
        let i = start + i;
        match c {
            '(' | '{' => depth += 1,
            ')' | '}' => {
                depth -= 1;
                if depth == 0 {
                    out.push(code[begin..i].into());
                    break;
                }
            }
            ',' if depth == 1 => {
                out.push(code[begin..i].into());
                begin = i + 1;
            }
            _ => {}
        }
    }
    out
}
fn reads_any(code: &str, names: &HashSet<String>, stop: &str) -> bool {
    let token = re(r"([A-Za-z_][A-Za-z0-9_.?λ]*)\s*\(|([A-Za-z_][A-Za-z0-9_.?λ]*)|([({])|([)}])");
    let mut calls = Vec::new();
    let mut resume = 0;
    for m in token.captures_iter(code) {
        let m = m.expect("token matching");
        let whole = m.get(0).expect("token");
        if whole.start() < resume {
            continue;
        }
        let call = m
            .get(1)
            .map_or(String::new(), |m| m.as_str().to_uppercase());
        let stopped = calls.iter().any(|c: StringRef<'_>| listed(stop, c));
        if call == "LET" && !stopped {
            let args = call_arguments(code, whole.end());
            if args.len() >= 3 && args.len() % 2 == 1 {
                let mut live = names.clone();
                for pair in args[..args.len() - 1].chunks(2) {
                    let bound = py::strip(&pair[0]).to_uppercase();
                    if reads_any(&pair[1], &live, stop) {
                        live.insert(bound);
                    } else {
                        live.remove(&bound);
                    }
                }
                if reads_any(args.last().expect("LET result"), &live, stop) {
                    return true;
                }
                resume = whole.end() + args.join(",").len() + 1;
                continue;
            }
        }
        if !call.is_empty() {
            calls.push(call);
        } else if let Some(open) = m.get(3) {
            calls.push(open.as_str().into());
        } else if m.get(4).is_some() {
            calls.pop();
        } else if m
            .get(2)
            .is_some_and(|m| names.contains(&m.as_str().to_uppercase()))
            && !stopped
        {
            return true;
        }
    }
    false
}
type StringRef<'a> = &'a String;
pub fn collapsed_defaults(body: &str) -> BTreeSet<String> {
    let code = split_literals(body)
        .into_iter()
        .map(|(lit, s)| if lit { "\"\"" } else { s })
        .collect::<String>();
    let head = re(r"(?i)^\s*LAMBDA\s*\(");
    let Some(head) = head.find(&code).expect("LAMBDA matching") else {
        return BTreeSet::new();
    };
    let params = call_arguments(&code, head.end());
    let mut found = BTreeSet::new();
    let params = params[..params.len().saturating_sub(1)]
        .iter()
        .map(|p| sub(r"[\[\]\s]", p, ""))
        .collect::<Vec<_>>();
    for m in re(r"(?i)(?<![A-Za-z0-9_.?λ])IF\s*\(").find_iter(&code) {
        let args = call_arguments(&code, m.expect("IF matching").end());
        if args.len() < 2 {
            continue;
        }
        let combined = re(r"(?i)^\s*(?:NOT\s*\(\s*)?(?:AND|OR)\s*\(");
        let Some(test) = combined.find(&args[0]).expect("combined matching") else {
            continue;
        };
        let tested = call_arguments(&args[0], test.end());
        for name in &params {
            let names = HashSet::from([name.to_uppercase()]);
            if tested.iter().any(|a| reads_any(a, &names, SHAPE))
                && args[1..].iter().any(|a| reads_any(a, &names, SCALAR))
            {
                found.insert(name.clone());
            }
        }
    }
    found
}

pub fn header_comments(text: &str) -> std::collections::BTreeMap<String, String> {
    captures(
        r"(?s)/\*\s*FUNCTION NAME:\s*([^\n]+?)\s*\n\s*DESCRIPTION:\*/\s*/\*\*(.*?)\*/",
        text,
    )
    .into_iter()
    .map(|m| {
        (
            py::strip(&m[1]).into(),
            prefix(py::strip(&m[2].replace("\r\n", "\n")), 255).replace('\n', "_x000a_"),
        )
    })
    .collect()
}
pub fn local_names(code: &str) -> Vec<String> {
    let code = split_literals(code)
        .into_iter()
        .map(|(lit, s)| if lit { "\"\"" } else { s })
        .collect::<String>();
    let mut out = Vec::new();
    for call in ["LAMBDA", "LET"] {
        for m in re(&format!(r"(?i)(?<![A-Za-z0-9_.λ]){call}\s*\(")).find_iter(&code) {
            let args = arguments(&code, m.expect("binding matching").end());
            for a in args[..args.len().saturating_sub(1)]
                .iter()
                .step_by(if call == "LET" { 2 } else { 1 })
            {
                // Python strips brackets from LAMBDA parameters only, not LET bindings.
                let bare = match call {
                    "LAMBDA" => py::strip(py::strip(a).trim_matches(['[', ']'])),
                    _ => py::strip(a),
                };
                if matches(r"^[A-Za-z_][A-Za-z0-9_λ]*\??$", bare) && !out.iter().any(|s| s == bare)
                {
                    out.push(bare.into());
                }
            }
        }
    }
    out
}
/// The position after the parenthesis that closes the one at `open`, or the
/// end of `code`, skipping string literals.
fn after_call(code: &str, open: usize) -> usize {
    let mut depth = 1;
    let mut end = open + 1;
    while end < code.len() {
        if code.as_bytes()[end] == b'"' {
            end = read_string(code, end).1;
            continue;
        }
        match code.as_bytes()[end] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return end + 1;
                }
            }
            _ => {}
        }
        end += 1;
    }
    end
}
fn unwrap_at(code: &str) -> Result<String> {
    let mut out = String::new();
    let mut i = 0;
    while i < code.len() {
        let rest = &code[i..];
        let c = rest.chars().next().expect("code");
        if c == '"' {
            let (s, end) = read_string(code, i);
            out.push_str(s);
            i = end;
            continue;
        }
        if c == '@' {
            let j =
                i + 1 + code[i + 1..].len() - code[i + 1..].trim_start_matches(py::is_space).len();
            let ident = re(r"^[A-Za-z_][A-Za-z0-9_.λ]*\??");
            let Some(m) = ident.find(&code[j..]).expect("name matching") else {
                return Err(format!(
                    "@ is not followed by a name: {}",
                    py::repr_str(prefix(&code[i..], 20))
                ));
            };
            let end = j + m.end();
            let k = end + code[end..].len() - code[end..].trim_start_matches(py::is_space).len();
            let end = if code[k..].starts_with('(') {
                after_call(code, k)
            } else {
                end
            };
            out.push_str(&format!("SINGLE({})", &code[j..end]));
            i = end;
            continue;
        }
        out.push(c);
        i += c.len_utf8();
    }
    Ok(out)
}
/// How the stored form marks one name: a library, local, Excel or plain name.
fn mark_name(
    word: &str,
    called: bool,
    library: &BTreeSet<String>,
    locals: &std::collections::HashMap<String, String>,
    unknown: &mut BTreeSet<String>,
) -> String {
    let upper = word.to_uppercase();
    if word
        .strip_prefix("oz.")
        .is_some_and(|n| library.contains(n))
    {
        return word.into();
    }
    if let Some(local) = locals.get(&word.to_lowercase())
        && !(called && (listed(XLFN, &upper) || listed(PLAIN, &upper)))
    {
        return format!("\0PM\0{local}");
    }
    if called {
        if upper == "SORT" || upper == "FILTER" {
            return format!("\0FN\0\0WS\0{upper}");
        }
        if listed(XLFN, &upper) {
            return format!("\0FN\0{upper}");
        }
        if listed(PLAIN, &upper) {
            return upper;
        }
        if library.contains(word) {
            return format!("\0LIB\0{word}");
        }
    }
    if upper == "TRUE" || upper == "FALSE" {
        return upper;
    }
    unknown.insert(word.to_string());
    word.into()
}
/// One stretch of code outside string literals, in its stored form.
fn render_chunk(
    chunk: &str,
    optional: &BTreeSet<String>,
    library: &BTreeSet<String>,
    locals: &std::collections::HashMap<String, String>,
    unknown: &mut BTreeSet<String>,
) -> String {
    let mut chunk = chunk.to_string();
    for param in optional {
        chunk = sub(
            &format!(r"\[\s*{}\s*\]", fancy_regex::escape(param)),
            &chunk,
            &format!("\0OP\0{param}"),
        );
    }
    for (i, error) in ERRORS.iter().enumerate() {
        chunk = sub(
            &format!("(?i){}", fancy_regex::escape(error)),
            &chunk,
            &format!("\0E{i}\0"),
        );
    }
    let before = chunk.clone();
    chunk = sub_fn(
        r"(?<![A-Za-z0-9_.λ\x00])([A-Za-z_][A-Za-z0-9_.λ]*\??)",
        &before,
        |m| {
            let called = py::strip(&before[m.get(0).expect("name").end()..]).starts_with('(');
            mark_name(&m[1], called, library, locals, unknown)
        },
    );
    for (i, error) in ERRORS.iter().enumerate() {
        chunk = chunk.replace(&format!("\0E{i}\0"), error);
    }
    sub(r"\s+", &chunk, " ")
}
pub fn render(body: &str, library: &BTreeSet<String>) -> Result<String> {
    let code = unwrap_at(&strip_comments(body))?;
    let head = code.find("LAMBDA(").ok_or("substring not found")? + 7;
    let args = arguments(&code, head);
    let optional = args[..args.len().saturating_sub(1)]
        .iter()
        .filter(|a| py::strip(a).starts_with('['))
        .map(|a| py::strip(py::inner(py::strip(a))).to_string())
        .collect::<BTreeSet<_>>();
    let locals = local_names(&code)
        .into_iter()
        .map(|n| (n.to_lowercase(), n))
        .collect::<std::collections::HashMap<_, _>>();
    let mut unknown = BTreeSet::new();
    let mut pieces = String::new();
    for (literal, chunk) in split_literals(&code) {
        if literal {
            pieces.push_str(chunk);
            continue;
        }
        pieces.push_str(&render_chunk(
            chunk,
            &optional,
            library,
            &locals,
            &mut unknown,
        ));
    }
    if !unknown.is_empty() {
        return Err(format!(
            "{} reached the stored form without a marker; Excel will not read the definition",
            unknown.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }
    for (old, new) in [
        ("OP", "_xlop."),
        ("FN", "_xlfn."),
        ("WS", "_xlws."),
        ("PM", "_xlpm."),
        ("LIB", "oz."),
    ] {
        pieces = pieces.replace(&format!("\0{old}\0"), new);
    }
    let stored = py::strip(&pieces);
    Ok(if stored.chars().count() > 8192 {
        tight(stored)
    } else {
        stored.into()
    })
}

#[cfg(test)]
mod compiler_retirement_tests {
    use super::unwrap_at;

    #[test]
    fn implicit_intersection_keeps_spacing_and_an_unclosed_call() {
        for (input, expected) in [
            ("@ First + 1", "SINGLE(First) + 1"),
            ("@INDEX (x, 1)", "SINGLE(INDEX (x, 1))"),
            ("@INDEX(x", "SINGLE(INDEX(x)"),
        ] {
            assert_eq!(unwrap_at(input).unwrap(), expected);
        }
    }

    #[test]
    fn implicit_intersection_without_a_name_is_refused_exactly() {
        assert_eq!(
            unwrap_at("@1 + x").unwrap_err(),
            "@ is not followed by a name: '@1 + x'"
        );
    }
}

#[cfg(test)]
mod source_contract_tests {
    use super::*;

    #[test]
    fn an_unclosed_call_retains_only_the_arguments_seen_before_its_final_segment() {
        assert_eq!(call_arguments("f(a,b", 2), ["a"]);
    }

    #[test]
    fn a_let_without_a_result_is_read_as_an_ordinary_call() {
        let names = ["X".to_owned()].into();
        assert!(reads_any("LET(x, 1)", &names, ""));
        assert!(!reads_any("LET(x, 1)", &names, "LET"));
    }

    #[test]
    fn a_stray_closing_bracket_does_not_hide_a_read() {
        assert!(reads_any(") x", &["X".to_owned()].into(), "SUM"));
    }
}
