// Copyright 2026 Ryan Duguid
// SPDX-License-Identifier: Apache-2.0

//! Worksheets 7-14: Packaging, Amortisation, Timeline, Deed register, Carry forward, Loan offers, Break even, Working capital.
use super::model::{
    BLACK, CRIMSON, FMT_DATE, FMT_DEC2, FMT_INT, FMT_MMM_YY, FMT_MONEY, FMT_PCT, FMT_TEXT,
    PALE_CRIMSON, PALE_PURPLE, PURPLE, Sheet, col_name,
};

pub fn packaging() -> Sheet {
    let mut s = Sheet::new("Packaging", "Packaging", "N", 48);
    s.pair_num(5, "Cash salary before packaging", 100000.0, FMT_MONEY);
    s.pair_num(6, "Assessed tax before packaging", 25000.0, FMT_MONEY);
    s.pair_num(7, "Employer on-costs before", 20000.0, FMT_MONEY);
    s.pair_num(8, "Assessed tax after packaging", 22000.0, FMT_MONEY);
    s.pair_num(9, "Employer on-costs after", 18000.0, FMT_MONEY);
    s.result(11, "Input status", "=IF(AND(COUNT(D5:D9)=5,MIN(D5:D9)>=0,COUNT(C19:E23)=15,MIN(C19:E23)>=0,SUM(D19:D23)<=D5,D6<=D5,D8+SUM(E19:E23)<=D5-SUM(D19:D23)),\"Ready\",\"Check inputs\")", FMT_TEXT);
    s.result(
        12,
        "Employee cash before",
        "=IF(D11=\"Ready\",D5-D6,NA())",
        FMT_MONEY,
    );
    s.result(
        13,
        "Employee cash after",
        "=IF(D11=\"Ready\",D5-SUM(D19:D23)-D8-SUM(E19:E23),NA())",
        FMT_MONEY,
    );
    s.result(
        14,
        "Employer cost before",
        "=IF(D11=\"Ready\",D5+D7,NA())",
        FMT_MONEY,
    );
    s.result(
        15,
        "Employer cost after",
        "=IF(D11=\"Ready\",D5-SUM(D19:D23)+SUM(C19:C23)+D9-SUM(E19:E23),NA())",
        FMT_MONEY,
    );
    packaging_benefits(&mut s);
    packaging_reminders(&mut s);
    s
}

fn packaging_benefits(s: &mut Sheet) {
    s.header(
        18,
        &[
            "Benefit",
            "Employer cash cost",
            "Pre-tax salary reduction",
            "Post-tax contribution",
        ],
        2,
    );
    for r in 19..=23 {
        let label = if r == 19 {
            "Example lease".to_owned()
        } else {
            format!("Benefit {}", r - 18)
        };
        s.input_text(&format!("B{r}"), &label, FMT_TEXT);
        s.input_num(
            &format!("C{r}"),
            if r == 19 { 10000.0 } else { 0.0 },
            FMT_MONEY,
        );
        s.input_num(
            &format!("D{r}"),
            if r == 19 { 10000.0 } else { 0.0 },
            FMT_MONEY,
        );
        s.input_num(
            &format!("E{r}"),
            if r == 19 { 2000.0 } else { 0.0 },
            FMT_MONEY,
        );
    }
    s.note("F5", "Supply assessed tax and total on-costs for each arrangement. No tax or FBT rates are inferred.");
    s.note(
        "F7",
        "Employer benefit costs must use one consistent GST basis.",
    );
    s.note(
        "F9",
        "Post-tax employee contributions are cash received by the employer.",
    );
    s.flag("D11", "Ready");
}

