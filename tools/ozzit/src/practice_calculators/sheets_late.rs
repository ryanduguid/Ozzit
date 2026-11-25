// Copyright 2026 Ryan Duguid
// SPDX-License-Identifier: Apache-2.0

//! Worksheets 15-22: Construction WIP, KPI awards, Asset disposal, Exit pay, Tax cash, Investment, Action register, Benefit totals.
use super::model::{
    BLACK, CRIMSON, FMT_DATE, FMT_DEC4, FMT_INT, FMT_MONEY, FMT_PCT, FMT_TEXT, GREEN, PALE_CRIMSON,
    PALE_GREEN, Sheet, col_name,
};

const WIP_HEADERS: [&str; 16] = [
    "Project",
    "Contract amount",
    "Costs to date",
    "Remaining costs",
    "Approved progress",
    "Prior modelled revenue",
    "Billings to date",
    "Cash collected",
    "Forecast cost",
    "Forecast profit",
    "Modelled revenue",
    "Current revenue",
    "Unbilled revenue",
    "Billings in advance",
    "Debtors",
    "Input status",
];
const WIP_VALUES: [f64; 7] = [200000.0, 60000.0, 40000.0, 0.6, 50000.0, 100000.0, 80000.0];

pub fn construction_wip() -> Sheet {
    let mut s = Sheet::new("Construction WIP", "Construction WIP", "Q", 48);
    s.result(5, "Input status", "=\"\"", FMT_TEXT);
    s.table_status("D5", "Q", 25);
    s.result(
        6,
        "Forecast project profit",
        "=IF(D5=\"Ready\",SUM(K14:K25),NA())",
        FMT_MONEY,
    );
    s.result(
        7,
        "Unbilled modelled revenue",
        "=IF(D5=\"Ready\",SUM(N14:N25),NA())",
        FMT_MONEY,
    );
    s.result(
        8,
        "Billings in advance",
        "=IF(D5=\"Ready\",SUM(O14:O25),NA())",
        FMT_MONEY,
    );
    s.result(
        9,
        "Outstanding billings",
        "=IF(D5=\"Ready\",SUM(P14:P25),NA())",
        FMT_MONEY,
    );
    s.note(
        "F5",
        "Progress is approved by the reviewer. It is not a revenue-recognition or tax decision.",
    );
    s.note(
        "F7",
        "Costs include the allocated overheads from Allocation. Use one consistent GST basis.",
    );
    s.header(13, &WIP_HEADERS, 2);
    for r in 14..=25 {
        wip_row(&mut s, r);
    }
    s.set_num_fmt_range("J14:P25", FMT_MONEY);
    s.note("B28", "A revised approved progress can produce negative current revenue. Forecast losses are preserved.");
    s.freeze = Some((2, 13));
    s
}

fn wip_row(s: &mut Sheet, r: u16) {
    s.input_text(
        &format!("B{r}"),
        if r == 14 { "Example project" } else { "" },
        FMT_TEXT,
    );
    for c in 3..=9u16 {
        let fmt = if c == 6 { FMT_PCT } else { FMT_MONEY };
        if r == 14 {
            s.input_num(
                &format!("{}{r}", col_name(c)),
                WIP_VALUES[usize::from(c - 3)],
                fmt,
            );
        } else {
            s.input_empty(&format!("{}{r}", col_name(c)), fmt);
        }
    }
    s.row_status(
        r,
        "I",
        "Q",
        7,
        &format!("MIN(C{r}:I{r})>=0,F{r}<=1,I{r}<=H{r}"),
        25,
    );
    let exprs = [
        ("J", format!("D{r}+E{r}")),
        ("K", format!("C{r}-J{r}")),
        ("L", format!("C{r}*F{r}")),
        ("M", format!("L{r}-G{r}")),
        ("N", format!("MAX(0,L{r}-H{r})")),
        ("O", format!("MAX(0,H{r}-L{r})")),
        ("P", format!("H{r}-I{r}")),
    ];
    for (c, f) in exprs {
        s.formula(
            &format!("{c}{r}"),
            &format!("=IF(B{r}=\"\",\"\",IF(Q{r}=\"Ready\",{f},NA()))"),
        );
    }
}

