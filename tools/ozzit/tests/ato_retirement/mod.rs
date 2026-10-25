//! Owned ATO copies reverse the original follower insertion, never the authority.
pub use super::copies::{Fixture, root};
use super::tooling::{
    archive::{BOOK, read_parts, write_deterministic},
    common::Csv,
};

pub const MODULES: [&str; 2] = ["Ratios", "Dates"];
pub fn data() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../src/data/postbuild/ato_benchmarks_rate_lookup.json"
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
    let follower = about[3].as_str().unwrap();
    assert_eq!(text.matches(follower).count(), 1);
    let start = text.find(follower).unwrap();
    let gap = &text[text[..start].trim_end_matches('\n').len()..start];
    let blocks = functions
        .iter()
        .map(|f| f[3].as_str())
        .collect::<Vec<_>>()
        .join("\n\n\n");
    assert_eq!(text.matches(&format!("{blocks}{gap}")).count(), 1);
    let text = text.replacen(&format!("{blocks}{gap}"), "", 1);
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
    let inserted = format!("{anchor} &\n{}", rows.join("\n"));
    assert_eq!(text.matches(&inserted).count(), 1);
    text.replacen(&inserted, &format!("{anchor} &"), 1)
}
pub fn absent() -> Fixture {
    let f = Fixture::new();
    for module in MODULES {
        let original = source(&f, module);
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
    let csv = Csv::read(&f.index).unwrap();
    assert_eq!(
        csv.rows
            .iter()
            .filter(|r| names
                .iter()
                .any(|n| Some(n.as_str()) == csv.get(r, "function")))
            .count(),
        6
    );
    f.remove_index_names(&names);
    f
}
#[allow(dead_code)] // Physical CLI tests also share this fixture with private failures.
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
    assert_eq!(
        std::fs::read(&f.index).unwrap(),
        std::fs::read(root().join("functions.csv")).unwrap()
    );
    for module in MODULES {
        assert_eq!(
            std::fs::read(f.src.join(format!("{module}.txt"))).unwrap(),
            std::fs::read(root().join(format!("src/{module}.txt"))).unwrap()
        );
    }
}
