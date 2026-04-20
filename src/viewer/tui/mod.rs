// SPDX-FileCopyrightText: 2020-2021 Robin Krahl <robin.krahl@ireas.org>
// SPDX-License-Identifier: MIT

mod views;

use std::convert;

use anyhow::Context as _;
use cursive::view::{Resizable as _, Scrollable as _};
use cursive::views::{
    Dialog, EditView, LinearLayout, OnEventView, PaddedView, Panel, SelectView, TextView,
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
        }
    }

    fn into_view(self) -> impl cursive::View {
        use cursive::view::scroll::Scroller as _;
        use cursive::With as _;

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
        let text = markup::StyledString::styled(text, theme::Effect::Bold);
        if let Some(link) = link {
            let heading = LinkView::new(text, move |s| {
                if let Err(err) = open_link(s, link.clone().into()) {
                    report_error(s, err);
                }
            });
            self.layout.add_child(indent_view(indent, heading));
        } else {
            let heading = TextView::new(text);
            self.layout.add_child(indent_view(indent, heading));
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
        // Render HTML to plain text using html2text
        let width = self.max_width.saturating_sub(indent_usize);
        let plain = html2text::config::plain()
            .string_from_read(text.html.as_bytes(), width)
            .unwrap_or_else(|_| text.plain.clone());
        let text_view = TextView::new(plain);
        self.layout.add_child(indent_view(indent, text_view));
        Ok(())
    }

    fn println(&mut self) -> Result<(), Self::Error> {
        self.layout.add_child(TextView::new(" "));
        Ok(())
    }
}

fn indent_view<V>(indent: impl Into<usize>, view: V) -> PaddedView<V> {
    PaddedView::lrtb(indent.into(), 0, 0, 0, view)
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

    // vim-like keybindings
    cursive.add_global_callback('j', |s| s.on_event(Key::Down.into()));
    cursive.add_global_callback('k', |s| s.on_event(Key::Up.into()));
    cursive.add_global_callback('h', |s| s.on_event(Key::Left.into()));
    cursive.add_global_callback('l', |s| s.on_event(Key::Right.into()));
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

    let mut theme = theme::Theme {
        shadow: false,
        ..Default::default()
    };
    theme.palette[theme::PaletteColor::Background] = theme::Color::TerminalDefault;
    theme.palette[theme::PaletteColor::View] = theme::Color::TerminalDefault;
    theme.palette[theme::PaletteColor::Primary] = theme::Color::TerminalDefault;
    cursive.set_theme(theme);

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
