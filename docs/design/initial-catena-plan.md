# catena — implementation plan

**Status:** proposal of record, ready to execute. All questions are answered and the implementation details are decided (§21). A departure from a decided item is a question for the owner, not a call for the implementing session.
**Name:** `catena`, owner-ruled. Latin *catena*, "chain": the root of *concatenate*, of the medieval *catena* commentaries that link sources passage to passage, and of the *catenary*, the curve a hanging chain makes and the natural line of a slack edge between two nodes. The crates are `catena` (core), `catena-ratatui` (widget) and `catena-testkit`. `catena-core` on crates.io belongs to an unrelated project (hellas-ai's "catena: the deterministic array language"), which is why the core crate takes the flagship name. Checked 2026-10-06: `catena`, `catena-ratatui` and `catena-testkit` are all unregistered on crates.io. Reserving them means a real publish (a `0.0.0` placeholder), so it is the owner's act; the plan recommends doing it before M1 lands, since a squat would force a rename across every crate, path and doc below.

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
6. The decision register (§21) is the index of everything ruled. When a section and §21 disagree, §21 wins and the section is the bug; fix the section in the same commit as the code.

**What "the seed" means in this document.** A "the seed does X" below is a statement about a file under `seed/`, cited by path and symbol. A few statements describe parts of the source application that were not seeded (its end-to-end test harness, its windowing, its DAG app renderers); those name "the source application" or say "not seeded", and nothing in them needs to be found. The seed's code refers to types that are not in this repository; they are the host application's and none of them are wanted:

| Seed type | What it was | catena's replacement |
|---|---|---|
| `App` | the host application's state struct (≈190 fields) | `GraphView` + `Controller` (§4, §10) |
| `Uuid` node and edge ids | hard-coded key type | generic `K: Key`, interned to `NodeIx` (§4.1) |
| `ColorPalette`, entity types, `TuiRelation`, communities | the host's domain model | `StyleResolver` callback, plain `u32` groups (§12) |
| `window_manager`, `WindowId`, floating windows | the host's windowing | host-supplied occlusion rectangles (§6) |
| `TuiTestRunner`, `dump_screen` (test harness) | the host's end-to-end harness | `catena-testkit::scenario` (§3) |

The originating application's name appears in a few seed comments and in one environment variable in `seed/tests/visual/snapshots.rs`. It is not a catena concept: drop or rename it when porting (the snapshot variable becomes `CATENA_UPDATE_SNAPSHOTS`).

**Vocabulary.** These terms are used in one fixed sense throughout; the section in brackets is where each is defined.

| Term | Meaning |
|---|---|
| **key** / `K` | the host's node identity (`String`, `u64`, `Uuid`, …); touched only at the API boundary [§4.1] |
| `NodeIx`, `EdgeIx` | dense internal indices every hot path runs on; `EdgeId` is the host-facing stable edge handle [§4.1] |
| **delta** | what one `update()` transaction changed, classified `Topology` / `Property` / `Geometry` [§4.2] |
| **world space** | the layout's isotropic `f64` coordinates, independent of viewport, pan and zoom [§6] |
| **canonical** / **derived** cells | integer cell positions snapped once from world space at `ref_zoom` / the per-frame pan-and-zoom transform of them [§6] |
| **snap** | mapping world positions to non-overlapping canonical cells; cheap, not a relayout [§6] |
| **relayout** | re-running a layout engine (the force simulation, the Sugiyama pipeline); warm or cold [§8.3] |
| **semantic zoom** | the six discrete label-detail levels a continuous zoom selects between [§5] |
| `ResolvedMetrics` | the one per-level measurement of every node box; the single source of geometric truth [§5] |
| **sub-cell** | the blitter's pixel grid inside a terminal cell (2×4 for braille) [§7.2] |
| **blitter** | the encoder from a sub-cell mask to one glyph per cell [§7.2] |
| `Surface` | the only sink catena writes to; `CellGrid` is the in-memory implementation tests and exports read back [§7.1] |
| `SceneGraph` | the complete display list layout emits and both the renderer and hit-testing consume [§7.4] |
| **compositor** | the one owner of z-order and cross-layer sub-cell merging [§7.4] |
| `tick(dt)` | the only entry point for time; every animation and timed behavior advances here [§10.4] |
| **gate** | a script whose exit 0 is the evidence something is done: `smoke.sh` at commit, `regression.sh` at push [§17] |
| **harvest** | moving a `seed/` file into a crate by the two-commit procedure [§18] |

---

## 0. Executive summary

**The verdict: harvest-and-rebuild.** Do **not** wrap the seed's graph views as a library, and do **not** rewrite from a blank page. Build a new workspace with a new architecture, seeded with ~4,300 lines of the seed's clean, tested modules imported nearly verbatim (braille rasterizer, Barnes-Hut quadtree, chord/Holten math, flex allocator, tuning constants and, most valuable of all, its property-test harnesses), while designing the orchestration, data model, rendering pipeline and interaction layers fresh to eliminate the five structural defects that make the seed's version un-extractable as-is.

**Why this is worth building at all:** the ecosystem research found a genuine four-way gap. No existing crate is simultaneously (1) general-topology (not just DAGs), (2) interactive (pan/zoom/select/drag/keyboard-nav), (3) actively maintained and (4) ratatui-native. The closest contenders each satisfy at most two: `tui-nodes`/`ratatui-flow` are DAG-only and non-interactive; `ascii-petgraph` is a 5-commit force-directed proof of concept; `egui_graphs` has the right API shape but targets GUI. The seed is, by a wide margin, the most sophisticated TUI graph renderer the review found anywhere, and it is trapped inside an application.

**Scale of the opportunity in the seed:** the coupling audit found the candidate surface (~7,300 production lines) contains **zero** async, **zero** I/O, **zero** logging, **zero** bare `unwrap()`. That is unusually extraction-friendly. What blocks verbatim extraction is architecture, not hygiene: a 497-line orchestrator (`seed/graph/mod.rs`) welded to 25 fields and 2 methods of the 190-field `App` (61 accesses), a hard-coded `Uuid` node key, a domain-specific `ColorPalette`, a DAG engine hard-coded to exactly three layers, and an interaction layer implemented entirely as `impl App` methods (`seed/app/`).

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
3. **Top stratum — orchestration, state, interaction glue: discard and redesign.** `seed/graph/mod.rs` (25 App fields and 2 App methods, 61 accesses, eighteen sequential jobs in one function), the full-screen view, all `impl App` interaction methods, the DAG renderer's card/bus geometry that the layout doesn't model.

The bug review makes the strongest case: the majority of the seed's real defects (§14 ledger) are *architectural leaks between strata* — hit-testing using full label width while rendering uses visual width; routing discarding edge color and forcing a downstream raster-rescan hack; three ad-hoc z-order conventions; a renderer drawing geometry the layout never modeled. Extraction would ship those defect generators as the library's public architecture. A rebuild designed around **single-source-of-truth geometry** and a **complete scene graph** (§5, §7.4) eliminates the entire defect class, not just the instances.

### 1.3 What the seed's DAG engine is *not* (and the successor must be)

`seed/ui/dag_layout/` is not Sugiyama: layer assignment is caller-supplied (hard-coded `Entity/Snippet/Document`), there is no cycle detection, no dummy nodes, layer-skipping edges are **silently dropped** (`route_between_layers` in `engine.rs` keeps only edges whose source is in the adjacent parent layer; nothing else is ever routed), crossing minimization (`order_entities` in `ordering.rs`) is a fixed two-pass barycenter on one layer only (the other two layers get a single centroid sort in `engine.rs`), all merge bars in a layer pair share a **single row** (`bar_row` is computed once per layer pair in `engine.rs`, making overlapping bars indistinguishable), and edge color is discarded by the router (`engine.rs` hard-codes `EdgeColor::Neutral` and never reads `color_hint`) and reconstructed by scanning the rendered raster in the un-seeded app renderer. Its *shape* — pure `Dag → layout_dag → Layout → renderers`, golden-ASCII tested (`engine_tests.rs`) — is exactly right; its algorithms must be replaced with the real pipeline (§9).

### 1.4 Ecosystem facts that shaped the design

- **petgraph has no layout module**; every layout crate bolts on. We interop with petgraph behind a feature but do not require it.
- **ForceAtlas2-style degree-scaled repulsion** produces more legible clustering than uniform FR, worth more, not less, at terminal resolution. Offered as a mode.
- **Barnes-Hut loses to brute force below ~1,000 points** (quadtree build overhead). We keep the harvested quadtree but gate it on `n > 500`.
- **Brandes-Köpf has two known flaws**; a 2020 erratum (arXiv:2008.01252) is required reading. v1 uses the simpler priority method; BK + erratum is a v1.1 upgrade.
- **ratatui is immediate-mode with diffed output**: only changed cells hit the wire. Layout *stability* is therefore a rendering-performance feature, not just UX — jittery layout defeats the diff.
- **Braille cells carry one fg color per 2×4 block** (ratatui#693). The color/resolution trade must be a user-visible blitter choice (braille vs half-block vs sextant), not a hidden constant.
- **ratatui 0.30.2 is current (re-verified on crates.io 2026-10-06; both it and `ratatui-core` 0.1.2 declare `rust-version = "1.88"`), and widget authors are advised to depend on `ratatui-core`** so the crate works across app-side ratatui versions. The seed was written against ratatui 0.29; targeting `ratatui-core` decouples catena from app-side versions entirely.
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
├── catena/                          # ALL logic. Deps: unicode-width, libm, thiserror. No ratatui. No I/O.
│   ├── src/
│   │   ├── graph/                   # store, keys, deltas, errors               (§4)
│   │   ├── geometry/                # metrics, snap, viewport                   (§5, §6)
│   │   ├── layout/                  # force/, layered/, tree/, radial/          (§8, §9)
│   │   ├── raster/                  # subcell canvas, blitters, primitives      (§7.2–7.3)
│   │   ├── scene/                   # SceneGraph, compositor, layers            (§7.4)
│   │   ├── interact/                # controller, hit-testing, spatial nav      (§10)
│   │   ├── style/                   # Theme, GlyphSet, StyleResolver, gradients (§12)
│   │   └── fmath.rs                 # the ONLY caller of transcendental fns (§11)
│   ├── tests/catena/main.rs         # the crate's ONE integration-test target; topics are `mod`s
│   └── benches/catena/main.rs       # the crate's ONE bench target (criterion, harness = false)
├── catena-ratatui/                  # ratatui widgets. Deps: catena, ratatui-core.
│   ├── src/
│   │   ├── widget.rs                # Graph StatefulWidget + the Buffer-backed Surface adapter
│   │   ├── minimap.rs               # Minimap widget
│   │   ├── legend.rs                # Legend widget (host-supplied (label, color) rows)
│   │   ├── input.rs                 # crossterm Event → Option<InputEvent> (feature `crossterm`)
│   │   └── bin/catena-demo.rs       # the demo binary (feature `demo`); T7's subject (§16.8)
│   └── tests/catena_ratatui/main.rs # T5 and (feature `pty-tests`) T7, as `mod`s
├── catena-testkit/                  # test support, published. Deps: catena, sha2, serde, serde_json.
│   ├── src/
│   │   ├── svg.rs                   # CellGrid → SVG + SHA-256 hash (harvested)
│   │   ├── braille_asserts.rs       # lit_pixels, assert_8_connected, endpoints, no-dups (harvested)
│   │   ├── oscillation.rs           # A→B→A detector, steady-state zero-diff, frame hashing
│   │   ├── invariants.rs            # no-overlap, edge-endpoint, determinism helpers
│   │   ├── fixtures.rs              # fixture loading + seeded generators (§20)
│   │   ├── scenario.rs              # scripted event-stream runner with injected time
│   │   └── bin/catena-render-hash.rs# fixture → frame hash on stdout; the cross-run test's subject (§16.5)
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
| catena | `unicode-width`, `libm` (§11), `thiserror` (§4.3) | `serde` (Theme/Positions ser/de), `petgraph` (interop constructors) |
| catena-ratatui | `catena`, `ratatui-core 0.1` | `crossterm` (event conversion), `widget-extras` (minimap/legend), `demo` (= `crossterm` + optional full `ratatui`; builds the `catena-demo` bin), `pty-tests` (T7; implies `demo`) |
| catena-testkit | `catena`, `sha2`, `serde`, `serde_json` (fixture files) | `png` (resvg rasterization for review galleries — heavy, off by default) |

Dev-deps (workspace): `insta` (with `filters`), `proptest`, `criterion`, `portable-pty`, `vt100`, `ratatui` (full, for `TestBackend` in widget tests). No `uuid`, no `tokio`, no `tracing`, no `anyhow` anywhere in published code. Published crates declare semver ranges, not `=x.y.z` pins (`CLAUDE.md`, override 3); `Cargo.lock` is committed.

**The dev-dependency cycle (decided).** `catena`'s own tests use `catena-testkit` (the braille asserts, fixtures, oscillation helpers), and `catena-testkit` depends on `catena`. Cargo allows exactly this cycle through `[dev-dependencies]`: `catena` dev-depends on `catena-testkit` by `path` only, with **no `version`**, which cargo strips at publish time, so `catena` publishes first and `catena-testkit` after it. The cost is that `cargo test -p catena` compiles `catena` twice (once as the lib under test, once as testkit's dependency); accepted, because the alternative (a `testing` feature exposing helpers from the core crate) leaks test code into the published API.

**Binaries.** Two tiny `[[bin]]` targets exist because tests need to spawn a real process and cargo only exposes a built path (`CARGO_BIN_EXE_<name>`) for a package's *binaries*, never for its examples: `catena-render-hash` in the testkit (the cross-run determinism subject, §16.5) and `catena-demo` in the widget crate (the PTY tier's subject, §16.8, and the owner's demo). Both carry `required-features` so a plain `cargo build` never pulls crossterm or full ratatui into a library build; neither is an example. The one-target rule of §3.1 is about `tests/` and `benches/`; these two bins are the only `src/bin/` entries in the workspace.

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

// Dense indices — all hot paths run on these, never on K. Public but opaque
// (private field, no arithmetic): the SceneGraph and Outcomes carry them so no
// frame ever clones a key; GraphView::key(ix) -> Option<&K> maps back.
pub struct NodeIx(u32);
pub struct EdgeIx(u32);
/// Host-facing edge identity: a monotone per-GraphView counter, never reused,
/// so a stale id after removal fails as UnknownEdge instead of aliasing a new edge.
pub struct EdgeId(u32);
```

**Decisions:**
- `Clone`, not `Copy`, so `String` keys work. Keys are touched only at the API boundary; the first thing `GraphStore` does is intern them to `NodeIx`. Hot loops (layout iterations, rasterization, hit-testing) are index-only, so key choice has zero per-frame cost.
- **`Ord` is required and is the determinism backbone.** The seed's hardest-won lesson (documented twice in `seed/graph/layout_fr.rs`): ordering by freshly generated UUIDs breaks run-to-run reproducibility. Every internal iteration that affects output order sorts by `(sort_key, key)` where `sort_key: Option<String>` is user-suppliable (e.g., a display name). Test §16.2-D pins this. The intern table is a `HashMap<K, NodeIx>` used for **lookup only**; nothing ever iterates a hash map on a path that reaches the output.
- **Edges need their own identity** because parallel edges are first-class: `(from, to)` does not name an edge. `Tx::add_edge` returns an `EdgeId`; `set_edge`/`remove_edge` take one. Among the parallel edges of one `(from, to)` pair the deterministic order is insertion order (their **rank**, 0-based, used for fan offsets in §7.3); across pairs it is `(from sort tuple, to sort tuple)`. Invariant D's shuffle test therefore shuffles nodes and distinct edges, and parallel edges of one pair are a fixed sequence by contract, not an unordered set.
- Parallel edges and self-loops are **first-class in the model from day 1** (the seed silently collapsed both). Rendering fans parallel edges and draws self-loops from v1 (§7.3); the model never dedups.
- **Capacities are explicit.** Indices are `u32`, so a `GraphView` holds at most 2³²−1 nodes and the same of live edges, and an `EdgeId` counter that would wrap fails the transaction with `GraphError::Capacity` rather than aliasing. Labels are truncated to `max_label_cols` display columns (default 256) at `set`/`add` time, so one hostile label cannot make a frame O(label).

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
    /// All-or-nothing: on Err the transaction is discarded and nothing changed.
    pub fn update<T>(&mut self, f: impl FnOnce(&mut Tx<'_, K>) -> Result<T, GraphError<K>>)
        -> Result<T, GraphError<K>>;
    pub fn key(&self, ix: NodeIx) -> Option<&K>;
    pub fn edge(&self, id: EdgeId) -> Option<EdgeRef<'_, K>>;   // from, to, rank, spec
}
impl<K: Key> Tx<'_, K> {
    pub fn add_node(&mut self, key: K, spec: NodeSpec) -> Result<NodeIx, GraphError<K>>;  // Err: DuplicateNode
    pub fn remove_node(&mut self, key: &K) -> Result<(), GraphError<K>>;                  // removes incident edges
    pub fn add_edge(&mut self, from: &K, to: &K, spec: EdgeSpec) -> Result<EdgeId, GraphError<K>>; // Err: UnknownNode
    pub fn remove_edge(&mut self, id: EdgeId) -> Result<(), GraphError<K>>;               // Err: UnknownEdge
    pub fn set_node(&mut self, key: &K, f: impl FnOnce(&mut NodeSpec)) -> Result<(), GraphError<K>>;
    pub fn set_edge(&mut self, id: EdgeId, f: impl FnOnce(&mut EdgeSpec)) -> Result<(), GraphError<K>>;
}
```

The transaction buffers its operations, validates each against the store plus the buffer, and applies them on `Ok`. On commit it computes a **delta classification** — the mechanism behind the seed's gem "enrichment updates never move the graph":

- `Topology` (nodes/edges added/removed, `layout_participating` flipped) → schedules incremental relayout (warm start, §8.3).
- `Property` (label text within the same measured width, weight, class) → re-render only, positions untouched.
- `Geometry` (label measured width changed, shape changed) → local re-snap of affected nodes only; full relayout only if collision resolution cascades past a threshold (8 displaced nodes).

One transaction has one class: the strongest of its operations (`Topology > Geometry > Property`). An empty transaction is a no-op and marks nothing dirty.

**When layout runs (decided):** a `Topology` commit marks the layout dirty and the relayout runs **synchronously at the end of `update()`**, so the next `render` already shows the settled graph and the §15.1 budgets apply to `update()`. `LayoutPacing::Animated { iterations_per_tick }` is the opt-in alternative: the simulation then advances only inside `tick(dt)` (§10.4), which gives a visible settle at no cost to determinism because tick is already injected time. Rendering a dirty layout before it has settled shows the current positions; nothing blocks.

### 4.3 Errors

```rust
#[derive(Debug, thiserror::Error)]
pub enum GraphError<K: Key> {
    #[error("node {0:?} already exists")]        DuplicateNode(K),
    #[error("no node {0:?}")]                    UnknownNode(K),
    #[error("no edge {0:?}")]                    UnknownEdge(EdgeId),
    #[error("capacity exceeded: {0}")]           Capacity(&'static str),
}
```

That is the whole error surface of the core crate: construction and mutation can fail; rendering, layout, hit-testing and `tick` are total functions that never return `Result` and never panic on any input (fuzz targets 1–3, §16.7, are the proof). The widget crate adds nothing. `thiserror` is the one place a derive macro enters the published dependency tree; `anyhow` appears only in `xtask`. The two bins (§3) are small enough to return `Box<dyn std::error::Error>` from `main`, so no published crate carries `anyhow` even behind a feature.

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

Two harvest facts about this table. First, in the seed only levels 0–2 ever take effect: `seed/graph/layout_fr.rs` clamps every label to `name.len().min(20) + 2 = 22` columns before the zoom code sees it, so the 34-column and full-label levels are dead code there (ledger row 27). In `catena` all six levels are live, with level 5 bounded only by `max_label_cols` (§4.1). Second, the level boundaries are **hysteresis-free** by decision: a continuous zoom crossing 1.5 flips between levels 2 and 3 exactly at 1.5 in both directions. Hysteresis would make the level a function of history, which breaks §11.5 (same input, same frame); the zoom glide's smoothing (§6) already keeps a level flip from oscillating.

---

## 6. Viewport and coordinate pipeline

Harvested architecture, with the fixes listed below.

```
world space (f64, layout output, y-units = x-units)
  → GridSnapper: ISOTROPIC scale × cell-aspect correction → canonical cells (i32)
  → per-frame derive: (c − margin) × (zoom / ref_zoom) + margin + pan  → view cells
  → blitter transform: ×SUB_W horizontal, ×SUB_H vertical (+SUB_H/2)  → sub-cell pixels
```

- **World space is isotropic; the cell aspect lives in exactly one place.** Layout engines run in `f64` world units where one unit is one cell *column* in both axes; they know nothing about cell shape. `GridSnapper` applies `cell_aspect` (default 0.5: a cell is about twice as tall as it is wide) to y once, when it maps world to cells, and `ResolvedMetrics` converts box heights into world units with the inverse. The seed applies the factor at placement time instead (`y … * 0.5 * angle.sin()` in `layout` and `radius * 0.5` in `radial_layout`, `seed/graph/layout_fr.rs`) and then stretches both axes independently at snap; `catena` does neither in world space, so §8.1's "kept mechanics" list omits the placement-time squash on purpose.
- **Canonical/derived split (harvested):** layout runs once into pan-independent canonical positions at `ref_zoom`; pan and zoom are O(n) derivations per frame and **never re-run a layout engine**. A relayout runs only on: a `Topology` delta (warm), a `Geometry` delta whose re-snap cascades past 8 displaced nodes (warm), a viewport resize (warm, since `k` depends on the usable area), and a semantic-zoom level transition (warm, since box widths changed). `ref_zoom` is the zoom at which the last snap ran.
- **Zoom-out re-snap (the one exception, stated).** Within a semantic level, zooming in only adds space between fixed-width boxes, so derived positions stay overlap-free. Zooming *out* below `ref_zoom` shrinks the spacing while the boxes keep their width, so boxes that the last snap had to displace can overlap again. On a zoom-out frame where the last snap displaced anything, `GridSnapper` re-runs collision resolution on the derived cells (no simulation; cheap; moves only colliding nodes) and the result becomes the new canonical set at the new `ref_zoom`. This is ledger row 10, and §11.2 is worded to match it.
- **Fix 1 — isotropic snap.** The seed normalizes x and y independently (`snap_to_grid` in `seed/graph/layout_fr.rs`), stretching the layout to fill the viewport and destroying the force-directed metric. `GridSnapper` defaults to `Fit::Contain`: uniform scale `s = min(usable_w / world_w, usable_h / (world_h × cell_aspect))`, centered with letterboxing. `Fit::Stretch` is offered as an explicit opt-in.
- **Fix 2 — clip, don't drop.** Positions are signed; off-viewport nodes keep coordinates so edges aim at them (harvested intent). The seed's renderer then returns early for any node with a negative coordinate (`render_node` in `seed/graph/render_overlay.rs`) — a node one cell off-screen pops out of existence. `catena` clips node boxes and labels to the viewport rectangle (per cell) and clips edge segments with **Cohen–Sutherland before rasterization** (the seed's `draw_line` steps every pixel of a ±30k-cell segment to use ~200 of them).
- **Grid snapping** (harvested): degree-descending placement (hubs claim cells first), 4-direction expanding spiral collision resolution. Three fixes: off-screen nodes participate in collision resolution (the seed's `on_screen` test skips them, so it reserves their cells but never resolves them); the spiral is one shared function with one bound (the seed has two copies, in `snap_to_grid` and `radial_layout`, bounded by 50 *attempts*, which is 25 rings; `catena` bounds it at 50 rings, after which the node keeps its last candidate cell, overlaps, and counts as displaced); and the **2-row vertical gap is enforced**, not just documented — the seed's comment promises one but `rect_overlaps` tests label cells only, so stacked nodes touch. `catena` inflates every box by one row above and below for the collision test, so an edge always has a row to pass between stacked nodes (ledger row 28).
- **Zoom mechanics** (harvested from `zoom_in`, `zoom_out`, `adjust_pan_for_zoom` and `tick_zoom_animation` in `seed/app/navigation.rs`): continuous zoom clamped [0.1, 4.0]; keyboard step 0.25, scroll step 0.08; **exponential smoothing** toward the target (retargeting mid-flight mutates the target — right for touchpad event storms); zoom-toward-cursor keeps the anchor cell fixed via `pan′ = pan − (anchor − margin − pan)(ratio − 1)`. Smoothing is time-injected: `α = 1 − exp(−dt/τ)`, τ = 40 ms. For calibration: the seed moves 35% of the remaining distance per tick, which is τ ≈ 39 ms at 60 fps and τ ≈ 116 ms at its own ~50 ms tick cadence; τ is a feel judgment (§16.9), and the doc comment on the seed's `ZoomAnimation` (`seed/app/types_excerpt.rs`) overstates the per-frame progress (it describes 65% per frame; the code in `tick_zoom_animation` moves 35%). One fix: the seed rounds the pan to `i32` on every animation tick, so a long glide accumulates rounding drift; `catena` keeps pan and zoom as `f64` state and rounds once, in the derive step.
- **Anchor compensation on relayout** (harvested): capture the focused node's view position before relayout, shift pan by the delta after, so relayout never teleports the graph. Contract test: ≤ 3 cells drift (§16.2-G).
- **Occlusion-aware centering** (harvested from `effective_visible_center` in `seed/app/navigation.rs`): `visible_center(viewport, occlusions: &[Rect])` — the area-weighted centroid of the unoccluded region, so "center on node" centers in what the user can actually see. In `catena` the occlusion list is host-supplied (the host knows its floating windows), keeping the library windowing-agnostic. One fix: the seed subtracts each occluding rectangle's area and moment separately, so two overlapping windows are subtracted twice and the centroid lands outside the visible region. `catena` computes the unoccluded region exactly by marking occluded cells in a viewport-sized bitmap (≤ 65k cells, one pass), so overlapping rectangles count once.

---

## 7. Rendering stack

### 7.1 Surface abstraction

```rust
/// The only thing catena writes to. The widget crate adapts this to ratatui Buffer.
pub trait Surface {
    fn size(&self) -> (u16, u16);
    /// `symbol` is one terminal cell's content: a single glyph plus any zero-width
    /// followers (combining marks), display width 1 or 2. A width-2 symbol owns
    /// (x, y) and (x + 1, y); the implementation marks the second cell as a
    /// continuation. Out-of-range writes are ignored, never an error.
    fn put(&mut self, x: u16, y: u16, symbol: &str, style: CellStyle);
    fn patch_bg(&mut self, x: u16, y: u16, bg: PaletteColor);   // glow halos preserve fg
    fn put_char(&mut self, x: u16, y: u16, glyph: char, style: CellStyle) { /* provided: encode + put */ }
}
pub struct CellStyle { pub fg: PaletteColor, pub bg: Option<PaletteColor>, pub attrs: Attrs } // Bold, Dim, Reversed, Italic
```

`PaletteColor` is `Rgb(u8,u8,u8) | Ansi(u8) | Indexed(u8)` — resolved to ratatui `Color` only in the adapter. `CellGrid` is the core's own readable implementation (`cell(x, y)`, `to_string()`, `to_ansi_string()`): it gives free static export — the seed's `render_to_string` pattern (`seed/ui/dag_layout/render_ascii.rs`) generalized — and it is the substrate of every golden, visual and oscillation test, since the trait itself is write-only. The surface's size at render time is authoritative: if it differs from the last `Resize` event the controller saw, the render treats it as one.

**Text cells (decided).** Labels are split into cells with `unicode-width` alone, no grapheme-segmentation dependency: each char of width 1 or 2 starts a cell, each width-0 char (combining marks, variation selectors, ZWJ) is appended to the cell before it, and a leading width-0 char is dropped. That is exactly the measurement `ResolvedMetrics` already makes, so a label's reserved width and its rendered width cannot disagree (invariant N). Sequences whose rendered width terminals disagree on (ZWJ emoji families, flags) are measured as the sum of their parts; a terminal that renders them narrower shows a gap inside the box, never an overrun. Theme glyphs (junctions, arrows, dashes) are `char`, width 1, by construction (§12).

### 7.2 Blitters

```rust
pub enum Blitter { Braille, HalfBlock, Sextant, Ascii }
```

| | sub-res/cell | colors/cell | notes |
|---|---|---|---|
| `Braille` | 2×4 | 1 fg | default; harvested canvas incl. the correct bit-6/7 bottom-row table (`BRAILLE_MAP`) |
| `HalfBlock` | 1×2 | 2 (fg+bg) | when per-node/edge color fidelity beats resolution |
| `Sextant` | 2×3 | 1 fg | Unicode 13; better font fidelity than braille in several terminals |
| `Ascii` | 1×1 | 1 | direction-quantized `─│╱╲` + junction glyphs; universal fallback, and what a host selects for `NO_COLOR`/dumb terminals (the library reads no environment; `theme::ascii()` + `Blitter::Ascii` is the host's choice) |

One `SubCellCanvas` (flat `Vec<u8>` mask + parallel `Vec<ColorSlot>`) parameterized by `(SUB_W, SUB_H)`; the blitter only encodes the final mask → char. All harvested primitives on `BrailleCanvas` (`draw_line`, `draw_line_with_hops`, `draw_dashed_line` with its zero-period guard, `draw_bezier`/`draw_bezier_ctrl`, `draw_dashed_bezier`/`draw_dashed_bezier_ctrl`, `draw_circle`) operate on the canvas generically. The four near-identical Bézier sampling loops in the seed collapse to one sampler with a plot callback. **Sampling density kept:** one sample per 2 sub-pixels, `steps = (chord/2).clamp(10, 200)`; the seed's comment about dash phase by sample index coarsening on short curves moves into the doc comment verbatim.

**Per-cell color policy for 1-color blitters (decided):** within one canvas, last writer wins per cell (cheap, deterministic given the sorted draw order); across canvases the compositor decides (§7.4). The chord overlay's sector-paint trick (colorize lit cells by angular sector after drawing) is kept for the radial view, where per-edge canvases would be wasteful.

### 7.3 Edge rendering (decided details)

- **Straight edges** braille-rasterized; `EdgeClass::Curved` gets the 20%-perpendicular quadratic bow.
- **Crossing hops** (harvested from `segment_intersection`, `HOP_RADIUS` and `render_edges` in `seed/graph/render.rs`): segment-segment intersection with near-endpoint exclusion `t,u ∈ (0.02, 0.98)`; the more-horizontal edge takes the gap (`HOP_RADIUS = 3` sub-pixels ≈ 1.5 cells, the PCB convention), endpoints never suppressed. **Fixed scaling:** candidate pairs come from the spatial grid (§10.2) instead of the all-pairs O(E²) scan — bucket-local pairs only, with a per-frame hop budget (`max_hops = 256`, after which remaining crossings render plain) so worst-case frames degrade gracefully instead of stalling.
- **Parallel edges** fan with perpendicular offsets (±2 sub-pixels per rank, capped at 3 ranks, then a `×n` count badge at the midpoint). The rank is the edge's insertion rank within its `(from, to)` pair (§4.1); a reciprocal pair (A→B and B→A) counts as two parallel edges of opposite direction and fans the same way (the seed's `render_edges` merges reciprocal pairs through `seen_pairs` and draws one undirected line).
- **Self-loops** render as a 3-cell-radius circle arc anchored at the node box's top-right corner, arrowhead per direction.
- **Arrowheads** for directed edges: final-2-sub-pixel chevron in braille; `▸▾◂▴` quantized in the Ascii blitter. (The seed has none.)
- **Label mask** (harvested): a flat `Vec<bool>` built from `ResolvedMetrics.boxes` — one builder, used by every canvas blit; edges never overprint text. One mask per frame, replacing the seed's two divergent blit paths (`render_canvas_to_buf` and `render_canvas_to_buf_overwrite` in `seed/graph/render_overlay.rs`, both reading the `label_grid` built in `render_edges`).

### 7.4 Scene graph and compositor — the completeness rule

The DAG review's sharpest architectural finding: the seed's renderer draws geometry its layout never modeled (entity card, bus), and its pipeline drops information (edge color, click columns) that downstream hacks then reconstruct. The fix is structural:

```rust
/// COMPLETE display list. If it's on screen, it's in here. If it's in here, it has bounds + payload.
/// Not generic over K: items carry indices, so building a scene never clones a key.
pub struct SceneGraph { items: Vec<SceneItem> }
pub struct SceneItem {
    pub z: Layer,                  // Background | EdgesUnder | EdgesOver | Glow | Nodes | Labels | Annotations
    pub bounds: CellBox,           // derived-cell space, viewport-relative, may extend past the viewport (clipped at render)
    pub payload: Payload,          // NodeBox{ix} | EdgePath{ix, route} | Junction{..} | Label{..} | Decoration{..}
    pub style: StyleId,
}
pub enum Route {
    Polyline(Vec<SubPt>),          // sub-cell points; any blitter rasterizes it (force, radial, tree-as-curves)
    Orthogonal(Vec<CellPt>),       // axis-aligned cell segments; drawn with the GlyphSet's box-drawing + junction
}                                  //   table regardless of blitter (layered, tree connectors)
```

- **Layout emits SceneGraph. The renderer interprets it and adds nothing.** Every renderable concept — including layered-layout connector bars and any future "card" — must be a scene item, or it cannot be drawn.
- **Two route geometries, one renderer.** A `Polyline` is in sub-cell units and goes through the sub-cell canvas of its layer, so it looks like whatever the blitter makes of it. An `Orthogonal` route is in whole cells and is drawn as box-drawing glyphs with junctions from the theme's `GlyphSet`, which is what a flowchart reader expects and what no blitter can improve on; it bypasses the canvas and writes cells directly, with the label mask still honored. The layered and tree engines emit `Orthogonal`; force and radial emit `Polyline`.
- **Hit-testing consumes the same SceneGraph** (§10.2). Click targets carry full bounds — the row-only click-target defect class (three sites in the seed) cannot exist.
- **The compositor owns z-order.** Fixed layer enum; within a layer, insertion order (which is deterministic, §4.1). Sub-cell canvases exist per layer; **braille bits OR-merge across layers** before glyph encoding, with color from the topmost layer contributing a bit in that cell — this fixes the seed's "overlay edge visually broken at crossings" (its paint-behind blit drops lower-layer dots entirely). The seed's three ad-hoc conventions (a write-only-if-blank grid helper in its DAG grid renderer, which is not seeded; a `symbol == " "` guard; the overwrite blit `render_canvas_to_buf_overwrite`) are replaced by this one policy. Glow halos use `patch_bg` so the halo background composes under edge foregrounds (the seed's `dim_color` trick in `seed/graph/render.rs`, now a Surface primitive).

---

## 8. Force-directed layout

The seed's Fruchterman-Reingold core (`layout` in `seed/graph/layout_fr.rs`) with its tuning intact, plus four upgrades.

### 8.1 Parameters (defaults = the seed's proven values)

```rust
pub struct ForceParams {
    pub iterations: u32,          // 100 — RESPECTED as given (the seed silently floors cold runs to 100 and warm
                                  //   runs to 30; presets carry the wisdom instead: Preset::Quality=100, Preset::Fast=30)
    pub warm_iterations: u32,     // 50 — a warm relayout's budget (the seed's orchestrator passes 50)
    pub cooling: f64,             // 0.95 multiplicative
    pub gravity: f64,             // 0.10 toward running centroid
    pub theta: f64,               // 0.80 Barnes-Hut opening criterion
    pub bh_threshold: usize,      // 500 — brute-force below (research: quadtree overhead loses < ~1k)
    pub repulsion: Repulsion,     // Uniform (FR classic) | DegreeScaled (FA2-flavored; better cluster legibility)
    pub converge_eps: f64,        // 0.5 cells max displacement → early exit
    pub k_label_scale: bool,      // true: k = sqrt(area/n) × max(avg_label_w/4, 1) — settles at label-surviving distances
    pub ramp_in_iterations: u32,  // 10 — new-node force ramp (§8.3)
}
```

Kept mechanics: position-based Euler with temperature clamp (no velocity — stability comes from warm-starting); cold `T₀ = max(usable w, h)/2`, warm `T₀ = max(usable w, h)/8` (the seed's values); displacement `min(‖F‖, T)` with `‖F‖` floored at 0.01; initial placement on a circle of radius `sqrt(n)·avg_label_w` (the seed's comment says average but its code uses each node's own width, so its "circle" is not one; `catena` uses the average and gets a circle) with no aspect factor (§6); quadtree coincident-point jitter + `MAX_DEPTH` valve.

**No randomness anywhere in core (decided, a correction to earlier drafts).** The audit found that nothing in the harvested layout code draws a random number: the seed's initial placement is a circle, its quadtree "jitter" is a fixed `1e-4` offset, and `rand` is not a dependency. `catena` keeps it that way: there is no seed parameter, no RNG in any engine, and §11.5 reads "identical input ⇒ identical frames" with nothing to hold constant but the input. The seeded `SplitMix64` lives in `catena-testkit::fixtures` for generated graph families (§20) and nowhere else; the `thread_rng` lint (§17) stays as a tripwire.

**Quadtree fix (harvested bug, `seed/graph/quadtree.rs`):** at `MAX_DEPTH` (64) a node accumulates mass with no body, so the self-skip in `compute_force` (which requires `body.is_some()`) never fires and a point repels its own cell's mass; the coincident-point offset (`ex += EPSILON`) is not clamped to the quadrant, so a point can land outside the bounds it was routed by. Both are fixed in the harvested copy with regression tests; the loose force-ratio invariant test (approx/exact ∈ (0.5, 2), `theta_approximation_reasonable`) is kept and tightened to (0.7, 1.4) at θ = 0.8.

### 8.2 Disconnected components and the peripheral ring

Harvested (`radial_layout`, `centroid`, `max_distance_from`): nodes with no layout-participating edge go to a radial ring around the force core — centered on the **centroid** (not the bbox midpoint), radius `(max_dist + 3)/2 × 1.1`, ordered by ideal angle toward each node's neighbor centroid but placed at uniform `2π/n` spacing (crossing reduction without label pile-ups). New: multiple non-singleton components get separate force islands packed left to right by descending node count with 4-cell gutters, replacing the seed's implicit single-blob assumption.

### 8.3 Warm start and stability (the incremental contract)

Harvested: surviving nodes start at their previous positions with the warm temperature and `warm_iterations` budget (§8.1); deleted nodes' positions are dropped; property deltas never touch positions. Added from the dynamic-layout literature: **new-node ramp-in** — a node added to a warm layout enters with force influence scaled linearly 0 → 1 over its first 10 iterations, so one arrival doesn't shove the neighborhood (the published technique for online dynamic graphs, and it directly serves ratatui's diff-based output: fewer moved cells per frame).

Contract (tested, §16.2-F): after a warm relayout triggered by adding ≤ 5% new nodes, ≥ 90% of surviving nodes move ≤ 2 cells.

---

## 9. Layered (DAG) layout — the real Sugiyama pipeline

Replaces the seed's 3-layer special case. All phases operate on `NodeIx` graphs with node widths in cells from `ResolvedMetrics`.

### 9.1 Pipeline (all decided)

1. **Cycle removal:** greedy Eades–Lin–Smyth (own ~60-line implementation; guarantees ≤ m/2 − n/6 reversals). Reversed edges are marked and restored at scene emission so arrowheads point true.
2. **Layering:** longest path from sources, then a pull-up tightening pass (shrink slack edges). Layer count unbounded; the caller may pin nodes to layers (`rank: Option<u32>` in a `LayeredSpec`) — this keeps the seed's "caller knows the strata" use case as an *option* instead of the only mode.
3. **Dummy chains** for edges spanning > 1 layer; chains carry their `EdgeIx` so routing and styling survive (the anti-"silently dropped edge" guarantee — invariant §16.2-B: every input edge appears in the SceneGraph).
4. **Crossing minimization:** alternating down/up **barycenter** sweeps with a **median** fallback pass; after each full sweep, count crossings exactly (bilayer counting via Fenwick tree, Barth–Jünger–Mutzel, O(E log V)); keep the best-seen ordering; stop after 4 sweeps or zero improvement. Ties break by `(sort_key, key)` — determinism, not a sort-stability accident.
5. **X-coordinate assignment:** v1 = **priority method** (Sugiyama '81): three median-alignment sweeps, node priority = degree (dummies = ∞ so long edges straighten), honoring real cell widths + 2-cell min separation. Deliberately chosen over Brandes-Köpf for v1: simpler, integer-friendly, handles variable node widths naturally. v1.1 upgrades to BK **with the 2020 erratum** behind the same interface.
6. **Y-coordinate assignment:** the harvested `flex_distribute` (`seed/ui/flex_layout.rs`: min/preferred/max, three proportional phases, deterministic remainder, `debug_assert`ed invariants) allocates layer bands and routing channels.
7. **Edge routing:** orthogonal, on reserved inter-layer channels. **Every merge/fork group gets its own channel row** (allocated by `flex_distribute`, assigned in leftmost-column order) — fixing the seed's shared bar row. Vertical pipes drop from the parent box's **actual bottom** to the bar, the bar runs to the child column, and the pipe drops to the child box's **actual top** — geometry always from measured content boxes, never from allocated band midpoints. This was the root cause of the seed's worst DAG rendering glitches and is now a stated invariant: **route from content bounds, never track bounds.** Junction glyphs come from the harvested position × continuation table (`JunctionKind` in `seed/ui/dag_layout/models.rs`, used by `build_merge_bar_with_continuation` in `routing.rs`: `└ ┴ ┘ ├ ┼ ┤ ┬`); crossings get the theme's crossing glyph. Style flows: every routed segment carries its `EdgeIx` → `StyleId` (the anti-"edge color discarded" guarantee; no raster re-scanning, ever).
8. **Scene emission:** boxes, pipes, bars, junctions, arrowheads, labels — all as SceneItems with bounds and payloads.

### 9.2 Why own implementation instead of rust-sugiyama / elk-rs (decided)

`rust-sugiyama` would bring petgraph into the required dep tree, targets continuous coordinates (we need integer cells + width-aware separation), and does no channel routing or glyph junctions. `elk-rs` is a heavyweight full-ELK port — wrong size for a core dependency. The pipeline above is ~1,200 lines against the harvested flex allocator and junction table; owning it keeps the integer-cell tuning loop tight. Both crates remain reference implementations for cross-checking test fixtures.

### 9.3 Tree and radial

- **TreeLayout:** the harvested centered-parent algorithm (`tree_layout` in `seed/graph/tree_layout.rs`: post-order widths, BFS depths, pre-order x, O(n), deterministic) — honestly documented as naive subtree-width packing, *not* Walker (the seed's module header claims "Walker O(n)"; the code has no contours or threads). Fine at terminal scale; proper Walker/Buchheim is a v2 item if deep-tree demand appears. Its orthogonal stem/bus/drop connector routing (`route_connectors`, minus its two unused parameters) feeds the same scene/junction machinery as §9.1. **Non-tree input is defined, not undefined:** the seed has no cycle guard. `TreeSpec { roots: Vec<K> }` names the roots (default: all in-degree-0 nodes, else the smallest node by sort tuple); the engine takes the BFS spanning forest from them in sort order, lays that out, and emits every non-tree edge (cross, back, second-parent) as a `Polyline` curve on `EdgesUnder` so invariant B still holds and nothing is dropped. Unreachable components become additional roots.
- **RadialLayout:** ring placement (§8.2 machinery) + the optional two-level **chord/Holten** overlay harvested intact from `seed/graph/chord.rs`: β = 0.85 bundling (`BUNDLING_BETA`), 3° arc gaps (`ARC_GAP_RAD`), inner/outer ring ratios 0.40/0.85, size-descending arcs with gap clamping (`usable ≥ π`), C⁰-continuous chained quadratics through the bundle root, dashed intra-group vs solid inter-group. Group ids are plain `u32` (the seed's chord code is already key-agnostic; only its `community_*` naming gets renamed `group_*`). One fix: the seed's `place_entities` finds each member's slot with a linear `position()` search, O(n²) per group; the port precomputes an index map.

---

## 10. Interaction

### 10.1 Event model and controller

```rust
pub enum InputEvent {              // library-native, defined in core; terminal crates convert INTO it
    Key(KeyInput),                 // KeyInput { code: KeyCode, mods: Modifiers } — a small enum, not crossterm's
    MouseDown(u16, u16, Button), MouseUp(u16, u16, Button), MouseMove(u16, u16),
    Scroll(i8, u16, u16), Resize(u16, u16),
}
pub enum Action { Pan(Dir), ZoomIn, ZoomOut, CycleNext, CyclePrev, NavTo(Dir), TogglePin, Clear, Select, … }
pub struct Controller { /* owns: viewport, selection, hover, pin, drag state, animations, key table */ }
impl Controller {
    pub fn handle(&mut self, ev: InputEvent, scene: &SceneGraph, metrics: &ResolvedMetrics) -> Vec<Outcome>;
    pub fn act(&mut self, action: Action, …) -> Vec<Outcome>;      // the key table maps Key → Action → act()
    pub fn tick(&mut self, dt: std::time::Duration) -> Redraw;     // ALL time enters here
}
pub enum Outcome {
    Selected(Option<NodeIx>), Hovered(Option<NodeIx>), EdgeSelected(Option<EdgeId>),
    Pinned(NodeIx, bool), DragCompleted(NodeIx, (f64, f64)), ViewChanged, Boundary(Dir),
}
```

- `GraphView<K>` wraps the controller: `gv.handle(ev)` and `gv.act(action)` forward to it with the current scene and metrics, and `gv.key(ix)` turns an `Outcome`'s index into the host's key when the host wants one. Outcomes carry indices so a frame of mouse-moves clones no keys (§15.2).
- The controller is a plain state machine over `(SceneGraph, ResolvedMetrics)`; **no `Instant` anywhere in the library** (the lint bans the type, not just `::now()`, because the seed's `PanAnimation` reads time through `start_time.elapsed()`; §17). Hosts call `tick(dt)` each frame; tests call `tick(16ms)` deterministically. This deletes the seed's animation-backdating test hack (`PanAnimation::force_complete` in `seed/app/types_excerpt.rs` rewinds a wall-clock start time by `duration + 1 s`).
- `Outcome` events are how the host reacts (egui_graphs' event-channel pattern, minus the channel — a returned Vec is simpler and allocation-cheap).
- **Terminal event conversion lives in the widget crate**, as `catena_ratatui::input::from_crossterm(&crossterm::event::Event) -> Option<InputEvent>` behind the `crossterm` feature (`None` for events catena has no use for, such as focus and paste). It cannot be a `From` impl: the core crate owns `InputEvent` and must not depend on crossterm even optionally, and the widget crate owns neither type, so a free function is the only shape the orphan rule allows. Any other backend (termion, termwiz) is one such function.
- Key bindings are a default table (`hjkl`/arrows pan, `+/-` zoom, `n/N` cycle, `Shift+arrows` spatial nav, `Enter` pin, `Esc` clear) mapping `KeyInput → Action`, fully remappable; the table lives in core (`Controller::keymap_mut()`), so a host without the widget crate still gets the defaults, and `act()` lets a host with its own key handling bypass the table entirely.
- Node drag is new (the seed drags windows and the minimap viewport, never nodes): a drag moves the node's canonical position, marks it pinned for the drag's duration, and emits `DragCompleted` on release; the host decides whether the pin persists.

### 10.2 Hit-testing (spec, fixing the seed's four defects)

The seed's hit-testing is `select_entity_at`, `hover_entity_at`, `select_edge_at` and `hover_edge_at` in `seed/app/selection_excerpt.rs`, over the `EdgeSpatialGrid` and `EdgeScreenPos` in `seed/app/types_excerpt.rs`.

1. **Nodes:** point-in-box against `ResolvedMetrics.boxes` (visual boxes — the same ones rendered; the seed's low-zoom "click selects a neighbor" bug is structurally impossible). Miss → nearest anchor within radius 3 cells using **aspect-corrected distance** `d² = dx² + (2·dy)²` (the seed uses a raw Euclidean `dist < 5.0` over the full label width, so a click 4 rows away selects a node that is visually 8 rows away). Node lookups go through a bucket grid (same 16×8-cell design as the seed's `EdgeSpatialGrid`, hit margin 2); no O(n) scans on mouse move — the seed's `select_edge_at` scans every edge linearly while its `hover_edge_at` uses the grid.
2. **Edges:** the harvested spatial bucket grid (bboxes inflated by the hit margin so boundary edges register in neighbor buckets), point-segment distance ≤ 1.5 cells in cell space (the seed's threshold is 2.0). Sub-cell (braille) precision is deliberately **not** used for hit-testing — a 0.5-cell-accurate click target is below pointer precision in a terminal; documented as a decision, not an accident.
3. **Priority:** topmost SceneGraph layer wins (nodes over edges); within a layer, the smallest bounds win (the most specific target); a remaining tie goes to the item whose node (or edge's `(from, to, rank)`) sorts first by the §4.1 tuple. The seed breaks ties by `HashMap` iteration order, so the same click can select different nodes in different runs.
4. **Every interactive SceneItem carries full 2-D bounds** — the row-only click table cannot recur. `catena` does not support rendering through wrapping widgets: the scene is positioned in cells and the widget renders it directly (the seed's visual-row reconstruction existed only because it drew through `Paragraph::wrap`).

### 10.3 Spatial keyboard navigation (new capability, decided algorithm)

From the focused node, direction d ∈ {up, down, left, right}: candidates are nodes whose aspect-corrected center offset has a positive component along d within a ±60° cone; score = `‖offset‖ₐ · (1 + 2(1 − cos θ))` (distance penalized by angular deviation); lowest score wins; an empty cone falls back to the nearest node in the half-plane. Wraps nothing (edge of graph = no-op, `Outcome::Boundary`). `n`/`N` keeps list-order cycling (sorted by `(sort_key, key)`), because both navigation styles have users.

### 10.4 Animations

All animations (pan glide 300 ms ease-out cubic `1 − (1 − t)³`, as in the seed's `PanAnimation::tick`; zoom exponential smoothing τ = 40 ms; the new-node ramp-in and the optional `LayoutPacing::Animated` settle of §4.2) advance exclusively in `tick(dt)` and are interruptible by retargeting. A `dt` of zero is a legal no-op and a `dt` above 250 ms is clamped to it, so a host that was suspended does not fast-forward a glide into a teleport. The `Redraw::Needed`/`Idle` return lets hosts skip rendering on quiet frames (the seed's dirty-flag economy, without owning the loop): `Idle` means no animation is running, no layout is dirty and no input changed state since the last render.

---

## 11. Stability contract (live-graph guarantees)

Published in the crate docs as testable guarantees, because live updating is the differentiator against all static prior art:

1. Property-only deltas move zero nodes (exact).
2. Pan never moves a node, and no pan or zoom ever re-runs a layout engine (exact). The one thing a zoom may do is the §6 zoom-out re-snap, which moves only nodes that would otherwise overlap and never the simulation's world positions.
3. Warm relayout after ≤ 5% node additions: ≥ 90% of survivors move ≤ 2 cells (statistical, seeded fixture suite).
4. Relayout keeps the focused node within 3 cells of its prior view position (anchor compensation).
5. Identical input ⇒ byte-identical frames across runs and platforms (full determinism; there is no RNG in the core at all, §8.1, and no hash-map iteration reaches the output, §4.1).
6. A quiescent scene re-rendered N times produces zero cell diffs (no hidden time/iteration-order dependence).
7. Every input edge is on screen or clipped, never dropped: for every edge there is a scene item, and for every scene item there is a model element (invariant B/C).

**Guarantee 5 across platforms is a dependency decision, not a hope.** IEEE arithmetic is deterministic, but `f64::exp`, `sin`, `cos`, `atan2`, `powf` and friends resolve to the platform's libm and differ at the last ulp between Linux glibc, macOS and MSVC; one ulp decides a `round()` often enough that a T2 golden committed on one OS would fail on another. So the core routes every transcendental call through `catena::fmath`, a thin module over the pure-Rust `libm` crate (same bits on every platform), and `cargo xtask lint` denies the `std` float methods (`.sin(`, `.cos(`, `.tan(`, `.exp(`, `.ln(`, `.log`, `.powf(`, `.powi(`, `.atan2(`, `.hypot(`, `.cbrt(`) everywhere in `catena/src` except `fmath.rs`. `sqrt` is correctly rounded by the IEEE standard and stays as is; float ordering uses `total_cmp`, never `partial_cmp().unwrap()`. The seed's own `save_visual_gallery` doc comment concedes that its graph canvases are non-deterministic ("HashMap iteration order in the FR layout"); T4 turns that concession into a failing test. The CI `check` matrix (§17) runs the committed goldens on all three OSes, so a platform divergence cannot hide.

---

## 12. Styling and theming

```rust
pub trait StyleResolver<K> {
    fn node(&self, key: &K, state: NodeState) -> NodeStyle;            // state: selected/hovered/pinned/dimmed/dragging
    fn edge(&self, edge: EdgeRef<'_, K>, state: EdgeState) -> EdgeStyle; // EdgeRef: id, from, to, rank, &EdgeSpec
    fn group(&self, group: u32) -> PaletteColor;                        // radial arcs (group hulls: backlog, §19)
}
```

- The resolver is the **only** place app meaning becomes color — the seed's 9-variant domain `ColorPalette` and 11-entity-type theme table become one user callback. The host's per-node data (metrics, categories, anything) stays in the host's own map, keyed by `K`; the resolver looks it up there. `catena` ships `DegreeResolver`, `GroupResolver` (8-color palette with offset rotation — the seed's trick for distinguishing overlapping palette families) and `MonoResolver` as examples. Resolvers are called once per visible node and edge per frame, after a delta or a state change; a quiescent frame reuses the cached `StyleId`s.
- The harvested **gradient library** (`seed/graph/render.rs`: `confidence_color` red → yellow → green, `centrality_color` blue → cyan → yellow → red with soft cap at 20, `pagerank_color`, `dim_color` = RGB/4) ships as pure `fn(f64) -> Rgb` helpers in `style::gradients` — including the harvested **visible-range renormalization** lesson (normalize a metric to the visible set's min/max or the gradient collapses; the helper takes `(value, visible_min, visible_max)`). The seed's `convergence_color` is engine-specific and is dropped. `dim_color` is `Rgb → Rgb` only: the seed's version maps every non-RGB color to a fixed `Rgb(20, 18, 30)`, so its 16-color theme emits true-color glow on terminals that chose that theme precisely because they cannot show it. In `catena` the glow color is the theme's `glow` slot, resolved per theme, and the T3 ansi16 fidelity snapshot asserts that an `ansi16` frame contains no `Rgb` at all (ledger row 29).
- `Theme` = semantic color slots (background, edge_default, edge_highlight, glow, selection, letterbox) + `GlyphSet` (junctions, arrowheads, box drawing, dash patterns) + sizing. Four built-ins: `dark`, `light`, `ansi16` (16-color fallback, ported from `seed/ui/themes/ansi16.rs`), `ascii` (new: a pure-ASCII glyph set that proves the indirection and serves `NO_COLOR` and dumb terminals). Selected node = reversed video; pinned = in-box `*` marker; dimmed = attr + color dim (all the seed's conventions).
- Glyphs are `char`, not `&str` (the seed's multi-codepoint glyph slots silently truncate in 12 call sites; a single `char` makes the truncation unrepresentable). Wide-glyph support is explicitly not a theme feature.

---

## 13. Public API — the 60-second view

```rust
use catena::{Blitter, EdgeSpec, ForceParams, GraphView, LayoutKind, NodeSpec, Outcome};

let mut gv: GraphView<String> = GraphView::builder()
    .layout(LayoutKind::Force(ForceParams::default()))
    .blitter(Blitter::Braille)
    .theme(catena::theme::dark())
    .style(catena::style::DegreeResolver::default())   // any StyleResolver<String>
    .build();

gv.update(|tx| {
    tx.add_node("ada".to_string(), NodeSpec::label("Ada Lovelace"))?;
    tx.add_node("babbage".to_string(), NodeSpec::label("Charles Babbage"))?;
    let _id = tx.add_edge(&"ada".into(), &"babbage".into(), EdgeSpec::directed())?;
    Ok(())
})?;   // Err ⇒ nothing changed

// host event loop (crossterm shown; any backend is one conversion function):
if let Some(ev) = catena_ratatui::input::from_crossterm(&crossterm_event) {
    for outcome in gv.handle(ev) {
        if let Outcome::Selected(Some(ix)) = outcome { let key: &String = gv.key(ix).unwrap(); /* react */ }
    }
}
gv.tick(dt);                                                                   // the only clock
frame.render_stateful_widget(catena_ratatui::Graph::new(), area, &mut gv);     // ratatui
// or, no terminal at all:
let ascii: String = gv.render_to_string(120, 40);                              // CellGrid under the hood
```

`GraphView<K>` is `Send` whenever `K` and the resolver are, and it is `!Sync` by design (it caches per frame); a host that renders from another thread moves it there. The builder's defaults are the ones shown, so `GraphView::<String>::builder().build()` is a complete, working configuration.

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
| 10 | Zoom rescale unsound over collision displacements | Zoom-out re-snap (cheap, not re-simulate) within a semantic level when the last snap displaced anything (§6); the contract in §11.2 says so |
| 11 | `iterations` parameter silently floored to 100 | Params respected; the wisdom lives in named presets (§8.1) |
| 12 | Quadtree self-repulsion at `MAX_DEPTH`; jitter escaping quadrant bounds | Fixed in the harvest + regression tests (§8.1) |
| 13 | `BrailleCanvas::get_cell` panics unchecked while its sibling accessors clip silently | One contract: all canvas accessors clip; `try_` variants return `Option` |
| 14 | Unbounded rasterization of mostly off-screen segments | Clip before rasterizing (§6) |
| 15 | Paint-behind blit breaks overlay edges at crossings | Compositor OR-merges sub-cell bits across layers (§7.4) |
| 16 | Theme symbol/color tables out of sync (`ThemeEntityTypes` has 11 types; `type_symbol` maps 6) | No domain tables; one `StyleResolver` callback (§12) |
| 17 | DAG: layer-skipping edges silently dropped | Dummy chains; invariant test: every input edge reaches the scene (§9.1) |
| 18 | DAG: all merge bars share one row | Per-group channel rows via `flex_distribute` (§9.1) |
| 19 | DAG: edge color discarded, reconstructed by raster scanning | `StyleId` flows through routing (§9.1) |
| 20 | DAG: renderer draws card/bus geometry the layout never modeled | SceneGraph completeness rule (§7.4) |
| 21 | Row-only click targets (leftmost box wins) | SceneItems carry 2-D bounds (§10.2) |
| 22 | Routing from flex-zone midpoints, not content bounds (pipe gaps) | Stated invariant: route from measured content bounds (§9.1) |
| 23 | Three ad-hoc z-order conventions | One compositor layer policy (§7.4) |
| 24 | Operator-precedence bug in a text-highlight fallback (`!a && b ‖ c`) | Not carried: text highlighting is a host concern |
| 25 | Duplicated spiral/label-grid/minimums/style-run code drifting apart | Single implementations by construction; file-size caps (§17) |
| 26 | `Instant::now()` in `navigation.rs` and `Instant::elapsed()` in `types_excerpt.rs` (untestable time) | All time via `tick(dt)`; the `Instant` type itself is denied by `xtask lint` (§10.4, §17) |
| 27 | Semantic-zoom levels 3–5 are dead: `layout_fr` clamps labels to 22 columns before `visual_label_width` sees them | One measurement, no hidden clamp; all six levels live, bounded only by `max_label_cols` (§5) |
| 28 | "Minimum 2-row vertical gap" is a comment, not code: `rect_overlaps` tests label cells only, so stacked nodes touch | Collision boxes are inflated by one row each side; a golden with stacked nodes pins it (§6) |
| 29 | `dim_color` maps non-RGB colors to a fixed `Rgb(20,18,30)`, so the 16-color theme emits true-color glow | Glow is a theme slot; T3 asserts an `ansi16` frame contains no `Rgb` (§12) |
| 30 | Hit-test ties broken by `HashMap` iteration order; node hit radius is raw Euclidean over the full label width | Deterministic tie-break by sort tuple; aspect-corrected radius over visual boxes (§10.2) |
| 31 | Occlusion centroid double-subtracts overlapping windows | Exact unoccluded region via a cell bitmap (§6) |
| 32 | Pan rounded to `i32` on every zoom tick, so long glides drift | `f64` pan/zoom state, rounded once in the derive step (§6) |
| 33 | Two `word_wrap`s with different semantics (byte length vs `chars().count()`, paragraph handling, empty-input result) | One `word_wrap` in display columns with stated semantics (§18) |
| 34 | `TreeLayout` has no cycle guard: a back edge recurses forever | Spanning forest from declared roots; non-tree edges drawn as overlay curves (§9.3) |

Rows 1–26 come from the review that preceded this plan; rows 27–34 were found by the pre-build audit of the seed against this document and are confirmed in the seed's code, not inferred.

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

A `FrameScratch` struct pools every per-frame buffer (sub-cell canvases via `clear_and_resize` — `BrailleCanvas::clear_and_resize` exists in the seed, `#[allow(dead_code)]`, and is never called; the label mask; segment/hop vectors; the scene item vec). Steady-state frame = zero heap allocations, asserted by a **test**, not a bench: `catena`'s integration-test target installs a counting `#[global_allocator]` (an integration target is its own binary, so it may), renders a 200/400 fixture three times, and asserts the third frame allocated zero bytes. No per-frame `String` or key clones: labels are measured at delta time, the scene carries indices (§7.4), and the renderer borrows.

---

## 16. Testing strategy

The hard problem, treated as a first-class deliverable. Design principles from the evidence: (a) the source application's ~40-minute functional tier was dominated by its embedded engine and event-loop mirroring — a pure library has neither, so **every tier below runs in seconds except the optional PTY smoke**; (b) the seed's DAG-glitch post-mortem showed screen-scraping tests missed geometry bugs that only positional/property assertions caught — so **property tests outrank snapshots** in the pyramid; (c) ratatui's insta recipe can't assert color — so the **SVG/hash tier is the style oracle**; (d) zellij's named polling predicates and gitui's insta filters are the proven patterns for the end-to-end edge.

All tiers live in each crate's single integration-test target or its lib unit tests (§3.1); a tier is a `mod`, never a new target.

### 16.1 Tier taxonomy

| Tier | Mechanism | Gate | Runtime | Count target at 1.0 |
|---|---|---|---|---|
| T1 Property/unit | proptest + colocated unit tests, pure functions | always (`cargo test`) | seconds | ~400 fns, every invariant in §16.2 |
| T2 Golden text | `CellGrid::to_string()` → `insta::assert_snapshot!` | always | seconds | ~60 snapshots |
| T3 Style/visual | `CellGrid` → SVG → SHA-256 vs committed hash; `.fail.svg` on mismatch | always | seconds | ~30 snapshots + non-gating PNG gallery |
| T4 Determinism/flicker | buffer-diff oscillation detector, steady-state zero-diff, cross-run hash equality | always | seconds | ~15 scenarios |
| T5 Widget composition | ratatui `TestBackend` + insta (widget crate only) | always | seconds | ~15 |
| T6 Fuzz | cargo-fuzz + `arbitrary` event/topology streams | nightly CI job | budgeted 10 min | 3 targets |
| T7 PTY smoke | portable-pty + vt100 against the `catena-demo` bin | `--features pty-tests`, serialized, own CI job | ~1 min | 5 tests |

No feature-gated 40-minute tier exists because nothing needs one: the expensive things the source application's harness simulated (async fetches, enrichment races, main-loop mirroring) are outside the library by construction.

### 16.2 T1 — the invariant catalog (property tests, proptest with shrinking)

Strategies generate: node counts 0–300, labels including empty/emoji/CJK/combining marks, edge sets including self-loops/parallels/disconnection, zoom ∈ [0.1, 4.0], viewport 3×3 → 300×100, all from proptest's own seeded RNG (the library under test has none, §8.1). Named invariants:

- **A. No-overlap:** after snap, no two visible node boxes intersect (assert geometry, not screen text).
- **B. Edge conservation:** every input edge yields exactly one scene path whose endpoints touch its nodes' anchor cells (catches silent drops; would have caught the seed's layer-skip drop and dedup collapse).
- **C. Containment:** every SceneItem's raster output stays within its declared bounds; every rendered cell traces to a SceneItem (completeness both ways).
- **D. Determinism:** same input ⇒ identical scene, across two fresh `GraphView`s and shuffled insertion orders of the same nodes and distinct edges (sort-key ordering, not HashMap luck); parallel edges of one pair keep their insertion rank by contract (§4.1), so the shuffle preserves order within a pair.
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

The pure `CellGrid` render of curated fixture scenes (the Ascii blitter doubles as the readable-diff renderer): the 12 canonical graphs of §20 × relevant layouts × 2 viewport sizes. `insta` with filters redacting nothing — because the library is deterministic by construction, snapshots need **zero** redaction; any needed redaction is itself a determinism bug (inverting gitui's filter pattern into an assertion of design health). `cargo insta review` is the update workflow; first-run failures are by design.

### 16.4 T3 — style-aware visual snapshots (the color oracle)

Harvested from `seed/tests/visual/svg_renderer.rs` (`buffer_to_svg`, `buffer_to_hash`, `color_to_css`) and `seed/tests/visual/snapshots.rs` (`assert_visual_snapshot`, `save_visual_gallery`) nearly verbatim into `catena-testkit`: `CellGrid` → self-contained SVG (run-length spans, REVERSED handled, full color mapping) → canonical per-cell hash line `{glyph}|{fg_css}|{bg_css}|{attrs:x}` → SHA-256 vs committed `.hash`; a mismatch writes `{name}.fail.svg` for eyeball/PR-diff inspection; `CATENA_UPDATE_SNAPSHOTS=1` refreshes. This closes the documented ratatui-ecosystem gap (the insta text recipe can't see color) with the Textual-SVG approach, and it is precisely what a *visualization* crate must test: selection highlight, glow halo composition, dim states, per-layer edge colors, ansi16 fallback fidelity. The `png` feature adds a resvg-rasterized **non-gating** gallery (`buffer_to_png`; `tests/gallery/*.png`) regenerated on demand for human/multimodal review — kept non-assertive by design.

### 16.5 T4 — determinism and flicker

Harvested oscillation machinery (`buffer_diff_details` and `diff_count` in `seed/tests/functional/test_blink_core.rs`), now cheap enough to run always-on:
- **Steady state:** render a quiescent scene 50 consecutive frames (`tick(16ms)` between) → zero cell diffs after frame 2 (catches iteration-order nondeterminism, hidden time reads, canvas-reuse leaks).
- **A→B→A detector:** across interaction scripts (pan burst, zoom glide, select/deselect storm), assert no frame equals frame-2-ago while differing from frame-1-ago (the flicker signature), and that the frame-hash count over a 200-tick script is at most the script's distinct logical states.
- **Cross-run:** two fresh processes (spawns of the testkit's `catena-render-hash` bin via `env!("CARGO_BIN_EXE_catena-render-hash")`, so the test lives in the testkit's integration target) produce identical hashes — catches ASLR-dependent HashMap ordering escaping into output. The same bin's output for each canonical fixture is committed as a `.hash` file, so the three CI OSes are compared against one another, not just against themselves (§11.5).

### 16.6 T5 — widget composition

ratatui `TestBackend` snapshots of the full widget (graph + minimap + legend in a layout), resize storms (10 random resizes → invariants A + C hold), and the adapter's style translation. This is the only tier that needs ratatui proper.

### 16.7 T6 — fuzzing

Three `cargo-fuzz` targets with `arbitrary`-derived inputs (nightly CI job, 10-minute budget, corpus committed), in the `fuzz/` crate excluded from the workspace. The fuzz crate defines its own `Arbitrary` mirror types (an event enum, a mutation enum) and maps them onto `InputEvent` and `Tx` calls, so `catena` carries no `arbitrary` feature or dependency:
1. `fuzz_events`: arbitrary `Vec<InputEvent>` (including mid-drag resizes, scroll storms, clicks at `u16::MAX`) against a fixture graph → no panic; invariants A/C/I hold at the end.
2. `fuzz_topology`: arbitrary interleaved `Tx` mutations and renders → no panic, edge conservation holds, no `NodeIx` leaks after removals.
3. `fuzz_layout_input`: arbitrary graphs straight into each layout engine → terminates within iteration bounds, all nodes placed, finite coordinates.

libFuzzer is Unix/nightly: the fuzz job runs on Linux CI only; Windows correctness is covered by T1–T5, which run on all three OSes.

### 16.8 T7 — PTY smoke (the only real-terminal tier)

Five tests against the `catena-demo` bin (located through `env!("CARGO_BIN_EXE_catena-demo")`, which is why it is a bin and not an example, §3) via `portable-pty` + `vt100`, serialized through a process-wide mutex in the `pty` module and additionally `--test-threads=1` in CI, own CI job, `TERM=xterm-256color` pinned:
1. Launches into the alternate screen; a braille-range glyph appears within 2 s (a named polling predicate à la zellij: `wait_until("graph visible", |screen| ...)` — no fixed sleeps).
2. A pan key produces a changed frame; quiescence returns (steady state via polling buffer equality).
3. Resize (SIGWINCH) rerenders within bounds; no panic.
4. Clean exit restores the main screen; **zero bytes written outside the alternate-screen bracket and zero stderr output during steady state** — the structural test for the startup-clobber bug class (an app writing to the main screen before entering the alternate one).
5. A `NO_COLOR=1` run emits no SGR color sequences (scan raw bytes with the `CaptureBuf` byte-capture pattern from `seed/tests/functional/test_blink_core.rs`, applied at the PTY). The demo bin, being host code, is where `NO_COLOR` is read and turned into `theme::ascii()` + `Blitter::Ascii`; the library never sees the variable.

Explicitly **not** built: a `tick()`-mirrors-the-main-loop harness family. The source application needed one only because an embedded engine raced its render loop, and its five-copy fetch-list duplication was its worst maintenance trap; `catena` has no engine and one canonical `tick`.

### 16.9 What is deliberately untested and why

Terminal-emulator glyph rendering fidelity (braille font quality varies by terminal — that's why blitters are pluggable, not something tests can fix); wall-clock animation *feel* (τ values are judgment; the math is tested); ratatui's own diff correctness (upstream's job; T4 ensures we feed it stable frames).

---

## 17. Gates and CI (decided)

**Local gates** (hooks enabled per clone with `git config core.hooksPath .githooks`):

- **`scripts/smoke.sh` — commit gate, under 60 s, scoped to the diff.** `cargo fmt --all -- --check`; `cargo xtask lint`; `cargo clippy -p <touched crates> --all-targets -- -D warnings`; `cargo test -p <touched crates> --lib`. Quiet on success (one OK line with the pass count); on failure prints only the failing tests. Skips loudly, never silently.
- **`scripts/regression.sh` — push gate, correctness only.** `cargo fmt --all -- --check`; `cargo xtask lint`; `cargo clippy --workspace --all-targets --all-features -- -D warnings`; `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features`; `cargo test --workspace -- --include-ignored` (T1–T5). No benchmarks, no fuzzing, no PTY tier.
- Neither script exports `RUSTFLAGS` or sets `CARGO_TARGET_DIR` (§3.1). Never re-run a gate on an unchanged tree.

**`cargo xtask lint`** (fails on match in `catena/src`, `catena-ratatui/src` and `catena-testkit/src`; `seed/` and any `*.seed.rs` staging file (§18) are never scanned):
- `\bInstant\b|SystemTime` (time injection, §10.4 — the type, because `.elapsed()` is as much a clock read as `::now()`; `Duration` is fine);
- `thread_rng|from_entropy|rand::` (no randomness in core, §8.1; the testkit's generators use the crate-local `SplitMix64`);
- `\.(sin|cos|tan|exp|ln|log2|log10|powf|powi|atan2|hypot|cbrt)\(` on anything, in `catena/src` outside `fmath.rs` (platform-independent math, §11);
- `partial_cmp\(.*\)\.unwrap` (float ordering goes through `total_cmp`, §11);
- `.len()` on identifiers matching `label|title|name|symbol` (the unicode-width rule, §5 — an allowlist keeps false positives manageable);
- `unwrap()` outside tests (the harvested surface has zero; `expect` with an invariant message is allowed);
- `std::env|std::fs|std::io|std::net|std::process|println!|eprintln!` in `catena/src` and `catena-ratatui/src`, excluding each crate's `src/bin/` (the §2.2 fences, mechanically; the bins are host code and read the environment on purpose);
- file length: warn above 500 lines, fail above 750 (the source application's 1,000-line modules are where its duplications hid);
- the scaffold rules (§3.1): any `RUSTFLAGS` in `scripts/`, `.githooks/` or `.github/`; any `rustflags` in `.cargo/config.toml`; more than one target under any crate's `tests/` or `benches/`;
- after M5: any `*.seed.rs` file or a `seed/` directory (the harvest is complete or it is not).

Each rule has a test in `xtask` that plants a violation and watches the rule fail. The rules are regexes over source text, not a type-checked analysis; they are tripwires, and a false positive is silenced by a one-line `// lint-allow: <rule> — <why>` comment on the offending line, which the lint recognizes and counts in its summary so allowances stay visible.

**CI (GitHub Actions; every `uses:` pinned by full commit SHA; the milestone in brackets is when the job lands):**
- **check** (linux/macos/windows × stable) [M0]: runs `scripts/regression.sh` (`shell: bash`). Committed T2/T3 goldens and `.hash` files are the same on all three OSes by §11.5; the matrix is where that is proved.
- **msrv** (linux) [M0]: `cargo msrv verify` against the workspace `rust-version`.
- **dco** (PRs) [M0]: every commit carries `Signed-off-by`.
- **coverage** (linux) [M2]: `cargo llvm-cov`, line-coverage gate ≥ 80% on `catena`.
- **bench** (linux, main only) [M2]: `scripts/check-perf.sh` against the runner's fingerprint entry, plus criterion `--save-baseline` with any regression over 15% posted as an advisory PR comment (advisory because blocking on microbenchmark noise is how benches get deleted; the 1.5× ratchet is the wall).
- **fuzz** (linux nightly cron) [M3]: 3 targets × 200 s, corpus cached.
- **pty** (linux + macos) [M5]: the T7 job, `cargo test -p catena-ratatui --features pty-tests -- pty:: --test-threads=1`.

`regression.sh` itself is deterministic-correctness only, so a developer and the `check` job see the same gate; the other five jobs add what a local push gate must not carry (benchmarks, fuzzing, a real PTY, a coverage build).

---

## 18. Seed manifest (file-by-file disposition)

Paths are under `seed/`. Line counts are of the seed file. "Verbatim+" = move, rename symbols, apply the listed fixes, keep the tests. "Reference" = read it to port the named pieces, then delete it; it never moves into a crate.

| Seed file | Lines | Disposition | Target | Required changes |
|---|---|---|---|---|
| `graph/braille.rs` + `graph/braille_tests.rs` | 472 + 591 | **Verbatim+** | `catena/src/raster/` | Generalize `BrailleCanvas` to `SubCellCanvas(SUB_W, SUB_H)`; unify the four Bézier loops into one sampler; make `get_cell` clip like its siblings; keep the dash-phase doc comment |
| `graph/quadtree.rs` | 385 | **Verbatim+** | `catena/src/layout/force/` | Fix the `MAX_DEPTH` self-repulsion and the jitter-quadrant bounds; keep it crate-internal; tighten the force-ratio test to (0.7, 1.4). It has no RNG: the "jitter" is a fixed `1e-4` offset and stays one |
| `graph/layout_fr.rs` + `graph/layout_fr_tests.rs` | 574 + 492 | **Port** | `catena/src/layout/force/` | `Uuid` → `NodeIx`; remove the three `visual_label_width` up-calls (metrics are injected); honor `iterations` (drop `.max(100)` cold / `.max(30)` warm); drop the placement-time `* 0.5` aspect factor (§6); isotropic snap; one shared, 50-ring spiral fn; an enforced 2-row gap; the two `name.len()` sites become display widths; keep the name-sort comment verbatim as the determinism rationale |
| `graph/chord.rs` + `graph/chord_tests.rs` | 394 + 579 | **Verbatim+** | `catena/src/layout/radial/` | Rename `community_*` → `group_*` (`CommunityArc` → `GroupArc`, `place_communities` → `place_groups`); keep all constants (β = 0.85, 3° gaps, ring ratios 0.40/0.85) and the C⁰/limit tests; replace the O(n²) `position()` lookup in `place_entities` with an index map |
| `graph/zoom.rs` | 92 | **Verbatim+** | `catena/src/geometry/` | Thresholds become a configurable table with these defaults; the file has no clamp constants (the [0.1, 4.0] literals live in `navigation.rs`) and gains them |
| `graph/tree_layout.rs` + `graph/tree_layout_tests.rs` | 181 + 513 | **Verbatim+** | `catena/src/layout/tree/` | Fix the "Walker" module header; remove `route_connectors`'s two unused parameters; add the spanning-forest guard of §9.3 (the seed has none); wire `route_connectors` output into the scene/junction machinery |
| `graph/render.rs` + `graph/render_tests.rs` | 521 + 307 | **Port, two pieces** | `catena/src/raster/`, `scene/`, `style/gradients.rs` | Hop/crossing math (`segment_intersection`, `HOP_RADIUS`, the hop pass in `render_edges`): spatial-grid candidates + hop budget, keep the endpoint-exclusion constants and the PCB convention; drop the `seen_pairs` reciprocal-edge merge (§7.3). Gradients (`confidence_color`, `centrality_color`, `pagerank_color`, `dim_color`): pure `fn(f64) -> Rgb` with an explicit `(value, min, max)` signature; `dim_color` becomes `Rgb → Rgb` only (§12). Drop the domain colorers (`type_color`, `entity_color*`, `type_symbol`) and the engine-specific `convergence_color`; finish the truncated doc comment on `render_edges` |
| `graph/render_overlay.rs` + `graph/render_overlay_tests.rs` | 506 + 415 | **Port** | `catena/src/scene/` | One label-mask builder replacing `render_canvas_to_buf` / `render_canvas_to_buf_overwrite`; glow via `Surface::patch_bg` with the theme's `glow` slot; drop the early return on negative coordinates in `render_node` (clip instead, §6); drop its `word_wrap` (byte-length, `\n`-paragraphs, no long-word split, `[""]` on empty) in favor of the one in `raster/text.rs`; the three `label.len()`/`title.len()`/`word.len()` sites become display widths |
| `graph/mod.rs` | 497 | **Reference** | — | The orchestrator not to rebuild. Its eighteen sequential jobs are redistributed: visibility snapshot and invalidation → delta classification §4.2; degree counting (O(V·E) there) → the store's incident lists §4; force/radial partition, FR, ring → §8; canonical merge, anchor compensation, derive → §6; render-entity/relation building, edge screen positions, spatial grid → §7.4 and §10.2; `render_graph` and overlays → the compositor §7.4; the `let _ = width;` overlay-width discard (line 92) is ledger row 2 |
| `ui/dag_layout/models.rs`, `builder.rs`, `ordering.rs`, `positioning.rs`, `routing.rs`, `engine.rs`, `engine_tests.rs`, `render_ascii.rs`, `mod.rs` | 186, 136, 151, 429, 480, 512, 453, 107, 32 | **Port selectively** | `catena/src/layout/layered/` | Keep: the `JunctionKind` table (9 kinds), flex-driven Y bands (`plan_tiers`, `assign_y`), box building (`DagBuilder`), the `render_to_string` ASCII pattern (`render_ascii.rs`; `mod.rs` and `engine_tests.rs` import it from an un-seeded `render` module, so repoint the import), the golden style of `engine_tests.rs` (minus its four `eprintln!`s). Replace: ordering, positioning and routing algorithms per §9.1. Drop: the 3-layer `Layer` enum, the dead `build_fork_bar`/`build_merge_bar`/`build_vertical_pipes` family (used only by their own tests), the `DagMeta` `Uuid` field, the hard-coded `EdgeColor::Neutral`. `mod.rs` re-exports modules that are not seeded (an app bridge and two app renderers); ignore them |
| `ui/flex_layout.rs` | 418 | **Verbatim+** | `catena/src/layout/layered/flex.rs` | The crown jewel of the DAG side. One doc fix: its comment says overflow "truncates the last item"; the code gives the first item that no longer fits the remainder and later items zero, which is the better behavior. Correct the comment and pin the behavior with a test |
| `ui/box_layout.rs` + `ui/box_layout_tests.rs` | 535 + 379 | **Port one function** | `catena/src/raster/text.rs` | Take `word_wrap` and its tests, converting its `chars().count()` to display columns. Stated semantics of the one `word_wrap`: input `\n` starts a new line, a word wider than the width is split at the width, empty input yields an empty `Vec`, every output line measures ≤ width; the rest of the file (row and box rendering for the app's panes) is reference for box drawing only |
| `ui/theme.rs`, `ui/themes/mod.rs`, `ui/themes/ansi16.rs` | 366, 23, 192 | **Port** | `catena/src/style/` | Drop `ThemeEntityTypes`; glyphs `&str` → `char`; keep the ansi16 theme and add the new `ascii` theme as proof of indirection. `themes/mod.rs` lists five more app themes that are not seeded and not wanted |
| `ui/vector_minimap.rs` | 127 | **Reference** | `catena-ratatui/src/minimap.rs` | Rewritten thin over core: the minimap is a second `GraphView` render at a fixed low zoom into a small Surface + a viewport-rectangle overlay |
| `app/types_excerpt.rs` | 176 | **Verbatim+ / Reference** | `catena/src/interact/` | `EdgeSpatialGrid` (+ `EdgeScreenPos`): Verbatim+, generalized to nodes too, generic over `NodeIx`/`EdgeIx` instead of `Vec<Vec<Uuid>>`. `PanAnimation`, `ZoomAnimation`: reference for §10.4, re-expressed over `tick(dt)`; both `start_time: Instant` and `.elapsed()` go, and the `ZoomAnimation` doc comment's 65%/88%/96% figures (which describe a 0.65 lerp, not the 0.35 the code uses) are not carried |
| `app/navigation.rs` | 736 | **Port as free functions** | `catena/src/interact/` | Zoom-anchor inversion (`adjust_pan_for_zoom`, without its per-tick `i32` rounding), exponential smoothing (`tick_zoom_animation`), ease-out pan (`tick_pan_animation`), occlusion-aware centering (`effective_visible_center`, with the union fix of §6 and without the `windows[&id]` index that can panic), the zoom steps and clamps (`zoom_in`/`zoom_out`) — extracted from `impl App` into the `Controller`. The file's one `Instant::now()` (line 312) is why the move commit uses the staging suffix below |
| `app/selection_excerpt.rs` | 243 | **Reference** | `catena/src/interact/` | The four hit-test methods §10.2 replaces; the defects in §14 rows 1, 21 and 30 live here |
| `tests/visual/svg_renderer.rs` + `tests/visual/snapshots.rs` | 454 + 140 | **Verbatim+** | `catena-testkit/src/svg.rs` | `Buffer` → `CellGrid` (so the move happens in M0 and the port in M1, when `CellGrid` exists); keep the hash-line canonicalization exactly (positional `{sym}|{fg}|{bg}|{mods:x}`, reverse-adjusted colors, `" "` for continuation cells — it was designed for cross-version stability); rename the update env var to `CATENA_UPDATE_SNAPSHOTS` |
| `graph/braille_tests.rs` helper quartet | (in the 591 above) | **Verbatim** | `catena-testkit/src/braille_asserts.rs` | `lit_pixels`, `assert_8_connected`, `assert_endpoints_exact`, `assert_no_duplicates` — the single most valuable harvest per the review; the rest of the file follows `braille.rs` |
| `tests/functional/test_blink_core.rs` | 558 | **Port** | `catena-testkit/src/oscillation.rs` | Take `buffer_diff_details`, `diff_count` and the `CaptureBuf` pattern; strip the app harness and its eight `#[tokio::test]` async tests (nothing async survives); operate on `CellGrid` snapshots |
| `fixtures/community.json` | 184 | **Verbatim** | `catena-testkit/fixtures/community.json` | None (§20) |

Not seeded, on purpose: the host's window manager and floating windows (a host concern; the occlusion input of §6 covers the interaction), its full-screen graph view and legend pane (rewritten from §12 and §3), its DAG app bridge and grid renderer (dropped). Also absent, and referenced by seed files that are present: `graph/mod_tests.rs` (`mod.rs` line 496), `dag_layout/{app_bridge,render,render_grid}.rs` and five of the six theme files listed in `themes/mod.rs`. None is wanted; a port that hits one of those references deletes the reference.

**Harvest procedure — two commits per seed file.** (1) `git mv seed/<path> <target>.seed.rs` with no content change, message `chore(harvest): move seed/<path> verbatim to <target>.seed.rs`. The `.seed.rs` suffix is the staging state: cargo ignores the file (no module declares it), and `cargo xtask lint` skips it (§17), so a seed file that still contains `Instant::now()`, byte-length `.len()` or an `expect` can be moved without tripping the commit gate or needing `--no-verify`. (2) The port: `git mv <target>.seed.rs <target>`, declare the module, apply the required changes from the table, keep or port the tests, message `refactor(<area>): port <file> — <changes>`. Git's rename detection pairs the two moves with the edit, so the diff of commit 2 is the whole delta from the seed, reviewable forever. A Reference file is read, then deleted in the same commit as the port it informed. A file whose port spans milestones (the SVG renderer: moved in M0, ported in M1) simply stays staged in between. `seed/` is empty, and removed, by the end of M5, and no `*.seed.rs` remains (M5's `verify:` and the post-M5 lint rule check both).

---

## 19. Milestones

Sequenced so every milestone ends green and demonstrable. Each closes on its `verify:` command, observed to exit 0.

**M0 — Bootstrap + harvest drop.** In this order, each step its own commit or commit pair, so the tree is green after every one:
1. Workspace skeleton with the §3.1 scaffold in the first crate-creating commit (root manifest, the three per-crate manifests with `autotests = false` and one `[[test]]`/`[[bench]]` each, profiles, workspace lints, `.cargo/config.toml` holding only the xtask alias, `thiserror` in `catena`, `libm` and the empty `fmath.rs`), `CONTRIBUTING.md` (DCO + contributor license grant per §3), `TODO.md` created from this section.
2. `xtask` with `lint` and its planted-violation tests; `scripts/smoke.sh`, `scripts/regression.sh`, `.githooks/`; `git config core.hooksPath .githooks` noted in `CONTRIBUTING.md`.
3. CI skeleton: `check` via `regression.sh` on the three-OS matrix, `msrv`, `dco`.
4. The pure modules harvested by the §18 procedure, one commit pair each: braille canvas (+ tests), quadtree, chord, zoom table, flex, `word_wrap`, tree_layout, compiling in their new homes with their tests; `catena-testkit` with the braille asserts and `fixtures/community.json` plus its loader; the SVG renderer moved to its `.seed.rs` staging name (ported in M1).
verify: `./scripts/regression.sh` exits 0; `cargo test -p xtask` exits 0, including one planted-violation test per lint rule; `test -z "$(git ls-files seed/graph/braille.rs seed/graph/braille_tests.rs seed/graph/quadtree.rs seed/graph/chord.rs seed/graph/chord_tests.rs seed/graph/zoom.rs seed/ui/flex_layout.rs seed/graph/tree_layout.rs seed/graph/tree_layout_tests.rs seed/fixtures seed/tests/visual)"`; `test -f CONTRIBUTING.md -a -f TODO.md`; `grep -q Signed-off-by .github/workflows/*.yml`.

**M1 — Raster + scene.**
`Surface`, `CellGrid`, the `SubCellCanvas` generalization, all four blitters (braille, half-block, sextant, ascii — the sextant encoder is a 2×3 table once the canvas is generic, so it lands here, not in M5), the compositor with OR-merge, the label mask, clipping, `SceneGraph` with both `Route` kinds, the text-cell rule of §7.1, the SVG renderer ported onto `CellGrid`. T2 goldens for primitive scenes; the T3 pipeline live.
verify: `cargo test -p catena raster:: scene::` exits 0; `cargo insta test` exits 0; a committed `.hash` set exists and a test that perturbs one color shows T3 failing; `test ! -e catena-testkit/src/svg.seed.rs`.

**M2 — Graph model + force layout + viewport.**
`GraphView`/`Tx`/`GraphError`/delta classification, `EdgeId`, `ResolvedMetrics`, `ForceLayout` (ported, both repulsion modes, synchronous and `Animated` pacing), `GridSnapper` with `cell_aspect` and the zoom-out re-snap, canonical/derived viewport, warm start + ramp-in + anchor compensation, `fmath` in use everywhere a transcendental is called. Invariants A–G, I, M, N live; the T4 determinism tier live, including the cross-run bin and its committed hashes; the zero-allocation frame test; the first bench and `scripts/check-perf.sh` + `bench-baseline.json` land (§15.1); the `coverage` and `bench` CI jobs land.
verify: `cargo test -p catena` reports ≥ 200 passing tests; the stability-contract suite and the cross-run hash test are green; `./scripts/check-perf.sh` exits 0 and has written this machine's baseline entry; `cargo xtask lint` reports zero `lint-allow` lines for the `fmath` rule.

**M3 — Interaction + widget + demo.**
`Controller` (hit-testing, spatial nav, node drag, animations via `tick`, the key table and `Action`s), `InputEvent`, the widget crate: `from_crossterm` behind `crossterm`, the `Graph` widget + `Buffer` adapter, `catena-demo` behind `demo` (loads `fixtures/community.json` and the 120-node generated fixture of §20; all bindings live; reads `NO_COLOR`), minimap + legend behind `widget-extras`. T5 live; fuzz targets 1–2 and the `fuzz` CI job live; invariant J live.
verify: `cargo test --workspace` exits 0; `cargo build -p catena-ratatui --bin catena-demo --features demo` exits 0; `cargo +nightly fuzz run fuzz_events -- -runs=10000` exits 0 (if no nightly toolchain is installed where the session runs, CI's `fuzz` job is the observed exit 0 and the session says so). Running the demo in a real terminal is the owner's look, not a gate.

**M4 — Layered engine.**
The full Sugiyama pipeline of §9.1, channel routing, junction glyphs, arrowheads, `LayeredSpec` rank pinning, the tree engine's spanning-forest guard and `TreeSpec`. Invariant H live; the DAG golden set (incl. the skip-edge and channel-stacking fixtures); fuzz target 3.
verify: `cargo test -p catena layered:: tree::` exits 0; goldens exist for the 12-fixture set; the edge-conservation property passes at 512 cases.

**M5 — Polish + package.**
Parallel-edge fanning + self-loop rendering, the radial/chord view wired to the widget, the T7 PTY suite and its CI job, benches within the §15.1 budgets, a rustdoc pass with a doctested README (the §13 snippet is the README's example and compiles as a doctest), CHANGELOG, `cargo publish --dry-run` for all three crates, `seed/` gone.
verify: `cargo test --workspace --all-features` exits 0; the PTY job is green on linux and macos CI; `cargo bench` meets §15.1 on the dev machine; `cargo publish --dry-run -p catena -p catena-ratatui -p catena-testkit` exits 0; `test ! -e seed`; `test -z "$(git ls-files '*.seed.rs')"`. Tagging 0.1.0 and publishing are the owner's acts.

Post-1.0 backlog (explicitly deferred, in priority order): Brandes-Köpf + erratum; **edge labels** (a `label: Option<String>` on `EdgeSpec`, placed at the route midpoint through the label mask; absent from v1 so that `EdgeSpec` can grow it without a breaking change, and because no seed code has it); off-screen node indicators (edge-of-viewport chevrons); **group hulls** (convex-hull tints on the `Annotations` layer, the reason `StyleResolver::group` exists beyond radial arcs); DOT/mermaid importer crate; `ratatui-image` high-fidelity tier; ELK backend feature; Buchheim tree layout; stress-majorization layout; a termwiz/termion `from_*` conversion alongside `from_crossterm`.

---

## 20. Fixtures

- **`fixtures/community.json`** (from `seed/fixtures/`): the most realistic graph available — 31 nodes in 3 planted groups of 10 plus one ungrouped bridger node, 146 directed edges (135 within a group: each group is a complete one-directional K₁₀; 11 between, 6 of them touching the bridger), no self-loops, no parallel or reciprocal pairs, all-ASCII labels of at most 26 columns. Top-level keys `description`, `nodes`, `edges`; each node carries `key`, `label`, `group` (`null` for the bridger), and `confidence` (0.80–0.96) / `pagerank` (0.041–0.069; the values sum to ≈ 2.0, so they are style inputs, not a distribution — the gradient helper renormalizes to the visible range anyway); each edge carries `source`, `target`, `kind` (17 distinct strings; the loader maps them to legend categories, not to `EdgeClass`) and `weight` (0.3–0.95). It exercises force layout cluster separation, the radial/chord view (groups → arcs), the gradient helpers and T2/T3 goldens. Because it has no self-loops, parallels, non-ASCII labels or skip edges, it proves nothing about those: the canonical graphs below do.
- **The 12 canonical graphs**, hand-built in `catena-testkit::fixtures`: triangle, star-8, two components + ring, self-loop, parallel ×3, deep DAG with skip edges, wide DAG forcing channel stacking, tree, chord-8-groups, CJK labels, single node, 300-node stress.
- **Generated families**, seeded and deterministic (`fixtures::generated(seed, n)`): planted-group graphs with mixed label lengths (ASCII, CJK, emoji, combining marks), occasional self-loops and parallel edges. The 120-node instance at a pinned seed is the demo bin's second dataset; the families feed invariants F and the §15.1 benches. Two runs of a generator at the same seed must hash equal.

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
| Node identity | Generic `Key` (Clone + Eq + Hash + Ord), interned to dense `u32` indices; `NodeIx`/`EdgeIx` public but opaque so scenes and outcomes carry indices, never key clones |
| Edge identity | `EdgeId` from `Tx::add_edge`, a monotone never-reused counter; parallel edges ordered by insertion rank within their pair (§4.1) |
| Mutation API | `update(|tx| …) -> Result`, all-or-nothing; `GraphError` is the core's whole error surface; render/layout/tick/hit-test are total (§4.2–4.3) |
| Layout timing | Synchronous at the end of `update()` by default; `LayoutPacing::Animated` advances in `tick` (§4.2) |
| Capacities | `u32` indices; labels truncated to `max_label_cols` (256) at the boundary (§4.1) |
| Graph storage | Own store; petgraph = optional interop feature, never required |
| Layouts in v1 | Force (FR + degree-scaled option), full Sugiyama, tree (naive-centered), radial + chord |
| Sugiyama x-coords | Priority method v1; Brandes-Köpf + 2020 erratum v1.1 |
| Sugiyama dependency | Own implementation; rust-sugiyama/elk-rs as test references only |
| Barnes-Hut | Harvested, gated `n > 500` |
| Snap policy | Isotropic contain + letterbox default; stretch opt-in; world space isotropic with `cell_aspect` (0.5) applied once in the snapper (§6) |
| Zoom vs layout | Pan/zoom never run an engine; a zoom-out within a level may re-snap colliding nodes (§6, §11.2) |
| Blitters | Braille default; half-block, sextant, ascii selectable; all four land in M1 |
| Surface text | `put(x, y, &str, style)`: one cell's symbol, width 1 or 2; width-0 chars attach to the preceding cell; no grapheme dependency (§7.1) |
| Route kinds | `Polyline` (sub-cell, any blitter) and `Orthogonal` (cell glyphs via `GlyphSet`) in one `SceneGraph` (§7.4) |
| Tree on non-trees | BFS spanning forest from `TreeSpec.roots`; non-tree edges drawn as overlay curves (§9.3) |
| Braille color limit | Exposed as a blitter trade-off; per-cell last writer within a layer, topmost layer across layers |
| Z-order | Single compositor with a fixed layer enum + OR-merged sub-cell bits |
| Scene/hit contract | Complete SceneGraph; hit-testing reads scene bounds; no row-only targets |
| Node measurement | One private fn → `ResolvedMetrics`; unicode-width only |
| Time | Injected exclusively via `tick(dt)`, `dt` clamped to 250 ms; the `Instant` type lint-banned |
| RNG | None in core (the harvested engines draw no random numbers); a seeded SplitMix64 in the testkit's generators only; `thread_rng`/`rand` banned |
| Float math | Transcendentals through `fmath` over the `libm` crate; `std` float methods lint-banned outside it; `total_cmp` for ordering (§11) |
| Async | None anywhere |
| Events | Own `InputEvent` in core; `catena_ratatui::input::from_crossterm` behind the `crossterm` feature (a free fn, by the orphan rule); key table in core, `Action`s for hosts with their own keys (§10.1) |
| Binaries | `catena-render-hash` (testkit) and `catena-demo` (widget, feature `demo`): bins, not examples, so tests can spawn them (§3) |
| Dev-dep cycle | `catena` dev-depends on `catena-testkit` by path only; `catena` publishes first (§3) |
| Harvest staging | A moved seed file is `<target>.seed.rs` until its port commit; lint and cargo ignore it (§18) |
| Spatial keyboard nav | 60° cone, distance × angular penalty, half-plane fallback (§10.3) |
| Edge hit precision | Cell space, 1.5-cell threshold; sub-cell precision deliberately not used |
| Animation curves | Pan 300 ms ease-out cubic; zoom exponential τ = 40 ms |
| Windowing | Out of scope; occlusion rects are a host-supplied input |
| Testing tiers | T1–T7 per §16; no embedded-engine tier, no main-loop mirroring |
| Style/color testing | Harvested SVG + SHA-256 pipeline (gating) + PNG gallery (non-gating) |
| Coverage / file-size / property-case norms | 80% line on core; 500/750 file lines; ≥ 256 proptest cases, ≥ 7 pinned regression seeds |
| Perf ratchet | Median of ≥ 7 runs + peak RSS vs a per-machine `bench-baseline.json`; past 1.5× needs the owner |
| Errors | `thiserror` in `catena` for `GraphError`; `anyhow` only in `xtask`; the bins use `Box<dyn Error>` (§4.3) |
| v1 non-goals | DOT/mermaid import, kitty/sixel, ELK, analytics, windowing, async (§2.2); edge labels and group hulls are backlog, not absent by accident (§19) |
| Crate names | `catena`, `catena-ratatui`, `catena-testkit` free on crates.io as of 2026-10-06; reserving them is the owner's act, recommended before M1 |
| Publishing | `cargo publish --dry-run` only; the real publish and the 0.1.0 tag are the owner's acts |
