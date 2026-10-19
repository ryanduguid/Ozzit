//! Residue pass: parts the workbook carries that nothing in it reads.
//!
//! Drops the hidden FMTs sheet, the per-sheet custom properties, the stale
//! custom-function declaration and unused differential formats and named cell
//! styles, and freezes the label columns of the wide demonstration sheets.
use super::*;
use num_bigint::BigInt;
use std::collections::{BTreeMap, BTreeSet};

const RESIDUE_SHEET: &str = "FMTs";
const FREEZE: [(&str, &str); 6] = [
    ("oz.IsOccurrenceDateλ", "H"),
    ("oz.Amortiseλ", "I"),
    ("oz.SumAmortiseλ", "I"),
    ("oz.Depreciateλ", "L"),
    ("oz.SumPeriodsλ", "I"),
    ("oz.TimelineOffsetλ", "C"),
];

/// Group `group` of every match of `pattern`, read as Python's `int()`.
fn ints(pattern: &str, text: &str, group: usize) -> Result<Vec<BigInt>> {
    captures(pattern, text)
        .iter()
        .map(|m| py::parse_int(&m[group], 10))
        .collect()
}

fn int(text: &str) -> Result<i64> {
    use num_traits::ToPrimitive;
    py::parse_int(text, 10)?
        .to_i64()
        .ok_or_else(|| "index out of range".into())
}

fn set_text(parts: &mut Parts, name: &str, text: String) {
    parts.insert(name.into(), text.into_bytes());
}

