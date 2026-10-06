//! Node measurement (plan §5): the one place a node's size is decided.
//!
//! Four of the seed's bugs came from subsystems measuring nodes each on their own (ledger rows
//! 1, 2, 4 and 27). Here one private function, `measure`, turns a node's spec and the semantic
//! zoom level into a [`NodeForm`]: the box the node takes, decorations included, and exactly
//! what it draws there. [`ResolvedMetrics`] keeps every node's form, measured once per level
//! and again only for the nodes a delta touches; every consumer (collision, grid snapping,
//! edge endpoints, the label mask, hit-testing, the renderer) reads it there.
//!
//! A node takes one of three forms, and its decorations are drawn inside its box, never past
//! it:
//!
//! - **a glyph**, at level 0 or wherever nothing else fits: a `Glyph` node's character, or else
//!   the label's first visible text cell, or else the glyph set's dot;
//! - **a row**: the pin marker, a `Glyph` node's character, then the label in brackets. At the
//!   level's width cap the label is cut, by whole text cells and with an ellipsis, so the
//!   decorations still fit;
//! - **a box**, for [`NodeShape::Box`]: a single border with the pin marker on its top edge,
//!   around the label word-wrapped to the widest the cap allows, centered.
//!
//! Widths are the display columns of the text cells of plan §7.1, so what is measured is what
//! is drawn (invariant N). They depend only on character widths and on where the whitespace is,
//! so a label edit the store classes as a property change (`same_layout`) never changes a box.

use super::cell::CellPt;
use super::zoom::{SemanticZoom, SemanticZoomTable};
use crate::graph::{Delta, GraphStore, Key, NodeIx, NodeShape, NodeSpec};
use crate::raster::text::{char_width, display_width, text_cells, text_cells_with_ends, word_wrap};

/// A glyph a node draws.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Mark {
    /// This text cell: one character of display width 1 or 2 and any width-0 ones after it.
    Text(String),
    /// The glyph set's node dot, of width 1.
    Dot,
}

impl Mark {
    fn width(&self) -> usize {
        match self {
            Mark::Text(text) => display_width(text),
            Mark::Dot => 1,
        }
    }
}

/// The label of a one-row form.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct RowLabel {
    /// The label, or when cut its longest run of leading text cells that fits, less any
    /// trailing blank cells.
    pub text: String,
    /// Whether the label was cut, so an ellipsis follows it.
    pub cut: bool,
}

/// What a node draws.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FormShape {
    /// One glyph.
    Glyph(Mark),
    /// One row: the pin marker, then the node's own glyph, then the label in brackets, each
    /// when present.
    Row {
        /// Whether the row starts with the pin marker.
        pin: bool,
        /// A `Glyph` node's character.
        icon: Option<Mark>,
        /// The label; `None` for a `Glyph` node with no visible label, which draws no brackets.
        label: Option<RowLabel>,
    },
    /// A bordered box with the label's lines centered inside.
    Boxed {
        /// Whether the top border carries the pin marker, in its second cell.
        pin: bool,
        /// The wrapped label, at most as wide as the inside of the box.
        lines: Vec<String>,
    },
}

/// A node's measured box and what it draws in it (plan §5). Only measurement makes one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeForm {
    width: u16,
    height: u16,
    shape: FormShape,
}

impl NodeForm {
    /// Columns the node takes, decorations included; at least 1.
    #[must_use]
    pub fn width(&self) -> u16 {
        self.width
    }

    /// Rows the node takes; at least 1.
    #[must_use]
    pub fn height(&self) -> u16 {
        self.height
    }

    /// What the node draws.
    #[must_use]
    pub fn shape(&self) -> &FormShape {
        &self.shape
    }

    /// The cell edges attach to, from the box's top left: its middle, rounding up and left.
    #[must_use]
    pub fn anchor(&self) -> CellPt {
        let middle = |side: u16| i32::from(side.saturating_sub(1) / 2);
        CellPt::new(middle(self.width), middle(self.height))
    }

    fn size(&self) -> (u16, u16) {
        (self.width, self.height)
    }

