//! Core data types for the DAG layout engine.
//!
//! Input types: Layer, EdgeColor, DagNode, DagEdge, Dag, DagMeta.
//! Output types: BoxBorder, LayoutBox, JunctionKind, EdgeSegment, Layout.

#![allow(dead_code)]

use std::collections::HashMap;
use uuid::Uuid;

// ── Data model: Input ────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub(crate) enum Layer {
    Entity,
    Snippet,
    Document,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum EdgeColor {
    Outgoing,
    Incoming,
    Neutral,
}

#[derive(Clone, Debug)]
pub(crate) struct DagNode {
    pub id: usize,
    pub layer: Layer,
    pub label: String,
    pub secondary_labels: Vec<String>,
    pub min_width: Option<usize>,
}

#[derive(Clone, Debug)]
pub(crate) struct DagEdge {
    pub from: usize,
    pub to: usize,
    pub color_hint: EdgeColor,
}

#[derive(Clone, Debug)]
pub(crate) struct Dag {
    pub nodes: Vec<DagNode>,
    pub edges: Vec<DagEdge>,
}

/// App-level metadata for DAG nodes, kept separate from pure layout data.
/// Maps DagNode IDs to application concepts (UUIDs, colors, directions).
pub(crate) struct DagMeta {
    /// Entity DagNode ID -> (TuiEntity UUID, is_outgoing, entity_type)
    pub entity_info: HashMap<usize, (Uuid, bool, String)>,
    /// Document DagNode ID -> (document UUID, source name)
    pub doc_info: HashMap<usize, (Option<Uuid>, String)>,
    /// Document DagNode ID -> color palette index (for per-document pipe coloring)
    pub doc_color_idx: HashMap<usize, usize>,
    /// Snippet DagNode ID -> list of (entity_name, relation_type) for highlighting
    pub snippet_highlights: HashMap<usize, Vec<(String, String)>>,
    /// The focused entity's info (for rendering the card)
    pub focused_entity_id: Uuid,
    pub focused_entity_name: String,
    pub focused_entity_type: String,
    pub focused_entity_confidence: f64,
    pub focused_entity_description: Option<String>,
    /// Metrics for the entity card
    pub relation_count: usize,
    pub total_connections: usize,
    pub degree_centrality: Option<i32>,
    pub pagerank: Option<f64>,
    pub convergence_score: Option<f64>,
    pub community_id: Option<i32>,
}

// ── Data model: Output ───────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum BoxBorder {
    Single,
    Double,
}

#[derive(Clone, Debug)]
pub(crate) struct LayoutBox {
    pub id: usize,
    pub col: usize,
    pub width: usize,
    pub row: usize,
    pub height: usize,
    pub content: Vec<String>,
    pub border: BoxBorder,
    pub layer: Layer,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum JunctionKind {
    TeeDown,  // ┬  horizontal bar, pipe drops down
    TeeUp,    // ┴  pipe from above, horizontal both ways
    TeeRight, // ├  pipe continues down, branch goes right
    TeeLeft,  // ┤  pipe continues down, branch goes left
    CornerDR, // ┌  horizontal right, pipe drops down
    CornerDL, // ┐  horizontal left, pipe drops down
    CornerUR, // └  pipe from above, turns right
    CornerUL, // ┘  pipe from above, turns left
    Cross,    // ┼  vertical crosses horizontal
}

impl JunctionKind {
    pub fn to_char(self) -> char {
        match self {
            JunctionKind::TeeDown => '┬',
            JunctionKind::TeeUp => '┴',
            JunctionKind::TeeRight => '├',
            JunctionKind::TeeLeft => '┤',
            JunctionKind::CornerDR => '┌',
            JunctionKind::CornerDL => '┐',
            JunctionKind::CornerUR => '└',
            JunctionKind::CornerUL => '┘',
            JunctionKind::Cross => '┼',
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum EdgeSegment {
    HBar {
        row: usize,
        col_start: usize,
        col_end: usize,
        color: EdgeColor,
    },
    VPipe {
        col: usize,
        row_start: usize,
        row_end: usize,
        color: EdgeColor,
    },
    Junction {
        col: usize,
        row: usize,
        kind: JunctionKind,
        color: EdgeColor,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct Layout {
    pub boxes: Vec<LayoutBox>,
    pub edges: Vec<EdgeSegment>,
    pub total_rows: usize,
    pub total_cols: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_enum_values() {
        assert_ne!(Layer::Entity, Layer::Snippet);
        assert_ne!(Layer::Snippet, Layer::Document);
    }

    #[test]
    fn junction_to_char() {
        assert_eq!(JunctionKind::TeeDown.to_char(), '┬');
        assert_eq!(JunctionKind::CornerUR.to_char(), '└');
        assert_eq!(JunctionKind::Cross.to_char(), '┼');
    }

    #[test]
    fn dag_struct_holds_nodes_and_edges() {
        let dag = Dag {
            nodes: vec![DagNode {
                id: 0,
                layer: Layer::Entity,
                label: "Bob".into(),
                secondary_labels: vec!["MANAGES".into()],
                min_width: None,
            }],
            edges: vec![],
        };
        assert_eq!(dag.nodes.len(), 1);
        assert_eq!(dag.nodes[0].label, "Bob");
    }
}