const KPI_HEADERS: [&str; 11] = [
    "Person",
    "Target cash award",
    "Achievement score",
    "Minimum score",
    "Maximum paid score",
    "Withholding",
    "Cash award",
    "Employer on-costs",
    "Employer total",
    "Net cash",
    "Input status",
];
const KPI_VALUES: [f64; 5] = [10000.0, 1.2, 0.8, 1.5, 2000.0];

pub fn kpi_awards() -> Sheet {
    let mut s = Sheet::new("KPI awards", "KPI awards", "N", 48);
    s.pair_num(5, "Super cost / cash award", 0.12, FMT_PCT);
    s.pair_num(6, "Payroll cost / award + super", 0.05, FMT_PCT);
    s.result(8, "Award inputs", "=\"\"", FMT_TEXT);
    s.table_status("D8", "L", 33);
    s.cell_text("F9", "Cost rates");
    s.formula(
        "H9",
        "=IF(AND(COUNT(D5:D6)=2,MIN(D5:D6)>=0),\"Ready\",\"Check cost rates\")",
    );
    s.flag("H9", "Ready");
    s.result(
        9,
        "Total employer cost",
        "=IF(AND(D8=\"Ready\",H9=\"Ready\"),SUM(J14:J33),NA())",
        FMT_MONEY,
    );
    s.result(
        10,
        "Total net awards",
        "=IF(D8=\"Ready\",SUM(K14:K33),NA())",
        FMT_MONEY,
    );
    s.note(
        "F5",
        "Award = target cash × capped score, only when the supplied threshold is reached.",
    );
    s.note("F7", "Scores, thresholds and caps are ratios. The independent rule here does not reproduce the private vendor rule.");
    s.header(13, &KPI_HEADERS, 2);
    for r in 14..=33 {
        kpi_row(&mut s, r);
    }
    s.set_num_fmt_range("H14:K33", FMT_MONEY);
    s.note("B36", "Individual awards do not redistribute a fixed pool. Use Allocation when total spending must equal one budget.");
    s
}

fn kpi_row(s: &mut Sheet, r: u16) {
    s.input_text(
        &format!("B{r}"),
        if r == 14 { "Example employee" } else { "" },
        FMT_TEXT,
    );
    for c in 3..=7u16 {
        let fmt = if (4..=6).contains(&c) {
            FMT_PCT
        } else {
            FMT_MONEY
        };
        if r == 14 {
            s.input_num(
                &format!("{}{r}", col_name(c)),
                KPI_VALUES[usize::from(c - 3)],
                fmt,
            );
        } else {
            s.input_empty(&format!("{}{r}", col_name(c)), fmt);
        }
    }
    s.row_status(
        r,
        "G",
        "L",
        5,
        &format!("MIN(C{r}:G{r})>=0,E{r}<=F{r},G{r}<=IF(D{r}<E{r},0,C{r}*MIN(D{r},F{r}))"),
        33,
    );
    s.formula(
        &format!("H{r}"),
        &format!("=IF(B{r}=\"\",\"\",IF(L{r}=\"Ready\",IF(D{r}<E{r},0,C{r}*MIN(D{r},F{r})),NA()))"),
    );
    s.formula(&format!("I{r}"), &format!("=IF(B{r}=\"\",\"\",IF(AND(L{r}=\"Ready\",$H$9=\"Ready\"),H{r}*((1+$D$5)*(1+$D$6)-1),NA()))"));
    s.formula(
        &format!("J{r}"),
        &format!("=IF(B{r}=\"\",\"\",SUM(H{r}:I{r}))"),
    );
    s.formula(&format!("K{r}"), &format!("=IF(B{r}=\"\",\"\",H{r}-G{r})"));
}

const DISPOSAL_HEADERS: [&str; 15] = [
    "Asset",
    "Opening book value",
    "Depreciation to sale",
    "Gross sale proceeds",
    "Sale GST assessed",
    "Disposal cash costs",
    "Disposal GST credit",
    "Replacement cash cost",
    "Replacement GST credit",
    "Book value at disposal",
    "Book profit / (loss)",
    "Cash before GST",
    "Net GST / (credit)",
    "Cash after GST",
    "Input status",
];
const DISPOSAL_VALUES: [f64; 8] = [
    20000.0, 2000.0, 22000.0, 2000.0, 1100.0, 100.0, 33000.0, 3000.0,
];

