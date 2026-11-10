//! greenread — application entry point and GTK application setup.

mod ui;

use gtk::prelude::*;
use gtk::{glib, Application};

const APP_ID: &str = "io.github.greenread";

fn main() -> glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(ui::window::present_main_window);
    app.run()
}
