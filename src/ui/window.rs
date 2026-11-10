//! The main application window: header bar, chapter sidebar and reader view.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use greenread::storage::ReadingPosition;
use gtk::prelude::*;
use gtk::{
    gio, glib, Application, ApplicationWindow, Button, FileDialog, FileFilter, HeaderBar, Label,
    Orientation, Paned, Spinner, Stack,
};

use super::chapter_list::ChapterList;
use super::loader::{self, LoadedBook};
use super::progress::ProgressSync;
use super::reader_view::ReaderView;

const APP_TITLE: &str = "greenread";
const DEFAULT_WIDTH: i32 = 900;
const DEFAULT_HEIGHT: i32 = 700;
const SIDEBAR_WIDTH: i32 = 240;

const PAGE_MESSAGE: &str = "message";
const PAGE_LOADING: &str = "loading";
const PAGE_BOOK: &str = "book";

const WELCOME_TEXT: &str = "Open a book to start reading.";
const NOT_LOCAL_TEXT: &str = "Only local files can be opened.";

/// Builds the main window and shows it.
///
/// Reuses the existing window if the application is activated again,
/// so a second launch focuses the running instance instead of opening a duplicate.
pub fn present_main_window(app: &Application, progress: &Rc<ProgressSync>) {
    if let Some(window) = app.active_window() {
        window.present();
        return;
    }
    let reader = ReaderWindow::new(app, Rc::clone(progress));
    reader.window.present();
}

struct ReaderWindow {
    window: ApplicationWindow,
    stack: Stack,
    message: Label,
    chapters: ChapterList,
    reader: ReaderView,
    progress: Rc<ProgressSync>,
    open_book: RefCell<Option<LoadedBook>>,
    current_chapter: Cell<usize>,
    /// Incremented on every load so a slow, superseded load cannot overwrite a newer one.
    load_generation: Cell<u64>,
}

impl ReaderWindow {
    fn new(app: &Application, progress: Rc<ProgressSync>) -> Rc<Self> {
        let window = ApplicationWindow::builder()
            .application(app)
            .title(APP_TITLE)
            .default_width(DEFAULT_WIDTH)
            .default_height(DEFAULT_HEIGHT)
            .build();

        let open_button = Button::builder()
            .icon_name("document-open-symbolic")
            .tooltip_text("Open a book")
            .build();
        let header = HeaderBar::new();
        header.pack_start(&open_button);
        window.set_titlebar(Some(&header));

        let message = Label::builder()
            .label(WELCOME_TEXT)
            .wrap(true)
            .justify(gtk::Justification::Center)
            .css_classes(["dim-label"])
            .build();
        let chapters = ChapterList::new();
        let reader = ReaderView::new();
        let paned = Paned::builder()
            .orientation(Orientation::Horizontal)
            .start_child(chapters.widget())
            .end_child(reader.widget())
            .resize_start_child(false)
            .shrink_start_child(false)
            .position(SIDEBAR_WIDTH)
            .build();

        let stack = Stack::new();
        stack.add_named(&message, Some(PAGE_MESSAGE));
        stack.add_named(&loading_page(), Some(PAGE_LOADING));
        stack.add_named(&paned, Some(PAGE_BOOK));
        window.set_child(Some(&stack));

        let this = Rc::new(Self {
            window,
            stack,
            message,
            chapters,
            reader,
            progress,
            open_book: RefCell::new(None),
            current_chapter: Cell::new(0),
            load_generation: Cell::new(0),
        });
        this.connect_signals(&open_button);
        this
    }