pub fn asset_disposal() -> Sheet {
    let mut s = Sheet::new("Asset disposal", "Asset disposal", "P", 48);
    s.result(5, "Input status", "=\"\"", FMT_TEXT);
    s.table_status("D5", "P", 25);
    for (r, label, c) in [
        (6, "Book profit / (loss)", "L"),
        (7, "Cash before GST settlement", "M"),
        (8, "Supplied net GST / (credit)", "N"),
        (9, "Cash after supplied GST", "O"),
    ] {
        s.result(
            r,
            label,
            &format!("=IF(D5=\"Ready\",SUM({c}14:{c}25),NA())"),
            FMT_MONEY,
        );
    }
    s.note("F5", "Book values, depreciation and GST are supplied amounts. No tax balancing adjustment is calculated.");
    s.note(
        "F7",
        "Cash after GST assumes all entered GST is settled in this modelling period.",
    );
    s.header(13, &DISPOSAL_HEADERS, 2);
    for r in 14..=25 {
        disposal_row(&mut s, r);
    }
    s.set_num_fmt_range("K14:O25", FMT_MONEY);
    s.freeze = Some((0, 13));
    s
}

fn disposal_row(s: &mut Sheet, r: u16) {
    s.input_text(
        &format!("B{r}"),
        if r == 14 { "Example asset" } else { "" },
        FMT_TEXT,
    );
    for c in 3..=10u16 {
        if r == 14 {
            s.input_num(
                &format!("{}{r}", col_name(c)),
                DISPOSAL_VALUES[usize::from(c - 3)],
                FMT_MONEY,
            );
        } else {
            s.input_empty(&format!("{}{r}", col_name(c)), FMT_MONEY);
        }
    }
    s.row_status(
        r,
        "J",
        "P",
        8,
        &format!("MIN(C{r}:J{r})>=0,D{r}<=C{r},F{r}<=E{r},H{r}<=G{r},J{r}<=I{r}"),
        25,
    );
    let exprs = [
        ("K", format!("C{r}-D{r}")),
        ("L", format!("E{r}-F{r}-G{r}+H{r}-K{r}")),
        ("M", format!("E{r}-G{r}-I{r}")),
        ("N", format!("F{r}-H{r}-J{r}")),
        ("O", format!("M{r}-N{r}")),
    ];
    for (c, f) in exprs {
        s.formula(
            &format!("{c}{r}"),
            &format!("=IF(B{r}=\"\",\"\",IF(P{r}=\"Ready\",{f},NA()))"),
        );
    }
}

const EXIT_HEADERS: [&str; 19] = [
    "Employee",
    "Annual leave hours",
    "Annual leave rate",
    "Leave loading ratio",
    "Other leave hours",
    "Other leave rate",
    "Notice weeks",
    "Weekly notice pay",
    "Approved redundancy",
    "Other gross payments",
    "Withholding",
    "Employer super",
    "Annual leave pay",
    "Other leave pay",
    "Notice pay",
    "Gross pay",
    "Net pay",
    "Employer cost",
    "Input status",
];
const EXIT_VALUES: [f64; 11] = [
    80.0, 50.0, 0.175, 40.0, 50.0, 2.0, 1900.0, 10000.0, 500.0, 4000.0, 300.0,
];

pub fn exit_pay() -> Sheet {
    let mut s = Sheet::new("Exit pay", "Exit pay", "T", 48);
    s.result(5, "Input status", "=\"\"", FMT_TEXT);
    s.table_status("D5", "T", 23);
    s.result(
        6,
        "Total approved gross pay",
        "=IF(D5=\"Ready\",SUM(Q14:Q23),NA())",
        FMT_MONEY,
    );
    s.result(
        7,
        "Total net pay",
        "=IF(D5=\"Ready\",SUM(R14:R23),NA())",
        FMT_MONEY,
    );
    s.result(
        8,
        "Total employer cost",
        "=IF(D5=\"Ready\",SUM(S14:S23),NA())",
        FMT_MONEY,
    );
    s.note("F5", "Enter approved entitlements and assessed withholding/super. No award, ETP cap, tax label or legal entitlement is inferred.");
    s.header(13, &EXIT_HEADERS, 2);
    for r in 14..=23 {
        exit_pay_row(&mut s, r);
    }
    s.set_num_fmt_range("N14:S23", FMT_MONEY);
    s.freeze = Some((2, 13));
    s
}

