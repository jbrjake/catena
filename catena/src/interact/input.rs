//! The library's own input events (plan §10.1). Terminal crates convert into these; the core
//! never names a terminal library's types, so a host on any backend speaks the same events.

/// A key, as the controller sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum KeyCode {
    /// A character key; its case carries Shift.
    Char(char),
    Up,
    Down,
    Left,
    Right,
    Enter,
    Esc,
    Tab,
    BackTab,
    Backspace,
    Home,
    End,
    PageUp,
    PageDown,
}

/// Modifier keys held with a key, as flags.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Modifiers(u8);

impl Modifiers {
    /// No modifier.
    pub const NONE: Modifiers = Modifiers(0);
    /// Shift.
    pub const SHIFT: Modifiers = Modifiers(1);
    /// Control.
    pub const CTRL: Modifiers = Modifiers(1 << 1);
    /// Alt, or Option.
    pub const ALT: Modifiers = Modifiers(1 << 2);

    /// Whether every flag of `other` is set here.
    #[must_use]
    pub const fn contains(self, other: Modifiers) -> bool {
        self.0 & other.0 == other.0
    }

    /// These flags less those of `other`.
    #[must_use]
    pub const fn without(self, other: Modifiers) -> Modifiers {
        Modifiers(self.0 & !other.0)
    }

    /// Whether no flag is set.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl std::ops::BitOr for Modifiers {
    type Output = Modifiers;

    fn bitor(self, other: Modifiers) -> Modifiers {
        Modifiers(self.0 | other.0)
    }
}

/// A key with its modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeyInput {
    pub code: KeyCode,
    pub mods: Modifiers,
}

impl KeyInput {
    /// `code` held with `mods`. A character key drops Shift: the character's case already
    /// carries it, and terminals disagree on whether they report Shift with `N`.
    #[must_use]
    pub const fn new(code: KeyCode, mods: Modifiers) -> Self {
        let mods = match code {
            KeyCode::Char(_) => mods.without(Modifiers::SHIFT),
            _ => mods,
        };
        KeyInput { code, mods }
    }
}

/// A mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Button {
    Left,
    Right,
    Middle,
}

/// A direction on the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

/// One input event (plan §10.1). Positions are cells of the surface the view draws into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum InputEvent {
    /// A key press.
    Key(KeyInput),
    /// A button went down at `(column, row)`.
    MouseDown(u16, u16, Button),
    /// A button came up at `(column, row)`.
    MouseUp(u16, u16, Button),
    /// The pointer moved to `(column, row)`, with or without a button held: a drag is these
    /// between a `MouseDown` and its `MouseUp`.
    MouseMove(u16, u16),
    /// The wheel turned by `notches` (positive away from the user, which zooms in) at
    /// `(column, row)`.
    Scroll(i8, u16, u16),
    /// The surface is now `(columns, rows)`.
    Resize(u16, u16),
}

/// What the controller can be asked to do, whatever key or button asks (plan §10.1). A host
/// with its own key handling calls these directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Action {
    /// Move the view one step in a direction.
    Pan(Dir),
    /// Zoom in one keyboard step about the view's center.
    ZoomIn,
    /// Zoom out one keyboard step about the view's center.
    ZoomOut,
    /// Select the next node in sort order (plan §10.3's list cycling).
    CycleNext,
    /// Select the previous node in sort order.
    CyclePrev,
    /// Select the nearest node in a direction (plan §10.3's spatial navigation).
    NavTo(Dir),
    /// Pin the selected node where it is, or unpin it.
    TogglePin,
    /// Clear the selection.
    Clear,
    /// Turn edge bundling off or back on (owner ruling A4: one key sets β = 0).
    ToggleBundling,
}
