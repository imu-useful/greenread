//! Scrollable, read-only view of one chapter's text.

use gtk::prelude::*;
use gtk::{ScrolledWindow, TextView, WrapMode};

const SIDE_MARGIN: i32 = 32;
const VERTICAL_MARGIN: i32 = 24;
const PARAGRAPH_SPACING: i32 = 4;
const RESTORE_MARK: &str = "greenread-restore";

pub struct ReaderView {
    root: ScrolledWindow,
    text_view: TextView,
}

impl ReaderView {
    pub fn new() -> Self {
        let text_view = TextView::builder()
            .editable(false)
            .cursor_visible(false)
            .wrap_mode(WrapMode::WordChar)
            .left_margin(SIDE_MARGIN)
            .right_margin(SIDE_MARGIN)
            .top_margin(VERTICAL_MARGIN)
            .bottom_margin(VERTICAL_MARGIN)
            .pixels_below_lines(PARAGRAPH_SPACING)
            .build();
        let root = ScrolledWindow::builder()
            .hexpand(true)
            .vexpand(true)
            .child(&text_view)
            .build();
        Self { root, text_view }
    }

    pub fn widget(&self) -> &ScrolledWindow {
        &self.root
    }

    /// Shows `text` and scrolls back to the top.
    pub fn show_text(&self, text: &str) {
        self.text_view.buffer().set_text(text);
        self.root.vadjustment().set_value(0.0);
    }

    /// Character offset of the first visible line in the current text.
    pub fn top_char_offset(&self) -> usize {
        let visible = self.text_view.visible_rect();
        self.text_view
            .iter_at_location(visible.x(), visible.y())
            .and_then(|iter| usize::try_from(iter.offset()).ok())
            .unwrap_or(0)
    }

    /// Scrolls so the line containing `char_offset` is at the top. Works before
    /// the text has been laid out: GTK performs the scroll once layout is ready.
    pub fn scroll_to_char_offset(&self, char_offset: usize) {
        let buffer = self.text_view.buffer();
        let iter = buffer.iter_at_offset(i32::try_from(char_offset).unwrap_or(i32::MAX));
        let mark = match buffer.mark(RESTORE_MARK) {
            Some(mark) => {
                buffer.move_mark(&mark, &iter);
                mark
            }
            None => buffer.create_mark(Some(RESTORE_MARK), &iter, true),
        };
        self.text_view.scroll_to_mark(&mark, 0.0, true, 0.0, 0.0);
    }

    /// Calls `on_scrolled` whenever the vertical scroll position changes.
    pub fn connect_scrolled(&self, on_scrolled: impl Fn() + 'static) {
        self.root
            .vadjustment()
            .connect_value_changed(move |_| on_scrolled());
    }
}