fn exit_pay_row(s: &mut Sheet, r: u16) {
    s.input_text(
        &format!("B{r}"),
        if r == 14 { "Example employee" } else { "" },
        FMT_TEXT,
    );
    for c in 3..=13u16 {
        let fmt = if c == 5 { FMT_PCT } else { FMT_MONEY };
        if r == 14 {
            s.input_num(
                &format!("{}{r}", col_name(c)),
                EXIT_VALUES[usize::from(c - 3)],
                fmt,
            );
        } else {
            s.input_empty(&format!("{}{r}", col_name(c)), fmt);
        }
    }
    s.row_status(
        r,
        "M",
        "T",
        11,
        &format!("MIN(C{r}:M{r})>=0,L{r}<=C{r}*D{r}*(1+E{r})+F{r}*G{r}+H{r}*I{r}+J{r}+K{r}"),
        23,
    );
    let exprs = [
        ("N", format!("C{r}*D{r}*(1+E{r})")),
        ("O", format!("F{r}*G{r}")),
        ("P", format!("H{r}*I{r}")),
        ("Q", format!("SUM(N{r}:P{r},J{r}:K{r})")),
        ("R", format!("Q{r}-L{r}")),
        ("S", format!("Q{r}+M{r}")),
    ];
    for (c, f) in exprs {
        s.formula(
            &format!("{c}{r}"),
            &format!("=IF(B{r}=\"\",\"\",IF(T{r}=\"Ready\",{f},NA()))"),
        );
    }
}

const TAX_HEADERS: [&str; 11] = [
    "Entity / account",
    "Opening liability / (credit)",
    "Assessed tax expense",
    "Signed adjustments",
    "Cash payments",
    "Cash refunds",
    "Noncash credits",
    "Closing liability / (credit)",
    "Payable",
    "Refund balance",
    "Input status",
];
const TAX_VALUES: [f64; 6] = [1000.0, 25000.0, -500.0, 20000.0, 0.0, 1000.0];

pub fn tax_cash() -> Sheet {
    let mut s = Sheet::new("Tax cash", "Tax cash", "N", 48);
    s.result(5, "Input status", "=\"\"", FMT_TEXT);
    s.table_status("D5", "L", 25);
    s.result(
        6,
        "Total payable balances",
        "=IF(D5=\"Ready\",SUM(J14:J25),NA())",
        FMT_MONEY,
    );
    s.result(
        7,
        "Total refund balances",
        "=IF(D5=\"Ready\",SUM(K14:K25),NA())",
        FMT_MONEY,
    );
    s.result(
        8,
        "Net closing liability",
        "=IF(D5=\"Ready\",SUM(I14:I25),NA())",
        FMT_MONEY,
    );
    s.note("F5", "Positive balances are liabilities. Negative balances are credits. Payments include cash instalments.");
    s.note("F7", "Assessed tax and signed adjustments are supplied. Balances are not legally offset between entities.");
    s.header(13, &TAX_HEADERS, 2);
    for r in 14..=25 {
        tax_cash_row(&mut s, r);
    }
    s.set_num_fmt_range("I14:K25", FMT_MONEY);
    s.note("B28", "Cash refunds consume credit balances. Due dates can be recorded in Action register. No tax rate or distribution is optimised.");
    s
}

fn tax_cash_row(s: &mut Sheet, r: u16) {
    s.input_text(
        &format!("B{r}"),
        if r == 14 { "Example entity" } else { "" },
        FMT_TEXT,
    );
    for c in 3..=8u16 {
        if r == 14 {
            s.input_num(
                &format!("{}{r}", col_name(c)),
                TAX_VALUES[usize::from(c - 3)],
                FMT_MONEY,
            );
        } else {
            s.input_empty(&format!("{}{r}", col_name(c)), FMT_MONEY);
        }
    }
    s.row_status(r, "H", "L", 6, &format!("MIN(F{r}:H{r})>=0"), 25);
    let exprs = [
        ("I", format!("SUM(C{r}:E{r})-F{r}+G{r}-H{r}")),
        ("J", format!("MAX(0,I{r})")),
        ("K", format!("MAX(0,-I{r})")),
    ];
    for (c, f) in exprs {
        s.formula(
            &format!("{c}{r}"),
            &format!("=IF(B{r}=\"\",\"\",IF(L{r}=\"Ready\",{f},NA()))"),
        );
    }
}

