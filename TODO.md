# TODO

The worklist (`CLAUDE.md`, override 1). An item closes when its `verify:` command is observed
to exit 0. Milestones and their gates come from plan §19.

## Now

**Absorb the owner's course correction** (`docs/design/owner-rulings.md`, A1–A4 below). A1 and
A2 change code that has landed, so each is a follow-up commit now: A1 first (the tessellator and
polyline primitives in `catena/src/geometry/curve.rs` and `catena/src/raster/polyline.rs`, with
the braille canvas's curve and dash primitives rebuilt on them), then A2 in its own commit (G1
chord cubics, which draw through A1's tessellator). A3 and A4 land with the code they shape (M1
scene, M3 controller, the radial view). Also confirm CI green on rustc 1.99 for `f61941b`.

Then M0's last harvest: `git mv seed/tests/visual/svg_renderer.rs
catena-testkit/src/svg.seed.rs` and `seed/tests/visual/snapshots.rs
catena-testkit/src/svg_snapshots.seed.rs` (one commit, no content change; ported at M1). Then
run the M0 gate's `verify:` and close M0.

## Decisions

- **Gates run on the newest stable rustc; CI keeps `toolchain: stable`.** Owner: "just update to
  rust 1.99 so you match latest stable so you don't have conflicts with gh ci." So CI is not
  pinned (plan §17's `check` is "× stable"), and a session switches its local default to the
  newest stable before gating (`rustup default 1.99` here, where the image's `stable` is 1.97).
  A clippy-lint scope that a release can stop firing takes `allow` with its reason, not
  `expect`, because an unfulfilled expectation fails the gate when clippy's lint set moves.
- **Only `catena` has a `[[bench]]` target; the other crates set `autobenches = false` and
  declare none.** Plan §3 shows `benches/` only under `catena` and §3.1 says "at most one bench
  target"; an empty bench target would cost a link in every `--all-targets` gate. Until criterion
  lands at M2 the bench is a `fn main() {}` placeholder.
- **Dependencies enter when first used.** `sha2` arrives with the SVG port (M1), `proptest` with
  the first property test; `catena-ratatui` takes `ratatui-core` at M0 so the lockfile pins the
  version whose `rust-version` (1.88) sets the MSRV (plan §3).
- **`xtask` pins its dependencies `=x.y.z`; published crates use semver ranges.** `xtask` is
  unpublished, so the baseline pin rule applies to it; override 3 covers the published crates.
- **The braille assert quartet lives in `catena-testkit` and runs from `catena`'s integration
  target.** The dev-dependency cycle compiles `catena` twice, so a `catena` unit test cannot
  pass its own `BrailleCanvas` to the testkit's copy; integration tests share one copy. The
  testkit decodes dots from the Unicode braille dot numbering, not `catena`'s `BRAILLE_MAP`, so
  the oracle is independent of the code under test.
- **The M0 braille port keeps `BrailleCanvas`; the `SubCellCanvas` generalization is M1.** Plan
  §19 lists the generalization under M1. M0 applies the other §18 changes: `get_cell` clips
  (with a `try_get_cell` returning `Option`), the four Bézier loops become one sampler. It also
  folds the three Bresenham loops into one walker, because fixing their `i32` overflows (found in
  the port: endpoint sums, centre-plus-radius, distance squares, dash periods) rewrote every line
  of them; a 33-raster characterization test pins the output to the seed's, dot for dot.
- **`word_wrap` leaves by copy, not `git mv`.** The rest of `seed/ui/box_layout.rs` and its tests
  stay as the box-drawing reference for M1 (plan §18 "reference for box drawing only"), and
  §19's M0 `verify:` does not list them. The copy is staged as `text.seed.rs` and ported in the
  next commit, so the port diff is still the record; the seed file is deleted with the M1 port
  it informs.
- **`word_wrap` semantics beyond plan §18.** Paragraphs split as `str::lines` does (a trailing
  `\n` adds no line; `\r\n` counts as one break); an empty paragraph yields one empty line; a
  character wider than `width` (double-width at width 1) is dropped, because "every output line
  measures ≤ width" is the stated invariant.
- **The SVG pair stages under two names.** `seed/tests/visual/svg_renderer.rs` becomes
  `catena-testkit/src/svg.seed.rs` and `seed/tests/visual/snapshots.rs` becomes
  `catena-testkit/src/svg_snapshots.seed.rs`; the M1 port merges both into `svg.rs`.
- **Quadtree: the jittered point is used for routing only.** Bodies keep their true positions for
  centre of mass and force, and a `MAX_DEPTH` leaf keeps its bodies and sums them exactly. This
  fixes both §18 bugs and a third found while porting: the seed's jittered copy sits √2·1e-4 from
  the true point, outside the 1e-4 self-skip radius, so a coincident pair repelled itself with
  force k².
- **A port that rewrites most of a file needs `-M20%` to show as a rename.** The braille port
  fell under git's default 50% similarity; `git show -M20% <port commit>` pairs it with its
  staged seed file, so the delta from the seed stays reviewable.
- **Property tests run proptest at a fixed seed with failure persistence off.** The gates are
  deterministic (plan §17), so every run draws the same 256 cases; a failure reproduces as-is,
  and a case worth keeping becomes a named regression test rather than a file under `src/`.
- **`chord` renames `entity` to `node` as well as `community` to `group`.** Plan §2.2 rules out
  domain vocabulary; the rest of the plan calls them nodes.
- **`cargo xtask lint` details the plan leaves open.** Matching runs on source with comments and
  string literals blanked, so prose that names a banned item passes. "Outside tests" (the
  `unwrap` rule) means outside `#[cfg(test)]` and `#[test]` items and outside `tests.rs` /
  `*_tests.rs` files. The I/O fence also catches `print!`, `eprint!` and `dbg!`; the `fmath`
  rule covers every `std` transcendental (`asin`, `sinh`, `exp2`, `ln_1p`, `sin_cos`, …) and the
  `f64::sin(x)` call form; float ordering also bans `partial_cmp(..).expect`; file length also
  covers `tests/`, `benches/` and `xtask/src`. The post-M5 harvest rule is a constant
  (`HARVEST_COMPLETE`) that M5 flips.