fn packaging_reminders(s: &mut Sheet) {
    s.cell_text("F11", "Review date");
    s.input_date("H11", 2026, 10, 19);
    s.header(
        26,
        &[
            "Benefit",
            "Commencement",
            "Cessation",
            "Days remaining",
            "Timing reminder",
        ],
        2,
    );
    for r in 27..=31 {
        s.formula(&format!("B{r}"), &format!("=B{}", r - 8));
        s.input_date(&format!("C{r}"), 2026, 7, 1);
        s.input_date(&format!("D{r}"), 2027, 6, 30);
        let valid = format!(
            "IF(COUNT(C{r}:D{r},$H$11)=3,AND(MIN(C{r}:D{r},$H$11)>0,C{r}=INT(C{r}),D{r}=INT(D{r}),$H$11=INT($H$11),D{r}>=C{r}),FALSE)"
        );
        s.formula(
            &format!("E{r}"),
            &format!("=IF({valid},MAX(0,D{r}-$H$11),NA())"),
        );
        s.formula(&format!("F{r}"), &format!("=IF({valid},IF($H$11>D{r},\"Cease date passed\",IF($H$11<C{r},\"Not started\",IF(D{r}-$H$11<=30,\"Ends within 30 days\",\"Active\"))),\"Check dates\")"));
    }
    s.set_col_width_range(3, 4, 18);
    s.set_col_width_range(6, 6, 24);
    s.set_h_align_range("F27:F31", "center");
    s.cf_expr(
        "F27:F31",
        "OR(F27=\"Check dates\",F27=\"Cease date passed\",F27=\"Ends within 30 days\")",
        CRIMSON,
        PALE_CRIMSON,
        true,
    );
    s.note("B34", "Supplied dates create diary reminders only. Annual amounts above are not automatically prorated.");
}

pub fn amortisation() -> Sheet {
    let mut s = Sheet::new("Amortisation", "Amortisation", "V", 48);
    s.pair_num(5, "First annual period", 2026.0, FMT_INT);
    s.note("F5", "Supply the life, first-period fraction in V and any early write-off year. A partial first period extends recovery into a final period.");
    let years: Vec<String> = (0..11).map(|i| (2026 + i).to_string()).collect();
    let mut hdr = vec![
        "Cost cohort",
        "Cost",
        "Residual",
        "Life in years",
        "First period",
        "Early write-off year",
    ];
    hdr.extend(years.iter().map(String::as_str));
    hdr.extend(["Allocated in horizon", "Closing book value", "Input status"]);
    s.header(8, &hdr, 2);
    for c in 8..=18 {
        s.formula(&format!("{}8", col_name(c)), &format!("=$D$5+{}", c - 8));
    }
    s.header(8, &["First period fraction"], 22);
    for r in 9..=19 {
        amortisation_row(&mut s, r);
    }
    s.set_num_fmt_range("H9:T19", FMT_MONEY);
    s.flag("U9:U19", "Ready");
    s.freeze = Some((2, 8));
    s
}

