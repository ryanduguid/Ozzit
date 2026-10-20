//! Component assertions retained from the Python palette tests.
use super::*;

#[test]
fn hue_map_moves_teal_and_salmon_into_the_palette_bands() {
    assert_eq!(hue_map("40A0A0").as_deref(), Some("865A74"));
    assert_eq!(hue_map("C08060").as_deref(), Some("9B8F85"));
}

#[test]
fn recolour_remaps_a_six_digit_theme_value() {
    assert_eq!(
        recolour(r#"<a:srgbClr val="3F656F"/>"#).unwrap(),
        (r#"<a:srgbClr val="66023C"/>"#.into(), 1)
    );
}

#[test]
fn tab_colours_follow_the_module_roles() {
    for (old, new) in [
        ("4F485E", "E5E4E2"),
        ("2B1A54", "000000"),
        ("5C2D91", "66023C"),
        ("7A5AB5", "990024"),
    ] {
        assert_eq!(
            tab_colours(&format!(r#"<sheetPr><tabColor rgb="FF{old}"/></sheetPr>"#)),
            (
                format!(r#"<sheetPr><tabColor rgb="FF{new}"/></sheetPr>"#),
                1
            )
        );
    }
}

#[test]
fn theme_slots_keep_the_system_window_colours() {
    let theme = concat!(
        r#"<a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1>"#,
        r#"<a:accent1><a:srgbClr val="5C2D91"/></a:accent1>"#,
        r#"<a:hlink><a:srgbClr val="0563C1"/></a:hlink>"#,
    );
    assert_eq!(
        theme_slots(theme),
        concat!(
            r#"<a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1>"#,
            r#"<a:accent1><a:srgbClr val="66023C"/></a:accent1>"#,
            r#"<a:hlink><a:srgbClr val="66023C"/></a:hlink>"#,
        )
    );
}

#[test]
fn white_text_becomes_ivory_but_white_borders_stay() {
    let styles = concat!(
        r#"<font><color rgb="FFFFFFFF"/></font>"#,
        r#"<border><top><color rgb="FFFFFFFF"/></top></border>"#,
    );
    assert_eq!(
        ivory_text(styles),
        (
            concat!(
                r#"<font><color rgb="FFFFFFF0"/></font>"#,
                r#"<border><top><color rgb="FFFFFFFF"/></top></border>"#,
            )
            .into(),
            1
        )
    );
}

#[test]
fn passed_checks_get_their_own_green_font_and_links_keep_theirs() {
    let styles = concat!(
        r#"<styleSheet><fonts count="1"><font><color rgb="FF008000"/></font></fonts>"#,
        r#"<fills count="2"><fill><patternFill patternType="none"/></fill>"#,
        r#"<fill><patternFill patternType="solid"><fgColor rgb="FFDBE4E0"/></patternFill></fill></fills>"#,
        r#"<cellXfs count="2"><xf fontId="0" fillId="0"/><xf fontId="0" fillId="1"/></cellXfs>"#,
        r#"<dxfs count="2"><dxf><font><color rgb="FF008000"/></font>"#,
        r#"<fill><patternFill><bgColor rgb="FFDBE4E0"/></patternFill></fill></dxf>"#,
        r#"<dxf><font><color rgb="FF008000"/></font></dxf></dxfs></styleSheet>"#,
    );
    let (result, count) = passed_green(styles).unwrap();
    assert_eq!(count, 2);
    assert!(result.contains(concat!(
        r#"<fonts count="2"><font><color rgb="FF008000"/></font>"#,
        r#"<font><color rgb="FF0F4B32"/></font></fonts>"#,
    )));
    assert!(result.contains(r#"<xf fontId="0" fillId="0"/><xf fontId="1" fillId="1"/>"#));
    assert!(result.contains(r#"<dxf><font><color rgb="FF008000"/></font></dxf>"#));
    assert_eq!(passed_green(&result).unwrap(), (result, 0));
}

#[test]
fn passed_checks_sharing_a_font_share_one_copy() {
    let styles = concat!(
        r#"<styleSheet><fonts count="2"><font><color rgb="FF008000"/></font><font><b/></font></fonts>"#,
        r#"<fills count="1"><fill><patternFill patternType="solid"><fgColor rgb="FFDBE4E0"/></patternFill></fill></fills>"#,
        r#"<cellXfs count="4"><xf fontId="0" fillId="0"/><xf fontId="0" fillId="0"/>"#,
        r#"<xf fontId="1" fillId="0"/><xf numFmtId="0"/></cellXfs></styleSheet>"#,
    );
    let (result, count) = passed_green(styles).unwrap();
    assert_eq!(count, 2);
    assert!(result.contains(r#"<font><b/></font><font><color rgb="FF0F4B32"/></font></fonts>"#));
    assert!(result.contains(concat!(
        r#"<xf fontId="2" fillId="0"/><xf fontId="2" fillId="0"/>"#,
        r#"<xf fontId="1" fillId="0"/><xf numFmtId="0"/>"#,
    )));
}

#[test]
fn passed_green_leaves_styles_without_cell_formats() {
    let styles = "<styleSheet><fonts/></styleSheet>";
    assert_eq!(passed_green(styles).unwrap(), (styles.into(), 0));
}

#[test]
fn hue_map_leaves_greys_alone() {
    assert_eq!(hue_map("808080"), None);
}

#[test]
fn recolour_refuses_a_colour_still_remappable_after_one_pass() {
    assert_eq!(
        recolour(r#"<color rgb="FFE07040"/>"#).unwrap_err(),
        "remap is not a fixed point: A28F7E is still remappable after one pass"
    );
}
