//! Interaction (plan §10): the library's input events, the actions they ask for, the key
//! table between them, and the controller that acts on them through [`GraphView`]'s
//! `handle`, `act` and `tick`, with hit-testing, spatial navigation, and animations over
//! `tick(dt)`.
//!
//! [`GraphView`]: crate::GraphView

pub(crate) mod animation;
pub(crate) mod controller;
pub(crate) mod hit;
mod input;
mod keymap;
pub(crate) mod navigation;
mod pointer;
mod spatial;

pub use controller::{MAX_TICK, Outcome, Redraw};
pub use input::{Action, Button, Dir, InputEvent, KeyCode, KeyInput, Modifiers};
pub use keymap::Keymap;
