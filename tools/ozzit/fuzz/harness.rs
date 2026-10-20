use ozzit::{formula, py, selftest};

fn positive_tolerance(line: &str, tolerance: &selftest::Decimal) {
    assert!(
        tolerance.is_positive(),
        "non-positive tolerance for {line:?}"
    );
}

pub fn test_one_input(data: &[u8]) {
    let text = String::from_utf8_lossy(data);
    // The native return types guarantee strings and finite exact decimals.
    std::hint::black_box(formula::statements(&text));
    for line in py::splitlines(&text) {
        if let Some((value, tolerance)) = selftest::number(line) {
            positive_tolerance(line, &tolerance);
            std::hint::black_box(value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "non-positive tolerance")]
    fn a_number_without_tolerance_is_caught() {
        let (zero, _) = selftest::number("0").unwrap();
        positive_tolerance("1", &zero);
    }
}
