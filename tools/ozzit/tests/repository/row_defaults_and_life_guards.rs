//! Row-by-row defaults in `GSTAddλ`, `GSTExtractλ` and `Movementλ`, and the Life guards.
//!
//! `OR()` reduced a column of rates or openings to one TRUE, so a single blank
//! gave every row the default: zero-rated GST lines were taxed and every opening
//! balance read as 0. A negative life wrote the whole cost off in one period and
//! a zero life divided by zero. The native self-test runs these cases in Excel;
//! this checks that the source and the names the workbook ships both carry the fix.
use super::{defined_name, index, part, parts, read, spaced};
use ozzit::common;

const EXPECTED: [(&str, &[&str]); 5] = [
    (
        "GSTAddλ",
        &[r#"IF(ISOMITTED(Rate), 0.1, IF(TRIM(Rate & "")="", 0.1, Rate))"#],
    ),
    (
        "GSTExtractλ",
        &[r#"IF(ISOMITTED(Rate), 0.1, IF(TRIM(Rate & "")="", 0.1, Rate))"#],
    ),
    (
        "Movementλ",
        &[
            "IF(ISOMITTED(BeginningValues), 0,",
            r#"IF(TRIM(BeginningValues & "")="", 0, BeginningValues))"#,
        ],
    ),
    (
        "DiminishingValueλ",
        &[
            "AND(ISNUMBER(Life), Life > 0)",
            r#""DiminishingValueλ needs a Life greater than 0""#,
        ],
    ),
    (
        "PrimeCostλ",
        &[
            "AND(ISNUMBER(Life), Life > 0)",
            r#""PrimeCostλ needs a Life greater than 0""#,
        ],
    ),
];
const COLLAPSED: &str = r"OR\(ISOMITTED\((Rate|BeginningValues)\)";

struct Stores {
    source: String,
    book: String,
}

impl Stores {
    fn new() -> Self {
        Self {
            source: read("src/Financial.txt"),
            book: part(&parts("ozzit.xlsx"), "xl/workbook.xml"),
        }
    }

    fn definition(&self, name: &str) -> String {
        let start = index(&self.source, &format!("\n{name} = LAMBDA("), 0);
        let end = index(&self.source, "\n);", start);
        spaced(&self.source[start..end])
    }

    fn shipped(&self, name: &str) -> String {
        let text =
            defined_name(&self.book, &format!("oz.{name}")).unwrap_or_else(|e| panic!("{e}"));
        let unescaped = common::unescape(&text).unwrap_or_else(|e| panic!("{e}"));
        spaced(&common::sub(r"_xl(?:fn|pm|op)\.", &unescaped, ""))
    }
}

#[test]
fn the_source_and_the_shipped_names_carry_the_fix() {
    let stores = Stores::new();
    for (name, fragments) in EXPECTED {
        for (store, text) in [
            ("src", stores.definition(name)),
            ("definedName", stores.shipped(name)),
        ] {
            for fragment in fragments {
                assert!(
                    text.contains(&spaced(fragment)),
                    "name={name} store={store}: {fragment:?} not in {text:?}"
                );
            }
        }
    }
}

#[test]
fn no_default_is_collapsed_by_or() {
    let stores = Stores::new();
    for name in ["GSTAddλ", "GSTExtractλ", "Movementλ"] {
        assert!(
            !common::matches(COLLAPSED, &stores.definition(name)),
            "{name} in src"
        );
        assert!(
            !common::matches(COLLAPSED, &stores.shipped(name)),
            "{name} shipped"
        );
    }
}
