//! The seeded generator families (plan §20): `SplitMix64` against its published outputs, and
//! generated graphs that are reproducible, consistent and as awkward as the plan asks.

use std::collections::BTreeSet;

use catena_testkit::fixtures::{FixtureGraph, SplitMix64, generated, parse};

#[test]
fn splitmix64_gives_the_reference_sequence() {
    // Vigna's reference `splitmix64.c` from state 0, and from state 1234567.
    let mut zero = SplitMix64::new(0);
    assert_eq!(
        [zero.next_u64(), zero.next_u64(), zero.next_u64()],
        [
            0xe220_a839_7b1d_cdaf,
            0x6e78_9e6a_a1b9_65f4,
            0x06c4_5d18_8009_454f
        ]
    );
    let mut other = SplitMix64::new(1_234_567);
    assert_eq!(
        [other.next_u64(), other.next_u64()],
        [6_457_827_717_110_365_317, 3_203_168_211_198_807_973]
    );
}

#[test]
fn below_stays_below_and_reaches_every_value() {
    let mut rng = SplitMix64::new(7);
    let mut seen = BTreeSet::new();
    for _ in 0..1000 {
        let v = rng.below(6);
        assert!(v < 6);
        seen.insert(v);
    }
    assert_eq!(seen.len(), 6);
    assert_eq!(
        rng.below(0),
        0,
        "an empty range gives 0 rather than panicking"
    );
}

#[test]
fn a_seed_always_generates_the_same_graph() {
    assert_eq!(generated(42, 120), generated(42, 120));
    assert_ne!(generated(42, 120), generated(43, 120));
}

#[test]
fn a_generated_graph_is_a_consistent_fixture() {
    for (seed, n) in [(0, 0), (1, 1), (2, 7), (42, 120), (9, 300)] {
        let graph = generated(seed, n);
        assert_eq!(graph.nodes.len(), n, "seed {seed}");
        let json = to_json(&graph);
        assert_eq!(parse(&json).expect("consistent"), graph, "seed {seed}");
    }
}

#[test]
fn the_demo_instance_has_planted_groups_and_awkward_parts() {
    let graph = generated(42, 120);
    let groups: BTreeSet<Option<u32>> = graph.nodes.iter().map(|n| n.group).collect();
    assert!(groups.len() >= 8, "planted groups: {groups:?}");
    let group_of = |key: &str| {
        graph
            .nodes
            .iter()
            .find(|n| n.key == key)
            .and_then(|n| n.group)
    };
    let within = graph
        .edges
        .iter()
        .filter(|e| group_of(&e.source) == group_of(&e.target))
        .count();
    assert!(
        within * 4 > graph.edges.len() * 3,
        "mostly within groups: {within} of {}",
        graph.edges.len()
    );
    assert!(
        graph.edges.iter().any(|e| e.source == e.target),
        "a self-loop"
    );
    let pairs: Vec<(&str, &str)> = graph
        .edges
        .iter()
        .map(|e| (e.source.as_str(), e.target.as_str()))
        .collect();
    let distinct: BTreeSet<_> = pairs.iter().collect();
    assert!(distinct.len() < pairs.len(), "a parallel edge");
    let labels: String = graph.nodes.iter().map(|n| n.label.as_str()).collect();
    for (what, found) in [
        (
            "CJK",
            labels
                .chars()
                .any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)),
        ),
        ("emoji", labels.chars().any(|c| c >= '\u{1f300}')),
        ("a combining mark", labels.contains('\u{301}')),
        ("ASCII", labels.chars().any(|c| c.is_ascii_alphabetic())),
    ] {
        assert!(found, "no {what} label");
    }
}

/// `graph` as fixture JSON, so the loader's checks run on it.
fn to_json(graph: &FixtureGraph) -> String {
    let quote = |s: &str| format!("{s:?}");
    let nodes: Vec<String> = graph
        .nodes
        .iter()
        .map(|n| {
            let group = n.group.map_or("null".to_string(), |g| g.to_string());
            format!(
                r#"{{"key":{},"label":{},"group":{group},"confidence":{},"pagerank":{}}}"#,
                quote(&n.key),
                json_string(&n.label),
                n.confidence,
                n.pagerank
            )
        })
        .collect();
    let edges: Vec<String> = graph
        .edges
        .iter()
        .map(|e| {
            format!(
                r#"{{"source":{},"target":{},"kind":{},"weight":{}}}"#,
                quote(&e.source),
                quote(&e.target),
                quote(&e.kind),
                e.weight
            )
        })
        .collect();
    format!(
        r#"{{"description":{},"nodes":[{}],"edges":[{}]}}"#,
        json_string(&graph.description),
        nodes.join(","),
        edges.join(",")
    )
}

/// A JSON string literal: `\u` escapes for everything outside printable ASCII.
fn json_string(s: &str) -> String {
    let mut out = String::from("\"");
    for unit in s.encode_utf16() {
        match unit {
            0x22 => out.push_str("\\\""),
            0x5c => out.push_str("\\\\"),
            0x20..=0x7e => out.push(char::from(u8::try_from(unit).expect("ASCII"))),
            _ => {
                use std::fmt::Write as _;
                write!(out, "\\u{unit:04x}").expect("writing to a String cannot fail");
            }
        }
    }
    out.push('"');
    out
}
