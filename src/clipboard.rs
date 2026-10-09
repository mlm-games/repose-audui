//! Clipboard bridge: `repose_core::clipboard` hooks backed by `clipawl`
//! (the same backend as `repose-platform`).

pub fn install_clipboard_hooks() {
    repose_core::clipboard::set_clipboard_fn(Box::new(|text| {
        let _ = clipawl::blocking::write(text);
    }));
    repose_core::clipboard::set_clipboard_read_fn(Box::new(|| clipawl::blocking::read().ok()));
    repose_core::clipboard::set_primary_fn(Box::new(|text| {
        let mut opts = clipawl::ClipboardOptions::default();
        opts.linux.selection = clipawl::LinuxSelection::Primary;
        match clipawl::Clipboard::new_with_options(opts) {
            Ok(cb) => {
                if let Err(e) = pollster::block_on(cb.write(text)) {
                    log::warn!("repose-audui: primary selection write failed: {e}");
                }
            }
            Err(e) => log::warn!("repose-audui: primary clipboard init failed: {e}"),
        }
    }));
}
