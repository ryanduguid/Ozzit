// Copyright 2026 Ryan Duguid
// SPDX-License-Identifier: Apache-2.0

//! Worksheets 1-6: Read me, Allocation, Contractor, Vendor finance, Mixed loan, Rental.
use super::model::{FMT_DEC2, FMT_INT, FMT_MONEY, FMT_PCT, FMT_TEXT, PLATINUM, Sheet, col_name};

const README_ROWS: [(&str, &str); 21] = [
    (
        "Allocation",
        "Allocate one budget across 20 people or jobs equally, by salary, or by eight weighted factors.",
    ),
    (
        "Contractor",
        "Compare employer cost with contractor revenue per billable week and hour.",
    ),
    (
        "Vendor finance",
        "Roll annual vendor debt forward using dividends, a cash reserve and own funds.",
    ),
    (
        "Mixed loan",
        "Allocate interest, fees and repayments across six uses and 36 dated events.",
    ),
    (
        "Rental",
        "Reconcile rent, operating costs, capital spending, three loans and up to five owners.",
    ),
    (
        "Packaging",
        "Compare employee cash pay and employer cost for up to five benefits, using supplied tax amounts.",
    ),
    (
        "Amortisation",
        "Allocate 11 cost cohorts across 11 annual periods with a supplied early write-off year.",
    ),
    (
        "Timeline",
        "Show 12 dated intervals across 24 months and measure overlap with a reporting window.",
    ),
    (
        "Deed register",
        "Record deed evidence for 20 trusts and flag vesting dates within five calendar years.",
    ),
    (
        "Carry forward",
        "Consume supplied annual capacity and five expiring buckets, oldest first.",
    ),
    (
        "Loan offers",
        "Compare eight fixed-rate offers including balloons, fees and a supplied rate shock.",
    ),
    (
        "Break even",
        "Calculate contribution, break-even units, target profit and capacity constraints.",
    ),
    (
        "Working capital",
        "Estimate cash released by changes in receivable, inventory and payable days.",
    ),
    (
        "Construction WIP",
        "Reconcile approved project progress, forecast costs, billings and collections.",
    ),
    (
        "KPI awards",
        "Calculate individual target bonuses using supplied scores, thresholds and caps.",
    ),
    (
        "Asset disposal",
        "Bridge supplied sale, book-value, replacement and GST amounts to profit and cash.",
    ),
    (
        "Exit pay",
        "Calculate gross and net pay from approved leave, notice and other entitlement inputs.",
    ),
    (
        "Tax cash",
        "Track assessed liabilities, payments, refunds and credits across twelve entities.",
    ),
    (
        "Investment",
        "Project ten annual periods of investment value, debt and after-tax cash flows.",
    ),
    (
        "Action register",
        "Track owners, supplied deadlines, evidence and completion of review actions.",
    ),
    (
        "Benefit totals",
        "Gross up reviewer-classified type 1 and type 2 taxable values using supplied FBT rates.",
    ),
];

