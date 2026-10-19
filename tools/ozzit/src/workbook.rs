//! Structural workbook checks, without evaluating formulas or altering evidence.
use crate::{afe, archive::*, common::*, py, pyjson};
use num_bigint::BigInt;
use roxmltree::{Document, Node};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

/// Python's BANNED with `\bBXL\b` as a plain `BXL`, so the fast engine scans it;
/// [`banned_tokens`] checks that alternative's word boundaries by hand.
const BANNED: &str = r"BX[DEFRLU]\.|BXLDebt|BXL|ozzit\.[a-z]+\.|beyondexcel|Eloquens|dropbox|Leonardo|Starter Pack|Calibri|MACRS|Modified Accelerated|US GAAP|IRS Depreciation|x15ac:absPath|[A-Za-z]:\\{1,2}Users\\{1,2}|/Users/|/home/";
const TOKEN: &str = r"oz\.[A-Za-z0-9_]+λ?(?:DV)?";
const WEB: &str = "xl/webextensions/";
const PANES: &str = "xl/webextensions/taskpanes.xml";
const PANE_RELS: &str = "xl/webextensions/_rels/taskpanes.xml.rels";
const FORBIDDEN: [&str; 6] = [
    "xl/activeX/",
    "xl/ctrlProps/",
    "xl/externalLinks/",
    "xl/embeddings/",
    "xl/macrosheets/",
    "xl/dialogsheets/",
];

