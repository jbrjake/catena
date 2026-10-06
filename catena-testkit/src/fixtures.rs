//! Fixture graphs (plan §20).
//!
//! The community graph, harvested from the seed, and its loader; the seeded generator
//! families ([`generated`]). The twelve canonical graphs join them in a later milestone.

use std::collections::BTreeSet;
use std::fmt;

use serde::Deserialize;

mod generated;

pub use generated::{SplitMix64, generated};

/// The largest fixture [`parse`] accepts, in bytes. The community graph is about 20 KiB.
pub const MAX_FIXTURE_BYTES: usize = 8 * 1024 * 1024;

/// A fixture graph as stored on disk: top-level `description`, `nodes` and `edges`, and no
/// other fields anywhere.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureGraph {
    pub description: String,
    pub nodes: Vec<FixtureNode>,
    pub edges: Vec<FixtureEdge>,
}

/// One node. `group` is `None` for an ungrouped node; `confidence` and `pagerank` are style
/// inputs for the gradient helpers, not layout inputs.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureNode {
    pub key: String,
    pub label: String,
    pub group: Option<u32>,
    pub confidence: f64,
    pub pagerank: f64,
}

/// One directed edge between node keys. `kind` maps to a legend category, not an `EdgeClass`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureEdge {
    pub source: String,
    pub target: String,
    pub kind: String,
    pub weight: f64,
}

/// Why a fixture failed to load.
#[derive(Debug)]
pub enum FixtureError {
    /// The input is larger than [`MAX_FIXTURE_BYTES`].
    TooLarge { bytes: usize },
    /// The input is not a well-formed fixture: bad JSON, a missing or unknown field, a value of
    /// the wrong type.
    Json(serde_json::Error),
    /// Two nodes share a key.
    DuplicateKey(String),
    /// An edge names a node that does not exist.
    UnknownEndpoint { edge: usize, key: String },
}

impl fmt::Display for FixtureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FixtureError::TooLarge { bytes } => write!(
                f,
                "fixture is {bytes} bytes, over the {MAX_FIXTURE_BYTES}-byte ceiling"
            ),
            FixtureError::Json(e) => write!(f, "malformed fixture: {e}"),
            FixtureError::DuplicateKey(key) => write!(f, "two nodes have the key {key:?}"),
            FixtureError::UnknownEndpoint { edge, key } => {
                write!(f, "edge {edge} names the unknown node {key:?}")
            }
        }
    }
}

impl std::error::Error for FixtureError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            FixtureError::Json(e) => Some(e),
            _ => None,
        }
    }
}

/// Parses and checks a fixture: the size ceiling, the schema, unique node keys, and edges that
/// name existing nodes.
///
/// # Errors
///
/// Any [`FixtureError`]; nothing is returned partially.
pub fn parse(json: &str) -> Result<FixtureGraph, FixtureError> {
    if json.len() > MAX_FIXTURE_BYTES {
        return Err(FixtureError::TooLarge { bytes: json.len() });
    }
    let graph: FixtureGraph = serde_json::from_str(json).map_err(FixtureError::Json)?;
    let mut keys = BTreeSet::new();
    for node in &graph.nodes {
        if !keys.insert(node.key.as_str()) {
            return Err(FixtureError::DuplicateKey(node.key.clone()));
        }
    }
    for (index, edge) in graph.edges.iter().enumerate() {
        for key in [&edge.source, &edge.target] {
            if !keys.contains(key.as_str()) {
                return Err(FixtureError::UnknownEndpoint {
                    edge: index,
                    key: key.clone(),
                });
            }
        }
    }
    Ok(graph)
}

/// The community graph (plan §20): 31 nodes in 3 planted groups of 10 plus one ungrouped
/// bridger, and 146 directed edges, 135 of them inside a group. Embedded at compile time, so
/// loading it does no I/O.
///
/// # Panics
///
/// Never for the committed file; the testkit's own tests parse it.
#[must_use]
pub fn community() -> FixtureGraph {
    parse(include_str!("../fixtures/community.json"))
        .expect("the committed community fixture parses and is consistent")
}