- **The `dco` job fails fork PRs that lack sign-off and only warns on same-repository PRs.**
  Plan §3 scopes the check to external PRs, and this repo's in-repo branches are the owner's own
  (including Claude sessions, whose commits carry no human sign-off). Unratified: the owner may
  prefer failing every PR.
- **CI runs on every push as well as on PRs,** so a session branch gets the three-OS `check`
  without opening a PR.
- **`.gitattributes` forces LF line endings,** so shell scripts run and goldens hash the same on
  the Windows leg of `check`.

## M0 — Bootstrap + harvest drop

- [x] **Create the worklist** — this file, from plan §19. verify: `test -f TODO.md`
- [x] **Workspace scaffold (plan §3.1)** — root and crate manifests, profiles, lints, alias-only
  `.cargo/config.toml`, one test target per crate, `thiserror`/`libm`/empty `fmath.rs` in
  `catena`. verify: `cargo build --workspace --all-targets && cargo test --workspace`
- [x] **CONTRIBUTING.md** — DCO sign-off plus the contributor license grant to `jbrjake` (plan
  §3). verify: `grep -q Signed-off-by CONTRIBUTING.md && grep -qi "license grant" CONTRIBUTING.md`
- [x] **`cargo xtask lint`** — every §17 rule, each with a planted-violation test.
  verify: `cargo test -p xtask && cargo xtask lint`
- [x] **Gates and hooks** — `scripts/smoke.sh`, `scripts/regression.sh`, `.githooks/`; the
  `core.hooksPath` setup noted in `CONTRIBUTING.md`. verify: `./scripts/regression.sh`
- [x] **CI skeleton** — `check` (linux/macos/windows via `regression.sh`), `msrv`, `dco`; every
  `uses:` pinned by SHA. verify: `grep -q Signed-off-by .github/workflows/*.yml`, plus a green
  run of all three `check` legs and `msrv` on this branch (run 37495030115: all four
  `conclusion=success`; `dco` skipped on push, as designed)
- [x] **Harvest braille canvas + tests** → `catena/src/raster/`, the assert quartet →
  `catena-testkit/src/braille_asserts.rs`. verify: `cargo test -p catena raster && test -z
  "$(git ls-files seed/graph/braille.rs seed/graph/braille_tests.rs)"`
- [x] **Harvest quadtree** → `catena/src/layout/force/`, with the `MAX_DEPTH` and jitter fixes and
  the force-ratio test tightened to (0.7, 1.4). verify: `cargo test -p catena quadtree && test -z
  "$(git ls-files seed/graph/quadtree.rs)"`
- [x] **Harvest chord + tests** → `catena/src/layout/radial/`, renamed, index-mapped, through
  `fmath`. verify: `cargo test -p catena chord && test -z "$(git ls-files seed/graph/chord.rs
  seed/graph/chord_tests.rs)"`
- [x] **Harvest the zoom table** → `catena/src/geometry/`, configurable, with the clamp constants.
  verify: `cargo test -p catena zoom && test -z "$(git ls-files seed/graph/zoom.rs)"`
