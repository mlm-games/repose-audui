//! Input mapping helpers (baseview / generic).

use repose_core::Vec2;
use repose_core::input::PointerButton;

/// Convert logical coordinates to physical device pixels for `ReposeRuntime`.
#[inline]
pub fn logical_to_physical(x: f64, y: f64, scale: f32) -> Vec2 {
    let s = scale as f64;
    Vec2 {
        x: (x * s) as f32,
        y: (y * s) as f32,
    }
}

/// Map a bare button index (0=primary, 1=tertiary/middle, 2=secondary/right).
#[inline]
pub fn map_pointer_button_index(button: u8) -> Option<PointerButton> {
    match button {
        0 => Some(PointerButton::Primary),
        1 => Some(PointerButton::Tertiary),
        2 => Some(PointerButton::Secondary),
        _ => None,
    }
}

/// Whether the UI should consume keyboard events.
///
/// Default is [`KeyCapture::CaptureAll`]. `IgnoreAll` / `CaptureKeys` /
/// `IgnoreKeys` let specific keys (e.g. space, numbers) reach the DAW host.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum KeyCapture {
    #[default]
    CaptureAll,
    IgnoreAll,
    CaptureKeys(Vec<repose_core::input::Key>),
    IgnoreKeys(Vec<repose_core::input::Key>),
}

impl KeyCapture {
    pub fn allows(&self, key: &repose_core::input::Key) -> bool {
        match self {
            Self::CaptureAll => true,
            Self::IgnoreAll => false,
            Self::CaptureKeys(keys) => keys.iter().any(|k| k == key),
            Self::IgnoreKeys(keys) => !keys.iter().any(|k| k == key),
        }
    }
}

#[cfg(feature = "baseview")]
pub mod baseview_map {
    use super::KeyCapture;
    use crate::embed::ReposeEmbed;
    use baseview::{Event, EventStatus, MouseButton, MouseEvent, ScrollDelta, WindowEvent};
    use keyboard_types::{Key as KbKey, KeyState, KeyboardEvent, Modifiers as KbMods};
    use repose_core::Vec2;
    use repose_core::input::{Key, KeyEvent, KeyEventType, Modifiers, PointerButton};

    pub fn map_modifiers(m: KbMods) -> Modifiers {
        let shift = m.contains(KbMods::SHIFT);
        let ctrl = m.contains(KbMods::CONTROL);
        let alt = m.contains(KbMods::ALT);
        let meta = m.contains(KbMods::META) || m.contains(KbMods::SUPER);
        let command = if cfg!(target_os = "macos") {
            meta
        } else {
            ctrl
        };
        Modifiers {
            shift,
            ctrl,
            alt,
            meta,
            command,
        }
    }

    pub fn map_key(key: &KbKey) -> Key {
        use KbKey as K;
        match key {
            K::Enter => Key::Enter,
            K::Tab => Key::Tab,
            K::Backspace => Key::Backspace,
            K::Delete => Key::Delete,
            K::Escape => Key::Escape,
            K::ArrowLeft => Key::ArrowLeft,
            K::ArrowRight => Key::ArrowRight,
            K::ArrowUp => Key::ArrowUp,
            K::ArrowDown => Key::ArrowDown,
            K::Home => Key::Home,
            K::End => Key::End,
            K::PageUp => Key::PageUp,
            K::PageDown => Key::PageDown,
            K::F1 => Key::F(1),
            K::F2 => Key::F(2),
            K::F3 => Key::F(3),
            K::F4 => Key::F(4),
            K::F5 => Key::F(5),
            K::F6 => Key::F(6),
            K::F7 => Key::F(7),
            K::F8 => Key::F(8),
            K::F9 => Key::F(9),
            K::F10 => Key::F(10),
            K::F11 => Key::F(11),
            K::F12 => Key::F(12),
            K::Character(s) => {
                if s == " " {
                    Key::Space
                } else {
                    let ch = s.chars().next().unwrap_or('\0');
                    Key::Character(ch)
                }
            }
            _ => Key::Unknown,
        }
    }

    pub fn map_mouse_button(b: MouseButton) -> Option<PointerButton> {
        match b {
            MouseButton::Left => Some(PointerButton::Primary),
            MouseButton::Right => Some(PointerButton::Secondary),
            MouseButton::Middle => Some(PointerButton::Tertiary),
            _ => None,
        }
    }

