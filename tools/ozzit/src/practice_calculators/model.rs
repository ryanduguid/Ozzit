// Copyright 2026 Ryan Duguid
// SPDX-License-Identifier: Apache-2.0

//! In-memory worksheet model and fluent helpers for the practice workbook.
use crate::common::column_number;
use jiff::civil::Date;
use std::collections::BTreeMap;

pub const BLUE: &str = "FF2455A4";
pub const PURPLE: &str = "FF66023C";
pub const GREEN: &str = "FF0F4B32";
pub const BLACK: &str = "FF000000";
pub const WHITE: &str = "FFFFFFFF";
pub const IVORY: &str = "FFFFFFF0";
pub const PLATINUM: &str = "FFE5E4E2";
pub const PALE_PURPLE: &str = "FFE8D9E2";
pub const CRIMSON: &str = "FF990024";
pub const PALE_CRIMSON: &str = "FFF0D9DE";
pub const PALE_GREEN: &str = "FFDBE4E0";

pub const FMT_MONEY: &str = r#"#,##0.00;(#,##0.00);"-""#;
pub const FMT_PCT: &str = r#"0.0%;(0.0%);"-""#;
pub const FMT_DATE: &str = "d-mmm-yyyy";
pub const FMT_INT: &str = "0";
pub const FMT_DEC2: &str = "0.00";
pub const FMT_DEC4: &str = "0.0000";
pub const FMT_MMM_YY: &str = "mmm-yy";
pub const FMT_TEXT: &str = "@";

#[derive(Clone, Debug, PartialEq)]
pub enum CellValue {
    Empty,
    Text(String),
    Number(f64),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FontSpec {
    pub name: &'static str,
    pub size: u16,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub color: Option<&'static str>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CellStyle {
    pub font: FontSpec,
    pub fill: Option<&'static str>,
    pub border_bottom: Option<&'static str>,
    pub num_fmt: Option<&'static str>,
    pub h_align: Option<&'static str>,
    pub v_align: Option<&'static str>,
    pub wrap: bool,
    pub xf_id: u16,
}

impl CellStyle {
    pub fn base_sheet() -> Self {
        Self {
            font: FontSpec {
                name: "Arial",
                size: 10,
                bold: false,
                italic: false,
                underline: false,
                color: Some(BLACK),
            },
            fill: Some(WHITE),
            border_bottom: None,
            num_fmt: None,
            h_align: None,
            v_align: Some("center"),
            wrap: false,
            xf_id: 0,
        }
    }

