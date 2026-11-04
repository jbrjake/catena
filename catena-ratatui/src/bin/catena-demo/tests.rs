//! The demo without a terminal: its frames through `TestBackend`, and its keys as crossterm
//! events (plan §19 M3: every binding live, `NO_COLOR`).

use catena::interact::{KeyCode as Code, KeyInput, Keymap, Modifiers};
use crossterm::event::KeyEvent;
use ratatui_core::backend::TestBackend;

use super::*;

/// The frame `app` draws into a 100 × 30 terminal.
fn frame(app: &mut App) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).expect("infallible");
    terminal.draw(|frame| app.draw(frame)).expect("infallible");
    terminal.backend().buffer().clone()
}

/// Runs the glides out.
fn settle(app: &mut App) {
    for _ in 0..40 {
        app.tick(Duration::from_millis(16));
    }
}

/// The crossterm press a terminal sends for `key`.
fn press(key: KeyInput) -> Event {
    let code = match key.code {
        Code::Char(c) => KeyCode::Char(c),
        Code::Up => KeyCode::Up,
        Code::Down => KeyCode::Down,
        Code::Left => KeyCode::Left,
        Code::Right => KeyCode::Right,
        Code::Enter => KeyCode::Enter,
        Code::Esc => KeyCode::Esc,
        Code::Tab => KeyCode::Tab,
        other => panic!("the default table binds no {other:?}"),
    };
    let mods = [
        (Modifiers::SHIFT, KeyModifiers::SHIFT),
        (Modifiers::CTRL, KeyModifiers::CONTROL),
        (Modifiers::ALT, KeyModifiers::ALT),
    ]
    .into_iter()
    .filter(|&(m, _)| key.mods.contains(m))
    .fold(KeyModifiers::NONE, |all, (_, held)| all | held);
    Event::Key(KeyEvent::new(code, mods))
}

fn key(c: char) -> Event {
    Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE))
}

#[test]
fn every_binding_of_the_default_table_changes_the_frame() {
    // Plan §19 M3: every binding live. Each one starts from the same state, a node selected
    // and the glide run out, and must change what the demo draws.
    let bindings = Keymap::default().bindings().to_vec();
    assert!(bindings.len() >= 15, "{bindings:?}");
    for (input, action) in bindings {
        let mut app = App::new(false);
        app.event(&key('n'));
        settle(&mut app);
        let before = frame(&mut app);
        assert_eq!(app.event(&press(input)), Flow::Continue);
        settle(&mut app);
        assert_ne!(
            frame(&mut app),
            before,
            "{input:?} ({action:?}) changed nothing"
        );
    }
}

#[test]
fn the_status_line_names_what_happened() {
    let mut app = App::new(false);
    let text = |app: &mut App| {
        let buf = frame(app);
        (0..buf.area.width)
            .map(|x| buf[(x, buf.area.height - 1)].symbol().to_string())
            .collect::<String>()
    };
    app.event(&key('n'));
    let first = &community().nodes[0];
    let line = text(&mut app);
    assert!(line.starts_with(" community | selected "), "{line}");
    assert!(
        community()
            .nodes
            .iter()
            .any(|n| line.contains(&format!("selected {} |", n.label))),
        "{line} names a label, not a key such as {}",
        first.key
    );
    app.event(&key('b'));
    assert!(text(&mut app).contains("bundling off"));
}

#[test]
fn tab_switches_graphs_and_q_quits() {
    let mut app = App::new(false);
    let first = frame(&mut app);
    assert_eq!(
        app.event(&Event::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE))),
        Flow::Continue
    );
    assert_eq!(app.current, 1);
    assert_ne!(frame(&mut app), first);
    assert_eq!(app.event(&key('q')), Flow::Quit);
    let ctrl_c = Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert_eq!(app.event(&ctrl_c), Flow::Quit);
    assert_eq!(app.event(&key('c')), Flow::Continue, "c alone is no quit");
}

#[test]
fn no_color_draws_plain_ascii_in_the_terminals_own_colors() {
    let cells = |buf: &Buffer| {
        (0..buf.area.height)
            .flat_map(|y| (0..buf.area.width).map(move |x| (x, y)))
            .map(|at| buf[at].clone())
            .collect::<Vec<_>>()
    };
    let colored = cells(&frame(&mut App::new(false)));
    assert!(colored.iter().any(|c| c.fg != Color::Reset), "it colors");
    assert!(
        colored.iter().any(|c| !c.symbol().is_ascii()),
        "it draws braille"
    );
    let mut plain = App::new(true);
    plain.event(&key('n'));
    let drawn = cells(&frame(&mut plain));
    assert!(drawn.iter().any(|c| c.symbol() != " "), "it draws");
    for cell in &drawn {
        assert!(cell.symbol().is_ascii(), "{:?}", cell.symbol());
        assert_eq!((cell.fg, cell.bg), (Color::Reset, Color::Reset));
    }
    assert!(
        drawn
            .iter()
            .any(|c| c.modifier.contains(Modifier::REVERSED)),
        "the selection and the status line keep their attribute"
    );
}
