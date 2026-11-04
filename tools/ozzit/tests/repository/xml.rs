//! The ElementTree lookups the Python tests made, over roxmltree.
use roxmltree::{Document, Node};

pub const MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
pub const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
pub const PACKAGE_REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";

pub fn parse(text: &str) -> Document<'_> {
    Document::parse(text).unwrap_or_else(|e| panic!("{e}"))
}

/// The child elements, as iterating an `Element` gives them.
pub fn elements<'a, 'i>(node: Node<'a, 'i>) -> impl Iterator<Item = Node<'a, 'i>> {
    node.children().filter(Node::is_element)
}

/// `node[index]`.
pub fn nth<'a, 'i>(node: Node<'a, 'i>, index: usize) -> Node<'a, 'i> {
    elements(node)
        .nth(index)
        .unwrap_or_else(|| panic!("<{}> has no child {index}", node.tag_name().name()))
}

/// `findall("ns:name")`.
pub fn children<'a, 'i>(
    node: Node<'a, 'i>,
    ns: &'static str,
    name: &'static str,
) -> impl Iterator<Item = Node<'a, 'i>> {
    elements(node).filter(move |n| n.has_tag_name((ns, name)))
}

/// `find("ns:name")`.
pub fn child<'a, 'i>(
    node: Node<'a, 'i>,
    ns: &'static str,
    name: &'static str,
) -> Option<Node<'a, 'i>> {
    children(node, ns, name).next()
}

/// `findall("ns:a/ns:b/...")`, each step a child step.
pub fn path<'a, 'i>(
    node: Node<'a, 'i>,
    ns: &'static str,
    steps: &[&'static str],
) -> Vec<Node<'a, 'i>> {
    steps.iter().fold(vec![node], |found, step| {
        found
            .into_iter()
            .flat_map(|n| children(n, ns, step))
            .collect()
    })
}

/// `findall(".//ns:name")`: every descendant with the tag, in document order.
pub fn descendants<'a, 'i>(
    node: Node<'a, 'i>,
    ns: &'static str,
    name: &'static str,
) -> impl Iterator<Item = Node<'a, 'i>> {
    node.descendants()
        .skip(1)
        .filter(move |n| n.has_tag_name((ns, name)))
}

/// `"".join(node.itertext())`.
pub fn text(node: Node<'_, '_>) -> String {
    node.descendants()
        .filter(Node::is_text)
        .filter_map(|n| n.text())
        .collect()
}

/// `find(".//m:c[@r='address']")`.
pub fn cell<'a, 'i>(root: Node<'a, 'i>, address: &str) -> Option<Node<'a, 'i>> {
    descendants(root, MAIN, "c").find(|c| c.attribute("r") == Some(address))
}

/// The attribute as an index, with ElementTree's `int(attrib.get(name, "0"))`.
pub fn index(node: Node<'_, '_>, name: &str) -> usize {
    let value = node.attribute(name).unwrap_or("0");
    value
        .parse()
        .unwrap_or_else(|_| panic!("{name}={value:?} is not an index"))
}
