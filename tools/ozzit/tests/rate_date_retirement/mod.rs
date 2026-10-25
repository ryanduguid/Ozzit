//! Reuse owned store copies; only reversal and command details differ by pass.
pub use super::copies::{Fixture, root};
use super::tooling;
use tooling::{
    archive::{BOOK, read_parts, write_deterministic},
    common::{Csv, csv_row},
};

pub const MODULES: [&str; 2] = ["Financial", "Dates"];
pub fn data() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../src/data/postbuild/rate_date_helpers.json"
    ))
    .unwrap()
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
pub fn source(f: &Fixture, module: &str) -> String {
    std::fs::read_to_string(f.src.join(format!("{module}.txt"))).unwrap()
}
pub fn unapply_source(module: &str, text: &str) -> String {
    let table = data();
    let about = &table["about"][module];
    let functions = functions()
        .into_iter()
        .filter(|f| f[0] == module)
        .collect::<Vec<_>>();
    let blocks = format!(
        "\n\n\n{}\n",
        functions
            .iter()
            .map(|f| f[3].as_str())
            .collect::<Vec<_>>()
            .join("\n\n\n")
    );
    let text = text.strip_suffix(&blocks).unwrap_or(text);
    let anchor = about[0].as_str().unwrap();
    let at = text.find(anchor).unwrap();
    let start = text[..at].rfind('\n').map_or(0, |i| i + 1);
    let indent = &text[start..at];
    let mut rows = match about[1].as_str() {
        Some(heading) => vec![
            format!("{indent}\"→¶\" &"),
            format!("{indent}\"{heading}¶\" &"),
        ],
        None => vec![],
    };
    let width = about[2].as_u64().unwrap() as usize;
    rows.extend(
        functions
            .iter()
            .map(|f| format!("{indent}\"{:<width$}→{}¶\" &", f[1], f[2])),
    );
    text.replacen(
        &format!("{anchor} &\n{}", rows.join("\n")),
        &format!("{anchor} &"),
        1,
    )
}
pub fn absent() -> Fixture {
    let f = Fixture::new();
    for module in MODULES {
        let original = source(&f, module);
        let blocks = format!(
            "\n\n\n{}\n",
            functions()
                .iter()
                .filter(|r| r[0] == module)
                .map(|r| r[3].as_str())
                .collect::<Vec<_>>()
                .join("\n\n\n")
        );
        assert!(original.ends_with(&blocks), "{module}");
        let absent = unapply_source(module, &original);
        assert_ne!(absent, original);
        std::fs::write(f.src.join(format!("{module}.txt")), absent).unwrap();
    }
    let mut parts = read_parts(&f.book).unwrap();
    let mut book = String::from_utf8(parts[BOOK].clone()).unwrap();
    let names = functions()
        .iter()
        .map(|r| format!("oz.{}", r[1]))
        .collect::<Vec<_>>();
    for name in &names {
        let needle = format!("<definedName name=\"{name}\"");
        assert_eq!(book.matches(&needle).count(), 1, "{name}");
        let at = book.find(&needle).unwrap();
        let end = at + book[at..].find("</definedName>").unwrap() + "</definedName>".len();
        book.replace_range(at..end, "");
    }
    parts.insert(BOOK.into(), book.into_bytes());
    write_deterministic(&f.book, &parts).unwrap();
    remove_index_names(&f, &names);
    f
}
pub fn remove_index_names(f: &Fixture, names: &[String]) {
    let csv = Csv::read(&f.index).unwrap();
    let mut text = csv_row(&csv.headers);
    let mut removed = 0;
    for row in &csv.rows {
        if names
            .iter()
            .any(|n| Some(n.as_str()) == csv.get(row, "function"))
        {
            removed += 1;
        } else {
            text.push_str(&csv_row(row));
        }
    }
    assert_eq!(removed, names.len());
    std::fs::write(&f.index, text).unwrap();
}
pub fn assert_applied(f: &Fixture) {
    let parts = read_parts(&f.book).unwrap();
    let book = String::from_utf8(parts[BOOK].clone()).unwrap();
    for row in functions() {
        assert_eq!(
            book.matches(&format!("<definedName name=\"oz.{}\"", row[1]))
                .count(),
            1
        );
    }
    for module in MODULES {
        assert_eq!(
            std::fs::read(f.src.join(format!("{module}.txt"))).unwrap(),
            std::fs::read(root().join(format!("src/{module}.txt"))).unwrap()
        );
    }
}
