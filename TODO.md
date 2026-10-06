# TODO

The worklist (`CLAUDE.md`, override 1). An item closes when its `verify:` command is observed
to exit 0. Milestones and their gates come from plan §19.

## Now

**M1 (raster + scene, plan §19), step 4 of 4: the scene.** Steps 1–3 are done (`Surface` and
`CellGrid`; T3 live; `SubCellCanvas` and the four blitters). Step 4 lands in this order, each
green on its own:

1. ~~Scene types~~ (done): `SceneGraph`, `SceneItem`, `Layer`, `Payload`, both `Route` kinds,
   `EdgeRoute`, shared segments, `CountBadge`, `edges_at` and `route_faults`.
2. ~~Clipping~~ (done): the canvas clips every walk exactly, which closed "Bound the work of a
   huge segment".
3. ~~The compositor~~ (done): `scene::Compositor` with per-layer OR-merge, `Orthogonal` arms,
   the label mask, glow, node text and badges; A3 closed.
4. `insta` and the first T2 goldens of primitive scenes (Ascii blitter), plus T3 scene
   snapshots; then the M1 milestone `verify:`.

The owner's crates.io name reservation is recommended before M1 lands (plan §0); it does not
block the work.

## Decisions

- **M1: `Surface::put` writes the first text cell of its symbol and nothing else.** A symbol
  with no text cell (empty, only zero-width or control characters) writes nothing, so every
  surface stays total on any `&str`. A control character is dropped and ends the cell before
  it (plan §7.1 names only width-0 characters); a mark after one is then leading and dropped.
  `text_cells` is public so the ratatui adapter applies the same split instead of its own.
- **M1: a style without a background keeps the cell's background,** as ratatui's style patch
  does, so a glyph drawn over a glow halo keeps the halo and the adapter needs no translation.
  A write replaces the foreground and the attributes.
- **M1: a width-2 glyph and its continuation are one unit.** The continuation carries the
  glyph's style and an empty symbol; patching either half's background patches both; writing
  over either half blanks the other, keeping its style; a width-2 glyph whose second cell is
  off the surface is not written. `CellGrid` is a terminal model, so a half glyph is
  unrepresentable rather than left to the renderer.
- **M1: `Attrs` uses ratatui's `Modifier` bit values** (bold 0x1, dim 0x2, italic 0x4,
  reversed 0x40), so T3's `{attrs:x}` hash field reads the same on both sides of the adapter.
- **M1: `PaletteColor::Ansi` reads only its low four bits,** so an `Ansi` color never emits more
  than a 16-color SGR code, which is what the `ansi16` theme is for (ledger row 29).
- **M1: a cell's symbol is 15 inline bytes.** Zero-width followers past that are dropped, which
  changes no width; it bounds untrusted labels and makes a grid one allocation.
- **M1: one cell ceiling, `raster::MAX_CELLS` = 2²² (2048 × 2048).** A larger `CellGrid` keeps its
  width and loses rows from the bottom (65 535 columns get 64 rows); step 3's `SubCellCanvas`
  takes the same bound, which closes "Bound canvas allocation".
- **M1: `CellGrid`'s text forms trim trailing blanks.** `to_string` (via `Display`, since clippy
  denies an inherent `to_string`) trims trailing spaces per row and joins rows with `\n`, the
  seed's `render_to_string` pattern; `to_ansi_string` trims trailing cells that show nothing (a
  space with no background and not reversed), emits one `ESC[0;…m` per style change (attributes,
  then foreground, then background) and resets at the end of a styled row. A cell's foreground
  is `None` until written, the terminal's default.
