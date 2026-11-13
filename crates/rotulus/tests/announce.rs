//! Where there is no accessibility bus, GTK gives every widget its test
//! context, which cannot announce. The view announces a change of zoom and
//! where keyboard focus lands, and must still do both there.
//!
//! A binary of its own, because GTK picks the backend once per process, and
//! one `#[test]`, because GTK belongs to the first thread that initializes
//! it.

use gtk4 as gtk;
use gtk4::prelude::*;
use rotulus::RotulusView;
use rotulus_layout::{Message, ParsedText, Speaker};

#[test]
fn a_view_announces_on_gtks_test_context() {
    std::env::set_var("GTK_A11Y", "test");
    assert!(
        gtk::init().is_ok(),
        "GTK could not be initialized; run under a display, e.g. xvfb-run -a, as CI does"
    );

    let view = RotulusView::new();
    let id = view.append(Message::live(
        Speaker::new(1, "al"),
        ParsedText::plain("hi"),
    ));
    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_child(Some(&view));
    let window = gtk::Window::new();
    window.set_default_size(400, 300);
    window.set_child(Some(&scrolled));
    window.present();
    let ctx = gtk::glib::MainContext::default();
    for _ in 0..500 {
        while ctx.iteration(false) {}
        if view.is_mapped() && view.height() > 0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(view.is_mapped(), "the window never mapped");
    assert_eq!(
        view.at_context().type_().name(),
        "GtkTestATContext",
        "the view should be on GTK's test context"
    );

    // GTK realizes a widget's context when it takes focus in an active
    // window, and announces nothing on one it has not realized.
    assert!(view.has_focus(), "the view should have focus");

    view.zoom_step(1);
    assert!(view.reveal(id, true));
    assert_eq!(view.focus_row(), Some(id), "keyboard focus is on the row");
}
