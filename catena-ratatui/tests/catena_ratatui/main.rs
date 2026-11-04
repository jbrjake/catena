//! `catena-ratatui`'s one integration-test target (plan §3.1). Each topic is a `mod` in this
//! directory, never a new file directly under `tests/`.

// The extras' tests need their feature, which the push gate enables through `demo`.
#[cfg(feature = "widget-extras")]
mod extras;
mod widget;