- **M1: a T3 snapshot with no `.hash` fails** (and writes `{name}.fail.svg`) instead of saving
  and passing as the seed's did, because gates fail closed and a deleted hash must not pass on
  CI; `CATENA_UPDATE_SNAPSHOTS=1` accepts. Snapshots live in `tests/visual/` of the package under
  test, found through the `CARGO_MANIFEST_DIR` that `cargo test` sets at run time (the seed's
  compile-time `env!` would put every caller's snapshots in the testkit). `VisualSnapshots::check`
  compares without writing, which is what the perturbed-color test uses on the committed hash.
- **M1: the T3 hash keeps the seed's line exactly; the picture resolves colors differently.** The
  hash swaps REVERSED colors and then resolves defaults by position, as the seed did (plan §18).
  The SVG resolves defaults first and then swaps, as a terminal does, so a reversed label on the
  default background draws in the background color rather than the default foreground.
- **M1: the SVG ends a text run at a skipped blank and after a wide glyph.** The seed appended a
  same-styled cell after a skipped blank to the run before it, so `a b` drew as `ab`, and wide
  glyphs pushed later cells to wherever the font's advance put them. Neither touches the hash.
- **M1: `svg.rs` keeps the renderer; its `snapshot` child module holds the snapshot half** (the
  merged `snapshots.rs`), to stay under 500 lines. `sha2` enters with default features off.
- **M1: the `png` gallery is deferred,** with its own item below. It is non-gating (plan §16.4),
  `resvg` is heavy for every `--all-features` gate leg, and no scene exists yet to look at.
- **M1: `SubCellCanvas` takes its `Blitter` at run time,** not as const generics, so a host can
  switch blitters and the frame scratch pools one type. Masks keep each blitter's bit layout:
  braille in Unicode's dot order (so `get_cell` and the testkit's independent decoder are
  unchanged), sextant and half block row-major. Per cell it keeps the `ColorSlot(u16)` of the last
  pen (`set_pen`), one per half for half blocks; `blit` resolves slots through a
  `Fn(ColorSlot) -> CellStyle`, which is where the scene's style ids will plug in. A half-block
  cell lit in two styles draws `▀` in the upper foreground over the lower as background, `█` when
  they match. `render()` still gives braille's blank as U+2800, which the seed characterization
  needs; other blitters' blank is a space. The seed's integer-coordinate primitives moved to
  `canvas/integer.rs` unchanged, to keep `canvas.rs` under 500 lines.
- **M1: the ascii blitter's pixels are line directions.** Plan §7.2 gives it 1×1 sub-resolution
  and "direction-quantized `─│╱╲` + junction glyphs"; a 1-bit pixel cannot carry a direction, so
  its mask holds direction bits (horizontal, vertical, rising, falling, dot) and lines crossing
  in one canvas OR into a junction. A pixel takes the direction of the polyline segment that lit
  it (circles: the tangent), quantized at 22.5° and 67.5° as drawn on screen, which needs the
  cell shape: the canvas takes `set_cell_aspect` (default 0.5, the `GridSnapper` default), so the
  renderer passes the configured one rather than a second constant. Horizontal with vertical, or
  any mix with a diagonal, is `cross`; the two diagonals are `diagonal_cross`. `LineGlyphs::BOX`
  and `LineGlyphs::ASCII` are the two sets until §12's `GlyphSet` carries them.
- **M1: `NodeIx` and `EdgeIx` arrive with the scene,** in a `graph` module that the M2 store
  fills in: opaque `u32`s, as plan §4.1 has them, made only inside the crate. So scenes are
  built by `catena`'s own unit tests until a layout emits them, and the scene's T2 goldens are
  unit-test `insta` snapshots. T3 scene snapshots wait for `GraphView` (M2): a unit test cannot
  hand its `CellGrid` to the testkit's copy of `catena` (the dev-dependency cycle above), and M1's
  T3 requirement (a committed set, a perturbed-color failure) is met by the testkit's own set.
