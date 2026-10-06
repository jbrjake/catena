//! DagBuilder — fluent test API for constructing Dag instances.

#![allow(dead_code)]

use super::models::*;

pub(crate) struct DagBuilder {
    nodes: Vec<DagNode>,
    edges: Vec<DagEdge>,
    next_id: usize,
}

impl DagBuilder {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            next_id: 0,
        }
    }

    pub fn entity(self, name: &str) -> Self {
        self.entity_with_labels(name, &[])
    }

    pub fn entity_with_labels(mut self, name: &str, labels: &[&str]) -> Self {
        self.nodes.push(DagNode {
            id: self.next_id,
            layer: Layer::Entity,
            label: name.into(),
            secondary_labels: labels.iter().map(|s| s.to_string()).collect(),
            min_width: None,
        });
        self.next_id += 1;
        self
    }

    pub fn snippet(mut self, text: &str, parents: &[&str]) -> Self {
        let snippet_id = self.next_id;
        self.nodes.push(DagNode {
            id: snippet_id,
            layer: Layer::Snippet,
            label: text.into(),
            secondary_labels: Vec::new(),
            min_width: None,
        });
        self.next_id += 1;
        for parent_label in parents {
            if let Some(parent) = self.nodes.iter().find(|n| n.label == *parent_label) {
                self.edges.push(DagEdge {
                    from: parent.id,
                    to: snippet_id,
                    color_hint: EdgeColor::Neutral,
                });
            }
        }
        self
    }

    pub fn document(mut self, name: &str, parents: &[&str]) -> Self {
        let doc_id = self.next_id;
        self.nodes.push(DagNode {
            id: doc_id,
            layer: Layer::Document,
            label: name.into(),
            secondary_labels: Vec::new(),
            min_width: None,
        });
        self.next_id += 1;
        for parent_label in parents {
            if let Some(parent) = self.nodes.iter().find(|n| n.label == *parent_label) {
                self.edges.push(DagEdge {
                    from: parent.id,
                    to: doc_id,
                    color_hint: EdgeColor::Neutral,
                });
            }
        }
        self
    }

    #[allow(dead_code)] // Available for tests that need explicit edge colors
    pub fn edge(mut self, from_label: &str, to_label: &str, color: EdgeColor) -> Self {
        let from_id = self
            .nodes
            .iter()
            .find(|n| n.label == from_label)
            .map(|n| n.id);
        let to_id = self
            .nodes
            .iter()
            .find(|n| n.label == to_label)
            .map(|n| n.id);
        if let (Some(from), Some(to)) = (from_id, to_id) {
            self.edges.push(DagEdge {
                from,
                to,
                color_hint: color,
            });
        }
        self
    }

    pub fn build(self) -> Dag {
        Dag {
            nodes: self.nodes,
            edges: self.edges,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dag_builder_creates_nodes_and_edges() {
        let dag = DagBuilder::new()
            .entity("Bob")
            .entity("ACME Corp")
            .snippet("Alice manages Bob at ACME", &["Bob", "ACME Corp"])
            .document("report.pdf", &["Alice manages Bob at ACME"])
            .build();

        assert_eq!(dag.nodes.len(), 4); // 2 entities + 1 snippet + 1 doc
        assert_eq!(dag.edges.len(), 3); // Bob→snip, ACME→snip, snip→doc

        let entities: Vec<&str> = dag
            .nodes
            .iter()
            .filter(|n| n.layer == Layer::Entity)
            .map(|n| n.label.as_str())
            .collect();
        assert_eq!(entities, &["Bob", "ACME Corp"]);
    }
}
