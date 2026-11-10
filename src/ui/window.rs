//! The main application window.

use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, HeaderBar};

const WINDOW_TITLE: &str = "greenread";
const DEFAULT_WIDTH: i32 = 900;
const DEFAULT_HEIGHT: i32 = 700;

/// Builds the main window with an empty header bar and shows it.
///
/// Reuses the existing window if the application is activated again,
/// so a second launch focuses the running instance instead of opening a duplicate.
pub fn present_main_window(app: &Application) {
    if let Some(window) = app.active_window() {
        window.present();
        return;
    }

    let window = ApplicationWindow::builder()
        .application(app)
        .title(WINDOW_TITLE)
        .default_width(DEFAULT_WIDTH)
        .default_height(DEFAULT_HEIGHT)
        .build();
    window.set_titlebar(Some(&HeaderBar::new()));
    window.present();
}