- **A3: the per-edge route is `EdgeRoute { Path(Route), Chain(Vec<SegmentId>) }`,** which settles
  the open name: plan §7.4's `Route` stays the geometry kind. `Payload::EdgePath { ix, route:
  EdgeRoute }` keeps its plan name; a chained edge draws nothing itself, and each shared segment
  is its own item, `Payload::Segment { id, route, members }`, drawn once. `SceneGraph::push`
  assigns segment ids and sorts and dedups members, so a scene cannot hold a stale id or an
  unsorted set. `CountBadge { count, at }` is a field of `SceneItem`, so any item can carry one,
  placed by the producer inside the item's bounds.
- **A3: invariant B is `scene::route_faults`,** returning every fault (missing, duplicate,
  unexpected, disconnected, unknown segment, not a member, stray member) rather than a bool, so a
  failing property names the edge. A path must start in its source anchor's cell and end in its
  target's; a chain is walked from the source and each segment may run either way, since a shared
  trunk serves edges going both ways.
- **M1: `Payload` starts with `NodeBox`, `EdgePath`, `Segment` and `Decoration(Glow)`.** Plan
  §7.4's `Junction{..}` and `Label{..}` arrive with their producers (the layered engine; node
  measurement), which `#[non_exhaustive]` allows without a break. Hit priority is the plan's
  §10.2 (topmost layer, then smallest bounds) with insertion order standing in for the §4.1 sort
  tuple, since layouts push items in that order.
- **M1: a `SubPt` lies in the cell it rounds to, in every blitter.** The compositor maps a point
  to sub-pixels as `x·SUB_W + (SUB_W−1)/2`, `y·SUB_H + (SUB_H−1)/2`: plan §6's `+SUB_H/2`,
  generalized to both axes. For integer rows it gives the plan's pixel in all four blitters; for
  columns it picks braille's right dot column instead of the left. In return a route's cells are
  its vertices' cells rounded, so `Route::bounds` is exact and invariant B compares anchors
  exactly.
- **M1: segments clip in the walk's integer space, not as Cohen–Sutherland on floats.** Plan
  §16.2-K wants clipped output equal to unclipped-then-cropped, and clipping the float segment then
  re-snapping its end moves Bresenham's choices along the whole line. Every walk step moves one
  pixel along the major axis, so the state after `k` steps has a closed form (minor steps
  `⌊(2·minor·k + major) / (2·major)⌋`, checked against stepping, ties included, before it was
  written, and by a property since); a clipped segment starts at its first in-clip step, found from
  the major-axis bounds and two binary searches on the monotone minor axis, and stops at its
  last. Arithmetic is `i128`, so the full `i32` range is exact. The canvas clips to itself grown by
  `CLIP_SLACK` = 256 pixels, so a segment's cost is its stretch near the canvas.
- **M1: a dash after a clip restarts on the pattern's nominal phase.** The A1 dash algorithm
  rounds each run to whole pixels and so drifts along a path; there is no closed form for the
  drift, so a dashed walk that skipped pixels resumes as if every run had been exactly its
  length, from its exact arc length. Within the slack nothing is skipped and dashes match the
  unclipped walk exactly (a property checks this); a dashed line whose start lies more than 256
  pixels off the canvas can shift its phase once, when that start crosses the slack. Axis-aligned
  integer patterns do not drift, so they match exactly at any distance (a test covers 10 000
  pixels).
- **M1: the compositor merges every layer's polylines into one canvas and blits it once,**
  before glow and text. Plan §7.4 has a canvas per layer, OR-merged with the topmost layer's
  color; drawing that merged canvas once, under the text layers, keeps one blit per frame, and the
  label mask (node boxes and badge cells) keeps any polyline off text whatever its layer.
  `Orthogonal` routes draw after the canvas, as a per-cell arm mask (N, E, S, W) ORed across every
  route, so runs that meet form the corner, tee or cross they make; the last route through a cell
  styles it, a cell with one arm draws the full line, and a step on neither row nor column bends
  horizontal first. `BoxGlyphs::BOX` and `::ASCII` stand in for §12's `GlyphSet`. A node box draws
  its label from the `labels` callback on its first row, clipped to its bounds and the surface
  (a wide glyph cut by either is dropped), until node forms arrive with `ResolvedMetrics` (M2). A
  badge draws `×n` at its cell after its item, clipped to the item's bounds. A glow patches its
  bounds' backgrounds with the style's background (its foreground if it has none), after edges so
  edge glyphs keep theirs.
