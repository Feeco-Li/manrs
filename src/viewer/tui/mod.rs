// SPDX-FileCopyrightText: 2020-2021 Robin Krahl <robin.krahl@ireas.org>
// SPDX-License-Identifier: MIT

mod views;

use std::convert;
use std::sync::{Arc, Mutex};

use anyhow::Context as _;
use cursive::view::{Nameable as _, Resizable as _, Scrollable as _};
use cursive::views::{
    Dialog, DummyView, EditView, LinearLayout, OnEventView, PaddedView, Panel, SelectView,
    TextView,
};
use cursive::{event, theme, utils::markup};

use crate::args;
use crate::doc;
use crate::index;
use crate::source;
use crate::viewer::{self, utils, utils::ManRenderer as _};

use views::{CodeView, LinkView};

#[derive(Clone, Debug)]
pub struct TuiViewer {}

impl TuiViewer {
    pub fn new() -> TuiViewer {
        TuiViewer {}
    }

    fn render<F>(
        &self,
        sources: source::Sources,
        args: args::ViewerArgs,
        doc: &doc::Doc,
        f: F,
    ) -> anyhow::Result<()>
    where
        F: Fn(&mut TuiManRenderer) -> Result<(), convert::Infallible>,
    {
        let mut s = create_cursive(sources, args)?;
        let mut renderer = context(&mut s).create_renderer(doc);
        f(&mut renderer)?;
        let view = renderer.into_view();
        s.add_fullscreen_layer(view);
        s.try_run_with(create_backend)?;
        Ok(())
    }
}

impl viewer::Viewer for TuiViewer {
    fn open(
        &self,
        sources: source::Sources,
        args: args::ViewerArgs,
        doc: &doc::Doc,
    ) -> anyhow::Result<()> {
        self.render(sources, args, doc, |renderer| renderer.render_doc(doc))
    }

    fn open_examples(
        &self,
        sources: source::Sources,
        args: args::ViewerArgs,
        doc: &doc::Doc,
        examples: Vec<doc::Example>,
    ) -> anyhow::Result<()> {
        self.render(sources, args, doc, |renderer| {
            renderer.render_examples(doc, &examples)
        })
    }
}

pub struct Context {
    pub sources: source::Sources,
    pub args: args::ViewerArgs,
    pub highlighter: Option<utils::Highlighter>,
}

impl Context {
    pub fn new(sources: source::Sources, args: args::ViewerArgs) -> anyhow::Result<Context> {
        let highlighter = utils::get_highlighter(&args)?;
        Ok(Context {
            sources,
            args,
            highlighter,
        })
    }

    pub fn create_renderer(&self, doc: &doc::Doc) -> TuiManRenderer<'_> {
        TuiManRenderer::new(
            doc,
            self.args.max_width.unwrap_or(100),
            self.highlighter.as_ref(),
        )
    }
}

pub struct TuiManRenderer<'s> {
    doc_name: doc::Fqn,
    doc_ty: doc::ItemType,
    layout: LinearLayout,
    max_width: usize,
    highlighter: Option<&'s utils::Highlighter>,
    collected_links: Vec<(String, utils::DocLink)>,
    seen_links: std::collections::HashSet<String>,
}

