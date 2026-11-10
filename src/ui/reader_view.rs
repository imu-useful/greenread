//! Scrollable, read-only view of one chapter's text.

use gtk::prelude::*;
use gtk::{ScrolledWindow, TextView, WrapMode};

const SIDE_MARGIN: i32 = 32;
const VERTICAL_MARGIN: i32 = 24;
const PARAGRAPH_SPACING: i32 = 4;

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
}
