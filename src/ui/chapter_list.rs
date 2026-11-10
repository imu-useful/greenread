//! Sidebar listing the chapters of the open book.

use gtk::pango::EllipsizeMode;
use gtk::prelude::*;
use gtk::{Label, ListBox, PolicyType, ScrolledWindow, SelectionMode};

const MIN_WIDTH: i32 = 200;
const ROW_MARGIN: i32 = 6;

pub struct ChapterList {
    root: ScrolledWindow,
    list: ListBox,
}

impl ChapterList {
    pub fn new() -> Self {
        let list = ListBox::builder()
            .selection_mode(SelectionMode::Single)
            .css_classes(["navigation-sidebar"])
            .build();
        let root = ScrolledWindow::builder()
            .hscrollbar_policy(PolicyType::Never)
            .min_content_width(MIN_WIDTH)
            .child(&list)
            .build();
        Self { root, list }
    }

    pub fn widget(&self) -> &ScrolledWindow {
        &self.root
    }

    /// Replaces the listed chapters. Nothing is selected afterwards.
    pub fn set_chapters<'a>(&self, titles: impl IntoIterator<Item = &'a str>) {
        self.list.remove_all();
        for title in titles {
            let label = Label::builder()
                .label(title)
                .xalign(0.0)
                .ellipsize(EllipsizeMode::End)
                .tooltip_text(title)
                .margin_top(ROW_MARGIN)
                .margin_bottom(ROW_MARGIN)
                .build();
            self.list.append(&label);
        }
    }

    pub fn select(&self, index: usize) {
        let row = i32::try_from(index)
            .ok()
            .and_then(|i| self.list.row_at_index(i));
        if let Some(row) = row {
            self.list.select_row(Some(&row));
        }
    }

    /// Calls `on_selected` with the chapter index whenever a row is selected.
    pub fn connect_chapter_selected(&self, on_selected: impl Fn(usize) + 'static) {
        self.list.connect_row_selected(move |_, row| {
            if let Some(index) = row.and_then(|r| usize::try_from(r.index()).ok()) {
                on_selected(index);
            }
        });
    }
}