fn amortisation_row(s: &mut Sheet, r: u16) {
    s.input_text(
        &format!("B{r}"),
        &format!("Example cohort {}", r - 8),
        FMT_TEXT,
    );
    let cost = match r {
        9 => 5000.0,
        10 => 2000.0,
        11 => 9000.0,
        _ => 0.0,
    };
    let life = match r {
        9 => 5.0,
        10 => 4.0,
        _ => 3.0,
    };
    s.input_num(&format!("C{r}"), cost, FMT_MONEY);
    s.input_num(&format!("D{r}"), 0.0, FMT_MONEY);
    s.input_num(&format!("E{r}"), life, FMT_INT);
    s.input_num(
        &format!("F{r}"),
        if r == 11 { 2028.0 } else { 2026.0 },
        FMT_INT,
    );
    s.input_num(
        &format!("G{r}"),
        if r == 10 { 2028.0 } else { 0.0 },
        FMT_INT,
    );
    s.input_num(&format!("V{r}"), 1.0, FMT_PCT);
    s.formula(&format!("U{r}"), &format!("=IF(AND(COUNT(C{r}:G{r},V{r})=6,MIN(C{r}:E{r})>=0,E{r}>0,E{r}=INT(E{r}),D{r}<=C{r},F{r}=INT(F{r}),G{r}=INT(G{r}),OR(G{r}=0,G{r}>=F{r}),COUNT($D$5)=1,$D$5=INT($D$5),F{r}>=$D$5,F{r}<=$D$5+10,V{r}>=0,V{r}<=1),\"Ready\",\"Check inputs\")"));
    for c in 8..=18 {
        let y = format!("{}$8", col_name(c));
        let prior = if c == 8 {
            "0".to_owned()
        } else {
            format!("SUM(H{r}:{}{r})", col_name(c - 1))
        };
        let rem = format!("MAX(0,$C{r}-$D{r}-{prior})");
        s.formula(&format!("{}{r}", col_name(c)), &format!("=IF($U{r}=\"Ready\",IF(OR({y}<$F{r},{y}>$F{r}+$E{r},AND($G{r}>0,{y}>$G{r})),0,IF(AND($G{r}>0,{y}=$G{r}),{rem},MIN({rem},($C{r}-$D{r})/$E{r}*IF({y}=$F{r},$V{r},1)))),NA())"));
    }
    s.formula(&format!("S{r}"), &format!("=SUM(H{r}:R{r})"));
    s.formula(&format!("T{r}"), &format!("=MAX(D{r},C{r}-S{r})"));
}

pub fn timeline() -> Sheet {
    let mut s = Sheet::new("Timeline", "Timeline", "AF", 48);
    s.tab_color = BLACK;
    s.pair_date(5, "Reporting window start", 2026, 7, 1);
    s.pair_date(6, "Reporting window end", 2027, 6, 30);
    let months: Vec<String> = (1..=24).map(|i| format!("Month {i}")).collect();
    let mut hdr = vec![
        "Interval",
        "Start",
        "End",
        "Inclusive days",
        "Days in window",
        "Purpose / note",
        "Evidence",
    ];
    hdr.extend(months.iter().map(String::as_str));
    s.header(9, &hdr, 2);
    for c in 9..=32 {
        let addr = format!("{}9", col_name(c));
        s.formula(
            &addr,
            &format!("=EDATE(DATE(YEAR($D$5),MONTH($D$5),1),{})", c - 9),
        );
        s.cell_mut((9, c)).style.num_fmt = Some(FMT_MMM_YY);
    }
    timeline_rows(&mut s);
    s.set_col_width_range(9, 32, 9);
    s.cf_cell_eq("I10:AF21", "1", PURPLE, PALE_PURPLE);
    s.note("B24", "Intervals and overlap use inclusive calendar days. Overlapping intervals are not automatically combined.");
    s.note("B26", "No residence exemption, CGT choice, estate obligation or statutory deadline is determined.");
    s.freeze = Some((2, 9));
    s
}

fn timeline_rows(s: &mut Sheet) {
    for r in 10..=21 {
        let label = match r {
            10 => "Example occupation",
            11 => "Example rental",
            _ => "",
        };
        s.input_text(&format!("B{r}"), label, FMT_TEXT);
        match r {
            10 => {
                s.input_date(&format!("C{r}"), 2026, 1, 1);
                s.input_date(&format!("D{r}"), 2026, 9, 30);
            }
            11 => {
                s.input_date(&format!("C{r}"), 2026, 10, 1);
                s.input_date(&format!("D{r}"), 2027, 9, 30);
            }
            _ => {
                s.input_empty(&format!("C{r}"), FMT_DATE);
                s.input_empty(&format!("D{r}"), FMT_DATE);
            }
        }
        s.input_empty(&format!("G{r}"), FMT_TEXT);
        s.input_empty(&format!("H{r}"), FMT_TEXT);
        let valid = format!("AND(COUNT(C{r}:D{r},$D$5:$D$6)=4,D{r}>=C{r},$D$6>=$D$5)");
        s.formula(
            &format!("E{r}"),
            &format!("=IF(B{r}=\"\",\"\",IF({valid},D{r}-C{r}+1,NA()))"),
        );
        s.formula(
            &format!("F{r}"),
            &format!("=IF(B{r}=\"\",\"\",IF({valid},MAX(0,MIN(D{r},$D$6)-MAX(C{r},$D$5)+1),NA()))"),
        );
        for c in 9..=32 {
            let col = col_name(c);
            s.formula(&format!("{col}{r}"), &format!("=IF($B{r}=\"\",\"\",IF(ISNUMBER($E{r}),IF(AND($C{r}<=EOMONTH({col}$9,0),$D{r}>={col}$9),1,\"\"),NA()))"));
        }
    }
}

