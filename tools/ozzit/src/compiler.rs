use crate::{archive::*, common::*, formula::*, py};
use indexmap::IndexMap;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[derive(Debug, Clone)]
pub struct Compiled {
    pub name: String,
    pub module: String,
    pub body: String,
    pub stored: String,
    pub comment: String,
}
pub fn compile_sources(src: &Path) -> Result<Vec<Compiled>> {
    let mut declared = Vec::new();
    for module in MODULES {
        let path = src.join(format!("{module}.txt"));
        if !path.is_file() {
            return Err(format!("missing source module {}", display(&path)));
        }
        let text = read_raw(&path)?;
        let comments = header_comments(&text);
        for st in statements(&text) {
            for m in captures(NAME, &st) {
                let bare = m[1].clone();
                let Some(comment) = comments.get(&bare) else {
                    return Err(format!(
                        "{bare}: no FUNCTION NAME / DESCRIPTION header in {}",
                        basename(&path)
                    ));
                };
                declared.push((bare, module, py::strip(&m[2]).to_string(), comment.clone()));
            }
        }
    }
    let library = declared
        .iter()
        .map(|(bare, _, _, _)| bare.clone())
        .collect::<BTreeSet<_>>();
    let mut out = Vec::new();
    for (bare, module, body, comment) in declared {
        let stored = if body.contains("LAMBDA(") {
            render(&body, &library)
        } else {
            render(&format!("LAMBDA({body})"), &library)
                .map(|s| py::strip(&s[13..s.len() - 1]).to_string())
        }
        .map_err(|e| format!("{bare}: {e}"))?;
        let length = stored.chars().count();
        if length > 8192 {
            return Err(format!(
                "{bare}: the stored definition is {length} characters and Excel refuses to open a workbook with a defined name over 8192"
            ));
        }
        let want = canonical(&qualify(&body, "oz", &library));
        let got = canonical(&stored);
        if want != got {
            let cut = want
                .chars()
                .zip(got.chars())
                .position(|(a, b)| a != b)
                .unwrap_or(want.chars().count().min(got.chars().count()));
            return Err(format!(
                "{bare}: the rendering does not reproduce its source at character {cut}\n  src   : {}\n  stored: {}",
                char_slice(&want, cut.saturating_sub(40), cut + 60),
                char_slice(&got, cut.saturating_sub(40), cut + 60)
            ));
        }
        out.push(Compiled {
            name: format!("oz.{bare}"),
            module: module.into(),
            body,
            stored,
            comment,
        });
    }
    Ok(out)
}
pub fn shipped_names(book: &str) -> IndexMap<String, (String, String)> {
    captures(NAME_RE, book)
        .into_iter()
        .map(|m| (m[1].clone(), (m[2].clone(), unescape(&m[3]))))
        .collect()
}
pub fn workbook_names(path: &Path) -> Result<BTreeSet<String>> {
    let parts = read_parts(path).map_err(|e| format!("cannot read workbook names: {e}"))?;
    let book = read_book(&parts).map_err(|e| format!("cannot read workbook names: {e}"))?;
    Ok(
        captures(r#"<definedName\b[^>]*\bname="([^"]+)"[^>]*>"#, &book)
            .into_iter()
            .map(|m| unescape(&m[1]))
            .filter(|s| s.starts_with("oz."))
            .collect(),
    )
}
pub fn insert_names(book: &str, compiled: &[Compiled]) -> Result<String> {
    let mut book = book.to_string();
    let mut existing = captures(r#"<definedName name="(oz\.[^"]+)""#, &book)
        .into_iter()
        .map(|m| m[1].clone())
        .collect::<Vec<_>>();
    existing.sort_by_key(|s| s.to_lowercase());
    let mut items = compiled.iter().collect::<Vec<_>>();
    items.sort_by_key(|s| s.name.to_lowercase());
    for item in items {
        let before = existing
            .iter()
            .filter(|n| n.to_lowercase() < item.name.to_lowercase())
            .collect::<Vec<_>>();
        let Some(prior) = before.last() else {
            return Err(format!(
                "{} sorts before every shipped name, which is not expected",
                item.name
            ));
        };
        let marker = format!("<definedName name=\"{}\"", escape(prior));
        let start = book.find(&marker).ok_or("substring not found")?;
        let end = start
            + book[start..]
                .find("</definedName>")
                .ok_or("substring not found")?
            + 14;
        let element = format!(
            "<definedName name=\"{}\" comment=\"{}\">{}</definedName>",
            escape(&item.name),
            escape_attr(&item.comment),
            escape(&item.stored)
        );
        book.insert_str(end, &element);
        existing.push(item.name.clone());
        existing.sort_by_key(|s| s.to_lowercase());
    }
    Ok(book)
}
pub fn apply(book: &str, compiled: &[Compiled]) -> Result<(String, Vec<String>)> {
    let shipped = shipped_names(book);
    let declared = compiled
        .iter()
        .map(|c| c.name.as_str())
        .collect::<BTreeSet<_>>();
    let missing = shipped
        .keys()
        .filter(|n| !declared.contains(n.as_str()))
        .cloned()
        .collect::<BTreeSet<_>>();
    if !missing.is_empty() {
        return Err(format!(
            "{} ship in the workbook but are not declared in src/",
            missing.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }
    let mut book = book.to_string();
    let mut changed = Vec::new();
    for item in compiled {
        let Some((attrs, current)) = shipped.get(&item.name) else {
            return Err(format!(
                "{} is declared in src/ but does not ship; add it first",
                item.name
            ));
        };
        let old = captures(r#" comment="([^"]*)""#, attrs);
        let same_comment = old.first().is_some_and(|m| unescape(&m[1]) == item.comment);
        let same = tight(current) == tight(&item.stored);
        if same && same_comment {
            continue;
        }
        let element = format!("<definedName name=\"{}\"{attrs}>", item.name);
        let start = book.find(&element).ok_or("substring not found")?;
        let end = start
            + book[start..]
                .find("</definedName>")
                .ok_or("substring not found")?;
        let new_attrs = format!(
            "{} comment=\"{}\"",
            sub(r#" comment="[^"]*""#, attrs, ""),
            escape_attr(&item.comment)
        );
        let body = if same {
            book[start + element.len()..end].to_string()
        } else {
            escape(&item.stored)
        };
        book.replace_range(
            start..end,
            &format!("<definedName name=\"{}\"{new_attrs}>{body}", item.name),
        );
        changed.push(item.name.clone());
    }
    Ok((book, changed))
}
pub fn index_fields(attrs: &str, body: &str, bare: &str) -> (String, String) {
    let pattern = re(r"FUNCTION:\s*+(?:→\s*+)?(.*?)¶");
    let sig = pattern.captures(body).expect("signature matching");
    let (mut text, mut end) = if let Some(m) = sig {
        (
            py::strip(&m[1]).to_string(),
            m.get(0).expect("signature").end(),
        )
    } else {
        (bare.into(), 0)
    };
    while count(&text, "(") > count(&text, ")") {
        let row = re(r"→(.*?)¶");
        let Some(m) = row.captures(&body[end..]).expect("row matching") else {
            break;
        };
        text = py::strip(&format!("{text} {}", py::strip(&m[1]))).to_string();
        end += m.get(0).expect("row").end();
    }
    let comment = captures(r#"comment="([^"]*)""#, attrs);
    let fallback = captures(r"DESCRIPTION:\s*→(.*?)¶", body);
    let blurb = comment.first().map(|m| unescape(&m[1])).unwrap_or_else(|| {
        fallback
            .first()
            .map_or(String::new(), |m| py::strip(&m[1]).into())
    });
    let blurb = blurb.replace("_x000a_", " ").replace("_x000D_", " ");
    (
        text,
        sub(r"\s{2,}", py::strip(blurb.trim_start_matches('→')), " "),
    )
}
pub fn source_modules(src: &Path) -> Result<BTreeMap<String, String>> {
    let mut modules = BTreeMap::new();
    for path in txt_files(src)? {
        for st in statements(&read_text(&path)?) {
            for m in captures(NAME, &st) {
                modules.insert(
                    format!("oz.{}", m[1]),
                    path.file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    Ok(modules)
}
pub fn render_index(
    path: &Path,
    book: &str,
    modules: &BTreeMap<String, String>,
) -> Result<Option<String>> {
    let csv = Csv::read(path)?;
    let fields = [
        "function",
        "module",
        "previous_name",
        "signature",
        "description",
    ]
    .map(String::from)
    .to_vec();
    // DictReader's previous-name map is read before Python validates the header.
    if !csv.rows.is_empty() && !csv.headers.iter().any(|h| h == "function") {
        return Err(py::repr_str("function"));
    }
    if csv.headers != fields {
        return Err(format!(
            "{} has unexpected columns: [{}]",
            display(path),
            csv.headers
                .iter()
                .map(|s| py::repr_str(s))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let previous = csv
        .rows
        .iter()
        .map(|r| {
            (
                csv.get(r, "function").unwrap_or(""),
                csv.get(r, "previous_name").unwrap_or(""),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let names = shipped_names(book).into_iter().collect::<BTreeMap<_, _>>();
    let mut after = csv_row(&fields);
    for (name, (attrs, body)) in names {
        let (sig, desc) = index_fields(&attrs, &body, name.rsplit('.').next().unwrap_or(""));
        after.push_str(&csv_row(&[
            name.clone(),
            modules.get(&name).cloned().unwrap_or_default(),
            previous.get(name.as_str()).copied().unwrap_or("").into(),
            sig,
            desc,
        ]));
    }
    Ok(if read_raw(path)?.trim_start_matches('\u{feff}') == after {
        None
    } else {
        Some(after)
    })
}
pub fn run(workbook: &Path, src: &Path, check: bool, index: Option<&Path>) -> Result<Vec<String>> {
    let compiled = compile_sources(src)?;
    let mut parts = read_parts(workbook)?;
    let book = read_book(&parts)?;
    let (new_book, mut changed) = apply(&book, &compiled)?;
    if check {
        return Ok(changed);
    }
    let modules = compiled
        .iter()
        .map(|c| (c.name.clone(), c.module.clone()))
        .collect();
    let new_index = if let Some(path) = index {
        render_index(path, &new_book, &modules)?
    } else {
        None
    };
    if !changed.is_empty() {
        parts.insert(BOOK.into(), new_book.into_bytes());
        write_deterministic(workbook, &parts)?;
    }
    if let (Some(path), Some(text)) = (index, new_index) {
        write_bytes(path, text.as_bytes())?;
        changed.push(basename(path));
    }
    Ok(changed)
}
pub fn command(args: &[String]) -> Output {
    let mut paths = Vec::new();
    let mut check = false;
    let mut index = None;
    for arg in args {
        if arg == "--check" {
            check = true;
        } else if let Some(p) = arg.strip_prefix("--index=") {
            index = Some(Path::new(p));
        } else if arg.starts_with("--") {
            return Output::stderr(
                "FAIL: usage: ozzit compile_sources [workbook] [src dir] [--check] [--index=functions.csv]",
            );
        } else {
            paths.push(arg);
        }
    }
    if paths.len() > 2 {
        return Output::stderr("FAIL: usage: ozzit compile_sources [workbook] [src dir] [--check]");
    }
    let workbook = Path::new(paths.first().map_or("ozzit.xlsx", |s| s.as_str()));
    let src = Path::new(paths.get(1).map_or("src", |s| s.as_str()));
    if !workbook.is_file() {
        return Output::stderr(format!("FAIL: no such workbook: {}", display(workbook)));
    }
    match run(workbook, src, check, index) {
        Err(e) => Output::stderr(format!("FAIL: {e}")),
        Ok(changed) => {
            let first = if changed.is_empty() {
                if check {
                    "every defined name already matches src/".into()
                } else {
                    "every defined name already matches src/; no changes".into()
                }
            } else {
                format!(
                    "{}: {}",
                    if check {
                        "would recompile"
                    } else {
                        "recompiled"
                    },
                    changed.join(", ")
                )
            };
            let last = if check {
                format!(
                    "OK: {} checked against {}/",
                    display(workbook),
                    display(src)
                )
            } else {
                format!("OK: {}", display(workbook))
            };
            Output::ok(vec![first, last])
        }
    }
}