const INVESTMENT_HEADERS: [&str; 17] = [
    "Period",
    "Opening investment",
    "Capital growth ratio",
    "Cash distributions",
    "Capital withdrawals",
    "Capital additions",
    "Closing investment",
    "Opening debt",
    "Interest ratio",
    "Interest cost",
    "Principal paid",
    "New borrowing",
    "Closing debt",
    "Assessed tax / (refund)",
    "Net cash flow",
    "Net equity",
    "Input status",
];
const INVESTMENT_COLS: [(&str, f64); 8] = [
    ("D", 0.08),
    ("E", 6000.0),
    ("F", 0.0),
    ("G", 2000.0),
    ("J", 0.06),
    ("L", 3000.0),
    ("M", 0.0),
    ("O", 1000.0),
];

pub fn investment() -> Sheet {
    let mut s = Sheet::new("Investment", "Investment", "R", 48);
    s.pair_num(5, "Opening investment value", 100000.0, FMT_MONEY);
    s.pair_num(6, "Opening debt", 60000.0, FMT_MONEY);
    s.pair_num(7, "First period label", 2027.0, FMT_INT);
    s.result(9, "Ending net equity", "=Q23", FMT_MONEY);
    s.result(10, "Total net cash flow", "=SUM(P14:P23)", FMT_MONEY);
    s.result(11, "Cash plus equity return", "=D10+D9-(D5-D6)", FMT_MONEY);
    s.note("F5", "Growth changes value. Distributions are separate cash income. Capital flows and debt payments occur at year end.");
    s.note("F7", "Supply assessed tax (negative for refunds). No unit-trust, super, franking or deductibility rules are inferred.");
    s.header(13, &INVESTMENT_HEADERS, 2);
    for r in 14..=23 {
        investment_row(&mut s, r);
    }
    s.set_num_fmt_range("C14:Q23", FMT_MONEY);
    s.set_num_fmt_range("D14:D23", FMT_PCT);
    s.set_num_fmt_range("J14:J23", FMT_PCT);
    s.flag("R14:R23", "Ready");
    s.note("B26", "Net return is undiscounted cash plus ending equity less opening equity. It is not an annualised return or an investment recommendation.");
    s.freeze = Some((0, 13));
    s
}

fn investment_row(s: &mut Sheet, r: u16) {
    s.formula(&format!("B{r}"), &format!("=$D$7+{}", r - 14));
    s.formula(
        &format!("C{r}"),
        &if r == 14 {
            "=$D$5".to_owned()
        } else {
            format!("=H{}", r - 1)
        },
    );
    s.formula(
        &format!("I{r}"),
        &if r == 14 {
            "=$D$6".to_owned()
        } else {
            format!("=N{}", r - 1)
        },
    );
    for (c, v) in INVESTMENT_COLS {
        let fmt = if c == "D" || c == "J" {
            FMT_PCT
        } else {
            FMT_MONEY
        };
        s.input_num(&format!("{c}{r}"), v, fmt);
    }
    let valid = format!(
        "IF(COUNT(D{r}:G{r},J{r},L{r}:M{r},O{r},$D$5:$D$7)=11,AND(MIN(E{r}:G{r},J{r},L{r}:M{r},$D$5:$D$6)>=0,$D$7=INT($D$7),D{r}>=-1,F{r}<=C{r}*(1+D{r})+G{r},L{r}<=I{r}+M{r}),FALSE)"
    );
    let prior = if r == 14 {
        "TRUE".to_owned()
    } else {
        format!("R{}=\"Ready\"", r - 1)
    };
    s.formula(
        &format!("R{r}"),
        &format!("=IF({prior},IF({valid},\"Ready\",\"Check inputs\"),\"Check prior year\")"),
    );
    investment_outputs(s, r);
}

fn investment_outputs(s: &mut Sheet, r: u16) {
    let exprs = [
        ("H", format!("C{r}*(1+D{r})+G{r}-F{r}")),
        ("K", format!("I{r}*J{r}")),
        ("N", format!("I{r}+M{r}-L{r}")),
        ("P", format!("E{r}+F{r}-G{r}-K{r}-L{r}+M{r}-O{r}")),
        ("Q", format!("H{r}-N{r}")),
    ];
    for (c, f) in exprs {
        s.formula(&format!("{c}{r}"), &format!("=IF(R{r}=\"Ready\",{f},NA())"));
    }
}