/// `BANNED.finditer(text)`, leftmost first. A bare `BXL` next to a word
/// character is not a match, and the scan resumes one character later, as
/// Python's engine tries the next start position.
pub fn banned_tokens(text: &str) -> Vec<String> {
    let pattern = re(BANNED);
    let word = |c: Option<char>| c.is_some_and(|c| matches(r"^\w$", c.encode_utf8(&mut [0; 4])));
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(m) = pattern
        .find_from_pos(text, pos)
        .expect("regular expression execution")
    {
        let bare = m.as_str() == "BXL";
        if bare
            && (word(text[..m.start()].chars().next_back()) || word(text[m.end()..].chars().next()))
        {
            pos = m.start() + 1;
            continue;
        }
        out.push(m.as_str().to_string());
        pos = m.end();
    }
    out
}
pub fn xml_part<'a>(name: &str, bytes: &'a [u8]) -> Result<Document<'a>> {
    if bytes
        .windows(9)
        .any(|b| b.eq_ignore_ascii_case(b"<!DOCTYPE"))
    {
        return Err(format!("DOCTYPE declaration in {name}"));
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| format!("malformed XML in {name}: {}", decode(bytes).unwrap_err()))?;
    Document::parse(text).map_err(|e| format!("malformed XML in {name}: {e}"))
}
fn elements<'a, 'i>(docs: &'a BTreeMap<&str, Document<'i>>, name: &str) -> Vec<Node<'a, 'i>> {
    docs.get(name)
        .into_iter()
        .flat_map(|doc| doc.descendants().filter(Node::is_element))
        .collect()
}
fn xml_text<'a>(docs: &BTreeMap<&str, Document<'a>>, name: &str) -> &'a str {
    docs.get(name).map_or("", Document::input_text)
}
fn web_extensions(
    names: &[String],
    docs: &BTreeMap<&str, Document<'_>>,
    failures: &mut Vec<String>,
) {
    let mut extensions = names
        .iter()
        .filter(|n| n.starts_with(WEB) && n.as_str() != PANES && n.as_str() != PANE_RELS)
        .cloned()
        .collect::<Vec<_>>();
    extensions.sort();
    if extensions.len() != 1 {
        failures.push(format!(
            "expected one web extension, the Advanced Formula Environment, found {}",
            extensions.len()
        ));
    }
    let mut targets = elements(docs, PANE_RELS)
        .into_iter()
        .filter(|e| e.tag_name().name() == "Relationship")
        .map(|e| {
            let target = e.attribute("Target").unwrap_or("");
            if target.starts_with('/') {
                target.trim_start_matches('/').into()
            } else {
                resolve_target(PANES, target)
            }
        })
        .collect::<Vec<String>>();
    targets.sort();
    if targets != extensions {
        failures.push("the task pane does not resolve to the one web extension part".into());
    }
    extension_references(&extensions, docs, failures);
    let tasks = elements(docs, PANES)
        .into_iter()
        .filter(|e| e.tag_name().name() == "taskpane")
        .collect::<Vec<_>>();
    if tasks.len() != 1 || tasks[0].attribute("visibility") != Some("0") {
        failures.push("expected one hidden task pane for the Advanced Formula Environment".into());
    }
    custom_function_declarations(&extensions, docs, failures);
}
fn extension_references(
    extensions: &[String],
    docs: &BTreeMap<&str, Document<'_>>,
    failures: &mut Vec<String>,
) {
    for name in extensions {
        let refs = elements(docs, name)
            .into_iter()
            .filter(|e| e.tag_name().name() == "reference")
            .collect::<Vec<_>>();
        if refs.is_empty()
            || refs.iter().any(|e| {
                e.attribute("id") != Some("wa200003696") || e.attribute("storeType") != Some("OMEX")
            })
        {
            failures.push(format!(
                "{name} references an add-in other than the Advanced Formula Environment"
            ));
        }
    }
}
fn custom_function_declarations(
    extensions: &[String],
    docs: &BTreeMap<&str, Document<'_>>,
    failures: &mut Vec<String>,
) {
    for name in extensions
        .iter()
        .map(String::as_str)
        .chain(std::iter::once(PANES))
    {
        for e in elements(docs, name) {
            if e.tag_name().name() == "containsCustomFunctions"
                || e.attributes()
                    .any(|a| a.name() == "containsCustomFunctions")
                || e.attribute("name") == Some("containsCustomFunctions")
            {
                failures.push(format!("containsCustomFunctions declared in {name}"));
            }
        }
    }
}
fn tokens(text: &str) -> BTreeSet<String> {
    captures(TOKEN, text)
        .into_iter()
        .map(|m| m[0].clone())
        .collect()
}
pub struct Report {
    pub failures: Vec<String>,
    pub functions: usize,
}
fn forbidden_parts(names: &[String], failures: &mut Vec<String>) {
    for name in names {
        if name == "xl/vbaProject.bin" || FORBIDDEN.iter().any(|prefix| name.starts_with(prefix)) {
            failures.push(format!("forbidden workbook part {name}"));
        }
    }
}
fn parse_documents<'a>(
    parts: &'a Parts,
    names: &'a [String],
    failures: &mut Vec<String>,
) -> BTreeMap<&'a str, Document<'a>> {
    let mut docs = BTreeMap::new();
    for name in names {
        if name.ends_with(".xml") || name.ends_with(".rels") {
            match xml_part(name, &parts[name]) {
                Ok(doc) => {
                    docs.insert(name.as_str(), doc);
                }
                Err(e) => failures.push(e),
            }
        }
    }
    docs
}
/// Banned tokens in every XML part except the AFE store, and in the binary parts.
fn banned_in_parts(
    parts: &Parts,
    names: &[String],
    docs: &BTreeMap<&str, Document<'_>>,
    failures: &mut Vec<String>,
) {
    for name in names {
        let text = if (name.ends_with(".xml") || name.ends_with(".rels"))
            && name != "customXml/item1.xml"
        {
            xml_text(docs, name).to_owned()
        } else if name.ends_with(".bin") {
            // Python's errors="ignore" drops incomplete and unpaired units.
            char::decode_utf16(
                parts[name]
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|b| u16::from_le_bytes([b[0], b[1]])),
            )
            .filter_map(std::result::Result::ok)
            .collect()
        } else {
            continue;
        };
        for token in banned_tokens(&text) {
            failures.push(format!("banned token {} in {name}", py::repr_str(&token)));
        }
    }
}
/// Banned tokens in the decoded AFE store; false when the store cannot be read.
fn banned_in_store(docs: &BTreeMap<&str, Document<'_>>, failures: &mut Vec<String>) -> bool {
    let blobs = captures(
        r">([A-Za-z0-9+/=]{100,})<",
        xml_text(docs, "customXml/item1.xml"),
    );
    let Some(blob) = blobs.first() else {
        failures.push("AFE project store missing".into());
        return false;
    };
    let store = match afe::decode_strict_blob(blob[1].as_bytes()) {
        Ok(store) => store,
        Err(e) => {
            failures.push(format!("invalid AFE project store: {e}"));
            return false;
        }
    };
    // The banned patterns do not depend on JSON separator whitespace.
    for token in banned_tokens(&pyjson::dumps_compact(&store)) {
        failures.push(format!(
            "banned token {} in the AFE project store",
            py::repr_str(&token)
        ));
    }
    true
}
/// The defined names and sheets, and the checks on each defined name's text.
struct Names {
    defined: BTreeSet<String>,
    sheets: Vec<String>,
    known: BTreeSet<String>,
}
fn defined_names(book: &str, failures: &mut Vec<String>) -> Names {
    let defined = captures(
        r#"<definedName name="([^"]+)"[^>]*>([^<]*)</definedName>"#,
        book,
    );
    let names = defined
        .iter()
        .map(|m| m[1].clone())
        .collect::<BTreeSet<_>>();
    let sheets = captures(r#"<sheet name="([^"]+)""#, book)
        .into_iter()
        .map(|m| m[1].clone())
        .collect::<Vec<_>>();
    let known = names
        .union(&sheets.iter().cloned().collect())
        .cloned()
        .collect::<BTreeSet<_>>();
    if defined.is_empty() {
        failures.push("no defined names found".into());
    }
    for m in defined {
        check_defined_name(&m[1], &unescape(&m[2]), &known, failures);
    }
    Names {
        defined: names,
        sheets,
        known,
    }
}
fn check_defined_name(
    name: &str,
    source: &str,
    known: &BTreeSet<String>,
    failures: &mut Vec<String>,
) {
    if source.chars().count() > 8192 {
        failures.push(format!(
            "{name} is {} characters; Excel refuses a defined name over 8192",
            source.chars().count()
        ));
    }
    if !source.matches('"').count().is_multiple_of(2) {
        failures.push(format!("unbalanced quotes in {name}"));
    }
    let code = sub(r#""(?:[^"]|"")*""#, source, "\"\"");
    if code.matches('(').count() != code.matches(')').count() {
        failures.push(format!("unbalanced parentheses in {name}"));
    }
    for token in tokens(source).difference(known) {
        failures.push(format!("{name} references undefined {token}"));
    }
}
fn sheet_formulas(
    names: &[String],
    docs: &BTreeMap<&str, Document<'_>>,
    known: &BTreeSet<String>,
    failures: &mut Vec<String>,
) {
    for name in names
        .iter()
        .filter(|n| matches(r"^xl/worksheets/sheet\d+\.xml$", n))
    {
        let text = xml_text(docs, name);
        if text.contains("#REF!") {
            failures.push(format!("#REF! present in {name}"));
        }
        for formula in captures(r"<f[^>]*>([^<]*)</f>", text) {
            for token in tokens(&formula[1]).difference(known) {
                failures.push(format!("{name} uses undefined {token}"));
            }
        }
    }
}
const VOLATILE: [&str; 7] = [
    "RANDBETWEEN(",
    "RANDARRAY(",
    "RAND()",
    "NOW()",
    "TODAY()",
    "OFFSET(",
    "INDIRECT(",
];
fn volatile_formulas(
    names: &[String],
    docs: &BTreeMap<&str, Document<'_>>,
    failures: &mut Vec<String>,
) {
    for name in names
        .iter()
        .filter(|n| matches(r"^xl/worksheets/sheet\d+\.xml$", n) || n.starts_with("xl/tables/"))
    {
        let text = xml_text(docs, name);
        let formulas = captures(r"(?s)<f[^>]*>(.*?)</f>", text)
            .into_iter()
            .chain(captures(
                r"(?s)<calculatedColumnFormula[^>]*>(.*?)</calculatedColumnFormula>",
                text,
            ));
        for formula in formulas {
            for volatile in VOLATILE {
                if formula[1].contains(volatile) {
                    failures.push(format!("volatile {volatile} in a formula in {name}"));
                }
            }
        }
        // A lazy match ends at the first </c>, exactly where Python's tempered loop stopped.
        for cell in captures(r#"(?s)<c r="[A-Z]+\d+"[^>]*>.*?</c>"#, text) {
            if (cell[0].contains("ca=\"1\"") || cell[0].contains("aca=\"1\""))
                && !cell[0].contains("CELL(")
            {
                failures.push(format!(
                    "always-calculate flag on a non-volatile cell in {name}"
                ));
                break;
            }
        }
    }
}
fn shared_string_count(docs: &BTreeMap<&str, Document<'_>>, failures: &mut Vec<String>) {
    let shared = docs.get("xl/sharedStrings.xml").map(Document::root_element);
    let unique_count = shared.and_then(|s| s.attribute("uniqueCount"));
    let declared = match unique_count {
        None => {
            failures.push("sharedStrings uniqueCount is missing".into());
            BigInt::from(-1)
        }
        Some(s) => match py::parse_int(s, 10) {
            Ok(n) => n,
            Err(_) => {
                failures.push(format!(
                    "sharedStrings uniqueCount is not an integer: {}",
                    py::repr_str(s)
                ));
                BigInt::from(-1)
            }
        },
    };
    let entries = shared.map_or(0, |s| s.children().filter(Node::is_element).count());
    if declared != BigInt::from(entries) {
        failures.push(format!(
            "sharedStrings uniqueCount {declared} != {entries} entries"
        ));
    }
}
fn sheet_titles(
    docs: &BTreeMap<&str, Document<'_>>,
    sheets: &[String],
    failures: &mut Vec<String>,
) {
    if !docs.contains_key("docProps/app.xml") {
        return;
    }
    let titles = captures(
        r"(?s)<TitlesOfParts>(.*?)</TitlesOfParts>",
        xml_text(docs, "docProps/app.xml"),
    );
    let listed = captures(
        r"<vt:lpstr>([^<]*)</vt:lpstr>",
        titles.first().map_or("", |m| &m[1]),
    )
    .into_iter()
    .map(|m| m[1].clone())
    .collect::<Vec<_>>();
    if listed.iter().take(sheets.len()).ne(sheets.iter()) {
        let first = listed
            .iter()
            .zip(sheets)
            .find(|(a, b)| a != b)
            .map_or("the end", |(a, _)| a);
        failures.push(format!("docProps/app.xml lists {} sheet titles that do not match the workbook's {}, first difference at {}", listed.len(), sheets.len(), py::repr_str(first)));
    }
}
fn slicer_caches(
    names: &[String],
    docs: &BTreeMap<&str, Document<'_>>,
    book: &str,
    failures: &mut Vec<String>,
) {
    for name in names.iter().filter(|n| n.starts_with("xl/slicerCaches/")) {
        for cache in captures(
            r#"<slicerCacheDefinition[^>]*name="([^"]+)""#,
            xml_text(docs, name),
        ) {
            if !book.contains(&format!(
                "<definedName name=\"{}\">#N/A</definedName>",
                cache[1]
            )) {
                failures.push(format!(
                    "slicer cache {} has no backing defined name",
                    cache[1]
                ));
            }
        }
    }
}
pub fn check(parts: &Parts, names: &[String]) -> Report {
    let mut failures = Vec::new();
    forbidden_parts(names, &mut failures);
    let docs = parse_documents(parts, names, &mut failures);
    web_extensions(names, &docs, &mut failures);
    banned_in_parts(parts, names, &docs, &mut failures);
    if !banned_in_store(&docs, &mut failures) {
        return Report {
            failures,
            functions: 0,
        };
    }
    let book = xml_text(&docs, BOOK);
    let defined = defined_names(book, &mut failures);
    sheet_formulas(names, &docs, &defined.known, &mut failures);
    volatile_formulas(names, &docs, &mut failures);
    shared_string_count(&docs, &mut failures);
    if !book.contains("fullCalcOnLoad=\"1\"") && !names.iter().any(|n| n == "xl/calcChain.xml") {
        failures.push("neither fullCalcOnLoad nor a calculation chain. Run tools/refresh_cache.py, or set fullCalcOnLoad".into());
    }
    sheet_titles(&docs, &defined.sheets, &mut failures);
    slicer_caches(names, &docs, book, &mut failures);
    let functions = defined
        .defined
        .iter()
        .filter(|n| matches(&format!("^(?:{TOKEN})$"), n))
        .count();
    Report {
        failures,
        functions,
    }
}
pub fn verify(args: &[String]) -> Output {
    if args.len() > 1 {
        return Output::stderr("FAIL: usage: ozzit verify_workbook [workbook]");
    }
    let workbook = args.first().map_or("ozzit.xlsx", String::as_str);
    let (parts, names) = match read_selected_inventory(Path::new(workbook), |name| {
        name.ends_with(".xml") || name.ends_with(".rels") || name.ends_with(".bin")
    }) {
        Ok(data) => data,
        Err(e) => return Output::failure(format!("FAIL: cannot read {workbook}: {e}")),
    };
    let report = check(&parts, &names);
    let bad = format!("FAIL: {} problem(s) in {workbook}", report.failures.len());
    Output::report(
        report.failures,
        bad,
        format!(
            "OK: {workbook}, {} functions, {} parts",
            report.functions,
            names.len()
        ),
    )
}
