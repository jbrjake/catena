//! Terminal events as catena's (plan §10.1), behind the `crossterm` feature.
//!
//! A free function rather than a `From` impl: catena owns `InputEvent` and cannot name
//! crossterm, and this crate owns neither type, so the orphan rule allows nothing else. Any
//! other backend is one more such function.

use catena::interact::{Button, InputEvent, KeyCode, KeyInput, Modifiers};
use crossterm::event::{
    Event, KeyCode as Code, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};

/// The catena event a crossterm event stands for, or `None` for one catena has no use for: a
/// key release, a key catena has no code for, a sideways scroll, focus changes and pastes.
///
/// Positions stay terminal cells, which the view reads against where its widget was drawn. A
/// held key's repeats are presses, so holding `l` keeps panning; a drag is a move, which the
/// view tells from a hover by the press before it; the wheel turned away from the user is a
/// positive scroll, which zooms in.
#[must_use]
pub fn from_crossterm(event: &Event) -> Option<InputEvent> {
    match event {
        Event::Key(key) => key_input(key).map(InputEvent::Key),
        Event::Mouse(mouse) => mouse_input(*mouse),
        Event::Resize(cols, rows) => Some(InputEvent::Resize(*cols, *rows)),
        // Focus, and the `Paste` a host's own crossterm features can add.
        _ => None,
    }
}

fn key_input(key: &KeyEvent) -> Option<KeyInput> {
    if key.kind == KeyEventKind::Release {
        return None;
    }
    let code = match key.code {
        Code::Char(c) => KeyCode::Char(c),
        Code::Up => KeyCode::Up,
        Code::Down => KeyCode::Down,
        Code::Left => KeyCode::Left,
        Code::Right => KeyCode::Right,
        Code::Enter => KeyCode::Enter,
        Code::Esc => KeyCode::Esc,
        Code::Tab => KeyCode::Tab,
        Code::BackTab => KeyCode::BackTab,
        Code::Backspace => KeyCode::Backspace,
        Code::Home => KeyCode::Home,
        Code::End => KeyCode::End,
        Code::PageUp => KeyCode::PageUp,
        Code::PageDown => KeyCode::PageDown,
        _ => return None,
    };
    let mods = [
        (KeyModifiers::SHIFT, Modifiers::SHIFT),
        (KeyModifiers::CONTROL, Modifiers::CTRL),
        (KeyModifiers::ALT, Modifiers::ALT),
    ]
    .into_iter()
    .filter(|&(held, _)| key.modifiers.contains(held))
    .fold(Modifiers::NONE, |all, (_, m)| all | m);
    Some(KeyInput::new(code, mods))
}

fn mouse_input(mouse: MouseEvent) -> Option<InputEvent> {
    let (x, y) = (mouse.column, mouse.row);
    let button = |b: MouseButton| match b {
        MouseButton::Left => Button::Left,
        MouseButton::Right => Button::Right,
        MouseButton::Middle => Button::Middle,
    };
    Some(match mouse.kind {
        MouseEventKind::Down(b) => InputEvent::MouseDown(x, y, button(b)),
        MouseEventKind::Up(b) => InputEvent::MouseUp(x, y, button(b)),
        MouseEventKind::Drag(_) | MouseEventKind::Moved => InputEvent::MouseMove(x, y),
        MouseEventKind::ScrollUp => InputEvent::Scroll(1, x, y),
        MouseEventKind::ScrollDown => InputEvent::Scroll(-1, x, y),
        MouseEventKind::ScrollLeft | MouseEventKind::ScrollRight => return None,
    })
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