impl<'s> TuiManRenderer<'s> {
    pub fn new(
        doc: &doc::Doc,
        max_width: usize,
        highlighter: Option<&'s utils::Highlighter>,
    ) -> TuiManRenderer<'s> {
        TuiManRenderer {
            doc_name: doc.name.clone(),
            doc_ty: doc.ty,
            layout: LinearLayout::vertical(),
            max_width,
            highlighter,
            collected_links: Vec::new(),
            seen_links: std::collections::HashSet::new(),
        }
    }

    fn into_view(mut self) -> impl cursive::View {
        use cursive::view::scroll::Scroller as _;
        use cursive::With as _;

        if !self.collected_links.is_empty() {
            // Insert blank line separator before the main content
            self.layout.insert_child(0, TextView::new(" "));
            // Insert link views in reverse order so they end up in original order at pos 0
            for (link_text, doc_link) in self.collected_links.into_iter().rev() {
                let display = markup::StyledString::styled(
                    format!("→ {}", link_text),
                    theme::Style::from(theme::ColorStyle::front(theme::Color::Dark(
                        theme::BaseColor::Cyan,
                    )))
                    .combine(theme::Effect::Underline),
                );
                let lv = LinkView::new(display, move |s| {
                    if let Err(err) = open_link(s, doc_link.clone().into()) {
                        report_error(s, err);
                    }
                });
                self.layout.insert_child(0, indent_view(2u8, lv));
            }
            // Insert "LINKS" section heading at the very top
            let heading_style = theme::Style::from(theme::ColorStyle::front(
                theme::Color::Dark(theme::BaseColor::Cyan),
            ))
            .combine(theme::Effect::Bold);
            self.layout.insert_child(
                0,
                TextView::new(markup::StyledString::styled("LINKS", heading_style)),
            );
        }

        let title = format!("{} {}", self.doc_ty.name(), self.doc_name);
        let scroll = self.layout.scrollable();
        let wrapper = scroll
            .wrap_with(OnEventView::new)
            .on_pre_event_inner(event::Key::PageUp, |v, _| {
                let scroller = v.get_scroller_mut();
                if scroller.can_scroll_up() {
                    scroller.scroll_up(scroller.last_outer_size().y.saturating_sub(1));
                }
                Some(event::EventResult::Consumed(None))
            })
            .on_pre_event_inner(event::Key::PageDown, |v, _| {
                let scroller = v.get_scroller_mut();
                if scroller.can_scroll_down() {
                    scroller.scroll_down(scroller.last_outer_size().y.saturating_sub(1));
                }
                Some(event::EventResult::Consumed(None))
            });
        Panel::new(wrapper.full_screen()).title(title)
    }
}

impl<'s> utils::ManRenderer for TuiManRenderer<'s> {
    type Error = convert::Infallible;

    fn print_title(&mut self, _left: &str, _center: &str, _right: &str) -> Result<(), Self::Error> {
        Ok(())
    }

    fn print_heading(
        &mut self,
        indent: u8,
        text: &str,
        link: Option<utils::DocLink>,
    ) -> Result<(), Self::Error> {
        let color = match indent {
            0 => theme::Color::Dark(theme::BaseColor::Cyan),
            3 => theme::Color::Dark(theme::BaseColor::Green),
            _ => theme::Color::TerminalDefault,
        };
        let style = theme::Style::from(theme::ColorStyle::front(color)).combine(theme::Effect::Bold);
        let styled = markup::StyledString::styled(text, style);
        if let Some(link) = link {
            let heading = LinkView::new(styled, move |s| {
                if let Err(err) = open_link(s, link.clone().into()) {
                    report_error(s, err);
                }
            });
            self.layout.add_child(indent_view(indent, heading));
        } else {
            self.layout.add_child(indent_view(indent, TextView::new(styled)));
        }
        Ok(())
    }

    fn print_code(&mut self, indent: u8, code: &doc::Code) -> Result<(), Self::Error> {
        if let Some(highlighter) = self.highlighter {
            let code = CodeView::new(&code.to_string(), highlighter);
            self.layout.add_child(indent_view(indent, code));
        } else {
            let text = TextView::new(code.to_string());
            self.layout.add_child(indent_view(indent, text));
        }
        Ok(())
    }