const ACTION_HEADERS: [&str; 11] = [
    "Action ID",
    "Workflow",
    "Action",
    "Owner",
    "Due date",
    "Complete: Yes / No",
    "Completed date",
    "Evidence / reference",
    "Input status",
    "Action status",
    "Days overdue",
];
const ACTION_WIDTHS: [(u16, u16); 7] = [
    (3, 21),
    (4, 30),
    (5, 22),
    (6, 18),
    (8, 18),
    (9, 30),
    (11, 24),
];

pub fn action_register() -> Sheet {
    let mut s = Sheet::new("Action register", "Action register", "N", 48);
    s.tab_color = BLACK;
    action_header(&mut s);
    s.header(13, &ACTION_HEADERS, 2);
    action_rows(&mut s);
    for (c, w) in ACTION_WIDTHS {
        s.set_col_width_range(c, c, w);
    }
    s.set_h_align_range("G14:G33", "center");
    s.list_validation("G14:G33", &["Yes", "No"]);
    s.cf_expr(
        "K14:K33",
        "K14=\"Recorded complete\"",
        GREEN,
        PALE_GREEN,
        true,
    );
    s.cf_expr(
        "K14:K33",
        "OR(K14=\"Overdue\",K14=\"Due soon\",K14=\"Check inputs\")",
        CRIMSON,
        PALE_CRIMSON,
        true,
    );
    s.set_num_fmt_range("L14:L33", FMT_INT);
    s.freeze = Some((0, 13));
    s
}

fn action_header(s: &mut Sheet) {
    s.pair_date(5, "Review date", 2026, 10, 19);
    s.pair_num(6, "Reminder window in days", 30.0, FMT_INT);
    s.result(7, "Input status", "=\"\"", FMT_TEXT);
    s.table_status("D7", "J", 33);
    s.result(8, "Open actions", "=IF(D7=\"Ready\",COUNTIF(K14:K33,\"Open\")+COUNTIF(K14:K33,\"Overdue\")+COUNTIF(K14:K33,\"Due soon\"),NA())", FMT_INT);
    s.result(
        9,
        "Overdue actions",
        "=IF(D7=\"Ready\",COUNTIF(K14:K33,\"Overdue\"),NA())",
        FMT_INT,
    );
    s.result(
        10,
        "Due soon",
        "=IF(D7=\"Ready\",COUNTIF(K14:K33,\"Due soon\"),NA())",
        FMT_INT,
    );
    s.note("F5", "Record reviewer-selected AML, corporate, SMSF, packaging or other actions. All legal scope and due dates are supplied.");
    s.note("F7", "Recorded complete requires an owner, completion date and evidence. It is an administrative status, not a legal conclusion.");
}

fn action_rows(s: &mut Sheet) {
    let text_vals = [
        (2, "EXAMPLE-01"),
        (3, "Other"),
        (4, "Confirm supplied scope"),
        (5, "Example reviewer"),
        (7, "No"),
    ];
    for r in 14..=33 {
        for c in 2..=9u16 {
            let addr = format!("{}{r}", col_name(c));
            if c == 6 || c == 8 {
                if r == 14 && c == 6 {
                    s.input_date(&addr, 2026, 11, 1);
                } else {
                    s.input_empty(&addr, FMT_DATE);
                }
            } else if r == 14 {
                let text = text_vals
                    .iter()
                    .find(|(col, _)| *col == c)
                    .map_or("", |(_, v)| *v);
                s.input_text(&addr, text, FMT_TEXT);
            } else {
                s.input_empty(&addr, FMT_TEXT);
            }
        }
        let valid = format!(
            "AND(C{r}<>\"\",D{r}<>\"\",E{r}<>\"\",COUNT(F{r},$D$5:$D$6)=3,F{r}>0,F{r}=INT(F{r}),$D$5>0,$D$5=INT($D$5),$D$6>=0,$D$6=INT($D$6),OR(AND(G{r}=\"No\",H{r}=\"\"),AND(G{r}=\"Yes\",ISNUMBER(H{r}),H{r}>0,H{r}=INT(H{r}),H{r}<=$D$5,I{r}<>\"\")),SUMPRODUCT(--($B$14:$B$33=B{r}))=1)"
        );
        s.formula(&format!("J{r}"), &format!("=IF(B{r}=\"\",IF(COUNTA(C{r}:I{r})=0,\"\",\"Missing name\"),IF(OR(COUNT(F{r},$D$5:$D$6)<>3,AND(H{r}<>\"\",NOT(ISNUMBER(H{r})))),\"Check inputs\",IF({valid},\"Ready\",\"Check inputs\")))"));
        s.formula(&format!("K{r}"), &format!("=IF(B{r}=\"\",\"\",IF(J{r}=\"Ready\",IF(G{r}=\"Yes\",\"Recorded complete\",IF(F{r}<$D$5,\"Overdue\",IF(F{r}<=$D$5+$D$6,\"Due soon\",\"Open\"))),\"Check inputs\"))"));
        s.formula(
            &format!("L{r}"),
            &format!(
                "=IF(B{r}=\"\",\"\",IF(J{r}=\"Ready\",IF(G{r}=\"Yes\",0,MAX(0,$D$5-F{r})),NA()))"
            ),
        );
    }
}

