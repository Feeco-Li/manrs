// SPDX-FileCopyrightText: 2020 Robin Krahl <robin.krahl@ireas.org>
// SPDX-License-Identifier: MIT

use std::cmp;

use cursive::{event, theme, utils::markup};

use crate::viewer::utils;

pub struct LinkView {
    text: markup::StyledString,
    cb: event::Callback,
    is_focused: bool,
}

impl LinkView {
    pub fn new<F>(text: impl Into<markup::StyledString>, cb: F) -> LinkView
    where
        F: Fn(&mut cursive::Cursive) + 'static + Send + Sync,
    {
        LinkView {
            text: text.into(),
            cb: event::Callback::from_fn(cb),
            is_focused: false,
        }
    }
}

impl cursive::View for LinkView {
    fn draw(&self, printer: &cursive::Printer) {
        let mut style = theme::Style::from(theme::Effect::Underline);
        if self.is_focused && printer.focused {
            style = style.combine(theme::PaletteColor::Highlight);
        };
        printer.with_style(style, |printer| {
            printer.print_styled((0, 0), &self.text)
        });
    }

    fn required_size(&mut self, _constraint: cursive::XY<usize>) -> cursive::XY<usize> {
        (self.text.width(), 1).into()
    }

    fn take_focus(
        &mut self,
        _direction: cursive::direction::Direction,
    ) -> Result<event::EventResult, cursive::view::CannotFocus> {
        self.is_focused = true;
        Ok(event::EventResult::Consumed(None))
    }

    fn on_event(&mut self, event: event::Event) -> event::EventResult {
        if event == event::Event::Key(event::Key::Enter) {
            event::EventResult::Consumed(Some(self.cb.clone()))
        } else {
            event::EventResult::Ignored
        }
    }
}

/// Convert a syntect RGB Color to a cursive Color.
fn syntect_color_to_cursive(c: syntect::highlighting::Color) -> theme::Color {
    theme::Color::Rgb(c.r, c.g, c.b)
}

/// Convert a syntect highlighting Style to a cursive theme Style.
fn syntect_style_to_cursive(style: &syntect::highlighting::Style) -> theme::Style {
    let fg = syntect_color_to_cursive(style.foreground);
    let effects = style.font_style;
    let mut cursive_style = theme::Style::from(theme::ColorStyle::front(fg));
    if effects.contains(syntect::highlighting::FontStyle::BOLD) {
        cursive_style = cursive_style.combine(theme::Effect::Bold);
    }
    if effects.contains(syntect::highlighting::FontStyle::ITALIC) {
        cursive_style = cursive_style.combine(theme::Effect::Italic);
    }
    if effects.contains(syntect::highlighting::FontStyle::UNDERLINE) {
        cursive_style = cursive_style.combine(theme::Effect::Underline);
    }
    cursive_style
}

pub struct CodeView {
    lines: Vec<markup::StyledString>,
    width: usize,
}

impl CodeView {
    pub fn new(code: &str, highlighter: &utils::Highlighter) -> CodeView {
        let mut lines = Vec::new();
        let mut width = 0;
        for line in highlighter.highlight(code) {
            let mut s = markup::StyledString::new();
            for (style, text) in &line {
                let cursive_style = syntect_style_to_cursive(style);
                s.append(markup::StyledString::styled(*text, cursive_style));
            }
            width = cmp::max(width, s.width());
            lines.push(s);
        }
        CodeView { lines, width }
    }
}

impl cursive::View for CodeView {
    fn draw(&self, printer: &cursive::Printer) {
        for (y, line) in self.lines.iter().enumerate() {
            printer.print_styled((0, y), line);
        }
    }

    fn required_size(&mut self, _constraint: cursive::XY<usize>) -> cursive::XY<usize> {
        (self.width, self.lines.len()).into()
    }
}