    fn print_text(&mut self, indent: u8, text: &doc::Text) -> Result<(), Self::Error> {
        let indent_usize = usize::from(indent);
        let width = self.max_width.saturating_sub(indent_usize);
        let decorator = utils::RichDecorator::annotating();
        let lines = html2text::config::with_decorator(decorator)
            .lines_from_read(text.html.as_bytes(), width)
            .unwrap_or_default();
        for elements in utils::highlight_html(&lines, self.highlighter) {
            let mut styled = markup::StyledString::new();
            for elem in elements {
                match elem {
                    utils::HighlightedHtmlElement::RichString(ts) => {
                        let style = rich_annotations_to_style(&ts.tag);
                        styled.append(markup::StyledString::styled(ts.s.as_str(), style));
                    }
                    utils::HighlightedHtmlElement::StyledString(ss) => {
                        let style = text_style_to_cursive(ss.style);
                        styled.append(markup::StyledString::styled(ss.s.to_owned(), style));
                    }
                }
            }
            self.layout.add_child(indent_view(indent, TextView::new(styled)));
        }

        // Collect navigable doc links for the top-of-page links section
        let base = module_path(&self.doc_name, self.doc_ty);
        for (link_text, doc_link) in extract_doc_links(&text.html, &base) {
            let key = doc_link.name.as_ref().to_owned();
            if self.seen_links.insert(key) {
                self.collected_links.push((link_text, doc_link));
            }
        }

        Ok(())
    }

    fn println(&mut self) -> Result<(), Self::Error> {
        self.layout.add_child(TextView::new(" "));
        Ok(())
    }
}

fn rich_annotations_to_style(
    tags: &[html2text::render::text_renderer::RichAnnotation],
) -> theme::Style {
    use html2text::render::text_renderer::RichAnnotation;
    let mut style = theme::Style::default();
    for tag in tags {
        match tag {
            RichAnnotation::Strong => {
                style = style
                    .combine(theme::Effect::Bold)
                    .combine(theme::ColorStyle::front(theme::Color::Light(theme::BaseColor::White)));
            }
            RichAnnotation::Emphasis => {
                style = style
                    .combine(theme::Effect::Italic)
                    .combine(theme::ColorStyle::front(theme::Color::Light(theme::BaseColor::Green)));
            }
            RichAnnotation::Code => {
                style = style.combine(theme::ColorStyle::front(theme::Color::Light(
                    theme::BaseColor::Yellow,
                )));
            }
            RichAnnotation::Link(_) => {
                style = style
                    .combine(theme::Effect::Underline)
                    .combine(theme::ColorStyle::front(theme::Color::Dark(theme::BaseColor::Cyan)));
            }
            _ => {}
        }
    }
    style
}

fn text_style_to_cursive(style: Option<text_style::Style>) -> theme::Style {
    use text_style::{AnsiColor, AnsiMode, Color};
    let Some(style) = style else {
        return theme::Style::default();
    };
    let mut out = theme::Style::default();
    if let Some(fg) = style.fg {
        let color = match fg {
            Color::Ansi { color, mode } => {
                let base = match color {
                    AnsiColor::Black => theme::BaseColor::Black,
                    AnsiColor::Red => theme::BaseColor::Red,
                    AnsiColor::Green => theme::BaseColor::Green,
                    AnsiColor::Yellow => theme::BaseColor::Yellow,
                    AnsiColor::Blue => theme::BaseColor::Blue,
                    AnsiColor::Magenta => theme::BaseColor::Magenta,
                    AnsiColor::Cyan => theme::BaseColor::Cyan,
                    AnsiColor::White => theme::BaseColor::White,
                };
                match mode {
                    AnsiMode::Dark => theme::Color::Dark(base),
                    AnsiMode::Light => theme::Color::Light(base),
                }
            }
            Color::Rgb { r, g, b } => theme::Color::Rgb(r, g, b),
        };
        out = out.combine(theme::ColorStyle::front(color));
    }
    let e = style.effects;
    if e.is_bold { out = out.combine(theme::Effect::Bold); }
    if e.is_italic { out = out.combine(theme::Effect::Italic); }
    if e.is_underline { out = out.combine(theme::Effect::Underline); }
    if e.is_strikethrough { out = out.combine(theme::Effect::Strikethrough); }
    out
}

