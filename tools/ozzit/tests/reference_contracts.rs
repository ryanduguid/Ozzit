//! The per-function reference: help-block parsing and the index cross-check.
use ozzit::{formula, reference};
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

const HELP: &str = "FUNCTION:      →Thisλ(Values, [Mode])¶\
DESCRIPTION:   →Totals each row of an array¶\
→and labels the result.¶\
SEE ALSO:      →Excel's BYROW.¶\
VERSION:       →20 Aug 2026¶\
WEBPAGE:       →https://github.com/ryanduguid/Ozzit¶\
PARAMETERS:    →¶\
Values         →(Required) Numbers to total.¶\
Mode           →(Optional: Default = 1) How to total¶\
→each row.¶\
NOTE!          →Blank cells count as zero.¶\
SUGGESTED ROW IDS:→¶\
→Total¶\
EXAMPLES:      →¶\
Result         →Formula (oz is assumed to be the module's name)¶\
3              →=oz.Thisλ({1,2})¶\
               →=oz.Thisλ(A1:B2,¶\
               →2)";

#[test]
fn a_help_block_reads_into_its_parts() {
    let block = reference::parse(HELP);
    assert_eq!(block.signature.as_deref(), Some("Thisλ(Values, [Mode])"));
    assert_eq!(
        block.description,
        "Totals each row of an array and labels the result."
    );
    assert_eq!(block.see_also.as_deref(), Some("Excel's BYROW."));
    assert_eq!(block.version.as_deref(), Some("20 Aug 2026"));
    assert_eq!(block.parameters.len(), 2);
    assert_eq!(block.parameters[0].requirement.as_deref(), Some("Required"));
    assert_eq!(
        block.parameters[1].qualifier.as_deref(),
        Some("Default = 1")
    );
    assert_eq!(block.parameters[1].text, "How to total each row.");
    assert_eq!(block.notes, ["Blank cells count as zero."]);
    assert_eq!(block.extra.len(), 1);
    assert_eq!(block.extra[0].heading, "SUGGESTED ROW IDS");
    assert_eq!(block.extra[0].rows, [(String::new(), "Total".to_string())]);
    assert_eq!(block.examples.len(), 2);
    assert_eq!(block.examples[0].result.as_deref(), Some("3"));
    assert_eq!(block.examples[1].formula, "=oz.Thisλ(A1:B2,\n2)");
    assert_eq!(block.examples[1].result, None);
}

#[test]
fn every_example_shape_in_the_library_is_read() {
    let vertical =
        reference::parse("EXAMPLES:→¶→Formula (oz is assumed)¶→=oz.Thisλ(1)¶→Result¶→FY2027");
    assert_eq!(vertical.examples.len(), 1);
    assert_eq!(vertical.examples[0].result.as_deref(), Some("FY2027"));
    let inline = reference::parse("EXAMPLES:      →=Thisλ(A1, , 3)");
    assert_eq!(inline.examples[0].formula, "=Thisλ(A1, , 3)");
    let labelled = reference::parse(
        "EXAMPLES:→NOTE! oz is assumed¶Formula:→=oz.Thisλ(0, 1)¶Result:→00, 01¶→01, 02",
    );
    assert_eq!(labelled.examples.len(), 1);
    assert_eq!(
        labelled.examples[0].result.as_deref(),
        Some("00, 01\n01, 02")
    );
}

#[test]
fn only_the_help_textsplit_is_read() {
    let body = r#"LAMBDA([x], LET(Help, TRIM(TEXTSPLIT("FUNCTION:→Thisλ(x)¶" & "PARAMETERS:→¶x→(Required) A value", "→", "¶")), Note, "Other→text¶", x))"#;
    let text = reference::help_text(body).expect("help block");
    assert!(text.starts_with("FUNCTION:→"));
    assert!(!text.contains("Other"));
    assert!(reference::help_text("LAMBDA([x], x)").is_none());
}

fn fixture(index: &str) -> (tempfile::TempDir, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    std::fs::create_dir(&src).unwrap();
    for module in formula::MODULES {
        let text = if module == "Dates" {
            r#"Thisλ = LAMBDA([x], LET(Help, TRIM(TEXTSPLIT("FUNCTION:→Thisλ(x)¶PARAMETERS:→¶x→(Required) A value", "→", "¶")), x));"#
        } else {
            ""
        };
        std::fs::write(src.join(format!("{module}.txt")), text).unwrap();
    }
    let csv = dir.path().join("functions.csv");
    std::fs::write(&csv, index).unwrap();
    (dir, src, csv)
}

