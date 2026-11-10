//! The main application window: header bar, chapter sidebar and reader view.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use greenread::parsers::Book;
use gtk::prelude::*;
use gtk::{
    gio, glib, Application, ApplicationWindow, Button, FileDialog, FileFilter, HeaderBar, Label,
    Orientation, Paned, Spinner, Stack,
};

use super::chapter_list::ChapterList;
use super::loader;
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
pub fn present_main_window(app: &Application) {
    if let Some(window) = app.active_window() {
        window.present();
        return;
    }
    let reader = ReaderWindow::new(app);
    reader.window.present();
}

struct ReaderWindow {
    window: ApplicationWindow,
    stack: Stack,
    message: Label,
    chapters: ChapterList,
    reader: ReaderView,
    book: RefCell<Option<Book>>,
    /// Incremented on every load so a slow, superseded load cannot overwrite a newer one.
    load_generation: Cell<u64>,
}

impl ReaderWindow {
    fn new(app: &Application) -> Rc<Self> {
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
            book: RefCell::new(None),
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

        let weak: Weak<Self> = Rc::downgrade(self);
        self.chapters.connect_chapter_selected(move |index| {
            if let Some(this) = weak.upgrade() {
                this.show_chapter(index);
            }
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
        self.stack.set_visible_child_name(PAGE_LOADING);

        let result = loader::load_book(path).await;
        if self.load_generation.get() != generation {
            return;
        }
        match result {
            Ok(book) => self.show_book(book),
            Err(err) => self.show_message(&format!("Could not open the book: {err}")),
        }
    }

    fn show_book(&self, book: Book) {
        self.window.set_title(Some(&book.title));
        self.chapters
            .set_chapters(book.chapters.iter().map(|c| c.title.as_str()));
        self.book.replace(Some(book));
        self.stack.set_visible_child_name(PAGE_BOOK);
        self.chapters.select(0);
    }

    fn show_chapter(&self, index: usize) {
        let book = self.book.borrow();
        if let Some(chapter) = book.as_ref().and_then(|b| b.chapters.get(index)) {
            self.reader.show_text(&chapter.text);
        }
    }

    fn show_message(&self, text: &str) {
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
