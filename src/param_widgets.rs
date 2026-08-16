//! Small layout helpers for plugin UIs (toolkit-agnostic).

use repose_core::{Modifier, View};
use repose_ui::{Column, DragValue, Row, Text, TextStyle, ViewExt};

/// Label + DragValue row (good default for float params without material).
pub fn param_row(
    label: impl Into<String>,
    value: f32,
    range: (f32, f32),
    speed: f32,
    on_change: impl Fn(f32) + 'static,
) -> View {
    Row(Modifier::new().gap(12.0).fill_max_width())
        .child((
            Text(label.into()).size(14.0),
            DragValue(value, range, speed, on_change),
        ))
}

/// Label + clickable toggle using a simple box (or use material Switch in the plugin).
pub fn toggle_row(
    label: impl Into<String>,
    on: bool,
    on_toggle: impl Fn() + 'static,
) -> View {
    let label = label.into();
    Row(Modifier::new().gap(12.0).fill_max_width())
        .child((
            Text(label).size(14.0),
            repose_ui::Box(
                Modifier::new()
                    .width(48.0)
                    .height(28.0)
                    .clip_rounded(14.0)
                    .background(if on {
                        repose_core::Color::from_rgba(80, 160, 255, 255)
                    } else {
                        repose_core::Color::from_rgba(60, 60, 70, 255)
                    })
                    .on_click(on_toggle),
            )
            .child(Text(if on { "ON" } else { "OFF" }).size(12.0)),
        ))
}

/// Section title.
pub fn section_title(t: impl Into<String>) -> View {
    Text(t.into()).size(18.0)
}

/// Vertical stack with padding for a whole editor panel.
pub fn panel(children: impl repose_ui::IntoChildren) -> View {
    Column(
        Modifier::new()
            .padding(16.0)
            .gap(10.0)
            .fill_max_size()
            .background(repose_core::Color::from_rgba(28, 28, 34, 255)),
    )
    .child(children)
}