const DEED_HEADERS: [&str; 12] = [
    "Trust",
    "Deed held",
    "Signed",
    "Stamped",
    "Trustee",
    "Appointor",
    "Vesting date",
    "Vesting review",
    "FTE record",
    "IEE record",
    "NSW amendment record",
    "Evidence",
];

pub fn deed_register() -> Sheet {
    let mut s = Sheet::new("Deed register", "Deed register", "N", 48);
    s.tab_color = BLACK;
    s.pair_date(5, "Review date", 2026, 10, 18);
    s.header(8, &DEED_HEADERS, 2);
    for r in 9..=28 {
        for c in 2..=13 {
            if c != 9 {
                s.input_empty(&format!("{}{r}", col_name(c)), FMT_TEXT);
            }
        }
        if r == 9 {
            s.input_text("B9", "Example trust", FMT_TEXT);
            s.input_date("H9", 2030, 10, 18);
        } else {
            s.cell_mut((r, 8)).style.num_fmt = Some(FMT_DATE);
        }
        s.formula(&format!("I{r}"), &format!("=IF(B{r}=\"\",\"\",IF(AND(ISNUMBER(H{r}),ISNUMBER($D$5)),IF(H{r}<$D$5,\"Past vesting date\",IF(H{r}<=EDATE($D$5,60),\"Within five years\",\"Later\")),\"Missing date\"))"));
    }
    s.set_col_width_range(9, 9, 22);
    s.flag("I9:I28", "Later");
    s.note("B31", "The five-year flag is a diary prompt. The deed and relevant law determine vesting and amendment requirements.");
    s.freeze = Some((2, 8));
    s
}

const CARRY_HEADERS: [&str; 7] = [
    "Origin period",
    "Last usable period",
    "Remaining bucket",
    "Available now",
    "Applied to proposal",
    "Usable after",
    "Input status",
];

pub fn carry_forward() -> Sheet {
    let mut s = Sheet::new("Carry forward", "Carry forward", "N", 48);
    s.pair_text(5, "Eligibility supplied by reviewer", "Yes");
    s.list_validation("D5", &["Yes", "No"]);
    s.pair_num(6, "Current annual capacity", 30000.0, FMT_MONEY);
    s.pair_num(7, "Already used this year", 20000.0, FMT_MONEY);
    s.pair_num(8, "Proposed additional use", 25000.0, FMT_MONEY);
    s.pair_num(9, "Current period label", 2027.0, FMT_INT);
    s.result(11, "Input status", "=IF(AND(OR(D5=\"Yes\",D5=\"No\"),COUNT(D6:D9)=4,MIN(D6:D8)>=0,D9=INT(D9),COUNT(B18:D22)=15,MIN(D18:D22)>=0,COUNTIF(H18:H22,\"Ready\")=5),\"Ready\",\"Check inputs\")", FMT_TEXT);
    s.result(
        12,
        "Current capacity left",
        "=IF(D11=\"Ready\",MAX(0,D6-D7),NA())",
        FMT_MONEY,
    );
    s.result(13, "Current capacity applied", "=MIN(D8,D12)", FMT_MONEY);
    s.result(
        14,
        "Proposal above all capacity",
        "=IF(D11=\"Ready\",MAX(0,D8-D13-SUM(F18:F22)),NA())",
        FMT_MONEY,
    );
    s.header(17, &CARRY_HEADERS, 2);
    carry_rows(&mut s);
    s
}

