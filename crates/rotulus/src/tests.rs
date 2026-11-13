//! Tests for the GTK skin, in two tiers.
//!
//! **Always-on:** the Pango measurer. `pangocairo`'s default font map
//! works without a `GdkDisplay`, so the one part of the widget carrying real
//! logic rather than plumbing is testable anywhere. Geometry correctness
//! proper is covered in `rotulus-layout` against a deterministic
//! measurer; what these check is that the *real* measurer upholds the
//! invariants that engine assumes.
//!
//! **Display-gated:** one smoke test covering class registration and
//! widget construction, at the bottom of this file. Those need a real
//! GTK — gtk4-rs asserts an initialised GTK inside the generated
//! `class_init` itself — which is precisely why every bring-up crash
//! was invisible to `cargo test`. It no-ops without a display and runs
//! on a desktop session.

use crate::measure::PangoMeasure;
use rotulus_layout::{Attrs, Style, TextMeasure};

fn m() -> PangoMeasure {
    PangoMeasure::headless("Monospace 10")
}

/// The palette crosses the C/Rust seam as a bare `GdkRGBA *`: C hands
/// over its `colors[]` array and `ffi.rs` reads `PALETTE_COLS` entries
/// from it. Nothing at the boundary checks the length, so a slot added on
/// one side only would read past the end of the C array in silence. Read
/// the C definitions out of `rotulus.h` and hold the Rust constants to
/// them.
#[test]
fn palette_constants_match_rotulus_h() {
    let header = include_str!("../include/rotulus.h");
    let define = |name: &str| -> usize {
        let prefix = format!("#define {name} ");
        let line = header
            .lines()
            .find(|l| l.starts_with(&prefix))
            .unwrap_or_else(|| panic!("rotulus.h no longer defines {name}"));
        line[prefix.len()..]
            .split_whitespace()
            .next()
            .and_then(|v| v.parse().ok())
            .unwrap_or_else(|| panic!("{name} is not a plain number: {line:?}"))
    };
    for (name, rust) in [
        ("ROTULUS_PAL_COLS", crate::view::PALETTE_COLS),
        ("ROTULUS_PAL_FG", crate::view::PAL_FG),
        ("ROTULUS_PAL_BG", crate::view::PAL_BG),
        ("ROTULUS_PAL_MARK_FG", crate::view::PAL_MARK_FG),
        ("ROTULUS_PAL_MARK_BG", crate::view::PAL_MARK_BG),
        ("ROTULUS_PAL_MUTED", crate::view::PAL_HISTORY_MUTED),
        ("ROTULUS_PAL_MARKER", crate::view::PAL_MARKER),
        ("ROTULUS_PAL_NICK", crate::view::PAL_NICK),
        ("ROTULUS_PAL_NICK_COLOR0", crate::view::PAL_NICK_COLOR0),
        ("ROTULUS_PAL_TIMESTAMP", crate::view::PAL_TIMESTAMP),
        ("ROTULUS_PAL_RULE", crate::view::PAL_RULE),
    ] {
        assert_eq!(define(name), rust, "{name} differs between C and Rust");
    }
    // The per-nick block is the palette's tail; if it isn't, a slot was
    // added after it and the count above has to account for it.
    assert_eq!(
        define("ROTULUS_PAL_NICK_COLOR0") + define("ROTULUS_PAL_NICK_COLORS"),
        crate::view::PALETTE_COLS,
        "the per-nick colors should end the palette"
    );
}

/// The run attribute bits and row constants cross the seam as bare
/// numbers too. Hold the Rust side to the header's.
#[test]
fn abi_constants_match_rotulus_h() {
    let header = include_str!("../include/rotulus.h");
    let shift = |name: &str| -> u32 {
        let prefix = format!("#define {name} ");
        let line = header
            .lines()
            .find(|l| l.starts_with(&prefix))
            .unwrap_or_else(|| panic!("rotulus.h no longer defines {name}"));
        let v = line[prefix.len()..].trim();
        let n = v
            .trim_start_matches("(1u << ")
            .trim_end_matches(')')
            .parse::<u32>()
            .unwrap_or_else(|_| panic!("{name} is not (1u << n): {v:?}"));
        1 << n
    };
    for (name, rust) in [
        ("ROTULUS_ATTR_BOLD", crate::ffi::ATTR_BOLD),
        ("ROTULUS_ATTR_ITALIC", crate::ffi::ATTR_ITALIC),
        ("ROTULUS_ATTR_UNDERLINE", crate::ffi::ATTR_UNDERLINE),
        ("ROTULUS_ATTR_STRIKETHROUGH", crate::ffi::ATTR_STRIKETHROUGH),
        ("ROTULUS_ATTR_MONOSPACE", crate::ffi::ATTR_MONOSPACE),
        ("ROTULUS_ATTR_REVERSE", crate::ffi::ATTR_REVERSE),
        ("ROTULUS_ATTR_BACKGROUND", crate::ffi::ATTR_BACKGROUND),
        ("ROTULUS_ATTR_RGB", crate::ffi::ATTR_RGB),
        (
            "ROTULUS_ATTR_BACKGROUND_RGB",
            crate::ffi::ATTR_BACKGROUND_RGB,
        ),
    ] {
        assert_eq!(
            shift(name),
            u32::from(rust),
            "{name} differs between C and Rust"
        );
    }
    assert_eq!(shift("ROTULUS_ROW_OUTGOING"), crate::ffi::ROW_OUTGOING);
    assert_eq!(shift("ROTULUS_ROW_ACTION"), crate::ffi::ROW_ACTION);

    // The row kinds are an enum, so their values are their order.
    let kinds: Vec<&str> = header
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("ROTULUS_ROW_") && l.ends_with(','))
        .map(|l| l.trim_end_matches(','))
        .collect();
    assert_eq!(
        kinds,
        [
            "ROTULUS_ROW_MESSAGE",
            "ROTULUS_ROW_SYSTEM",
            "ROTULUS_ROW_HISTORY",
            "ROTULUS_ROW_DIVIDER",
            "ROTULUS_ROW_LOAD_OLDER",
            "ROTULUS_ROW_LOAD_NEWER",
        ]
    );
    assert_eq!(
        [
            crate::ffi::ROW_MESSAGE,
            crate::ffi::ROW_SYSTEM,
            crate::ffi::ROW_HISTORY,
            crate::ffi::ROW_DIVIDER,
            crate::ffi::ROW_LOAD_OLDER,
            crate::ffi::ROW_LOAD_NEWER,
        ],
        [0, 1, 2, 3, 4, 5]
    );
}

/// The structs C builds on the stack. A field added on one side only
/// would shift every field after it, silently.
#[test]
fn abi_struct_layouts_are_what_c_computes() {
    use std::mem::{align_of, size_of};
    let ptr = size_of::<*const u8>();
    // text, len, color, attrs, background (+2 pad), rgb, background_rgb.
    let run = ptr + 4 + 2 + 2 + 4 + 4 + 4;
    assert_eq!(
        size_of::<crate::ffi::RotulusRun>(),
        run.next_multiple_of(align_of::<*const u8>())
    );
    assert_eq!(
        size_of::<crate::ffi::RotulusSpeaker>(),
        (8 + ptr + 4).next_multiple_of(8)
    );
}

#[test]
fn metrics_are_sane() {
    let m = m();
    let met = m.metrics();
    assert!(met.line_height > 0, "line height must be positive");
    assert!(met.space_width > 0, "space width must be positive");
    assert!(
        met.ascent <= met.line_height,
        "ascent {} exceeds line height {}",
        met.ascent,
        met.line_height
    );
}

#[test]
fn empty_run_is_zero_width() {
    assert_eq!(m().run_width("", Style::default()), 0);
}

#[test]
fn width_is_monotonic_in_length() {
    // The wrap algorithm's binary-search fallback and the break-point
    // walk both assume longer means wider. A measurer that violated this
    // would produce silently wrong wrap points rather than an error.
    let m = m();
    let s = "the quick brown fox jumps over the lazy dog";
    let mut prev = 0;
    for i in (1..s.len()).step_by(3) {
        let w = m.run_width(&s[..i], Style::default());
        assert!(w >= prev, "width shrank between {} and {i}", i - 3);
        prev = w;
    }
}

#[test]
fn bold_is_never_narrower_than_regular() {
    // Not strictly guaranteed by every font, but any font where bold is
    // *narrower* would make the style-aware break-point search
    // pessimistic rather than wrong. Assert the assumption so a font
    // stack change that breaks it is visible.
    let m = m();
    let s = "measurement";
    let plain = m.run_width(s, Style::default());
    let bold = m.run_width(s, Style::default().with_attrs(Attrs::BOLD));
    assert!(bold >= plain, "bold {bold} < regular {plain}");
}

#[test]
fn fit_prefix_honours_its_contract() {
    // The property the whole wrap algorithm rests on — stated as
    // TextMeasure::fit_prefix documents it, including the
    // minimum-progress exemption. A budget too small for even one
    // character must still consume one: you cannot draw less than a
    // character, and returning 0 would leave the wrap loop unable to
    // advance. One clipped grapheme beats a hung UI.
    let m = m();
    let s = "the quick brown fox jumps over the lazy dog";
    let one_char = m.run_width(&s[..1], Style::default());

    for budget in [1u32, 5, 10, 25, 50, 100, 200, 10_000] {
        let (n, w) = m.fit_prefix(s, Style::default(), budget);
        assert!(n <= s.len());
        assert!(n > 0, "must make progress at budget {budget}");
        assert!(s.is_char_boundary(n), "prefix {n} splits a character");
        if n < s.len() {
            assert!(
                w <= budget || w <= one_char,
                "prefix of {n} bytes measures {w}, over budget {budget} \
                 and wider than a single character ({one_char})"
            );
        }
    }
}

#[test]
fn right_to_left_text_fits_as_much_as_left_to_right() {
    // An x on a right-to-left line is visual, so the index Pango maps it
    // to isn't the end of the logical prefix that fits: a Hebrew excerpt
    // kept a few characters, and a Hebrew body wrapped a few to a line.
    let m = m();
    let plain = Style::default();
    for s in [
        "\u{5e9}\u{5dc}\u{5d5}\u{5dd} \u{5e2}\u{5d5}\u{5dc}\u{5dd} \u{5d6}\u{5d4} \u{5de}\u{5d1}\u{5d7}\u{5df} \u{5d0}\u{5e8}\u{5d5}\u{5da} \u{5e9}\u{5dc} \u{5e9}\u{5d5}\u{5e8}\u{5d4}",
        "\u{645}\u{631}\u{62d}\u{628}\u{627} \u{628}\u{627}\u{644}\u{639}\u{627}\u{644}\u{645} \u{647}\u{630}\u{627} \u{633}\u{637}\u{631} \u{637}\u{648}\u{64a}\u{644}",
        "mixed \u{5e9}\u{5dc}\u{5d5}\u{5dd} \u{5e2}\u{5d5}\u{5dc}\u{5dd} and more text",
    ] {
        let budget = m.run_width(s, plain) / 2;
        let (n, w) = m.fit_prefix(s, plain, budget);
        let (kept, all) = (s[..n].chars().count(), s.chars().count());
        assert!(w <= budget, "{s:?}: {w} over {budget}");
        assert!(
            kept + 4 >= all / 2,
            "{s:?}: kept {kept} of {all} characters in half its width"
        );

        let width = budget + m.metrics().space_width;
        let msg = rotulus_layout::Message::system(rotulus_layout::ParsedText::plain(s));
        let p = rotulus_layout::LayoutParams {
            width,
            indent: false,
            ..Default::default()
        };
        let l = rotulus_layout::wrap::layout_message(
            &msg,
            &p,
            rotulus_layout::LayoutGeneration::default(),
            &m,
        );
        assert!(
            l.lines.len() <= 3,
            "{s:?}: twice the width wraps to {} lines",
            l.lines.len()
        );
    }
}

#[test]
fn a_click_lands_where_right_to_left_text_is_drawn() {
    // Hebrew is drawn from the right: a point near the left edge is near
    // its logical end, and one near the right edge near its start. The
    // logical prefix search that wraps it would put both at the start.
    let m = m();
    let plain = Style::default();
    let s = "\u{5e9}\u{5dc}\u{5d5}\u{5dd} \u{5e2}\u{5d5}\u{5dc}\u{5dd} \u{5d6}\u{5d4}";
    let full = m.run_width(s, plain);
    let chars = |at: usize| s[..at].chars().count();
    let all = s.chars().count();
    let left = m.offset_at_x(s, plain, 2);
    let right = m.offset_at_x(s, plain, full as i32 - 2);
    assert!(s.is_char_boundary(left) && s.is_char_boundary(right));
    assert!(
        chars(left) + 2 >= all,
        "the left edge is the end, got {left}"
    );
    assert!(
        chars(right) <= 1,
        "the right edge is the start, got {right}"
    );
    let latin = "left to right";
    // Left of the line is its end, after the last character; inside the
    // leftmost glyph, the last character itself.
    let last = s.char_indices().next_back().unwrap().0;
    assert_eq!(m.offset_at_x(s, plain, -5), s.len());
    assert_eq!(m.offset_at_x(s, plain, 1), last);
    assert_eq!(m.offset_at_x(latin, plain, -5), 0);
    assert_eq!(m.offset_at_x(latin, plain, 1), 0);
    assert_eq!(m.offset_at_x(latin, plain, 10_000), latin.len());
}

#[test]
fn fit_prefix_makes_progress_on_a_tiny_budget() {
    // Returning 0 for a non-empty run would hang the wrap loop. The
    // engine guards against it too, but the measurer must not rely on
    // that.
    let m = m();
    let (n, _) = m.fit_prefix("hello", Style::default(), 1);
    assert!(n > 0, "must consume at least one character");
}

#[test]
fn fit_prefix_handles_multibyte() {
    let m = m();
    let s = "héllo → wörld 😀 more text here";
    for budget in [1u32, 7, 13, 40, 90] {
        let (n, _) = m.fit_prefix(s, Style::default(), budget);
        assert!(s.is_char_boundary(n), "prefix {n} split a codepoint");
    }
}

#[test]
fn zoom_scales_measurements() {
    // Zoom is a view scale, so everything must grow together (§3.7).
    let mut m = m();
    let s = "hello world";
    let base = m.run_width(s, Style::default());
    let base_h = m.metrics().line_height;

    m.set_zoom_permille(2000);
    let big = m.run_width(s, Style::default());
    let big_h = m.metrics().line_height;

    assert!(big > base, "text should widen with zoom ({base} -> {big})");
    assert!(
        big_h > base_h,
        "line height should grow with zoom ({base_h} -> {big_h})"
    );

    m.set_zoom_permille(1000);
    assert_eq!(
        m.run_width(s, Style::default()),
        base,
        "returning to 100% must restore the original measurement"
    );
}

