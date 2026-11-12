//! A small element tree for HWPX parts: local names, attributes and text, with character and
//! entity references resolved.

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

/// Deepest element nesting kept (hostile files); deeper elements are left out.
const MAX_XML_DEPTH: usize = 256;

/// Most elements a part keeps (hostile files); later ones are left out.
const MAX_ELEMENTS: usize = 4_000_000;

/// An element: its local name (prefix dropped), attributes (local names) and content.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Elem {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Elem(Elem),
    Text(String),
}

impl Elem {
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn int(&self, key: &str) -> Option<i64> {
        self.attr(key)?.trim().parse().ok()
    }

    /// An attribute that is `1` or `true`.
    pub fn flag(&self, key: &str) -> bool {
        self.attr(key).is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
    }

    pub fn elems(&self) -> impl Iterator<Item = &Elem> {
        self.children.iter().filter_map(|n| if let Node::Elem(e) = n { Some(e) } else { None })
    }

    pub fn child(&self, name: &str) -> Option<&Elem> {
        self.elems().find(|e| e.name == name)
    }

    /// The first element named `name` at any depth below this one (depth first).
    pub fn find(&self, name: &str) -> Option<&Elem> {
        self.elems().find_map(|e| if e.name == name { Some(e) } else { e.find(name) })
    }

    /// Every element named `name` below this one, outermost first, not looking inside a match.
    pub fn find_all<'a>(&'a self, name: &str, out: &mut Vec<&'a Elem>) {
        for e in self.elems() {
            if e.name == name {
                out.push(e);
            } else {
                e.find_all(name, out);
            }
        }
    }
}

fn local(q: &[u8]) -> String {
    let q = String::from_utf8_lossy(q);
    q.rsplit(':').next().unwrap_or_default().to_string()
}

fn elem(e: &BytesStart<'_>) -> Elem {
    let attrs = e
        .attributes()
        .with_checks(false)
        .flatten()
        .take(256)
        .map(|a| {
            let v = a
                .normalized_value(quick_xml::XmlVersion::Explicit1_0)
                .map(|c| c.into_owned())
                .unwrap_or_else(|_| String::from_utf8_lossy(&a.value).into_owned());
            (local(a.key.as_ref()), v)
        })
        .collect();
    Elem { name: local(e.name().as_ref()), attrs, children: Vec::new() }
}

/// The root element of `xml`. Malformed XML ends the tree where it breaks.
pub fn parse(xml: &[u8]) -> Option<Elem> {
    let mut r = Reader::from_reader(xml);
    let cfg = r.config_mut();
    cfg.trim_text(false);
    cfg.check_end_names = false;
    cfg.allow_unmatched_ends = true;
    // The document node holds the root element.
    let mut open = vec![Elem::default()];
    let mut skipped = 0usize;
    let mut count = 0usize;
    let mut buf = Vec::new();
    loop {
        buf.clear();
        let ev = r.read_event_into(&mut buf);
        match ev {
            Ok(Event::Start(e)) => {
                if skipped > 0 || open.len() > MAX_XML_DEPTH || count >= MAX_ELEMENTS {
                    skipped += 1;
                } else {
                    count += 1;
                    open.push(elem(&e));
                }
            }
            Ok(Event::Empty(e)) => {
                if skipped == 0
                    && count < MAX_ELEMENTS
                    && let Some(parent) = open.last_mut()
                {
                    count += 1;
                    parent.children.push(Node::Elem(elem(&e)));
                }
            }
            Ok(Event::End(_)) => {
                if skipped > 0 {
                    skipped -= 1;
                } else if open.len() > 1
                    && let Some(e) = open.pop()
                    && let Some(parent) = open.last_mut()
                {
                    parent.children.push(Node::Elem(e));
                }
            }
            Ok(Event::Text(t)) => push_text(&mut open, skipped, &t.decode().map(|c| c.into_owned()).unwrap_or_default()),
            Ok(Event::CData(t)) => push_text(&mut open, skipped, &String::from_utf8_lossy(&t)),
            Ok(Event::GeneralRef(g)) => {
                let s = match g.as_ref() {
                    b"amp" => "&".to_string(),
                    b"lt" => "<".to_string(),
                    b"gt" => ">".to_string(),
                    b"quot" => "\"".to_string(),
                    b"apos" => "'".to_string(),
                    _ => g.resolve_char_ref().ok().flatten().map(|c| c.to_string()).unwrap_or_default(),
                };
                push_text(&mut open, skipped, &s);
            }
            Ok(Event::Eof) | Err(_) => break,
            Ok(_) => {}
        }
    }
    // Close what malformed XML left open.
    while open.len() > 1 {
        if let Some(e) = open.pop()
            && let Some(parent) = open.last_mut()
        {
            parent.children.push(Node::Elem(e));
        }
    }
    let root = open.pop()?;
    root.children.into_iter().find_map(|n| if let Node::Elem(e) = n { Some(e) } else { None })
}

fn push_text(open: &mut [Elem], skipped: usize, s: &str) {
    if skipped > 0 || s.is_empty() {
        return;
    }
    let Some(parent) = open.last_mut() else { return };
    if let Some(Node::Text(t)) = parent.children.last_mut() {
        t.push_str(s);
    } else {
        parent.children.push(Node::Text(s.to_string()));
    }
}
