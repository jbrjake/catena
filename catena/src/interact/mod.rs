//! Interaction (plan §10): the library's input events, the actions they ask for, and the key
//! table between them. The controller that acts on them, with hit-testing, spatial navigation
//! and animations over `tick(dt)`, joins in later M3 steps.

mod input;
mod keymap;

pub use input::{Action, Button, Dir, InputEvent, KeyCode, KeyInput, Modifiers};
pub use keymap::Keymap;