    pub fn hyperlink(fill: Option<&'static str>, v_align: Option<&'static str>) -> Self {
        Self {
            font: FontSpec {
                name: "Carlito",
                size: 11,
                bold: false,
                italic: false,
                underline: true,
                color: Some(PURPLE),
            },
            fill,
            border_bottom: None,
            num_fmt: None,
            h_align: None,
            v_align,
            wrap: false,
            xf_id: 1,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Cell {
    pub value: CellValue,
    pub formula: Option<String>,
    pub style: CellStyle,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DxfStyle {
    pub bold: bool,
    pub font_color: &'static str,
    pub fill_color: &'static str,
}

#[derive(Clone, Debug)]
pub struct ConditionalFormat {
    pub sqref: String,
    pub kind: &'static str,
    pub operator: Option<&'static str>,
    pub formula: String,
    pub dxf: DxfStyle,
    pub priority: usize,
}

#[derive(Clone, Debug)]
pub struct DataValidation {
    pub sqref: String,
    pub list_formula: String,
}

#[derive(Clone, Debug)]
pub struct Hyperlink {
    pub cell_ref: String,
    pub location: String,
    pub display: String,
}

pub struct Sheet {
    pub name: &'static str,
    pub last_col: u16,
    pub rows_count: u16,
    pub tab_color: &'static str,
    pub tab_selected: bool,
    pub freeze: Option<(u16, u16)>,
    pub col_widths: BTreeMap<u16, u16>,
    pub row_heights: BTreeMap<u16, u16>,
    pub cells: BTreeMap<(u16, u16), Cell>,
    pub cf_rules: Vec<ConditionalFormat>,
    pub validations: Vec<DataValidation>,
    pub hyperlinks: Vec<Hyperlink>,
}

pub fn col_name(mut n: u16) -> String {
    let mut bytes = Vec::new();
    while n > 0 {
        n -= 1;
        bytes.push(b'A' + (n % 26) as u8);
        n /= 26;
    }
    bytes.reverse();
    String::from_utf8(bytes).expect("ASCII letters")
}

pub fn parse_cell_ref(addr: &str) -> (u16, u16) {
    let split = addr
        .find(|c: char| c.is_ascii_digit())
        .expect("cell row digits");
    let col = column_number(&addr[..split]) as u16;
    let row = addr[split..].parse::<u16>().expect("row number");
    (row, col)
}

pub fn parse_range(range: &str) -> ((u16, u16), (u16, u16)) {
    match range.split_once(':') {
        Some((a, b)) => (parse_cell_ref(a), parse_cell_ref(b)),
        None => {
            let single = parse_cell_ref(range);
            (single, single)
        }
    }
}

pub fn excel_serial(year: i32, month: i32, day: i32) -> f64 {
    let total_months = year * 12 + (month - 1);
    let y = total_months.div_euclid(12) as i16;
    let m = (total_months.rem_euclid(12) + 1) as i8;
    let date = Date::new(y, m, day as i8).expect("valid calendar date");
    let epoch = Date::new(1899, 12, 30).expect("1899-12-30 epoch");
    f64::from((date - epoch).get_days())
}

impl Sheet {
    pub fn new(name: &'static str, title: &str, last: &str, rows: u16) -> Self {
        let last_col = column_number(last) as u16;
        let mut sheet = Self {
            name,
            last_col,
            rows_count: rows,
            tab_color: PURPLE,
            tab_selected: false,
            freeze: None,
            col_widths: BTreeMap::new(),
            row_heights: BTreeMap::new(),
            cells: BTreeMap::new(),
            cf_rules: Vec::new(),
            validations: Vec::new(),
            hyperlinks: Vec::new(),
        };
        sheet.init_grid(title, last_col, rows);
        sheet
    }

    fn init_grid(&mut self, title: &str, last_col: u16, rows: u16) {
        self.set_col_width_range(2, 2, 30);
        self.set_col_width_range(3, last_col, 15);
        for r in 2..=rows {
            self.row_heights.insert(r, 21);
            for c in 2..=last_col {
                self.cells.insert(
                    (r, c),
                    Cell {
                        value: CellValue::Empty,
                        formula: None,
                        style: CellStyle::base_sheet(),
                    },
                );
            }
        }
        self.cell_text("B2", title);
        let b2 = self.cell_mut((2, 2));
        b2.style.font.size = 15;
        b2.style.font.bold = true;
        b2.style.font.color = Some(PURPLE);
        self.cell_text(
            "B3",
            "Illustrative AUD inputs. Blue cells are editable. Replace examples before use.",
        );
        self.cell_mut((3, 2)).style.font.italic = true;
        for c in 2..=last_col {
            self.cell_mut((2, c)).style.border_bottom = Some(PURPLE);
        }
    }

    pub fn cell_mut(&mut self, at: (u16, u16)) -> &mut Cell {
        self.cells.entry(at).or_insert_with(|| Cell {
            value: CellValue::Empty,
            formula: None,
            style: CellStyle::base_sheet(),
        })
    }

    pub fn each_in_range(&mut self, range: &str, mut f: impl FnMut(&mut Cell)) {
        let ((r1, c1), (r2, c2)) = parse_range(range);
        for r in r1..=r2 {
            for c in c1..=c2 {
                f(self.cell_mut((r, c)));
            }
        }
    }

    pub fn cell_text(&mut self, addr: &str, text: &str) {
        let at = parse_cell_ref(addr);
        self.cell_mut(at).value = if text.is_empty() {
            CellValue::Empty
        } else {
            CellValue::Text(text.to_owned())
        };
    }

    pub fn cell_num(&mut self, addr: &str, value: f64) {
        let at = parse_cell_ref(addr);
        self.cell_mut(at).value = CellValue::Number(value);
    }

    pub fn formula(&mut self, addr: &str, expr: &str) {
        let at = parse_cell_ref(addr);
        let cleaned = expr.strip_prefix('=').unwrap_or(expr);
        self.cell_mut(at).formula = Some(cleaned.to_owned());
    }

    pub fn input_style(&mut self, addr: &str, fmt: &'static str) {
        let at = parse_cell_ref(addr);
        let cell = self.cell_mut(at);
        cell.style.font.color = Some(BLUE);
        cell.style.fill = Some(PLATINUM);
        cell.style.num_fmt = Some(fmt);
    }

    pub fn input_num(&mut self, addr: &str, value: f64, fmt: &'static str) {
        self.cell_num(addr, value);
        self.input_style(addr, fmt);
    }

    pub fn input_text(&mut self, addr: &str, text: &str, fmt: &'static str) {
        self.cell_text(addr, text);
        self.input_style(addr, fmt);
    }

    pub fn input_empty(&mut self, addr: &str, fmt: &'static str) {
        self.cell_text(addr, "");
        self.input_style(addr, fmt);
    }

    pub fn input_date(&mut self, addr: &str, year: i32, month: i32, day: i32) {
        self.input_num(addr, excel_serial(year, month, day), FMT_DATE);
    }

    pub fn header(&mut self, row: u16, labels: &[&str], start_col: u16) {
        self.row_heights.insert(row, 56);
        for (i, label) in labels.iter().enumerate() {
            let c = start_col + i as u16;
            let cell = self.cell_mut((row, c));
            cell.value = CellValue::Text((*label).to_owned());
            cell.style.fill = Some(PURPLE);
            cell.style.font.bold = true;
            cell.style.font.color = Some(IVORY);
            cell.style.h_align = Some("center");
            cell.style.v_align = Some("center");
            cell.style.wrap = true;
        }
    }

    pub fn pair_num(&mut self, row: u16, label: &str, value: f64, fmt: &'static str) {
        self.cell_text(&format!("B{row}"), label);
        self.input_num(&format!("D{row}"), value, fmt);
    }

    pub fn pair_text(&mut self, row: u16, label: &str, text: &str) {
        self.cell_text(&format!("B{row}"), label);
        self.input_text(&format!("D{row}"), text, FMT_TEXT);
    }

    pub fn pair_date(&mut self, row: u16, label: &str, y: i32, m: i32, d: i32) {
        self.cell_text(&format!("B{row}"), label);
        self.input_date(&format!("D{row}"), y, m, d);
    }

    pub fn result(&mut self, row: u16, label: &str, expr: &str, fmt: &'static str) {
        self.cell_text(&format!("B{row}"), label);
        let d_addr = format!("D{row}");
        self.formula(&d_addr, expr);
        self.cell_mut((row, 4)).style.num_fmt = Some(fmt);
        for c in 2..=4 {
            let cell = self.cell_mut((row, c));
            cell.style.font.bold = true;
            cell.style.fill = Some(PALE_PURPLE);
        }
    }

    pub fn flag(&mut self, range: &str, good: &str) {
        let first = range.split(':').next().unwrap_or(range);
        let expr = format!(r#"AND({first}<>"",{first}<>"{good}")"#);
        self.cf_expr(range, &expr, CRIMSON, PALE_CRIMSON, true);
        self.set_h_align_range(range, "center");
    }

    pub fn cf_expr(
        &mut self,
        sqref: &str,
        expr: &str,
        font_color: &'static str,
        fill_color: &'static str,
        bold: bool,
    ) {
        let priority = self.cf_rules.len() + 1;
        self.cf_rules.push(ConditionalFormat {
            sqref: sqref.to_owned(),
            kind: "expression",
            operator: None,
            formula: expr.to_owned(),
            dxf: DxfStyle {
                bold,
                font_color,
                fill_color,
            },
            priority,
        });
    }

    pub fn cf_cell_eq(
        &mut self,
        sqref: &str,
        val: &str,
        font_color: &'static str,
        fill_color: &'static str,
    ) {
        let priority = self.cf_rules.len() + 1;
        self.cf_rules.push(ConditionalFormat {
            sqref: sqref.to_owned(),
            kind: "cellIs",
            operator: Some("equal"),
            formula: val.to_owned(),
            dxf: DxfStyle {
                bold: false,
                font_color,
                fill_color,
            },
            priority,
        });
    }

    pub fn note(&mut self, addr: &str, text: &str) {
        self.cell_text(addr, text);
        let at = parse_cell_ref(addr);
        self.cell_mut(at).style.font.italic = true;
    }

    pub fn row_status(
        &mut self,
        row: u16,
        last_input: &str,
        status: &str,
        count: usize,
        valid: &str,
        last_row: u16,
    ) {
        let inputs = format!("C{row}:{last_input}{row}");
        let expr = format!(
            "IF(B{row}=\"\",IF(COUNTA({inputs})=0,\"\",\"Missing name\"),\
             IF(COUNT({inputs})<>{count},\"Check inputs\",\
             IF(AND({valid},SUMPRODUCT(--($B$14:$B${last_row}=B{row}))=1),\
             \"Ready\",\"Check inputs\")))"
        );
        self.formula(&format!("{status}{row}"), &expr);
    }

    pub fn table_status(&mut self, addr: &str, status: &str, last_row: u16) {
        let expr = format!(
            "IF(AND(COUNTA(B14:B{last_row})>0,\
             COUNTIF({status}14:{status}{last_row},\"Ready\")=COUNTA(B14:B{last_row}),\
             COUNTIF({status}14:{status}{last_row},\"Missing name\")=0),\
             \"Ready\",\"Check rows\")"
        );
        self.formula(addr, &expr);
        self.flag(addr, "Ready");
        self.flag(&format!("{status}14:{status}{last_row}"), "Ready");
    }

    pub fn set_num_fmt_range(&mut self, range: &str, fmt: &'static str) {
        self.each_in_range(range, |c| c.style.num_fmt = Some(fmt));
    }

    pub fn set_h_align_range(&mut self, range: &str, align: &'static str) {
        self.each_in_range(range, |c| c.style.h_align = Some(align));
    }

    pub fn set_col_width_range(&mut self, c1: u16, c2: u16, width: u16) {
        for c in c1..=c2 {
            self.col_widths.insert(c, width);
        }
    }

    pub fn list_validation(&mut self, sqref: &str, values: &[&str]) {
        self.validations.push(DataValidation {
            sqref: sqref.to_owned(),
            list_formula: format!("\"{}\"", values.join(",")),
        });
    }

    pub fn add_hyperlink(&mut self, cell_ref: &str, location: &str, display: &str) {
        self.hyperlinks.push(Hyperlink {
            cell_ref: cell_ref.to_owned(),
            location: location.to_owned(),
            display: display.to_owned(),
        });
    }
}