/// Open an interactive TUI crate picker and return the selected crate as a doc::Name.
pub fn pick_crate(crates: &[String]) -> anyhow::Result<Option<doc::Name>> {
    anyhow::ensure!(
        termion::is_tty(&std::io::stdout()),
        "No keyword provided and stdout is not a TTY — please provide a keyword."
    );

    let chosen: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let crates_for_filter = crates.to_vec();
    let chosen_select = chosen.clone();
    let chosen_edit = chosen.clone();

    let mut select: SelectView<String> = SelectView::new();
    for c in crates {
        select.add_item(c.clone(), c.clone());
    }
    select.set_on_submit(move |s, item: &String| {
        *chosen_select.lock().unwrap() = Some(item.clone());
        s.quit();
    });

    let edit = EditView::new()
        .on_edit(move |s, text, _| {
            let text_lower = text.to_lowercase();
            s.call_on_name("crates", |v: &mut SelectView<String>| {
                v.clear();
                for c in &crates_for_filter {
                    if c.to_lowercase().contains(&text_lower) {
                        v.add_item(c.clone(), c.clone());
                    }
                }
            });
        })
        .on_submit(move |s, _| {
            let first = s
                .call_on_name("crates", |v: &mut SelectView<String>| {
                    v.selection().map(|rc| rc.as_ref().clone())
                })
                .flatten();
            if let Some(item) = first {
                *chosen_edit.lock().unwrap() = Some(item);
                s.quit();
            }
        });

    let inner = LinearLayout::vertical()
        .child(PaddedView::lrtb(
            2, 2, 1, 1,
            LinearLayout::horizontal()
                .child(TextView::new(
                    markup::StyledString::styled("Filter: ", theme::Effect::Bold),
                ))
                .child(edit.full_width()),
        ))
        .child(DummyView)
        .child(PaddedView::lrtb(
            1, 1, 0, 1,
            select.with_name("crates").scrollable().full_screen(),
        ));

    let hint = markup::StyledString::styled(
        "  ↑/↓ navigate  ·  j/k scroll  ·  J/K jump links  ·  Enter open  ·  q quit",
        theme::ColorStyle::front(theme::Color::Dark(theme::BaseColor::Cyan)),
    );

    let mut siv = cursive::Cursive::new();
    apply_theme(&mut siv);
    siv.add_global_callback('q', |s| s.quit());
    siv.add_fullscreen_layer(
        LinearLayout::vertical()
            .child(
                Panel::new(inner)
                    .title("manrs — select documentation")
                    .full_screen(),
            )
            .child(TextView::new(hint)),
    );
    siv.try_run_with(create_backend)?;

    let result = chosen.lock().unwrap().clone();
    Ok(result.map(|s| s.into()))
}

fn apply_theme(siv: &mut cursive::Cursive) {
    use theme::*;
    let mut t = Theme { shadow: false, borders: BorderStyle::Simple, ..Default::default() };
    t.palette[PaletteColor::Background] = Color::TerminalDefault;
    t.palette[PaletteColor::View] = Color::TerminalDefault;
    t.palette[PaletteColor::Primary] = Color::TerminalDefault;
    t.palette[PaletteColor::Secondary] = Color::Dark(BaseColor::Cyan);
    t.palette[PaletteColor::TitlePrimary] = Color::Dark(BaseColor::Cyan);
    t.palette[PaletteColor::TitleSecondary] = Color::Dark(BaseColor::Green);
    // Selection bar: dark blue bg, bright white text
    t.palette[PaletteColor::Highlight] = Color::Dark(BaseColor::Blue);
    t.palette[PaletteColor::HighlightInactive] = Color::Dark(BaseColor::Black);
    t.palette[PaletteColor::HighlightText] = Color::Light(BaseColor::White);
    siv.set_theme(t);
}

fn indent_view<V>(indent: impl Into<usize>, view: V) -> PaddedView<V> {
    PaddedView::lrtb(indent.into(), 0, 0, 0, view)
}

