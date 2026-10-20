use ozzit::{formula, py, selftest};

pub fn test_one_input(data: &[u8]) {
    let text = String::from_utf8_lossy(data);
    // The native return types guarantee strings and finite exact decimals.
    std::hint::black_box(formula::statements(&text));
    for line in py::splitlines(&text) {
        if let Some((value, tolerance)) = selftest::number(line) {
            assert!(
                tolerance.is_positive(),
                "non-positive tolerance for {line:?}"
            );
            std::hint::black_box(value);
        }
    }
}