/// Removes sheet `name` from workbook.xml and returns its relationship id,
/// or `None` when the workbook has no such sheet.
fn remove_from_book(parts: &mut Parts, name: &str) -> Result<Option<String>> {
    let book = part_text(parts, BOOK)?;
    let sheets = captures(&bounded(r"<sheet\b[^>]*/>"), &book)
        .into_iter()
        .map(|m| m[0].clone())
        .collect::<Vec<_>>();
    let Some(position) = sheets
        .iter()
        .position(|s| s.contains(&format!("name=\"{name}\"")))
    else {
        return Ok(None);
    };
    let element = &sheets[position];
    let rid = captures(r#"r:id="([^"]+)""#, element)
        .first()
        .map(|m| m[1].clone())
        .ok_or("'NoneType' object has no attribute 'group'")?;
    let book = book.replacen(element.as_str(), "", 1);
    // Every index is read as Python's int() before any is replaced, so one it
    // refuses fails the pass rather than becoming a different scope.
    let pattern = r#"localSheetId="(\d+)""#;
    let mut ids = ints(pattern, &book, 1)?.into_iter();
    let position = BigInt::from(position);
    let book = sub_fn(pattern, &book, |_| {
        let n = ids.next().expect("one index per match");
        let n = if n > position { n - 1 } else { n };
        format!("localSheetId=\"{n}\"")
    });
    set_text(parts, BOOK, book);
    Ok(Some(rid))
}

/// Removes relationship `rid` from the workbook's relationships and returns its target part.
fn remove_relationship(parts: &mut Parts, name: &str, rid: &str) -> Result<String> {
    let rels_name = "xl/_rels/workbook.xml.rels";
    let rels = part_text(parts, rels_name)?;
    let relationship = captures(&format!(r#"<Relationship Id="{rid}"[^>]*/>"#), &rels)
        .into_iter()
        .next()
        .ok_or_else(|| format!("{name}: relationship {rid} missing"))?;
    let target_attr = captures(r#"Target="([^"]+)""#, &relationship[0])
        .first()
        .map(|m| m[1].clone())
        .ok_or("'NoneType' object has no attribute 'group'")?;
    let target = resolve_target(BOOK, &target_attr);
    set_text(
        parts,
        rels_name,
        rels.replacen(relationship[0].as_str(), "", 1),
    );
    Ok(target)
}

/// Removes the sheet part `target`, its relationships and its tables, with
/// their content types, and returns the table part names.
fn remove_sheet_parts(parts: &mut Parts, target: &str) -> Result<Vec<String>> {
    let sheet_rels = format!(
        "{}.rels",
        target.replace("worksheets/", "worksheets/_rels/")
    );
    let mut tables = Vec::new();
    if parts.contains_key(&sheet_rels) {
        tables = captures(
            r#"Target="\.\./tables/([^"]+)""#,
            &part_text(parts, &sheet_rels)?,
        )
        .into_iter()
        .map(|m| m[1].clone())
        .collect();
        parts.shift_remove(&sheet_rels);
    }
    parts
        .shift_remove(target)
        .ok_or_else(|| py::repr_str(target))?;
    for table in &tables {
        let table = format!("xl/tables/{table}");
        parts
            .shift_remove(&table)
            .ok_or_else(|| py::repr_str(&table))?;
    }

    let types = "[Content_Types].xml";
    let mut ct = part_text(parts, types)?;
    ct = sub(
        &format!(
            r#"<Override PartName="/{}"[^>]*/>"#,
            fancy_regex::escape(target)
        ),
        &ct,
        "",
    );
    for table in &tables {
        ct = sub(
            &format!(
                r#"<Override PartName="/xl/tables/{}"[^>]*/>"#,
                fancy_regex::escape(table)
            ),
            &ct,
            "",
        );
    }
    set_text(parts, types, ct);
    Ok(tables)
}

/// Removes sheet `name` from the titles and worksheet count in docProps/app.xml.
fn remove_title(parts: &mut Parts, name: &str) -> Result<()> {
    let app_name = "docProps/app.xml";
    let app = part_text(parts, app_name)?;
    let needle = format!("<vt:lpstr>{name}</vt:lpstr>");
    let listed = count(&app, &needle);
    if listed != 1 {
        return Err(format!(
            "docProps/app.xml lists {name} {listed} times, expected 1"
        ));
    }
    let app = app.replacen(&needle, "", 1);
    let decrement = |pattern: &str, text: &str| -> Result<String> {
        let mut counts = ints(pattern, text, 2)?.into_iter();
        Ok(sub_fn(pattern, text, |m| {
            let n = counts.next().expect("one count per match");
            format!("{}{}{}", &m[1], n - 1, &m[3])
        }))
    };
    let app = decrement(
        r"(<vt:lpstr>Worksheets</vt:lpstr></vt:variant><vt:variant><vt:i4>)(\d+)(</vt:i4>)",
        &app,
    )?;
    let app = decrement(r#"(<TitlesOfParts><vt:vector size=")(\d+)(")"#, &app)?;
    set_text(parts, app_name, app);
    Ok(())
}

fn drop_sheet(parts: &mut Parts, name: &str, log: &mut Vec<String>) -> Result<()> {
    let Some(rid) = remove_from_book(parts, name)? else {
        return Ok(());
    };
    let target = remove_relationship(parts, name, &rid)?;
    let tables = remove_sheet_parts(parts, &target)?;
    remove_title(parts, name)?;
    log.push(format!(
        "removed the hidden {name} sheet, its table and its {} table part(s)",
        tables.len()
    ));
    Ok(())
}

/// Drops the custom-property relationships and elements from every worksheet.
fn strip_sheet_properties(parts: &mut Parts) -> Result<()> {
    let rels = parts
        .keys()
        .filter(|n| n.starts_with("xl/worksheets/_rels/"))
        .cloned()
        .collect::<Vec<_>>();
    for name in rels {
        let text = sub(
            r#"<Relationship [^>]*Target="\.\./customProperty\d+\.bin"[^>]*/>"#,
            &part_text(parts, &name)?,
            "",
        );
        if matches(r"<Relationships[^>]*>\s*</Relationships>", &text) {
            parts.shift_remove(&name);
        } else {
            set_text(parts, &name, text);
        }
    }
    let sheets = parts
        .keys()
        .filter(|n| fullmatch(r"xl/worksheets/sheet\d+\.xml", n))
        .cloned()
        .collect::<Vec<_>>();
    for name in sheets {
        let text = part_text(parts, &name)?;
        let new = sub(
            r"<customProperties>\s*(?:<customPr [^>]*/>\s*)+</customProperties>",
            &text,
            "",
        );
        if new != text {
            set_text(parts, &name, new);
        }
    }
    Ok(())
}

fn drop_custom_properties(parts: &mut Parts, log: &mut Vec<String>) -> Result<()> {
    let bins = parts
        .keys()
        .filter(|n| fullmatch(r"xl/customProperty\d+\.bin", n))
        .cloned()
        .collect::<Vec<_>>();
    if bins.is_empty() {
        return Ok(());
    }
    for name in &bins {
        parts.shift_remove(name);
    }
    strip_sheet_properties(parts)?;
    let types = "[Content_Types].xml";
    let mut ct = part_text(parts, types)?;
    if !parts.keys().any(|n| n.to_lowercase().ends_with(".bin")) {
        ct = sub(r#"<Default Extension="bin"[^>]*/>"#, &ct, "");
    }
    set_text(parts, types, ct);
    log.push(format!(
        "removed {} per-sheet custom properties",
        bins.len()
    ));
    Ok(())
}

fn drop_custom_function_declaration(parts: &mut Parts, log: &mut Vec<String>) -> Result<()> {
    let name = "xl/webextensions/webextension1.xml";
    if !parts.contains_key(name) {
        return Ok(());
    }
    let text = part_text(parts, name)?;
    let pattern = r"(?s)<we:extLst>.*?</we:extLst>";
    if re(pattern).find(&text).ok().flatten().is_some() {
        set_text(parts, name, sub(pattern, &text, ""));
        log.push(
            "removed the stale custom-function declaration from the Excel Labs reference".into(),
        );
    }
    Ok(())
}

/// The worksheet and table parts in archive order, and the text of each.
fn style_users(parts: &Parts) -> Result<(Vec<String>, Vec<String>)> {
    let mut sheets = BTreeMap::new();
    let mut tables = BTreeMap::new();
    let mut order = Vec::new();
    for name in parts.keys() {
        if fullmatch(r"xl/worksheets/sheet\d+\.xml", name) {
            order.push((true, name.clone()));
        } else if fullmatch(r"xl/tables/table\d+\.xml", name) {
            order.push((false, name.clone()));
        }
    }
    for (is_sheet, name) in &order {
        let text = part_text(parts, name)?;
        if *is_sheet {
            sheets.insert(name.clone(), text);
        } else {
            tables.insert(name.clone(), text);
        }
    }
    // Sheets before tables, each in name order, as Python scanned them.
    let texts = sheets.into_values().chain(tables.into_values()).collect();
    Ok((order.into_iter().map(|(_, name)| name).collect(), texts))
}

/// Every differential format index the sheets, tables and table styles use.
fn used_dxfs(texts: &[String], table_styles: Option<&String>) -> Result<BTreeSet<i64>> {
    let dxf_ref = bounded(r#"\b[A-Za-z]*[dD]xfId="(\d+)""#);
    let mut used = BTreeSet::new();
    for text in texts.iter().chain(table_styles) {
        for m in captures(&dxf_ref, text) {
            used.insert(int(&m[1])?);
        }
    }
    Ok(used)
}

/// Rewrites every dxfId in the parts and table styles to its position in `keep`.
fn renumber_dxfs(
    parts: &mut Parts,
    styles: &mut String,
    order: &[String],
    table_styles: Option<&String>,
    keep: &[i64],
) -> Result<()> {
    let renumber = keep
        .iter()
        .enumerate()
        .map(|(new, old)| (*old, new))
        .collect::<BTreeMap<_, _>>();
    let dxf_attr = bounded(r#"\b([A-Za-z]*[dD]xfId)="(\d+)""#);
    let rewrite = |text: &str| {
        sub_fn(&dxf_attr, text, |m| {
            format!(
                "{}=\"{}\"",
                &m[1],
                renumber[&int(&m[2]).unwrap_or_default()]
            )
        })
    };
    for name in order {
        let text = part_text(parts, name)?;
        let new = rewrite(&text);
        if new != text {
            set_text(parts, name, new);
        }
    }
    if let Some(found) = table_styles {
        *styles = styles.replacen(found.as_str(), &rewrite(found), 1);
    }
    Ok(())
}

fn prune_dxfs(
    parts: &mut Parts,
    styles: &mut String,
    (order, texts): &(Vec<String>, Vec<String>),
    log: &mut Vec<String>,
) -> Result<()> {
    let dxfs = re(r#"(?s)<dxfs count="(\d+)">(.*?)</dxfs>"#)
        .captures(styles.as_str())
        .ok()
        .flatten()
        .map(|m| (m[0].to_string(), m[1].to_string(), m[2].to_string()))
        .ok_or("styles.xml has no dxfs collection")?;
    let entries = captures(r"(?s)<dxf>.*?</dxf>|<dxf/>", &dxfs.2)
        .into_iter()
        .map(|m| m[0].clone())
        .collect::<Vec<_>>();
    if entries.len() as i64 != int(&dxfs.1)? {
        return Err("styles.xml dxfs count disagrees with its entries".into());
    }
    let table_styles = re(&bounded(r"(?s)<tableStyles\b.*?</tableStyles>"))
        .find(styles.as_str())
        .ok()
        .flatten()
        .map(|m| m.as_str().to_string());
    let used = used_dxfs(texts, table_styles.as_ref())?;
    if used.last().is_some_and(|max| *max >= entries.len() as i64) {
        return Err("a dxfId points past the dxfs collection".into());
    }
    let keep = used.iter().copied().collect::<Vec<_>>();
    if keep.len() != entries.len() {
        renumber_dxfs(parts, styles, order, table_styles.as_ref(), &keep)?;
        let kept = keep
            .iter()
            .map(|i| entries[*i as usize].as_str())
            .collect::<String>();
        *styles = styles.replacen(
            &dxfs.0,
            &format!("<dxfs count=\"{}\">{kept}</dxfs>", keep.len()),
            1,
        );
        log.push(format!(
            "dropped {} unused differential formats",
            entries.len() - keep.len()
        ));
    }
    Ok(())
}

/// The whole match and every group of `pattern`'s first match, unmatched groups empty.
fn first_match(pattern: &str, text: &str) -> Option<Vec<String>> {
    re(pattern).captures(text).ok().flatten().map(|m| {
        (0..m.len())
            .map(|i| m.get(i).map_or(String::new(), |g| g.as_str().to_string()))
            .collect::<Vec<_>>()
    })
}

/// The named cell styles whose format is kept, renumbered to its new position.
fn kept_style_names(
    style_names: &[String],
    renumber: &BTreeMap<i64, usize>,
) -> Result<Vec<String>> {
    let mut kept_names = Vec::new();
    for element in style_names {
        let id = captures(r#"xfId="(\d+)""#, element)
            .first()
            .map(|m| m[1].clone())
            .ok_or("'NoneType' object has no attribute 'group'")?;
        let id = int(&id)?;
        if let Some(new) = renumber.get(&id) {
            kept_names.push(element.replacen(
                &format!("xfId=\"{id}\""),
                &format!("xfId=\"{new}\""),
                1,
            ));
        }
    }
    Ok(kept_names)
}

/// The cell style collections: cellStyleXfs, cellStyles and cellXfs matches,
/// each with its groups.
type StyleCollections = (Vec<String>, Vec<String>, Vec<String>);

/// Keeps only the cell style formats in `keep_xf`, renumbering their users.
fn rewrite_named_styles(
    styles: &mut String,
    (xfs, names, cell_xfs): &StyleCollections,
    style_xfs: &[String],
    style_names: &[String],
    keep_xf: &[i64],
) -> Result<()> {
    let renumber = keep_xf
        .iter()
        .enumerate()
        .map(|(new, old)| (*old, new))
        .collect::<BTreeMap<_, _>>();
    let kept_names = kept_style_names(style_names, &renumber)?;
    let new_cell_xfs = sub_fn(r#"xfId="(\d+)""#, &cell_xfs[0], |m| {
        format!("xfId=\"{}\"", renumber[&int(&m[1]).unwrap_or_default()])
    });
    *styles = styles.replacen(&cell_xfs[0], &new_cell_xfs, 1);
    let kept_xfs = keep_xf
        .iter()
        .map(|i| style_xfs[*i as usize].as_str())
        .collect::<String>();
    *styles = styles.replacen(
        &xfs[0],
        &format!(
            "<cellStyleXfs count=\"{}\">{kept_xfs}</cellStyleXfs>",
            keep_xf.len()
        ),
        1,
    );
    *styles = styles.replacen(
        &names[0],
        &format!(
            "<cellStyles count=\"{}\">{}</cellStyles>",
            kept_names.len(),
            kept_names.concat()
        ),
        1,
    );
    Ok(())
}

fn prune_named_styles(styles: &mut String, log: &mut Vec<String>) -> Result<()> {
    let xfs = first_match(
        r#"(?s)<cellStyleXfs count="(\d+)">(.*?)</cellStyleXfs>"#,
        styles,
    );
    let names = first_match(
        r#"(?s)<cellStyles count="(\d+)">(.*?)</cellStyles>"#,
        styles,
    );
    let cell_xfs = first_match(&bounded(r"(?s)<cellXfs\b.*?</cellXfs>"), styles);
    let (Some(xfs), Some(names), Some(cell_xfs)) = (xfs, names, cell_xfs) else {
        return Err("styles.xml lacks a cell style collection".into());
    };
    let style_xfs = captures(&bounded(r"(?s)<xf\b[^>]*/>|<xf\b[^>]*>.*?</xf>"), &xfs[2])
        .into_iter()
        .map(|m| m[0].clone())
        .collect::<Vec<_>>();
    let style_names = captures(&bounded(r"<cellStyle\b[^>]*/>"), &names[2])
        .into_iter()
        .map(|m| m[0].clone())
        .collect::<Vec<_>>();
    let mut used_xf = BTreeSet::from([0]);
    for m in captures(r#"xfId="(\d+)""#, &cell_xfs[0]) {
        used_xf.insert(int(&m[1])?);
    }
    if used_xf
        .last()
        .is_some_and(|max| *max >= style_xfs.len() as i64)
    {
        return Err("an xfId points past the cellStyleXfs collection".into());
    }
    let keep_xf = used_xf.iter().copied().collect::<Vec<_>>();
    if keep_xf.len() != style_xfs.len() {
        let collections = (xfs, names, cell_xfs);
        rewrite_named_styles(styles, &collections, &style_xfs, &style_names, &keep_xf)?;
        log.push(format!(
            "dropped {} unused named cell styles",
            style_xfs.len() - keep_xf.len()
        ));
    }
    Ok(())
}

fn prune_styles(parts: &mut Parts, log: &mut Vec<String>) -> Result<()> {
    let mut styles = part_text(parts, "xl/styles.xml")?;
    let users = style_users(parts)?;
    prune_dxfs(parts, &mut styles, &users, log)?;
    prune_named_styles(&mut styles, log)?;
    set_text(parts, "xl/styles.xml", styles);
    Ok(())
}

fn freeze_panes(parts: &mut Parts, log: &mut Vec<String>) -> Result<()> {
    let targets = sheet_targets(parts)?;
    let mut frozen = 0;
    for (sheet, column) in FREEZE {
        let part =
            lookup(&targets, sheet).ok_or_else(|| format!("{sheet} is not in the workbook"))?;
        let text = part_text(parts, part)?;
        if text.contains("<pane ") {
            continue;
        }
        let Some(view) = re(&bounded(r"<sheetView\b[^>]*>"))
            .find(&text)
            .ok()
            .flatten()
            .map(|m| m.as_str().to_string())
        else {
            return Err(format!("{sheet} has no sheetView"));
        };
        let pane = format!(
            "<pane xSplit=\"{}\" topLeftCell=\"{column}1\" activePane=\"topRight\" state=\"frozen\"/>",
            column_number(column) - 1
        );
        let selection =
            format!("<selection pane=\"topRight\" activeCell=\"{column}1\" sqref=\"{column}1\"/>");
        let text = text.replacen(&view, &format!("{view}{pane}"), 1).replacen(
            "</sheetView>",
            &format!("{selection}</sheetView>"),
            1,
        );
        set_text(parts, part, text);
        frozen += 1;
    }
    if frozen > 0 {
        log.push(format!(
            "froze the label columns on {frozen} wide demonstration sheets"
        ));
    }
    Ok(())
}

pub fn run(workbook: &Path) -> Result<Vec<String>> {
    let mut parts = read_parts(workbook)?;
    let mut log = Vec::new();
    drop_sheet(&mut parts, RESIDUE_SHEET, &mut log)?;
    drop_custom_properties(&mut parts, &mut log)?;
    drop_custom_function_declaration(&mut parts, &mut log)?;
    prune_styles(&mut parts, &mut log)?;
    freeze_panes(&mut parts, &mut log)?;
    if !log.is_empty() {
        write_deterministic(workbook, &parts)?;
    }
    Ok(log)
}

pub fn command(args: &[String]) -> Output {
    let args = match pass_args(args, "ozzit remove_residue [workbook]", false) {
        Ok(args) => args,
        Err(output) => return output,
    };
    match run(&args.workbook.fs()) {
        Err(e) => Output::stderr(format!("FAIL: {e}")),
        Ok(mut log) => {
            if log.is_empty() {
                log.push("residue already removed; no changes".into());
            }
            log.push(format!("OK: {}", args.workbook));
            Output::ok(log)
        }
    }
}