- **M1: the sextant table is checked against Unicode's character names,** a 60-entry table
  generated from the Unicode 15.1 database (`BLOCK SEXTANT-<cells>`), not against the encoder's
  own arithmetic; `▌`, `▐`, `█` and a space fill the four patterns Unicode encodes elsewhere.
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
  (with a `try_get_cell` returning `Option`), the four Bézier loops become one sampler (since
  A1, the tessellator). It also folds the three Bresenham loops into one walker, because fixing
  their `i32` overflows (found in the port: endpoint sums, centre-plus-radius, distance squares,
  dash periods) rewrote every line of them; a 33-raster characterization test pins the output to
  the seed's (since A1, where A1 leaves it; see the A1 decisions).
- **A1: curves live in `geometry::curve`, polylines are drawn by `raster`.** `Bezier` (quadratic
  or cubic, `f64` sub-pixels) and `tessellate(chain, &mut out)` are public in `geometry::curve`,
  since a curve is geometry the layouts emit (A2's chord cubics) and the raster only sees
  polylines. `raster::polyline_pixels` (the walk, public so the integration tests can check it)
  and the canvas's `draw_polyline`, `draw_dashed_polyline`, `draw_polyline_with_hops`,
  `draw_curve` and `draw_dashed_curve` sit beside the seed's integer primitives, which now
  delegate to them. The canvas keeps scratch vectors for tessellation and arc lengths, so
  drawing allocates only on first use.
- **A1: arc length is Euclidean along the `f64` polyline**, not a count of pixel steps, so a dash
  looks the same length at any angle. A pixel sits at the arc length of its centre's nearest
  point on the polyline (searched over the segments around the walk, never stepping backwards).
- **A1: each dash and gap rounds to its own length, not to absolute multiples of the period.** A
  run spans from the midpoint before its first pixel to the midpoint after its last, and ends at
  the boundary closest to its pattern length, so every run is within half a pixel step (≤ √2/2)
  of it. Boundaries locked to `k·period` would each be up to √2/2 off, so a run up to √2 off on a
  diagonal, past the ruled ±1; the cost is a phase that drifts along long curves, which nothing
  shows. On horizontal and vertical lines a run of n pixels spans exactly n, so those dashes
  match the seed's dot for dot; on other slopes they differ from the seed's step counts by design.
- **A1: the walk skips a pixel equal to either of the two last emitted.** That drops each joint's
  repeat and the one-pixel back-step rounding makes at a curve's extremum, so a polyline that does
  not cross itself emits each pixel once. Vertices round, then clamp to the `i32` range, so a
  polyline costs no more than the widest integer line. Hop gaps never suppress the polyline's
  first or last pixel; interior vertices get no protection, being tessellation artifacts.
- **A1: "no duplicates" is checked on the emitted walk**, since `lit_pixels` decodes a set and
  cannot show a repeat; the canvas test then checks it lights exactly the walk's pixels. The
  generated curves (bowed quadratics, forward-armed cubics, two-cubic G1 chains, chords from 6 to
  120 sub-pixels) all advance along their chord, so none crosses itself.
- **A1: the dash oracle measures along the tessellated polyline in walk order,** projecting each
  pixel onto its globally nearest point. Sorting pixels by arc position instead is ambiguous
  where several pixels outside a bend project onto the same vertex. Both A1 properties were seen
  failing against the seed's sampler (sample-only curves, sample-index dashes) and the dash one
  also against step-count dashing, before the walk made them pass.
- **A2: the waist is a fraction of the edge's chord, `a = waist · |tgt − src|`.** Dimensionless
  like β, so the bundle keeps its shape at any canvas size. The chord, not `|b_tgt − b_src|`,
  because the latter vanishes in the ruling's own degenerate case; `d` falls back to
  `unit(tgt − src)` when `|b_tgt − b_src| ≤ 1e-9 · max(chord, 1)`, and a zero chord gives zero
  arms. `generate_bundled_edges` takes `waist` after `beta`; `default_waist()` sits beside
  `default_beta()`.
- **A2: the default waist, 0.24, minimizes bending energy.** At the default β the mean of
  `chord · ∫ κ² ds` over the test wheels' inter-group edges bottoms out near 0.245 and is flat
  from 0.22 to 0.26 (β = 0.5 and 1.0 put it at 0.22 and 0.26); 0.12 costs 58% more bending and
  0.48 134% more, which is the pinch and the balloon the ruling describes. The other objective
  tried, staying closest to the seed's C⁰ chain, has its optimum at a zero arm, which is a stall
  point at J and no tangent at all, so it was dropped. At most 0.25 keeps the β = 0 chain from
  doubling back past its quarter points. Even so the ruling's cubics start on `b_src` itself, so
  they pull harder toward the group than the seed's quadratics did: at the default the chain sits
  a mean 3.0 sub-pixels from the seed's on a 100-sub-pixel-radius wheel.
- **A2: `fmath::atan2` joins `sin` and `cos`,** for the tangent-angle test (the radial engine will
  need it too). The chord tests split into `chord_tests.rs` (the seed's), `chord_g1_tests.rs`
  (A2's) and `chord_wheel_tests.rs` (their shared wheel), to stay under the 500-line target.
- **A1: the seed characterization keeps what A1 leaves.** 16 of the 33 rasters stay dot for dot
  (lines, hop lines, circles, axis-aligned dashes); the 7 solid curves keep every dot the seed
  plotted, now joined into a connected line; the 10 dashed rasters on curves and slopes are
  measured by the arc-length properties instead and must lie on their solid twin.
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
- [x] **Stage the SVG renderer** for its M1 port. verify: `test -f catena-testkit/src/svg.seed.rs
  && test -z "$(git ls-files seed/tests/visual)"`
- [x] **M0 gate** — verify: `./scripts/regression.sh && cargo test -p xtask && test -z "$(git
  ls-files seed/graph/braille.rs seed/graph/braille_tests.rs seed/graph/quadtree.rs
  seed/graph/chord.rs seed/graph/chord_tests.rs seed/graph/zoom.rs seed/ui/flex_layout.rs
  seed/graph/tree_layout.rs seed/graph/tree_layout_tests.rs seed/fixtures seed/tests/visual)" &&
  test -f CONTRIBUTING.md -a -f TODO.md && grep -q Signed-off-by .github/workflows/*.yml`

## M1 — Raster + scene

Plan §19, in four steps, each green on its own. The milestone closes on its own `verify:` below.

- [x] **Step 1: `Surface` and `CellGrid`** — `Surface`, `CellStyle`, `PaletteColor`, `Attrs`,
  `CellGrid` (`cell`, `to_string`, `to_ansi_string`) and `text_cells`, the §7.1 text-cell rule.
  verify: `cargo test -p catena -- --list | grep -c
  'a_wide_symbol_owns_a_continuation_cell\|text_cells_follow_the_rule_and_measure_as_display_width\|to_ansi_string_emits_sgr_only_where_the_style_changes'
  | grep -qx 3 && cargo test -p catena raster::`
- [x] **Step 2: T3 live** — `catena-testkit/src/svg.rs` from the two staged SVG files, on
  `CellGrid`; a committed `.hash` set; a test that perturbs one color and watches T3 fail.
  verify: `cargo test -p catena-testkit svg && test ! -e catena-testkit/src/svg.seed.rs && test
  -n "$(git ls-files '*.hash')"`
- [x] **Step 3: `SubCellCanvas` and the four blitters** — `SubCellCanvas(SUB_W, SUB_H)` replaces
  `BrailleCanvas`, with `u16` dimensions under `MAX_CELLS`; braille, half-block, sextant and
  ascii encoders. verify: `cargo test -p catena -- --list | grep -c
  'braille_glyphs_are_the_dot_pattern_offset_from_u2800\|half_blocks_carry_a_color_per_half\|sextant_glyphs_follow_their_unicode_names\|ascii_lines_draw_as_direction_glyphs_with_a_junction_where_they_cross'
  | grep -qx 4 && cargo test -p catena raster::`
- [ ] **Step 4: the scene** — `SceneGraph`, `SceneItem`, `Layer`, `Payload` (A3), both `Route`
  kinds, per-edge routes and shared segments (A3), `CountBadge` (A3), the compositor's OR-merge,
  the label mask, Cohen–Sutherland clipping; `insta` and the first T2 goldens.
  verify: `cargo test -p catena scene:: && cargo insta test --check`
- [ ] **M1 — Raster + scene** — `Surface`, `CellGrid`, `SubCellCanvas`, four blitters,
  compositor with OR-merge, label mask, clipping, `SceneGraph` with both `Route` kinds, the
  §7.1 text-cell rule, the SVG renderer ported onto `CellGrid`; T2 goldens, T3 live; A3's
  shared-geometry room in the scene.
  verify: `cargo test -p catena raster:: scene:: && cargo insta test && test ! -e
  catena-testkit/src/svg.seed.rs` (plus a committed `.hash` set and a test that perturbs one
  color and watches T3 fail)

## Course correction (owner rulings A1–A4)

Quoted in full in `docs/design/owner-rulings.md`, with a table of the plan text each overrides.
Each `verify:` lists the named tests first, because a test filter that matches nothing exits 0.

- [x] **A1 — the sampler is a tessellator** — one `tessellate` turns a quadratic, a cubic or a
  chain of either into a polyline at the seed's density, `steps = (chord/2).clamp(10, 200)`;
  solid, dashed and hop-gapped primitives work on polylines; dashes by arc length along the
  polyline; the seed's dash-phase note is not carried. Tests: invariant E on tessellated
  polylines; dash and gap lengths within ±1 sub-pixel, including on curves under 20 px.
  verify: `cargo test -p catena -- --list | grep -c
  'invariant_e_holds_on_tessellated_curves\|dash_and_gap_lengths_follow_the_pattern' | grep -qx
  2 && cargo test -p catena && ! grep -rq "counted by sample index" catena/src`
- [x] **A2 — chord edges are G1** (own commit, after the chord port `b986741`) — an inter-group
  edge is the cubics `(src, b_src, J − a·d, J)` and `(J, J + a·d, b_tgt, tgt)`, `J = b_root`,
  `d = unit(b_tgt − b_src)` or `unit(tgt − src)` when degenerate; the waist arm `a` exposed next
  to β with a tuned default. Tests: tangent angle at J < 1e-9 rad; collinear at β = 0 with the
  limit tests green; the C⁰ test replaced, not deleted. verify: `cargo test -p catena -- --list |
  grep -c 'inter_group_chain_is_g1_at_the_junction\|beta_zero_chain_is_collinear' | grep -qx 2
  && cargo test -p catena -- chord`
- [x] **A3 — room in the scene for shared geometry (M1)** — `Payload` is `#[non_exhaustive]`;
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

- [x] **Bound the work of a huge segment** — since the M0 port, a line spanning the whole `i32`
  range no longer overflows, but the walk then steps all ~4·10⁹ pixels; since A1 the same holds
  for a curve, which used to cost at most 201 samples. Clipping segments to the canvas before
  rasterizing (plan §6, ledger row 14) fixes both in M1. verify: `cargo test -p catena --lib
  a_segment_across` runs both tests (a line and a bowed curve from `(i32::MIN, 0)` to
  `(i32::MAX, 3)` on an 8×2 canvas; the walk's pixel count bounded by the canvas plus slack) and
  reports finishing in < 1 s
- [x] **Bound canvas allocation** — `BrailleCanvas::new` takes `usize` dimensions and allocates
  their product unchecked. M1's `SubCellCanvas` should take terminal-sized `u16` dimensions.
  verify: `cargo test -p catena raster::blit::tests::a_canvas_request_past_the_cell_ceiling_is_bounded`
  (bounded to `MAX_CELLS`: 65 535 columns get 64 rows)

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

- [ ] **The `png` gallery (plan §16.4)** — `catena-testkit`'s `png` feature: `resvg`
  rasterizes `grid_to_svg` into a non-gating `tests/gallery/*.png`, regenerated on demand. Lands
  once scenes render (M3 at the latest, with the demo); check `resvg`'s `rust-version` against
  the MSRV first. verify: `cargo test -p catena-testkit --features png png`

## Later milestones

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
