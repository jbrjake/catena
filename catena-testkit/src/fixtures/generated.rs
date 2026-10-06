//! Seeded generator families (plan §20): planted-group graphs with mixed labels, occasional
//! self-loops and parallel edges, all drawn from one [`SplitMix64`] stream, so a seed always gives
//! the same graph. The core draws no random numbers (plan §8.1); this is the only RNG in the
//! workspace, and it lives in test support.

use super::{FixtureEdge, FixtureGraph, FixtureNode};

/// Sebastiano Vigna's `SplitMix64`: a 64-bit state stepped by the golden-ratio increment and
/// finalized by two xor-shift-multiplies. Tiny, fast, and the same on every platform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    /// A generator whose state starts at `seed`.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        SplitMix64 { state: seed }
    }

    /// The next 64 bits.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A value in `0..n`, by Lemire's widening multiply (no modulo bias); 0 when `n` is 0.
    pub fn below(&mut self, n: u64) -> u64 {
        let wide = u128::from(self.next_u64()) * u128::from(n);
        u64::try_from(wide >> 64).unwrap_or(0)
    }

    /// A value in `0..n` as an index.
    fn index(&mut self, n: usize) -> usize {
        let n = u64::try_from(n).unwrap_or(u64::MAX);
        usize::try_from(self.below(n)).unwrap_or(0)
    }

    /// A float in `[0, 1)`, from the top 53 bits.
    #[expect(
        clippy::cast_precision_loss,
        reason = "53-bit values convert to f64 exactly"
    )]
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// `true` with probability `p`.
    fn chance(&mut self, p: f64) -> bool {
        self.unit() < p
    }
}

const ASCII: [&str; 12] = [
    "node", "alpha", "relay", "Ada", "Babbage", "engine", "store", "parser", "cache", "Lovelace",
    "gateway", "queue",
];
const CJK: [&str; 4] = ["東京", "日本語", "中心", "網絡"];
const EMOJI: [&str; 3] = ["🚀", "📦", "🔥"];
const MARKED: [&str; 3] = ["cafe\u{301}", "re\u{301}sume\u{301}", "nai\u{308}ve"];

/// Nodes per planted group, about.
const GROUP_SIZE: usize = 12;

/// A planted-group graph of `n` nodes drawn from `seed` (plan §20).
///
/// Nodes fall into groups of about twelve, with one in twenty ungrouped; each pair inside a
/// group is joined with probability 0.3, each node reaches outside its group with probability
/// 0.03, and an ungrouped node gets two edges anywhere. Labels are one to three words drawn
/// from ASCII, CJK, emoji and combining-mark words. Then `max(1, n / 50)` self-loops and
/// `max(1, n / 40)` parallel copies of existing edges are added, when there are nodes and
/// edges to add them to. Keys are `n0`, `n1`, …; every edge has the kind `link`.
#[must_use]
pub fn generated(seed: u64, n: usize) -> FixtureGraph {
    let mut rng = SplitMix64::new(seed);
    let groups = n.div_ceil(GROUP_SIZE).max(1);
    let nodes: Vec<FixtureNode> = (0..n)
        .map(|i| {
            let group = if rng.below(20) == 0 {
                None
            } else {
                u32::try_from(i * groups / n.max(1)).ok()
            };
            FixtureNode {
                key: format!("n{i}"),
                label: label(&mut rng),
                group,
                confidence: thousandths(0.5 + 0.5 * rng.unit()),
                pagerank: thousandths(rng.unit() / 10.0),
            }
        })
        .collect();

    let mut ends: Vec<(usize, usize)> = Vec::new();
    for a in 0..n {
        for b in a + 1..n {
            if nodes[a].group.is_some() && nodes[a].group == nodes[b].group && rng.chance(0.3) {
                ends.push(if rng.below(2) == 0 { (a, b) } else { (b, a) });
            }
        }
    }
    for (a, node) in nodes.iter().enumerate() {
        let reaches = if node.group.is_none() {
            2
        } else {
            usize::from(rng.chance(0.03))
        };
        for _ in 0..reaches {
            let b = rng.index(n);
            if b != a {
                ends.push((a, b));
            }
        }
    }
    if n > 0 {
        for _ in 0..(n / 50).max(1) {
            let a = rng.index(n);
            ends.push((a, a));
        }
    }
    if !ends.is_empty() {
        for _ in 0..(n / 40).max(1) {
            let copy = ends[rng.index(ends.len())];
            ends.push(copy);
        }
    }

    let edges = ends
        .into_iter()
        .map(|(a, b)| FixtureEdge {
            source: nodes[a].key.clone(),
            target: nodes[b].key.clone(),
            kind: "link".to_string(),
            weight: thousandths(0.3 + 0.7 * rng.unit()),
        })
        .collect();
    FixtureGraph {
        description: format!("generated(seed {seed}, n {n}): planted groups of about {GROUP_SIZE}"),
        nodes,
        edges,
    }
}

/// `x` to the nearest thousandth, like the community fixture's decimals: a value of three
/// decimals reads back from JSON exactly, where a full 17-digit one can lose its last bit.
fn thousandths(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

/// One to three words, each ASCII with probability 0.7, else CJK, emoji or marked.
fn label(rng: &mut SplitMix64) -> String {
    let words = 1 + rng.index(3);
    let mut out: Vec<&str> = Vec::with_capacity(words);
    for _ in 0..words {
        let pool: &[&str] = match rng.below(10) {
            0..=6 => &ASCII,
            7 => &CJK,
            8 => &EMOJI,
            _ => &MARKED,
        };
        out.push(pool[rng.index(pool.len())]);
    }
    out.join(" ")
}
