// Copyright 2026 Ryan Duguid
// SPDX-License-Identifier: Apache-2.0

//! Deterministic OOXML serialization for the practice calculators workbook.
use super::model::{
    CellStyle, CellValue, ConditionalFormat, DxfStyle, FMT_DATE, FMT_DEC2, FMT_DEC4, FMT_INT,
    FMT_MMM_YY, FMT_MONEY, FMT_PCT, FMT_TEXT, FontSpec, Sheet, col_name,
};
use crate::archive::Parts;
use crate::common::{escape, escape_attr};
use indexmap::IndexMap;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

const MAIN_NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const REL_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const PKG_REL_NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const MC_NS: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";
const X14AC_NS: &str = "http://schemas.microsoft.com/office/spreadsheetml/2009/9/ac";
const CT_NS: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
const THEME_XML: &str = include_str!("theme1.xml");

struct StyleTable {
    num_fmts: Vec<(u16, &'static str)>,
    fonts: Vec<FontSpec>,
    font_ids: HashMap<FontSpec, usize>,
    fills: Vec<Option<&'static str>>,
    fill_ids: HashMap<Option<&'static str>, usize>,
    borders: Vec<Option<&'static str>>,
    border_ids: HashMap<Option<&'static str>, usize>,
    xfs: Vec<XfRecord>,
    xf_ids: HashMap<CellStyle, usize>,
    dxfs: Vec<DxfStyle>,
    dxf_ids: HashMap<DxfStyle, usize>,
}

struct XfRecord {
    num_fmt_id: u16,
    font_id: usize,
    fill_id: usize,
    border_id: usize,
    xf_id: u16,
    style: CellStyle,
}

impl StyleTable {
    fn new() -> Self {
        let normal_font = FontSpec {
            name: "Carlito",
            size: 11,
            bold: false,
            italic: false,
            underline: false,
            color: None,
        };
        let hyperlink_base = FontSpec {
            name: "Carlito",
            size: 11,
            bold: false,
            italic: false,
            underline: true,
            color: Some("theme:10"),
        };
        let mut table = Self {
            num_fmts: vec![
                (164, FMT_MONEY),
                (165, FMT_PCT),
                (166, FMT_DATE),
                (167, FMT_DEC4),
            ],
            fonts: vec![normal_font.clone(), hyperlink_base.clone()],
            font_ids: HashMap::from([(normal_font, 0), (hyperlink_base, 1)]),
            fills: vec![None, Some("gray125")],
            fill_ids: HashMap::from([(None, 0), (Some("gray125"), 1)]),
            borders: vec![None],
            border_ids: HashMap::from([(None, 0)]),
            xfs: Vec::new(),
            xf_ids: HashMap::new(),
            dxfs: Vec::new(),
            dxf_ids: HashMap::new(),
        };
        table.xfs.push(XfRecord {
            num_fmt_id: 0,
            font_id: 0,
            fill_id: 0,
            border_id: 0,
            xf_id: 0,
            style: CellStyle::base_sheet(),
        });
        table
    }

    fn num_fmt_id(&self, fmt: Option<&'static str>) -> u16 {
        match fmt {
            None => 0,
            Some(FMT_INT) => 1,
            Some(FMT_DEC2) => 2,
            Some(FMT_MMM_YY) => 17,
            Some(FMT_TEXT) => 49,
            Some(custom) => self
                .num_fmts
                .iter()
                .find(|(_, code)| *code == custom)
                .map_or(0, |(id, _)| *id),
        }
    }

    fn intern_font(&mut self, font: &FontSpec) -> usize {
        if let Some(&id) = self.font_ids.get(font) {
            return id;
        }
        let id = self.fonts.len();
        self.fonts.push(font.clone());
        self.font_ids.insert(font.clone(), id);
        id
    }

    fn intern_fill(&mut self, fill: Option<&'static str>) -> usize {
        if let Some(&id) = self.fill_ids.get(&fill) {
            return id;
        }
        let id = self.fills.len();
        self.fills.push(fill);
        self.fill_ids.insert(fill, id);
        id
    }