fn carry_rows(s: &mut Sheet) {
    for r in 18..=22 {
        s.input_num(&format!("B{r}"), f64::from(2022 + r - 18), FMT_INT);
        s.input_num(&format!("C{r}"), f64::from(2027 + r - 18), FMT_INT);
        s.input_num(&format!("D{r}"), 5000.0, FMT_MONEY);
        let prev = if r > 18 {
            format!(",B{r}>B{},C{r}>=C{}", r - 1, r - 1)
        } else {
            String::new()
        };
        s.formula(&format!("H{r}"), &format!("=IF(AND(COUNT(B{r}:D{r})=3,B{r}=INT(B{r}),C{r}=INT(C{r}),C{r}>=B{r},D{r}>=0{prev}),\"Ready\",\"Check buckets\")"));
        s.formula(
            &format!("E{r}"),
            &format!("=IF($D$11=\"Ready\",IF(AND($D$5=\"Yes\",C{r}>=$D$9,B{r}<$D$9),D{r},0),NA())"),
        );
        let prior = if r == 18 {
            "0".to_owned()
        } else {
            format!("SUM($F$18:F{})", r - 1)
        };
        s.formula(
            &format!("F{r}"),
            &format!("=MIN(E{r},MAX(0,$D$8-$D$13-{prior}))"),
        );
        s.formula(&format!("G{r}"), &format!("=E{r}-F{r}"));
    }
    s.set_num_fmt_range("D18:G22", FMT_MONEY);
    s.flag("D11", "Ready");
    s.flag("H18:H22", "Ready");
    s.note("B25", "Enter remaining buckets after all earlier use. Rows must be oldest first, with nondecreasing expiry.");
    s.note("B27", "Examples are arbitrary capacity figures. This sheet does not determine super eligibility, caps or deductibility.");
}

const LOAN_OFFER_HEADERS: [&str; 13] = [
    "Offer",
    "Principal",
    "Annual rate",
    "Months",
    "Final balloon",
    "Upfront fee",
    "Monthly fee",
    "Loan payment",
    "Total interest",
    "Total fees",
    "Total outlay",
    "Stressed payment",
    "Input status",
];

pub fn loan_offers() -> Sheet {
    let mut s = Sheet::new("Loan offers", "Loan offers", "N", 48);
    s.pair_num(5, "Annual rate shock", 0.02, FMT_PCT);
    s.note(
        "F5",
        "Nominal annual rates, monthly compounding, end-of-month payments. Fees are paid in cash.",
    );
    s.note("B7", "Compare offers with the same principal and term. Total outlay includes regular payments, balloon and fees.");
    s.header(9, &LOAN_OFFER_HEADERS, 2);
    for r in 10..=17 {
        loan_offer_row(&mut s, r);
    }
    s.set_num_fmt_range("I10:M17", FMT_MONEY);
    s.flag("N10:N17", "Ready");
    s.note("B20", "Loan payment excludes fees; stressed payment includes the monthly fee. Fixed-rate assumptions do not forecast a lender.");
    s.note("B22", "No extra payments, rate resets, offset account, break costs or borrowing-capacity decision.");
    s
}

