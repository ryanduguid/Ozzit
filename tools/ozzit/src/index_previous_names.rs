//! Small-fixture counterparts of PreviousNamesTests; the command keeps floor 100.
use super::*;

struct Fixture {
    dir: tempfile::TempDir,
    index: std::path::PathBuf,
    baseline: std::path::PathBuf,
}
impl Fixture {
    fn new(names: &[&str]) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let index = dir.path().join("functions.csv");
        let baseline = dir.path().join("baseline.txt");
        std::fs::write(
            &baseline,
            format!("# released names\n{}\n", names.join("\n")),
        )
        .unwrap();
        Self {
            dir,
            index,
            baseline,
        }
    }
    fn write(&self, fields: &[&str], rows: &[&[&str]]) {
        let text = std::iter::once(fields)
            .chain(rows.iter().copied())
            .map(|row| csv_row(&row.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>()))
            .collect::<String>();
        std::fs::write(&self.index, text).unwrap();
    }
    fn run(&self, floor: usize) -> Output {
        let before = [
            std::fs::read(&self.index).unwrap(),
            std::fs::read(&self.baseline).unwrap(),
        ];
        let output =
            previous_names_with_floor(&[display(&self.index), display(&self.baseline)], floor);
        assert_eq!(std::fs::read(&self.index).unwrap(), before[0]);
        assert_eq!(std::fs::read(&self.baseline).unwrap(), before[1]);
        assert_eq!(std::fs::read_dir(self.dir.path()).unwrap().count(), 2);
        output
    }
}

const FIELDS: &[&str] = &["function", "previous_name", "signature", "description"];

#[test]
fn every_index_defect_is_reported_in_original_order() {
    let f = Fixture::new(&["ozD.Alphaλ", "ozD.Bareλ", "ozD.Deltaλ", "ozD.Spareλ"]);
    f.write(
        FIELDS,
        &[
            &["oz.AlphaDλ", "ozD.Alphaλ", "AlphaDλ(x)", "Alpha"],
            &["oz.Betaλ", "ozD.Nopeλ", "Betaλ(x)", "Beta"],
            &["oz.AlphaEλ", "ozD.Alphaλ", "AlphaEλ(x)", "Alpha again"],
            &["Bareλ", "ozD.Bareλ", "Bareλ(x)", "Bare"],
            &["oz.Gammaλ", "ozD.Deltaλ", "Gammaλ(x)", "Gamma"],
            &["oz.Sigλ", "", "Sigλ(x", "Sig"],
            &["oz.Escλ", "", "Escλ(x)", "line_x000D_break"],
            &["oz.Arrowλ", "", "Arrowλ(x)", "→ Arrow"],
            &["oz.NoSigλ", "", "", "No signature"],
            &["oz.NoDescλ", "", "NoDescλ(x)", ""],
        ],
    );
    let expected = [
        "oz.Betaλ claims to replace ozD.Nopeλ, which baseline.txt never shipped",
        "ozD.Alphaλ is claimed by both oz.AlphaDλ and oz.AlphaEλ",
        "Bareλ is not a qualified function name",
        "oz.Gammaλ is not a renaming of ozD.Deltaλ: Gammaλ does not begin with Delta",
        "oz.Sigλ publishes a signature with unbalanced brackets: Sigλ(x",
        "oz.Escλ's description carries a raw OOXML escape: line_x000D_break",
        "oz.Arrowλ's description starts with the help table's column delimiter",
        "oz.NoSigλ publishes an empty signature",
        "oz.NoDescλ publishes an empty description",
        "ozD.Spareλ shipped in the baseline and no function claims it",
    ];
    let result = f.run(1);
    assert_eq!(result.code, 1);
    assert!(result.err.is_empty(), "{:?}", result.err);
    assert_eq!(
        result.out,
        std::iter::once(format!("FAIL: 10 problem(s) in {}", display(&f.index)))
            .chain(expected.iter().map(|e| format!("  - {e}")))
            .collect::<Vec<_>>()
    );
}

#[test]
fn unreadable_index_refusals_precede_the_floor_check() {
    let f = Fixture::new(&["ozD.Alphaλ"]);
    for (fields, rows, floor, expected) in [
        (
            FIELDS,
            vec![],
            1,
            format!("no rows read from {}", display(&f.index)),
        ),
        (
            &["function", "signature"][..],
            vec![&["oz.Alphaλ", "Alphaλ(x)"][..]],
            1,
            format!("{} has no previous_name column", display(&f.index)),
        ),
        (
            FIELDS,
            vec![&["oz.Alphaλ", "ozD.Alphaλ", "Alphaλ(x)", "Alpha"][..]],
            100,
            "only 1 functions and 1 baseline names read: the check is not reading them".to_owned(),
        ),
    ] {
        f.write(fields, &rows);
        let result = f.run(floor);
        assert_eq!(result.code, 1);
        assert!(result.out.is_empty(), "{:?}", result.out);
        assert_eq!(result.err, [expected]);
    }
}