#[test]
fn zoom_is_clamped() {
    let mut m = m();
    m.set_zoom_permille(0);
    assert!(m.zoom_permille() >= 250);
    m.set_zoom_permille(u32::MAX);
    assert!(m.zoom_permille() <= 5000);
}

#[test]
fn cache_returns_consistent_widths() {
    // The cache keys on (text, attrs) and ignores colour, since colour
    // cannot affect advance width. Verify that assumption holds rather
    // than trusting it.
    let m = m();
    let s = "cached";
    let a = Style::default();
    let b = Style {
        fg: rotulus_layout::ColorRef::Palette(4),
        ..Default::default()
    };
    assert_eq!(m.run_width(s, a), m.run_width(s, b));
    // And a repeat hit agrees with the first.
    assert_eq!(m.run_width(s, a), m.run_width(s, a));
}

#[test]
fn font_change_invalidates_the_cache() {
    let mut m = m();
    let s = "the quick brown fox";
    let small = m.run_width(s, Style::default());
    m.set_font(pango::FontDescription::from_string("Monospace 30"));
    let large = m.run_width(s, Style::default());
    assert!(
        large > small,
        "a larger font must measure wider ({small} -> {large}); a stale \
         cache would return the old value"
    );
}

// ---- the GTK smoke test (requires a display) ------------------------
//
// Every bring-up crash lived in `class_init` or in widget
// construction, and none was visible to `cargo test`: gtk4-rs asserts an
// initialised GTK inside the generated `class_init` itself
// (gtk4-0.10.3/src/subclass/widget.rs:563), so even registering the type
// needs a display.
//
// **This is deliberately ONE test, not several.** `gtk4::init()` records
// the calling thread as *the* GTK thread, and libtest runs each test on
// its own spawned thread — so a second test that touches GTK trips
// "GTK may only be used from the main thread", which aborts rather than
// failing, taking the whole run with it. (That is not hypothetical: the
// first version of this file was three tests behind a shared `OnceLock`
// gate, and the OnceLock made it worse by telling the second thread GTK
// was ready.) Keeping all GTK work in a single test function means only
// one thread ever touches it, whatever the harness does.
//
// It needs a display, and fails without one. It exercises the exact call
// sequence an application's setup performs, which is what all three crashes
// died in.

use gtk4::glib::prelude::*;
use gtk4::glib::translate::IntoGlib;
use gtk4::prelude::*;

#[test]
fn gtk_class_and_construction_smoke() {
    // No display is a failure, not a skip.
    //
    // This used to return early with a message, which made it pass on CI
    // without running a line of it — so it read as coverage on the dashboard
    // while covering nothing, and the reference-cycle leak it exists to catch
    // sat undetected until someone ran the suite on a desktop. A skip that
    // looks like a pass is worse than no test at all; the repo has a rule
    // about this and it applies to Rust tests as much as to the C ones.
    //
    // CI now supplies a display (`xvfb-run`, see .github/workflows/ci.yml),
    // so failing here means the harness is misconfigured, which is exactly
    // what we want to hear about.
    assert!(
        gtk4::init().is_ok(),
        "GTK could not be initialised — this test needs a display. \
         Run it under `xvfb-run -a` (with GDK_BACKEND=x11), the way CI does."
    );

    let t = crate::view::RotulusView::static_type();

    // --- class_init ran at all -------------------------------------
    unsafe {
        let c = gtk4::glib::gobject_ffi::g_type_class_ref(t.into_glib());
        assert!(!c.is_null(), "class_init failed for RotulusView");
        gtk4::glib::gobject_ffi::g_type_class_unref(c);
    }

    // --- the GtkScrollable properties are installed ----------------
    //
    // Pins the third bring-up crash: declaring fresh ParamSpecs named
    // "hadjustment" etc. collides with the interface's, GLib refuses to
    // install them, and g_object_new yields an object that fails
    // GTK_IS_WIDGET. ParamSpecOverride::for_interface is the fix.
    unsafe {
        let class = gtk4::glib::gobject_ffi::g_type_class_ref(t.into_glib())
            as *mut gtk4::glib::gobject_ffi::GObjectClass;
        for name in [
            c"hadjustment",
            c"vadjustment",
            c"hscroll-policy",
            c"vscroll-policy",
        ] {
            let p = gtk4::glib::gobject_ffi::g_object_class_find_property(
                class,
                name.as_ptr() as *const _,
            );
            assert!(
                !p.is_null(),
                "GtkScrollable property {name:?} is not installed on RotulusView"
            );
        }
        gtk4::glib::gobject_ffi::g_type_class_unref(class as *mut _);
    }

    // --- the typed signals are registered ---------------------------
    //
    // glib-rs's Signal::builder panics on a non-canonical name, and that
    // panic aborts out of class_init; reaching here at all says they
    // registered, and this says they are findable by the names the
    // header documents.
    unsafe {
        for name in [
            c"link-activated",
            c"link-menu",
            c"speaker-activated",
            c"speaker-menu",
            c"load-more",
            c"media-activated",
            c"selection-changed",
            c"reply-activated",
            c"row-menu",
            c"reaction-activated",
            c"reaction-menu",
        ] {
            let id =
                gtk4::glib::gobject_ffi::g_signal_lookup(name.as_ptr() as *const _, t.into_glib());
            assert_ne!(id, 0, "{name:?} is not registered");
        }
        assert_eq!(
            gtk4::glib::gobject_ffi::g_signal_lookup(
                c"word-click".as_ptr() as *const _,
                t.into_glib()
            ),
            0,
            "word-click is gone"
        );
    }

    // --- a constructed view is a usable widget ---------------------
    //
    // Pins the first bring-up crash (use-after-free on the returned pointer)
    // and re-checks the third: all three produced something that failed
    // exactly this.
    let view = crate::view::RotulusView::new();
    assert!(view.is::<gtk4::Widget>(), "not a GtkWidget");
    assert!(view.is::<gtk4::Scrollable>(), "not a GtkScrollable");

    // --- it survives an application's setup calls ---------------
    view.set_font_from_string("Monospace 10");
    view.set_word_wrap(true);
    view.set_max_rows(500);
    view.set_indent(true);
    view.set_max_indent(256);
    view.set_zoom_permille(1000);

    // Appending must not panic on a RefCell re-entrancy, which is the
    // remaining untested hazard in the adjustment plumbing.
    let a = view.append(crate::view::plain_message("hello"));
    let b = view.append(crate::view::plain_message("world"));
    assert_ne!(a, b, "marks must be distinct");
    assert!(view.remove(a), "removing a live mark should succeed");
    assert!(
        !view.remove(a),
        "removing a stale mark is a no-op, not a panic"
    );
    view.clear();

    // --- the FFI path, on a floating pointer -----------------------
    //
    // The check the Rust-side construction above cannot make, and the
    // one that would have caught the longest-running bring-up bug.
    //
    // C receives the widget *floating* with refcount 1. glib-rs's
    // `from_glib_none` sinks floating references, so wrapping the
    // incoming pointer with it handed ownership to a temporary Rust
    // wrapper that dropped at the end of the call and destroyed the
    // widget — on the *first* FFI call after construction. Constructing
    // in Rust never sees this, because the wrapper holds a real
    // reference and nothing is floating.
    //
    // So: build it the way C does, call through the C ABI, and assert
    // it is still alive and still floating afterwards.
    unsafe {
        let pal = [gtk4::gdk::RGBA::BLACK; crate::view::PALETTE_COLS];
        let raw = crate::ffi::rotulus_view_new();
        assert!(!raw.is_null(), "rotulus_view_new returned NULL");
        let as_obj = raw as *mut gtk4::glib::gobject_ffi::GObject;
        assert_ne!(
            gtk4::glib::gobject_ffi::g_object_is_floating(as_obj),
            0,
            "rotulus_view_new must hand C a floating ref, like a GTK constructor"
        );

        // The entry points an application's setup calls.
        crate::ffi::rotulus_view_set_palette(raw, pal.as_ptr() as *const gtk4::gdk::ffi::GdkRGBA);
        crate::ffi::rotulus_view_set_font(raw, c"Monospace 10".as_ptr());
        crate::ffi::rotulus_view_set_word_wrap(raw, 1);
        crate::ffi::rotulus_view_set_max_lines(raw, 500);
        crate::ffi::rotulus_view_set_indent(raw, 1);
        crate::ffi::rotulus_view_set_show_timestamps(raw, 1);
        crate::ffi::rotulus_view_set_max_indent(raw, 256);
        crate::ffi::rotulus_view_set_link_schemes(
            raw,
            [
                c"https://".as_ptr(),
                c"hotline://".as_ptr(),
                std::ptr::null(),
            ]
            .as_ptr(),
        );
        let _ = crate::ffi::rotulus_view_get_vadjustment(raw);
        let gutter = [plain_run(c"<alice>")];
        let body = [plain_run(c"hello")];
        let row = crate::ffi::RotulusRow {
            kind: crate::ffi::ROW_MESSAGE,
            flags: 0,
            stamp: 0,
            speaker: crate::ffi::RotulusSpeaker {
                key: 7,
                nick: c"alice".as_ptr(),
                nick_len: -1,
            },
            gutter: gutter.as_ptr(),
            n_gutter: 1,
            body: body.as_ptr(),
            n_body: 1,
        };
        let mark = crate::ffi::rotulus_view_append(raw, &row);
        assert!(!mark.is_null(), "append returned no mark");
        assert_eq!(crate::ffi::rotulus_view_get_last(raw), mark);
        assert_ne!(
            crate::ffi::rotulus_view_replace(raw, mark, &row),
            0,
            "a live row can be replaced"
        );
        crate::ffi::rotulus_view_set_marker(raw, mark);

        // Still alive, still a widget, still ours to sink.
        //
        // A count above 1 means something took a reference and never gave it
        // back. The usual cause is a closure that captured a strong clone of
        // the view and was then handed to an event controller the view itself
        // owns — a cycle, so the view can never be finalized and its whole
        // message buffer outlives the window. That is what this caught: the
        // gesture and shortcut handlers installed in `constructed` accounted
        // for seventeen of them, and the adjustment's value-changed handler
        // for one more. Weak captures throughout are the fix.
        assert_eq!(
            (*as_obj).ref_count,
            1,
            "something took a reference to the view and never released it — \
             look for a strong self.clone() captured into a closure the view \
             transitively owns"
        );
        assert_ne!(
            gtk4::glib::gobject_ffi::g_object_is_floating(as_obj),
            0,
            "an FFI call sank the caller's floating reference — \
             from_glib_none does this; use g_object_ref instead"
        );
        assert_ne!(
            gtk4::glib::gobject_ffi::g_type_check_instance_is_a(
                raw as *mut gtk4::glib::gobject_ffi::GTypeInstance,
                crate::ffi::rotulus_view_get_type(),
            ),
            0,
            "the widget was destroyed by an FFI call"
        );

        // Clean up the way a C caller would — and check the view actually dies.
        //
        // The refcount assertion above is a proxy for the property that
        // matters, and a proxy a future cycle could satisfy by accident (drop
        // a reference somewhere else and the arithmetic works out again). So
        // watch the object through a weak pointer, which GLib clears in
        // `finalize`, and require it to have been cleared. That fails on any
        // cycle, whatever the count happened to read.
        let mut watch: *mut std::ffi::c_void = as_obj as *mut _;
        gtk4::glib::gobject_ffi::g_object_add_weak_pointer(as_obj, &mut watch);
        gtk4::glib::gobject_ffi::g_object_ref_sink(as_obj);
        gtk4::glib::gobject_ffi::g_object_unref(as_obj);
        assert!(
            watch.is_null(),
            "the view survived its last unref — something still holds a \
             reference to it, so every chat view ever opened leaks along \
             with its message buffer"
        );
    }

    // --- selection + zoom -------------------------------------------
    let view = crate::view::RotulusView::new();
    view.set_font_from_string("Monospace 10");
    view.set_indent(false);
    view.append(crate::view::plain_message("alpha"));
    view.append(crate::view::plain_message("bravo"));

    assert!(!view.has_selection());
    assert_eq!(view.selected_text(), "");
    view.clear_selection(); // no-op, must not panic

    // Word and line select, through the view's own buffer.
    {
        let buf = view.imp_ref().buffer.borrow();
        let id = buf.id_at(0).expect("row 0");
        let caret = rotulus_layout::Caret {
            message: id,
            source: rotulus_layout::LineSource::Block(0),
            offset: 1,
        };
        let word = buf.select_word(&caret).expect("word select");
        assert_eq!(buf.selected_text(&word), "alpha");
        let line = buf.select_row(0).expect("row select");
        assert_eq!(buf.selected_text(&line), "alpha");
    }

    // Zoom walks a fixed ladder and returns to exactly 100%.
    assert_eq!(view.zoom_permille(), 1000);
    view.zoom_step(1);
    let zoomed = view.zoom_permille();
    assert!(zoomed > 1000, "zoom in should raise the level");
    view.zoom_step(-1);
    assert_eq!(
        view.zoom_permille(),
        1000,
        "in then out must land back on exactly 100%, not drift"
    );
    // Select All covers the buffer; Copy is a no-op with nothing
    // selected rather than a panic.
    view.select_all();
    assert!(view.has_selection(), "select_all should select something");
    assert!(view.selected_text().contains("alpha"));
    assert!(view.selected_text().contains("bravo"));
    view.clear_selection();
    assert!(!view.has_selection());

    // Clamps at both ends rather than running off the ladder.
    for _ in 0..40 {
        view.zoom_step(1);
    }
    assert!(view.zoom_permille() <= 4000);
    for _ in 0..80 {
        view.zoom_step(-1);
    }
    assert!(view.zoom_permille() >= 500);

    // --- a following append must land at the true bottom -----------
    //
    // The reported bug: with a multi-line message the view stopped just
    // short of the bottom and the newest row was clipped off the edge.
    //
    // Two independent causes, so this runs twice.
    //
    // `estimate_height` divided a body's byte length by a column count
    // to guess how many lines it wrapped to, which answers the wrong
    // question for a body containing hard newlines: a five-line message
    // was estimated at one. And the view read the scroll offset *before*
    // the frame's layout pass, when following the bottom means
    // `total - viewport` and the layout pass is exactly what corrects
    // the total. Either alone puts the newest row below the bottom edge.
    //
    // The second case is the one no estimator can fix. A proportional
    // font's space is far narrower than its average glyph, so the column
    // count is wildly optimistic and any long line under-counts — and
    // the layout engine deliberately has no font stack to know better.
    //
    // Its own widgets, not the ones above: this asserts on live
    // adjustment state, and reusing a view that earlier checks have
    // zoomed, selected and cleared would make the result depend on what
    // they happened to leave behind.
    for (font, body, case) in [
        (
            "Monospace 10",
            "one\ntwo\nthree\nfour\nfive",
            "hard newlines",
        ),
        ("Sans 10", &"WWWWWWWWWW ".repeat(12), "a proportional font"),
    ] {
        let view = crate::view::RotulusView::new();
        view.set_font_from_string(font);
        view.set_indent(false);
        view.set_word_wrap(true);

        let adj = gtk4::Adjustment::new(0.0, 0.0, 1.0, 1.0, 1.0, 1.0);
        view.set_vadjustment(Some(&adj));

        let holder = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        holder.append(&view);
        // Drives the real `size_allocate`, which is what tells the
        // buffer its width and first configures the adjustment.
        holder.allocate(400, 200, -1, None);

        // The `snapshot` vfunc, called directly on the private impl
        // rather than through `gtk_widget_snapshot_child`. GTK skips an
        // unmapped widget, and mapping one means a real window and a
        // frame clock — an asynchronous dependency in a test asking a
        // synchronous question. The vfunc is the same code GTK runs.
        //
        // Followed by the layout pass the next frame runs: a paint that
        // corrects the scroll position hands the new extent to the
        // adjustment through `size_allocate`, never from the paint
        // itself, which is what the draw-time check below pins.
        let frame = || {
            use gtk4::subclass::prelude::WidgetImpl;
            let before = (adj.upper(), adj.value(), adj.page_size());
            view.imp_ref().snapshot(&gtk4::Snapshot::new());
            assert_eq!(
                before,
                (adj.upper(), adj.value(), adj.page_size()),
                "the paint reconfigured the adjustment; that re-lays out the \
                 scrollbar mid-frame and GTK draws its slider unallocated"
            );
            holder.allocate(400, 200, -1, None);
        };

        for i in 0..60 {
            view.append(crate::view::plain_message(&format!("line {i}")));
        }
        // One frame, so everything already in the buffer is measured —
        // the state a live window is actually in when a message lands.
        frame();
        assert!(
            view.imp_ref().buffer.borrow().is_following(),
            "the view should still be following the bottom"
        );

        view.append(crate::view::plain_message(body));
        frame();

        let bottom = {
            let mut buf = view.imp_ref().buffer.borrow_mut();
            let last = buf.len() - 1;
            let top = buf.index_mut().offset_of(last);
            top + u64::from(buf.index_mut().height_at(last))
        };
        let viewport_end = (adj.value() + adj.page_size()) as u64;
        assert!(
            bottom <= viewport_end,
            "with {case}: the content ends at {bottom}px but the viewport \
             ends at {viewport_end}px — the newest row hangs {}px below \
             the bottom edge",
            bottom.saturating_sub(viewport_end)
        );
        assert!(
            (adj.value() - (adj.upper() - adj.page_size())).abs() < 1.0,
            "with {case}: following the bottom means value == upper - page; \
             got value {} against upper {} page {}",
            adj.value(),
            adj.upper(),
            adj.page_size()
        );
    }

    check_offscreen_animation_stops();
    check_typed_signals();
    check_replies();
    check_reactions();
    check_replace_and_marker();
    check_reveal();
    check_spotlight();
    check_visible_rows();
    check_focus_row();
    check_text_color();
    check_text_scale_remeasures();
    check_keyboard_focus();
    check_row_keyboard();
    #[cfg(feature = "v4_14")]
    check_accessible_caret();
}

