//! Frame stability checks (plan §16.5) and the byte capture of the PTY tier (plan §16.8).
//!
//! Ported from the source application's blink tests (`test_blink_core.rs`): `diff_count` and
//! `diff_details` compare two frames cell by cell, glyph and style, now over [`CellGrid`] instead
//! of ratatui's `Buffer`, and [`CaptureBuf`] collects the bytes a terminal backend writes. The
//! application harness and its eight async tests are gone: `catena` has no engine to wait on,
//! so a frame is stable or not the moment it is drawn. New here: [`flickers`], the A→B→A
//! detector.

use std::io::{self, Write};
use std::sync::{Arc, Mutex, PoisonError};

use catena::raster::{Cell, CellGrid, Surface};

/// How many differing cells [`diff_details`] describes.
const DETAILS: usize = 20;

/// The cell at `(x, y)` of `grid`, if it is that large.
fn at(grid: &CellGrid, x: u16, y: u16) -> Option<&Cell> {
    grid.cell(x, y)
}

/// Every cell position where `a` and `b` differ in glyph, colors or attributes, row by row.
/// Grids of different sizes differ in every cell of the larger one.
fn diffs<'g>(
    a: &'g CellGrid,
    b: &'g CellGrid,
) -> impl Iterator<Item = (u16, u16, Option<&'g Cell>, Option<&'g Cell>)> + 'g {
    let same_size = a.size() == b.size();
    let (width, height) = (a.size().0.max(b.size().0), a.size().1.max(b.size().1));
    (0..height).flat_map(move |y| {
        (0..width).filter_map(move |x| {
            let (old, new) = (at(a, x, y), at(b, x, y));
            (!same_size || old != new).then_some((x, y, old, new))
        })
    })
}

/// How many cells differ between `a` and `b`: zero for identical frames.
#[must_use]
pub fn diff_count(a: &CellGrid, b: &CellGrid) -> usize {
    diffs(a, b).count()
}

/// The first twenty cells where `a` and `b` differ, each as
/// `"(x,y): old→new fg:old→new bg:old→new mod:old→new"`, for a failure message.
#[must_use]
pub fn diff_details(a: &CellGrid, b: &CellGrid) -> Vec<String> {
    diffs(a, b)
        .take(DETAILS)
        .map(|(x, y, old, new)| {
            let symbol = |c: Option<&Cell>| c.map_or("", Cell::symbol).to_string();
            format!(
                "({x},{y}): {:?}→{:?} fg:{:?}→{:?} bg:{:?}→{:?} mod:{:?}→{:?}",
                symbol(old),
                symbol(new),
                old.and_then(Cell::fg),
                new.and_then(Cell::fg),
                old.and_then(Cell::bg),
                new.and_then(Cell::bg),
                old.map(Cell::attrs),
                new.map(Cell::attrs),
            )
        })
        .collect()
}

/// The indices of every frame that equals the one two before it while differing from the one
/// just before: the A→B→A signature of flicker (plan §16.5). Empty for a steady run.
#[must_use]
pub fn flickers(frames: &[CellGrid]) -> Vec<usize> {
    (2..frames.len())
        .filter(|&i| frames[i] == frames[i - 2] && frames[i] != frames[i - 1])
        .collect()
}

/// A writer that keeps every byte written to it, shared between its clones: hand one clone
/// to a terminal backend and read what it wrote through another.
#[derive(Debug, Clone, Default)]
pub struct CaptureBuf(Arc<Mutex<Vec<u8>>>);

impl CaptureBuf {
    /// An empty buffer.
    #[must_use]
    pub fn new() -> Self {
        CaptureBuf::default()
    }

    /// How many bytes it holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes().len()
    }

    /// Whether it holds no bytes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Everything written since the last drain, which it then forgets.
    #[must_use]
    pub fn drain(&self) -> Vec<u8> {
        std::mem::take(&mut *self.bytes())
    }

    /// The bytes, whatever a panicking writer left: they are plain data, so a poisoned lock
    /// still holds every byte written before the panic.
    fn bytes(&self) -> std::sync::MutexGuard<'_, Vec<u8>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Write for CaptureBuf {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.bytes().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