#[test]
fn the_index_and_sources_must_describe_the_same_library() {
    let header = "function,module,previous_name,signature,description\n";
    let (_dir, src, csv) = fixture(&format!("{header}oz.Thisλ,Dates,,Thisλ(x),Does this\n"));
    let (document, count) = reference::build(&src, &csv).unwrap();
    assert_eq!(count, 1);
    assert_eq!(document["format"], reference::FORMAT);
    let function = &document["modules"][0]["functions"][0];
    assert_eq!(function["name"], "oz.Thisλ");
    assert_eq!(function["summary"], "Does this");
    assert_eq!(function["parameters"][0]["name"], "x");

    let (_dir, src, csv) = fixture(&format!("{header}oz.Thisλ,Ratios,,Thisλ(x),Does this\n"));
    let problems = reference::build(&src, &csv).unwrap_err();
    assert_eq!(
        problems,
        ["oz.Thisλ is in src/Dates.txt but the index files it under Ratios"]
    );

    let (_dir, src, csv) = fixture(&format!("{header}oz.Thatλ,Dates,,Thatλ(x),Does that\n"));
    let problems = reference::build(&src, &csv).unwrap_err();
    assert!(problems.contains(&"oz.Thisλ is in src/Dates.txt but not in the index".to_string()));
}

#[test]
fn the_released_library_reads_completely() {
    let (document, count) =
        reference::build(&root().join("src"), &root().join("functions.csv")).unwrap();
    let functions = document["modules"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|m| m["functions"].as_array().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(functions.len(), count);
    let documented = functions
        .iter()
        .filter(|f| f["description"].is_string() && !f["parameters"].as_array().unwrap().is_empty())
        .count();
    assert!(
        documented >= 120,
        "only {documented} functions read with help"
    );
    for function in &functions {
        let name = function["name"].as_str().unwrap();
        assert!(function["signature"].is_string(), "{name} has no signature");
        for example in function["examples"].as_array().unwrap() {
            assert!(
                example["formula"].as_str().unwrap().starts_with('='),
                "{name}"
            );
        }
    }
}

#[test]
fn the_command_writes_the_reference() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("reference.json");
    let args = [
        root().join("src").display().to_string(),
        root().join("functions.csv").display().to_string(),
        output.display().to_string(),
    ];
    let result = reference::command(&args);
    assert_eq!(result.code, 0, "{:?}", result.out);
    let written = std::fs::read_to_string(&output).unwrap();
    assert!(written.ends_with("}\n") && !written.contains('\r'));
    assert_eq!(reference::command(&args[..2]).code, 1);
}

#[test]
fn bullet_rows_stay_on_their_own_lines() {
    let block = reference::parse(
        "PARAMETERS:→¶Assets→(Required) Quick assets. Includes:¶→• Cash¶→• Receivables¶→Alternatively:¶→• Current assets",
    );
    assert_eq!(
        block.parameters[0].text,
        "Quick assets. Includes:\n• Cash\n• Receivables\nAlternatively:\n• Current assets"
    );
    let wrapped = reference::parse("DESCRIPTION:→Totals each row¶→of an array.");
    assert_eq!(wrapped.description, "Totals each row of an array.");
}

#[test]
fn rows_after_an_introducing_colon_stay_on_their_own_lines() {
    let block = reference::parse(
        "PARAMETERS:→¶Liabilities→(Required) Current liabilities. Includes:¶→Accounts payable¶→Wages",
    );
    assert_eq!(
        block.parameters[0].text,
        "Current liabilities. Includes:\nAccounts payable\nWages"
    );
}

#[test]
fn a_lower_case_row_after_a_colon_continues_the_sentence() {
    let block = reference::parse(
        "PARAMETERS:→¶Life→(Required) A date here is not a life:¶→the arguments read Values.",
    );
    assert_eq!(
        block.parameters[0].text,
        "A date here is not a life: the arguments read Values."
    );
}

const INDEX_HEADER: &str = "function,module,previous_name,signature,description\n";

