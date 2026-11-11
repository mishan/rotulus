//! `RotulusReactions`: a row's reactions, built a call at a time.
//!
//! A builder rather than an array of structs, so that C and the language
//! bindings share one path, and so that a kind of reaction added later (a
//! custom emoji's image) is a new call rather than a change to a struct
//! applications build on the stack.

use crate::ffi::{ptr_to_mark, view_of, CGtkWidget};
use gtk4::glib;
use gtk4::prelude::*;
use rotulus_layout::Reaction;
use std::ffi::{c_char, c_int, c_uint, c_void, CStr};

#[derive(Clone, Debug, Default, glib::Boxed)]
#[boxed_type(name = "RotulusReactions")]
pub struct RotulusReactions(Vec<Reaction>);

#[no_mangle]
pub extern "C" fn rotulus_reactions_get_type() -> glib::ffi::GType {
    use glib::translate::IntoGlib;
    RotulusReactions::static_type().into_glib()
}

#[no_mangle]
pub extern "C" fn rotulus_reactions_new() -> *mut RotulusReactions {
    Box::into_raw(Box::default())
}

/// # Safety
/// `r` is NULL or a valid `RotulusReactions *`.
#[no_mangle]
pub unsafe extern "C" fn rotulus_reactions_copy(
    r: *const RotulusReactions,
) -> *mut RotulusReactions {
    match r.as_ref() {
        Some(r) => Box::into_raw(Box::new(r.clone())),
        None => std::ptr::null_mut(),
    }
}

/// # Safety
/// `r` is NULL or a `RotulusReactions *` the caller owns, and doesn't use
/// again.
#[no_mangle]
pub unsafe extern "C" fn rotulus_reactions_free(r: *mut RotulusReactions) {
    if !r.is_null() {
        drop(Box::from_raw(r));
    }
}

/// # Safety
/// `r` is NULL or a valid `RotulusReactions *`; `key` and `label` are NULL
/// or NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn rotulus_reactions_add(
    r: *mut RotulusReactions,
    key: *const c_char,
    count: c_uint,
    mine: c_int,
    label: *const c_char,
) {
    rotulus_reactions_add_full(r, key, std::ptr::null(), count, mine, label);
}

/// # Safety
/// `r` is NULL or a valid `RotulusReactions *`; `key`, `text` and `label`
/// are NULL or NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn rotulus_reactions_add_full(
    r: *mut RotulusReactions,
    key: *const c_char,
    text: *const c_char,
    count: c_uint,
    mine: c_int,
    label: *const c_char,
) {
    let (Some(r), false) = (r.as_mut(), key.is_null()) else {
        return;
    };
    // The key comes back in reaction-activated as the reaction's identity,
    // so one that isn't UTF-8 is left out rather than changed.
    let Ok(key) = CStr::from_ptr(key).to_str() else {
        return;
    };
    let lossy =
        |p: *const c_char| (!p.is_null()).then(|| CStr::from_ptr(p).to_string_lossy().into_owned());
    r.0.push(Reaction {
        key: key.to_owned(),
        text: lossy(text),
        count,
        mine: mine != 0,
        label: lossy(label),
    });
}

/// # Safety
/// `w` is a valid `RotulusView *`; `mark` is a mark or NULL; `r` is NULL
/// or a valid `RotulusReactions *`.
#[no_mangle]
pub unsafe extern "C" fn rotulus_view_set_reactions(
    w: CGtkWidget,
    mark: *mut c_void,
    r: *const RotulusReactions,
) -> c_int {
    let reactions = r.as_ref().map(|r| r.0.clone()).unwrap_or_default();
    match (view_of(w), ptr_to_mark(mark)) {
        (Some(v), Some(id)) => c_int::from(v.set_reactions(id, reactions)),
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reactions_are_built_in_order_and_a_key_that_is_null_or_not_utf8_is_skipped() {
        let r = rotulus_reactions_new();
        unsafe {
            rotulus_reactions_add(r, c"+1".as_ptr(), 3, 1, c"thumbs up".as_ptr());
            rotulus_reactions_add(r, std::ptr::null(), 1, 0, std::ptr::null());
            rotulus_reactions_add(r, c"\xff".as_ptr(), 1, 0, std::ptr::null());
            rotulus_reactions_add(r, c"ok".as_ptr(), 1, 0, std::ptr::null());
            rotulus_reactions_add_full(r, c"e:1".as_ptr(), c"?".as_ptr(), 2, 0, std::ptr::null());
            let copy = rotulus_reactions_copy(r);
            rotulus_reactions_free(r);
            let got = &copy.as_ref().expect("a copy").0;
            assert_eq!(
                got.iter()
                    .map(|r| (
                        r.key.as_str(),
                        r.text.as_deref(),
                        r.count,
                        r.mine,
                        r.label.as_deref()
                    ))
                    .collect::<Vec<_>>(),
                [
                    ("+1", None, 3, true, Some("thumbs up")),
                    ("ok", None, 1, false, None),
                    ("e:1", Some("?"), 2, false, None)
                ]
            );
            rotulus_reactions_free(copy);
        }
    }
}