    fn intern_border(&mut self, border: Option<&'static str>) -> usize {
        if let Some(&id) = self.border_ids.get(&border) {
            return id;
        }
        let id = self.borders.len();
        self.borders.push(border);
        self.border_ids.insert(border, id);
        id
    }

    fn intern_xf(&mut self, style: &CellStyle) -> usize {
        if let Some(&id) = self.xf_ids.get(style) {
            return id;
        }
        let font_id = self.intern_font(&style.font);
        let fill_id = self.intern_fill(style.fill);
        let border_id = self.intern_border(style.border_bottom);
        let num_fmt_id = self.num_fmt_id(style.num_fmt);
        let id = self.xfs.len();
        self.xfs.push(XfRecord {
            num_fmt_id,
            font_id,
            fill_id,
            border_id,
            xf_id: style.xf_id,
            style: style.clone(),
        });
        self.xf_ids.insert(style.clone(), id);
        id
    }

    fn intern_dxf(&mut self, dxf: &DxfStyle) -> usize {
        if let Some(&id) = self.dxf_ids.get(dxf) {
            return id;
        }
        let id = self.dxfs.len();
        self.dxfs.push(dxf.clone());
        self.dxf_ids.insert(dxf.clone(), id);
        id
    }

    fn to_xml(&self) -> String {
        let mut out = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<styleSheet xmlns="{MAIN_NS}" xmlns:mc="{MC_NS}" mc:Ignorable="x14ac" xmlns:x14ac="{X14AC_NS}">"#
        );
        self.write_num_fmts(&mut out);
        self.write_fonts(&mut out);
        self.write_fills(&mut out);
        self.write_borders(&mut out);
        out.push_str(
            r#"<cellStyleXfs count="2"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/><xf numFmtId="0" fontId="1" fillId="0" borderId="0" applyNumberFormat="0" applyFill="0" applyBorder="0" applyAlignment="0" applyProtection="0"/></cellStyleXfs>"#,
        );
        self.write_xfs(&mut out);
        self.write_dxfs(&mut out);
        out.push_str("</styleSheet>");
        out
    }