/// Tab enters the view, steps through what can be activated in reading
/// order, and leaves past the end; Enter does what a click would.
fn check_keyboard_focus() {
    use rotulus_layout::FocusItem;
    use std::cell::Cell;
    use std::rc::Rc;

    let view = crate::view::RotulusView::new();
    view.set_indent(true);
    let al = view.append(message_row(7, "al", "see https://example.com now"));
    let bo = view.append(message_row(8, "bo", "plain"));
    let holder = laid_out(&view);
    let window = gtk4::Window::new();
    window.set_child(Some(&holder));
    holder.allocate(400, 300, -1, None);
    let focused = || view.imp_ref().focus_item.borrow().clone();

    assert!(view.child_focus(gtk4::DirectionType::TabForward));
    assert!(view.is_focus(), "Tab from outside lands on the view");
    assert_eq!(focused(), None, "as a whole, not on an item");

    let mut seen = Vec::new();
    while view.child_focus(gtk4::DirectionType::TabForward) {
        let item = focused().expect("each Tab inside lands on an item");
        assert!(
            view.item_boxes(&item)
                .iter()
                .all(|r| r.width() > 0.0 && r.y() >= 0.0 && r.y() < 300.0),
            "{item:?} is drawn on screen"
        );
        seen.push(item);
    }
    assert!(
        matches!(
            seen.as_slice(),
            [
                FocusItem::Speaker { message: a },
                FocusItem::Link { message: l, .. },
                FocusItem::Speaker { message: b },
            ] if *a == al && *l == al && *b == bo
        ),
        "reading order: {seen:?}"
    );
    assert_eq!(focused(), None, "past the last item, focus leaves");

    assert!(view.is_focus());
    assert!(
        !view.child_focus(gtk4::DirectionType::TabBackward),
        "Shift+Tab from the view as a whole leaves backward"
    );

    gtk4::prelude::GtkWindowExt::set_focus(&window, None::<&gtk4::Widget>);
    assert!(view.child_focus(gtk4::DirectionType::TabBackward));
    assert_eq!(
        focused(),
        Some(FocusItem::Speaker { message: bo }),
        "Shift+Tab from outside takes the last item on screen"
    );
    let mut back = vec![focused().unwrap()];
    while view.child_focus(gtk4::DirectionType::TabBackward) {
        match focused() {
            Some(item) => back.push(item),
            None => break,
        }
    }
    back.reverse();
    assert_eq!(back, seen, "Shift+Tab visits the same items");
    assert_eq!(
        focused(),
        None,
        "before the first item: the view as a whole"
    );
    assert!(!view.child_focus(gtk4::DirectionType::TabBackward));
    for _ in 0..3 {
        assert!(view.child_focus(gtk4::DirectionType::TabForward));
    }
    assert_eq!(focused(), Some(FocusItem::Speaker { message: bo }));
    let activated = Rc::new(Cell::new(0u64));
    let got = activated.clone();
    view.connect_local("speaker-activated", false, move |args| {
        got.set(args[1].get::<u64>().unwrap());
        None
    });
    assert!(view.activate_focused());
    assert_eq!(
        activated.get(),
        8,
        "Enter activates the item as a click does"
    );

    // The Menu key on the view as a whole opens its menu, whose rows take
    // keyboard focus; closing the menu gives focus back to the view.
    //
    // Whether the popover grabs focus by itself on showing depends on the
    // window being active, which takes a window manager; Xvfb has none.
    // So the window isn't presented, and focus is moved into the menu the
    // way Tab moves it there.
    let ctx = gtk4::glib::MainContext::default();
    let settle = || {
        for _ in 0..100 {
            ctx.iteration(false);
        }
    };
    view.imp_ref().focus_item.replace(None);
    assert!(view.grab_focus());
    view.open_focused_menu();
    settle();
    let popover = {
        let mut child = window.first_child();
        loop {
            let c = child.expect("the menu is parented to the window");
            if let Ok(p) = c.clone().downcast::<gtk4::Popover>() {
                break p;
            }
            child = c.next_sibling();
        }
    };
    // Which row holds the focus takes an active window, and Xvfb has no
    // window manager to make one: what matters, and what this checks, is
    // that every row can take it.
    let mut rows = Vec::new();
    let mut child = popover.child().and_then(|b| b.first_child());
    while let Some(c) = child {
        if c.is::<gtk4::Button>() {
            rows.push(c.clone());
        }
        child = c.next_sibling();
    }
    assert!(!rows.is_empty(), "the menu has rows");
    assert!(
        rows.iter().all(|r| r.is_focusable()),
        "the menu's rows take focus, so they work from the keyboard"
    );
    // Away from the view first, so getting it back is the menu's doing.
    gtk4::prelude::GtkWindowExt::set_focus(&window, None::<&gtk4::Widget>);
    assert!(!view.is_focus());
    popover.popdown();
    settle();
    assert!(
        view.is_focus(),
        "closing the menu returns focus to the view"
    );
    window.destroy();
}

/// Up and Down put the keyboard on a row as a whole, any row with a menu,
/// and Menu there is the row's `row-menu`.
fn check_row_keyboard() {
    use crate::view::RowMark;
    use rotulus_layout::{FocusItem, LoadMoreDirection, Message, MessageKind};
    use std::cell::RefCell;
    use std::rc::Rc;

    let view = crate::view::RotulusView::new();
    view.set_indent(true);
    let framing = |kind| Message {
        kind,
        ..Message::system(rotulus_layout::ParsedText::plain("framing"))
    };
    view.append(framing(MessageKind::LoadMore(LoadMoreDirection::Older)));
    let old = view.append(Message {
        kind: MessageKind::History {
            server_message_id: 9,
        },
        ..message_row(7, "al", "earlier")
    });
    view.append(framing(MessageKind::Divider));
    let al = view.append(message_row(7, "al", "see https://example.com"));
    let grouped = view.append(message_row(7, "al", "more"));
    let emote = view.append(message_row(0, "*", "al waves"));
    let holder = laid_out(&view);
    let window = gtk4::Window::new();
    window.set_child(Some(&holder));
    holder.allocate(400, 300, -1, None);
    let focused = || view.imp_ref().focus_item.borrow().clone();

    let seen: Rc<RefCell<Vec<u64>>> = Rc::default();
    {
        let seen = seen.clone();
        view.connect_closure(
            "row-menu",
            false,
            gtk4::glib::closure_local!(move |_: crate::view::RotulusView,
                                             m: RowMark,
                                             _x: f64,
                                             _y: f64|
                  -> bool {
                seen.borrow_mut().push(m.0 .0);
                true
            }),
        );
    }
    assert!(view.grab_focus());
    gtk4::prelude::GtkWindowExt::set_focus_visible(&window, false);
    let mut up = Vec::new();
    while view.step_row_focus(false) {
        let item = focused().expect("Up lands on a row");
        assert!(!view.item_boxes(&item).is_empty(), "it has a ring to draw");
        assert!(!view.activate_focused(), "Enter on a row does nothing");
        view.open_focused_menu();
        up.push(item);
    }
    assert!(
        gtk4::prelude::GtkWindowExt::gets_focus_visible(&window),
        "the keyboard was used: the ring shows"
    );
    let rows = [emote, grouped, al, old];
    assert_eq!(
        up,
        rows.map(|message| FocusItem::Row { message }),
        "from the view as a whole, the last row, then up past the framing rows"
    );
    assert_eq!(
        *seen.borrow(),
        rows.map(|m| m.0),
        "Menu on each is its row's menu"
    );
    assert_eq!(
        focused(),
        Some(FocusItem::Row { message: old }),
        "the top holds"
    );

    view.imp_ref()
        .focus_item
        .replace(Some(FocusItem::Row { message: al }));
    assert!(view.child_focus(gtk4::DirectionType::TabForward));
    assert_eq!(
        focused(),
        Some(FocusItem::Speaker { message: al }),
        "Tab goes on from the row to its items"
    );
    assert!(view.step_row_focus(true));
    assert_eq!(focused(), Some(FocusItem::Row { message: grouped }));
    // A header set on the row, or an edit of it, leaves the keyboard there.
    let quote = rotulus_layout::Reply::Unavailable("gone".into());
    assert!(view.set_reply(grouped, Some(quote)));
    assert_eq!(focused(), Some(FocusItem::Row { message: grouped }));
    assert!(view.replace(grouped, message_row(7, "al", "edited")));
    assert_eq!(focused(), Some(FocusItem::Row { message: grouped }));
    assert!(view.replace(grouped, framing(MessageKind::Divider)));
    assert_eq!(focused(), None, "not on a row that has no menu");
    window.destroy();
}

/// A change to the desktop's text scale throws away every measurement.
fn check_text_scale_remeasures() {
    let view = crate::view::RotulusView::new();
    let settings = gtk4::Settings::default().expect("GTK is initialized");
    let before = view.imp_ref().font_generation.get();
    let dpi = settings.gtk_xft_dpi();
    settings.set_gtk_xft_dpi(if dpi == 96 * 1024 {
        120 * 1024
    } else {
        96 * 1024
    });
    let ctx = gtk4::glib::MainContext::default();
    while ctx.iteration(false) {}
    settings.set_gtk_xft_dpi(dpi);
    while ctx.iteration(false) {}
    assert_ne!(
        view.imp_ref().font_generation.get(),
        before,
        "the measurements should have been invalidated"
    );
}

/// The accessible caret: at the newest row until something moves it, and
/// on the current match when a search reveals one.
#[cfg(feature = "v4_14")]
fn check_accessible_caret() {
    use gtk4::subclass::prelude::AccessibleTextImpl;

    let view = crate::view::RotulusView::new();
    view.append(message_row(1, "al", "one"));
    view.append(message_row(2, "bo", "two"));
    let _holder = laid_out(&view);
    let imp = view.imp_ref();
    // "al: one\nbo: two"
    assert_eq!(
        imp.caret_position(),
        8,
        "the caret starts on the newest row"
    );
    view.search_set("one", false);
    assert_eq!(imp.caret_position(), 7, "a revealed match moves it");
    let sel = imp.selection();
    assert_eq!(sel.len(), 1);
    assert_eq!((sel[0].start(), sel[0].length()), (4, 3), "and selects it");
    view.search_clear();
    assert_eq!(imp.caret_position(), 8, "clearing the search puts it back");
    assert!(imp.selection().is_empty());
    view.search_set("one", false);

    #[cfg(feature = "v4_16")]
    {
        let r = imp.extents(4, 7).expect("an on-screen range has extents");
        assert!(r.width() > 0.0 && r.height() > 0.0);
        for (dx, want) in [(1.0, 4), (r.width() - 3.0, 6)] {
            let p = gtk4::graphene::Point::new(r.x() + dx, r.y() + r.height() / 2.0);
            assert_eq!(
                imp.offset(&p),
                Some(want),
                "a point maps to the character under it"
            );
        }
    }
}

