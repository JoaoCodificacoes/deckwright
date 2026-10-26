use crate::{Error, Result};
use quick_xml::{NsReader, events::Event};
use std::collections::BTreeMap;
#[derive(Debug, Default, serde::Serialize)]
pub(crate) struct Node {
    pub name: String,
    pub namespace: String,
    pub attrs: BTreeMap<String, String>,
    pub children: Vec<Node>,
    pub text: String,
}
impl Node {
    pub fn descendants<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> {
        let mut out = Vec::new();
        self.collect(name, &mut out);
        out.into_iter()
    }
    fn collect<'a>(&'a self, name: &str, out: &mut Vec<&'a Node>) {
        if self.name == name && self.namespace == namespace_for(name) {
            out.push(self);
        }
        for c in &self.children {
            c.collect(name, out);
        }
    }
}
pub(crate) fn parse(bytes: &[u8], part: &str, max_depth: usize) -> Result<Node> {
    let bad = |message: String| Error::InvalidPart {
        part: part.into(),
        message,
    };
    let mut r = NsReader::from_reader(bytes);
    let mut stack = vec![Node::default()];
    loop {
        match r.read_event().map_err(|e| bad(e.to_string()))? {
            event @ (Event::Start(_) | Event::Empty(_)) => {
                let empty = matches!(event, Event::Empty(_));
                let e = match event {
                    Event::Start(e) | Event::Empty(e) => e,
                    _ => unreachable!(),
                };
                let namespace = match r.resolve_element(e.name()).0 {
                    quick_xml::name::ResolveResult::Bound(ns) => {
                        String::from_utf8_lossy(ns.as_ref()).into_owned()
                    }
                    quick_xml::name::ResolveResult::Unbound => String::new(),
                    quick_xml::name::ResolveResult::Unknown(_) => {
                        return Err(bad("unbound XML prefix".into()));
                    }
                };
                let mut n = Node {
                    namespace,
                    name: String::from_utf8_lossy(e.local_name().as_ref()).into(),
                    ..Node::default()
                };
                for a in e.attributes() {
                    let a = a.map_err(|e| bad(e.to_string()))?;
                    let duplicate = n.attrs.insert(
                        {
                            let (ns, local) = r.resolve_attribute(a.key);
                            match ns {
                                quick_xml::name::ResolveResult::Bound(ns) => format!(
                                    "{{{}}}{}",
                                    String::from_utf8_lossy(ns.as_ref()),
                                    String::from_utf8_lossy(local.as_ref())
                                ),
                                quick_xml::name::ResolveResult::Unbound => {
                                    String::from_utf8_lossy(local.as_ref()).into_owned()
                                }
                                quick_xml::name::ResolveResult::Unknown(_) => {
                                    return Err(bad("unbound attribute prefix".into()));
                                }
                            }
                        },
                        a.decode_and_unescape_value(r.decoder())
                            .map_err(|e| bad(e.to_string()))?
                            .into_owned(),
                    );
                    if duplicate.is_some() {
                        return Err(bad("duplicate expanded attribute name".into()));
                    }
                }
                if stack.len() > max_depth {
                    return Err(Error::Limit(format!("XML depth in {part}")));
                }
                if empty {
                    stack
                        .last_mut()
                        .ok_or_else(|| bad("invalid XML stack".into()))?
                        .children
                        .push(n);
                } else {
                    stack.push(n);
                }
            }
            Event::End(_) => {
                if stack.len() < 2 {
                    return Err(bad("unexpected closing tag".into()));
                }
                let n = stack.pop().ok_or_else(|| bad("invalid XML stack".into()))?;
                stack
                    .last_mut()
                    .ok_or_else(|| bad("invalid XML stack".into()))?
                    .children
                    .push(n);
            }
            Event::Text(e) => stack
                .last_mut()
                .ok_or_else(|| bad("invalid XML stack".into()))?
                .text
                .push_str(&e.xml_content().map_err(|e| bad(e.to_string()))?),
            Event::CData(e) => stack
                .last_mut()
                .ok_or_else(|| bad("invalid XML stack".into()))?
                .text
                .push_str(&e.decode().map_err(|e| bad(e.to_string()))?),
            Event::GeneralRef(e) => {
                let t = e.decode().map_err(|e| bad(e.to_string()))?;
                let entity = format!("&{t};");
                let decoded =
                    quick_xml::escape::unescape(&entity).map_err(|e| bad(e.to_string()))?;
                stack
                    .last_mut()
                    .ok_or_else(|| bad("invalid XML stack".into()))?
                    .text
                    .push_str(&decoded);
            }
            Event::DocType(_) => return Err(bad("DTD declarations are forbidden".into())),
            Event::Eof => break,
            _ => {}
        }
    }
    if stack.len() != 1 || stack[0].children.len() != 1 || !stack[0].text.trim().is_empty() {
        return Err(bad("expected one complete XML document".into()));
    }
    Ok(stack
        .pop()
        .ok_or_else(|| bad("invalid XML stack".into()))?
        .children
        .remove(0))
}

fn namespace_for(name: &str) -> &str {
    match name {
        "Relationship" => "http://schemas.openxmlformats.org/package/2006/relationships",
        "t" | "tbl" | "custGeom" | "videoFile" | "audioFile" => {
            "http://schemas.openxmlformats.org/drawingml/2006/main"
        }
        _ => "http://schemas.openxmlformats.org/presentationml/2006/main",
    }
}
