# catena — implementation plan

**Status:** proposal of record, ready to execute. All questions are answered and the implementation details are decided (§21). A departure from a decided item is a question for the owner, not a call for the implementing session.
**Name:** `catena`, owner-ruled. Latin *catena*, "chain": the root of *concatenate*, of the medieval *catena* commentaries that link sources passage to passage, and of the *catenary*, the curve a hanging chain makes and the natural line of a slack edge between two nodes. The crates are `catena` (core), `catena-ratatui` (widget) and `catena-testkit`. `catena-core` on crates.io belongs to an unrelated project, which is why the core crate takes the flagship name.

---

## Read this first — what is in this repository and how to start

This repository is self-contained. Implementing this plan needs nothing outside it: no other repository, service, credential or private tool.

| Path | What it is |
|---|---|
| `docs/design/initial-catena-plan.md` | This plan: the design (§1–§16), gates (§17), the seed manifest (§18), milestones (§19), fixtures (§20), the decision register (§21). |
| `CLAUDE.md` | Working rules, loaded every session: the engineering baseline plus this repo's overrides. |
| `LICENSE` | AGPL-3.0, the full text. Crates declare `license = "AGPL-3.0-only"` (§3). |
| `seed/` | **The harvest input.** Source files from an earlier terminal application's graph views (a knowledge-graph TUI), copied verbatim or as excerpts, plus one converted fixture graph. Not a crate, never compiled. §18 says what each file becomes. |

**How to work it.**

1. Start at **M0** (§19). Every milestone ends green and demonstrable, closed by a `verify:` command whose exit 0 you watched. Any milestone boundary is a clean place to stop.
2. The worklist is `TODO.md`, created at M0 from §19 (format in `CLAUDE.md`, override 1). Its `## Now` section always names the next concrete step, startable cold, and `## Decisions` records any implementation choice this plan left open, with its reason.
3. The seed leaves `seed/` one file at a time by the two-commit procedure in §18. Never edit a seed file in place.
4. The workspace scaffold in §3.1 lands in the **first** commit that creates a crate, not later.
5. Publishing to crates.io for real (`cargo publish` without `--dry-run`) is the owner's act, never the session's.

**What "the seed" means in this document.** A "the seed does X" below is a statement about a file under `seed/`, cited by path and symbol. A few statements describe parts of the source application that were not seeded (its end-to-end test harness, its windowing, its DAG app renderers); those name "the source application" or say "not seeded", and nothing in them needs to be found. The seed's code refers to types that are not in this repository; they are the host application's and none of them are wanted:

| Seed type | What it was | catena's replacement |
|---|---|---|
| `App` | the host application's state struct (≈190 fields) | `GraphView` + `Controller` (§4, §10) |
| `Uuid` node and edge ids | hard-coded key type | generic `K: Key`, interned to `NodeIx` (§4.1) |
| `ColorPalette`, entity types, `TuiRelation`, communities | the host's domain model | `StyleResolver` callback, plain `u32` groups (§12) |
| `window_manager`, `WindowId`, floating windows | the host's windowing | host-supplied occlusion rectangles (§6) |
| `TuiTestRunner`, `dump_screen` (test harness) | the host's end-to-end harness | `catena-testkit::scenario` (§3) |

The originating application's name appears in a few seed comments and in one environment variable in `seed/tests/visual/snapshots.rs`. It is not a catena concept: drop or rename it when porting (the snapshot variable becomes `CATENA_UPDATE_SNAPSHOTS`).

---

## 0. Executive summary

**The verdict: harvest-and-rebuild.** Do **not** wrap the seed's graph views as a library, and do **not** rewrite from a blank page. Build a new workspace with a new architecture, seeded with ~4,300 lines of the seed's clean, tested modules imported nearly verbatim (braille rasterizer, Barnes-Hut quadtree, chord/Holten math, flex allocator, tuning constants and, most valuable of all, its property-test harnesses), while designing the orchestration, data model, rendering pipeline and interaction layers fresh to eliminate the five structural defects that make the seed's version un-extractable as-is.

**Why this is worth building at all:** the ecosystem research found a genuine four-way gap. No existing crate is simultaneously (1) general-topology (not just DAGs), (2) interactive (pan/zoom/select/drag/keyboard-nav), (3) actively maintained and (4) ratatui-native. The closest contenders each satisfy at most two: `tui-nodes`/`ratatui-flow` are DAG-only and non-interactive; `ascii-petgraph` is a 5-commit force-directed proof of concept; `egui_graphs` has the right API shape but targets GUI. The seed is, by a wide margin, the most sophisticated TUI graph renderer the review found anywhere, and it is trapped inside an application.

**Scale of the opportunity in the seed:** the coupling audit found the candidate surface (~7,300 production lines) contains **zero** async, **zero** I/O, **zero** logging, **zero** bare `unwrap()`. That is unusually extraction-friendly. What blocks verbatim extraction is architecture, not hygiene: a 497-line orchestrator (`seed/graph/mod.rs`) welded to 25 fields of the 190-field `App`, a hard-coded `Uuid` node key, a domain-specific `ColorPalette`, a DAG engine hard-coded to exactly three layers, and an interaction layer implemented entirely as `impl App` methods (`seed/app/`).

---

## 1. Evidence base and decision rationale

### 1.1 What the seed contains

Two independent graph systems that share almost nothing:

| | `seed/graph/` (force-directed) | `seed/ui/dag_layout/` ("Sugiyama-lite") |
|---|---|---|
| Purpose | Full-screen knowledge-graph view | Provenance DAG in an inspector window |
| Layout | Fruchterman-Reingold + Barnes-Hut quadtree, warm-started; radial ring for disconnected nodes; chord/Holten overlay | Fixed 3-layer barycenter ordering; no ranking, no cycle handling, no dummy nodes |
| Rendering | Braille sub-cell (2×4) canvases, label-mask blitting, crossing hops | Character-cell boxes + orthogonal pipes, junction glyph table |
| Interaction | Pan/zoom/hover/select/drag, edge spatial index | Row-only click targets |
| Purity | Layout modules pure; orchestrator (`seed/graph/mod.rs`) heavily App-coupled | Layout pure function; renderer App-coupled (not seeded) |

### 1.2 Extract vs. rewrite, scored

**Arguments for pure extraction** (rejected): the pure modules are genuinely excellent; test coverage is deep (~6,100 test lines on the candidate surface); the code embeds hard-won tuning.

**Arguments for pure rewrite** (rejected): the orchestrator is unsalvageable; interaction is structurally app-bound; the DAG engine's algorithms are shallow (§1.3); 20+ documented defects.

**Why harvest-and-rebuild wins:** the review separated the seed's assets into three strata with opposite dispositions:

1. **Bottom stratum — pure math + rasterization (~4,300 lines): harvest verbatim.** `seed/graph/braille.rs` (zero-dep, i32-tolerant, Bresenham/Bézier/dashed/hop-gap/circle), `seed/graph/quadtree.rs` (correct Barnes-Hut with coincident-point jitter), `seed/graph/chord.rs` (Holten bundling, already key-agnostic), `seed/ui/flex_layout.rs` (min/preferred/max allocator with `debug_assert`ed invariants), `seed/graph/zoom.rs`, `word_wrap` in `seed/ui/box_layout.rs`. These carry no App coupling and come with portable tests.
2. **Middle stratum — architecture patterns (~15 "gems"): harvest as designs, reimplement in the new shape.** Canonical/derived position split, warm start + anchor compensation, `visual_label_width` as a shared contract, label-mask blitting, the hop-gap crossing convention, exponential-smoothing zoom, occlusion-aware centering, the junction glyph table, name-based deterministic ordering. These are the differentiators; §5–§10 bake each in.
3. **Top stratum — orchestration, state, interaction glue: discard and redesign.** `seed/graph/mod.rs` (25 App fields, 54 accesses), the full-screen view, all `impl App` interaction methods, the DAG renderer's card/bus geometry that the layout doesn't model.

The bug review makes the strongest case: the majority of the seed's real defects (§14 ledger) are *architectural leaks between strata* — hit-testing using full label width while rendering uses visual width; routing discarding edge color and forcing a downstream raster-rescan hack; three ad-hoc z-order conventions; a renderer drawing geometry the layout never modeled. Extraction would ship those defect generators as the library's public architecture. A rebuild designed around **single-source-of-truth geometry** and a **complete scene graph** (§5, §7.4) eliminates the entire defect class, not just the instances.

### 1.3 What the seed's DAG engine is *not* (and the successor must be)

`seed/ui/dag_layout/` is not Sugiyama: layer assignment is caller-supplied (hard-coded `Entity/Snippet/Document`), there is no cycle detection, no dummy nodes, layer-skipping edges are **silently dropped**, crossing minimization (`order_entities` in `ordering.rs`) is a fixed two-pass barycenter on one layer only, all merge bars in a layer pair share a **single row** (making overlapping bars indistinguishable), and edge color is discarded by the router then reconstructed by scanning the rendered raster. Its *shape* — pure `Dag → layout_dag → Layout → renderers`, golden-ASCII tested (`engine_tests.rs`) — is exactly right; its algorithms must be replaced with the real pipeline (§9).

### 1.4 Ecosystem facts that shaped the design