- [x] **Harvest flex** → `catena/src/layout/layered/flex.rs`, overflow comment corrected and
  pinned. verify: `cargo test -p catena flex && test -z "$(git ls-files seed/ui/flex_layout.rs)"`
- [x] **Harvest `word_wrap`** → `catena/src/raster/text.rs`, in display columns with the §18
  semantics. verify: `cargo test -p catena text`
- [x] **Harvest tree_layout + tests** → `catena/src/layout/tree/`, header fixed, unused parameters
  removed. verify: `cargo test -p catena tree && test -z "$(git ls-files seed/graph/tree_layout.rs
  seed/graph/tree_layout_tests.rs)"`
- [x] **Testkit fixtures** — `fixtures/community.json` moved verbatim, plus its loader.
  verify: `cargo test -p catena-testkit && test -z "$(git ls-files seed/fixtures)"`
- [ ] **Stage the SVG renderer** for its M1 port. verify: `test -f catena-testkit/src/svg.seed.rs
  && test -z "$(git ls-files seed/tests/visual)"`
- [ ] **M0 gate** — verify: `./scripts/regression.sh && cargo test -p xtask && test -z "$(git
  ls-files seed/graph/braille.rs seed/graph/braille_tests.rs seed/graph/quadtree.rs
  seed/graph/chord.rs seed/graph/chord_tests.rs seed/graph/zoom.rs seed/ui/flex_layout.rs
  seed/graph/tree_layout.rs seed/graph/tree_layout_tests.rs seed/fixtures seed/tests/visual)" &&
  test -f CONTRIBUTING.md -a -f TODO.md && grep -q Signed-off-by .github/workflows/*.yml`

## Course correction (owner rulings A1–A4)

Quoted in full in `docs/design/owner-rulings.md`, with a table of the plan text each overrides.
Each `verify:` lists the named tests first, because a test filter that matches nothing exits 0.

- [ ] **A1 — the sampler is a tessellator** — one `tessellate` turns a quadratic, a cubic or a
  chain of either into a polyline at the seed's density, `steps = (chord/2).clamp(10, 200)`;
  solid, dashed and hop-gapped primitives work on polylines; dashes by arc length along the
  polyline; the seed's dash-phase note is not carried. Tests: invariant E on tessellated
  polylines; dash and gap lengths within ±1 sub-pixel, including on curves under 20 px.
  verify: `cargo test -p catena -- --list | grep -c
  'invariant_e_holds_on_tessellated_curves\|dash_and_gap_lengths_follow_the_pattern' | grep -qx
  2 && cargo test -p catena && ! grep -rq "counted by sample index" catena/src`
- [ ] **A2 — chord edges are G1** (own commit, after the chord port `b986741`) — an inter-group
  edge is the cubics `(src, b_src, J − a·d, J)` and `(J, J + a·d, b_tgt, tgt)`, `J = b_root`,
  `d = unit(b_tgt − b_src)` or `unit(tgt − src)` when degenerate; the waist arm `a` exposed next
  to β with a tuned default. Tests: tangent angle at J < 1e-9 rad; collinear at β = 0 with the
  limit tests green; the C⁰ test replaced, not deleted. verify: `cargo test -p catena -- --list |
  grep -c 'inter_group_chain_is_g1_at_the_junction\|beta_zero_chain_is_collinear' | grep -qx 2
  && cargo test -p catena -- chord`
- [ ] **A3 — room in the scene for shared geometry (M1)** — `Payload` is `#[non_exhaustive]`;
  each edge has a route, either one `EdgePath` or an ordered chain of segment ids; a segment
  carries its member `EdgeIx` set and hit-testing it returns that set; invariant B in route
  terms (every input edge maps to exactly one route, and that route's segments connect its
  source anchor to its target anchor); the parallel-edge `×n` badge becomes a `CountBadge`
  decoration any scene item can carry. Open until M1: the name of the per-edge route type, since
  plan §7.4's `Route` already names the geometry kind (`Polyline | Orthogonal`). verify:
  `cargo test -p catena -- --list | grep -c 'shared_segment_hit_returns_its_members\|invariant_b_every_edge_has_one_connected_route\|count_badge_rides_any_item'
  | grep -qx 3 && cargo test -p catena -- scene:: && grep -rq 'non_exhaustive' catena/src/scene`
- [ ] **A4 — bundling is never the only path (radial view)** — an unbundle toggle (one key sets
  β = 0) and hover-lift (hovering a node or an edge redraws its edges on `EdgesOver`,
  highlighted), both in the default key table (M3) and in the T2/T3 goldens once the radial
  view renders. verify: `cargo test -p catena -- --list | grep -c
  'unbundle_key_sets_beta_zero\|hover_lift_redraws_incident_edges_over' | grep -qx 2 && cargo
  test --workspace && cargo insta test --check && test -n "$(git ls-files '*unbundled*')" &&
  test -n "$(git ls-files '*hover_lift*')"`

