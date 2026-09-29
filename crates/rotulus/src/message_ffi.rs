//! `RotulusMessage`: the row API for language bindings.
//!
//! `RotulusRow` is shaped for C: arrays of runs on the stack, borrowed
//! for the call. A binding can't build that, so a message owns its runs
//! and becomes a `RotulusRow` pointing into itself when it is appended.
//! It is a boxed type, so introspection knows how to copy and free it.

use crate::ffi::{
    mark_to_ptr, ptr_to_mark, row_message, view_of, CGtkWidget, RotulusRow, RotulusRun,
    RotulusSpeaker, ROTULUS_COLOR_DEFAULT, ROW_MESSAGE,
};
use gtk4::glib;
use gtk4::prelude::*;
use std::ffi::{c_char, c_int, c_uint, c_void, CStr};

/// A run that owns its text.
#[derive(Clone, Debug)]
pub(crate) struct OwnedRun {
    pub text: String,
    pub color: i16,
    pub attrs: u16,
    pub background: i16,
    pub rgb: u32,
    pub background_rgb: u32,
}

impl OwnedRun {
    fn plain(text: String, color: c_int, attrs: c_uint) -> OwnedRun {
        OwnedRun {
            text,
            color: if color < 0 {
                ROTULUS_COLOR_DEFAULT
            } else {
                color.min(255) as i16
            },
            attrs: attrs as u16,
            background: 0,
            rgb: 0,
            background_rgb: 0,
        }
    }

    fn as_run(&self) -> RotulusRun {
        RotulusRun {
            text: self.text.as_ptr() as *const c_char,
            len: self.text.len() as c_int,
            color: self.color,
            attrs: self.attrs,
            background: self.background,
            rgb: self.rgb,
            background_rgb: self.background_rgb,
        }
    }
}

#[derive(Clone, Debug, glib::Boxed)]
#[boxed_type(name = "RotulusMessage")]
pub struct RotulusMessage {
    kind: c_int,
    flags: c_uint,
    stamp: i64,
    key: u64,
    nick: String,
    gutter: Vec<OwnedRun>,
    body: Vec<OwnedRun>,
}

impl RotulusMessage {
    fn new(kind: c_int) -> RotulusMessage {
        RotulusMessage {
            kind,
            flags: 0,
            stamp: 0,
            key: 0,
            nick: String::new(),
            gutter: Vec::new(),
            body: Vec::new(),
        }
    }

    /// Hand `f` the `RotulusRow` this message describes, pointing into it.
    fn with_row<R>(&self, f: impl FnOnce(&RotulusRow) -> R) -> R {
        let gutter: Vec<RotulusRun> = self.gutter.iter().map(OwnedRun::as_run).collect();
        let body: Vec<RotulusRun> = self.body.iter().map(OwnedRun::as_run).collect();
        let row = RotulusRow {
            kind: self.kind,
            flags: self.flags,
            stamp: self.stamp,
            speaker: RotulusSpeaker {
                key: self.key,
                nick: self.nick.as_ptr() as *const c_char,
                nick_len: self.nick.len() as c_int,
            },
            gutter: gutter.as_ptr(),
            n_gutter: gutter.len() as c_int,
            body: body.as_ptr(),
            n_body: body.len() as c_int,
        };
        f(&row)
    }

    #[cfg(feature = "mirc")]
    pub(crate) fn push_body(&mut self, run: OwnedRun) {
        self.body.push(run);
    }
}

/// # Safety
/// `p` is NULL or a valid NUL-terminated C string.
unsafe fn string(p: *const c_char) -> String {
    if p.is_null() {
        String::new()
    } else {
        CStr::from_ptr(p).to_string_lossy().into_owned()
    }
}

#[no_mangle]
pub extern "C" fn rotulus_message_get_type() -> glib::ffi::GType {
    use glib::translate::IntoGlib;
    RotulusMessage::static_type().into_glib()
}

#[no_mangle]
pub extern "C" fn rotulus_message_new(kind: c_int) -> *mut RotulusMessage {
    let kind = if (0..=5).contains(&kind) {
        kind
    } else {
        ROW_MESSAGE
    };
    Box::into_raw(Box::new(RotulusMessage::new(kind)))
}

/// # Safety
/// `m` is NULL or a valid `RotulusMessage *`.
#[no_mangle]
pub unsafe extern "C" fn rotulus_message_copy(m: *const RotulusMessage) -> *mut RotulusMessage {
    match m.as_ref() {
        Some(m) => Box::into_raw(Box::new(m.clone())),
        None => std::ptr::null_mut(),
    }
}

/// # Safety
/// `m` is NULL or a `RotulusMessage *` the caller owns, and doesn't use
/// again.
#[no_mangle]
pub unsafe extern "C" fn rotulus_message_free(m: *mut RotulusMessage) {
    if !m.is_null() {
        drop(Box::from_raw(m));
    }
}

/// # Safety
/// `m` is NULL or a valid `RotulusMessage *`.
#[no_mangle]
pub unsafe extern "C" fn rotulus_message_set_flags(m: *mut RotulusMessage, flags: c_uint) {
    if let Some(m) = m.as_mut() {
        m.flags = flags;
    }
}

/// # Safety
/// `m` is NULL or a valid `RotulusMessage *`.
#[no_mangle]
pub unsafe extern "C" fn rotulus_message_set_timestamp(m: *mut RotulusMessage, stamp: i64) {
    if let Some(m) = m.as_mut() {
        m.stamp = stamp;
    }
}

