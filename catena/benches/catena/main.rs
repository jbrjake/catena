//! `catena`'s one bench target (plan §3.1): the plan §15.1 budgets that exist so far, through
//! the public API. Each bench's id is what `cargo xtask perf` (`scripts/check-perf.sh`) holds
//! to its budget and to this machine's baseline.
//!
//! The graphs are the seeded generated families (plan §20): 200 nodes come with about 330
//! edges, 1,000 with about 1,650. Hit-testing (M3) and the layered pipeline (M4) join later.
//! After the run the binary prints its peak resident set, on Linux, for the ratchet.

use std::hint::black_box;
use std::time::Duration;

use catena::raster::CellGrid;
use catena::{EdgeSpec, GraphView, NodeSpec};
use catena_testkit::fixtures::{FixtureGraph, generated};
use criterion::{BatchSize, Criterion, SamplingMode};

/// The frame the render benches draw into (plan §15.1).
const FRAME: (u16, u16) = (200, 50);

fn view(graph: &FixtureGraph) -> GraphView<String> {
    graph
        .view(GraphView::builder())
        .expect("generated graphs are consistent")
}

/// A cold layout: an empty view takes the whole graph in one commit, which measures, lays out
/// and snaps it.
fn cold(c: &mut Criterion) {
    let mut group = c.benchmark_group("layout");
    for n in [200, 1_000] {
        let graph = generated(1, n);
        if n > 200 {
            // About 0.1 s an iteration: ten samples of one iteration each.
            group.sample_size(10).sampling_mode(SamplingMode::Flat);
        }
        group.bench_function(format!("cold_{n}"), |b| b.iter(|| black_box(view(&graph))));
    }
    group.finish();
}

/// A warm relayout after 5% of the nodes arrive (plan §8.3), from a view of the rest.
fn warm(c: &mut Criterion) {
    let graph = generated(1, 200);
    let held = |i: usize| i % 20 == 7;
    let mut rest = FixtureGraph {
        nodes: Vec::new(),
        edges: Vec::new(),
        ..graph.clone()
    };
    rest.nodes = graph
        .nodes
        .iter()
        .enumerate()
        .filter(|(i, _)| !held(*i))
        .map(|(_, n)| n.clone())
        .collect();
    let kept = |key: &String| rest.nodes.iter().any(|n| &n.key == key);
    rest.edges = graph
        .edges
        .iter()
        .filter(|e| kept(&e.source) && kept(&e.target))
        .cloned()
        .collect();
    let base = view(&rest);
    let arriving: Vec<_> = graph
        .nodes
        .iter()
        .enumerate()
        .filter(|(i, _)| held(*i))
        .map(|(_, n)| n.clone())
        .collect();
    let joining: Vec<_> = graph
        .edges
        .iter()
        .filter(|e| !(kept(&e.source) && kept(&e.target)))
        .cloned()
        .collect();
    // In a group, so criterion files it as `layout/warm_5pct_200` beside the cold layouts.
    let mut group = c.benchmark_group("layout");
    group.bench_function("warm_5pct_200", |b| {
        b.iter_batched(
            || base.clone(),
            |mut gv| {
                gv.update(|tx| {
                    for node in &arriving {
                        tx.add_node(node.key.clone(), NodeSpec::label(node.label.clone()))?;
                    }
                    for edge in &joining {
                        tx.add_edge(&edge.source, &edge.target, EdgeSpec::directed())?;
                    }
                    Ok(())
                })
                .expect("the held-out nodes are fresh");
                gv
            },
            BatchSize::LargeInput,
        );
    });
    group.finish();
}

/// A property edit (plan §4.2): no layout runs, nothing moves.
fn property(gv: &mut GraphView<String>, key: &String, step: &mut u32) {
    *step = step.wrapping_add(1);
    let weight = 1.0 + f32::from(u8::try_from(*step % 7).unwrap_or(0)) / 10.0;
    gv.update(|tx| tx.set_node(key, |spec| spec.weight = weight))
        .expect("a live node");
}

/// Frames into a kept grid: steady (the kept scene, plan §15.2) and rebuilt after a property
/// edit, which keeps the layout but drops the scene (plan §15.1's "full frame, layout cached").
fn frames(c: &mut Criterion) {
    let mut group = c.benchmark_group("frame");
    group.sample_size(50);
    for n in [200, 1_000] {
        let graph = generated(1, n);
        let mut gv = view(&graph);
        let mut grid = CellGrid::new(FRAME.0, FRAME.1);
        gv.render(&mut grid);
        group.bench_function(format!("steady_{n}"), |b| {
            b.iter(|| gv.render(black_box(&mut grid)));
        });
        let key = graph.nodes[0].key.clone();
        let mut step = 0;
        group.bench_function(format!("rebuilt_{n}"), |b| {
            b.iter(|| {
                property(&mut gv, &key, &mut step);
                gv.render(black_box(&mut grid));
            });
        });
    }
    group.finish();
}

/// `update()` with a property delta on a 1,000-node graph (plan §15.1).
fn update(c: &mut Criterion) {
    let graph = generated(1, 1_000);
    let mut gv = view(&graph);
    let key = graph.nodes[0].key.clone();
    let mut step = 0;
    let mut group = c.benchmark_group("update");
    group.bench_function("property_1000", |b| {
        b.iter(|| property(&mut gv, &key, &mut step));
    });
    group.finish();
}

/// This process's peak resident set in KiB, where the platform says (Linux).
fn peak_rss_kib() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|l| l.starts_with("VmHWM:"))?;
    line.split_whitespace().nth(1)?.parse().ok()
}

fn main() {
    let mut c = Criterion::default()
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3))
        .configure_from_args();
    cold(&mut c);
    warm(&mut c);
    frames(&mut c);
    update(&mut c);
    c.final_summary();
    match peak_rss_kib() {
        Some(kib) => println!("catena-peak-rss-kib {kib}"),
        None => println!("catena-peak-rss-kib unknown"),
    }
}