const BENEFIT_HEADERS: [&str; 8] = [
    "Benefit / employee",
    "Final type 1 taxable value",
    "Final type 2 taxable value",
    "Type 1 grossed up",
    "Type 2 grossed up",
    "Grossed-up total",
    "Modelled FBT",
    "Input status",
];

pub fn benefit_totals() -> Sheet {
    let mut s = Sheet::new("Benefit totals", "Benefit totals", "N", 48);
    s.pair_num(5, "Type 1 gross-up multiplier", 2.0802, FMT_DEC4);
    s.pair_num(6, "Type 2 gross-up multiplier", 1.8868, FMT_DEC4);
    s.pair_num(7, "FBT rate", 0.47, FMT_PCT);
    s.result(
        9,
        "Rate inputs",
        "=IF(AND(COUNT(D5:D7)=3,MIN(D5:D6)>=1,D7>=0,D7<=1),\"Ready\",\"Check rates\")",
        FMT_TEXT,
    );
    s.flag("D9", "Ready");
    s.result(10, "Benefit inputs", "=\"\"", FMT_TEXT);
    s.table_status("D10", "I", 33);
    s.result(
        11,
        "Modelled FBT",
        "=IF(AND(D9=\"Ready\",D10=\"Ready\"),SUM(H14:H33),NA())",
        FMT_MONEY,
    );
    s.note("F5", "Enter final taxable values after reviewer classification and adjustments. No exemptions, rebates or reportability decisions are made.");
    s.note("F7", "Rates for FBT years ending 31 March 2023 to 31 March 2027, checked 3 Nov 2026. Confirm the applicable FBT year and employer treatment.");
    s.header(13, &BENEFIT_HEADERS, 2);
    benefit_rows(&mut s);
    s.set_num_fmt_range("E14:H33", FMT_MONEY);
    s.note(
        "B36",
        "Source: ATO, Calculating your FBT, basic business/government gross-up steps.",
    );
    s.cell_text("B37", "https://www.ato.gov.au/businesses-and-organisations/hiring-and-paying-your-workers/fringe-benefits-tax/calculating-your-fbt");
    s.note("B39", "No statutory rates are selected automatically. No whole-dollar return rounding is applied.");
    s
}

fn benefit_rows(s: &mut Sheet) {
    for r in 14..=33 {
        s.input_text(
            &format!("B{r}"),
            if r == 14 { "Example benefits" } else { "" },
            FMT_TEXT,
        );
        if r == 14 {
            s.input_num(&format!("C{r}"), 10000.0, FMT_MONEY);
            s.input_num(&format!("D{r}"), 5000.0, FMT_MONEY);
        } else {
            s.input_empty(&format!("C{r}"), FMT_MONEY);
            s.input_empty(&format!("D{r}"), FMT_MONEY);
        }
        s.row_status(r, "D", "I", 2, &format!("MIN(C{r}:D{r})>=0"), 33);
        let exprs = [
            ("E", format!("C{r}*$D$5")),
            ("F", format!("D{r}*$D$6")),
            ("G", format!("SUM(E{r}:F{r})")),
            ("H", format!("G{r}*$D$7")),
        ];
        for (c, f) in exprs {
            s.formula(
                &format!("{c}{r}"),
                &format!("=IF(B{r}=\"\",\"\",IF(AND(I{r}=\"Ready\",$D$9=\"Ready\"),{f},NA()))"),
            );
        }
    }
}