/// # Safety
/// `m` is NULL or a valid `RotulusMessage *`; `nick` is NULL or
/// NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn rotulus_message_set_speaker(
    m: *mut RotulusMessage,
    key: u64,
    nick: *const c_char,
) {
    if let Some(m) = m.as_mut() {
        m.key = key;
        m.nick = string(nick);
    }
}

/// # Safety
/// `m` is NULL or a valid `RotulusMessage *`; `text` is NULL or
/// NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn rotulus_message_add_nick(
    m: *mut RotulusMessage,
    text: *const c_char,
    color: c_int,
    attrs: c_uint,
) {
    if let Some(m) = m.as_mut() {
        m.gutter.push(OwnedRun::plain(string(text), color, attrs));
    }
}

/// # Safety
/// `m` is NULL or a valid `RotulusMessage *`; `text` is NULL or
/// NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn rotulus_message_add_text(
    m: *mut RotulusMessage,
    text: *const c_char,
    color: c_int,
    attrs: c_uint,
) {
    if let Some(m) = m.as_mut() {
        m.body.push(OwnedRun::plain(string(text), color, attrs));
    }
}

/// # Safety
/// `w` is a valid `RotulusView *`; `m` is NULL or a valid
/// `RotulusMessage *`.
#[no_mangle]
pub unsafe extern "C" fn rotulus_view_append_message(
    w: CGtkWidget,
    m: *const RotulusMessage,
) -> *mut c_void {
    match (view_of(w), m.as_ref()) {
        (Some(v), Some(m)) => m.with_row(|row| mark_to_ptr(v.append(row_message(&v, row)))),
        _ => std::ptr::null_mut(),
    }
}

/// # Safety
/// As [`rotulus_view_append_message`]; `anchor` is a mark or NULL.
#[no_mangle]
pub unsafe extern "C" fn rotulus_view_insert_message_before(
    w: CGtkWidget,
    anchor: *mut c_void,
    m: *const RotulusMessage,
) -> *mut c_void {
    match (view_of(w), m.as_ref()) {
        (Some(v), Some(m)) => m.with_row(|row| {
            let msg = row_message(&v, row);
            mark_to_ptr(v.insert_before(ptr_to_mark(anchor), msg))
        }),
        _ => std::ptr::null_mut(),
    }
}

/// # Safety
/// As [`rotulus_view_append_message`]; `mark` is a mark or NULL.
#[no_mangle]
pub unsafe extern "C" fn rotulus_view_replace_message(
    w: CGtkWidget,
    mark: *mut c_void,
    m: *const RotulusMessage,
) -> c_int {
    match (view_of(w), ptr_to_mark(mark), m.as_ref()) {
        (Some(v), Some(id), Some(m)) => m.with_row(|row| {
            let msg = row_message(&v, row);
            c_int::from(v.replace(id, msg))
        }),
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffi::{runs_to_text, ATTR_BOLD, ROW_SYSTEM};

    #[test]
    fn a_message_becomes_the_row_it_describes() {
        let m = rotulus_message_new(ROW_SYSTEM);
        unsafe {
            rotulus_message_set_speaker(m, 9, c"alice".as_ptr());
            rotulus_message_add_nick(m, c"alice".as_ptr(), 3, ATTR_BOLD as c_uint);
            rotulus_message_add_text(m, c"hello ".as_ptr(), -1, 0);
            rotulus_message_add_text(m, c"there".as_ptr(), 4, 0);
            let copy = rotulus_message_copy(m);
            rotulus_message_free(m);
            (*copy).with_row(|row| {
                assert_eq!(row.kind, ROW_SYSTEM);
                assert_eq!(row.speaker.key, 9);
                let nick = std::slice::from_raw_parts(
                    row.speaker.nick as *const u8,
                    row.speaker.nick_len as usize,
                );
                assert_eq!(nick, b"alice");
                let gutter = runs_to_text(row.gutter, row.n_gutter);
                assert_eq!(gutter.text, "alice");
                assert_eq!(gutter.spans.len(), 1);
                let body = runs_to_text(row.body, row.n_body);
                assert_eq!(body.text, "hello there");
                // The default-colored run adds no span; the colored one does.
                assert_eq!(body.spans.len(), 1);
                assert_eq!(body.spans[0].range, 6..11);
            });
            rotulus_message_free(copy);
        }
    }

    #[test]
    fn an_unknown_kind_is_a_message() {
        let m = rotulus_message_new(99);
        unsafe {
            assert_eq!((*m).kind, ROW_MESSAGE);
            rotulus_message_free(m);
        }
    }

    #[cfg(feature = "mirc")]
    #[test]
    fn mirc_codes_become_runs() {
        let m = rotulus_message_new(ROW_MESSAGE);
        unsafe {
            crate::mirc_ffi::rotulus_message_add_mirc(m, c"a \x02bold\x02 \x034red".as_ptr());
            let texts: Vec<&str> = (*m).body.iter().map(|r| r.text.as_str()).collect();
            assert_eq!(texts.concat(), "a bold red");
            let bold = (*m).body.iter().find(|r| r.text == "bold").unwrap();
            assert_ne!(bold.attrs & ATTR_BOLD, 0);
            let red = (*m).body.iter().find(|r| r.text == "red").unwrap();
            assert_eq!(red.color, 4);
            rotulus_message_free(m);
        }
    }
}
