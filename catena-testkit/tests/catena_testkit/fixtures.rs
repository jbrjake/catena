//! The fixture loader: the community graph's plan §20 facts, and adversarial inputs.

use std::collections::{BTreeMap, BTreeSet};

use catena_testkit::fixtures::{FixtureError, MAX_FIXTURE_BYTES, community, parse};

#[test]
fn community_has_the_shape_plan_section_20_describes() {
    let graph = community();
    assert_eq!(graph.nodes.len(), 31);
    assert_eq!(graph.edges.len(), 146);

    let group_of: BTreeMap<&str, Option<u32>> = graph
        .nodes
        .iter()
        .map(|n| (n.key.as_str(), n.group))
        .collect();
    let mut sizes: BTreeMap<Option<u32>, usize> = BTreeMap::new();
    for node in &graph.nodes {
        *sizes.entry(node.group).or_default() += 1;
    }
    assert_eq!(sizes.len(), 4);
    assert_eq!(
        sizes,
        BTreeMap::from([(None, 1), (Some(0), 10), (Some(1), 10), (Some(2), 10)])
    );
    let bridger = graph
        .nodes
        .iter()
        .find(|n| n.group.is_none())
        .map(|n| n.key.as_str());
    assert_eq!(bridger, Some("global-consulting-partners"));

    let same_group = |s: &str, t: &str| group_of[s].is_some() && group_of[s] == group_of[t];
    let within = graph
        .edges
        .iter()
        .filter(|e| same_group(&e.source, &e.target))
        .count();
    let touching_bridger = graph
        .edges
        .iter()
        .filter(|e| [&e.source, &e.target].contains(&&"global-consulting-partners".to_string()))
        .count();
    assert_eq!(
        (within, graph.edges.len() - within, touching_bridger),
        (135, 11, 6)
    );

    // Each group is a complete one-directional K10: all 45 unordered pairs, one direction each.
    let pairs: BTreeSet<(&str, &str)> = graph
        .edges
        .iter()
        .map(|e| (e.source.as_str(), e.target.as_str()))
        .collect();
    assert_eq!(pairs.len(), 146, "no parallel edges");
    assert!(pairs.iter().all(|(s, t)| s != t), "no self-loops");
    assert!(
        pairs.iter().all(|&(s, t)| !pairs.contains(&(t, s))),
        "no reciprocal pairs"
    );
    for group in 0..3 {
        let inside = pairs
            .iter()
            .filter(|(s, t)| group_of[s] == Some(group) && group_of[t] == Some(group))
            .count();
        assert_eq!(inside, 45, "group {group}");
    }
}

#[test]
fn community_values_stay_in_their_documented_ranges() {
    let graph = community();
    assert_eq!(graph.nodes.len(), 31);
    for node in &graph.nodes {
        assert!(node.label.is_ascii(), "{:?}", node.label);
        // ASCII, so bytes are display columns.
        assert!(node.label.len() <= 26, "{:?}", node.label);
        assert!((0.80..=0.96).contains(&node.confidence), "{node:?}");
        assert!((0.041..=0.069).contains(&node.pagerank), "{node:?}");
    }
    let pagerank_sum: f64 = graph.nodes.iter().map(|n| n.pagerank).sum();
    assert!(
        (pagerank_sum - 2.0).abs() < 0.01,
        "sums to {pagerank_sum}, a style input"
    );

    let kinds: BTreeSet<&str> = graph.edges.iter().map(|e| e.kind.as_str()).collect();
    assert_eq!(kinds.len(), 17);
    for edge in &graph.edges {
        assert!((0.3..=0.95).contains(&edge.weight), "{edge:?}");
    }
    assert!(!graph.description.is_empty());
}

const MINIMAL: &str = r#"{"description": "d",
  "nodes": [{"key": "a", "label": "A", "group": 1, "confidence": 0.9, "pagerank": 0.1},
            {"key": "b", "label": "B", "group": null, "confidence": 0.8, "pagerank": 0.2}],
  "edges": [{"source": "a", "target": "b", "kind": "k", "weight": 0.5}]}"#;

#[test]
fn a_minimal_fixture_parses() {
    let graph = parse(MINIMAL).expect("valid");
    assert_eq!((graph.nodes.len(), graph.edges.len()), (2, 1));
    assert_eq!(graph.nodes[1].group, None);
    assert_eq!(graph.edges[0].target, "b");
}

fn json_error(input: &str) -> String {
    match parse(input) {
        Err(FixtureError::Json(e)) => e.to_string(),
        other => panic!("expected a JSON error, got {other:?}"),
    }
}

#[test]
fn malformed_and_mistyped_input_is_refused() {
    assert!(json_error("").contains("EOF"));
    assert!(json_error("{").contains("EOF"));
    assert!(json_error("[]").contains("expected struct"));
    assert!(
        json_error(&MINIMAL.replace("\"group\": 1", "\"group\": -1")).contains("invalid value")
    );
    assert!(json_error(&MINIMAL.replace("0.5}", "\"heavy\"}")).contains("invalid type"));
    assert!(
        json_error(&MINIMAL.replace("\"group\": 1", "\"group\": 4294967296"))
            .contains("invalid value")
    );
}

#[test]
fn unknown_and_missing_fields_are_refused() {
    assert!(
        json_error(&MINIMAL.replace("\"kind\"", "\"colour\": 1, \"kind\""))
            .contains("unknown field")
    );
    assert!(json_error(&MINIMAL.replace("\"description\": \"d\",", "")).contains("missing field"));
    assert!(json_error(&MINIMAL.replace(", \"weight\": 0.5", "")).contains("missing field"));
    assert!(
        json_error(&MINIMAL.replace("{\"description\"", "{\"extra\": 0, \"description\""))
            .contains("unknown field")
    );
}

#[test]
fn inconsistent_graphs_are_refused() {
    let duplicate = MINIMAL.replace("\"key\": \"b\"", "\"key\": \"a\"");
    assert!(matches!(parse(&duplicate), Err(FixtureError::DuplicateKey(k)) if k == "a"));
    let dangling = MINIMAL.replace("\"target\": \"b\"", "\"target\": \"zz\"");
    assert!(matches!(
        parse(&dangling),
        Err(FixtureError::UnknownEndpoint { edge: 0, key }) if key == "zz"
    ));
}

#[test]
fn oversized_input_is_refused_before_parsing() {
    let huge = " ".repeat(MAX_FIXTURE_BYTES + 1);
    let err = parse(&huge).expect_err("over the ceiling");
    assert!(matches!(err, FixtureError::TooLarge { bytes } if bytes == MAX_FIXTURE_BYTES + 1));
    assert!(err.to_string().contains("ceiling"));
    // Exactly at the ceiling is parsed (and here fails as JSON, not as size).
    let at_limit = " ".repeat(MAX_FIXTURE_BYTES);
    assert!(matches!(parse(&at_limit), Err(FixtureError::Json(_))));
}

#[test]
fn deep_nesting_is_refused_at_the_first_unexpected_bracket() {
    // Every field is typed and unknown fields are refused, so nothing in the schema can hold
    // a nested array: a hostile fixture is rejected at its first bracket, long before
    // serde_json's own 128-level recursion limit or the stack.
    let nest = |depth: usize| format!("{}{}", "[".repeat(depth), "]".repeat(depth));
    assert!(json_error(&nest(10_000)).contains("invalid type: sequence"));
    let in_field = MINIMAL.replace("\"d\"", &nest(10_000));
    assert!(json_error(&in_field).contains("invalid type: sequence"));
}