/// A row of runs, the way an application appends one.
fn message_row(key: u64, nick: &str, body: &str) -> rotulus_layout::Message {
    rotulus_layout::Message {
        kind: rotulus_layout::MessageKind::Live,
        timestamp: 1_000,
        speaker: (key != 0).then(|| rotulus_layout::Speaker::new(key, nick)),
        gutter: Some(rotulus_layout::ParsedText::plain(format!("<{nick}>"))),
        blocks: {
            let mut p = rotulus_layout::ParsedText::plain(body);
            crate::links::autolink(&mut p, &rotulus_layout::Linkifier::default());
            vec![rotulus_layout::Block::Text(p)].into()
        },
        flags: rotulus_layout::MessageFlags::NONE,
    }
}

/// Lay a view out at 400x300 and paint it once, so hit-testing has line
/// boxes to find.
fn laid_out(view: &crate::view::RotulusView) -> gtk4::Box {
    use gtk4::subclass::prelude::WidgetImpl;
    // A view laid out once already is laid out again where it is, after
    // whatever was appended since.
    let holder = match view.parent().and_then(|p| p.downcast::<gtk4::Box>().ok()) {
        Some(holder) => holder,
        None => {
            let adj = gtk4::Adjustment::new(0.0, 0.0, 1.0, 1.0, 1.0, 1.0);
            view.set_vadjustment(Some(&adj));
            let holder = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
            holder.append(view);
            holder
        }
    };
    holder.allocate(400, 300, -1, None);
    view.imp_ref().snapshot(&gtk4::Snapshot::new());
    holder.allocate(400, 300, -1, None);
    holder
}

/// The first widget-space point on row `row` whose hover target `want`
/// accepts.
fn find_point(
    view: &crate::view::RotulusView,
    row: usize,
    want: impl Fn(&crate::view::HoverTarget) -> bool,
) -> Option<(f64, f64)> {
    let (top, height) = {
        let mut buf = view.imp_ref().buffer.borrow_mut();
        (
            buf.index_mut().offset_of(row),
            buf.index_mut().height_at(row),
        )
    };
    let y0 = f64::from(crate::view::PAD_Y) + top as f64;
    for dy in (1..height).step_by(4) {
        for x in (crate::view::PAD_X..396).step_by(3) {
            let (x, y) = (f64::from(x), y0 + f64::from(dy));
            if view.hover_target_at(x, y).as_ref().is_some_and(&want) {
                return Some((x, y));
            }
        }
    }
    None
}

/// Clicks reach the application as typed signals, not as words to match.
fn check_typed_signals() {
    use crate::view::HoverTarget;
    use std::cell::RefCell;
    use std::rc::Rc;

    let view = crate::view::RotulusView::new();
    view.set_indent(true);
    view.append(message_row(7, "al", "see https://example.com now"));
    view.append(rotulus_layout::Message {
        kind: rotulus_layout::MessageKind::LoadMore(rotulus_layout::LoadMoreDirection::Older),
        timestamp: 1_000,
        speaker: None,
        gutter: None,
        blocks: vec![rotulus_layout::Block::text("load older")].into(),
        flags: rotulus_layout::MessageFlags::NONE,
    });
    view.append(rotulus_layout::Message {
        kind: rotulus_layout::MessageKind::Live,
        timestamp: 1_000,
        speaker: None,
        gutter: None,
        blocks: vec![rotulus_layout::Block::Image {
            token: 9,
            size: None,
            alt: "[image]".into(),
        }]
        .into(),
        flags: rotulus_layout::MessageFlags::NONE,
    });
    let _holder = laid_out(&view);

    let seen: Rc<RefCell<Vec<String>>> = Rc::default();
    {
        let seen = seen.clone();
        // Handled, so the view doesn't go on to launch a browser.
        view.connect_closure(
            "link-activated",
            false,
            gtk4::glib::closure_local!(move |_: crate::view::RotulusView, href: String| -> bool {
                seen.borrow_mut().push(format!("link {href}"));
                true
            }),
        );
    }
    {
        let seen = seen.clone();
        view.connect_closure(
            "speaker-activated",
            false,
            gtk4::glib::closure_local!(move |_: crate::view::RotulusView, key: u64| {
                seen.borrow_mut().push(format!("speaker {key}"));
            }),
        );
    }
    {
        let seen = seen.clone();
        view.connect_closure(
            "load-more",
            false,
            gtk4::glib::closure_local!(
                move |_: crate::view::RotulusView, d: crate::view::LoadDirection| {
                    seen.borrow_mut().push(format!("load {d:?}"));
                }
            ),
        );
    }
    {
        let seen = seen.clone();
        view.connect_closure(
            "media-activated",
            false,
            gtk4::glib::closure_local!(move |_: crate::view::RotulusView, token: u32| {
                seen.borrow_mut().push(format!("media {token}"));
            }),
        );
    }

    let link = find_point(&view, 0, |t| matches!(t, HoverTarget::Link { .. }))
        .expect("the link is hittable");
    view.activate_at(link.0, link.1);
    let nick = find_point(&view, 0, |t| matches!(t, HoverTarget::Nick { key: 7, .. }))
        .expect("the nick is hittable");
    view.activate_at(nick.0, nick.1);
    let more = find_point(&view, 1, |t| matches!(t, HoverTarget::LoadMore { .. }))
        .expect("the load-more row is hittable");
    view.activate_at(more.0, more.1);
    let media = find_point(&view, 2, |t| {
        matches!(t, HoverTarget::Media { token: 9, .. })
    })
    .expect("the image is hittable");
    view.activate_at(media.0, media.1);
    assert_eq!(
        *seen.borrow(),
        [
            "link https://example.com",
            "speaker 7",
            "load Older",
            "media 9"
        ]
    );

    // With activation off, a primary click on the link is just a click.
    view.set_activate_links(false);
    view.activate_at(link.0, link.1);
    assert_eq!(
        seen.borrow().len(),
        4,
        "a link doesn't activate when activation is off"
    );
    view.set_activate_links(true);

    // A link whose label isn't its address opens the menu instead, so the
    // real destination is on screen before anything opens.
    {
        let seen = seen.clone();
        view.connect_closure(
            "link-menu",
            false,
            gtk4::glib::closure_local!(move |_: crate::view::RotulusView,
                                             href: String,
                                             _x: f64,
                                             _y: f64|
                  -> bool {
                seen.borrow_mut().push(format!("menu {href}"));
                true
            }),
        );
    }
    view.append(rotulus_layout::Message {
        kind: rotulus_layout::MessageKind::Live,
        timestamp: 1_000,
        speaker: None,
        gutter: None,
        blocks: rotulus_layout::Block::Text(rotulus_layout::markdown::parse_inline(
            "[the docs](https://elsewhere.example)",
        ))
        .into(),
        flags: rotulus_layout::MessageFlags::NONE,
    });
    let _holder = laid_out(&view);
    let last = view.imp_ref().buffer.borrow().len() - 1;
    let label = find_point(&view, last, |t| {
        matches!(
            t,
            HoverTarget::Link {
                disguised: true,
                ..
            }
        )
    })
    .expect("the labeled link is hittable");
    view.activate_at(label.0, label.1);
    assert_eq!(
        seen.borrow().last().map(String::as_str),
        Some("menu https://elsewhere.example"),
        "a disguised link shows its menu rather than opening"
    );

    // Selection changes are announced, and has-selection follows them.
    let changes = Rc::new(std::cell::Cell::new(0));
    {
        let changes = changes.clone();
        view.connect_closure(
            "selection-changed",
            false,
            gtk4::glib::closure_local!(move |_: crate::view::RotulusView| {
                changes.set(changes.get() + 1);
            }),
        );
    }
    let notified = Rc::new(std::cell::Cell::new(0));
    {
        let notified = notified.clone();
        view.connect_notify_local(Some("has-selection"), move |_, _| {
            notified.set(notified.get() + 1)
        });
    }
    view.select_all();
    assert!(view.property::<bool>("has-selection"));
    view.clear_selection();
    assert!(!view.property::<bool>("has-selection"));
    assert_eq!((changes.get(), notified.get()), (2, 2));
}

