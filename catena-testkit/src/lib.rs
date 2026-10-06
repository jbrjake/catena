//! Test support for `catena`: fixture graphs, braille raster asserts, SVG and hash visual
//! snapshots, and the oscillation and determinism checks.
//!
//! **License note for test-only use.** This crate is AGPL-3.0-only like the rest of `catena`.
//! Test binaries are not conveyed to anyone, so depending on `catena-testkit` only as a
//! dev-dependency triggers no AGPL obligations for the code under test.

pub mod braille_asserts;
pub mod fixtures;
pub mod svg;