pub fn read_me(sheet_names: &[&'static str]) -> Sheet {
    let mut s = Sheet::new("Read me", "Ozzit practice calculators", "N", 48);
    s.tab_selected = true;
    s.tab_color = PLATINUM;
    s.cell_text(
        "B5",
        "Twenty-one standalone modelling workflows using ordinary Excel formulas.",
    );
    s.cell_text(
        "B7",
        "Open a tab and replace its fabricated inputs. No Ozzit LAMBDA import is needed.",
    );
    s.cell_text(
        "B9",
        "Tax treatment, rates, eligibility and legal deadlines must be supplied by the reviewer.",
    );
    s.cell_text(
        "B10",
        "This workbook does not reproduce the purchased BTS workbooks or their hidden rules.",
    );
    s.header(13, &["Calculator", "What it calculates"], 2);
    s.set_col_width_range(3, 3, 100);
    for (i, (name, desc)) in README_ROWS.iter().enumerate() {
        let row = 14 + i as u16;
        debug_assert_eq!(sheet_names[i + 1], *name);
        s.cell_text(&format!("B{row}"), name);
        s.cell_text(&format!("C{row}"), desc);
    }
    s.note(
        "B36",
        "Sources and coverage: docs/between-spreadsheets.md and docs/practice-improvements.md.",
    );
    s.note(
        "B38",
        "Sample rates and capacities are modelling assumptions, not statutory rates or limits.",
    );
    s.note(
        "B40",
        "Fixed ranges: extend formulas and native checks if you add rows. Inputs remain unrounded.",
    );
    s
}

pub fn allocation() -> Sheet {
    let mut s = Sheet::new("Allocation", "Allocation", "T", 48);
    allocation_top(&mut s);
    let scores: Vec<String> = (1..=8).map(|i| format!("Score {i}")).collect();
    let mut cols = vec!["Person / job", "Salary / base"];
    cols.extend(scores.iter().map(String::as_str));
    cols.extend([
        "Weighted score",
        "Allocation weight",
        "Cash bonus",
        "Super cost",
        "Payroll cost",
        "Total cost",
        "Withholding",
        "Net bonus",
        "Input status",
    ]);
    s.header(13, &cols, 2);
    allocation_rows(&mut s);
    s.set_num_fmt_range("N14:S35", FMT_MONEY);
    s.cell_text("B35", "Total");
    for c in ["N", "O", "P", "Q", "R", "S"] {
        s.formula(&format!("{c}35"), &format!("=SUM({c}14:{c}33)"));
    }
    s.flag("D9", "Ready");
    s.flag("T14:T33", "Ready");
    s.note("B38", "Set both on-cost rates to zero for ordinary overhead allocations. Withholding is supplied, not calculated.");
    s.freeze = Some((2, 13));
    s
}

fn allocation_top(s: &mut Sheet) {
    s.pair_num(5, "Total employer budget", 12000.0, FMT_MONEY);
    s.pair_num(6, "Super cost / cash bonus", 0.12, FMT_PCT);
    s.pair_num(7, "Payroll cost / bonus + super", 0.05, FMT_PCT);
    s.pair_num(8, "Method: 1 equal, 2 salary, 3 score", 3.0, FMT_INT);
    s.list_validation("D8", &["1", "2", "3"]);
    let factors: Vec<String> = (1..=8).map(|i| format!("Factor {i}")).collect();
    let f_refs: Vec<&str> = factors.iter().map(String::as_str).collect();
    s.header(5, &f_refs, 10);
    for c in 10..=17 {
        s.input_num(&format!("{}6", col_name(c)), 1.0, FMT_DEC2);
    }
    s.note(
        "J8",
        "Scores use a common scale. Factor weights are relative, nonnegative inputs.",
    );
    s.result(9, "Input status", "=IF(AND(COUNT(D5:D8)=4,MIN(D5:D7)>=0,D8=INT(D8),D8>=1,D8<=3,OR(D8<>3,AND(COUNT(J6:Q6)=8,MIN(J6:Q6)>=0)),COUNTA(B14:B33)>0,COUNTIF(T14:T33,\"Ready\")=COUNTA(B14:B33)),IF(SUM(M14:M33)>0,\"Ready\",\"Zero allocation weight\"),\"Check inputs\")", FMT_TEXT);
    s.result(
        10,
        "Cash bonus pool",
        "=IF(D9=\"Ready\",D5/((1+D6)*(1+D7)),NA())",
        FMT_MONEY,
    );
}

fn allocation_rows(s: &mut Sheet) {
    for r in 14..=33 {
        let name = if r < 17 {
            format!("Example {}", r - 13)
        } else {
            String::new()
        };
        s.input_text(&format!("B{r}"), &name, FMT_TEXT);
        s.input_num(
            &format!("C{r}"),
            if r < 17 {
                f64::from(r - 13) * 50000.0
            } else {
                0.0
            },
            FMT_MONEY,
        );
        for c in 4..=11 {
            s.input_num(
                &format!("{}{r}", col_name(c)),
                if r < 17 { f64::from(r - 13) } else { 0.0 },
                FMT_DEC2,
            );
        }
        s.input_num(&format!("R{r}"), 0.0, FMT_MONEY);
        s.formula(&format!("T{r}"), &format!("=IF(B{r}=\"\",\"\",IF(AND(OR($D$8=1,AND($D$8=2,COUNT(C{r})=1,C{r}>=0),AND($D$8=3,COUNT(D{r}:K{r})=8,MIN(D{r}:K{r})>=0)),COUNT(R{r})=1,R{r}>=0,SUMPRODUCT(--($B$14:$B$33=B{r}))=1),\"Ready\",\"Check inputs\"))"));
        s.formula(&format!("L{r}"), &format!("=IF(B{r}=\"\",\"\",IF($D$8=3,IF(T{r}=\"Ready\",SUMPRODUCT(D{r}:K{r},$J$6:$Q$6),NA()),\"\"))"));
        s.formula(&format!("M{r}"), &format!("=IF(B{r}=\"\",\"\",IF(AND(T{r}=\"Ready\",$D$8>=1,$D$8<=3,$D$8=INT($D$8)),CHOOSE($D$8,1,C{r},L{r}),NA()))"));
        s.formula(
            &format!("N{r}"),
            &format!("=IF(B{r}=\"\",\"\",IF($D$9=\"Ready\",$D$10*M{r}/SUM($M$14:$M$33),NA()))"),
        );
        s.formula(&format!("O{r}"), &format!("=IF(B{r}=\"\",\"\",N{r}*$D$6)"));
        s.formula(
            &format!("P{r}"),
            &format!("=IF(B{r}=\"\",\"\",(N{r}+O{r})*$D$7)"),
        );
        s.formula(
            &format!("Q{r}"),
            &format!("=IF(B{r}=\"\",\"\",SUM(N{r}:P{r}))"),
        );
        s.formula(
            &format!("S{r}"),
            &format!("=IF(B{r}=\"\",\"\",IF(R{r}<=N{r},N{r}-R{r},NA()))"),
        );
    }
}

const CONTRACTOR_INPUTS: [(&str, f64); 8] = [
    ("Employee cash salary", 100000.0),
    ("Employee employer super", 12000.0),
    ("Employee other on-costs", 8000.0),
    ("Contractor retirement saving", 12000.0),
    ("Contractor annual overheads", 10000.0),
    ("Contractor insurance", 2000.0),
    ("Billable weeks per year", 42.0),
    ("Billable hours per week", 35.0),
];

pub fn contractor() -> Sheet {
    let mut s = Sheet::new("Contractor", "Contractor", "N", 48);
    for (i, (label, val)) in CONTRACTOR_INPUTS.iter().enumerate() {
        s.pair_num(
            i as u16 + 5,
            label,
            *val,
            if i >= 6 { FMT_DEC2 } else { FMT_MONEY },
        );
    }
    s.result(
        14,
        "Input status",
        "=IF(AND(COUNT(D5:D12)=8,MIN(D5:D10)>=0,D11>0,D12>0),\"Ready\",\"Check inputs\")",
        FMT_TEXT,
    );
    s.result(
        16,
        "Annual employer cost",
        "=IF(D14=\"Ready\",SUM(D5:D7),NA())",
        FMT_MONEY,
    );
    s.result(
        17,
        "Contractor annual revenue",
        "=IF(D14=\"Ready\",D5+SUM(D8:D10),NA())",
        FMT_MONEY,
    );
    s.result(18, "Revenue per billable week", "=D17/D11", FMT_MONEY);
    s.result(19, "Revenue per billable hour", "=D18/D12", FMT_MONEY);
    s.note("F5", "Amounts exclude GST and personal income tax.");
    s.note(
        "F7",
        "Retirement saving is entered separately for each arrangement.",
    );
    s.note(
        "F9",
        "Billable weeks exclude leave and time that cannot be invoiced.",
    );
    s.note(
        "F11",
        "This comparison does not determine employment status.",
    );
    s.flag("D14", "Ready");
    s
}

const VENDOR_HEADERS: [&str; 12] = [
    "Year",
    "Opening debt",
    "Annual rate",
    "Interest",
    "Dividends",
    "Cash reserve",
    "Own funds",
    "Cash available",
    "Debt payment",
    "Closing debt",
    "Excess cash",
    "Input status",
];

pub fn vendor_finance() -> Sheet {
    let mut s = Sheet::new("Vendor finance", "Vendor finance", "N", 48);
    s.pair_num(5, "Initial vendor debt", 100000.0, FMT_MONEY);
    s.note(
        "F5",
        "Annual interest is charged on opening debt. Cash is paid at year end.",
    );
    s.note(
        "F7",
        "The reserve is a supplied cash amount, not an income tax calculation.",
    );
    s.header(10, &VENDOR_HEADERS, 2);
    for r in 11..=20 {
        vendor_row(&mut s, r);
    }
    s.set_num_fmt_range("C11:L20", FMT_MONEY);
    s.set_num_fmt_range("D11:D20", FMT_PCT);
    s.flag("M11:M20", "Ready");
    s.result(23, "Debt after ten years", "=K20", FMT_MONEY);
    s
}

fn vendor_row(s: &mut Sheet, r: u16) {
    s.cell_num(&format!("B{r}"), f64::from(r - 10));
    s.input_num(&format!("D{r}"), 0.06, FMT_PCT);
    s.input_num(&format!("F{r}"), 14000.0, FMT_MONEY);
    s.input_num(&format!("G{r}"), 3500.0, FMT_MONEY);
    s.input_num(&format!("H{r}"), 2000.0, FMT_MONEY);
    let open_f = if r == 11 {
        "=$D$5".to_owned()
    } else {
        format!("=K{}", r - 1)
    };
    s.formula(&format!("C{r}"), &open_f);
    let prev = if r > 11 {
        format!(",M{}=\"Ready\"", r - 1)
    } else {
        String::new()
    };
    s.formula(&format!("M{r}"), &format!("=IF(AND(COUNT(D{r},F{r}:H{r},$D$5)=5,MIN(D{r},F{r}:H{r},$D$5)>=0,G{r}<=F{r}{prev}),\"Ready\",\"Check inputs\")"));
    s.formula(
        &format!("E{r}"),
        &format!("=IF(M{r}=\"Ready\",C{r}*D{r},NA())"),
    );
    s.formula(
        &format!("I{r}"),
        &format!("=IF(M{r}=\"Ready\",F{r}-G{r}+H{r},NA())"),
    );
    s.formula(&format!("J{r}"), &format!("=MIN(C{r}+E{r},I{r})"));
    s.formula(&format!("K{r}"), &format!("=C{r}+E{r}-J{r}"));
    s.formula(&format!("L{r}"), &format!("=I{r}-J{r}"));
}

pub fn mixed_loan() -> Sheet {
    let mut s = Sheet::new("Mixed loan", "Mixed loan", "AW", 54);
    s.pair_date(5, "Opening date", 2026, 7, 1);
    s.pair_num(6, "Annual simple interest rate", 0.06, FMT_PCT);
    s.pair_num(7, "Days per interest year", 365.0, FMT_INT);
    let uses: Vec<String> = (1..=6).map(|i| format!("Use {i}")).collect();
    let use_refs: Vec<&str> = uses.iter().map(String::as_str).collect();
    s.header(5, &use_refs, 6);
    for c in 6..=11 {
        s.input_num(
            &format!("{}6", col_name(c)),
            if c == 6 {
                60000.0
            } else if c == 7 {
                40000.0
            } else {
                0.0
            },
            FMT_MONEY,
        );
    }
    mixed_loan_summary(&mut s);
    mixed_loan_headers(&mut s);
    for r in 16..=51 {
        mixed_loan_inputs(&mut s, r);
        mixed_loan_formulas(&mut s, r);
    }
    s.set_num_fmt_range("C16:AP51", FMT_MONEY);
    s.set_num_fmt_range("K16:K51", FMT_INT);
    s.set_num_fmt_range("AR16:AW51", FMT_MONEY);
    s.flag("AQ16:AQ51", "Ready");
    s.freeze = Some((3, 15));
    s
}

fn mixed_loan_summary(s: &mut Sheet) {
    s.note(
        "F8",
        "Six opening balances above. Uses are labels, not tax classifications.",
    );
    s.note("B10", "Interest accrues first. Draws and signed purpose adjustments precede proportional repayments and fee allocation.");
    s.note("B11", "Interest and fees are reported separately and are not capitalised. Same-day events accrue zero days.");
    s.note("B14", "Interest stops at the final event. AR:AW are principal adjustments, not cash flows. Use zero-sum entries for transfers between uses.");
    s.result(12, "Final principal", "=SUM(X51:AC51)", FMT_MONEY);
    s.result(13, "Total allocated interest", "=SUM(R16:W51)", FMT_MONEY);
    s.cell_text("F12", "Input status");
    s.formula(
        "H12",
        "=IF(COUNTIF(AQ16:AQ51,\"Ready\")=36,\"Ready\",\"Check events\")",
    );
    s.flag("H12", "Ready");
    s.cell_text("F13", "Unapplied repayments");
    s.formula("H13", "=SUM(AP16:AP51)");
    s.cell_mut((13, 8)).style.num_fmt = Some(FMT_MONEY);
}

fn mixed_loan_headers(s: &mut Sheet) {
    let mut labels: Vec<String> = vec!["Event date".into(), "Repayment".into()];
    for i in 1..=6 {
        labels.push(format!("Draw {i}"));
    }
    labels.extend(["Fee".into(), "Days".into()]);
    for prefix in [
        "Opening",
        "Interest",
        "Closing",
        "Fee share",
        "Repayment share",
    ] {
        for i in 1..=6 {
            labels.push(format!("{prefix} {i}"));
        }
    }
    labels.extend(["Unapplied cash".into(), "Input status".into()]);
    for i in 1..=6 {
        labels.push(format!("Adjustment {i}"));
    }
    let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
    s.header(15, &refs, 2);
}

fn mixed_loan_inputs(s: &mut Sheet, r: u16) {
    let i = i32::from(r - 16);
    s.input_date(&format!("B{r}"), 2026, 7 + i / 3, 1 + (i % 3) * 10);
    s.input_num(
        &format!("C{r}"),
        if i == 1 { 10000.0 } else { 0.0 },
        FMT_MONEY,
    );
    for c in 4..=9 {
        s.input_num(
            &format!("{}{r}", col_name(c)),
            if i == 2 && c == 5 { 10000.0 } else { 0.0 },
            FMT_MONEY,
        );
    }
    s.input_num(
        &format!("J{r}"),
        if i == 1 { 100.0 } else { 0.0 },
        FMT_MONEY,
    );
    for c in 44..=49 {
        s.input_num(&format!("{}{r}", col_name(c)), 0.0, FMT_MONEY);
    }
}

fn mixed_loan_formulas(s: &mut Sheet, r: u16) {
    let prior = if r == 16 {
        "$D$5".to_owned()
    } else {
        format!("B{}", r - 1)
    };
    let balances = (0..6u16)
        .map(|u| {
            format!(
                "{}{r}+{}{r}+{}{r}",
                col_name(12 + u),
                col_name(4 + u),
                col_name(44 + u)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let prev = if r > 16 {
        format!(",AQ{}<>\"Ready\"", r - 1)
    } else {
        String::new()
    };
    s.formula(&format!("AQ{r}"), &format!("=IF(OR(COUNT(B{r}:J{r},AR{r}:AW{r})<>15,COUNT($D$5:$D$7)<>3,COUNT($F$6:$K$6)<>6{prev}),\"Check inputs\",IF(AND(MIN(C{r}:J{r},$D$6,$F$6:$K$6)>=0,MIN({balances})>=0,$D$7>0,B{r}>={prior},B{r}=INT(B{r}),$D$5=INT($D$5),OR(SUM(L{r}:Q{r},D{r}:I{r},AR{r}:AW{r})>0,J{r}=0)),\"Ready\",\"Check inputs\"))"));
    s.formula(
        &format!("K{r}"),
        &format!("=IF(AQ{r}=\"Ready\",B{r}-{prior},NA())"),
    );
    for u in 0..6u16 {
        mixed_loan_use_formulas(s, r, u);
    }
    s.formula(
        &format!("AP{r}"),
        &format!("=IF(AQ{r}=\"Ready\",C{r}-SUM(AJ{r}:AO{r}),NA())"),
    );
}

fn mixed_loan_use_formulas(s: &mut Sheet, r: u16, u: u16) {
    let basis = format!("SUM(L{r}:Q{r},D{r}:I{r},AR{r}:AW{r})");
    let open = format!("{}{r}", col_name(12 + u));
    let draw = format!("{}{r}", col_name(4 + u));
    let adj = format!("{}{r}", col_name(44 + u));
    let rep = format!("{}{r}", col_name(36 + u));
    let open_expr = if r == 16 {
        format!("={}6", col_name(6 + u))
    } else {
        format!("={}{}", col_name(24 + u), r - 1)
    };
    s.formula(&open, &open_expr);
    s.formula(
        &format!("{}{r}", col_name(18 + u)),
        &format!("=IF(AQ{r}=\"Ready\",{open}*$D$6*K{r}/$D$7,NA())"),
    );
    s.formula(&rep, &format!("=IF(AQ{r}=\"Ready\",IF({basis}=0,0,MIN(C{r},{basis})*({open}+{draw}+{adj})/{basis}),NA())"));
    s.formula(&format!("{}{r}", col_name(30 + u)), &format!("=IF(AQ{r}=\"Ready\",IF({basis}=0,IF(J{r}=0,0,NA()),J{r}*({open}+{draw}+{adj})/{basis}),NA())"));
    s.formula(
        &format!("{}{r}", col_name(24 + u)),
        &format!("=IF(AQ{r}=\"Ready\",{open}+{draw}+{adj}-{rep},NA())"),
    );
}

const RENTAL_NOTES: [(&str, &str); 7] = [
    ("H17", "Capital portions include their GST."),
    ("H18", "Column E: GST credits on operating costs only."),
    (
        "H20",
        "The result excludes depreciation and tax adjustments.",
    ),
    ("H21", "Supply the GST and expense treatment."),
    ("H23", "Principal affects cash only."),
    ("H24", "Funds held bridge cash; they are not an expense."),
    ("H26", "GST settlements affect cash, not operating profit."),
];

pub fn rental() -> Sheet {
    let mut s = Sheet::new("Rental", "Rental", "N", 48);
    s.pair_num(5, "Gross cash rent", 50000.0, FMT_MONEY);
    s.pair_num(6, "GST assessed on rent", 0.0, FMT_MONEY);
    s.pair_num(7, "Opening funds held by agent", 1000.0, FMT_MONEY);
    s.pair_num(8, "Closing funds held by agent", 3500.0, FMT_MONEY);
    s.pair_num(9, "Other net cash receipts", 0.0, FMT_MONEY);
    s.pair_num(10, "GST cash paid / (refund)", 0.0, FMT_MONEY);
    s.result(11, "Net operating result", "=IF(AND(COUNT(D5:D6)=2,MIN(D5:D6)>=0,D6<=D5,COUNT(C32:C34)=3,MIN(C32:C34)>=0),D5-D6-SUM(F17:F28)-SUM(C32:C34),NA())", FMT_MONEY);
    s.result(12, "Cash before income tax", "=IF(AND(ISNUMBER(D11),COUNT(D7:D10)=4,MIN(D7:D8)>=0,COUNT(D32:D34)=3,MIN(D32:D34)>=0),D5-SUM(C17:C28)-SUM(C32:D34)-(D8-D7)+D9-D10,NA())", FMT_MONEY);
    s.result(13, "Ownership share status", "=IF(AND(COUNTA(B39:B43)>0,COUNT(C39:C43)=COUNTA(B39:B43),MIN(C39:C43)>=0,ABS(SUM(C39:C43)-1)<0.00000001,COUNTIF(F39:F43,\"Ready\")=COUNTA(B39:B43)),\"Ready\",\"Check shares\")", FMT_TEXT);
    rental_expenses(&mut s);
    rental_loans(&mut s);
    rental_owners_and_notes(&mut s);
    s
}

fn rental_expenses(s: &mut Sheet) {
    s.header(
        16,
        &[
            "Expense",
            "Cash paid",
            "Capital portion",
            "Operating GST credit",
            "Operating cost",
        ],
        2,
    );
    let names = ["Agent costs", "Repairs", "Other costs"];
    for (idx, r) in (17..=28).enumerate() {
        let label = names
            .get(idx)
            .map_or_else(|| format!("Expense {}", idx + 1), |v| (*v).to_owned());
        s.input_text(&format!("B{r}"), &label, FMT_TEXT);
        let cash = match r {
            17 => 5000.0,
            18 => 3000.0,
            19 => 2000.0,
            _ => 0.0,
        };
        let gst = match r {
            17 => 400.0,
            18 => 100.0,
            _ => 0.0,
        };
        s.input_num(&format!("C{r}"), cash, FMT_MONEY);
        s.input_num(
            &format!("D{r}"),
            if r == 18 { 1000.0 } else { 0.0 },
            FMT_MONEY,
        );
        s.input_num(&format!("E{r}"), gst, FMT_MONEY);
        s.formula(&format!("F{r}"), &format!("=IF(AND(COUNT(C{r}:E{r})=3,MIN(C{r}:E{r})>=0,SUM(D{r}:E{r})<=C{r}),C{r}-SUM(D{r}:E{r}),NA())"));
    }
}

fn rental_loans(s: &mut Sheet) {
    s.header(
        31,
        &["Loan", "Interest cash paid", "Principal cash paid"],
        2,
    );
    for r in 32..=34 {
        s.input_text(&format!("B{r}"), &format!("Loan {}", r - 31), FMT_TEXT);
        s.input_num(
            &format!("C{r}"),
            if r == 32 { 4000.0 } else { 0.0 },
            FMT_MONEY,
        );
        s.input_num(
            &format!("D{r}"),
            if r == 32 { 6000.0 } else { 0.0 },
            FMT_MONEY,
        );
    }
}

fn rental_owners_and_notes(s: &mut Sheet) {
    s.header(
        38,
        &[
            "Owner",
            "Ownership share",
            "Operating allocation",
            "Cash allocation",
            "Input status",
        ],
        2,
    );
    for r in 39..=43 {
        let label = if r < 41 {
            format!("Example owner {}", r - 38)
        } else {
            String::new()
        };
        s.input_text(&format!("B{r}"), &label, FMT_TEXT);
        match r {
            39 => s.input_num(&format!("C{r}"), 0.6, FMT_PCT),
            40 => s.input_num(&format!("C{r}"), 0.4, FMT_PCT),
            _ => s.input_empty(&format!("C{r}"), FMT_PCT),
        }
        s.formula(&format!("F{r}"), &format!("=IF(B{r}=\"\",IF(C{r}=\"\",\"\",\"Missing owner\"),IF(AND(ISNUMBER(C{r}),C{r}>=0,SUMPRODUCT(--($B$39:$B$43=B{r}))=1),\"Ready\",\"Check shares\"))"));
        s.formula(
            &format!("D{r}"),
            &format!("=IF(B{r}=\"\",\"\",IF($D$13=\"Ready\",$D$11*C{r},NA()))"),
        );
        s.formula(
            &format!("E{r}"),
            &format!("=IF(B{r}=\"\",\"\",IF($D$13=\"Ready\",$D$12*C{r},NA()))"),
        );
    }
    s.set_num_fmt_range("C17:F28", FMT_MONEY);
    s.set_num_fmt_range("C32:D34", FMT_MONEY);
    s.set_num_fmt_range("D39:E43", FMT_MONEY);
    for (addr, text) in RENTAL_NOTES {
        s.note(addr, text);
    }
    s.flag("D13", "Ready");
    s.flag("F39:F43", "Ready");
}
