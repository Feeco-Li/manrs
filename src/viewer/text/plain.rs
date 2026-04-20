// SPDX-FileCopyrightText: 2020 Robin Krahl <robin.krahl@ireas.org>
// SPDX-License-Identifier: MIT

use std::io::{self, Write};

use html2text::render::text_renderer;

use crate::args;
use crate::doc;
use crate::viewer::utils;

#[derive(Clone, Debug)]
pub struct PlainTextRenderer {
    line_length: usize,
}

#[derive(Clone, Debug, Default)]
struct Decorator {
    ignore_next_link: bool,
}

impl PlainTextRenderer {
    pub fn new(args: &args::ViewerArgs) -> Self {
        Self {
            line_length: utils::get_line_length(args),
        }
    }
}

impl utils::ManRenderer for PlainTextRenderer {
    type Error = io::Error;

    fn print_title(&mut self, left: &str, middle: &str, right: &str) -> io::Result<()> {
        let title = super::format_title(self.line_length, left, middle, right);
        writeln!(io::stdout(), "{}", title)?;
        writeln!(io::stdout())
    }

    fn print_text(&mut self, indent: u8, s: &doc::Text) -> io::Result<()> {
        let width = self.line_length.saturating_sub(usize::from(indent));
        let lines = html2text::config::with_decorator(Decorator::new())
            .lines_from_read(s.html.as_bytes(), width)
            .unwrap_or_default();
        let text: String = lines
            .into_iter()
            .map(|l| l.into_string())
            .collect::<Vec<_>>()
            .join("\n");
        for line in text.trim().split('\n') {
            writeln!(io::stdout(), "{}{}", " ".repeat(indent.into()), line)?;
        }
        Ok(())
    }

    fn print_code(&mut self, indent: u8, code: &doc::Code) -> io::Result<()> {
        for line in code.split('\n') {
            writeln!(io::stdout(), "{}{}", " ".repeat(indent.into()), line)?;
        }
        Ok(())
    }

    fn print_heading(
        &mut self,
        indent: u8,
        s: &str,
        _link: Option<utils::DocLink>,
    ) -> io::Result<()> {
        writeln!(io::stdout(), "{}{}", " ".repeat(indent.into()), s)
    }

    fn println(&mut self) -> io::Result<()> {
        writeln!(io::stdout())
    }
}

impl Decorator {
    pub fn new() -> Self {
        Decorator::default()
    }
}

impl text_renderer::TextDecorator for Decorator {
    type Annotation = ();

    fn decorate_link_start(&mut self, url: &str) -> (String, Self::Annotation) {
        if super::list_link(url) {
            self.ignore_next_link = false;
            ("[".to_owned(), ())
        } else {
            self.ignore_next_link = true;
            (String::new(), ())
        }
    }

    fn decorate_link_end(&mut self) -> String {
        if self.ignore_next_link {
            String::new()
        } else {
            "]".to_owned()
        }
    }

    fn decorate_em_start(&self) -> (String, Self::Annotation) {
        ("*".to_owned(), ())
    }

    fn decorate_em_end(&self) -> String {
        "*".to_owned()
    }

    fn decorate_strong_start(&self) -> (String, Self::Annotation) {
        ("**".to_owned(), ())
    }

    fn decorate_strong_end(&self) -> String {
        "**".to_owned()
    }

    fn decorate_strikeout_start(&self) -> (String, Self::Annotation) {
        ("~".to_owned(), ())
    }

    fn decorate_strikeout_end(&self) -> String {
        "~".to_owned()
    }

    fn decorate_code_start(&self) -> (String, Self::Annotation) {
        ("`".to_owned(), ())
    }

    fn decorate_code_end(&self) -> String {
        "`".to_owned()
    }

    fn decorate_preformat_first(&self) -> Self::Annotation {}
    fn decorate_preformat_cont(&self) -> Self::Annotation {}

    fn decorate_image(&mut self, _src: &str, title: &str) -> (String, Self::Annotation) {
        (format!("[{}]", title), ())
    }

    fn header_prefix(&self, level: usize) -> String {
        "#".repeat(level) + " "
    }

    fn quote_prefix(&self) -> String {
        "> ".to_string()
    }

    fn unordered_item_prefix(&self) -> String {
        "* ".to_string()
    }

    fn ordered_item_prefix(&self, i: i64) -> String {
        format!("{}. ", i)
    }

    fn finalise(&mut self, _links: Vec<String>) -> Vec<text_renderer::TaggedLine<()>> {
        Vec::new()
    }

    fn make_subblock_decorator(&self) -> Self {
        Decorator::new()
    }
}