/// A reply header and the row menu reach the application as signals naming
/// the row by its mark, from the pointer and from the keyboard; beside the
/// header, and on a link or a nick, nothing else answers.
fn check_replies() {
    use crate::view::{HoverTarget, RowMark};
    use rotulus_layout::{ColorRef, FocusItem, Reply};
    use std::cell::RefCell;
    use std::rc::Rc;

    let view = crate::view::RotulusView::new();
    view.set_indent(true);
    view.append(message_row(7, "al", "see https://example.com now"));
    let id = view.append(message_row(8, "bo", "agreed"));
    let quote = Reply::Quote {
        nick: "al".into(),
        color: ColorRef::Default,
        excerpt: "see https://example.com now".into(),
    };
    assert!(view.set_reply(id, Some(quote)));
    // In a window, so a menu the view pops for want of a handler has
    // somewhere to go.
    let holder = laid_out(&view);
    let window = gtk4::Window::new();
    window.set_child(Some(&holder));
    holder.allocate(400, 300, -1, None);

    let seen: Rc<RefCell<Vec<String>>> = Rc::default();
    {
        let seen = seen.clone();
        view.connect_closure(
            "reply-activated",
            false,
            gtk4::glib::closure_local!(
                move |_: crate::view::RotulusView, m: RowMark, keyboard: bool| {
                    let from = if keyboard { "key" } else { "click" };
                    seen.borrow_mut().push(format!("reply {} {from}", m.0 .0));
                }
            ),
        );
    }
    {
        let seen = seen.clone();
        view.connect_closure(
            "row-menu",
            false,
            gtk4::glib::closure_local!(move |_: crate::view::RotulusView,
                                             m: RowMark,
                                             _x: f64,
                                             _y: f64|
                  -> bool {
                seen.borrow_mut().push(format!("row {}", m.0 .0));
                true
            }),
        );
    }
    {
        let seen = seen.clone();
        view.connect_closure(
            "link-menu",
            false,
            gtk4::glib::closure_local!(move |_: crate::view::RotulusView,
                                             _href: String,
                                             _x: f64,
                                             _y: f64|
                  -> bool {
                seen.borrow_mut().push("link".into());
                true
            }),
        );
    }
    {
        let seen = seen.clone();
        view.connect_closure(
            "speaker-menu",
            false,
            gtk4::glib::closure_local!(move |_: crate::view::RotulusView,
                                             key: u64,
                                             _x: f64,
                                             _y: f64| {
                seen.borrow_mut().push(format!("speaker {key}"));
            }),
        );
    }

    let header = find_point(&view, 1, |t| *t == HoverTarget::Reply { message: id })
        .expect("the header is hittable");
    let (beside, body) = {
        let buf = view.imp_ref().buffer.borrow();
        let l = buf.layout_at(1).unwrap();
        let r = **l.reply.as_ref().unwrap();
        let b = l
            .lines
            .iter()
            .find(|l| l.source == rotulus_layout::LineSource::Block(0))
            .unwrap();
        let x = |x: u32| f64::from(crate::view::PAD_X) + f64::from(x) + 2.0;
        let y = |y: u32| header.1 + f64::from(y) - f64::from(r.y);
        ((x(r.x + r.width + 8), header.1), (x(b.x), y(b.y)))
    };
    assert_eq!(
        view.hover_target_at(beside.0, beside.1),
        None,
        "past the header is not the nick on its line"
    );
    // Nor is the gap above the row, across its whole width.
    let row_top = {
        let mut buf = view.imp_ref().buffer.borrow_mut();
        f64::from(crate::view::PAD_Y) + buf.index_mut().offset_of(1) as f64
    };
    for x in [f64::from(crate::view::PAD_X) + 2.0, header.0, 390.0] {
        assert_eq!(
            view.hover_target_at(x, row_top + 1.0),
            None,
            "the gap at {x}"
        );
    }
    view.activate_at(header.0, header.1);
    view.activate_at(beside.0, beside.1);
    for (x, y) in [header, body, beside] {
        view.open_menu_at(x, y);
    }
    let link = find_point(&view, 0, |t| matches!(t, HoverTarget::Link { .. })).unwrap();
    view.open_menu_at(link.0, link.1);
    let nick = find_point(&view, 0, |t| matches!(t, HoverTarget::Nick { .. })).unwrap();
    view.open_menu_at(nick.0, nick.1);
    let n = id.0;
    assert_eq!(
        *seen.borrow(),
        [
            format!("reply {n} click"),
            format!("row {n}"),
            format!("row {n}"),
            format!("row {n}"),
            "link".into(),
            "speaker 7".into(),
        ]
    );

    // From the keyboard: the header is its row's stop after the speaker,
    // Enter is a click on it, and the Menu key opens its row's menu.
    assert!(view.child_focus(gtk4::DirectionType::TabForward));
    while view
        .imp_ref()
        .focus_item
        .borrow()
        .as_ref()
        .map(|i| i.message())
        != Some(id)
    {
        assert!(view.child_focus(gtk4::DirectionType::TabForward));
    }
    assert_eq!(
        *view.imp_ref().focus_item.borrow(),
        Some(FocusItem::Speaker { message: id })
    );
    assert!(view.child_focus(gtk4::DirectionType::TabForward));
    let item = view.imp_ref().focus_item.borrow().clone().unwrap();
    assert_eq!(item, FocusItem::Reply { message: id });
    assert!(!view.item_boxes(&item).is_empty(), "it has a ring to draw");
    seen.borrow_mut().clear();
    assert!(view.activate_focused());
    view.open_focused_menu();
    assert_eq!(
        *seen.borrow(),
        [format!("reply {n} key"), format!("row {n}")]
    );

    // The Menu key on an image opens its row's menu, though the point the
    // menu opens from, under the image, is on the selected row below.
    let image = view.append(rotulus_layout::Message {
        kind: rotulus_layout::MessageKind::Live,
        timestamp: 1_000,
        speaker: None,
        gutter: None,
        blocks: vec![rotulus_layout::Block::Image {
            token: 3,
            size: None,
            alt: "[image]".into(),
        }]
        .into(),
        flags: rotulus_layout::MessageFlags::NONE,
    });
    let below = view.append(message_row(8, "bo", "selected words"));
    let _holder = laid_out(&view);
    let below_row = view.imp_ref().buffer.borrow().row_of(below).unwrap();
    let below_sel = view.imp_ref().buffer.borrow().select_row(below_row);
    view.imp_ref().selection.replace(below_sel);
    view.imp_ref().focus_item.replace(Some(FocusItem::Image {
        message: image,
        block: 0,
    }));
    seen.borrow_mut().clear();
    view.open_focused_menu();
    assert_eq!(*seen.borrow(), [format!("row {}", image.0)]);
    view.clear_selection();
    assert!(view.remove(image) && view.remove(below));
    let _holder = laid_out(&view);
    seen.borrow_mut().clear();
    view.imp_ref()
        .focus_item
        .replace(Some(FocusItem::Reply { message: id }));

    // The header changes and goes without the rest of the row.
    assert!(view.set_reply(id, Some(Reply::Unavailable("deleted".into()))));
    assert_eq!(
        *view.imp_ref().focus_item.borrow(),
        Some(FocusItem::Reply { message: id }),
        "a header that changes keeps focus"
    );
    let _holder = laid_out(&view);
    assert!(find_point(&view, 1, |t| *t == HoverTarget::Reply { message: id }).is_some());
    assert!(view.replace(id, message_row(8, "bo", "agreed, see https://example.com")));
    view.imp_ref()
        .focus_item
        .replace(Some(FocusItem::Reply { message: id }));
    assert!(view.set_reply(id, None));
    assert!(
        matches!(
            *view.imp_ref().focus_item.borrow(),
            Some(FocusItem::Link { message, .. }) if message == id
        ),
        "focus on a header that went moves on to what came after it"
    );
    let _holder = laid_out(&view);
    assert!(find_point(&view, 1, |t| matches!(t, HoverTarget::Reply { .. })).is_none());

    // On the selection, or on a row that isn't a message, a right-click is
    // the view's: no row menu. A selection elsewhere doesn't matter.
    view.append(rotulus_layout::Message {
        kind: rotulus_layout::MessageKind::LoadMore(rotulus_layout::LoadMoreDirection::Older),
        timestamp: 1_000,
        speaker: None,
        gutter: None,
        blocks: vec![rotulus_layout::Block::text("load older")].into(),
        flags: rotulus_layout::MessageFlags::NONE,
    });
    let _holder = laid_out(&view);
    seen.borrow_mut().clear();
    let more = find_point(&view, 2, |t| matches!(t, HoverTarget::LoadMore { .. })).unwrap();
    view.open_menu_at(more.0, more.1);
    let on_text = {
        let mut buf = view.imp_ref().buffer.borrow_mut();
        let top = buf.index_mut().offset_of(1) as f64;
        let l = buf.layout_at(1).unwrap();
        let b = l
            .lines
            .iter()
            .find(|l| l.source == rotulus_layout::LineSource::Block(0))
            .unwrap();
        (
            f64::from(crate::view::PAD_X) + f64::from(b.x) + 2.0,
            f64::from(crate::view::PAD_Y) + top + f64::from(b.y) + 2.0,
        )
    };
    assert_eq!(view.hover_target_at(on_text.0, on_text.1), None);
    view.select_all();
    view.open_menu_at(on_text.0, on_text.1);
    assert_eq!(*seen.borrow(), Vec::<String>::new());
    let first_row = view.imp_ref().buffer.borrow().select_row(0);
    view.imp_ref().selection.replace(first_row);
    view.open_menu_at(on_text.0, on_text.1);
    assert_eq!(*seen.borrow(), [format!("row {n}")]);

    // A right-click is on the selection where it is painted: inside a
    // selected character's glyph, whatever caret the point maps to, and
    // not beside it, though a point left of the body maps to its start.
    let x = view.append(message_row(9, "cy", "x"));
    let _holder = laid_out(&view);
    let (glyph, past, before) = {
        let mut buf = view.imp_ref().buffer.borrow_mut();
        let row = buf.row_of(x).unwrap();
        let top = buf.index_mut().offset_of(row) as f64;
        let b = buf
            .layout_at(row)
            .unwrap()
            .lines
            .iter()
            .find(|l| l.source == rotulus_layout::LineSource::Block(0))
            .unwrap()
            .clone();
        let y = f64::from(crate::view::PAD_Y) + top + f64::from(b.y) + 2.0;
        let x0 = f64::from(crate::view::PAD_X) + f64::from(b.x);
        (
            (x0 + 1.0, y),
            (x0 + f64::from(b.width) + 20.0, y),
            (x0 - 3.0, y),
        )
    };
    let only_x = {
        let buf = view.imp_ref().buffer.borrow();
        let row = buf.row_of(x).unwrap();
        buf.select_row(row).map(|mut s| {
            s.anchor = rotulus_layout::Caret {
                message: x,
                source: rotulus_layout::LineSource::Block(0),
                offset: 0,
            };
            s
        })
    };
    view.imp_ref().selection.replace(only_x);
    seen.borrow_mut().clear();
    view.open_menu_at(glyph.0, glyph.1);
    assert_eq!(*seen.borrow(), Vec::<String>::new(), "on the glyph: Copy");
    view.open_menu_at(past.0, past.1);
    // Just before it, short of the body, whose first caret is selected.
    view.open_menu_at(before.0, before.1);
    assert_eq!(
        *seen.borrow(),
        [format!("row {}", x.0), format!("row {}", x.0)],
        "off the glyph either side: the row's"
    );

    // A decoded image paints no selection over itself, so a right-click
    // on a selected one is its row's.
    let picture = view.append(rotulus_layout::Message {
        kind: rotulus_layout::MessageKind::Live,
        timestamp: 1_000,
        speaker: None,
        gutter: None,
        blocks: vec![rotulus_layout::Block::Image {
            token: 5,
            size: None,
            alt: "[a picture]".into(),
        }]
        .into(),
        flags: rotulus_layout::MessageFlags::NONE,
    });
    let pixels = gtk4::glib::Bytes::from_owned(vec![128u8; 60 * 40 * 4]);
    let texture =
        gtk4::gdk::MemoryTexture::new(60, 40, gtk4::gdk::MemoryFormat::R8g8b8a8, &pixels, 60 * 4);
    view.set_media_frames(5, vec![(texture.upcast(), 0)]);
    let _holder = laid_out(&view);
    let on_picture = {
        let mut buf = view.imp_ref().buffer.borrow_mut();
        let row = buf.row_of(picture).unwrap();
        let top = buf.index_mut().offset_of(row) as f64;
        let l = buf.layout_at(row).unwrap().lines[0].clone();
        assert_eq!((l.width, l.height), (60, 40), "drawn as the picture");
        (
            f64::from(crate::view::PAD_X) + f64::from(l.x) + 4.0,
            f64::from(crate::view::PAD_Y) + top + f64::from(l.y) + 4.0,
        )
    };
    assert!(matches!(
        view.hover_target_at(on_picture.0, on_picture.1),
        Some(HoverTarget::Media { token: 5, .. })
    ));
    view.select_all();
    seen.borrow_mut().clear();
    view.open_menu_at(on_picture.0, on_picture.1);
    assert_eq!(*seen.borrow(), [format!("row {}", picture.0)]);
    view.clear_selection();

    // Below the last row is no row, though the caret clamps to it.
    let below = {
        let mut buf = view.imp_ref().buffer.borrow_mut();
        f64::from(crate::view::PAD_Y) + buf.total_height() as f64 + 10.0
    };
    assert!(below < 300.0, "room below the rows");
    seen.borrow_mut().clear();
    view.open_menu_at(on_picture.0, below);
    assert_eq!(*seen.borrow(), Vec::<String>::new(), "the view's own menu");

    // A header set since the last draw is found at once: a right-click on
    // it is its row's menu, not its speaker's.
    let first = view.append(message_row(8, "bo", "one"));
    let second = view.append(message_row(
        8,
        "bo",
        "see https://a.example and https://b.example",
    ));
    let quote = || Reply::Quote {
        nick: "al".into(),
        color: ColorRef::Default,
        excerpt: "hi".into(),
    };
    assert!(view.set_reply(second, Some(quote())));
    let _holder = laid_out(&view);
    let row = view.imp_ref().buffer.borrow().row_of(second).unwrap();
    let on_header = find_point(&view, row, |t| *t == HoverTarget::Reply { message: second })
        .expect("the header is hittable");
    assert!(view.set_reply(second, Some(Reply::Unavailable("gone".into()))));
    seen.borrow_mut().clear();
    view.open_menu_at(on_header.0, on_header.1);
    assert_eq!(
        *seen.borrow(),
        [format!("row {}", second.0)],
        "before any draw"
    );

    // Its row grouped with the one above once its header goes, so the
    // speaker it had is no stop any more: focus moves on to the first link,
    // the stop that came after the header.
    let _holder = laid_out(&view);
    view.imp_ref()
        .focus_item
        .replace(Some(FocusItem::Reply { message: second }));
    assert!(view.set_reply(second, None));
    let buf = view.imp_ref().buffer.borrow();
    assert!(buf
        .message(second)
        .unwrap()
        .flags
        .contains(rotulus_layout::MessageFlags::GROUPED));
    let links: Vec<_> = buf.focus_items(buf.row_of(second).unwrap());
    drop(buf);
    assert_eq!(
        view.imp_ref().focus_item.borrow().as_ref(),
        links.first(),
        "the first link, not the second"
    );
    assert!(matches!(links.first(), Some(FocusItem::Link { .. })));
    // So does focus on the speaker itself, which the regroup takes away.
    assert!(view.set_reply(second, Some(quote())));
    let _holder = laid_out(&view);
    view.imp_ref()
        .focus_item
        .replace(Some(FocusItem::Speaker { message: second }));
    assert!(view.set_reply(second, None));
    assert_eq!(
        view.imp_ref().focus_item.borrow().as_ref(),
        links.first(),
        "from a speaker that went, the first stop after it"
    );
    assert!(view.remove(first) && view.remove(second));

    view.clear_selection();
    assert!(view.remove(id));
    assert!(!view.set_reply(id, None), "a stale mark sets nothing");
    window.destroy();
}

/// Reaction chips reach the application as `reaction-activated` with the
/// row's mark and the chip's key, from the pointer and the keyboard;
/// between chips is nothing, and a chip's menu is its row's.
fn check_reactions() {
    use crate::view::{HoverTarget, RowMark};
    use rotulus_layout::{FocusItem, Reaction};
    use std::cell::RefCell;
    use std::rc::Rc;

    let react = |key: &str, count, mine| Reaction {
        key: key.into(),
        text: None,
        count,
        mine,
        label: None,
    };
    let view = crate::view::RotulusView::new();
    view.set_indent(true);
    // A link under the chips' band, which the caret hit-test would answer
    // a point between them with.
    let id = view.append(message_row(7, "al", "see https://example.com/a/long/path"));
    assert!(view.set_reactions(id, vec![react("+1", 3, true), react("ok", 1, false)]));
    let holder = laid_out(&view);
    let window = gtk4::Window::new();
    window.set_child(Some(&holder));
    holder.allocate(400, 300, -1, None);

    let seen: Rc<RefCell<Vec<String>>> = Rc::default();
    {
        let seen = seen.clone();
        view.connect_closure(
            "reaction-activated",
            false,
            gtk4::glib::closure_local!(move |_: crate::view::RotulusView,
                                             m: RowMark,
                                             key: String| {
                seen.borrow_mut().push(format!("{} {key}", m.0 .0));
            }),
        );
    }
    {
        let seen = seen.clone();
        view.connect_closure(
            "row-menu",
            false,
            gtk4::glib::closure_local!(move |_: crate::view::RotulusView,
                                             m: RowMark,
                                             _x: f64,
                                             _y: f64|
                  -> bool {
                seen.borrow_mut().push(format!("row {}", m.0 .0));
                true
            }),
        );
    }
    // Whether the application takes a chip's menu, or leaves it to the row.
    let takes = Rc::new(std::cell::Cell::new(false));
    {
        let (seen, takes) = (seen.clone(), takes.clone());
        view.connect_closure(
            "reaction-menu",
            false,
            gtk4::glib::closure_local!(move |_: crate::view::RotulusView,
                                             m: RowMark,
                                             key: String,
                                             _x: f64,
                                             _y: f64|
                  -> bool {
                seen.borrow_mut().push(format!("menu {} {key}", m.0 .0));
                takes.get()
            }),
        );
    }
    let chip = |i: usize| {
        find_point(
            &view,
            0,
            |t| matches!(t, HoverTarget::Reaction { index, .. } if *index == i),
        )
        .expect("the chip is hittable")
    };
    let (first, second) = (chip(0), chip(1));
    view.activate_at(first.0, first.1);
    view.activate_at(second.0, second.1);
    view.open_menu_at(second.0, second.1);
    takes.set(true);
    view.open_menu_at(second.0, second.1);
    let between = {
        let buf = view.imp_ref().buffer.borrow();
        let c = buf.layout_at(0).unwrap().chips.as_deref().unwrap()[0];
        f64::from(crate::view::PAD_X) + f64::from(c.x + c.width) + 1.0
    };
    assert_eq!(
        view.hover_target_at(between, first.1),
        None,
        "between chips"
    );
    // The padding above the chips is theirs too, not the link above it.
    let (over_link, padding) = {
        let mut buf = view.imp_ref().buffer.borrow_mut();
        let top = f64::from(crate::view::PAD_Y) + buf.index_mut().offset_of(0) as f64;
        let l = buf.layout_at(0).unwrap();
        let body = l
            .lines
            .iter()
            .rfind(|l| l.source == rotulus_layout::LineSource::Block(0))
            .unwrap();
        let c = l.chips.as_deref().unwrap()[0];
        let x = f64::from(crate::view::PAD_X) + f64::from(body.x + body.width) - 4.0;
        (
            (x, top + f64::from(body.y) + 2.0),
            (x, top + f64::from(c.y) - 1.0),
        )
    };
    assert!(matches!(
        view.hover_target_at(over_link.0, over_link.1),
        Some(HoverTarget::Link { .. })
    ));
    assert_eq!(
        view.hover_target_at(padding.0, padding.1),
        None,
        "the chips' padding"
    );
    let n = id.0;
    assert_eq!(
        *seen.borrow(),
        [
            format!("{n} +1"),
            format!("{n} ok"),
            format!("menu {n} ok"),
            format!("row {n}"),
            format!("menu {n} ok"),
        ],
        "a chip's menu is its row's only when the application doesn't take it"
    );

    // From the keyboard: the chips are the row's last stops, and Enter is
    // a click on one.
    assert!(view.child_focus(gtk4::DirectionType::TabForward));
    while !matches!(
        *view.imp_ref().focus_item.borrow(),
        Some(FocusItem::Reaction { index: 1, .. })
    ) {
        assert!(view.child_focus(gtk4::DirectionType::TabForward));
    }
    let item = view.imp_ref().focus_item.borrow().clone().unwrap();
    assert!(!view.item_boxes(&item).is_empty(), "it has a ring to draw");
    seen.borrow_mut().clear();
    assert!(view.activate_focused());
    view.open_focused_menu();
    assert_eq!(*seen.borrow(), [format!("{n} ok"), format!("menu {n} ok")]);
    takes.set(false);

    // Focus stays on its reaction when the ones before it go.
    assert!(view.set_reactions(
        id,
        vec![
            react("+1", 3, true),
            react("ok", 1, false),
            react("eyes", 2, false)
        ]
    ));
    assert_eq!(
        *view.imp_ref().focus_item.borrow(),
        Some(FocusItem::Reaction {
            message: id,
            index: 1
        })
    );
    assert!(view.set_reactions(id, vec![react("ok", 1, false), react("eyes", 2, false)]));
    let keys = |id| {
        view.reactions_of(id)
            .into_iter()
            .map(|r| (r.key, r.count))
            .collect::<Vec<_>>()
    };
    assert_eq!(keys(id), [("ok".into(), 1), ("eyes".into(), 2)]);
    assert_eq!(
        *view.imp_ref().focus_item.borrow(),
        Some(FocusItem::Reaction {
            message: id,
            index: 0
        }),
        "still ok, not eyes"
    );
    assert!(view.set_reactions(id, vec![react("+1", 3, true), react("ok", 1, false)]));
    let _holder = laid_out(&view);

    // A focused chip that goes leaves focus on what is left of its row,
    // and the pointer's chip is forgotten, since it may be another now.
    view.imp_ref()
        .hovered
        .replace(view.hover_target_at(second.0, second.1));
    assert!(view.imp_ref().hovered.borrow().is_some());
    assert!(view.set_reactions(id, vec![react("+1", 3, true)]));
    assert_eq!(*view.imp_ref().hovered.borrow(), None);
    assert_eq!(
        *view.imp_ref().focus_item.borrow(),
        Some(FocusItem::Reaction {
            message: id,
            index: 0
        })
    );
    assert!(view.set_reactions(id, Vec::new()));
    let _holder = laid_out(&view);
    assert!(find_point(&view, 0, |t| matches!(t, HoverTarget::Reaction { .. })).is_none());
    assert!(view.set_reactions(id, vec![react("+1", 3, true)]));
    assert!(view.remove(id));
    assert_eq!(keys(id), [], "a removed row shows none");
    assert!(
        !view.set_reactions(id, Vec::new()),
        "a stale mark sets nothing"
    );

    // Tab to a chip under a message taller than the view scrolls the chip,
    // not just its row's top, into sight.
    let tall = (0..40)
        .map(|i| match i {
            33 => "see https://example.com/mid".to_string(),
            i => format!("line {i}"),
        })
        .collect::<Vec<_>>()
        .join("\n");
    let tall = view.insert_before(None, message_row(7, "al", &tall));
    assert!(view.set_reactions(tall, vec![react("+1", 3, false)]));
    let _holder = laid_out(&view);
    view.scroll_to_extreme(false);
    let _holder = laid_out(&view);
    assert!(view.grab_focus());
    view.imp_ref().focus_item.replace(None);
    while !matches!(
        *view.imp_ref().focus_item.borrow(),
        Some(FocusItem::Reaction { message, .. }) if message == tall
    ) {
        assert!(
            view.child_focus(gtk4::DirectionType::TabForward),
            "Tab reaches the chip"
        );
    }
    let item = view.imp_ref().focus_item.borrow().clone().unwrap();
    let _holder = laid_out(&view);
    let boxes = view.item_boxes(&item);
    assert!(
        boxes
            .iter()
            .all(|r| r.y() >= 0.0 && r.y() + r.height() <= 300.0),
        "the chip is on screen: {boxes:?}"
    );
    // Back to a link already in sight in the same tall row: nothing moves,
    // where revealing the row would jump to its top.
    let scroll = |v: &crate::view::RotulusView| v.vadjustment().unwrap().value();
    let before = scroll(&view);
    assert!(view.child_focus(gtk4::DirectionType::TabBackward));
    assert!(matches!(
        *view.imp_ref().focus_item.borrow(),
        Some(FocusItem::Link { message, .. }) if message == tall
    ));
    assert_eq!(scroll(&view), before, "an item in sight stays put");
    window.destroy();
}

