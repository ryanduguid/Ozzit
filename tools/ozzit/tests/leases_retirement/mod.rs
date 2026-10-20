//! Owned copies reverse every lease insertion exactly once.
pub use super::copies::{Fixture, root};
use super::tooling::{
    archive::{BOOK, read_parts, write_deterministic},
    common::{Csv, escape},
};

pub fn data() -> serde_json::Value {
    serde_json::from_str(include_str!("../../src/data/postbuild/aasb16_leases.json")).unwrap()
}
pub fn functions() -> Vec<Vec<String>> {
    data()["functions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            v.as_array()
                .unwrap()
                .iter()
                .map(|s| s.as_str().unwrap().into())
                .collect()
        })
        .collect()
}
pub fn source(f: &Fixture) -> String {
    std::fs::read_to_string(f.src.join("Financial.txt")).unwrap()
}
fn unapply_source(mut text: String) -> String {
    let table = data();
    let functions = functions();
    for row in &functions {
        let block = format!("\n\n\n{}", row[2]);
        assert_eq!(text.matches(&block).count(), 1);
        text = text.replacen(&block, "", 1);
    }
    let anchor = table["anchor"].as_str().unwrap();
    let heading = table["heading"].as_str().unwrap();
    let at = text.find(anchor).unwrap();
    let start = text[..at].rfind('\n').map_or(0, |i| i + 1);
    let indent = &text[start..at];
    let mut rows = vec![format!("{indent}\"{heading}¶\" &")];
    rows.extend(
        functions
            .iter()
            .map(|r| format!("{indent}\"{:<19}→{}¶\" &", r[0], r[3])),
    );
    let inserted = format!("{anchor} &\n{}", rows.join("\n"));
    assert_eq!(text.matches(&inserted).count(), 1);
    text.replacen(&inserted, &format!("{anchor} &"), 1)
}

pub fn absent() -> Fixture {
    let f = Fixture::new();
    std::fs::write(f.src.join("Financial.txt"), unapply_source(source(&f))).unwrap();
    let table = data();
    let functions = functions();
    let anchor = table["anchor"].as_str().unwrap();
    let heading = table["heading"].as_str().unwrap();

    let mut parts = read_parts(&f.book).unwrap();
    let mut book = String::from_utf8(parts[BOOK].clone()).unwrap();
    let mut addition = format!(" &amp; \"{}¶\"", escape(heading));
    for row in &functions {
        addition.push_str(&format!(
            " &amp; \"{}¶\"",
            escape(&format!("{:<19}→{}", row[0], row[3]))
        ));
    }
    let stored_anchor = escape(anchor);
    let inserted = format!("{stored_anchor}{addition}");
    assert_eq!(book.matches(&inserted).count(), 1);
    book = book.replacen(&inserted, &stored_anchor, 1);
    let names = functions
        .iter()
        .map(|r| format!("oz.{}", r[0]))
        .collect::<Vec<_>>();
    for name in &names {
        let needle = format!("<definedName name=\"{name}\"");
        assert_eq!(book.matches(&needle).count(), 1);
        let at = book.find(&needle).unwrap();
        let end = at + book[at..].find("</definedName>").unwrap() + "</definedName>".len();
        book.replace_range(at..end, "");
    }
    parts.insert(BOOK.into(), book.into_bytes());
    write_deterministic(&f.book, &parts).unwrap();
    let csv = Csv::read(&f.index).unwrap();
    assert_eq!(
        csv.rows
            .iter()
            .filter(|r| names
                .iter()
                .any(|n| Some(n.as_str()) == csv.get(r, "function")))
            .count(),
        4
    );
    f.remove_index_names(&names);
    f
}
