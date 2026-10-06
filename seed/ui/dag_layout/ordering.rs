//! Phase 2: Entity ordering via barycenter heuristic.

#![allow(dead_code)]

use super::models::*;

/// Order entity nodes to minimize edge crossings between entity and snippet layers.
///
/// Uses the barycenter heuristic: each entity's position is the average position
/// of its connected snippets. Two passes to converge.
pub(super) fn order_entities(dag: &Dag) -> Vec<usize> {
    let entity_ids: Vec<usize> = dag
        .nodes
        .iter()
        .filter(|n| n.layer == Layer::Entity)
        .map(|n| n.id)
        .collect();

    if entity_ids.len() <= 1 {
        return entity_ids;
    }

    let snippet_ids: Vec<usize> = dag
        .nodes
        .iter()
        .filter(|n| n.layer == Layer::Snippet)
        .map(|n| n.id)
        .collect();

    // Build adjacency: entity_id → set of snippet indices.
    let entity_to_snippets = |eid: usize| -> Vec<usize> {
        dag.edges
            .iter()
            .filter(|e| e.from == eid && snippet_ids.contains(&e.to))
            .filter_map(|e| snippet_ids.iter().position(|&sid| sid == e.to))
            .collect::<Vec<_>>()
    };

    // Two-pass barycenter.
    let mut ordered = entity_ids.clone();
    for _pass in 0..2 {
        // Assign snippet positions based on current entity order.
        let snippet_positions: Vec<f64> = snippet_ids
            .iter()
            .map(|&sid| {
                let parent_positions: Vec<f64> = dag
                    .edges
                    .iter()
                    .filter(|e| e.to == sid && ordered.contains(&e.from))
                    .filter_map(|e| ordered.iter().position(|&eid| eid == e.from))
                    .map(|p| p as f64)
                    .collect();
                if parent_positions.is_empty() {
                    0.0
                } else {
                    parent_positions.iter().sum::<f64>() / parent_positions.len() as f64
                }
            })
            .collect();

        // Compute barycenter for each entity.
        ordered.sort_by(|&a, &b| {
            let a_snippets = entity_to_snippets(a);
            let b_snippets = entity_to_snippets(b);
            let a_bary = if a_snippets.is_empty() {
                0.0
            } else {
                a_snippets
                    .iter()
                    .map(|&si| snippet_positions.get(si).copied().unwrap_or(0.0))
                    .sum::<f64>()
                    / a_snippets.len() as f64
            };
            let b_bary = if b_snippets.is_empty() {
                0.0
            } else {
                b_snippets
                    .iter()
                    .map(|&si| snippet_positions.get(si).copied().unwrap_or(0.0))
                    .sum::<f64>()
                    / b_snippets.len() as f64
            };
            a_bary
                .partial_cmp(&b_bary)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    ordered
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::dag_layout::builder::DagBuilder;

    #[test]
    fn order_entities_minimizes_crossings() {
        let dag = DagBuilder::new()
            .entity("Bob")
            .entity("ACME Corp")
            .entity("Carol")
            .snippet("S1: Bob and ACME", &["Bob", "ACME Corp"])
            .snippet("S2: Bob and Carol", &["Bob", "Carol"])
            .snippet("S3: Carol solo", &["Carol"])
            .build();

        let ordered = order_entities(&dag);
        let names: Vec<&str> = ordered
            .iter()
            .map(|id| {
                dag.nodes
                    .iter()
                    .find(|n| n.id == *id)
                    .unwrap()
                    .label
                    .as_str()
            })
            .collect();

        let acme_pos = names.iter().position(|n| *n == "ACME Corp").unwrap();
        let bob_pos = names.iter().position(|n| *n == "Bob").unwrap();
        let carol_pos = names.iter().position(|n| *n == "Carol").unwrap();
        assert_eq!(
            (acme_pos as i32 - bob_pos as i32).abs(),
            1,
            "ACME and Bob must be adjacent, got {:?}",
            names
        );
        assert_eq!(
            (bob_pos as i32 - carol_pos as i32).abs(),
            1,
            "Bob and Carol must be adjacent, got {:?}",
            names
        );
    }

    #[test]
    fn order_entities_single_entity() {
        let dag = DagBuilder::new().entity("Bob").build();
        let ordered = order_entities(&dag);
        assert_eq!(ordered.len(), 1);
    }

    #[test]
    fn order_entities_no_snippets() {
        let dag = DagBuilder::new().entity("Bob").entity("Carol").build();
        let ordered = order_entities(&dag);
        assert_eq!(ordered.len(), 2);
    }
}