## Owner

- [ ] **Reserve the crate names on crates.io** — a real `0.0.0` publish of `catena`,
  `catena-ratatui` and `catena-testkit`; the owner's act, recommended before M1 (plan §0).
  verify: `cargo info catena && cargo info catena-ratatui && cargo info catena-testkit`

## Found while porting

- [ ] **Bound the work of a huge segment** — since the M0 port, a line spanning the whole `i32`
  range no longer overflows, but `walk_line` then steps all ~4·10⁹ pixels. Clipping segments to
  the canvas before rasterizing (plan §6, ledger row 14) fixes it in M1. verify: a test drawing
  `(i32::MIN, 0)` to `(i32::MAX, 3)` on an 8×2 canvas finishes under `cargo test` in < 1 s
- [ ] **Bound canvas allocation** — `BrailleCanvas::new` takes `usize` dimensions and allocates
  their product unchecked. M1's `SubCellCanvas` should take terminal-sized `u16` dimensions.
  verify: `cargo test -p catena raster::` with a test that a `u16::MAX`-square canvas request is
  either refused or bounded

- [ ] **Make the tree passes total on any input (M4)** — `compute_subtree_width` and
  `assign_x` recurse once per level, so a chain of ~10⁵ nodes overflows the stack; the BFS has
  no visited set, so shared children are revisited (exponentially in a stack of diamonds) and a
  cycle never terminates. The M4 spanning-forest guard (plan §9.3, ledger row 34) should come
  with iterative passes. verify: `cargo test -p catena tree::` with a 200 000-node chain, a
  32-level diamond stack and a 3-cycle, each finishing in < 1 s
- [ ] **Widen tree coordinates past `i16` (M4)** — the seed's `TreePosition` is `i16`, so the
  port saturates: a tree taller or wider than 32 767 cells clamps and overlaps. Scene cells are
  wider; switch when the tree feeds the scene. verify: `cargo test -p catena tree::` with a
  40 000-row node keeping its child strictly below it
- [ ] **Drop the harvest's dead-code attributes once callers land** — the M0 ports are wired
  to nothing yet, so their modules carry `cfg_attr(not(test), expect(dead_code))`, and `fmath`
  an `allow` (rustc 1.88 does not report it, 1.97 does). Each goes when its engine lands (M2,
  M4, M5); the `expect`s announce themselves, the `fmath` `allow` will not.
  verify: `! grep -rn "dead_code" catena/src`

## Later milestones

- [ ] **M1 — Raster + scene** — `Surface`, `CellGrid`, `SubCellCanvas`, four blitters,
  compositor with OR-merge, label mask, clipping, `SceneGraph` with both `Route` kinds, the
  §7.1 text-cell rule, the SVG renderer ported onto `CellGrid`; T2 goldens, T3 live; A3's
  shared-geometry room in the scene.
  verify: `cargo test -p catena raster:: scene:: && cargo insta test && test ! -e
  catena-testkit/src/svg.seed.rs` (plus a committed `.hash` set and a test that perturbs one
  color and watches T3 fail)
- [ ] **M2 — Graph model + force layout + viewport** — per plan §19.
  verify: `cargo test -p catena` reports ≥ 200 passing tests, the stability suite and the
  cross-run hash test are green, `./scripts/check-perf.sh` exits 0 and wrote this machine's
  baseline entry, `cargo xtask lint` reports zero `lint-allow` lines for the `fmath` rule
- [ ] **M3 — Interaction + widget + demo** — per plan §19, with A4's unbundle key and hover-lift
  in the default key table. verify: `cargo test --workspace &&
  cargo build -p catena-ratatui --bin catena-demo --features demo && cargo +nightly fuzz run
  fuzz_events -- -runs=10000`
- [ ] **M4 — Layered engine** — per plan §19. verify: `cargo test -p catena layered:: tree::`
  exits 0, goldens exist for the 12-fixture set, edge conservation passes at 512 cases
- [ ] **M5 — Polish + package** — per plan §19, including `seed/` removed and `HARVEST_COMPLETE`
  flipped in `xtask`, and A4's goldens once the radial view renders. verify: `cargo test --workspace --all-features && cargo publish --dry-run
  -p catena -p catena-ratatui -p catena-testkit && test ! -e seed && test -z "$(git ls-files
  '*.seed.rs')"` (plus the PTY job green on linux and macos, benches within §15.1)