    // Closures hold `Weak` references: the window owns the widgets that own
    // the closures, so a strong `Rc` would form a cycle and leak.
    fn connect_signals(self: &Rc<Self>, open_button: &Button) {
        let weak = Rc::downgrade(self);
        open_button.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                glib::spawn_future_local(this.choose_and_load());
            }
        });

        let weak = Rc::downgrade(self);
        self.chapters.connect_chapter_selected(move |index| {
            if let Some(this) = weak.upgrade() {
                this.show_chapter(index);
            }
        });

        let weak = Rc::downgrade(self);
        self.reader.connect_scrolled(move || {
            if let Some(this) = weak.upgrade() {
                this.record_position();
            }
        });

        let weak = Rc::downgrade(self);
        self.window.connect_close_request(move |_| {
            if let Some(this) = weak.upgrade() {
                this.save_position_now();
            }
            glib::Propagation::Proceed
        });
    }

    async fn choose_and_load(self: Rc<Self>) {
        match book_dialog().open_future(Some(&self.window)).await {
            Ok(file) => match file.path() {
                Some(path) => self.load(path).await,
                None => self.show_message(NOT_LOCAL_TEXT),
            },
            Err(err) if is_dialog_cancel(&err) => {}
            Err(err) => self.show_message(&format!("Could not open the file chooser: {err}")),
        }
    }

    async fn load(&self, path: PathBuf) {
        let generation = self.load_generation.get() + 1;
        self.load_generation.set(generation);
        self.save_position_now();
        self.stack.set_visible_child_name(PAGE_LOADING);

        let loaded = match loader::load_book(path).await {
            Ok(loaded) => loaded,
            Err(err) => {
                if self.load_generation.get() == generation {
                    self.show_message(&format!("Could not open the book: {err}"));
                }
                return;
            }
        };
        let saved = self.progress.load(loaded.path.clone()).await;
        if self.load_generation.get() == generation {
            self.show_book(loaded, saved.unwrap_or_default());
        }
    }

    fn show_book(&self, loaded: LoadedBook, saved: ReadingPosition) {
        self.window.set_title(Some(&loaded.book.title));
        self.chapters
            .set_chapters(loaded.book.chapters.iter().map(|c| c.title.as_str()));
        // A saved chapter beyond the end means the file changed; start over.
        let start = if saved.chapter < loaded.book.chapters.len() {
            saved
        } else {
            ReadingPosition::default()
        };
        self.open_book.replace(Some(loaded));
        self.stack.set_visible_child_name(PAGE_BOOK);
        self.chapters.select(start.chapter);
        self.reader.scroll_to_char_offset(start.char_offset);
    }

    fn show_chapter(&self, index: usize) {
        let open_book = self.open_book.borrow();
        let Some(chapter) = open_book.as_ref().and_then(|b| b.book.chapters.get(index)) else {
            return;
        };
        self.current_chapter.set(index);
        self.reader.show_text(&chapter.text);
        drop(open_book);
        self.record_position();
    }

    fn current_position(&self) -> Option<(PathBuf, ReadingPosition)> {
        let open_book = self.open_book.borrow();
        let path = open_book.as_ref()?.path.clone();
        let position = ReadingPosition {
            chapter: self.current_chapter.get(),
            char_offset: self.reader.top_char_offset(),
        };
        Some((path, position))
    }

    fn record_position(&self) {
        if let Some((path, position)) = self.current_position() {
            self.progress.record(path, position);
        }
    }

    fn save_position_now(&self) {
        if let Some((path, position)) = self.current_position() {
            self.progress.save_now(path, position);
        }
    }

    fn show_message(&self, text: &str) {
        self.open_book.replace(None);
        self.message.set_label(text);
        self.stack.set_visible_child_name(PAGE_MESSAGE);
    }
}

fn loading_page() -> Spinner {
    Spinner::builder()
        .spinning(true)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .width_request(32)
        .height_request(32)
        .build()
}

fn book_dialog() -> FileDialog {
    let text = FileFilter::new();
    text.set_name(Some("Plain text"));
    text.add_mime_type("text/plain");
    text.add_suffix("txt");
    let filters = gio::ListStore::new::<FileFilter>();
    filters.append(&text);
    FileDialog::builder()
        .title("Open Book")
        .modal(true)
        .filters(&filters)
        .default_filter(&text)
        .build()
}

/// The user closing the dialog is not an error worth reporting.
fn is_dialog_cancel(err: &glib::Error) -> bool {
    err.matches(gtk::DialogError::Dismissed) || err.matches(gtk::DialogError::Cancelled)
}