    fn one_row(width: usize, shape: FormShape) -> Self {
        NodeForm {
            width: to_u16(width),
            height: 1,
            shape,
        }
    }
}

/// Measures one node: the only function that does (plan §5). Private to `geometry/`, so every
/// other module reads [`ResolvedMetrics`] instead.
pub(super) fn measure(spec: &NodeSpec, level: SemanticZoom, table: &SemanticZoomTable) -> NodeForm {
    let cap = table.cap(level).map_or(usize::from(u16::MAX), usize::from);
    if level.index() > 0 {
        let boxed = match spec.shape {
            NodeShape::Box { min_w, min_h } => {
                boxed(&spec.label, spec.pinned.is_some(), (min_w, min_h), cap)
            }
            NodeShape::Label | NodeShape::Glyph(_) => None,
        };
        if let Some(form) = boxed.or_else(|| row(spec, cap)) {
            return form;
        }
    }
    glyph(spec, cap)
}

/// The glyph form: the node's own glyph, else its label's first visible cell, else the dot,
/// whichever comes first that fits `cap`.
fn glyph(spec: &NodeSpec, cap: usize) -> NodeForm {
    let mark = match spec.shape {
        NodeShape::Glyph(c) => own_glyph(c),
        NodeShape::Label | NodeShape::Box { .. } => text_cells(&spec.label)
            .find(|cell| !cell.symbol.starts_with(char::is_whitespace))
            .map_or(Mark::Dot, |cell| Mark::Text(cell.symbol.to_string())),
    };
    let mark = if mark.width() <= cap { mark } else { Mark::Dot };
    NodeForm::one_row(mark.width(), FormShape::Glyph(mark))
}

/// A `Glyph` node's character: itself when it takes one or two columns, else the dot.
fn own_glyph(c: char) -> Mark {
    if matches!(char_width(c), 1 | 2) {
        Mark::Text(c.to_string())
    } else {
        Mark::Dot
    }
}

/// The one-row form within `cap` columns, or `None` when not even a cut label fits.
fn row(spec: &NodeSpec, cap: usize) -> Option<NodeForm> {
    let pin = spec.pinned.is_some();
    let icon = match spec.shape {
        NodeShape::Glyph(c) => Some(own_glyph(c)),
        NodeShape::Label | NodeShape::Box { .. } => None,
    };
    let decorations = usize::from(pin) + icon.as_ref().map_or(0, Mark::width);
    let label_width = display_width(&spec.label);
    let (label, width) = if icon.is_some() && label_width == 0 {
        (None, decorations)
    } else if decorations + 2 + label_width <= cap {
        let text = spec.label.clone();
        (
            Some(RowLabel { text, cut: false }),
            decorations + 2 + label_width,
        )
    } else if decorations + 3 <= cap {
        let (text, text_width) = leading_cells(&spec.label, cap - decorations - 3);
        (
            Some(RowLabel { text, cut: true }),
            decorations + 3 + text_width,
        )
    } else {
        return None;
    };
    (width <= cap).then(|| NodeForm::one_row(width, FormShape::Row { pin, icon, label }))
}

/// `text`'s longest run of leading text cells within `budget` columns, less any trailing blank
/// cells, and its width.
fn leading_cells(text: &str, budget: usize) -> (String, usize) {
    let (mut end, mut width, mut used) = (0, 0, 0);
    for (cell, cell_end) in text_cells_with_ends(text) {
        used += usize::from(cell.width);
        if used > budget {
            break;
        }
        if !cell.symbol.starts_with(char::is_whitespace) {
            (end, width) = (cell_end, used);
        }
    }
    (text[..end].to_string(), width)
}

