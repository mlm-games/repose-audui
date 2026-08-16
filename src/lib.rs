//! Embed [Repose](https://github.com/mlm-games/repose) UI inside DAW plugin windows.

#![allow(clippy::type_complexity)]

pub mod embed;
pub mod input;
pub mod param_widgets;
pub mod render;
pub mod rwh_compat;

#[cfg(feature = "baseview")]
pub mod baseview_host;
#[cfg(feature = "baseview")]
pub mod clipboard;
#[cfg(feature = "baseview")]
pub mod cursor;

#[cfg(feature = "nih-plug")]
pub mod nih_plug;

pub use embed::ReposeEmbed;

pub mod prelude {
    pub use crate::embed::ReposeEmbed;
    pub use crate::input::{logical_to_physical, map_pointer_button_index};
    pub use crate::param_widgets::{panel, section_title};
    pub use crate::rwh_compat::RawHandles;
    pub use repose_app::{FrameOutput, PlatformOutput, ReposeRuntime};
    pub use repose_core::input::{ImeEvent, Key, KeyEvent, KeyEventType, Modifiers, PointerButton};
    pub use repose_core::{RenderContext, Scheduler, Vec2, View};
    pub use repose_ui;

    #[cfg(feature = "baseview")]
    pub use crate::baseview_host::{
        ReposeBaseviewConfig, ReposeWindowHandler, open_blocking, open_parented,
    };
    #[cfg(feature = "baseview")]
    pub use repose_core::cursor::CursorIcon;

    #[cfg(feature = "nih-plug")]
    pub use crate::nih_plug::{ReposeEditorState, create_repose_editor};
}

#[cfg(feature = "nih-plug")]
pub use nih_plug::{ReposeEditorState, create_repose_editor};
