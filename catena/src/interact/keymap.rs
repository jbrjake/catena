//! The key table (plan §10.1): which [`Action`] each key asks for. It lives in the core, so a
//! host without the widget crate gets the defaults, and it is an ordered list, so listing it
//! (for a help screen) is deterministic.

use super::input::{Action, Dir, KeyCode, KeyInput, Modifiers};

/// The key table, in binding order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keymap {
    bindings: Vec<(KeyInput, Action)>,
}

impl Default for Keymap {
    /// Plan §10.1's table: `hjkl` and the arrows pan, `+` (or `=`) and `-` zoom, `n` and `N`
    /// cycle through the nodes, Shift with an arrow moves to the nearest node that way, Enter
    /// pins, Esc clears, and `b` turns bundling off or on (owner ruling A4).
    fn default() -> Self {
        let key = |c: char| KeyInput::new(KeyCode::Char(c), Modifiers::NONE);
        let plain = |code| KeyInput::new(code, Modifiers::NONE);
        let shifted = |code| KeyInput::new(code, Modifiers::SHIFT);
        let mut bindings = vec![
            (key('h'), Action::Pan(Dir::Left)),
            (key('j'), Action::Pan(Dir::Down)),
            (key('k'), Action::Pan(Dir::Up)),
            (key('l'), Action::Pan(Dir::Right)),
        ];
        for (code, dir) in [
            (KeyCode::Left, Dir::Left),
            (KeyCode::Down, Dir::Down),
            (KeyCode::Up, Dir::Up),
            (KeyCode::Right, Dir::Right),
        ] {
            bindings.push((plain(code), Action::Pan(dir)));
        }
        bindings.extend([
            (key('+'), Action::ZoomIn),
            (key('='), Action::ZoomIn),
            (key('-'), Action::ZoomOut),
            (key('n'), Action::CycleNext),
            (key('N'), Action::CyclePrev),
        ]);
        for (code, dir) in [
            (KeyCode::Left, Dir::Left),
            (KeyCode::Down, Dir::Down),
            (KeyCode::Up, Dir::Up),
            (KeyCode::Right, Dir::Right),
        ] {
            bindings.push((shifted(code), Action::NavTo(dir)));
        }
        bindings.extend([
            (plain(KeyCode::Enter), Action::TogglePin),
            (plain(KeyCode::Esc), Action::Clear),
            (key('b'), Action::ToggleBundling),
        ]);
        Keymap { bindings }
    }
}

impl Keymap {
    /// A table that binds nothing.
    #[must_use]
    pub fn empty() -> Self {
        Keymap {
            bindings: Vec::new(),
        }
    }

    /// The action `key` asks for, if it is bound.
    #[must_use]
    pub fn action(&self, key: KeyInput) -> Option<Action> {
        let key = KeyInput::new(key.code, key.mods);
        self.bindings
            .iter()
            .find(|&&(bound, _)| bound == key)
            .map(|&(_, action)| action)
    }

    /// Binds `key` to `action`: in its place if it was bound, else last.
    pub fn bind(&mut self, key: KeyInput, action: Action) {
        let key = KeyInput::new(key.code, key.mods);
        match self.bindings.iter_mut().find(|(bound, _)| *bound == key) {
            Some(binding) => binding.1 = action,
            None => self.bindings.push((key, action)),
        }
    }

    /// Unbinds `key`, returning the action it had.
    pub fn unbind(&mut self, key: KeyInput) -> Option<Action> {
        let key = KeyInput::new(key.code, key.mods);
        let at = self.bindings.iter().position(|&(bound, _)| bound == key)?;
        Some(self.bindings.remove(at).1)
    }

    /// Every binding, in order.
    #[must_use]
    pub fn bindings(&self) -> &[(KeyInput, Action)] {
        &self.bindings
    }
}

#[cfg(test)]
#[path = "keymap_tests.rs"]
mod tests;