- **petgraph has no layout module**; every layout crate bolts on. We interop with petgraph behind a feature but do not require it.
- **ForceAtlas2-style degree-scaled repulsion** produces more legible clustering than uniform FR, worth more, not less, at terminal resolution. Offered as a mode.
- **Barnes-Hut loses to brute force below ~1,000 points** (quadtree build overhead). We keep the harvested quadtree but gate it on `n > 500`.
- **Brandes-Köpf has two known flaws**; a 2020 erratum (arXiv:2008.01252) is required reading. v1 uses the simpler priority method; BK + erratum is a v1.1 upgrade.
- **ratatui is immediate-mode with diffed output**: only changed cells hit the wire. Layout *stability* is therefore a rendering-performance feature, not just UX — jittery layout defeats the diff.
- **Braille cells carry one fg color per 2×4 block** (ratatui#693). The color/resolution trade must be a user-visible blitter choice (braille vs half-block vs sextant), not a hidden constant.
- **ratatui 0.30.2 is current, and widget authors are advised to depend on `ratatui-core`** (0.1.2, `rust-version = "1.88"`) so the crate works across app-side ratatui versions. The seed was written against ratatui 0.29; targeting `ratatui-core` decouples catena from app-side versions entirely.
- **Testing state of the art** (zellij, gitui, ratatui's own docs, Textual): a two-tier pattern — fast `TestBackend`/buffer tests + `insta` snapshots always on; a small, serialized PTY tier for what TestBackend can't see. Ratatui's official insta recipe **cannot assert color**; Textual's SVG snapshots can. The seed carries an SVG + SHA-256 snapshot pipeline that closes exactly that gap (`seed/tests/visual/`); we harvest it (§16.4).

---

## 2. Product definition

### 2.1 What `catena` is

A Rust workspace providing interactive graph/network visualization for terminal UIs:

- **General topology**: directed/undirected, cycles, self-loops, parallel edges, disconnected components.
- **Four layout engines**: force-directed (default for general graphs), layered/Sugiyama (DAGs, flowcharts, provenance), tree (hierarchies), radial ring + chord/Holten overlay (cluster structure).
- **Interactive**: pan, zoom (anchor-fixed, smoothed), semantic zoom (6 detail levels), select/hover/pin, node drag, spatial keyboard navigation, hit-testing for nodes *and* edges.
- **Live-data-ready**: incremental updates with a stability contract (§11) — topology deltas warm-start, property deltas never move nodes, new nodes ramp in.
- **Renderer-flexible**: braille (max resolution), half-block (2 colors/cell), sextant (Unicode 13), pure-ASCII/box-drawing (universal fallback) blitters behind one trait.
- **ratatui-native**: `StatefulWidget` impls over `ratatui-core`, plus a renderer-agnostic core usable without ratatui (e.g., static `to_string()` export for docs/CI).

### 2.2 What `catena` is not (scope fences, decided)

- **No async, no I/O, no logging, no clock reads.** The library is a pure state machine; the host owns the event loop, time (§10.4) and data acquisition. This is what made the seed's surface extractable and it is non-negotiable.
- **No domain semantics.** No entities/claims/communities; node meaning arrives via generic keys, user data and a style-resolver callback.
- **No graph analytics.** No PageRank/Louvain/centrality — the host computes metrics; `catena` renders them (they arrive as style inputs).
- **No data-format parsing** (DOT/mermaid/D2) in v1. The scene-graph API makes an importer crate trivial later; explicitly out of scope now.
- **No pixel-graphics protocols** (kitty/sixel) in v1. The `Surface` abstraction (§7.1) leaves room for a `ratatui-image`-backed tier later; do not build protocol negotiation.
- **No ELK/elk-rs backend.** Own Sugiyama implementation, tuned for integer cell coordinates (§9.2). elk-rs is noted as a future optional backend; nothing in v1 depends on it.

### 2.3 Competitive positioning

| | general topology | interactive | maintained | ratatui-native |
|---|---|---|---|---|
| tui-nodes / ratatui-flow | ✗ (DAG wires) | ✗ | ~ | ✓ |
| ascii-petgraph | ✓ | ✗ | ✗ (5 commits) | ✓ |
| egui_graphs | ✓ | ✓ | ✓ | ✗ (egui) |
| graphs-tui / mermaid-ascii / D2-ascii | ✓ | ✗ (static) | ~ | ✗ |
| **catena** | **✓** | **✓** | **✓** | **✓** |

---

## 3. Workspace architecture

```
catena/                              # repo root: virtual workspace manifest
├── Cargo.toml                       # §3.1 — members, lints, profiles
├── Cargo.lock                       # committed
├── .cargo/config.toml               # [alias] xtask only — never rustflags
├── CLAUDE.md  LICENSE  CONTRIBUTING.md  CHANGELOG.md  TODO.md
├── bench-baseline.json              # per-machine perf baseline, from the first bench (§15)
├── scripts/  smoke.sh  regression.sh  check-perf.sh
├── .githooks/  pre-commit → scripts/smoke.sh   pre-push → scripts/regression.sh
├── .github/workflows/               # §17
├── catena/                          # ALL logic. Deps: unicode-width. No ratatui. No I/O.
│   ├── src/
│   │   ├── graph/                   # store, keys, deltas                  (§4)
│   │   ├── geometry/                # metrics, snap, viewport              (§5, §6)
│   │   ├── layout/                  # force/, layered/, tree/, radial/     (§8, §9)
│   │   ├── raster/                  # subcell canvas, blitters, primitives (§7.2–7.3)
│   │   ├── scene/                   # SceneGraph, compositor, layers       (§7.4)
│   │   ├── interact/                # controller, hit-testing, spatial nav (§10)
│   │   └── style/                   # Theme, GlyphSet, StyleResolver       (§12)
│   ├── tests/catena/main.rs         # the crate's ONE integration-test target; topics are `mod`s
│   ├── benches/catena/main.rs       # the crate's ONE bench target (criterion, harness = false)
│   └── examples/render_hash.rs      # tiny render-to-hash binary for the cross-run test (§16.5)
├── catena-ratatui/                  # ratatui widgets. Deps: catena, ratatui-core.
│   ├── src/
│   │   ├── widget.rs                # GraphView StatefulWidget
│   │   ├── minimap.rs               # Minimap widget
│   │   ├── legend.rs                # Legend widget (generic categories)
│   │   └── input.rs                 # InputEvent + From<crossterm::event::Event> (feature)
│   ├── tests/catena_ratatui/main.rs # T5 and (feature `pty-tests`) T7, as `mod`s
│   └── examples/interactive.rs      # the demo binary; static exports alongside
├── catena-testkit/                  # test support, published. Deps: catena, sha2.
│   ├── src/
│   │   ├── svg.rs                   # Surface → SVG + SHA-256 hash (harvested)
│   │   ├── braille_asserts.rs       # lit_pixels, assert_8_connected, endpoints, no-dups (harvested)
│   │   ├── oscillation.rs           # A→B→A detector, steady-state zero-diff, frame hashing
│   │   ├── invariants.rs            # no-overlap, edge-endpoint, determinism helpers
│   │   ├── fixtures.rs              # fixture loading + seeded generators (§20)
│   │   └── scenario.rs              # scripted event-stream runner with injected time
│   ├── fixtures/                    # community.json (from seed/fixtures) + canonical graphs (§20)
│   └── tests/catena_testkit/main.rs
├── xtask/                           # unpublished: `cargo xtask lint` (§17)
├── fuzz/                            # cargo-fuzz crate, excluded from the workspace (§16.7)
├── seed/                            # harvest input (§18): not a crate; empty and deleted by M5
└── docs/design/initial-catena-plan.md
```

**Crate boundary rule (enforced by dependency direction):** `catena` never names a ratatui type. The widget crate contains *no logic* — it adapts `Surface` to ratatui `Buffer`, converts events and forwards to the controller. Anything testable lives in core; the widget crate stays thin enough that its own tests are only composition snapshots.

**Dependency policy (decided):**

| Crate | Runtime deps | Optional features |
|---|---|---|
| catena | `unicode-width` | `serde` (Theme/Positions ser/de), `petgraph` (interop constructors) |
| catena-ratatui | `catena`, `ratatui-core 0.1` | `crossterm` (event conversion), `widget-extras` (minimap/legend), `pty-tests` (T7) |
| catena-testkit | `catena`, `sha2` | `png` (resvg rasterization for review galleries — heavy, off by default) |

Dev-deps (workspace): `insta` (with `filters`), `proptest`, `criterion`, `portable-pty`, `vt100`, `ratatui` (full, for `TestBackend` in widget tests). No `uuid`, no `tokio`, no `tracing`, no `anyhow` anywhere in published code. Published crates declare semver ranges, not `=x.y.z` pins (`CLAUDE.md`, override 3); `Cargo.lock` is committed.

**Toolchain:** edition **2024**, `rust-version = "1.88"` in `[workspace.package]` and inherited by every crate. 1.88 is `ratatui-core` 0.1.2's own declared `rust-version`, so the edition costs no MSRV; confirm it against the resolved `ratatui-core` at M0 and only raise it deliberately. Edition 2024 is chosen for one gate reason: it compiles each crate's doctests into **one** binary instead of one per doctest, which is the doctest half of the one-test-target rule (§3.1). A CI `cargo msrv verify` job holds the floor. `unsafe_code = "forbid"` is a workspace lint, inherited by all three crates.

**License (owner-ruled):** `AGPL-3.0-only` on all three crates, with commercial exceptions sold separately by the owner (dual-license exclusion play: no closed-source or hosted commercial embedding without a paid exception). Chosen over plain GPL deliberately — identical corporate deterrence, plus the network clause closes the hosted-terminal hole (catena behind ttyd/xterm.js/cloud IDEs counts as conveying). SPDX field: `license = "AGPL-3.0-only"` (not `-or-later`). The seed's sole copyright holder is the repository owner, who licenses it here under the same terms, so no third-party attribution or `NOTICE` file is owed. Dev-dependency note for `catena-testkit`: test binaries are not conveyed, so downstream test-only use triggers no AGPL obligations — documented in its README for cautious legal teams.

**Contribution rider (required from M0, non-negotiable):** `CONTRIBUTING.md` ships in M0 with a DCO sign-off requirement **plus** a contributor license grant giving the repository owner (GitHub `jbrjake`) the right to relicense and sell exceptions. Without it, the first merged outside PR permanently destroys the commercial-exception arm. CI checks for `Signed-off-by` on external PRs.

### 3.1 Workspace scaffold — lands with the first crate

A default cargo workspace drifts into a multi-minute, mostly off-CPU test gate and a 100+ GB `target/` without doing anything unusual. The four causes compound, none announces itself, all four are cheap to prevent and expensive to retrofit. They land in the same commit as the first crate.

**1. Debug-info profile.** Cargo's default `debug = 2` writes full DWARF for the workspace *and every dependency*:

```toml
[profile.dev]
debug = "line-tables-only"
split-debuginfo = "unpacked"

[profile.dev.package."*"]
debug = false
```

**2. Warnings fail the gate, not the build.** `RUSTFLAGS` is part of cargo's fingerprint for every unit, so each distinct value keeps its own complete copy of the dependency graph under `target/`. Therefore: **never export `RUSTFLAGS`** (in a script, a hook, a workflow or a shell profile), **never put `-D warnings` or any `rustflags` in `.cargo/config.toml`**, and never `#![deny(warnings)]` in source. Lint levels live in `[workspace.lints]`; warnings become errors only in the gate's own commands:

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features
```

(`RUSTDOCFLAGS` touches only rustdoc units, so it never forks the build or test artifacts.) `.cargo/config.toml` holds only `[alias] xtask = "run --package xtask --"`.

**3. One test target per crate.** Cargo compiles *every* `.rs` directly under `tests/` (and `benches/`) into its own crate and its own statically linked binary; each costs a full link and, on macOS, a first-exec stall while the OS validates the new ad-hoc-signed binary (a process sampled in that stall sits in `_dyld_start` at 0% CPU). So each crate has exactly one integration-test target, `tests/<crate>/main.rs`, with each topic a `mod` in that directory, and at most one bench target, `benches/<crate>/main.rs`. Make it structural: `autotests = false` and `autobenches = false` in every crate, with one explicit `[[test]]` and one `[[bench]]`. Unit tests stay `#[cfg(test)]` modules inside the lib (the seed's `*_tests.rs` files attach as `#[cfg(test)] #[path = "…_tests.rs"] mod tests;`). Edition 2024 merges doctests (above).

**4. `target/` outside indexed folders (macOS).** On a Mac, set `CARGO_TARGET_DIR` in the shell profile to a path outside `~/Documents` and any other Spotlight-indexed tree, or every object cargo writes is queued for metadata import. Never set it in a script or config file. Linux machines, including cloud sessions, are unaffected.

The manifests that satisfy all four:

```toml
# Cargo.toml (root)
[workspace]
members = ["catena", "catena-ratatui", "catena-testkit", "xtask"]
exclude = ["fuzz"]
resolver = "3"

[workspace.package]
edition = "2024"
rust-version = "1.88"
license = "AGPL-3.0-only"
repository = "https://github.com/jbrjake/catena"

[workspace.lints.rust]
unsafe_code = "forbid"

[workspace.lints.clippy]
all = { level = "deny", priority = -1 }
pedantic = { level = "warn", priority = -1 }

[profile.dev]
debug = "line-tables-only"
split-debuginfo = "unpacked"

[profile.dev.package."*"]
debug = false
```

```toml
# catena/Cargo.toml (same shape for catena-ratatui and catena-testkit)
[package]
name = "catena"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
autotests = false
autobenches = false

[lints]
workspace = true

[[test]]
name = "catena"
path = "tests/catena/main.rs"

[[bench]]
name = "catena"
path = "benches/catena/main.rs"
harness = false
```

`cargo xtask lint` (§17) checks rules 2 and 3 mechanically, so they cannot erode: it fails on any `RUSTFLAGS` in `scripts/`, `.githooks/` or `.github/`, on any `rustflags` in `.cargo/config.toml`, and on any crate with more than one target under `tests/` or `benches/`.

**Diagnosing a slow gate later:** check these four before profiling anything — count direct `.rs` children of every `tests/` and `benches/`; `grep profile.dev Cargo.toml`; `grep -r RUSTFLAGS scripts .githooks .github`; sample a stalled test binary (an all-`_dyld_start` stack means the cost is code signing, not code).

---

## 4. Graph data model

### 4.1 Keys and indices

```rust
/// User-facing node identity. Uuid, u64, String, &'static str all qualify.
pub trait Key: Clone + Eq + std::hash::Hash + Ord + std::fmt::Debug + 'static {}
impl<T: Clone + Eq + std::hash::Hash + Ord + std::fmt::Debug + 'static> Key for T {}

// Internal dense indices — all hot paths run on these, never on K.
pub(crate) struct NodeIx(u32);
pub(crate) struct EdgeIx(u32);
```

**Decisions:**
- `Clone`, not `Copy`, so `String` keys work. Keys are touched only at the API boundary; the first thing `GraphStore` does is intern them to `NodeIx`. Hot loops (layout iterations, rasterization, hit-testing) are index-only, so key choice has zero per-frame cost.
- **`Ord` is required and is the determinism backbone.** The seed's hardest-won lesson (documented twice in `seed/graph/layout_fr.rs`): ordering by freshly generated UUIDs breaks run-to-run reproducibility. Every internal iteration that affects output order sorts by `(sort_key, key)` where `sort_key: Option<String>` is user-suppliable (e.g., a display name). Test §16.2-D pins this.
- Parallel edges and self-loops are **first-class in the model from day 1** (the seed silently collapsed both). Rendering fans parallel edges and draws self-loops from v1 (§7.3); the model never dedups.

### 4.2 Store and mutation API

```rust
pub struct GraphView<K: Key> { /* store + positions + controller + caches */ }

pub struct NodeSpec {
    pub label: String,              // display text; measured via unicode-width, never .len()
    pub sort_key: Option<String>,   // stable ordering key; defaults to label
    pub weight: f32,                // layout mass, default 1.0
    pub pinned: Option<(f64, f64)>, // fixed world position, excluded from simulation
    pub shape: NodeShape,           // Label | Box { min_w, min_h } | Glyph(char)
}

pub struct EdgeSpec {
    pub weight: f32,                // attraction multiplier / routing priority
    pub directed: bool,
    pub class: EdgeClass,           // Solid | Dashed | Curved — resolved to style via StyleResolver
    pub layout_participating: bool, // false = drawn but exerts no forces (the seed's "overlay edges")
}

impl<K: Key> GraphView<K> {
    pub fn update(&mut self, f: impl FnOnce(&mut Tx<'_, K>));
}
// Tx: add_node / remove_node / add_edge / remove_edge / set_node / set_edge
```

The transaction computes a **delta classification** on commit — the mechanism behind the seed's gem "enrichment updates never move the graph":

- `Topology` (nodes/edges added/removed, `layout_participating` flipped) → schedules incremental relayout (warm start, §8.3).
- `Property` (label text within the same measured width, weight, class) → re-render only, positions untouched.
- `Geometry` (label measured width changed, shape changed) → local re-snap of affected nodes only; full relayout only if collision resolution cascades past a threshold (8 displaced nodes).

**Interop:** `GraphView::from_petgraph(&Graph<N, E>, key_fn, spec_fn)` behind the `petgraph` feature. No petgraph types in the core API.

---

## 5. Geometry: the single source of truth

This section exists because four of the seed's confirmed bugs (hit-test vs render width disagreement; expanded-overlay width silently discarded; pin marker overrunning its mask; edge endpoints vs collision boxes) share one root cause: **multiple subsystems independently computing node size.**

**The rule:** exactly one function measures a node, exactly once per (node, semantic-zoom level), and every consumer reads the result from one struct.

```rust
/// Computed once per zoom-level change or geometry delta; cached.
pub(crate) struct ResolvedMetrics {
    /// Per node: full cell-space bounding box INCLUDING decorations
    /// (brackets, pin marker, glyph prefix). Nothing renders outside it;
    /// nothing smaller is reserved for it.
    boxes: Vec<CellBox>,          // indexed by NodeIx
    anchor: Vec<(i32, i32)>,      // edge-attachment center, derived from boxes
    zoom_level: SemanticZoom,
}
```

Consumers — force-layout collision terms, grid-snap reservation, edge endpoint computation, label-mask construction, hit-testing and the renderer — all take `&ResolvedMetrics`. None of them may call the measuring function directly (enforced: the measuring fn is private to `geometry/`). Width measurement uses `unicode-width` display columns exclusively; `str::len()` on display text is denied by `cargo xtask lint` (§17) — this retires the seed's entire byte-vs-char bug family (six sites found).

**Semantic zoom** (harvested thresholds from `seed/graph/zoom.rs`, `zoom_to_semantic` and `visual_label_width`; now a configurable table with these defaults):

| zoom ≤ | level | node form | width cap (display cols) |
|---|---|---|---|
| 0.30 | 0 | single glyph | 1 |
| 0.35 | 1 | `[A…]` | 5 |
| 1.5 | 2 | `[truncated]` | 14 |
| 2.5 | 3 | | 22 |
| 3.5 | 4 | | 34 |
| ∞ | 5 | full label | unlimited |

Decoration rule (fixes the pin-marker overrun): decorations are drawn **inside** the measured box — a pinned node at width cap renders `*[trunc…]` by shortening the label, never by growing past the box.

---

## 6. Viewport and coordinate pipeline

Harvested architecture, with two fixes.

```
world space (f64, layout output, y-units = x-units)
  → GridSnapper: ISOTROPIC scale × cell-aspect correction → canonical cells (i32)
  → per-frame derive: (c − margin) × (zoom / ref_zoom) + margin + pan  → view cells
  → blitter transform: ×SUB_W horizontal, ×SUB_H vertical (+SUB_H/2)  → sub-cell pixels
```

- **Canonical/derived split (harvested):** layout runs once into pan-independent canonical positions at `ref_zoom`; pan and zoom are O(n) derivations per frame and **never invalidate layout**. Full relayout triggers only on: topology delta, viewport resize, semantic-zoom level transition.
- **Fix 1 — isotropic snap.** The seed normalizes x and y independently (`snap_to_grid` in `seed/graph/layout_fr.rs`), stretching the layout to fill the viewport and destroying the force-directed metric. `GridSnapper` defaults to `Fit::Contain`: uniform scale `s = min(usable_w / world_w, usable_h / world_h)` with the 0.5 cell-aspect factor applied to y *before* fitting, centered with letterboxing. `Fit::Stretch` is offered as an explicit opt-in.
- **Fix 2 — clip, don't drop.** Positions are signed; off-viewport nodes keep coordinates so edges aim at them (harvested intent). The seed then skips any node with a negative coordinate entirely — a node one cell off-screen pops out of existence. `catena` clips node boxes and labels to the viewport rectangle (per cell) and clips edge segments with **Cohen–Sutherland before rasterization** (the seed rasterizes a ±30k-cell segment pixel by pixel to use ~200 of them).
- **Grid snapping** (harvested): degree-descending placement (hubs claim cells first), 4-direction expanding spiral collision resolution (≤ 50 rings), minimum 2-row vertical gap so edges stay visible between stacked nodes. Two fixes: off-screen nodes participate in collision resolution (the seed reserves their cells but never resolves them), and the spiral is one shared function (the seed has two copies, in `snap_to_grid` and `radial_layout`).
- **Zoom mechanics** (harvested from `zoom_in`, `zoom_out`, `adjust_pan_for_zoom` and `tick_zoom_animation` in `seed/app/navigation.rs`): continuous zoom clamped [0.1, 4.0]; keyboard step 0.25, scroll step 0.08; **exponential smoothing** toward the target (retargeting mid-flight mutates the target — right for touchpad event storms); zoom-toward-cursor keeps the anchor cell fixed via `pan′ = pan − (anchor − margin − pan)(ratio − 1)`. Smoothing is time-injected: `α = 1 − exp(−dt/τ)`, τ = 40 ms. For calibration: the seed moves 35% of the remaining distance per tick, which is τ ≈ 39 ms at 60 fps and τ ≈ 116 ms at its own ~50 ms tick cadence; τ is a feel judgment (§16.9), and the doc comment on the seed's `ZoomAnimation` overstates the per-frame progress (it describes 65% per frame; the code moves 35%).
- **Anchor compensation on relayout** (harvested): capture the focused node's view position before relayout, shift pan by the delta after, so relayout never teleports the graph. Contract test: ≤ 3 cells drift (§16.2-G).
- **Occlusion-aware centering** (harvested from `effective_visible_center` in `seed/app/navigation.rs`): `visible_center(viewport, occlusions: &[Rect])` — the area-weighted centroid of the unoccluded region, so "center on node" centers in what the user can actually see. In `catena` the occlusion list is host-supplied (the host knows its floating windows), keeping the library windowing-agnostic.

---

## 7. Rendering stack

### 7.1 Surface abstraction

```rust
/// The only thing catena writes to. The widget crate adapts this to ratatui Buffer.
pub trait Surface {
    fn size(&self) -> (u16, u16);
    fn put(&mut self, x: u16, y: u16, glyph: char, style: CellStyle);
    fn patch_bg(&mut self, x: u16, y: u16, bg: Rgb);   // glow halos preserve fg
}
pub struct CellStyle { pub fg: PaletteColor, pub bg: Option<PaletteColor>, pub attrs: Attrs } // Bold, Dim, Reversed, Italic
```

`PaletteColor` is `Rgb(u8,u8,u8) | Ansi(u8) | Indexed(u8)` — resolved to ratatui `Color` only in the adapter. A `StringSurface` implementation in core gives free static export (`to_string()` / `to_ansi_string()`) — the seed's `render_to_string` pattern (`seed/ui/dag_layout/render_ascii.rs`) generalized, and the substrate for golden tests.

### 7.2 Blitters

```rust
pub enum Blitter { Braille, HalfBlock, Sextant, Ascii }
```

| | sub-res/cell | colors/cell | notes |
|---|---|---|---|
| `Braille` | 2×4 | 1 fg | default; harvested canvas incl. the correct bit-6/7 bottom-row table (`BRAILLE_MAP`) |
| `HalfBlock` | 1×2 | 2 (fg+bg) | when per-node/edge color fidelity beats resolution |
| `Sextant` | 2×3 | 1 fg | Unicode 13; better font fidelity than braille in several terminals |
| `Ascii` | 1×1 | 1 | direction-quantized `─│╱╲` + junction glyphs; universal fallback, and what `NO_COLOR`/dumb terminals get |

One `SubCellCanvas` (flat `Vec<u8>` mask + parallel `Vec<ColorSlot>`) parameterized by `(SUB_W, SUB_H)`; the blitter only encodes the final mask → char. All harvested primitives on `BrailleCanvas` (`draw_line`, `draw_line_with_hops`, `draw_dashed_line` with its zero-period guard, `draw_bezier`/`draw_bezier_ctrl`, `draw_dashed_bezier`/`draw_dashed_bezier_ctrl`, `draw_circle`) operate on the canvas generically. The four near-identical Bézier sampling loops in the seed collapse to one sampler with a plot callback. **Sampling density kept:** one sample per 2 sub-pixels, `steps = (chord/2).clamp(10, 200)`; the seed's comment about dash phase by sample index coarsening on short curves moves into the doc comment verbatim.

**Per-cell color policy for 1-color blitters (decided):** within one canvas, last writer wins per cell (cheap, deterministic given the sorted draw order); across canvases the compositor decides (§7.4). The chord overlay's sector-paint trick (colorize lit cells by angular sector after drawing) is kept for the radial view, where per-edge canvases would be wasteful.

### 7.3 Edge rendering (decided details)

- **Straight edges** braille-rasterized; `EdgeClass::Curved` gets the 20%-perpendicular quadratic bow.
- **Crossing hops** (harvested from `segment_intersection`, `HOP_RADIUS` and `render_edges` in `seed/graph/render.rs`): segment-segment intersection with near-endpoint exclusion `t,u ∈ (0.02, 0.98)`; the more-horizontal edge takes the gap (`HOP_RADIUS = 3` sub-pixels ≈ 1.5 cells, the PCB convention), endpoints never suppressed. **Fixed scaling:** candidate pairs come from the spatial grid (§10.2) instead of the all-pairs O(E²) scan — bucket-local pairs only, with a per-frame hop budget (`max_hops = 256`, after which remaining crossings render plain) so worst-case frames degrade gracefully instead of stalling.
- **Parallel edges** fan with perpendicular offsets (±2 sub-pixels per rank, capped at 3 ranks, then a `×n` count badge at the midpoint).
- **Self-loops** render as a 3-cell-radius circle arc anchored at the node box's top-right corner, arrowhead per direction.
- **Arrowheads** for directed edges: final-2-sub-pixel chevron in braille; `▸▾◂▴` quantized in the Ascii blitter. (The seed has none.)
- **Label mask** (harvested): a flat `Vec<bool>` built from `ResolvedMetrics.boxes` — one builder, used by every canvas blit; edges never overprint text. One mask per frame, replacing the seed's two divergent blit paths (`render_canvas_to_buf` and `render_canvas_to_buf_overwrite` in `seed/graph/render_overlay.rs`, both reading the `label_grid` built in `render_edges`).

### 7.4 Scene graph and compositor — the completeness rule

The DAG review's sharpest architectural finding: the seed's renderer draws geometry its layout never modeled (entity card, bus), and its pipeline drops information (edge color, click columns) that downstream hacks then reconstruct. The fix is structural:

```rust
/// COMPLETE display list. If it's on screen, it's in here. If it's in here, it has bounds + payload.
pub struct SceneGraph<K> { items: Vec<SceneItem<K>> }
pub struct SceneItem<K> {
    pub z: Layer,                  // Background | EdgesUnder | EdgesOver | Glow | Nodes | Labels | Annotations
    pub bounds: CellBox,
    pub payload: Payload<K>,       // NodeBox{key} | EdgePath{key, points} | Junction{..} | Label{..} | Decoration{..}
    pub style: StyleId,
}
```

- **Layout emits SceneGraph. The renderer interprets it and adds nothing.** Every renderable concept — including layered-layout connector bars and any future "card" — must be a scene item, or it cannot be drawn.
- **Hit-testing consumes the same SceneGraph** (§10.2). Click targets carry full bounds — the row-only click-target defect class (three sites in the seed) cannot exist.
- **The compositor owns z-order.** Fixed layer enum; within a layer, insertion order (which is deterministic, §4.1). Sub-cell canvases exist per layer; **braille bits OR-merge across layers** before glyph encoding, with color from the topmost layer contributing a bit in that cell — this fixes the seed's "overlay edge visually broken at crossings" (its paint-behind blit drops lower-layer dots entirely). The seed's three ad-hoc conventions (a write-only-if-blank grid helper in its DAG grid renderer, which is not seeded; a `symbol == " "` guard; the overwrite blit `render_canvas_to_buf_overwrite`) are replaced by this one policy. Glow halos use `patch_bg` so the halo background composes under edge foregrounds (the seed's `dim_color` trick in `seed/graph/render.rs`, now a Surface primitive).

---

## 8. Force-directed layout

The seed's Fruchterman-Reingold core (`layout` in `seed/graph/layout_fr.rs`) with its tuning intact, plus four upgrades.

### 8.1 Parameters (defaults = the seed's proven values)

```rust
pub struct ForceParams {
    pub iterations: u32,          // 100 — RESPECTED as given (the seed silently floors to 100; presets carry the wisdom instead: Preset::Quality=100, Preset::Fast=30)
    pub cooling: f64,             // 0.95 multiplicative
    pub gravity: f64,             // 0.10 toward running centroid
    pub theta: f64,               // 0.80 Barnes-Hut opening criterion
    pub bh_threshold: usize,      // 500 — brute-force below (research: quadtree overhead loses < ~1k)
    pub repulsion: Repulsion,     // Uniform (FR classic) | DegreeScaled (FA2-flavored; better cluster legibility)
    pub converge_eps: f64,        // 0.5 cells max displacement → early exit
    pub k_label_scale: bool,      // true: k = sqrt(area/n) × max(avg_label_w/4, 1) — settles at label-surviving distances
}
```

Kept mechanics: position-based Euler with temperature clamp (no velocity — stability comes from warm-starting); displacement `min(‖F‖, T)` with `‖F‖` floored at 0.01; initial placement on a circle of radius `sqrt(n)·label_w` with y×0.5 aspect pre-correction; quadtree coincident-point jitter + `MAX_DEPTH` valve.

**Quadtree fix (harvested bug, `seed/graph/quadtree.rs`):** at `MAX_DEPTH` a node can accumulate mass with no body and repel itself (`body.is_none()` escapes the self-skip in `compute_force`); jittered points can land outside their routed quadrant's bounds. Both are fixed in the harvested copy with regression tests; the loose force-ratio invariant test (approx/exact ∈ (0.5, 2), `theta_approximation_reasonable`) is kept and tightened to (0.7, 1.4) at θ = 0.8.

### 8.2 Disconnected components and the peripheral ring

Harvested (`radial_layout`, `centroid`, `max_distance_from`): nodes with no layout-participating edge go to a radial ring around the force core — centered on the **centroid** (not the bbox midpoint), radius `(max_dist + 3)/2 × 1.1`, ordered by ideal angle toward each node's neighbor centroid but placed at uniform `2π/n` spacing (crossing reduction without label pile-ups). New: multiple non-singleton components get separate force islands packed left to right by descending node count with 4-cell gutters, replacing the seed's implicit single-blob assumption.

### 8.3 Warm start and stability (the incremental contract)

Harvested: surviving nodes seed at previous positions, `T₀/8`, 50 iterations; deleted nodes' positions dropped; property deltas never touch positions. Added from the dynamic-layout literature: **new-node ramp-in** — a node added to a warm layout enters with force influence scaled linearly 0 → 1 over its first 10 iterations, so one arrival doesn't shove the neighborhood (the published technique for online dynamic graphs, and it directly serves ratatui's diff-based output: fewer moved cells per frame).

Contract (tested, §16.2-F): after a warm relayout triggered by adding ≤ 5% new nodes, ≥ 90% of surviving nodes move ≤ 2 cells.

---

## 9. Layered (DAG) layout — the real Sugiyama pipeline

Replaces the seed's 3-layer special case. All phases operate on `NodeIx` graphs with node widths in cells from `ResolvedMetrics`.

### 9.1 Pipeline (all decided)

1. **Cycle removal:** greedy Eades–Lin–Smyth (own ~60-line implementation; guarantees ≤ m/2 − n/6 reversals). Reversed edges are marked and restored at scene emission so arrowheads point true.
2. **Layering:** longest path from sources, then a pull-up tightening pass (shrink slack edges). Layer count unbounded; the caller may pin nodes to layers (`rank: Option<u32>` in a `LayeredSpec`) — this keeps the seed's "caller knows the strata" use case as an *option* instead of the only mode.
3. **Dummy chains** for edges spanning > 1 layer; chains carry their `EdgeIx` so routing and styling survive (the anti-"silently dropped edge" guarantee — invariant §16.2-B: every input edge appears in the SceneGraph).
4. **Crossing minimization:** alternating down/up **barycenter** sweeps with a **median** fallback pass; after each full sweep, count crossings exactly (bilayer counting via Fenwick tree, O(E log E)); keep the best-seen ordering; stop after 4 sweeps or zero improvement. Ties break by `(sort_key, key)` — determinism, not a sort-stability accident.
5. **X-coordinate assignment:** v1 = **priority method** (Sugiyama '81): three median-alignment sweeps, node priority = degree (dummies = ∞ so long edges straighten), honoring real cell widths + 2-cell min separation. Deliberately chosen over Brandes-Köpf for v1: simpler, integer-friendly, handles variable node widths naturally. v1.1 upgrades to BK **with the 2020 erratum** behind the same interface.
6. **Y-coordinate assignment:** the harvested `flex_distribute` (`seed/ui/flex_layout.rs`: min/preferred/max, three proportional phases, deterministic remainder, `debug_assert`ed invariants) allocates layer bands and routing channels.
7. **Edge routing:** orthogonal, on reserved inter-layer channels. **Every merge/fork group gets its own channel row** (allocated by `flex_distribute`, assigned in leftmost-column order) — fixing the seed's shared bar row. Vertical pipes drop from the parent box's **actual bottom** to the bar, the bar runs to the child column, and the pipe drops to the child box's **actual top** — geometry always from measured content boxes, never from allocated band midpoints. This was the root cause of the seed's worst DAG rendering glitches and is now a stated invariant: **route from content bounds, never track bounds.** Junction glyphs come from the harvested position × continuation table (`JunctionKind` in `seed/ui/dag_layout/models.rs`, used by `build_merge_bar_with_continuation` in `routing.rs`: `└ ┴ ┘ ├ ┼ ┤ ┬`); crossings get the theme's crossing glyph. Style flows: every routed segment carries its `EdgeIx` → `StyleId` (the anti-"edge color discarded" guarantee; no raster re-scanning, ever).
8. **Scene emission:** boxes, pipes, bars, junctions, arrowheads, labels — all as SceneItems with bounds and payloads.

### 9.2 Why own implementation instead of rust-sugiyama / elk-rs (decided)

`rust-sugiyama` would bring petgraph into the required dep tree, targets continuous coordinates (we need integer cells + width-aware separation), and does no channel routing or glyph junctions. `elk-rs` is a heavyweight full-ELK port — wrong size for a core dependency. The pipeline above is ~1,200 lines against the harvested flex allocator and junction table; owning it keeps the integer-cell tuning loop tight. Both crates remain reference implementations for cross-checking test fixtures.

### 9.3 Tree and radial

- **TreeLayout:** the harvested centered-parent algorithm (`tree_layout` in `seed/graph/tree_layout.rs`: post-order widths, BFS depths, pre-order x, O(n), deterministic) — honestly documented as naive Reingold-Tilford-lite, *not* Walker (the seed's module header mislabels it). Fine at terminal scale; proper Walker/Buchheim is a v2 item if deep-tree demand appears. Its orthogonal stem/bus/drop connector routing (`route_connectors`) feeds the same scene/junction machinery as §9.1.
- **RadialLayout:** ring placement (§8.2 machinery) + the optional two-level **chord/Holten** overlay harvested intact from `seed/graph/chord.rs`: β = 0.85 bundling (`BUNDLING_BETA`), 3° arc gaps (`ARC_GAP_RAD`), size-descending arcs with gap clamping (`usable ≥ π`), C⁰-continuous chained quadratics through the bundle root, dashed intra-group vs solid inter-group. Group ids are plain `u32` (the seed's chord code is already key-agnostic; only its `community_*` naming gets renamed `group_*`).

---

## 10. Interaction

### 10.1 Event model and controller

```rust
pub enum InputEvent {              // library-native; From<crossterm::event::Event> behind feature
    Key(KeyInput), MouseDown(u16,u16,Button), MouseUp(..), MouseMove(..),
    Scroll(i8, u16, u16), Resize(u16, u16),
}
pub struct Controller<K> { /* owns: viewport, selection, hover, pin, drag state, animations */ }
impl<K: Key> Controller<K> {
    pub fn handle(&mut self, ev: InputEvent, scene: &SceneGraph<K>) -> Vec<Outcome<K>>;
    pub fn tick(&mut self, dt: std::time::Duration) -> Redraw;   // ALL time enters here
}
pub enum Outcome<K> { Selected(Option<K>), Hovered(Option<K>), EdgeSelected(Option<K2>), Pinned(K, bool), ViewChanged, DragCompleted(K, (f64,f64)) }
```

- The controller is a plain state machine over `(SceneGraph, ResolvedMetrics)`; **no `Instant::now()` anywhere in the library** (enforced by `cargo xtask lint`, §17). Hosts call `tick(dt)` each frame; tests call `tick(16ms)` deterministically. This deletes the seed's animation-backdating test hack (`PanAnimation::force_complete` in `seed/app/types_excerpt.rs` rewinds a wall-clock start time).
- `Outcome` events are how the host reacts (egui_graphs' event-channel pattern, minus the channel — a returned Vec is simpler and allocation-cheap).
- Key bindings are a default table (`hjkl`/arrows pan, `+/-` zoom, `n/N` cycle, `Shift+arrows` spatial nav, `Enter` pin, `Esc` clear), fully remappable; the widget crate ships the table, core exposes semantic `Action`s.
- Node drag is new (the seed drags windows and the minimap viewport, never nodes): a drag moves the node's canonical position, marks it pinned for the drag's duration, and emits `DragCompleted` on release; the host decides whether the pin persists.

### 10.2 Hit-testing (spec, fixing the seed's four defects)

The seed's hit-testing is `select_entity_at`, `hover_entity_at`, `select_edge_at` and `hover_edge_at` in `seed/app/selection_excerpt.rs`, over the `EdgeSpatialGrid` and `EdgeScreenPos` in `seed/app/types_excerpt.rs`.

1. **Nodes:** point-in-box against `ResolvedMetrics.boxes` (visual boxes — the same ones rendered; the seed's low-zoom "click selects a neighbor" bug is structurally impossible). Miss → nearest anchor within radius 3 cells using **aspect-corrected distance** `d² = dx² + (2·dy)²`. Node lookups go through a bucket grid (same 16×8-cell design as edges); no O(n) scans on mouse move.
2. **Edges:** the harvested spatial bucket grid (bboxes inflated by the hit margin so boundary edges register in neighbor buckets), point-segment distance ≤ 1.5 cells in cell space. Sub-cell (braille) precision is deliberately **not** used for hit-testing — a 0.5-cell-accurate click target is below pointer precision in a terminal; documented as a decision, not an accident.
3. **Priority:** topmost SceneGraph layer wins (nodes over edges); within a layer, the smallest bounds win (the most specific target).
4. **Every interactive SceneItem carries full 2-D bounds** — the row-only click table cannot recur. `catena` does not support rendering through wrapping widgets: the scene is positioned in cells and the widget renders it directly (the seed's visual-row reconstruction existed only because it drew through `Paragraph::wrap`).

### 10.3 Spatial keyboard navigation (new capability, decided algorithm)

From the focused node, direction d ∈ {up, down, left, right}: candidates are nodes whose aspect-corrected center offset has a positive component along d within a ±60° cone; score = `‖offset‖ₐ · (1 + 2(1 − cos θ))` (distance penalized by angular deviation); lowest score wins; an empty cone falls back to the nearest node in the half-plane. Wraps nothing (edge of graph = no-op, `Outcome::Boundary`). `n`/`N` keeps list-order cycling (sorted by `(sort_key, key)`), because both navigation styles have users.

### 10.4 Animations

All animations (pan glide 300 ms ease-out cubic, as in the seed's `PanAnimation::tick`; zoom exponential smoothing τ = 40 ms; layout ramp-in) advance exclusively in `tick(dt)` and are interruptible by retargeting. The `Redraw::Needed`/`Idle` return lets hosts skip rendering on quiet frames (the seed's dirty-flag economy, without owning the loop).

---

## 11. Stability contract (live-graph guarantees)

Published in the crate docs as testable guarantees, because live updating is the differentiator against all static prior art:

1. Property-only deltas move zero nodes (exact).
2. Pan/zoom never trigger relayout (exact).
3. Warm relayout after ≤ 5% node additions: ≥ 90% of survivors move ≤ 2 cells (statistical, seeded fixture suite).
4. Relayout keeps the focused node within 3 cells of its prior view position (anchor compensation).
5. Identical input + identical seed ⇒ byte-identical frames across runs and platforms (full determinism; the RNG is a seeded `SplitMix64` owned by the layout, never `rand::thread_rng`).
6. A quiescent scene re-rendered N times produces zero cell diffs (no hidden time/iteration-order dependence).

---

## 12. Styling and theming

```rust
pub trait StyleResolver<K> {
    fn node(&self, key: &K, state: NodeState) -> NodeStyle;   // state: selected/hovered/pinned/dimmed/dragging
    fn edge(&self, key: &EdgeKey<K>, state: EdgeState) -> EdgeStyle;
    fn group(&self, group: u32) -> Rgb;                        // radial arcs, hull tints
}
```

- The resolver is the **only** place app meaning becomes color — the seed's 9-variant domain `ColorPalette` and 11-entity-type theme table become one user callback. `catena` ships `DegreeResolver`, `GroupResolver` (8-color palette with offset rotation — the seed's trick for distinguishing overlapping palette families) and `MonoResolver` as examples.
- The harvested **gradient library** (`seed/graph/render.rs`: `confidence_color` red → yellow → green, `centrality_color` blue → cyan → yellow → red with soft cap, `pagerank_color`, `dim_color` = RGB/4) ships as pure `fn(f64) -> Rgb` helpers in `style::gradients` — including the harvested **visible-range renormalization** lesson (normalize a metric to the visible set's min/max or the gradient collapses; the helper takes `(value, visible_min, visible_max)`).
- `Theme` = semantic color slots (background, edge_default, edge_highlight, glow, selection, letterbox) + `GlyphSet` (junctions, arrowheads, box drawing, dash patterns) + sizing. Four built-ins: `dark`, `light`, `ansi16` (16-color fallback, ported from `seed/ui/themes/ansi16.rs`), `ascii` (new: a pure-ASCII glyph set that proves the indirection and serves `NO_COLOR` and dumb terminals). Selected node = reversed video; pinned = in-box `*` marker; dimmed = attr + color dim (all the seed's conventions).
- Glyphs are `char`, not `&str` (the seed's multi-codepoint glyph slots silently truncate in 12 call sites; a single `char` makes the truncation unrepresentable). Wide-glyph support is explicitly not a theme feature.

---

## 13. Public API — the 60-second view

```rust
use catena::{GraphView, ForceParams, LayoutKind, Blitter, InputEvent};

let mut gv: GraphView<String> = GraphView::builder()
    .layout(LayoutKind::Force(ForceParams::default()))
    .blitter(Blitter::Braille)
    .theme(catena::theme::dark())
    .seed(42)
    .build();

gv.update(|tx| {
    tx.add_node("ada", NodeSpec::label("Ada Lovelace"));
    tx.add_node("babbage", NodeSpec::label("Charles Babbage"));
    tx.add_edge("ada", "babbage", EdgeSpec::directed());
});

// host event loop:
for outcome in gv.handle(InputEvent::from(crossterm_event)) { /* react */ }
gv.tick(dt);
frame.render_stateful_widget(catena_ratatui::Graph::new(), area, &mut gv);   // ratatui
// or, no terminal at all:
let ascii: String = gv.render_to_string(120, 40);
```

---

## 14. Bug-prevention ledger

Every confirmed seed defect, mapped to the design element that makes it impossible or fixed. This table is the acceptance checklist for the harvest: each row gets a regression or property test.

| # | Seed defect | catena countermeasure |
|---|---|---|
| 1 | Hit-test uses full label width; render uses visual width (low-zoom clicks miss) | Single `ResolvedMetrics` consumed by both (§5) |
| 2 | Expanded-overlay width computed then discarded (`let _ = width;` in `seed/graph/mod.rs`) | SceneGraph completeness: rendered box == scene bounds == reserved box (§7.4) |
| 3 | Byte-vs-char length in 6 sites (non-ASCII shears layout) | `unicode-width` everywhere; `xtask lint` denies `.len()` on display text (§5, §17) |
| 4 | Pin marker overruns the reserved box by one cell | Decorations render inside the measured box (§5) |
| 5 | O(E²) all-pairs crossing detection per frame (`render_edges`) | Spatial-grid candidate pairs + per-frame hop budget (§7.3) |
| 6 | Parallel edges silently collapsed; self-loops draw a single pixel | First-class multi-edge fanning + self-loop arcs (§4.1, §7.3) |
| 7 | Off-screen nodes reserve cells but skip collision resolution | Off-screen nodes fully participate in snapping (§6) |
| 8 | Partially visible nodes dropped entirely (pop at pan boundary) | Per-cell clipping of boxes/labels; Cohen–Sutherland for segments (§6) |
| 9 | Anisotropic snap stretch destroys the FR metric (`snap_to_grid`) | Isotropic `Fit::Contain` default with aspect pre-correction (§6) |
| 10 | Zoom rescale unsound over collision displacements | Re-snap (cheap, not re-simulate) on zoom within a semantic level when collision displacement was nonzero |
| 11 | `iterations` parameter silently floored to 100 | Params respected; the wisdom lives in named presets (§8.1) |
| 12 | Quadtree self-repulsion at `MAX_DEPTH`; jitter escaping quadrant bounds | Fixed in the harvest + regression tests (§8.1) |
| 13 | `BrailleCanvas::get_cell` panics unchecked while its sibling accessors clip silently | One contract: all canvas accessors clip; `try_` variants return `Option` |
| 14 | Unbounded rasterization of mostly off-screen segments | Clip before rasterizing (§6) |
| 15 | Paint-behind blit breaks overlay edges at crossings | Compositor OR-merges sub-cell bits across layers (§7.4) |
| 16 | Theme symbol/color tables out of sync (10 vs 6 entries) | No domain tables; one `StyleResolver` callback (§12) |
| 17 | DAG: layer-skipping edges silently dropped | Dummy chains; invariant test: every input edge reaches the scene (§9.1) |
| 18 | DAG: all merge bars share one row | Per-group channel rows via `flex_distribute` (§9.1) |
| 19 | DAG: edge color discarded, reconstructed by raster scanning | `StyleId` flows through routing (§9.1) |
| 20 | DAG: renderer draws card/bus geometry the layout never modeled | SceneGraph completeness rule (§7.4) |
| 21 | Row-only click targets (leftmost box wins) | SceneItems carry 2-D bounds (§10.2) |
| 22 | Routing from flex-zone midpoints, not content bounds (pipe gaps) | Stated invariant: route from measured content bounds (§9.1) |
| 23 | Three ad-hoc z-order conventions | One compositor layer policy (§7.4) |
| 24 | Operator-precedence bug in a text-highlight fallback (`!a && b ‖ c`) | Not carried: text highlighting is a host concern |
| 25 | Duplicated spiral/label-grid/minimums/style-run code drifting apart | Single implementations by construction; file-size caps (§17) |
| 26 | `Instant::now()` scattered through animation paths (untestable time) | All time via `tick(dt)`; wall-clock reads denied by `xtask lint` (§10.4, §17) |

---

## 15. Performance

### 15.1 Budgets (targets, enforced by criterion benches)

| Operation | Scale | Budget |
|---|---|---|
| Cold force layout | 200 nodes / 400 edges | ≤ 50 ms |
| Warm force relayout | same, ≤ 5% delta | ≤ 10 ms |
| Cold force layout | 1,000 / 2,000 | ≤ 400 ms |
| Full frame render (layout cached) | 200/400 on 200×50 cells | ≤ 3 ms |
| Full frame render | 1,000/2,000 on 200×50 | ≤ 16 ms |
| Sugiyama full pipeline | 100 nodes / 4 ranks / 200 edges | ≤ 25 ms |
| Hit-test (mouse move) | 1,000 nodes | ≤ 50 µs |
| `update()` property delta | 1,000 nodes | ≤ 100 µs |

Rationale: at a 50 ms input-poll cadence the render budget leaves ≥ 45 ms headroom; 1k nodes is the honest ceiling of what a 200×50 terminal can *display* distinguishably (grid saturation, per the review's estimate), so we optimize to be comfortably fast at the display ceiling rather than chasing sizes the medium can't show.

**The regression ratchet** (from the first bench, M2): `scripts/check-perf.sh` runs the crate's bench target, takes the median of ≥ 7 runs after warm-up plus peak RSS, and compares against this machine's entry in the committed `bench-baseline.json` (one entry per machine fingerprint: host, CPU, cores, RAM). A regression past 1.5× fails and needs the owner's approval to accept; a new machine seeds its entry loudly instead of failing; `UPDATE_BASELINE=1` reseeds explicitly. The budgets above are absolute targets checked by the same benches; the ratchet catches drift below them.

### 15.2 Allocation policy

A `FrameScratch` struct pools every per-frame buffer (sub-cell canvases via `clear_and_resize` — `BrailleCanvas::clear_and_resize` exists in the seed and is never called; the label mask; segment/hop vectors; the scene item vec). Steady-state frame = zero heap allocations (asserted in a bench with a counting allocator). No per-frame `String` clones: labels are measured at delta time; the renderer borrows.

---

## 16. Testing strategy

The hard problem, treated as a first-class deliverable. Design principles from the evidence: (a) the source application's ~40-minute functional tier was dominated by its embedded engine and event-loop mirroring — a pure library has neither, so **every tier below runs in seconds except the optional PTY smoke**; (b) the seed's DAG-glitch post-mortem showed screen-scraping tests missed geometry bugs that only positional/property assertions caught — so **property tests outrank snapshots** in the pyramid; (c) ratatui's insta recipe can't assert color — so the **SVG/hash tier is the style oracle**; (d) zellij's named polling predicates and gitui's insta filters are the proven patterns for the end-to-end edge.

All tiers live in each crate's single integration-test target or its lib unit tests (§3.1); a tier is a `mod`, never a new target.

### 16.1 Tier taxonomy

| Tier | Mechanism | Gate | Runtime | Count target at 1.0 |
|---|---|---|---|---|
| T1 Property/unit | proptest + colocated unit tests, pure functions | always (`cargo test`) | seconds | ~400 fns, every invariant in §16.2 |
| T2 Golden text | `StringSurface` → `insta::assert_snapshot!` | always | seconds | ~60 snapshots |
| T3 Style/visual | Surface → SVG → SHA-256 vs committed hash; `.fail.svg` on mismatch | always | seconds | ~30 snapshots + non-gating PNG gallery |
| T4 Determinism/flicker | buffer-diff oscillation detector, steady-state zero-diff, cross-run hash equality | always | seconds | ~15 scenarios |
| T5 Widget composition | ratatui `TestBackend` + insta (widget crate only) | always | seconds | ~15 |
| T6 Fuzz | cargo-fuzz + `arbitrary` event/topology streams | nightly CI job | budgeted 10 min | 3 targets |
| T7 PTY smoke | portable-pty + vt100 against `examples/interactive` | `--features pty-tests`, serialized, own CI job | ~1 min | 5 tests |

No feature-gated 40-minute tier exists because nothing needs one: the expensive things the source application's harness simulated (async fetches, enrichment races, main-loop mirroring) are outside the library by construction.

### 16.2 T1 — the invariant catalog (property tests, proptest with shrinking)

Strategies generate: node counts 0–300, labels including empty/emoji/CJK/combining marks, edge sets including self-loops/parallels/disconnection, zoom ∈ [0.1, 4.0], viewport 3×3 → 300×100, seeded RNG. Named invariants:

- **A. No-overlap:** after snap, no two visible node boxes intersect (assert geometry, not screen text).
- **B. Edge conservation:** every input edge yields exactly one scene path whose endpoints touch its nodes' anchor cells (catches silent drops; would have caught the seed's layer-skip drop and dedup collapse).
- **C. Containment:** every SceneItem's raster output stays within its declared bounds; every rendered cell traces to a SceneItem (completeness both ways).
- **D. Determinism:** same input + seed ⇒ identical scene, across two fresh `GraphView`s and shuffled insertion orders of the same node set (sort-key ordering, not HashMap luck).
- **E. Braille raster trio (harvested verbatim):** every line 8-connected (BFS over lit sub-pixels), endpoints exact, no duplicate plots — across all 8 octants; hop-split segments lose exactly `2r+1` pixels. The helpers are `lit_pixels`, `assert_8_connected`, `assert_endpoints_exact` and `assert_no_duplicates` in `seed/graph/braille_tests.rs`.
- **F. Warm stability:** the §11.3 statistical bound over seeded fixture families.
- **G. Anchor drift ≤ 3 cells** across forced relayouts.
- **H. Sugiyama phase invariants:** post-cycle-removal graph is acyclic; layering respects all edges (dummy chains monotone); crossing count non-increasing per accepted sweep; min separation honored at every layer; every channel row hosts ≤ 1 bar group.
- **I. Snap round-trip:** derived view positions at `zoom == ref_zoom`, `pan == 0` equal canonical positions exactly.
- **J. Hit-render agreement:** for every node, a click at its rendered box center selects it (the bug-1 killer, run across zoom levels).
- **K. Clip equivalence:** rendering a scene into a viewport equals rendering unclipped then cropping (validates Cohen–Sutherland and per-cell clipping).
- **L. Flex allocator:** offsets contiguous, sums exact, min ≤ alloc ≤ max, deterministic remainder (harvested with `seed/ui/flex_layout.rs`'s tests).
- **M. Quadtree:** approx/exact force ratio ∈ (0.7, 1.4) at θ = 0.8; no self-force; coincident-point termination.
- **N. Unicode:** layouts with emoji/CJK labels produce boxes whose rendered display width equals the reserved width (kills the byte/char family for good).
- **O. Degenerate inputs:** zero-period dash, zero-size viewport, single-node graph, empty graph — total absence of panics, each a named regression test.

Norm: each property runs ≥ 256 cases in CI (the proptest default), plus ≥ 7 hand-picked regression seeds pinned per past bug.

### 16.3 T2 — golden text snapshots

The pure `StringSurface` render of curated fixture scenes (the Ascii blitter doubles as the readable-diff renderer): the 12 canonical graphs of §20 × relevant layouts × 2 viewport sizes. `insta` with filters redacting nothing — because the library is deterministic by construction, snapshots need **zero** redaction; any needed redaction is itself a determinism bug (inverting gitui's filter pattern into an assertion of design health). `cargo insta review` is the update workflow; first-run failures are by design.

### 16.4 T3 — style-aware visual snapshots (the color oracle)

Harvested from `seed/tests/visual/svg_renderer.rs` (`buffer_to_svg`, `buffer_to_hash`, `color_to_css`) and `seed/tests/visual/snapshots.rs` (`assert_visual_snapshot`, `save_visual_gallery`) nearly verbatim into `catena-testkit`: Surface → self-contained SVG (run-length spans, REVERSED handled, full color mapping) → canonical per-cell hash line `{glyph}|{fg_css}|{bg_css}|{attrs:x}` → SHA-256 vs committed `.hash`; a mismatch writes `{name}.fail.svg` for eyeball/PR-diff inspection; `CATENA_UPDATE_SNAPSHOTS=1` refreshes. This closes the documented ratatui-ecosystem gap (the insta text recipe can't see color) with the Textual-SVG approach, and it is precisely what a *visualization* crate must test: selection highlight, glow halo composition, dim states, per-layer edge colors, ansi16 fallback fidelity. The `png` feature adds a resvg-rasterized **non-gating** gallery (`buffer_to_png`; `tests/gallery/*.png`) regenerated on demand for human/multimodal review — kept non-assertive by design.

### 16.5 T4 — determinism and flicker

Harvested oscillation machinery (`buffer_diff_details` and `diff_count` in `seed/tests/functional/test_blink_core.rs`), now cheap enough to run always-on:
- **Steady state:** render a quiescent scene 50 consecutive frames (`tick(16ms)` between) → zero cell diffs after frame 2 (catches iteration-order nondeterminism, hidden time reads, canvas-reuse leaks).
- **A→B→A detector:** across interaction scripts (pan burst, zoom glide, select/deselect storm), assert no frame equals frame-2-ago while differing from frame-1-ago (the flicker signature), and that the frame-hash count over a 200-tick script is at most the script's distinct logical states.
- **Cross-run:** two fresh processes (subprocess spawns of `catena/examples/render_hash.rs`) produce identical hashes — catches ASLR-dependent HashMap ordering escaping into output.

### 16.6 T5 — widget composition

ratatui `TestBackend` snapshots of the full widget (graph + minimap + legend in a layout), resize storms (10 random resizes → invariants A + C hold), and the adapter's style translation. This is the only tier that needs ratatui proper.

### 16.7 T6 — fuzzing

Three `cargo-fuzz` targets with `arbitrary`-derived inputs (nightly CI job, 10-minute budget, corpus committed), in the `fuzz/` crate excluded from the workspace:
1. `fuzz_events`: arbitrary `Vec<InputEvent>` (including mid-drag resizes, scroll storms, clicks at `u16::MAX`) against a fixture graph → no panic; invariants A/C/I hold at the end.
2. `fuzz_topology`: arbitrary interleaved `Tx` mutations and renders → no panic, edge conservation holds, no `NodeIx` leaks after removals.
3. `fuzz_layout_input`: arbitrary graphs straight into each layout engine → terminates within iteration bounds, all nodes placed, finite coordinates.

libFuzzer is Unix/nightly: the fuzz job runs on Linux CI only; Windows correctness is covered by T1–T5, which run on all three OSes.

### 16.8 T7 — PTY smoke (the only real-terminal tier)

Five tests against `examples/interactive` via `portable-pty` + `vt100`, serialized (`--test-threads=1`), own CI job, `TERM=xterm-256color` pinned:
1. Launches into the alternate screen; a braille-range glyph appears within 2 s (a named polling predicate à la zellij: `wait_until("graph visible", |screen| ...)` — no fixed sleeps).
2. A pan key produces a changed frame; quiescence returns (steady state via polling buffer equality).
3. Resize (SIGWINCH) rerenders within bounds; no panic.
4. Clean exit restores the main screen; **zero bytes written outside the alternate-screen bracket and zero stderr output during steady state** — the structural test for the startup-clobber bug class (an app writing to the main screen before entering the alternate one).
5. A `NO_COLOR=1` run emits no SGR color sequences (scan raw bytes with the `CaptureBuf` byte-capture pattern from `seed/tests/functional/test_blink_core.rs`, applied at the PTY).

Explicitly **not** built: a `tick()`-mirrors-the-main-loop harness family. The source application needed one only because an embedded engine raced its render loop, and its five-copy fetch-list duplication was its worst maintenance trap; `catena` has no engine and one canonical `tick`.

### 16.9 What is deliberately untested and why

Terminal-emulator glyph rendering fidelity (braille font quality varies by terminal — that's why blitters are pluggable, not something tests can fix); wall-clock animation *feel* (τ values are judgment; the math is tested); ratatui's own diff correctness (upstream's job; T4 ensures we feed it stable frames).

---

## 17. Gates and CI (decided)

**Local gates** (hooks enabled per clone with `git config core.hooksPath .githooks`):

- **`scripts/smoke.sh` — commit gate, under 60 s, scoped to the diff.** `cargo fmt --all -- --check`; `cargo xtask lint`; `cargo clippy -p <touched crates> --all-targets -- -D warnings`; `cargo test -p <touched crates> --lib`. Quiet on success (one OK line with the pass count); on failure prints only the failing tests. Skips loudly, never silently.
- **`scripts/regression.sh` — push gate, correctness only.** `cargo fmt --all -- --check`; `cargo xtask lint`; `cargo clippy --workspace --all-targets --all-features -- -D warnings`; `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features`; `cargo test --workspace -- --include-ignored` (T1–T5). No benchmarks, no fuzzing, no PTY tier.
- Neither script exports `RUSTFLAGS` or sets `CARGO_TARGET_DIR` (§3.1). Never re-run a gate on an unchanged tree.

**`cargo xtask lint`** (fails on match in `catena/src` and `catena-ratatui/src`; `seed/` is never scanned):
- `Instant::now|SystemTime::now` (time injection, §10.4);
- `thread_rng|from_entropy` (seeded determinism, §11.5);
- `.len()` on identifiers matching `label|title|name` (the unicode-width rule, §5 — an allowlist keeps false positives manageable);
- `unwrap()` outside tests (the harvested surface has zero; `expect` with an invariant message is allowed);
- file length: warn above 500 lines, fail above 750 (the source application's 1,000-line modules are where its duplications hid);
- the scaffold rules (§3.1): any `RUSTFLAGS` in `scripts/`, `.githooks/` or `.github/`; any `rustflags` in `.cargo/config.toml`; more than one target under any crate's `tests/` or `benches/`.

Each rule has a test in `xtask` that plants a violation and watches the rule fail.

**CI (GitHub Actions; every `uses:` pinned by full commit SHA):**
- **check** (linux/macos/windows × stable): runs `scripts/regression.sh` (`shell: bash`).
- **msrv** (linux): `cargo msrv verify` against the workspace `rust-version`.
- **dco** (PRs): every commit carries `Signed-off-by`.
- **bench** (linux, main only): `scripts/check-perf.sh` against the runner's fingerprint entry, plus criterion `--save-baseline` with any regression over 15% posted as an advisory PR comment (advisory because blocking on microbenchmark noise is how benches get deleted; the 1.5× ratchet is the wall).
- **fuzz** (linux nightly cron): 3 targets × 200 s, corpus cached.
- **pty** (linux + macos): the T7 job, `cargo test -p catena-ratatui --features pty-tests -- pty:: --test-threads=1`.
- **coverage** (linux): `cargo llvm-cov`, line-coverage gate ≥ 80% on `catena`.

---

## 18. Seed manifest (file-by-file disposition)

Paths are under `seed/`. Line counts are of the seed file. "Verbatim+" = move, rename symbols, apply the listed fixes, keep the tests. "Reference" = read it to port the named pieces, then delete it; it never moves into a crate.

| Seed file | Lines | Disposition | Target | Required changes |
|---|---|---|---|---|
| `graph/braille.rs` + `graph/braille_tests.rs` | 472 + 591 | **Verbatim+** | `catena/src/raster/` | Generalize `BrailleCanvas` to `SubCellCanvas(SUB_W, SUB_H)`; unify the four Bézier loops into one sampler; make `get_cell` clip like its siblings; keep the dash-phase doc comment |
| `graph/quadtree.rs` | 385 | **Verbatim+** | `catena/src/layout/force/` | Fix the `MAX_DEPTH` self-repulsion and the jitter-quadrant bounds; keep it crate-internal; tighten the force-ratio test to (0.7, 1.4) |
| `graph/layout_fr.rs` + `graph/layout_fr_tests.rs` | 574 + 492 | **Port** | `catena/src/layout/force/` | `Uuid` → `NodeIx`; remove the `visual_label_width` up-call (metrics are injected); honor `iterations`; isotropic snap; one shared spiral fn; unicode-width |
| `graph/chord.rs` + `graph/chord_tests.rs` | 394 + 579 | **Verbatim+** | `catena/src/layout/radial/` | Rename `community_*` → `group_*` (`CommunityArc` → `GroupArc`, `place_communities` → `place_groups`); keep all constants (β = 0.85, 3° gaps, ring ratios) and the C⁰/limit tests |
| `graph/zoom.rs` | 92 | **Verbatim+** | `catena/src/geometry/` | Thresholds become a configurable table with these defaults |
| `graph/tree_layout.rs` + `graph/tree_layout_tests.rs` | 181 + 513 | **Verbatim+** | `catena/src/layout/tree/` | Fix the "Walker" module header; wire `route_connectors` output into the scene/junction machinery |
| `graph/render.rs` + `graph/render_tests.rs` | 521 + 307 | **Port, two pieces** | `catena/src/raster/`, `scene/`, `style/gradients.rs` | Hop/crossing math (`segment_intersection`, `HOP_RADIUS`, the hop pass in `render_edges`): spatial-grid candidates + hop budget, keep the endpoint-exclusion constants and the PCB convention. Gradients (`confidence_color`, `centrality_color`, `pagerank_color`, `dim_color`): pure `fn(f64) -> Rgb` with an explicit `(value, min, max)` signature. Drop the domain colorers (`type_color`, `entity_color*`, `type_symbol`) |
| `graph/render_overlay.rs` + `graph/render_overlay_tests.rs` | 506 + 415 | **Port** | `catena/src/scene/` | One label-mask builder replacing `render_canvas_to_buf` / `render_canvas_to_buf_overwrite`; glow via `Surface::patch_bg`; drop the duplicate `word_wrap` |
| `graph/mod.rs` | 497 | **Reference** | — | The orchestrator not to rebuild. Its twelve jobs are redistributed: delta classification §4.2, derive §6, anchor §6, scene assembly §7.4 |
| `ui/dag_layout/models.rs`, `builder.rs`, `ordering.rs`, `positioning.rs`, `routing.rs`, `engine.rs`, `engine_tests.rs`, `render_ascii.rs`, `mod.rs` | 186, 136, 151, 429, 480, 512, 453, 107, 32 | **Port selectively** | `catena/src/layout/layered/` | Keep: the `JunctionKind` table, flex-driven Y bands (`plan_tiers`, `assign_y`), box building (`DagBuilder`), the `render_to_string` ASCII pattern, the golden style of `engine_tests.rs`. Replace: ordering, positioning and routing algorithms per §9.1. Drop: the 3-layer `Layer` enum and the dead `build_fork_bar` family. `mod.rs` re-exports modules that are not seeded (an app bridge and two app renderers); ignore them |
| `ui/flex_layout.rs` | 418 | **Verbatim+** | `catena/src/layout/layered/flex.rs` | None — the crown jewel of the DAG side |
| `ui/box_layout.rs` + `ui/box_layout_tests.rs` | 535 + 379 | **Port one function** | `catena/src/raster/text.rs` | Take `word_wrap` and its tests, in unicode-width units; the rest of the file (row and box rendering for the app's panes) is reference for box drawing only |
| `ui/theme.rs`, `ui/themes/mod.rs`, `ui/themes/ansi16.rs` | 366, 23, 192 | **Port** | `catena/src/style/` | Drop `ThemeEntityTypes`; glyphs `&str` → `char`; keep the ansi16 theme and add the new `ascii` theme as proof of indirection. `themes/mod.rs` lists five more app themes that are not seeded and not wanted |
| `ui/vector_minimap.rs` | 127 | **Reference** | `catena-ratatui/src/minimap.rs` | Rewritten thin over core: the minimap is a second `GraphView` render at a fixed low zoom into a small Surface + a viewport-rectangle overlay |
| `app/types_excerpt.rs` | 176 | **Verbatim+ / Reference** | `catena/src/interact/` | `EdgeSpatialGrid` (+ `EdgeScreenPos`): Verbatim+, generalized to nodes too, generic over `NodeIx`/`EdgeIx` instead of `Uuid`. `PanAnimation`, `ZoomAnimation`: reference for §10.4, re-expressed over `tick(dt)` |
| `app/navigation.rs` | 736 | **Port as free functions** | `catena/src/interact/` | Zoom-anchor inversion (`adjust_pan_for_zoom`), exponential smoothing (`tick_zoom_animation`), ease-out pan (`tick_pan_animation`), occlusion-aware centering (`effective_visible_center`), the zoom steps (`zoom_in`/`zoom_out`) — extracted from `impl App` into the `Controller` |
| `app/selection_excerpt.rs` | 243 | **Reference** | `catena/src/interact/` | The four hit-test methods §10.2 replaces; the defects in §14 rows 1 and 21 live here |
| `tests/visual/svg_renderer.rs` + `tests/visual/snapshots.rs` | 454 + 140 | **Verbatim+** | `catena-testkit/src/svg.rs` | `Buffer` → `Surface`; keep the hash-line canonicalization exactly (it was designed for cross-version stability); rename the update env var to `CATENA_UPDATE_SNAPSHOTS` |
| `graph/braille_tests.rs` helper quartet | (in the 591 above) | **Verbatim** | `catena-testkit/src/braille_asserts.rs` | `lit_pixels`, `assert_8_connected`, `assert_endpoints_exact`, `assert_no_duplicates` — the single most valuable harvest per the review; the rest of the file follows `braille.rs` |
| `tests/functional/test_blink_core.rs` | 558 | **Port** | `catena-testkit/src/oscillation.rs` | Take `buffer_diff_details`, `diff_count` and the `CaptureBuf` pattern; strip the app harness; operate on `Surface` snapshots |
| `fixtures/community.json` | 184 | **Verbatim** | `catena-testkit/fixtures/community.json` | None (§20) |

Not seeded, on purpose: the host's window manager and floating windows (a host concern; the occlusion input of §6 covers the interaction), its full-screen graph view and legend pane (rewritten from §12 and §3), its DAG app bridge and grid renderer (dropped).

**Harvest procedure — two commits per seed file.** (1) `git mv seed/<path> <target>` with no content change, message `chore(harvest): move seed/<path> verbatim to <target>`; the module is not yet declared, so the tree stays green. (2) The port: declare the module, apply the required changes from the table, keep or port the tests, message `refactor(<area>): port <file> — <changes>`. The diff of commit 2 is the whole delta from the seed, reviewable forever. A Reference file is deleted in the same commit as the port it informed. `seed/` is empty, and removed, by the end of M5.

---

## 19. Milestones

Sequenced so every milestone ends green and demonstrable. Each closes on its `verify:` command, observed to exit 0.

**M0 — Bootstrap + harvest drop.**
Workspace skeleton with the §3.1 scaffold in the first crate-creating commit (root manifest, per-crate manifests with `autotests = false` and one `[[test]]`/`[[bench]]` each, profiles, workspace lints, `.cargo/config.toml` holding only the xtask alias); `xtask` with `lint` and its planted-violation tests; `scripts/smoke.sh`, `scripts/regression.sh`, `.githooks/`; CI skeleton (check via `regression.sh`, msrv, dco); `CONTRIBUTING.md` (DCO + contributor license grant per §3); `TODO.md` created from this section; the pure modules harvested by the §18 procedure (braille, quadtree, chord, zoom, flex, `word_wrap`, tree_layout) compiling in their new homes with their tests; `catena-testkit` with the braille asserts, the SVG renderer and `fixtures/community.json`.
verify: `./scripts/regression.sh` exits 0; `cargo test -p xtask` exits 0, including one planted-violation test per lint rule; `test -z "$(git ls-files seed/graph/braille.rs seed/graph/quadtree.rs seed/graph/chord.rs seed/graph/zoom.rs seed/ui/flex_layout.rs seed/graph/tree_layout.rs seed/fixtures)"`; `test -f CONTRIBUTING.md -a -f TODO.md`; `grep -q Signed-off-by .github/workflows/*.yml`.

**M1 — Raster + scene.**
`Surface`, `StringSurface`, the `SubCellCanvas` generalization, four blitters, the compositor with OR-merge, the label mask, clipping, `SceneGraph`. T2 goldens for primitive scenes; the T3 pipeline live.
verify: `cargo test -p catena raster:: scene::`; `cargo insta test` exits 0; a committed `.hash` set exists and a test that perturbs one color shows T3 failing.

**M2 — Graph model + force layout + viewport.**
`GraphView`/`Tx`/delta classification, `ResolvedMetrics`, `ForceLayout` (ported, both repulsion modes), `GridSnapper`, canonical/derived viewport, warm start + ramp-in + anchor compensation. Invariants A–G, I, M, N live; the T4 determinism tier live; the first bench and `scripts/check-perf.sh` + `bench-baseline.json` land (§15.1).
verify: `cargo test -p catena` reports ≥ 200 passing tests; the stability-contract suite and the cross-run hash test are green; `./scripts/check-perf.sh` exits 0 and has written this machine's baseline entry.

**M3 — Interaction + widget + example.**
`Controller` (hit-testing, spatial nav, node drag, animations via `tick`), `InputEvent` + the crossterm feature, the ratatui widget + adapter, `examples/interactive` (loads `fixtures/community.json` and the 120-node generated fixture of §20; all bindings live), minimap + legend. T5 live; fuzz targets 1–2 live; invariant J live.
verify: `cargo test --workspace` exits 0; `cargo build -p catena-ratatui --example interactive --features crossterm` exits 0; `cargo +nightly fuzz run fuzz_events -- -runs=10000` exits 0. Running the example in a real terminal is the owner's look, not a gate.

**M4 — Layered engine.**
The full Sugiyama pipeline of §9.1, channel routing, junction glyphs, arrowheads, `LayeredSpec` rank pinning. Invariant H live; the DAG golden set (incl. the skip-edge and channel-stacking fixtures); fuzz target 3.
verify: `cargo test -p catena layered::` exits 0; goldens exist for the 12-fixture set; the edge-conservation property passes at 512 cases.

**M5 — Polish + package.**
Sextant blitter, parallel-edge fanning + self-loop rendering, the radial/chord view wired to the widget, the T7 PTY suite, benches within the §15.1 budgets, a rustdoc pass with a doctested README, CHANGELOG, `cargo publish --dry-run` for all three crates, `seed/` gone.
verify: `cargo test --workspace --all-features` exits 0; the PTY job is green on linux and macos CI; `cargo bench` meets §15.1 on the dev machine; `cargo publish --dry-run -p catena -p catena-ratatui -p catena-testkit` exits 0; `test ! -e seed`. Tagging 0.1.0 and publishing are the owner's acts.

Post-1.0 backlog (explicitly deferred, in priority order): Brandes-Köpf + erratum; off-screen node indicators (edge-of-viewport chevrons); DOT/mermaid importer crate; `ratatui-image` high-fidelity tier; ELK backend feature; Buchheim tree layout; stress-majorization layout.

---

## 20. Fixtures

- **`fixtures/community.json`** (from `seed/fixtures/`): the most realistic graph available — 31 nodes in 3 planted groups of 10 plus one ungrouped bridger node, 146 directed edges (135 within a group), dense inside groups and sparse between them, no self-loops or parallel edges. Each node carries `key`, `label`, `group` (`null` for the bridger), and `confidence`/`pagerank` as style inputs for the gradient helpers; each edge carries `source`, `target`, `kind` and `weight`. It exercises force layout cluster separation, the radial/chord view (groups → arcs), the gradient helpers and T2/T3 goldens.
- **The 12 canonical graphs**, hand-built in `catena-testkit::fixtures`: triangle, star-8, two components + ring, self-loop, parallel ×3, deep DAG with skip edges, wide DAG forcing channel stacking, tree, chord-8-groups, CJK labels, single node, 300-node stress.
- **Generated families**, seeded and deterministic (`fixtures::generated(seed, n)`): planted-group graphs with mixed label lengths (ASCII, CJK, emoji, combining marks), occasional self-loops and parallel edges. The 120-node instance at a pinned seed is the interactive example's second dataset; the families feed invariants F and the §15.1 benches. Two runs of a generator at the same seed must hash equal.

---

## 21. Decision register (everything ruled, one place)

| Question | Ruling |
|---|---|
| Extract vs rewrite | Harvest-and-rebuild (§1.2) |
| Name | `catena`, owner-ruled (Latin "chain" → *concatenate*/*catenary*). Crates: `catena` (core), `catena-ratatui` (widget; `catena-core` is taken on crates.io), `catena-testkit` |
| Repo | `github.com/jbrjake/catena`, public; self-contained |
| License | AGPL-3.0-only + commercial exceptions, owner-ruled (exclusion play, not revenue play); DCO + contributor license grant from M0 to preserve the exception arm; the seed is the owner's own code, so no NOTICE |
| Crate split | core / widget / testkit (§3), plus an unpublished `xtask` |
| ratatui coupling | Widget crate on `ratatui-core` 0.1; core ratatui-free |
| Toolchain | Edition 2024; `rust-version` 1.88 (ratatui-core 0.1.2's), CI-verified |
| Build scaffold | §3.1 from the first crate: debug-info profile; warnings fail the gate, never `RUSTFLAGS` or config rustflags; one test and one bench target per crate; `target/` outside indexed folders on macOS |
| Worklist | `TODO.md`, every item with a `verify:` (`CLAUDE.md`, override 1) |
| Node identity | Generic `Key` (Clone + Eq + Hash + Ord), interned to dense `u32` indices |
| Graph storage | Own store; petgraph = optional interop feature, never required |
| Layouts in v1 | Force (FR + degree-scaled option), full Sugiyama, tree (naive-centered), radial + chord |
| Sugiyama x-coords | Priority method v1; Brandes-Köpf + 2020 erratum v1.1 |
| Sugiyama dependency | Own implementation; rust-sugiyama/elk-rs as test references only |
| Barnes-Hut | Harvested, gated `n > 500` |
| Snap policy | Isotropic contain + letterbox default; stretch opt-in |
| Blitters | Braille default; half-block, sextant, ascii selectable |
| Braille color limit | Exposed as a blitter trade-off; per-cell last writer within a layer, topmost layer across layers |
| Z-order | Single compositor with a fixed layer enum + OR-merged sub-cell bits |
| Scene/hit contract | Complete SceneGraph; hit-testing reads scene bounds; no row-only targets |
| Node measurement | One private fn → `ResolvedMetrics`; unicode-width only |
| Time | Injected exclusively via `tick(dt)`; wall-clock reads lint-banned |
| RNG | Seeded SplitMix64 in layout state; `thread_rng` banned |
| Async | None anywhere |
| Events | Own `InputEvent`; crossterm conversion behind a feature |
| Spatial keyboard nav | 60° cone, distance × angular penalty, half-plane fallback (§10.3) |
| Edge hit precision | Cell space, 1.5-cell threshold; sub-cell precision deliberately not used |
| Animation curves | Pan 300 ms ease-out cubic; zoom exponential τ = 40 ms |
| Windowing | Out of scope; occlusion rects are a host-supplied input |
| Testing tiers | T1–T7 per §16; no embedded-engine tier, no main-loop mirroring |
| Style/color testing | Harvested SVG + SHA-256 pipeline (gating) + PNG gallery (non-gating) |
| Coverage / file-size / property-case norms | 80% line on core; 500/750 file lines; ≥ 256 proptest cases, ≥ 7 pinned regression seeds |
| Perf ratchet | Median of ≥ 7 runs + peak RSS vs a per-machine `bench-baseline.json`; past 1.5× needs the owner |
| v1 non-goals | DOT/mermaid import, kitty/sixel, ELK, analytics, windowing, async (§2.2) |
| Publishing | `cargo publish --dry-run` only; the real publish and the 0.1.0 tag are the owner's acts |
