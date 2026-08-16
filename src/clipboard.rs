//! Clipboard bridge: `repose_core::clipboard` hooks backed by `clipawl`
//! (the same backend as `repose-platform`).

pub fn install_clipboard_hooks() {
    repose_core::clipboard::set_clipboard_fn(Box::new(|text| {
        let _ = clipawl::blocking::write(text);
    }));
    repose_core::clipboard::set_clipboard_read_fn(Box::new(|| clipawl::blocking::read().ok()));
}
