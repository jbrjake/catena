//! What the fuzz targets share (plan §16.7): `Arbitrary` mirrors of catena's inputs, so that
//! catena itself carries no `arbitrary` feature or dependency, and the invariants each target
//! checks on what it drew.

use std::time::Duration;

use arbitrary::Arbitrary;
use catena::GraphView;
use catena::geometry::cell::{CellBox, CellPt};
use catena::interact::{Action, Button, Dir, InputEvent, KeyCode, KeyInput, Modifiers};
use catena::raster::CellGrid;
use catena::scene::Payload;

/// The largest frame a target draws: plan §16.2's viewport range tops out at 300 × 100.
pub const MAX_FRAME: (u16, u16) = (300, 100);

/// Every key the default table binds, and some it does not.
const KEYS: [KeyCode; 24] = [
    KeyCode::Char('h'),
    KeyCode::Char('j'),
    KeyCode::Char('k'),
    KeyCode::Char('l'),
    KeyCode::Char('+'),
    KeyCode::Char('='),
    KeyCode::Char('-'),
    KeyCode::Char('n'),
    KeyCode::Char('N'),
    KeyCode::Char('b'),
    KeyCode::Char('x'),
    KeyCode::Up,
    KeyCode::Down,
    KeyCode::Left,
    KeyCode::Right,
    KeyCode::Enter,
    KeyCode::Esc,
    KeyCode::Tab,
    KeyCode::BackTab,
    KeyCode::Backspace,
    KeyCode::Home,
    KeyCode::End,
    KeyCode::PageUp,
    KeyCode::PageDown,
];

const DIRS: [Dir; 4] = [Dir::Up, Dir::Down, Dir::Left, Dir::Right];

/// One thing a host can do to a view, as the fuzzer makes it.
#[derive(Arbitrary, Debug, Clone)]
pub enum Event {
    /// A key from `KEYS` by index, with modifier bits: Shift, Ctrl, Alt.
    Key(u8, u8),
    /// A button (left, right, middle by index) going down, anywhere a `u16` reaches.
    Down(u16, u16, u8),
    Up(u16, u16, u8),
    Move(u16, u16),
    Scroll(i8, u16, u16),
    /// A resize event, of any size; frames are drawn at most `MAX_FRAME`.
    Resize(u16, u16),
    /// An action by index, bypassing the key table.
    Act(u8),
    /// Time, in milliseconds.
    Tick(u16),
    /// A frame of this size, cut to `MAX_FRAME`.
    Render(u16, u16),
}

fn button(i: u8) -> Button {
    match i % 3 {
        0 => Button::Left,
        1 => Button::Right,
        _ => Button::Middle,
    }
}

fn action(i: u8) -> Action {
    let dir = DIRS[usize::from(i / 16) % 4];
    match i % 9 {
        0 => Action::Pan(dir),
        1 => Action::ZoomIn,
        2 => Action::ZoomOut,
        3 => Action::CycleNext,
        4 => Action::CyclePrev,
        5 => Action::NavTo(dir),
        6 => Action::TogglePin,
        7 => Action::Clear,
        _ => Action::ToggleBundling,
    }
}

/// The frame size `(cols, rows)` cut to `MAX_FRAME`.
#[must_use]
pub fn frame_size(cols: u16, rows: u16) -> (u16, u16) {
    (cols.min(MAX_FRAME.0), rows.min(MAX_FRAME.1))
}

/// Does `event` to `gv`; a render draws, and becomes the frame size to check at the end.
pub fn apply(gv: &mut GraphView<String>, event: &Event, size: &mut (u16, u16)) {
    match *event {
        Event::Key(code, mods) => {
            let mods = [Modifiers::SHIFT, Modifiers::CTRL, Modifiers::ALT]
                .into_iter()
                .enumerate()
                .filter(|(bit, _)| mods & (1 << bit) != 0)
                .fold(Modifiers::NONE, |all, (_, m)| all | m);
            let key = KeyInput::new(KEYS[usize::from(code) % KEYS.len()], mods);
            gv.handle(InputEvent::Key(key));
        }
        Event::Down(x, y, b) => {
            gv.handle(InputEvent::MouseDown(x, y, button(b)));
        }
        Event::Up(x, y, b) => {
            gv.handle(InputEvent::MouseUp(x, y, button(b)));
        }
        Event::Move(x, y) => {
            gv.handle(InputEvent::MouseMove(x, y));
        }
        Event::Scroll(n, x, y) => {
            gv.handle(InputEvent::Scroll(n, x, y));
        }
        Event::Resize(cols, rows) => {
            gv.handle(InputEvent::Resize(cols, rows));
        }
        Event::Act(i) => {
            gv.act(action(i));
        }
        Event::Tick(ms) => {
            gv.tick(Duration::from_millis(u64::from(ms)));
        }
        Event::Render(cols, rows) => {
            *size = frame_size(cols, rows);
            let mut grid = CellGrid::new(size.0, size.1);
            gv.render(&mut grid);
        }
    }
}

fn meet(a: CellBox, b: CellBox) -> bool {
    i64::from(a.x) < b.right()
        && i64::from(b.x) < a.right()
        && i64::from(a.y) < b.bottom()
        && i64::from(b.y) < a.bottom()
}

/// Draws `gv` at `size` and checks plan §16.2's invariants on the frame: A, no two node boxes on
/// the frame meet; B, every node and every edge is drawn once; C, every cell drawn lies in the
/// bounds of a scene item. Panics with the broken one.
pub fn check_frame(gv: &mut GraphView<String>, (cols, rows): (u16, u16)) {
    let mut grid = CellGrid::new(cols, rows);
    gv.render(&mut grid);
    let scene = gv.scene();
    let frame = CellBox::new(0, 0, u32::from(cols), u32::from(rows));
    let boxes: Vec<CellBox> = scene
        .items()
        .iter()
        .filter(|item| matches!(item.payload, Payload::NodeBox { .. }))
        .map(|item| item.bounds)
        .collect();
    let paths = scene
        .items()
        .iter()
        .filter(|item| matches!(item.payload, Payload::EdgePath { .. }))
        .count();
    assert_eq!(boxes.len(), gv.node_count(), "B: every node drawn once");
    assert_eq!(paths, gv.edge_count(), "B: every edge drawn once");
    let shown: Vec<CellBox> = boxes.into_iter().filter(|&b| meet(b, frame)).collect();
    for (i, &a) in shown.iter().enumerate() {
        for &b in &shown[i + 1..] {
            assert!(!meet(a, b), "A: {a:?} meets {b:?} at {cols} × {rows}");
        }
    }
    for y in 0..rows {
        for x in 0..cols {
            let lit = grid.cell(x, y).is_some_and(|c| c.symbol() != " ");
            let at = CellPt::new(i32::from(x), i32::from(y));
            assert!(
                !lit || scene.items().iter().any(|item| item.bounds.contains(at)),
                "C: ({x}, {y}) traces to no scene item"
            );
        }
    }
}