    fn write_num_fmts(&self, out: &mut String) {
        let _ = write!(out, r#"<numFmts count="{}">"#, self.num_fmts.len());
        for (id, code) in &self.num_fmts {
            let _ = write!(
                out,
                r#"<numFmt numFmtId="{id}" formatCode="{}"/>"#,
                escape_attr(code)
            );
        }
        out.push_str("</numFmts>");
    }

    fn write_fonts(&self, out: &mut String) {
        let _ = write!(
            out,
            r#"<fonts count="{}" x14ac:knownFonts="1">"#,
            self.fonts.len()
        );
        for f in &self.fonts {
            write_font(out, f);
        }
        out.push_str("</fonts>");
    }

    fn write_fills(&self, out: &mut String) {
        let _ = write!(out, r#"<fills count="{}">"#, self.fills.len());
        for fill in &self.fills {
            match fill {
                None => out.push_str(r#"<fill><patternFill patternType="none"/></fill>"#),
                Some("gray125") => {
                    out.push_str(r#"<fill><patternFill patternType="gray125"/></fill>"#)
                }
                Some(rgb) => {
                    let _ = write!(
                        out,
                        r#"<fill><patternFill patternType="solid"><fgColor rgb="{rgb}"/></patternFill></fill>"#
                    );
                }
            }
        }
        out.push_str("</fills>");
    }

    fn write_borders(&self, out: &mut String) {
        let _ = write!(out, r#"<borders count="{}">"#, self.borders.len());
        for border in &self.borders {
            match border {
                None => out.push_str("<border><left/><right/><top/><bottom/><diagonal/></border>"),
                Some(rgb) => {
                    let _ = write!(
                        out,
                        r#"<border><left/><right/><top/><bottom style="thin"><color rgb="{rgb}"/></bottom><diagonal/></border>"#
                    );
                }
            }
        }
        out.push_str("</borders>");
    }

    fn write_xfs(&self, out: &mut String) {
        let _ = write!(out, r#"<cellXfs count="{}">"#, self.xfs.len());
        for (i, xf) in self.xfs.iter().enumerate() {
            if i == 0 {
                out.push_str(r#"<xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/>"#);
                continue;
            }
            write_xf_entry(out, xf, xf.font_id);
        }
        out.push_str("</cellXfs>");
    }

    fn write_dxfs(&self, out: &mut String) {
        let _ = write!(out, r#"<dxfs count="{}">"#, self.dxfs.len());
        for d in &self.dxfs {
            let b = if d.bold { "<b/>" } else { "" };
            let _ = write!(
                out,
                r#"<dxf><font>{b}<color rgb="{}"/></font><fill><patternFill patternType="solid"><bgColor rgb="{}"/></patternFill></fill></dxf>"#,
                d.font_color, d.fill_color
            );
        }
        out.push_str("</dxfs>");
    }
}

fn write_font(out: &mut String, f: &FontSpec) {
    out.push_str("<font>");
    if f.bold {
        out.push_str("<b/>");
    }
    if f.italic {
        out.push_str("<i/>");
    }
    if f.underline {
        out.push_str("<u/>");
    }
    let _ = write!(out, r#"<sz val="{}"/>"#, f.size);
    if let Some(color) = f.color {
        if let Some(theme) = color.strip_prefix("theme:") {
            let _ = write!(out, r#"<color theme="{theme}"/>"#);
        } else {
            let _ = write!(out, r#"<color rgb="{color}"/>"#);
        }
    }
    let _ = write!(out, r#"<name val="{}"/></font>"#, f.name);
}

fn write_xf_entry(out: &mut String, xf: &XfRecord, font_id: usize) {
    let s = &xf.style;
    let has_align = s.h_align.is_some() || s.v_align.is_some() || s.wrap;
    let _ = write!(
        out,
        r#"<xf numFmtId="{}" fontId="{font_id}" fillId="{}" borderId="{}" xfId="{}""#,
        xf.num_fmt_id, xf.fill_id, xf.border_id, xf.xf_id
    );
    if xf.num_fmt_id != 0 {
        out.push_str(r#" applyNumberFormat="1""#);
    }
    out.push_str(r#" applyFont="1""#);
    if xf.fill_id != 0 {
        out.push_str(r#" applyFill="1""#);
    }
    if xf.border_id != 0 {
        out.push_str(r#" applyBorder="1""#);
    }
    if has_align {
        out.push_str(r#" applyAlignment="1"><alignment"#);
        if let Some(h) = s.h_align {
            let _ = write!(out, r#" horizontal="{h}""#);
        }
        if let Some(v) = s.v_align {
            let _ = write!(out, r#" vertical="{v}""#);
        }
        if s.wrap {
            out.push_str(r#" wrapText="1""#);
        }
        out.push_str("/></xf>");
    } else {
        out.push_str("/>");
    }
}

struct SharedStrings {
    strings: Vec<String>,
    ids: HashMap<String, usize>,
    total_count: usize,
}

impl SharedStrings {
    fn new() -> Self {
        Self {
            strings: Vec::new(),
            ids: HashMap::new(),
            total_count: 0,
        }
    }

    fn intern(&mut self, text: &str) -> usize {
        self.total_count += 1;
        if let Some(&id) = self.ids.get(text) {
            return id;
        }
        let id = self.strings.len();
        self.strings.push(text.to_owned());
        self.ids.insert(text.to_owned(), id);
        id
    }

    fn to_xml(&self) -> String {
        let mut out = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<sst xmlns="{MAIN_NS}" count="{}" uniqueCount="{}">"#,
            self.total_count,
            self.strings.len()
        );
        for text in &self.strings {
            let _ = write!(out, "<si><t>{}</t></si>", escape(text));
        }
        out.push_str("</sst>");
        out
    }
}

pub fn serialize_workbook(sheets: &[Sheet]) -> Parts {
    let mut styles = StyleTable::new();
    let mut sst = SharedStrings::new();
    let mut parts = IndexMap::new();
    for (idx, sheet) in sheets.iter().enumerate() {
        let xml = serialize_sheet(sheet, &mut styles, &mut sst);
        parts.insert(
            format!("xl/worksheets/sheet{}.xml", idx + 1),
            xml.into_bytes(),
        );
    }
    parts.insert(
        "[Content_Types].xml".into(),
        content_types_xml(sheets.len()).into_bytes(),
    );
    parts.insert("_rels/.rels".into(), root_rels_xml().into_bytes());
    parts.insert("docProps/app.xml".into(), app_xml(sheets).into_bytes());
    parts.insert("docProps/core.xml".into(), core_xml().into_bytes());
    parts.insert("xl/workbook.xml".into(), workbook_xml(sheets).into_bytes());
    parts.insert(
        "xl/_rels/workbook.xml.rels".into(),
        workbook_rels_xml(sheets.len()).into_bytes(),
    );
    parts.insert("xl/theme/theme1.xml".into(), THEME_XML.as_bytes().to_vec());
    parts.insert("xl/styles.xml".into(), styles.to_xml().into_bytes());
    parts.insert("xl/sharedStrings.xml".into(), sst.to_xml().into_bytes());
    parts
}

fn serialize_sheet(sheet: &Sheet, styles: &mut StyleTable, sst: &mut SharedStrings) -> String {
    let first_row = if sheet.cells.keys().any(|(r, _)| *r == 1) {
        1
    } else {
        2
    };
    let dim = format!(
        "B{first_row}:{}{}",
        col_name(sheet.last_col),
        sheet.rows_count
    );
    let mut out = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="{MAIN_NS}" xmlns:r="{REL_NS}" xmlns:mc="{MC_NS}" mc:Ignorable="x14ac" xmlns:x14ac="{X14AC_NS}">"#
    );
    let _ = write!(
        out,
        r#"<sheetPr><tabColor rgb="{}"/></sheetPr>"#,
        sheet.tab_color
    );
    let _ = write!(out, r#"<dimension ref="{dim}"/>"#);
    write_sheet_views(&mut out, sheet);
    out.push_str(r#"<sheetFormatPr defaultRowHeight="15" x14ac:dyDescent="0.25"/>"#);
    write_cols(&mut out, sheet);
    write_sheet_data(&mut out, sheet, styles, sst);
    write_cf_rules(&mut out, &sheet.cf_rules, styles);
    write_validations(&mut out, sheet);
    write_hyperlinks(&mut out, sheet);
    out.push_str(r#"<pageMargins left="0.7" right="0.7" top="0.75" bottom="0.75" header="0.3" footer="0.3"/></worksheet>"#);
    out
}

fn write_sheet_views(out: &mut String, sheet: &Sheet) {
    let selected = if sheet.tab_selected {
        r#" tabSelected="1""#
    } else {
        ""
    };
    let _ = write!(
        out,
        r#"<sheetViews><sheetView showGridLines="0"{selected} workbookViewId="0""#
    );
    if let Some((x, y)) = sheet.freeze {
        let top_left = format!("{}{}", col_name(x + 1), y + 1);
        if x > 0 {
            let _ = write!(
                out,
                r#"><pane xSplit="{x}" ySplit="{y}" topLeftCell="{top_left}" activePane="bottomRight" state="frozen"/><selection pane="topRight"/><selection pane="bottomLeft"/><selection pane="bottomRight" activeCell="{top_left}" sqref="{top_left}"/></sheetView></sheetViews>"#
            );
        } else {
            let _ = write!(
                out,
                r#"><pane ySplit="{y}" topLeftCell="{top_left}" activePane="bottomLeft" state="frozen"/><selection pane="bottomLeft" activeCell="{top_left}" sqref="{top_left}"/></sheetView></sheetViews>"#
            );
        }
    } else {
        out.push_str("/></sheetViews>");
    }
}

fn write_cols(out: &mut String, sheet: &Sheet) {
    out.push_str("<cols>");
    let mut c = 2u16;
    while c <= sheet.last_col {
        let w = *sheet.col_widths.get(&c).unwrap_or(&15);
        let start = c;
        while c < sheet.last_col && *sheet.col_widths.get(&(c + 1)).unwrap_or(&15) == w {
            c += 1;
        }
        let _ = write!(
            out,
            r#"<col min="{start}" max="{c}" width="{w}" customWidth="1"/>"#
        );
        c += 1;
    }
    out.push_str("</cols>");
}

fn write_sheet_data(
    out: &mut String,
    sheet: &Sheet,
    styles: &mut StyleTable,
    sst: &mut SharedStrings,
) {
    out.push_str("<sheetData>");
    let spans = format!("2:{}", sheet.last_col);
    let mut rows: BTreeMap<u16, Vec<(u16, &super::model::Cell)>> = BTreeMap::new();
    for (&(r, c), cell) in &sheet.cells {
        rows.entry(r).or_default().push((c, cell));
    }
    for (r, cells) in rows {
        if let Some(&ht) = sheet.row_heights.get(&r) {
            let _ = write!(
                out,
                r#"<row r="{r}" spans="{spans}" ht="{ht}" customHeight="1" x14ac:dyDescent="0.25">"#
            );
        } else {
            let _ = write!(
                out,
                r#"<row r="{r}" spans="{spans}" x14ac:dyDescent="0.25">"#
            );
        }
        for (c, cell) in cells {
            write_cell(out, r, c, cell, styles, sst);
        }
        out.push_str("</row>");
    }
    out.push_str("</sheetData>");
}

fn write_cell(
    out: &mut String,
    row: u16,
    col: u16,
    cell: &super::model::Cell,
    styles: &mut StyleTable,
    sst: &mut SharedStrings,
) {
    let addr = format!("{}{row}", col_name(col));
    let s = styles.intern_xf(&cell.style);
    if let Some(formula) = &cell.formula {
        let _ = write!(
            out,
            r#"<c r="{addr}" s="{s}"><f>{}</f></c>"#,
            escape(formula)
        );
        return;
    }
    match &cell.value {
        CellValue::Empty => {
            let _ = write!(out, r#"<c r="{addr}" s="{s}"/>"#);
        }
        CellValue::Text(text) => {
            let idx = sst.intern(text);
            let _ = write!(out, r#"<c r="{addr}" s="{s}" t="s"><v>{idx}</v></c>"#);
        }
        CellValue::Number(num) => {
            let _ = write!(out, r#"<c r="{addr}" s="{s}"><v>{num}</v></c>"#);
        }
    }
}

fn write_cf_rules(out: &mut String, rules: &[ConditionalFormat], styles: &mut StyleTable) {
    let mut grouped: IndexMap<&str, Vec<&ConditionalFormat>> = IndexMap::new();
    for rule in rules {
        grouped.entry(rule.sqref.as_str()).or_default().push(rule);
    }
    for (sqref, group) in grouped {
        let _ = write!(out, r#"<conditionalFormatting sqref="{sqref}">"#);
        for r in group {
            let dxf_id = styles.intern_dxf(&r.dxf);
            let op = r
                .operator
                .map_or(String::new(), |o| format!(r#" operator="{o}""#));
            let _ = write!(
                out,
                r#"<cfRule type="{}" dxfId="{dxf_id}" priority="{}"{op}><formula>{}</formula></cfRule>"#,
                r.kind,
                r.priority,
                escape(&r.formula)
            );
        }
        out.push_str("</conditionalFormatting>");
    }
}

fn write_validations(out: &mut String, sheet: &Sheet) {
    if sheet.validations.is_empty() {
        return;
    }
    let _ = write!(
        out,
        r#"<dataValidations count="{}">"#,
        sheet.validations.len()
    );
    for dv in &sheet.validations {
        let _ = write!(
            out,
            r#"<dataValidation type="list" sqref="{}"><formula1>{}</formula1></dataValidation>"#,
            dv.sqref,
            escape(&dv.list_formula)
        );
    }
    out.push_str("</dataValidations>");
}

fn write_hyperlinks(out: &mut String, sheet: &Sheet) {
    if sheet.hyperlinks.is_empty() {
        return;
    }
    out.push_str("<hyperlinks>");
    for link in &sheet.hyperlinks {
        let _ = write!(
            out,
            r#"<hyperlink ref="{}" location="{}" display="{}"/>"#,
            escape_attr(&link.cell_ref),
            escape_attr(&link.location),
            escape_attr(&link.display)
        );
    }
    out.push_str("</hyperlinks>");
}

fn content_types_xml(sheet_count: usize) -> String {
    let mut out = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="{CT_NS}"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>"#
    );
    for i in 1..=sheet_count {
        let _ = write!(
            out,
            r#"<Override PartName="/xl/worksheets/sheet{i}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#
        );
    }
    out.push_str(
        r#"<Override PartName="/xl/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/><Override PartName="/xl/sharedStrings.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/><Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/></Types>"#,
    );
    out
}

fn root_rels_xml() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="{PKG_REL_NS}"><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/><Relationship Id="rId1" Type="{REL_NS}/officeDocument" Target="xl/workbook.xml"/></Relationships>"#
    )
}

fn core_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:dcmitype="http://purl.org/dc/dcmitype/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dcterms:modified xsi:type="dcterms:W3CDTF">2026-01-01T00:00:00Z</dcterms:modified></cp:coreProperties>"#
        .to_owned()
}

fn app_xml(sheets: &[Sheet]) -> String {
    let n = sheets.len();
    let mut titles = String::new();
    for sheet in sheets {
        let _ = write!(titles, "<vt:lpstr>{}</vt:lpstr>", escape(sheet.name));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"><Application>Microsoft Excel</Application><DocSecurity>0</DocSecurity><ScaleCrop>false</ScaleCrop><HeadingPairs><vt:vector size="2" baseType="variant"><vt:variant><vt:lpstr>Worksheets</vt:lpstr></vt:variant><vt:variant><vt:i4>{n}</vt:i4></vt:variant></vt:vector></HeadingPairs><TitlesOfParts><vt:vector size="{n}" baseType="lpstr">{titles}</vt:vector></TitlesOfParts><LinksUpToDate>false</LinksUpToDate><SharedDoc>false</SharedDoc><HyperlinksChanged>false</HyperlinksChanged><AppVersion>16.0300</AppVersion></Properties>"#
    )
}

fn workbook_xml(sheets: &[Sheet]) -> String {
    let mut entries = String::new();
    for (idx, sheet) in sheets.iter().enumerate() {
        let id = idx + 1;
        let _ = write!(
            entries,
            r#"<sheet name="{}" sheetId="{id}" r:id="rId{id}"/>"#,
            escape_attr(sheet.name)
        );
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="{MAIN_NS}" xmlns:r="{REL_NS}"><fileVersion appName="xl" lastEdited="7" lowestEdited="7"/><workbookPr/><bookViews><workbookView xWindow="0" yWindow="0" windowWidth="28800" windowHeight="16000"/></bookViews><sheets>{entries}</sheets><calcPr calcId="191029"/></workbook>"#
    )
}

fn workbook_rels_xml(sheet_count: usize) -> String {
    let mut out = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="{PKG_REL_NS}">"#
    );
    for i in 1..=sheet_count {
        let _ = write!(
            out,
            r#"<Relationship Id="rId{i}" Type="{REL_NS}/worksheet" Target="worksheets/sheet{i}.xml"/>"#
        );
    }
    let theme_id = sheet_count + 1;
    let styles_id = sheet_count + 2;
    let sst_id = sheet_count + 3;
    let _ = write!(
        out,
        r#"<Relationship Id="rId{theme_id}" Type="{REL_NS}/theme" Target="theme/theme1.xml"/><Relationship Id="rId{styles_id}" Type="{REL_NS}/styles" Target="styles.xml"/><Relationship Id="rId{sst_id}" Type="{REL_NS}/sharedStrings" Target="sharedStrings.xml"/></Relationships>"#
    );
    out
}