#[test]
fn a_duplicate_index_row_or_definition_is_refused() {
    let (_dir, src, csv) = fixture(&format!(
        "{INDEX_HEADER}oz.Thisλ,Dates,,Thisλ(x),Does this\noz.Thisλ,Dates,,Thisλ(x),Does this\n"
    ));
    let problems = reference::build(&src, &csv).unwrap_err();
    assert!(problems.contains(&"oz.Thisλ is listed more than once in the index".to_string()));

    let (_dir, src, csv) = fixture(&format!(
        "{INDEX_HEADER}oz.Thisλ,Dates,,Thisλ(x),Does this\noz.Thatλ,Dates,,Thatλ(x),Does that\n"
    ));
    let text = std::fs::read_to_string(src.join("Dates.txt")).unwrap();
    std::fs::write(src.join("Dates.txt"), format!("{text}\n{text}")).unwrap();
    let problems = reference::build(&src, &csv).unwrap_err();
    assert!(problems.contains(&"oz.Thisλ is defined more than once in the sources".to_string()));
    assert!(problems.contains(&"oz.Thatλ is in the index but not in the sources".to_string()));
}

#[test]
fn the_help_signature_must_match_the_index_apart_from_spacing() {
    let (_dir, src, csv) = fixture(&format!(
        "{INDEX_HEADER}oz.Thisλ,Dates,,Thisλ( x ),Does this\n"
    ));
    assert!(reference::build(&src, &csv).is_ok());
    let (_dir, src, csv) = fixture(&format!(
        "{INDEX_HEADER}oz.Thisλ,Dates,,Thisλ(y),Does this\n"
    ));
    let problems = reference::build(&src, &csv).unwrap_err();
    assert_eq!(
        problems,
        ["oz.Thisλ: its help signature Thisλ(x) disagrees with the index signature Thisλ(y)"]
    );
}

#[test]
fn text_on_an_unrecognised_heading_row_is_kept() {
    let block = reference::parse("ROW CONTENTS:→Opening balance¶→Closing balance");
    assert_eq!(block.extra[0].heading, "ROW CONTENTS");
    assert_eq!(
        block.extra[0].rows,
        [
            (String::new(), "Opening balance".to_string()),
            (String::new(), "Closing balance".to_string())
        ]
    );
}

/// Expected values read by hand from the released help in src/, not from the parser.
#[test]
fn released_help_survives_in_full() {
    let (document, _) =
        reference::build(&root().join("src"), &root().join("functions.csv")).unwrap();
    let function = |name: &str| {
        document["modules"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|m| m["functions"].as_array().unwrap())
            .find(|f| f["name"] == name)
            .unwrap()
            .clone()
    };
    let labour = function("oz.ATOLabourRatioλ");
    assert_eq!(
        labour["notes"][0],
        "(Wages + Contractors - AssociatePayments) / Turnover, where Wages is SalaryWages, or W1 when W1 is greater. The ATO adds total salary and wages expenses to contractor, subcontractor and commission expenses and deducts payments to associated persons. Where the activity statements' label W1 (total salary, wages and other payments) for the year exceeds the return's salary and wages, it uses W1. Returns a fraction, as ATOBenchmarkRatioλ() does. An omitted or blank AssociatePayments or W1 counts as 0. Turnover of 0 returns #DIV/0!. Arithmetic only. ATO benchmark ratio calculations checked 19 October 2026."
    );
    assert_eq!(labour["parameters"][3]["name"], "AssociatePayments");
    assert_eq!(labour["parameters"][3]["requirement"], "Optional");
    assert_eq!(labour["parameters"][3]["qualifier"], "Default = 0");
    assert_eq!(
        labour["parameters"][3]["text"],
        "Payments to associated persons."
    );
    assert_eq!(
        labour["examples"][0]["formula"],
        "=oz.ATOLabourRatioλ(90000, 20000, 480000, 10000, 95000)"
    );
    assert_eq!(labour["examples"][0]["result"], "0.21875");
    let quick = function("oz.QuickRatioλ");
    assert_eq!(
        quick["parameters"][0]["text"],
        "Quick Assets. Includes:\n• Cash & Cash equivalents\n• Marketable securities\n• Accounts receivable\nAlterantively it includes:\n• Current assets\n• Less inventory\n• Less prepaid expenses"
    );
    assert_eq!(
        quick["parameters"][1]["text"],
        "Current Liabilities. Includes:\nAccounts payable\nWages\nTaxes payable\nShort-term debts\nCurrent portion of long-term debt"
    );
    assert_eq!(quick["examples"][0]["result"], "1.2");
    let year = function("oz.FinancialYearλ");
    assert_eq!(year["parameters"][1]["qualifier"], "Default = 7");
    assert_eq!(year["examples"][0]["result"], "FY2027");
}