fn loan_offer_row(s: &mut Sheet, r: u16) {
    s.input_text(
        &format!("B{r}"),
        &if r < 12 {
            format!("Example {}", r - 9)
        } else {
            String::new()
        },
        FMT_TEXT,
    );
    s.input_num(
        &format!("C{r}"),
        if r < 12 { 100000.0 } else { 0.0 },
        FMT_MONEY,
    );
    s.input_num(
        &format!("D{r}"),
        if r == 10 { 0.06 } else { 0.065 },
        FMT_PCT,
    );
    s.input_num(&format!("E{r}"), 60.0, FMT_INT);
    s.input_num(&format!("F{r}"), 0.0, FMT_MONEY);
    s.input_num(
        &format!("G{r}"),
        if r == 10 { 500.0 } else { 0.0 },
        FMT_MONEY,
    );
    s.input_num(
        &format!("H{r}"),
        if r == 10 { 10.0 } else { 0.0 },
        FMT_MONEY,
    );
    s.formula(&format!("N{r}"), &format!("=IF(B{r}=\"\",\"\",IF(AND(COUNT(C{r}:H{r})=6,C{r}>0,MIN(D{r}:H{r})>=0,E{r}>0,E{r}=INT(E{r}),F{r}<=C{r},ISNUMBER($D$5),D{r}+$D$5>=0,SUMPRODUCT(--($B$10:$B$17=B{r}))=1),\"Ready\",\"Check inputs\"))"));
    s.formula(&format!("I{r}"), &format!("=IF(B{r}=\"\",\"\",IF(N{r}=\"Ready\",IF(D{r}=0,(C{r}-F{r})/E{r},-PMT(D{r}/12,E{r},C{r},-F{r})),NA()))"));
    s.formula(
        &format!("J{r}"),
        &format!("=IF(B{r}=\"\",\"\",I{r}*E{r}+F{r}-C{r})"),
    );
    s.formula(
        &format!("K{r}"),
        &format!("=IF(B{r}=\"\",\"\",G{r}+H{r}*E{r})"),
    );
    s.formula(
        &format!("L{r}"),
        &format!("=IF(B{r}=\"\",\"\",I{r}*E{r}+F{r}+K{r})"),
    );
    s.formula(&format!("M{r}"), &format!("=IF(B{r}=\"\",\"\",IF(N{r}=\"Ready\",IF(D{r}+$D$5=0,(C{r}-F{r})/E{r},-PMT((D{r}+$D$5)/12,E{r},C{r},-F{r}))+H{r},NA()))"));
}

const BREAK_EVEN_INPUTS: [(&str, f64); 6] = [
    ("Fixed costs per period", 60000.0),
    ("Selling price per unit", 100.0),
    ("Variable cost per unit", 40.0),
    ("Expected units sold", 1500.0),
    ("Target profit per period", 30000.0),
    ("Maximum units per period", 1800.0),
];

pub fn break_even() -> Sheet {
    let mut s = Sheet::new("Break even", "Break even", "N", 48);
    for (i, (label, val)) in BREAK_EVEN_INPUTS.iter().enumerate() {
        s.pair_num(
            i as u16 + 5,
            label,
            *val,
            if i >= 3 { FMT_DEC2 } else { FMT_MONEY },
        );
    }
    break_even_results(&mut s);
    s.note(
        "F5",
        "One common period for costs, volume, capacity and target profit.",
    );
    s.note(
        "F7",
        "Price and variable cost are constant. Supply amounts on a consistent GST basis.",
    );
    s.note(
        "F9",
        "Whole break-even/target units round upwards; other stored values remain unrounded.",
    );
    s.note("F11", "Losses and capacity shortfalls remain visible. This model excludes financing and income tax.");
    s.flag("D12", "Ready");
    s.flag("D18", "Within capacity");
    s
}

