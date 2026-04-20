// SPDX-FileCopyrightText: 2020 Robin Krahl <robin.krahl@ireas.org>
// SPDX-License-Identifier: MIT

//! Helper methods for HTML parsing.

use ego_tree::NodeRef;
use scraper::{CaseSensitivity, ElementRef, Node};

pub trait NodeRefExt<'a> {
    fn get_attribute(&self, name: &str) -> Option<String>;
    fn is_element_name(&self, name: &str) -> bool;
    fn has_class(&self, class: &str) -> bool;
    fn next_sibling_element(self) -> Option<NodeRef<'a, Node>>;
    fn previous_sibling_element(self) -> Option<NodeRef<'a, Node>>;
    fn text_contents(&self) -> String;
}

impl<'a> NodeRefExt<'a> for ElementRef<'a> {
    fn get_attribute(&self, name: &str) -> Option<String> {
        self.value().attr(name).map(ToOwned::to_owned)
    }

    fn is_element_name(&self, name: &str) -> bool {
        self.value().name() == name
    }

    fn has_class(&self, class: &str) -> bool {
        self.value()
            .has_class(class, CaseSensitivity::CaseSensitive)
    }

    fn next_sibling_element(self) -> Option<NodeRef<'a, Node>> {
        let node: NodeRef<'a, Node> = *self;
        node.next_sibling_element()
    }

    fn previous_sibling_element(self) -> Option<NodeRef<'a, Node>> {
        let node: NodeRef<'a, Node> = *self;
        node.previous_sibling_element()
    }

    fn text_contents(&self) -> String {
        self.text().collect()
    }
}

impl<'a> NodeRefExt<'a> for NodeRef<'a, Node> {
    fn get_attribute(&self, name: &str) -> Option<String> {
        self.value().as_element()?.attr(name).map(ToOwned::to_owned)
    }

    fn is_element_name(&self, name: &str) -> bool {
        self.value()
            .as_element()
            .map(|e| e.name() == name)
            .unwrap_or(false)
    }

    fn has_class(&self, class: &str) -> bool {
        self.value()
            .as_element()
            .map(|e| e.has_class(class, CaseSensitivity::CaseSensitive))
            .unwrap_or(false)
    }

    fn next_sibling_element(self) -> Option<NodeRef<'a, Node>> {
        let mut next = self.next_sibling();
        while let Some(node) = next {
            if node.value().is_element() {
                return Some(node);
            }
            next = node.next_sibling();
        }
        None
    }

    fn previous_sibling_element(self) -> Option<NodeRef<'a, Node>> {
        let mut prev = self.prev_sibling();
        while let Some(node) = prev {
            if node.value().is_element() {
                return Some(node);
            }
            prev = node.prev_sibling();
        }
        None
    }

    fn text_contents(&self) -> String {
        collect_text(self)
    }
}

fn collect_text(node: &NodeRef<'_, Node>) -> String {
    let mut s = String::new();
    for child in node.children() {
        if let Some(text) = child.value().as_text() {
            s.push_str(text);
        } else {
            s.push_str(&collect_text(&child));
        }
    }
    s
}