/// Returns the containing-module FQN string used as the base for URL resolution.
fn module_path(doc_name: &doc::Fqn, doc_ty: doc::ItemType) -> String {
    use doc::ItemType::*;
    let s = doc_name.as_ref();
    let parts: Vec<&str> = s.split("::").collect();
    let trimmed = match doc_ty {
        Module => parts.as_slice(),
        Method | StructField | Variant | AssocType | AssocConst => {
            let end = parts.len().saturating_sub(2);
            &parts[..end]
        }
        _ => {
            let end = parts.len().saturating_sub(1);
            &parts[..end]
        }
    };
    if trimmed.is_empty() { s.to_owned() } else { trimmed.join("::") }
}

/// Parse relative rustdoc URL into a DocLink, returning None for external / anchor-only links.
fn parse_doc_url(base: &str, url: &str) -> Option<utils::DocLink> {
    let url = url.split('#').next().unwrap_or(url);
    let url = url.split('?').next().unwrap_or(url);
    if url.is_empty() { return None; }

    let segs: Vec<&str> = url.split('/').filter(|s| !s.is_empty()).collect();
    if segs.is_empty() { return None; }

    let mut path: Vec<String> = base
        .split("::")
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();

    for seg in &segs[..segs.len() - 1] {
        if *seg == ".." {
            path.pop();
        } else {
            path.push(seg.to_string());
        }
    }

    let filename = segs.last()?;
    if *filename == "index.html" {
        if path.is_empty() { return None; }
        Some(utils::DocLink {
            name: path.join("::").into(),
            ty: Some(doc::ItemType::Module),
        })
    } else {
        let stem = filename.strip_suffix(".html")?;
        let dot = stem.find('.')?;
        let ty_str = &stem[..dot];
        let item_name = &stem[dot + 1..];
        if item_name.contains('.') { return None; }
        let ty: doc::ItemType = ty_str.parse().ok()?;
        path.push(item_name.to_string());
        Some(utils::DocLink {
            name: path.join("::").into(),
            ty: Some(ty),
        })
    }
}

/// Extract navigable doc links from an HTML fragment.
fn extract_doc_links(html: &str, base: &str) -> Vec<(String, utils::DocLink)> {
    use scraper::{Html, Selector};
    let fragment = Html::parse_fragment(html);
    let Ok(sel) = Selector::parse("a[href]") else { return Vec::new() };
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for elem in fragment.select(&sel) {
        let Some(href) = elem.value().attr("href") else { continue };
        if href.starts_with("http") || href.starts_with('#') { continue; }
        let Some(link) = parse_doc_url(base, href) else { continue };
        let text: String = elem.text().collect::<Vec<_>>().join(" ");
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if text.is_empty() { continue; }
        if seen.insert(href.to_owned()) {
            result.push((text, link));
        }
    }
    result
}

fn create_backend() -> anyhow::Result<Box<dyn cursive::backend::Backend>> {
    cursive::backends::termion::Backend::init().context("Could not create termion backend")
}

fn create_cursive(
    sources: source::Sources,
    args: args::ViewerArgs,
) -> anyhow::Result<cursive::Cursive> {
    use cursive::event::{Event, Key};

    let mut cursive = cursive::Cursive::new();

    cursive.set_user_data(Context::new(sources, args)?);

    // j/k: scroll line by line
    cursive.add_global_callback('j', |s| s.on_event(Key::Down.into()));
    cursive.add_global_callback('k', |s| s.on_event(Key::Up.into()));
    // J/K: jump to next/previous link (Tab traversal)
    cursive.add_global_callback('J', |s| s.on_event(Event::Key(Key::Tab)));
    cursive.add_global_callback('K', |s| s.on_event(Event::Shift(Key::Tab)));
    cursive.add_global_callback('G', |s| s.on_event(Key::End.into()));
    cursive.add_global_callback('g', |s| s.on_event(Key::Home.into()));
    cursive.add_global_callback(Event::CtrlChar('f'), |s| s.on_event(Key::PageDown.into()));
    cursive.add_global_callback(Event::CtrlChar('b'), |s| s.on_event(Key::PageUp.into()));

    cursive.add_global_callback('q', |s| s.quit());
    cursive.add_global_callback(event::Key::Backspace, |s| {
        let screen = s.screen_mut();
        if screen.len() > 1 {
            screen.pop_layer();
        }
    });
    cursive.add_global_callback('o', open_doc_dialog);

    apply_theme(&mut cursive);

    Ok(cursive)
}