fn break_even_results(s: &mut Sheet) {
    s.result(
        12,
        "Input status",
        "=IF(AND(COUNT(D5:D10)=6,MIN(D5:D10)>=0,D6>D7),\"Ready\",\"Check costs or inputs\")",
        FMT_TEXT,
    );
    s.result(
        13,
        "Contribution per unit",
        "=IF(D12=\"Ready\",D6-D7,NA())",
        FMT_MONEY,
    );
    s.result(
        14,
        "Break-even units",
        "=IF(D12=\"Ready\",ROUNDUP(D5/D13,0),NA())",
        FMT_INT,
    );
    s.result(
        15,
        "Units for target profit",
        "=IF(D12=\"Ready\",ROUNDUP((D5+D9)/D13,0),NA())",
        FMT_INT,
    );
    s.result(
        16,
        "Expected profit",
        "=IF(D12=\"Ready\",D8*D13-D5,NA())",
        FMT_MONEY,
    );
    s.result(
        17,
        "Units above break even",
        "=IF(D12=\"Ready\",D8-D14,NA())",
        FMT_DEC2,
    );
    s.result(
        18,
        "Target capacity status",
        "=IF(D12<>\"Ready\",\"Check inputs\",IF(D15>D10,\"Above capacity\",\"Within capacity\"))",
        FMT_TEXT,
    );
    s.result(
        19,
        "Price for target at capacity",
        "=IF(D12=\"Ready\",IF(D10>0,(D5+D9)/D10+D7,NA()),NA())",
        FMT_MONEY,
    );
}

const WC_INPUTS: [(&str, f64); 4] = [
    ("Credit sales for period", 1200000.0),
    ("Cost of sales for period", 600000.0),
    ("Credit purchases for period", 720000.0),
    ("Days in reporting period", 365.0),
];
const WC_HEADERS: [&str; 6] = [
    "Balance",
    "Current days",
    "Target days",
    "Current balance",
    "Target balance",
    "Cash released",
];
const WC_ITEMS: [(&str, f64, f64, u16); 3] = [
    ("Receivables", 45.0, 30.0, 5),
    ("Inventory", 60.0, 50.0, 6),
    ("Payables", 30.0, 45.0, 7),
];

pub fn working_capital() -> Sheet {
    let mut s = Sheet::new("Working capital", "Working capital", "N", 48);
    for (i, (label, val)) in WC_INPUTS.iter().enumerate() {
        s.pair_num(
            i as u16 + 5,
            label,
            *val,
            if i == 3 { FMT_INT } else { FMT_MONEY },
        );
    }
    s.result(10, "Input status", "=IF(AND(COUNT(D5:D8)=4,MIN(D5:D7)>=0,D8>0,COUNT(C15:D17)=6,MIN(C15:D17)>=0),\"Ready\",\"Check inputs\")", FMT_TEXT);
    s.result(
        11,
        "Cash released / (invested)",
        "=IF(D10=\"Ready\",SUM(G15:G17),NA())",
        FMT_MONEY,
    );
    s.header(14, &WC_HEADERS, 2);
    for (i, (label, cur, tgt, src)) in WC_ITEMS.iter().enumerate() {
        let r = 15 + i as u16;
        s.cell_text(&format!("B{r}"), label);
        s.input_num(&format!("C{r}"), *cur, FMT_DEC2);
        s.input_num(&format!("D{r}"), *tgt, FMT_DEC2);
        s.formula(
            &format!("E{r}"),
            &format!("=IF($D$10=\"Ready\",$D${src}/$D$8*C{r},NA())"),
        );
        s.formula(
            &format!("F{r}"),
            &format!("=IF($D$10=\"Ready\",$D${src}/$D$8*D{r},NA())"),
        );
        s.formula(
            &format!("G{r}"),
            &if i == 2 {
                format!("=F{r}-E{r}")
            } else {
                format!("=E{r}-F{r}")
            },
        );
    }
    s.set_num_fmt_range("E15:G17", FMT_MONEY);
    s.note("B20", "Use credit sales for receivables, cost of sales for inventory and credit purchases for payables.");
    s.note("B22", "Keep balance and flow valuations consistent, including GST. Amounts are steady-state estimates, not a dated cash forecast.");
    s.note("B24", "Negative cash release means more cash is tied up. Supplier terms and feasible targets require business review.");
    s.flag("D10", "Ready");
    s
}