/// The boxed form within `cap` columns, or `None` when the cap leaves no room inside a border.
/// The label wraps to the widest inside the cap allows; the box then fits the widest line, but
/// is at least `min_w` (up to the cap) by `min_h`, and 3 × 3.
fn boxed(label: &str, pin: bool, (min_w, min_h): (u16, u16), cap: usize) -> Option<NodeForm> {
    if cap < 3 {
        return None;
    }
    let mut lines = word_wrap(label, cap - 2);
    lines.truncate(usize::from(u16::MAX) - 2);
    let widest = lines.iter().map(|line| display_width(line)).max();
    let width = (widest.unwrap_or(0) + 2)
        .max(usize::from(min_w).min(cap))
        .max(3);
    let height = (lines.len() + 2).max(usize::from(min_h)).max(3);
    Some(NodeForm {
        width: to_u16(width),
        height: to_u16(height),
        shape: FormShape::Boxed { pin, lines },
    })
}

fn to_u16(n: usize) -> u16 {
    u16::try_from(n).unwrap_or(u16::MAX)
}

/// Every node's form at one semantic zoom level (plan §5), measured once per level and again
/// only for the nodes a delta touches. Indexed by node slot.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ResolvedMetrics {
    /// Per node slot, its form; `None` for a vacant slot.
    forms: Vec<Option<NodeForm>>,
    zoom_level: SemanticZoom,
    table: SemanticZoomTable,
}

impl ResolvedMetrics {
    /// Metrics at `level` of `table`, with no node measured yet.
    pub(crate) fn new(table: SemanticZoomTable, level: SemanticZoom) -> Self {
        ResolvedMetrics {
            forms: Vec::new(),
            zoom_level: level,
            table,
        }
    }

    /// The level every form is measured at.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the viewport (M2 step 4) reads and moves the level"
        )
    )]
    pub(crate) fn level(&self) -> SemanticZoom {
        self.zoom_level
    }

    /// The form of the node at `ix`, or `None` for a vacant slot.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "layout, snapping and rendering (M2 steps 3 to 5) read it"
        )
    )]
    pub(crate) fn form(&self, ix: NodeIx) -> Option<&NodeForm> {
        self.forms.get(ix.slot())?.as_ref()
    }

    /// Measures every live node of `store` afresh.
    pub(crate) fn measure_all<K: Key>(&mut self, store: &GraphStore<K>) {
        let (level, table) = (self.zoom_level, &self.table);
        self.forms.clear();
        self.forms.extend(
            store
                .nodes
                .iter()
                .map(|slot| slot.as_ref().map(|node| measure(&node.spec, level, table))),
        );
    }

    /// Moves to `level`, measuring every node again if that is a new level. Returns whether it
    /// was, since boxes may then have changed (plan §6: a level transition relayouts warm).
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "the viewport (M2 step 4) moves the level on zoom")
    )]
    pub(crate) fn set_level<K: Key>(&mut self, store: &GraphStore<K>, level: SemanticZoom) -> bool {
        if level == self.zoom_level {
            return false;
        }
        self.zoom_level = level;
        self.measure_all(store);
        true
    }

    /// Follows one commit's `delta`, which `store` now reflects: forgets removed nodes,
    /// measures added, relabeled and reshaped ones, and keeps in `delta.reshaped` only the nodes
    /// whose box did change. The store classes edits without measuring (`same_layout`), so its
    /// reshapes are a superset; this is where measured boxes decide which nodes move.
    pub(crate) fn apply<K: Key>(&mut self, store: &GraphStore<K>, delta: &mut Delta) {
        let ResolvedMetrics {
            forms,
            zoom_level,
            table,
        } = self;
        let measured = |ix: NodeIx| {
            store
                .node(ix)
                .map(|node| measure(&node.spec, *zoom_level, table))
        };
        forms.resize(store.nodes.len(), None);
        for ix in &delta.removed {
            forms[ix.slot()] = None;
        }
        let relabeled_only = delta
            .relabeled
            .iter()
            .filter(|ix| !delta.reshaped.contains(ix));
        for &ix in delta.added.iter().chain(relabeled_only) {
            forms[ix.slot()] = measured(ix);
        }
        delta.reshaped.retain(|&ix| {
            let form = measured(ix);
            let changed =
                form.as_ref().map(NodeForm::size) != forms[ix.slot()].as_ref().map(NodeForm::size);
            forms[ix.slot()] = form;
            changed
        });
    }
}

#[cfg(test)]
#[path = "metrics_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "metrics_property_tests.rs"]
mod property_tests;
