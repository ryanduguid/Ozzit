//! Replay observations from the immutable Python test behaviours.
use ozzit::{formula, help};
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn text(value: &Value) -> &str {
    value.as_str().expect("contract string")
}

/// The port's result for recorded call `i`, or `None` once its recorded error matched.
fn replay(record: &Value, i: usize) -> Option<Value> {
    let args = record["args"].as_array().unwrap();
    let first = text(&args[0]);
    Some(match text(&record["function"]) {
        "render" => {
            let library = args[1]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| text(v).to_owned())
                .collect::<BTreeSet<_>>();
            let result = formula::render(first, &library);
            if let Some(error) = record.get("error") {
                assert_eq!(result.unwrap_err(), text(error), "contract {i}");
                return None;
            }
            json!(result.unwrap_or_else(|e| panic!("contract {i}: {e}")))
        }
        "header_comments" => json!(formula::header_comments(first)),
        "canonical" => json!(formula::canonical(first)),
        "statements" => json!(formula::statements(first)),
        "split_literals" => json!(formula::split_literals(first)),
        "qualify" => {
            let names = args[2]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| text(v).to_owned())
                .collect();
            json!(formula::qualify(first, text(&args[1]), &names))
        }
        "dead_conversions" => json!(formula::dead_conversions(first)),
        "collapsed_defaults" => json!(formula::collapsed_defaults(first)),
        "literals" => json!(help::literals(first)),
        "help_rows" => json!(help::rows(first)),
        "declared" => json!(help::declared(first)),
        "worked_examples" => json!(help::worked_examples(first)),
        "parameter_table" => json!(help::parameter_table(first)),
        function => panic!("unknown contract {function}"),
    })
}

#[test]
fn python_formula_and_help_contracts_replay_without_differences() {
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data/python-contracts.json.gz"),
    )
    .unwrap();
    let data: Value = serde_json::from_reader(flate2::read::GzDecoder::new(&bytes[..])).unwrap();
    let records = data["records"].as_array().unwrap();
    assert!(
        records.len() >= 100,
        "the original tests must exercise the parser"
    );
    for (i, record) in records.iter().enumerate() {
        assert!(
            record["kwargs"].as_object().unwrap().is_empty(),
            "unmapped keyword argument in contract {i}"
        );
        let Some(actual) = replay(record, i) else {
            continue;
        };
        assert_eq!(
            actual, record["expected"],
            "contract {i}: {}",
            record["function"]
        );
    }
}
