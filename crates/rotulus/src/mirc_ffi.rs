//! The C side of `rotulus-mirc`: IRC formatting codes as run arrays.

use crate::ffi::{
    RotulusRun, ATTR_BACKGROUND, ATTR_BACKGROUND_RGB, ATTR_BOLD, ATTR_ITALIC, ATTR_MONOSPACE,
    ATTR_REVERSE, ATTR_RGB, ATTR_STRIKETHROUGH, ATTR_UNDERLINE, ROTULUS_COLOR_DEFAULT,
};
use rotulus_layout::{Attrs, ColorRef};
use std::ffi::{c_char, c_int};

/// The run for one segment, pointing into `base`.
fn run_of(base: *const c_char, seg: &rotulus_mirc::Segment) -> RotulusRun {
    let mut run = RotulusRun {
        text: base.wrapping_add(seg.range.start),
        len: (seg.range.end - seg.range.start) as c_int,
        color: ROTULUS_COLOR_DEFAULT,
        attrs: 0,
        background: 0,
        rgb: 0,
        background_rgb: 0,
    };
    for (a, bit) in [
        (Attrs::BOLD, ATTR_BOLD),
        (Attrs::ITALIC, ATTR_ITALIC),
        (Attrs::UNDERLINE, ATTR_UNDERLINE),
        (Attrs::STRIKETHROUGH, ATTR_STRIKETHROUGH),
        (Attrs::CODE, ATTR_MONOSPACE),
        (Attrs::REVERSE, ATTR_REVERSE),
    ] {
        if seg.style.attrs.contains(a) {
            run.attrs |= bit;
        }
    }
    match seg.style.fg {
        ColorRef::Default => {}
        ColorRef::Palette(i) => run.color = i16::from(i),
        ColorRef::Rgb(v) => {
            run.attrs |= ATTR_RGB;
            run.rgb = v;
        }
    }
    match seg.style.bg {
        ColorRef::Default => {}
        ColorRef::Palette(i) => {
            run.attrs |= ATTR_BACKGROUND;
            run.background = i16::from(i);
        }
        ColorRef::Rgb(v) => {
            run.attrs |= ATTR_BACKGROUND_RGB;
            run.background_rgb = v;
        }
    }
    run
}

/// # Safety
/// `text` points to `len` readable bytes (NUL-terminated when `len` is
/// negative) that outlive the returned runs; `n_runs` is writable or
/// NULL. The array is `g_malloc`ed; free it with `g_free`.
#[no_mangle]
pub unsafe extern "C" fn rotulus_mirc_parse(
    text: *const c_char,
    len: c_int,
    n_runs: *mut c_int,
) -> *mut RotulusRun {
    let set_n = |n: usize| {
        if let Some(out) = n_runs.as_mut() {
            *out = n as c_int;
        }
    };
    if text.is_null() {
        set_n(0);
        return std::ptr::null_mut();
    }
    let bytes = if len < 0 {
        std::ffi::CStr::from_ptr(text).to_bytes()
    } else {
        std::slice::from_raw_parts(text as *const u8, len as usize)
    };
    // Invalid UTF-8 is cut at the first bad byte rather than replaced: the
    // runs point into the caller's bytes, so they cannot carry text the
    // caller didn't send.
    let valid = match std::str::from_utf8(bytes) {
        Ok(s) => s,
        Err(e) => std::str::from_utf8(&bytes[..e.valid_up_to()]).unwrap_or(""),
    };
    let segs = rotulus_mirc::segments(valid);
    set_n(segs.len());
    if segs.is_empty() {
        return std::ptr::null_mut();
    }
    let out = gtk4::glib::ffi::g_malloc_n(segs.len(), std::mem::size_of::<RotulusRun>())
        as *mut RotulusRun;
    for (i, seg) in segs.iter().enumerate() {
        out.add(i).write(run_of(text, seg));
    }
    out
}

/// # Safety
/// `m` is NULL or a valid `RotulusMessage *`; `text` is NULL or
/// NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn rotulus_message_add_mirc(
    m: *mut crate::message_ffi::RotulusMessage,
    text: *const c_char,
) {
    let (Some(m), false) = (m.as_mut(), text.is_null()) else {
        return;
    };
    let text = std::ffi::CStr::from_ptr(text).to_string_lossy();
    for seg in rotulus_mirc::segments(&text) {
        let run = run_of(text.as_ptr() as *const c_char, &seg);
        m.push_body(crate::message_ffi::OwnedRun {
            text: text[seg.range.clone()].to_owned(),
            color: run.color,
            attrs: run.attrs,
            background: run.background,
            rgb: run.rgb,
            background_rgb: run.background_rgb,
        });
    }
}
