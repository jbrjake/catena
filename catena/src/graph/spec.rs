//! What the host says about a node or an edge (plan §4.2), and the boundary rules that make
//! every stored spec finite and bounded: a hostile label cannot make a frame O(label), and no
//! weight or pin can put a NaN into a layout.

use crate::raster::text::{char_width, truncate_to_columns};

/// How a node is drawn; its box is measured from this and its label (plan §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NodeShape {
    /// The label, as much of it as the semantic zoom level shows.
    #[default]
    Label,
    /// A bordered box at least `min_w` columns wide and `min_h` rows tall.
    Box {
        /// The box's least width in columns, border included.
        min_w: u16,
        /// The box's least height in rows, border included.
        min_h: u16,
    },
    /// One character standing for the node.
    Glyph(char),
}

/// How an edge's line is drawn. The style resolver turns it into colors (plan §12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EdgeClass {
    /// A solid line.
    #[default]
    Solid,
    /// A dashed line.
    Dashed,
    /// A quadratic bowed 20% of its length to one side (plan §7.3).
    Curved,
}

/// A node as the host describes it (plan §4.2).
///
/// The store keeps it as given, except at the boundary: the label and sort key are cut to the
/// view's `max_label_cols` display columns (and as many bytes, times the 15 a drawn cell
/// holds); a weight that is NaN becomes 1.0 and any other is clamped into `0.0..=f32::MAX`; a
/// pin with a NaN coordinate is dropped and any other is clamped into the `i32` range, since a
/// world unit is one cell column (plan §6); and a negative zero becomes zero.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct NodeSpec {
    /// The display text, measured in display columns, never bytes.
    pub label: String,
    /// The node's place in every ordering that reaches the output; the label when `None`.
    pub sort_key: Option<String>,
    /// The node's layout mass; 1.0 by default.
    pub weight: f32,
    /// A fixed world position, which the simulation does not move.
    pub pinned: Option<(f64, f64)>,
    /// How the node is drawn.
    pub shape: NodeShape,
}

impl NodeSpec {
    /// A plain node showing `text`, with every other field at its default.
    #[must_use]
    pub fn label(text: impl Into<String>) -> Self {
        NodeSpec {
            label: text.into(),
            ..NodeSpec::default()
        }
    }

    /// The string the node sorts by: its sort key, or its label.
    pub(crate) fn order_key(&self) -> &str {
        self.sort_key.as_deref().unwrap_or(&self.label)
    }

    /// Applies the boundary rules of the type's doc comment.
    pub(crate) fn sanitize(&mut self, max_label_cols: u16) {
        let cols = usize::from(max_label_cols);
        truncate_to_columns(&mut self.label, cols);
        if let Some(sort_key) = &mut self.sort_key {
            truncate_to_columns(sort_key, cols);
        }
        self.weight = sanitize_weight(self.weight);
        self.pinned = self.pinned.and_then(|(x, y)| {
            let limit = f64::from(i32::MAX);
            let clamp = |v: f64| v.clamp(-limit, limit) + 0.0;
            (!x.is_nan() && !y.is_nan()).then(|| (clamp(x), clamp(y)))
        });
    }
}

impl Default for NodeSpec {
    fn default() -> Self {
        NodeSpec {
            label: String::new(),
            sort_key: None,
            weight: 1.0,
            pinned: None,
            shape: NodeShape::Label,
        }
    }
}

/// An edge as the host describes it (plan §4.2). A NaN weight becomes 1.0 and any other is
/// clamped into `0.0..=f32::MAX`, as a node's is.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct EdgeSpec {
    /// Its attraction multiplier and routing priority; 1.0 by default.
    pub weight: f32,
    /// Whether it points from its source to its target.
    pub directed: bool,
    /// How its line is drawn.
    pub class: EdgeClass,
    /// Whether it exerts layout forces; `false` draws it without letting it pull.
    pub layout_participating: bool,
}

impl EdgeSpec {
    /// A directed, solid, layout-participating edge of weight 1.
    #[must_use]
    pub fn directed() -> Self {
        EdgeSpec {
            directed: true,
            ..EdgeSpec::undirected()
        }
    }

    /// An undirected, solid, layout-participating edge of weight 1.
    #[must_use]
    pub fn undirected() -> Self {
        EdgeSpec {
            weight: 1.0,
            directed: false,
            class: EdgeClass::Solid,
            layout_participating: true,
        }
    }

    /// Applies the boundary rule of the type's doc comment.
    pub(crate) fn sanitize(&mut self) {
        self.weight = sanitize_weight(self.weight);
    }
}

impl Default for EdgeSpec {
    fn default() -> Self {
        EdgeSpec::undirected()
    }
}

/// Whether two labels lay out alike: the same whitespace characters in the same places, and
/// characters of the same display width everywhere else, width-0 ones included. Every measure
/// built from widths, whitespace and line breaks (truncation to a width, word wrap, a box
/// around the lines) then gives both the same box, so an edit between them is a property
/// change, not a geometric one (plan §4.2). Width-0 characters count because word wrap places
/// a word of them on a line of its own; adding one is a geometric change, which costs at most
/// a re-snap that moves nothing.
pub(crate) fn same_layout(a: &str, b: &str) -> bool {
    layout_units(a).eq(layout_units(b))
}

/// What of a character can change a layout: which whitespace it is, or else its width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LayoutUnit {
    Space(char),
    Ink(usize),
}

fn layout_units(text: &str) -> impl Iterator<Item = LayoutUnit> + '_ {
    text.chars().map(|c| {
        if c.is_whitespace() {
            LayoutUnit::Space(c)
        } else {
            LayoutUnit::Ink(char_width(c))
        }
    })
}

/// NaN becomes the default 1.0; anything else is clamped into `0.0..=f32::MAX`, and a
/// negative zero becomes zero.
fn sanitize_weight(weight: f32) -> f32 {
    if weight.is_nan() {
        1.0
    } else {
        weight.clamp(0.0, f32::MAX) + 0.0
    }
}

#[cfg(test)]
#[path = "spec_tests.rs"]
mod tests;