    /// Dispatch a keyboard event; returns `(consumed, captured_allowed)`.
    #[allow(clippy::type_complexity)]
    fn dispatch_keyboard(
        embed: &mut ReposeEmbed,
        ke: KeyboardEvent,
        key_capture: &KeyCapture,
        fall_through_unhandled: bool,
    ) -> EventStatus {
        let mods = map_modifiers(ke.modifiers);
        embed.set_modifiers(mods);

        let key = map_key(&ke.key);

        let ui_wants_keys = embed
            .last_frame()
            .map(|f| f.wants_keyboard)
            .unwrap_or(false);
        if fall_through_unhandled && !ui_wants_keys {
            // Modifiers were already updated so the next click sees them.
            return EventStatus::Ignored;
        }

        if !key_capture.allows(&key) {
            return EventStatus::Ignored;
        }

        let event_type = if ke.state == KeyState::Down {
            KeyEventType::Down
        } else {
            KeyEventType::Up
        };

        let utf16 = match &key {
            Key::Character(c) => *c as u16,
            Key::Space => b' ' as u16,
            _ => 0,
        };

        let ev = KeyEvent {
            key: key.clone(),
            modifiers: mods,
            is_repeat: ke.repeat,
            event_type,
            utf16_code_point: utf16,
        };

        // Composed text on key-down, unless a shortcut modifier is held (avoids
        // pasting the 'v' of Ctrl/Cmd+V into text fields).
        let modifier_hotkey = mods.ctrl || mods.meta;
        let composed = if event_type == KeyEventType::Down && !modifier_hotkey {
            match &ke.key {
                KbKey::Character(s) if !s.is_empty() => Some(s.as_str()),
                _ => None,
            }
        } else {
            None
        };

        // Restore modifiers as they were for this event.
        embed.set_modifiers(mods);
        let consumed = embed.key_with_text(&ev, composed);

        if consumed || !fall_through_unhandled || ui_wants_keys {
            EventStatus::Captured
        } else {
            EventStatus::Ignored
        }
    }

    fn mouse_button(b: MouseButton) -> Option<PointerButton> {
        map_mouse_button(b)
    }

    /// Dispatch a baseview event into the embed.
    pub fn dispatch(
        embed: &mut ReposeEmbed,
        event: Event,
        last_cursor_logical: &mut (f64, f64),
        key_capture: &KeyCapture,
        fall_through_unhandled: bool,
    ) -> EventStatus {
        match event {
            Event::Mouse(me) => match me {
                MouseEvent::CursorMoved {
                    position,
                    modifiers,
                } => {
                    *last_cursor_logical = (position.x, position.y);
                    embed.set_modifiers(map_modifiers(modifiers));
                    embed.set_pointer_inside(true);
                    let pos = super::logical_to_physical(position.x, position.y, embed.scale);
                    embed.pointer_move(pos);
                    EventStatus::Captured
                }
                MouseEvent::ButtonPressed { button, modifiers } => {
                    embed.set_modifiers(map_modifiers(modifiers));
                    if let Some(b) = mouse_button(button) {
                        let (x, y) = *last_cursor_logical;
                        embed.pointer_down(super::logical_to_physical(x, y, embed.scale), b);
                    }
                    EventStatus::Captured
                }
                MouseEvent::ButtonReleased { button, modifiers } => {
                    embed.set_modifiers(map_modifiers(modifiers));
                    if let Some(b) = mouse_button(button) {
                        let (x, y) = *last_cursor_logical;
                        embed.pointer_up(super::logical_to_physical(x, y, embed.scale), b);
                    }
                    EventStatus::Captured
                }
                MouseEvent::WheelScrolled { delta, modifiers } => {
                    embed.set_modifiers(map_modifiers(modifiers));
                    let (dx, dy) = match delta {
                        ScrollDelta::Lines { x, y } => {
                            let unit = repose_core::locals::dp_to_px(60.0);
                            (-x * unit, -y * unit)
                        }
                        ScrollDelta::Pixels { x, y } => (-x, -y),
                    };
                    embed.scroll(Vec2 { x: dx, y: dy });
                    EventStatus::Captured
                }
                MouseEvent::CursorLeft => {
                    embed.set_pointer_inside(false);
                    embed.clear_hover();
                    EventStatus::Captured
                }
                MouseEvent::CursorEntered => {
                    embed.set_pointer_inside(true);
                    EventStatus::Captured
                }
                _ => EventStatus::Ignored,
            },
            Event::Keyboard(ke) => {
                dispatch_keyboard(embed, ke, key_capture, fall_through_unhandled)
            }
            Event::Window(WindowEvent::Unfocused) => {
                embed.set_modifiers(Modifiers::default());
                embed.focus_lost();
                EventStatus::Captured
            }
            Event::Window(
                WindowEvent::Resized(_) | WindowEvent::Focused | WindowEvent::WillClose,
            ) => EventStatus::Ignored,
        }
    }
}
