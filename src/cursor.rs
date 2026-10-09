//! Cursor icon maps from Repose `CursorIcon` to baseview `MouseCursor`.

use baseview::MouseCursor;
use repose_core::cursor::CursorIcon;

pub fn map_cursor(icon: CursorIcon) -> MouseCursor {
    match icon {
        CursorIcon::Default => MouseCursor::Default,
        CursorIcon::Pointer => MouseCursor::Hand,
        CursorIcon::Text => MouseCursor::Text,
        CursorIcon::EwResize => MouseCursor::EwResize,
        CursorIcon::NsResize => MouseCursor::NsResize,
        CursorIcon::NwseResize => MouseCursor::NwseResize,
        CursorIcon::NeswResize => MouseCursor::NeswResize,
        CursorIcon::Grab => MouseCursor::Hand,
        CursorIcon::Grabbing => MouseCursor::HandGrabbing,
        CursorIcon::Hidden => MouseCursor::Hidden,
        // baseview has no custom-image cursor.
        CursorIcon::Custom(_) => MouseCursor::Default,
    }
}