fn context(s: &mut cursive::Cursive) -> &mut Context {
    s.user_data()
        .expect("Missing context in cursive application")
}

fn report_error(s: &mut cursive::Cursive, error: anyhow::Error) {
    let context: Vec<_> = error
        .chain()
        .skip(1)
        .map(|e| format!("    {}", e.to_string()))
        .collect();

    let mut msg = error.to_string();
    if !context.is_empty() {
        msg.push_str("\n\nContext:\n");
        msg.push_str(&context.join("\n"));
    }

    let dialog = Dialog::info(msg).title("Error");
    s.add_layer(dialog);
}

fn with_report_error<F>(s: &mut cursive::Cursive, f: F)
where
    F: Fn(&mut cursive::Cursive) -> anyhow::Result<()>,
{
    if let Err(err) = f(s) {
        report_error(s, err);
    }
}

fn open_doc_dialog(s: &mut cursive::Cursive) {
    let mut edit_view = EditView::new();
    edit_view.set_on_submit(|s, val| {
        with_report_error(s, |s| {
            s.pop_layer();
            let sources = &context(s).sources;
            let name = doc::Name::from(val.to_owned());
            let mut doc = sources.find(&name, None)?;
            if doc.is_none() {
                let items = sources.search(&name)?;
                if items.len() > 1 {
                    select_doc_dialog(s, items);
                    return Ok(());
                } else if !items.is_empty() {
                    doc = sources.find(&items[0].name, Some(items[0].ty))?;
                }
            }
            if let Some(doc) = doc {
                open_doc(s, &doc);
                Ok(())
            } else {
                Err(anyhow::anyhow!("Could not find documentation for {}", name))
            }
        });
    });
    let dialog = Dialog::around(edit_view.min_width(40)).title("Open documentation");
    s.add_layer(dialog);
}

fn select_doc_dialog(s: &mut cursive::Cursive, items: Vec<index::IndexItem>) {
    let mut select_view = SelectView::new();
    select_view.add_all(
        items
            .into_iter()
            .map(|item| (item.name.as_ref().to_owned(), item)),
    );
    select_view.set_on_submit(|s, item| {
        with_report_error(s, |s| {
            let doc = context(s).sources.find(&item.name, Some(item.ty))?;
            if let Some(doc) = doc {
                open_doc(s, &doc);
                Ok(())
            } else {
                Err(anyhow::anyhow!(
                    "Could not find documentation for {}",
                    item.name
                ))
            }
        });
    });
    let dialog = Dialog::around(select_view.scrollable()).title("Select documentation item");
    s.add_layer(dialog);
}

fn open_doc(s: &mut cursive::Cursive, doc: &doc::Doc) {
    let mut renderer = context(s).create_renderer(doc);
    renderer.render_doc(doc).unwrap();
    let view = renderer.into_view();
    s.add_fullscreen_layer(view);
}

fn open_link(s: &mut cursive::Cursive, link: ResolvedLink) -> anyhow::Result<()> {
    let ResolvedLink::Doc(ty, name) = link;
    let doc = context(s)
        .sources
        .find(&name, ty)?
        .with_context(|| format!("Could not find documentation for item: {}", name))?;
    open_doc(s, &doc);
    Ok(())
}

enum ResolvedLink {
    Doc(Option<doc::ItemType>, doc::Fqn),
}

impl From<utils::DocLink> for ResolvedLink {
    fn from(link: utils::DocLink) -> ResolvedLink {
        ResolvedLink::Doc(link.ty, link.name)
    }
}
