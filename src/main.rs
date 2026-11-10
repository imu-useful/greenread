//! greenread — application entry point and GTK application setup.

mod ui;

use gtk::prelude::*;
use gtk::{glib, Application};

const APP_ID: &str = "io.github.greenread";

fn main() -> glib::ExitCode {
    let progress = ui::progress::ProgressSync::open_default();
    let app = Application::builder().application_id(APP_ID).build();
    let window_progress = progress.clone();
    app.connect_activate(move |app| ui::window::present_main_window(app, &window_progress));
    let exit_code = app.run();
    // The main loop has ended, so blocking here cannot freeze the UI.
    progress.shutdown();
    exit_code
}
