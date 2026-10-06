//! ratatui widgets for `catena` graphs.
//!
//! This crate holds no logic. It adapts `catena`'s `Surface` to a ratatui `Buffer`, converts
//! terminal events into `catena`'s `InputEvent`, and forwards them to the controller. It depends
//! on `ratatui-core`, not `ratatui`, so it works across application-side ratatui versions.