#[test]
fn a_mark_in_a_value_is_a_row_and_null_is_none() {
    use crate::view::RowMark;
    use gtk4::glib::value::ToValue;
    let m = RowMark(rotulus_layout::MessageId(5));
    assert_eq!(m.to_value().get::<RowMark>(), Ok(m));
    let null = None::<RowMark>.to_value();
    assert!(null.get::<RowMark>().is_err(), "NULL is no row");
    assert_eq!(null.get::<Option<RowMark>>(), Ok(None));
}

/// `reveal` brings a row from far above into view, moving keyboard focus
/// and the accessible caret to it only when asked to.
fn check_reveal() {
    use rotulus_layout::FocusItem;

    let view = crate::view::RotulusView::new();
    let ids: Vec<_> = (0..200)
        .map(|i| view.append(message_row(1, "al", &format!("line {i}"))))
        .collect();
    let holder = laid_out(&view);
    let window = gtk4::Window::new();
    window.set_child(Some(&holder));
    holder.allocate(400, 300, -1, None);
    let imp = view.imp_ref();
    let focused = || imp.focus_item.borrow().clone();
    #[cfg(feature = "v4_14")]
    let caret_on = || imp.a11y_caret.get().map(|s| s.focus.message);
    assert!(
        view.rows_on_screen().unwrap().0 > 100,
        "it starts at the bottom"
    );

    let mut focus_on = None;
    for (row, focus) in [(100, true), (0, false)] {
        assert!(view.reveal(ids[row], focus));
        let (first, last) = view.rows_on_screen().unwrap();
        assert!((first..=last).contains(&row), "row {row} is on screen");
        assert!(!imp.buffer.borrow().is_following(), "it left the bottom");
        if focus {
            focus_on = Some(ids[row]);
        }
        assert_eq!(
            focused(),
            focus_on.map(|message| FocusItem::Row { message }),
            "focus moves only with focus"
        );
        #[cfg(feature = "v4_14")]
        assert_eq!(caret_on(), focus_on, "and so does the caret");
    }

    assert!(view.remove(ids[150]));
    view.set_max_rows(150);
    assert!(view.reveal(ids[100], false));
    let shown = view.rows_on_screen();
    for (gone, focus) in [(150, false), (150, true), (0, false), (0, true)] {
        assert!(!view.reveal(ids[gone], focus), "row {gone} is gone");
    }
    assert_eq!(view.rows_on_screen(), shown, "and is no scroll");
    assert_eq!(
        focused(),
        focus_on.map(|message| FocusItem::Row { message })
    );
    window.destroy();
}

/// The spotlight tints its own row alone, and goes when cleared or with
/// its row.
fn check_spotlight() {
    use gtk4::subclass::prelude::WidgetImpl;

    let view = crate::view::RotulusView::new();
    let ids: Vec<_> = ["one", "two", "three"]
        .iter()
        .map(|t| view.append(message_row(1, "al", t)))
        .collect();
    let _holder = laid_out(&view);
    // Pixels right of the text, at the top and the bottom of each row.
    let pixels = || -> Vec<[u8; 4]> {
        let snapshot = gtk4::Snapshot::new();
        view.imp_ref().snapshot(&snapshot);
        let node = snapshot.to_node().expect("the view draws");
        let renderer = gtk4::gsk::CairoRenderer::new();
        #[allow(deprecated)]
        renderer.realize(None).expect("an offscreen renderer");
        let out = (0..ids.len() * 2)
            .map(|i| {
                let row = i / 2;
                let (top, h) = {
                    let mut buf = view.imp_ref().buffer.borrow_mut();
                    (
                        buf.index_mut().offset_of(row),
                        buf.index_mut().height_at(row),
                    )
                };
                let edge = if i % 2 == 0 { 0.0 } else { h as f32 - 1.0 };
                let y = crate::view::PAD_Y as f32 + top as f32 + edge;
                let at = gtk4::graphene::Rect::new(300.0, y, 1.0, 1.0);
                let texture = renderer.render_texture(&node, Some(&at));
                let mut d = gtk4::gdk::TextureDownloader::new(&texture);
                d.set_format(gtk4::gdk::MemoryFormat::R8g8b8a8);
                let (bytes, _) = d.download_bytes();
                [bytes[0], bytes[1], bytes[2], bytes[3]]
            })
            .collect();
        renderer.unrealize();
        out
    };
    let plain = pixels();
    assert!(plain.iter().all(|p| *p == plain[0]));

    view.set_spotlight(Some(ids[1]));
    assert_eq!(view.spotlight(), Some(ids[1]));
    let lit = pixels();
    assert!(
        lit[2..4].iter().zip(&plain[2..4]).all(|(l, p)| l != p),
        "its row is tinted, top to bottom"
    );
    assert_eq!(
        [&lit[..2], &lit[4..]],
        [&plain[..2], &plain[4..]],
        "and no other"
    );

    view.set_spotlight(None);
    assert_eq!(pixels(), plain, "cleared, it is gone");
    view.set_spotlight(Some(ids[1]));
    assert!(view.remove(ids[1]));
    assert_eq!(view.spotlight(), None, "it goes with its row");
}

/// The visible rows are those with any pixel in the viewport, and
/// `visible-rows-changed` follows a frame that changed them, once.
fn check_visible_rows() {
    use gtk4::subclass::prelude::WidgetImpl;
    use std::cell::Cell;
    use std::rc::Rc;

    let view = crate::view::RotulusView::new();
    let ids: Vec<_> = (0..200)
        .map(|i| view.append(message_row(1, "al", &format!("line {i}"))))
        .collect();
    assert_eq!(view.visible_rows(), None, "no viewport before a size");
    assert!(!view.is_row_visible(ids[199]));
    let holder = laid_out(&view);
    let told = Rc::new(Cell::new(0));
    {
        let told = told.clone();
        view.connect_closure(
            "visible-rows-changed",
            false,
            gtk4::glib::closure_local!(move |_: crate::view::RotulusView| {
                told.set(told.get() + 1);
            }),
        );
    }
    let frame = || {
        view.imp_ref().snapshot(&gtk4::Snapshot::new());
        let ctx = gtk4::glib::MainContext::default();
        while ctx.iteration(false) {}
    };
    let height = || (view.height() - 2 * crate::view::PAD_Y) as u32;
    // A row's top and bottom edges in the scrollback, by its place.
    let edges = |row: usize| {
        let mut buf = view.imp_ref().buffer.borrow_mut();
        let top = buf.index_mut().offset_of(row);
        (top, top + u64::from(buf.index_mut().height_at(row)))
    };
    let row_of = |id| view.imp_ref().buffer.borrow().row_of(id).unwrap();
    let adj = view.vadjustment().unwrap();
    let scroll = |y: u64| adj.set_value(y as f64);
    let scroll_to = |id| scroll(edges(row_of(id)).0);
    let raw = view.upcast_ref::<gtk4::Widget>().as_ptr() as *mut _;
    let mark = crate::ffi::mark_to_ptr;
    frame();
    told.set(0);

    // (case, what is done, the first row in view if it is known, how
    // often it is told)
    type Act<'a> = Box<dyn Fn() + 'a>;
    let cases: [(&str, Act, Option<usize>, u32); 9] = [
        ("a frame that moves nothing", Box::new(|| {}), None, 0),
        (
            "a row with its last pixel at the top",
            Box::new(|| scroll(edges(50).0 - 1)),
            Some(49),
            1,
        ),
        (
            "that row scrolled out",
            Box::new(|| scroll_to(ids[50])),
            Some(50),
            1,
        ),
        (
            "a row appended out of sight",
            Box::new(|| {
                view.append(message_row(1, "al", "new"));
            }),
            Some(50),
            0,
        ),
        (
            "rows trimmed above",
            Box::new(|| view.set_max_rows(180)),
            Some(50),
            0,
        ),
        (
            "a resize",
            Box::new(|| holder.allocate(400, 200, -1, None)),
            Some(50),
            1,
        ),
        (
            "a reveal",
            Box::new(|| assert!(view.reveal(ids[150], false))),
            None,
            1,
        ),
        (
            "two frames before the idle",
            Box::new(|| {
                scroll_to(ids[90]);
                view.imp_ref().snapshot(&gtk4::Snapshot::new());
                scroll_to(ids[80]);
            }),
            Some(80),
            1,
        ),
        (
            "a row starting at the bottom edge",
            Box::new(|| scroll(edges(90).0 - u64::from(height()))),
            None,
            1,
        ),
    ];
    for (case, act, first, times) in cases {
        act();
        frame();
        assert_eq!(told.replace(0), times, "{case}: told");
        let (f, l) = view.visible_rows().expect("rows are in view");
        if let Some(first) = first {
            assert_eq!(f, ids[first], "{case}: the first row");
        }
        let top = view.imp_ref().buffer.borrow_mut().scroll_offset(height());
        let bottom = top + u64::from(height());
        let (fr, lr) = (row_of(f), row_of(l));
        assert!(
            edges(fr).1 > top && edges(fr).0 <= top,
            "{case}: the first has a pixel in view"
        );
        assert!(
            edges(lr).0 < bottom && edges(lr).1 >= bottom,
            "{case}: so has the last"
        );
        assert!(view.is_row_visible(f) && view.is_row_visible(l), "{case}");
        let past: Vec<_> = [fr.wrapping_sub(1), lr + 1]
            .into_iter()
            .filter_map(|r| view.imp_ref().buffer.borrow().id_at(r))
            .collect();
        assert!(
            !past.iter().any(|&id| view.is_row_visible(id)),
            "{case}: and the rows past them none"
        );
        let (mut cf, mut cl) = (std::ptr::null_mut(), std::ptr::null_mut());
        unsafe {
            assert_eq!(
                crate::ffi::rotulus_view_get_visible_rows(raw, &mut cf, &mut cl),
                1
            );
            assert_eq!((cf, cl), (mark(f), mark(l)), "{case}: and so C is told");
            assert_eq!(crate::ffi::rotulus_view_is_row_visible(raw, mark(l)), 1);
            assert_eq!(
                crate::ffi::rotulus_view_is_row_visible(raw, mark(past[0])),
                0
            );
        }
    }
    assert!(!view.is_row_visible(ids[0]), "nor is a trimmed row");
    view.clear();
    frame();
    assert_eq!(
        (view.visible_rows(), told.get()),
        (None, 1),
        "nor rows once cleared"
    );
}

