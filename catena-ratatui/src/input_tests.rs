use catena::interact::{Button, InputEvent, KeyCode, KeyInput, Modifiers};
use crossterm::event::{
    Event, KeyCode as Code, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};

use super::*;

fn key(code: Code, modifiers: KeyModifiers) -> Event {
    Event::Key(KeyEvent::new(code, modifiers))
}

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

fn pressed(code: KeyCode, mods: Modifiers) -> InputEvent {
    InputEvent::Key(KeyInput::new(code, mods))
}

#[test]
fn keys_catena_has_a_code_for_convert_with_their_modifiers() {
    let none = KeyModifiers::NONE;
    for (from, to) in [
        (Code::Char('n'), KeyCode::Char('n')),
        (Code::Up, KeyCode::Up),
        (Code::Down, KeyCode::Down),
        (Code::Left, KeyCode::Left),
        (Code::Right, KeyCode::Right),
        (Code::Enter, KeyCode::Enter),
        (Code::Esc, KeyCode::Esc),
        (Code::Tab, KeyCode::Tab),
        (Code::BackTab, KeyCode::BackTab),
        (Code::Backspace, KeyCode::Backspace),
        (Code::Home, KeyCode::Home),
        (Code::End, KeyCode::End),
        (Code::PageUp, KeyCode::PageUp),
        (Code::PageDown, KeyCode::PageDown),
    ] {
        assert_eq!(
            from_crossterm(&key(from, none)),
            Some(pressed(to, Modifiers::NONE)),
            "{from:?}"
        );
    }
    assert_eq!(
        from_crossterm(&key(Code::Left, KeyModifiers::SHIFT)),
        Some(pressed(KeyCode::Left, Modifiers::SHIFT)),
        "Shift+arrow navigates (plan §10.1)"
    );
    assert_eq!(
        from_crossterm(&key(
            Code::Char('x'),
            KeyModifiers::CONTROL | KeyModifiers::ALT
        )),
        Some(pressed(
            KeyCode::Char('x'),
            Modifiers::CTRL | Modifiers::ALT
        ))
    );
    assert_eq!(
        from_crossterm(&key(Code::Char('N'), KeyModifiers::SHIFT)),
        Some(pressed(KeyCode::Char('N'), Modifiers::NONE)),
        "a character's case carries Shift"
    );
    assert_eq!(
        from_crossterm(&key(Code::BackTab, KeyModifiers::SHIFT)),
        Some(pressed(KeyCode::BackTab, Modifiers::NONE))
    );
}

#[test]
fn a_held_key_repeats_and_a_release_is_nothing() {
    let mut held = KeyEvent::new(Code::Char('l'), KeyModifiers::NONE);
    held.kind = KeyEventKind::Repeat;
    assert_eq!(
        from_crossterm(&Event::Key(held)),
        Some(pressed(KeyCode::Char('l'), Modifiers::NONE))
    );
    held.kind = KeyEventKind::Release;
    assert_eq!(from_crossterm(&Event::Key(held)), None);
}

#[test]
fn keys_and_events_catena_has_no_use_for_are_none() {
    for code in [Code::F(1), Code::Delete, Code::Insert, Code::Null] {
        assert_eq!(
            from_crossterm(&key(code, KeyModifiers::NONE)),
            None,
            "{code:?}"
        );
    }
    assert_eq!(from_crossterm(&Event::FocusGained), None);
    assert_eq!(from_crossterm(&Event::FocusLost), None);
    assert_eq!(
        from_crossterm(&mouse(MouseEventKind::ScrollLeft, 3, 4)),
        None
    );
}

#[test]
fn the_pointer_converts_in_terminal_cells_and_a_drag_is_a_move() {
    for (from, to) in [
        (
            MouseEventKind::Down(MouseButton::Left),
            InputEvent::MouseDown(7, 3, Button::Left),
        ),
        (
            MouseEventKind::Up(MouseButton::Right),
            InputEvent::MouseUp(7, 3, Button::Right),
        ),
        (
            MouseEventKind::Down(MouseButton::Middle),
            InputEvent::MouseDown(7, 3, Button::Middle),
        ),
        (
            MouseEventKind::Drag(MouseButton::Left),
            InputEvent::MouseMove(7, 3),
        ),
        (MouseEventKind::Moved, InputEvent::MouseMove(7, 3)),
        (MouseEventKind::ScrollUp, InputEvent::Scroll(1, 7, 3)),
        (MouseEventKind::ScrollDown, InputEvent::Scroll(-1, 7, 3)),
    ] {
        assert_eq!(from_crossterm(&mouse(from, 7, 3)), Some(to), "{from:?}");
    }
    assert_eq!(
        from_crossterm(&Event::Resize(132, 43)),
        Some(InputEvent::Resize(132, 43))
    );
}