/// `focus_row` is the row whatever on it the keyboard is on, and nothing
/// while the view has no focus; the view leaves letters to the
/// application.
fn check_focus_row() {
    use gtk4::glib::translate::IntoGlib;
    use rotulus_layout::{LoadMoreDirection, Message, MessageKind};

    let view = crate::view::RotulusView::new();
    view.set_indent(true);
    let more = view.append(Message {
        kind: MessageKind::LoadMore(LoadMoreDirection::Older),
        ..Message::system(rotulus_layout::ParsedText::plain("load older"))
    });
    let al = view.append(message_row(7, "al", "see https://example.com"));
    let quote = rotulus_layout::Reply::Unavailable("gone".into());
    assert!(view.set_reply(al, Some(quote)));
    let bo = view.append(message_row(8, "bo", "plain"));
    let holder = laid_out(&view);
    let window = gtk4::Window::new();
    window.set_child(Some(&holder));
    holder.allocate(400, 300, -1, None);
    let raw = view.upcast_ref::<gtk4::Widget>().as_ptr() as *mut _;
    let from_c = || crate::ffi::ptr_to_mark(unsafe { crate::ffi::rotulus_view_get_focus_row(raw) });

    assert_eq!(view.focus_row(), None, "without focus");
    assert!(view.child_focus(gtk4::DirectionType::TabForward));
    assert_eq!(view.focus_row(), None, "on the view as a whole");
    let mut rows = Vec::new();
    while view.child_focus(gtk4::DirectionType::TabForward) {
        let item = view.imp_ref().focus_item.borrow().clone().unwrap();
        assert_eq!(view.focus_row(), Some(item.message()), "{item:?}");
        assert_eq!(from_c(), view.focus_row(), "{item:?}, from C");
        rows.push(item.message());
    }
    assert_eq!(
        rows,
        [more, al, al, al, bo],
        "the load-more row, al's speaker, header and link, and bo's speaker"
    );
    assert!(view.step_row_focus(false));
    assert_eq!(view.focus_row(), Some(bo), "a row as a whole");

    let keys = view
        .observe_controllers()
        .into_iter()
        .filter_map(|c| c.ok()?.downcast::<gtk4::EventControllerKey>().ok())
        .collect::<Vec<_>>();
    for (key, state) in [
        (gtk4::gdk::Key::r, gtk4::gdk::ModifierType::empty()),
        (gtk4::gdk::Key::R, gtk4::gdk::ModifierType::SHIFT_MASK),
    ] {
        for c in &keys {
            let handled: bool = c.emit_by_name("key-pressed", &[&key.into_glib(), &0u32, &state]);
            assert!(!handled, "{key:?} on a focused row is the application's");
        }
    }

    gtk4::prelude::GtkWindowExt::set_focus(&window, None::<&gtk4::Widget>);
    assert_eq!(view.focus_row(), None, "focus gone from the view");
    assert_eq!(from_c(), None);
    window.destroy();
}

/// Every text node under `node`, by its color.
fn text_colors(node: &gtk4::gsk::RenderNode, out: &mut Vec<gtk4::gdk::RGBA>) {
    use gtk4::gsk;
    if let Some(t) = node.downcast_ref::<gsk::TextNode>() {
        out.push(t.color());
    } else if let Some(c) = node.downcast_ref::<gsk::ContainerNode>() {
        for i in 0..c.n_children() {
            text_colors(&c.child(i), out);
        }
    } else if let Some(t) = node.downcast_ref::<gsk::TransformNode>() {
        text_colors(&t.child(), out);
    } else if let Some(c) = node.downcast_ref::<gsk::ClipNode>() {
        text_colors(&c.child(), out);
    } else if let Some(c) = node.downcast_ref::<gsk::RoundedClipNode>() {
        text_colors(&c.child(), out);
    } else if let Some(o) = node.downcast_ref::<gsk::OpacityNode>() {
        text_colors(&o.child(), out);
    }
}

/// `text_color` is the color a row's text in that color is drawn in, from
/// Rust and from C.
fn check_text_color() {
    use gtk4::gdk::RGBA;
    use gtk4::subclass::prelude::WidgetImpl;
    use rotulus_layout::{ColorRef, ParsedText, Span, Style};

    let view = crate::view::RotulusView::new();
    view.set_time_stamp(false);
    let _holder = laid_out(&view);
    let drawn = |color: ColorRef| {
        let styled = |text: &str| ParsedText {
            text: text.into(),
            spans: vec![Span {
                range: 0..text.len(),
                style: Style {
                    fg: color,
                    ..Style::default()
                },
            }],
            links: Vec::new(),
        };
        view.clear();
        let mut row = message_row(1, "al", "");
        row.gutter = Some(styled("<al>"));
        row.blocks = vec![rotulus_layout::Block::Text(styled("hello"))].into();
        let id = view.append(row);
        // A quoted nick's default is the nick color, not the text color,
        // so only a color named outright is the same there.
        if color != ColorRef::Default {
            let quote = rotulus_layout::Reply::Quote {
                nick: "bo".into(),
                color,
                excerpt: String::new(),
            };
            assert!(view.set_reply(id, Some(quote)));
        }
        laid_out(&view);
        let snapshot = gtk4::Snapshot::new();
        view.imp_ref().snapshot(&snapshot);
        let mut out = Vec::new();
        text_colors(&snapshot.to_node().expect("the view draws"), &mut out);
        out
    };
    let near = |a: &RGBA, b: &RGBA| {
        [
            (a.red(), b.red()),
            (a.green(), b.green()),
            (a.blue(), b.blue()),
            (a.alpha(), b.alpha()),
        ]
        .iter()
        .all(|(x, y)| (x - y).abs() < 1.0 / 512.0)
    };
    let raw = view.upcast_ref::<gtk4::Widget>().as_ptr() as *mut _;
    let yellow = RGBA::new(1.0, 1.0, 0.0, 1.0);
    let follow = RGBA::new(0.0, 0.0, 0.0, 0.0);
    // (case, the slot set, what it is set to, the color asked for, and
    // the same as C names it, when it can)
    let cases = [
        (
            "a legible slot",
            47,
            RGBA::new(0.1, 0.3, 0.6, 1.0),
            ColorRef::Palette(47),
            Some(47),
        ),
        (
            "a slot short of the contrast",
            47,
            yellow,
            ColorRef::Palette(47),
            Some(47),
        ),
        (
            "a slot left to the system",
            40,
            follow,
            ColorRef::Palette(40),
            Some(40),
        ),
        (
            "the default text color",
            crate::view::PAL_FG,
            RGBA::new(0.4, 0.0, 0.0, 1.0),
            ColorRef::Default,
            Some(-1),
        ),
        ("a literal", 47, yellow, ColorRef::Rgb(0x1c71d8), None),
    ];
    for (case, slot, set, color, c_color) in cases {
        let mut pal = crate::view::default_palette();
        pal[crate::view::PAL_BG] = RGBA::WHITE;
        pal[slot] = set;
        view.set_palette(&pal);
        let got = view.text_color(color);
        let seen = drawn(color);
        let texts = if color == ColorRef::Default { 2 } else { 3 };
        assert!(
            seen.len() >= texts,
            "{case}: the nick, the body and any quoted nick are drawn"
        );
        assert!(
            seen.iter().all(|s| near(s, &got)),
            "{case}: drawn in {seen:?}, said to be {got}"
        );
        if color == ColorRef::Palette(47) && set == yellow {
            assert_ne!(got, yellow, "{case}: moved until it reads");
        }
        let Some(c_color) = c_color else {
            continue;
        };
        let mut from_c = gtk4::gdk::ffi::GdkRGBA {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
            alpha: 0.0,
        };
        unsafe { crate::ffi::rotulus_view_get_text_color(raw, c_color, &mut from_c) };
        let from_c = RGBA::new(from_c.red, from_c.green, from_c.blue, from_c.alpha);
        assert!(
            near(&from_c, &got),
            "{case}: C is told {from_c}, Rust {got}"
        );
    }
}

/// Replacing keeps a row's place and id; the marker follows its row out.
fn check_replace_and_marker() {
    let view = crate::view::RotulusView::new();
    let a = view.append(message_row(1, "al", "draft"));
    let b = view.append(message_row(2, "bo", "reply"));
    assert!(view.replace(a, message_row(1, "al", "final text")));
    assert_eq!(view.last(), Some(b), "a replace doesn't move the row");
    {
        let buf = view.imp_ref().buffer.borrow();
        assert_eq!(buf.row_of(a), Some(0));
        assert_eq!(buf.message(a).unwrap().to_plain_text(), "final text");
    }
    view.set_marker(Some(a));
    assert_eq!(view.marker(), Some(a));
    assert!(view.remove(a));
    assert_eq!(view.marker(), None, "the marker goes with its row");
    assert!(
        !view.replace(a, message_row(1, "al", "late")),
        "a stale mark replaces nothing"
    );

    // Properties round-trip, including the ones the setters wrap.
    view.set_link_schemes(&["hotline://"]);
    assert_eq!(view.property::<Vec<String>>("link-schemes"), ["hotline://"]);
    view.set_link_schemes(&[]);
    assert!(view
        .property::<Vec<String>>("link-schemes")
        .contains(&"https://".to_string()));
    view.set_zoom_permille(1250);
    assert!((view.property::<f64>("zoom") - 1.25).abs() < 1e-9);
    view.set_stamp_format("");
    assert_eq!(
        view.property::<String>("timestamp-format"),
        crate::view::DEFAULT_STAMP_FORMAT
    );

    #[cfg(feature = "v4_14")]
    {
        let text = view
            .with_a11y(|m, _| Some(m.contents(0, u32::MAX).to_string()))
            .expect("the accessible text builds");
        assert_eq!(text, "bo: reply");
        view.append(message_row(3, "cy", "new"));
        let text = view
            .with_a11y(|m, _| Some(m.contents(0, u32::MAX).to_string()))
            .unwrap();
        assert_eq!(
            text, "bo: reply\ncy: new",
            "an append extends the accessible text"
        );
    }
}

/// An animated image drawn on screen runs the frame tick; scrolled out of
/// view, the next paint stops it, and scrolled back, the paint after that
/// starts it again. Unmapping stops it too. Part of the one display-backed
/// test above.
fn check_offscreen_animation_stops() {
    use gtk4::prelude::*;
    use gtk4::subclass::prelude::WidgetImpl;

    // Animating at all depends on the desktop allowing it; the check must
    // not depend on the desktop it runs on.
    let settings = gtk4::Settings::default().expect("GTK is initialized");
    let desktop = settings.is_gtk_enable_animations();
    settings.set_gtk_enable_animations(true);

    let view = crate::view::RotulusView::new();
    let adj = gtk4::Adjustment::new(0.0, 0.0, 1.0, 1.0, 1.0, 1.0);
    view.set_vadjustment(Some(&adj));
    let holder = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    holder.append(&view);
    holder.allocate(400, 200, -1, None);
    let frame = || {
        view.imp_ref().snapshot(&gtk4::Snapshot::new());
        holder.allocate(400, 200, -1, None);
    };
    let ticking = || view.imp_ref().anim_tick.borrow().is_some();

    const TOKEN: u32 = 7;
    view.append(rotulus_layout::Message {
        kind: rotulus_layout::MessageKind::Live,
        timestamp: 0,
        speaker: None,
        gutter: None,
        blocks: vec![rotulus_layout::Block::Image {
            token: TOKEN,
            size: None,
            alt: "gif".into(),
        }]
        .into(),
        flags: rotulus_layout::MessageFlags::NONE,
    });
    let texture = |v: u8| -> gtk4::gdk::Texture {
        gtk4::gdk::MemoryTexture::new(
            8,
            8,
            gtk4::gdk::MemoryFormat::R8g8b8a8,
            &gtk4::glib::Bytes::from_owned(vec![v; 8 * 8 * 4]),
            8 * 4,
        )
        .upcast()
    };
    view.set_media_frames(TOKEN, vec![(texture(10), 100), (texture(200), 100)]);
    frame();
    assert!(ticking(), "an animated image on screen should run the tick");

    for i in 0..200 {
        view.append(crate::view::plain_message(&format!("line {i}")));
    }
    frame();
    frame();
    assert!(
        !view.imp_ref().drawn_media.borrow().contains(&TOKEN),
        "the image should be out of view by now"
    );
    assert!(
        !ticking(),
        "an animation scrolled out of view must not keep the tick running"
    );

    view.scroll_to_extreme(false);
    frame();
    frame();
    assert!(
        ticking(),
        "scrolled back into view, it should animate again"
    );

    // Hidden — a tab switched away — no snapshot runs to say the image
    // left the screen; unmapping has to.
    view.imp_ref().unmap();
    assert!(
        !ticking(),
        "an unmapped view must not keep the tick running"
    );
    frame();
    assert!(ticking(), "the next paint should start it again");

    // The desktop's animations turned off: the tick stops at once and the
    // image goes back to its first frame, and stays there.
    view.imp_ref()
        .media
        .borrow_mut()
        .get_mut(&TOKEN)
        .unwrap()
        .current = 1;
    settings.set_gtk_enable_animations(false);
    assert!(!ticking(), "animations off should stop the tick");
    assert_eq!(view.imp_ref().media.borrow()[&TOKEN].current, 0);
    frame();
    assert!(
        !ticking(),
        "a paint must not restart it with animations off"
    );
    settings.set_gtk_enable_animations(true);
    frame();
    assert!(ticking(), "animations back on, the next paint starts it");
    settings.set_gtk_enable_animations(desktop);
}

// ---- run-based append ------------------------------------------------

/// Build a C run array from Rust and hand it to the FFI converter.
///
/// Headless: `runs_to_text` touches no GTK, which is the point of
/// keeping the conversion separate from the widget.
/// A run with only the four common fields set, as C writes them.
fn run_of(
    text: *const std::ffi::c_char,
    len: std::ffi::c_int,
    color: i16,
    attrs: u16,
) -> crate::ffi::RotulusRun {
    crate::ffi::RotulusRun {
        text,
        len,
        color,
        attrs,
        background: 0,
        rgb: 0,
        background_rgb: 0,
    }
}

fn plain_run(t: &'static std::ffi::CStr) -> crate::ffi::RotulusRun {
    run_of(t.as_ptr(), -1, -1, 0)
}

fn to_text(runs: &[(&str, i16, u16)]) -> rotulus_layout::ParsedText {
    let cstrings: Vec<std::ffi::CString> = runs
        .iter()
        .map(|(t, _, _)| std::ffi::CString::new(*t).unwrap())
        .collect();
    let c_runs: Vec<crate::ffi::RotulusRun> = runs
        .iter()
        .zip(&cstrings)
        .map(|((t, color, attrs), cs)| {
            run_of(cs.as_ptr(), t.len() as std::ffi::c_int, *color, *attrs)
        })
        .collect();
    unsafe { crate::ffi::runs_to_text(c_runs.as_ptr(), c_runs.len() as std::ffi::c_int) }
}

#[test]
fn plain_runs_produce_no_spans() {
    // An unstyled row must come out span-free, not one-span-per-run.
    // Otherwise every ordinary chat line carries a span list the
    // renderer then has to walk, and `draw_runs` splits text it has no
    // reason to split.
    let p = to_text(&[("hello ", -1, 0), ("world", -1, 0)]);
    assert_eq!(p.text, "hello world");
    assert!(p.spans.is_empty(), "plain runs must not create spans");
}

#[test]
fn styled_runs_carry_byte_ranges_into_the_joined_text() {
    // The nick-bracket shape: bracket coloured, name default, bracket
    // coloured. This is what used to be
    // "\003NN<\003name\003NN>\003" and had to be re-parsed to find
    // where the name began.
    let p = to_text(&[("<", 5, 0), ("alice", -1, 0), (">", 5, 0)]);
    assert_eq!(p.text, "<alice>");
    assert_eq!(p.spans.len(), 2, "only the styled runs get spans");
    assert_eq!(&p.text[p.spans[0].range.clone()], "<");
    assert_eq!(&p.text[p.spans[1].range.clone()], ">");
    assert_eq!(p.spans[0].style.fg, rotulus_layout::ColorRef::Palette(5));
    assert_eq!(p.spans[1].style.fg, rotulus_layout::ColorRef::Palette(5));
}

#[test]
fn run_ranges_survive_multibyte_text() {
    // Ranges are byte offsets into the joined string, so a multi-byte
    // run before a styled one must not shift it.
    let p = to_text(&[("héllo ", -1, 0), ("wörld", 3, 1)]);
    assert_eq!(p.text, "héllo wörld");
    assert_eq!(p.spans.len(), 1);
    assert_eq!(&p.text[p.spans[0].range.clone()], "wörld");
    assert!(p.spans[0].style.attrs.contains(rotulus_layout::Attrs::BOLD));
}

#[test]
fn empty_runs_are_skipped_not_recorded() {
    let p = to_text(&[("", 4, 0), ("text", -1, 0), ("", 4, 1)]);
    assert_eq!(p.text, "text");
    assert!(p.spans.is_empty());
}

#[test]
fn a_run_with_len_minus_one_uses_strlen() {
    // rotulus.h documents len as "bytes, or -1 for strlen". cslice
    // treats anything <= 0 as empty, so -1 silently dropped the run —
    // an ABI the header promised and the implementation didn't keep.
    let cs = std::ffi::CString::new("hello").unwrap();
    let runs = [run_of(cs.as_ptr(), -1, -1, 0)];
    let p = unsafe { crate::ffi::runs_to_text(runs.as_ptr(), 1) };
    assert_eq!(p.text, "hello");
}

#[test]
fn a_speakers_nick_is_length_delimited_not_nul_delimited() {
    // The chat path hands over a slice into the middle of the received
    // line: the name is bytes [0,5) of "misha:  hello world". Reading to
    // the NUL would take the colon and the body too — and that string
    // feeds the gutter-width estimate in wrap.rs, so it would have
    // reserved a column wide enough for the whole message.
    let line = std::ffi::CString::new("misha:  hello world").unwrap();
    let sp = crate::ffi::RotulusSpeaker {
        key: 7,
        nick: line.as_ptr(),
        nick_len: 5,
    };
    let got = unsafe { crate::ffi::speaker_of(&sp) }.expect("key 7 is known");
    assert_eq!(got.nick, "misha");
    assert_eq!(got.key, 7);

    // -1 still means "this really is a C string".
    let whole = crate::ffi::RotulusSpeaker {
        key: 7,
        nick: line.as_ptr(),
        nick_len: -1,
    };
    let got = unsafe { crate::ffi::speaker_of(&whole) }.unwrap();
    assert_eq!(got.nick, "misha:  hello world");
}

// ---- markdown rendering ---------------------------------------------

/// Build a message body from one plain run, the way live chat does.
fn body_of(text: &str, markdown: bool) -> Vec<rotulus_layout::Block> {
    let cs = std::ffi::CString::new(text).unwrap();
    let runs = [run_of(cs.as_ptr(), text.len() as std::ffi::c_int, -1, 0)];
    unsafe {
        crate::ffi::body_blocks(
            runs.as_ptr(),
            1,
            markdown,
            &rotulus_layout::Linkifier::default(),
        )
    }
}

fn text_of(b: &rotulus_layout::Block) -> &rotulus_layout::ParsedText {
    match b {
        rotulus_layout::Block::Text(p) | rotulus_layout::Block::Quote { content: p, .. } => p,
        _ => panic!("not a text block"),
    }
}

#[test]
fn markdown_renders_inline_emphasis() {
    let blocks = body_of("look at **this** and *that*", true);
    assert_eq!(blocks.len(), 1);
    let p = text_of(&blocks[0]);
    assert_eq!(p.text, "look at this and that");
    let styled: Vec<_> = p
        .spans
        .iter()
        .map(|s| (&p.text[s.range.clone()], s.style.attrs))
        .collect();
    assert_eq!(
        styled,
        vec![
            ("this", rotulus_layout::Attrs::BOLD),
            ("that", rotulus_layout::Attrs::ITALIC),
        ]
    );
}

#[test]
fn markdown_off_leaves_the_delimiters_alone() {
    let blocks = body_of("look at **this**", false);
    let p = text_of(&blocks[0]);
    assert_eq!(p.text, "look at **this**", "text is untouched");
    assert!(p.spans.is_empty());
}

#[test]
fn a_fenced_block_becomes_an_inert_code_block() {
    let blocks = body_of("see:\n```\nlet x = **not bold**;\n```\ndone", true);
    assert_eq!(blocks.len(), 3, "paragraph, code, paragraph");
    match &blocks[1] {
        rotulus_layout::Block::Code { text, .. } => {
            assert!(
                text.contains("**not bold**"),
                "code contents stay literal: {text:?}"
            );
        }
        other => panic!("expected a code block, got {other:?}"),
    }
}

#[test]
fn a_quote_becomes_a_quote_block_with_its_markers_gone() {
    let blocks = body_of("> quoted **bold**", true);
    match &blocks[0] {
        rotulus_layout::Block::Quote { content, depth } => {
            assert_eq!(content.text, "quoted bold");
            assert_eq!(*depth, 1);
        }
        other => panic!("expected a quote, got {other:?}"),
    }
}

#[test]
fn a_styled_body_keeps_its_colour_under_the_markdown() {
    // History rows arrive muted. The renderer treats a gap between spans
    // as *default* style, so without laying the base colour under the
    // parse, a muted line would come back with only its bold words muted
    // and everything else at full contrast.
    let text = "muted **bold** tail";
    let cs = std::ffi::CString::new(text).unwrap();
    let runs = [run_of(cs.as_ptr(), text.len() as std::ffi::c_int, 37, 0)]; // ROTULUS_PAL_MUTED
    let blocks = unsafe {
        crate::ffi::body_blocks(
            runs.as_ptr(),
            1,
            true,
            &rotulus_layout::Linkifier::default(),
        )
    };
    let p = text_of(&blocks[0]);
    assert_eq!(p.text, "muted bold tail");

    // The spans must *tile* the text: start at 0, be contiguous, and
    // reach the end, every one of them carrying the muted colour.
    //
    // Tiling is the property that matters, and asserting it directly is
    // better than sampling positions. The renderer draws a gap between
    // spans in the *default* style, so a single uncovered byte is a
    // visibly unmuted stretch of a muted row — and a per-character loop
    // (which this was) cannot see a gap that falls inside a character
    // anyway.
    let mut spans: Vec<_> = p.spans.iter().collect();
    spans.sort_by_key(|s| s.range.start);
    let mut at = 0usize;
    for s in &spans {
        assert_eq!(
            s.range.start,
            at,
            "gap or overlap before {:?} — the row would draw unmuted there",
            &p.text[s.range.clone()]
        );
        assert_eq!(
            s.style.fg,
            rotulus_layout::ColorRef::Palette(37),
            "{:?} lost the row colour",
            &p.text[s.range.clone()]
        );
        at = s.range.end;
    }
    assert_eq!(at, p.text.len(), "the tail of the row is uncovered");

    // ...and the emphasis is still there on top.
    assert!(p.spans.iter().any(|s| &p.text[s.range.clone()] == "bold"
        && s.style.attrs.contains(rotulus_layout::Attrs::BOLD)));
}

#[test]
fn the_base_colour_tiles_across_multibyte_text() {
    // The tiling above is only interesting if it survives text where a
    // character is several bytes: `under` splices around span
    // boundaries, and getting that wrong on a multi-byte boundary is
    // both a wrong render and a potential panic.
    let text = "héllo **wörld** ☃";
    let cs = std::ffi::CString::new(text).unwrap();
    let runs = [run_of(cs.as_ptr(), text.len() as std::ffi::c_int, 37, 0)];
    let blocks = unsafe {
        crate::ffi::body_blocks(
            runs.as_ptr(),
            1,
            true,
            &rotulus_layout::Linkifier::default(),
        )
    };
    let p = text_of(&blocks[0]);
    assert_eq!(p.text, "héllo wörld ☃");

    let mut spans: Vec<_> = p.spans.iter().collect();
    spans.sort_by_key(|s| s.range.start);
    let mut at = 0usize;
    for s in &spans {
        assert_eq!(s.range.start, at);
        assert!(p.text.is_char_boundary(s.range.start));
        assert!(p.text.is_char_boundary(s.range.end));
        at = s.range.end;
    }
    assert_eq!(at, p.text.len());
}

#[test]
fn a_body_the_caller_styled_run_by_run_is_left_alone() {
    // Chrome — a divider, a "[hx]" line — is styled deliberately by the
    // caller. Re-parsing it would fight that, so a non-uniform body opts
    // out of markdown entirely.
    let a = std::ffi::CString::new("plain ").unwrap();
    let b = std::ffi::CString::new("**loud**").unwrap();
    let runs = [run_of(a.as_ptr(), 6, -1, 0), run_of(b.as_ptr(), 8, 4, 0)];
    let blocks = unsafe {
        crate::ffi::body_blocks(
            runs.as_ptr(),
            2,
            true,
            &rotulus_layout::Linkifier::default(),
        )
    };
    assert_eq!(blocks.len(), 1);
    assert_eq!(text_of(&blocks[0]).text, "plain **loud**");
}

#[test]
fn a_one_line_fence_reaches_the_view_as_a_code_block() {
    // The reported bug end-to-end: "```hello world```" rendered as a
    // blank row, because the scanner read it as an unterminated fence
    // and produced an empty block.
    let blocks = body_of("```hello world```", true);
    assert_eq!(blocks.len(), 1);
    match &blocks[0] {
        rotulus_layout::Block::Code { text, .. } => assert_eq!(text, "hello world"),
        other => panic!("expected code, got {other:?}"),
    }
}

#[test]
fn inline_code_carries_the_code_attr_for_the_renderer_to_tint() {
    // `code` used to be indistinguishable from plain text, because the
    // chat font is already monospace and CODE only set the family. The
    // parse has to at least *mark* it so the draw path can tint it.
    let blocks = body_of("try `ls -l` now", true);
    let p = text_of(&blocks[0]);
    assert_eq!(p.text, "try ls -l now");
    assert!(
        p.spans.iter().any(|s| &p.text[s.range.clone()] == "ls -l"
            && s.style.attrs.contains(rotulus_layout::Attrs::CODE)),
        "the code span must be marked: {:?}",
        p.spans
    );
}

#[test]
fn extended_run_fields_take_effect_only_under_their_bits() {
    let cs = std::ffi::CString::new("x").unwrap();
    let mut r = run_of(cs.as_ptr(), 1, 3, 0);
    r.background = 5;
    r.rgb = 0x123456;
    let plain = crate::ffi::run_style(&r);
    assert_eq!(plain.fg, rotulus_layout::ColorRef::Palette(3));
    assert_eq!(
        plain.bg,
        rotulus_layout::ColorRef::Default,
        "no BACKGROUND bit, no background"
    );

    r.attrs = crate::ffi::ATTR_BACKGROUND | crate::ffi::ATTR_RGB | crate::ffi::ATTR_REVERSE;
    let styled = crate::ffi::run_style(&r);
    assert_eq!(styled.fg, rotulus_layout::ColorRef::Rgb(0x123456));
    assert_eq!(styled.bg, rotulus_layout::ColorRef::Palette(5));
    assert!(styled.attrs.contains(rotulus_layout::Attrs::REVERSE));
}

#[test]
fn a_view_links_its_own_schemes() {
    let only = rotulus_layout::Linkifier::new(["hotline://"]);
    let text = "hotline://example.org and https://example.com";
    let cs = std::ffi::CString::new(text).unwrap();
    let runs = [run_of(cs.as_ptr(), text.len() as std::ffi::c_int, -1, 0)];
    let blocks = unsafe { crate::ffi::body_blocks(runs.as_ptr(), 1, true, &only) };
    let p = text_of(&blocks[0]);
    let hrefs: Vec<_> = p.links.iter().map(|l| l.href.as_str()).collect();
    assert_eq!(hrefs, ["hotline://example.org"]);
}

#[cfg(feature = "mirc")]
#[test]
fn mirc_runs_point_into_the_callers_text() {
    let text = "a \x02bold\x02 \x0304red";
    let cs = std::ffi::CString::new(text).unwrap();
    let mut n = 0;
    let runs = unsafe { crate::mirc_ffi::rotulus_mirc_parse(cs.as_ptr(), -1, &mut n) };
    assert_eq!(n, 4);
    let joined = unsafe { crate::ffi::runs_to_text(runs, n) };
    assert_eq!(joined.text, "a bold red");
    let styles: Vec<_> = joined
        .spans
        .iter()
        .map(|s| (&joined.text[s.range.clone()], s.style))
        .collect();
    assert!(styles
        .iter()
        .any(|(t, s)| *t == "bold" && s.attrs.contains(rotulus_layout::Attrs::BOLD)));
    assert!(styles
        .iter()
        .any(|(t, s)| *t == "red" && s.fg == rotulus_layout::ColorRef::Palette(4)));
    unsafe { gtk4::glib::ffi::g_free(runs as *mut _) };
}
