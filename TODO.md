# TODO

The worklist (`CLAUDE.md`, override 1). An item closes when its `verify:` command is observed
to exit 0. Milestones and their gates come from plan §19.

## Now

**M3 (interaction + widget + demo, plan §19) is under way: steps 1 to 6 have landed, and
step 7, the fuzz crate, is next.** A view lays out, snaps and draws; T3 and T4 hold its frames
to committed hashes on every platform; a steady frame allocates nothing; the benches sit
inside every §15.1 budget. It takes input: `GraphView::{handle, act, tick}` select, hover
with A4's hover-lift, pin, drag, cycle, navigate, pan and zoom, with time entering only
through `tick`. `catena-ratatui` draws it as a `Graph` widget and converts crossterm events,
so plan §13's host loop runs as written, and `catena-demo` (`cargo run -p catena-ratatui
--features demo --bin catena-demo`) shows it. The joined-islands pull the owner ruled for has
landed switched off, waiting on the owner's call about its cost to invariant F (`## Owner`).

M3 in this order, each step green on its own:

1. ~~The input model~~ (plan §10.1): landed (`## M3`, step 1).
2. ~~Hit-testing~~ (plan §10.2): landed (`## M3`, step 2).
3. ~~The `Controller`~~ (plan §10.1, §10.3, §10.4): landed (`## M3`, steps 3a to 3c).
4. ~~The widget crate~~: landed (`## M3`, steps 4a and 4b).
5. ~~`catena-demo` behind `demo`~~: landed (`## M3`, step 5).
6. ~~Minimap and legend behind `widget-extras`~~: landed (`## M3`, step 6).
7. The `fuzz/` crate with `fuzz_events` and `fuzz_topology`, and the nightly `fuzz` CI job.

Carried from M2: invariant F is measured on world positions at the later layout's
`Fit::Contain` scale; with drawing live, read it again on drawn cells, since a refit after a
node lands outside the old bounding box moves every cell (anchor compensation holds only the
selected node). The owner ruled on M2's open choices (`owner-rulings.md`,
"Relayout follow-ups"): F stays pooled with no per-run floor, joined islands must pull
together, and the rest stands except fitting islands to the frame, still the owner's question.

M2 as it landed, each step green on its own:

1. ~~The graph store~~ (plan §4): landed.
2. ~~`ResolvedMetrics`~~ (plan §5): landed. `GraphView::pending` now holds every commit's
   `Delta` with `reshaped` cut to the nodes whose measured box changed; `repinned` lists pin
   edits, which re-snap with an unchanged box. Steps 3 and 4 consume it; nothing clears it yet.
   No `Payload::Label`: a node's text is part of its `NodeBox` form (see the decision), so
   §7.4's `Label{..}` waits for free-standing text, if any.
3. ~~The force layout~~ (plan §8), with ~~the relayout rework~~: landed as
   `layout::force::engine::lay_out(params, store, metrics, Frame { area, cell_aspect }, change,
   &mut ForceState)`, world positions (node centers) by slot. Nothing calls it outside tests
   yet, so its modules carry `expect(dead_code)`.
4. ~~`GridSnapper` and the viewport~~ (plan §6): landed as `geometry::snap` (`fit`,
   `resolve`, `derive`, `placement_order`) and `geometry::viewport` (the canonical/derived
   split, zoom-out re-snap, anchor compensation), and `GraphView` lays out and snaps at the end
   of every `update`. `simulation::Run` takes one step at a time, which is what
   `LayoutPacing::Animated` needs once `tick` exists.
5. ~~Layout → `SceneGraph` → `render_to_string`; T3 scene snapshots; the T4 tier; invariants
   B–E and N on real scenes~~: landed (`## M2`, steps 5a to 5d) as `draw::build`,
   `GraphView::{scene, render, render_to_string}`, `catena/tests/visual/`, the
   `catena-render-hash` bin with `catena-testkit/tests/hashes/`, and the invariant tests.
6. ~~The zero-allocation frame test, the first criterion benches, `scripts/check-perf.sh`
   with `bench-baseline.json`, and the `coverage` and `bench` CI jobs~~: landed (`## M2`, steps
   6a to 6d).

The owner's crates.io name reservation is still recommended (plan §0); it blocks nothing.

## Decisions

- **M3: the minimap reads the view; it is not a second render of it.** Plan §18 has "a second
  `GraphView` render at a fixed low zoom", but rendering the view at the minimap's size would
  resize and relayout it, and a second `GraphView` would lay out a graph of its own. So
  `render_overview(&self)` draws the view's own world positions, one dot a node as the seed's
  minimap did, with no edges and no collision re-spacing, so a crowded minimap stays a picture
  of the layout. It fits the nodes together with the view's frame, unlike the seed, whose
  rectangle left the minimap whenever the view showed more than the nodes' bounding box.
- **M3: the demo runs on `ratatui-crossterm`, not all of `ratatui`.** Plan §3's `demo` is
  "`crossterm` + optional full `ratatui`"; the demo needs only a crossterm `Backend` for
  `ratatui-core`'s `Terminal`, which `ratatui-crossterm` 0.1 is, over the same crossterm 0.29.
  It takes `catena-testkit` as an optional dependency for its fixtures, with a version, so the
  testkit publishes before `catena-ratatui`. The bin reads the clock, as a host does: its two
  lines carry `lint-allow: time`, the only allowances in the tree.
- **M3: the push gate's test step enables `catena-ratatui/demo`.** The demo's own tests need
  the feature, and `--all-features` would add plan §16.8's PTY tier, which the gate excludes;
  plan §17's command gains that one flag.
- **M3: T5 runs on `ratatui-core`'s `TestBackend`, not full `ratatui`.** Plan §3 lists full
  `ratatui` as a dev-dependency "for `TestBackend` in widget tests"; `ratatui-core` 0.1 carries
  `TestBackend` and `Terminal` itself, so the widget crate's tests need nothing past its own
  dependency. Full `ratatui` arrives with the demo (`demo` feature), if it needs its widgets.
- **M3: `catena_ratatui::input` builds for the crate's tests without its feature.** The push
  gate tests the workspace without features (plan §17: `--all-features` would pull in the PTY
  tier), so a feature-gated `from_crossterm` would go untested there. `crossterm` is also a
  dev-dependency and the module is `cfg(any(feature = "crossterm", test))`; clippy and rustdoc
  see it with `--all-features`.
- **M3: pointer events arrive in screen cells, and the surface says where it is.** Plan §13
  passes `from_crossterm`'s event straight to `gv.handle`, in terminal cells, while the widget
  draws in an area of the terminal. The widget's state is the `GraphView` itself, so the view
  must know the offset: `Surface::origin`, a provided method returning `(0, 0)`, which the
  `Buffer` adapter answers with its area's corner and `render` records. A host drawing a
  `CellGrid` passes surface cells as before. Unratified.
- **M3: anchor compensation never keeps a node the commit repinned.** Plan §6 keeps the focused
  node where it was drawn through a re-snap or refit. When the commit moved that node's pin,
  the move is the edit: keeping it would pan every other node by the move instead, which is
  what dragging the selected node showed. Such a re-snap keeps nothing; a relayout it cascades
  into keeps the node where the re-snap drew it. Seen failing first in
  `view::tests::repinning_the_focused_node_moves_it_and_not_the_view`.
- **M3: the controller lives in `GraphView`, not beside it.** Plan §10.1 has
  `Controller::handle(ev, scene, metrics)`; here `interact::controller::Controller` is the
  state and `GraphView::{handle, act, tick}` drive it, since every action needs the store,
  viewport and kept scene together and the plan already has `GraphView` forward to it. A
  `Redraw` reads the drawing's own `shown` flag, which a frame sets and any change clears, so a
  hit-test that rebuilds the scene without drawing it cannot make a frame look current.
- **M2 step 4: `GraphView` catches up at the end of every `update`** (plan §4.2's synchronous
  pacing), consuming `pending`: a `Topology` commit relayouts (`Change::Topology` with the
  absorbed `added`) and refits; a `Geometry` commit puts each repinned node's world position
  at its pin and re-snaps the reshaped and repinned nodes, and if that pushes more than 8 nodes
  aside, relayouts around them (`Change::Reshaped`, the same reach a level change has) and
  refits; a `Property` commit moves nothing. A zoom that crosses a semantic level remeasures
  (`ResolvedMetrics::set_level` now returns the nodes whose box changed size) and relayouts
  around those; a zoom within a level only re-snaps on the way out. A resize relayouts in
  full. Until the first render a view lays out for an 80 × 24 viewport, so a first render at
  another size is a resize: one more layout, rather than a public size option nobody asked
  for. The builder gains `layout(LayoutKind)`, `fit(Fit)` and `cell_aspect(f64)`
  (`DEFAULT_CELL_ASPECT` 0.5; an aspect that is not finite and positive keeps it); `Fit` and
  `LayoutKind` are public and `#[non_exhaustive]`. Known gap: a commit that both changes the
  topology and reshapes a box relayouts by topology alone, so the reshaped node is not a source
  of the reach.
- **M2 step 4: the snapper's fit (plan §6).** At zoom 1 the nodes' centers span the grid less
  a one-cell margin and half the widest and tallest box at each edge, so every box fits.
  `Fit::Contain` takes the smaller of the two axis scales (columns per world unit across, rows
  per world unit times `cell_aspect` down) and centers the slack axis; `Fit::Stretch` scales
  each axis to fill. A world of one point fits to the middle. The zoom the snap runs at scales
  the fit about the margin, the same point `derive` scales about, so a snap at `ref_zoom`
  derived at `ref_zoom` with no pan is the snap (invariant I). A snap stores each node's
  anchor cell (`NodeForm::anchor`, its box's middle), and its box is placed around it, so an
  edge's end and a node's box cannot disagree.
- **M2 step 4: collision resolution.** Nodes are placed pinned first, then by degree (every
  incident edge, most first), each in canonical order. Every box is grown by one row above and
  below for the test, so stacked boxes keep two empty rows between them (ledger row 28); plan
  §6 rules no horizontal gap, and there is none. A colliding node walks the seed's spiral: its
  corner walk, down, left, up, right, each leg longer every other turn, 100 legs being plan
  §6's 50 rings, with the seed's ring counting kept. The seed stepped a box width across and
  `max(2, height)` down; here a step down is the box's height plus the two-row gap, so a node
  moved down lands right below its blocker. Past the last ring a node keeps the last candidate
  and overlaps (`Snap::overlapping`); every node moved off its rounded fit cell is in
  `Snap::displaced`, which plan §6's re-snap cascade counts. Claimed cells sit in a hash set
  with a fixed hasher, only ever asked about, so nothing random reaches the output.
- **M2 step 4: the viewport derives half up, and moves canonical cells three ways.** `derive`
  rounds `⌊v + ½⌋`, not half away from zero, because only that commutes with whole cells: a
  pan grown by `d` cells moves every drawn cell by exactly `d`. So anchor compensation, which
  moves the pan by whole cells after a refit, keeps the focused node exactly where it was
  drawn (invariant G asks within 3 cells), and the zoom-out re-snap leaves every node that does
  not move exactly where it was drawn. A refit fits a new layout afresh at the current zoom,
  which becomes `ref_zoom`. A re-snap, after boxes or pins change, keeps the last refit's
  transform: the changed nodes go where it puts them, every other node starts from its cell,
  and what a changed box pushes is the cascade plan §4.2 counts (`Snap::displaced`). The
  focused node is the host's (M3's controller); with none, nothing is compensated, as in the
  seed.
- **M2 step 4: the zoom-out re-snap runs when drawn boxes meet, not only after a displacing
  snap.** Plan §6 re-snaps "on a zoom-out frame where the last snap displaced anything", but
  within one semantic level boxes keep their width while spacing shrinks with the zoom, so
  nodes no snap ever displaced meet too: level 2 spans zooms 0.35 to 1.5, and two 14-column
  boxes 20 columns apart at zoom 1 overlap at 0.5. On a zoom below `ref_zoom` the drawn cells
  are resolved again; if nothing has to move nothing changes, so this moves no more than plan
  §11.2 allows ("only nodes that would otherwise overlap"). It costs one resolve per zoom-out
  frame, O(n × box cells). It departs from plan §6's wording; the owner ratified it ("fine
  with the rest", "Relayout follow-ups").
- **M2 step 3: `layout_fr.rs` splits three ways.** Its FR core is ported in
  `layout/force/simulation.rs`, a rewrite of the loop, so `git show -M20%` pairs the staged seed
  with `ring.seed.rs`, its largest verbatim slice, and shows the simulation as new.
  `snap_to_grid`, `bounding_box` and `rect_overlaps` with their tests are staged verbatim as
  `geometry/snap{,_tests}.seed.rs` for step 4, and `radial_layout`, `centroid`,
  `max_distance_from` (with `rect_overlaps`) and their tests as
  `layout/force/ring{,_tests}.seed.rs` for the peripheral ring. The staged files are byte-exact
  slices of the moved file, as `word_wrap`'s copy was.
- **M2 step 3: the simulation's arithmetic.** A world position is a node's center. Each body's
  repelling mass is its weight (`Uniform`), times its degree plus one (`DegreeScaled`), and
  the force on `i` is `mᵢkᵢ Σ mⱼkⱼ / d` with `kᵢ` its ideal distance (see the per-node
  decision), which is the seed's FR when every weight is 1 and every `kᵢ` is `k`. The quadtree
  takes weighted bodies (mass times ideal distance) and returns each point's stiffness, `Σ mⱼ
  k² / d²`, from the same walk; a massless body keeps its place (emptiness is tested by body,
  not by mass) and pushes nothing. At or below `bh_threshold` the exact sum runs on the tree's
  own kernel, so θ = 0 matches it. Gravity stays unweighted, toward the unweighted centroid, as
  in the seed; attraction is `weight · d² / √(kᵢ kⱼ)`. A pinned body pushes and pulls but never
  moves, and its own force is not summed; a move that would leave a coordinate non-finite is
  skipped. `ForceParams` is
  `#[non_exhaustive]` with public fields, so a host edits a default or a preset (`quality`,
  `fast`), and a run clamps its floats (NaN takes the default, `cooling` keeps to `0..=1`, the
  rest to finite non-negative values) rather than refusing them.
- **M2 step 3: a new body starts beside its neighbors.** A run is warm when any body has a
  previous position (the seed's `has_seeds`). A new body in it starts half `k` from the mean of
  its already-placed neighbors, turned by its index times the golden angle so siblings fan out;
  with none placed, it starts on the circle around the placed bodies. The seed put every new
  node on the cold circle around the origin, so its first steps dragged its neighbors across the
  layout, which ramp-in only softens. The ramp scales what a new body exerts (its repelling mass
  and its springs' pull on others) by `(step + 1) / ramp_in_iterations` until it is whole.
- **M2 step 3: islands and the ring (plan §8.2).** The core is the nodes joined by a layout
  edge that is not a self-loop; each connected component is simulated alone with one base
  distance `√(area / core size)` for the whole core, so all islands share a scale. An island none of whose nodes has a place
  (previous or pinned) is new: islands are ordered largest first, then by first node, and new
  ones are packed left to right after the placed ones' box (from x = 0 when none is placed),
  4 columns apart, centered on the placed ones' middle. A placed island stays where its
  simulation leaves it, so a relayout never shifts an island wholesale. Every other node rings
  the core: pinned at its pin, else kept where it was, else placed with the other new ones,
  evenly, in order of the bearing of its core neighbors along any edge (none last), the first
  on its own bearing. The radius is `(farthest core corner + 3) · 1.1`, at least the ring's
  boxes plus 2 each, over 2π. The seed halved the first term (`(max_dist + 3) / 2 · 1.1`)
  because its core already filled the viewport when the ring was placed, so a ring outside it
  would have been off screen; here the snapper fits core and ring together afterwards, and the
  halved ring cut through the core (the ring tests fail with it). The ring is round in world
  space; the seed squashed its height by half in cells. A topology `Change` carries the
  commit's `added` set, whose slots start fresh even where the state still holds a removed
  node's place.
- **Relayout: a change reaches `tether_reach` hops (owner, "Relayout").** The ruling asks
  that new nodes lead the older nodes they affect to be reassessed, that a level change stay
  local to the reshaped nodes, and that a resize free every node. A topology or level `Change`
  reaches `tether_reach` (2) hops along layout edges from what changed: a node new to the
  layout, or one that had no layout edge and now has (its ring place says nothing about where
  it belongs, so it starts beside its neighbors), is hop 0; a survivor that gained or lost a
  layout neighbor is hop 1, found by comparing each node's neighbors with those `ForceState`
  kept from the last run (the store's `Delta` does not say which survivors were rewired); a
  reshaped node is hop 0 of a level change. Within reach a node moves as its forces say,
  tethered to where it was by `tether_near` (0.1) up to one hop and linearly up to
  `tether_far` (30) at the reach; beyond it a node holds its place exactly (pinned there for
  the run, still pushing), and an island with no node left free is not simulated at all. This
  is mobility fading with graph distance (Frishman and Tal 2008) with a horizon. Measured over
  the generated families (12 seeds each at 60, 120 and 200 nodes, 5% added, 120 × 40 cells),
  among options weighed in one sitting: every survivor free kept 38% to 56% within two cells
  (F) on average and 2% in the worst run, and moved 25% to 46% of an unchanged graph's nodes
  past two cells; a uniform tether to the old place trades one for the other (at 1, F 78% to
  82%; at 10, 93% to 96%, but the new nodes' neighbors were left 5.6 to 8.9 world units from
  where their forces balance, measured as force over stiffness, against 0.1 to 0.3 with the
  reach); letting merged islands slide as rigid bodies made F worse; the reach kept F at 92%
  (pooled; worst run 79%) with every unchanged rerun at 100%. (The comparisons ran on an
  earlier, undamped version of the step; the final numbers below are on the damped one.) Who moves: 44% to 47% of the new
  nodes' direct neighbors move more than two cells, 1% to 3% at two hops, none beyond, so F's
  remainder is the reassessment itself. Reassessment, as the net force on a new node's
  neighbors where they end over the force had they stayed put: 0.04 to 0.1 over the suite,
  and 0.06 in the reassessment test, where the step 3 stopgap left 0.81. A resize scales the old layout about its centroid by the ratio of base
  distances (the equilibrium when every force scales with distance), then relaxes it cold with
  every node free, no tether, islands packed again and the ring placed again. The owner
  ratified the reach and tether values and overruled a merged island's far side holding: "it's
  not okay for islands to be stretched, they need to pull together." Two islands a new node
  joins must be drawn together rather than left apart on a stretched edge; the rework is
  "Pull joined islands together" under `## M2`.
- **M2: the simulation steps by damped Jacobi under an adaptive temperature,** departing from
  plan §8.1's kept "displacement `min(‖F‖, T)`" and multiplicative cooling. The seed's
  schedule (span / 2, × 0.95 a step) drops below `converge_eps` near step 94, so a cold run
  stopped frozen, not at equilibrium (500 or 2000 iterations changed nothing), and a warm
  restart relaxed everything at once: the reason step 3 had to freeze survivors. Moving a body
  its whole force overshoots whenever it is stiffer than 2, which one spring with its other
  end free already is. Now a body moves `F / max(D, 1)`, capped by the temperature, with `D` a
  Gershgorin bound on its force's Jacobian: twice the stiffness of its springs (`2wd / k`),
  repulsion (`k² / d²`) and gravity, plus its tether's. With `D` only once, two free bodies
  on a spring oscillate forever at a factor of −1 (a test caught it); doubled, a step never
  overshoots, and a body whose forces balance stays put however hot the run. The temperature
  follows Hu (2005): `× cooling` after a step that raised the total squared force, `÷
  cooling` after five in a row that lowered it. Defaults unchanged. Cost, release on this
  container, median of 7 × 12 seeds, against the stopgap in one sitting: cold 200 nodes /
  ~320 edges 9.9 ms (was 7.7), warm 3.1 ms (was 5.3; holds skip most of the work), cold 1,000
  / ~1,650 105 ms (was 82): within §15.1's 50, 10 and 400 ms. Ratified by the owner ("fine
  with the rest").
- **M2: each node has its own ideal distance,** `kᵢ = √(area / core size) × max(widthᵢ / 4,
  1)` with `k_label_scale` on, and a pair rests at `√(kᵢ kⱼ)`. Plan §8.1 scales one `k` by the
  average label width, so a level change that widens some boxes would rescale every distance;
  per node, a widened box pushes harder and its neighbors make room (the level test: they move
  out by 36 world units for a box 31 columns wider), and nodes past the reach hold. The base
  is recomputed every run, so a topology change that grows the core shrinks every ideal
  distance slightly; held nodes keep the old scale until a resize or a change reaches them.
  Ratified by the owner ("fine with the rest").
- **M2: invariant F is read over the suite, not run by run.** Plan §11.3 states it as
  statistical over a seeded fixture suite; step 3 had asserted it per run, which only frozen
  survivors could meet. With reassessment, a run whose new nodes neighbor many survivors keeps
  less (the worst of 24 keeps 79%), so the test pools all 24 runs' survivors: 1890 of 2053,
  92.1%, against 90%. The threshold is unchanged. Ratified, and the owner ruled out a
  per-run floor: "no per-run floor."
- **M2: joined islands pull together as rigid bodies, not by freeing them** (owner, "Relayout
  follow-ups"). Measured on two grids joined by one new node: freeing the islands cannot pull
  them together, because FR's repulsion reaches across any distance, and a cold layout of the
  joined graph sets their centroids 160 world units apart, against 59 when packed. So a join
  first runs `merge::pull_together`: each island of the last layout that the change joins,
  except the one with the most survivors (then the one holding the canonically first) and any
  with a pinned node, slides and turns as one body; new nodes move as points. Only the joining
  springs pull, and bodies of different islands push only within `CONTACT` = 2 ideal distances
  (Fruchterman and Reingold's own 1991 grid cutoff), so islands close in without overlapping.
  Steps are damped by stiffness and capped by a Hu temperature, starting hot enough to halve
  the longest joining edge, since packed islands can sit further apart than the frame is wide.
  The relayout then runs from the moved places as after any topology change: its FR repulsion
  re-lengthens the new node's edges from about 1.0–1.3 rest lengths to 1.6–1.9, which is 1.2–1.4
  of the islands' median edge, while every node past the reach keeps its moved place. `ForceState`
  now keeps each node's island. Unratified: rigid motion over freeing, the anchor choice, and
  `CONTACT`.
- **M2 step 3: the generated families (plan §20) are `fixtures::generated(seed, n)`.** One
  `SplitMix64` stream (checked against Vigna's reference outputs) draws, in order: each node's
  group (one in twenty ungrouped, the rest in contiguous groups of about 12), label (one to
  three words, 70% ASCII, else CJK, emoji or combining-mark words) and style values; then each
  in-group pair joined with probability 0.3, each grouped node reaching out with probability
  0.03, each ungrouped node twice; then `max(1, n / 50)` self-loops and `max(1, n / 40)`
  parallel copies. Floats are kept to thousandths, like the community fixture's decimals,
  because `serde_json`'s default parser can miss the last bit of a 17-digit float, and a
  generated graph must read back from JSON unchanged.
- **M2 step 3: coincident bodies do not push each other.** The kernel gives no force within
  1e-4 (the quadtree's coincidence rule, shared by the exact sum), so two free bodies at one
  point part only through other forces. Starting positions make that unlikely; a deterministic
  split by index is the fix if it shows.
- **M2: `ResolvedMetrics` holds each node's measured form, not its position.** A form is the
  box's size and exactly what the node draws in it (`geometry::NodeForm`); the snapper (step 4)
  places it, and the positioned box is that origin plus this size. Plan §5's `boxes:
  Vec<CellBox>` cannot carry positions, since pan moves them every frame while metrics change
  only per zoom level or geometry delta. `anchor` is a method, the middle cell rounding up and
  left, so it cannot disagree with the size. `measure` is `pub(super)`, private to `geometry/`
  as §5 rules. `GraphView` measures on every commit (`ResolvedMetrics::apply`): removed slots
  forget their form, added, relabeled and reshaped nodes are measured, and `reshaped` keeps
  only the nodes whose size changed, which is how measured boxes decide what moves.
- **M2: three node forms, decorations inside the box (plan §5).** Level 0, and any cap too
  narrow for brackets, draws one glyph: a `Glyph` node's character, else the label's first text
  cell that is not whitespace, else the glyph set's dot; one that is wider than the cap (`日`
  at cap 1) or is no glyph at all (width 0) gives way to the dot. So level 0's width is the
  glyph's, at most the cap, rather than the seed's "the cap itself"; `visual_width` became
  `SemanticZoomTable::cap`. Levels 1 to 5 draw one row, `*`, the node's glyph, then `[label]`:
  §5's "glyph prefix" is a `Glyph` node's character, which with no visible label draws alone
  (a `Label` node with an empty label draws `[]`). The pin marker is dropped only where a lone
  glyph is drawn. Past the cap the label is cut to its longest run of leading text cells that
  fits, less trailing blank cells, then `…`, so a wide character never splits and the box can
  be a column under the cap. A `Box` draws a single border (the seed's live style; nothing
  asks for its double one) around the label word-wrapped to the cap less 2, then shrinks to the
  widest line; it is at least `min_w` up to the cap (the level's promise wins), at least `min_h`
  always, and at least 3 × 3, with the pin marker in its top border's second cell, so a pin
  never resizes it. Every width is a sum of text-cell widths, so `same_layout` labels measure
  alike (property-tested at every cap from 1 to 24).
- **M2: the compositor draws each node's `NodeForm`, which is public and read-only.** `render`
  takes a `forms` callback in place of M1's `labels`, and draws a node's form from the top left
  of its item's bounds, clipped to them; a node with no form draws nothing. A node's text is
  part of its `NodeBox` item rather than a separate `Payload::Label`, so ledger row 2's
  "rendered box == scene bounds == reserved box" holds for one item, not two that must agree. `NodeForm`,
  `FormShape`, `Mark` and `RowLabel` are public because `Compositor` is (scenes are still built
  only inside the crate, so nothing outside can make one); `ResolvedMetrics` stays
  crate-private. `RenderOptions::nodes` is a `NodeGlyphs` (`UNICODE` `[ ] … * •`, `ASCII`
  `[ ] ~ * o`) beside `BoxGlyphs`, which also draws a boxed node's border, until §12's
  `GlyphSet` gathers them. A box fills its inside with spaces in the node's style and centers
  its lines both ways, rounding up and left. Seed `ui/box_layout.rs` is deleted with this port
  (plan §18: "reference for box drawing only"); nothing of its row, pipe or routing helpers has
  a caller. Invariant N is `invariant_n_every_form_draws_exactly_its_measured_box`: the cells a
  form writes are exactly its measured box, at every level and at every cap from 1 to 24.
- **M2: a view starts at zoom 1.0** (`DEFAULT_ZOOM`, one world unit per cell column, the
  identity), which is semantic level 2, so labels show `[truncated]` to 14 columns until zoomed.
  The seed excerpts carry no starting zoom, so this is a choice, not a harvest.
- **M2: `GraphView` lives in `view.rs` at the crate root; `graph/` is the store.** Plan §3 puts
  "store, keys, deltas, errors" in `graph/`, and `GraphView` composes the store with positions,
  the controller and caches (plan §4.2), so it sits above all of them. `catena::{GraphView,
  GraphViewBuilder, NodeSpec, EdgeSpec, GraphError}` are re-exported at the root for §13.
- **M2: slots are reused, lowest first, once the transaction that freed them commits.** A
  `NodeIx`/`EdgeIx` names its node or edge until the next commit; the host's handles are the key
  and the never-reused `EdgeId`. A freed slot waits for the commit so no index returned inside a
  transaction names two nodes. Nothing iterates slots on an output path: the store keeps the
  canonical order (nodes by `(sort key, key)`, the sort key defaulting to the label; edges by
  their ends' places in node order, lower then higher, then `EdgeId`), and incidence as CSR
  lists in that order, rebuilt on a topology commit, or on a property commit whose sort-key
  changes broke an adjacent pair. The interning and edge-id tables are `HashMap`s, lookup only.
- **M2: a parallel edge's pair is unordered.** Its rank counts the live edges joining the same
  two nodes either way round that were added before it, since plan §7.3 fans a reciprocal pair
  as two parallel edges. Ranks close up when a parallel edge leaves.
- **M2: how edits classify, beyond plan §4.2's lists.** A class is the strongest of the
  transaction's operations, so adding and removing one node is `Topology`; an edit that leaves a
  spec equal is no change, and a transaction of only such edits is no delta. An edge's
  `directed` flip is `Topology` (it changes a DAG's ranks); a pin set, moved or cleared is
  `Geometry` (a local re-snap moves only that node); a sort key is `Property`. The delta lists a
  node as `reshaped` when its box may change (shape, label layout, or a pin set or cleared,
  since the in-box marker comes and goes), as `repinned` on any pin edit, so a moved pin still
  re-snaps once step 2's measured boxes drop the reshapes that changed no box, and as
  `relabeled` on any label edit, so a property edit's new text is measured again. A label edit is
  `Property` exactly when both labels have the same whitespace characters in the same places and
  characters of the same width everywhere else, width-0 ones included (`same_layout`): then
  every measure built from widths and breaks, word wrap included, gives the same box. Counting
  width-0 characters makes adding a combining mark `Geometry`, which costs a re-snap that moves
  nothing; skipping them was unsound (property-tested: a word of width-0 characters takes a
  line in `word_wrap`).
- **M2: specs are sanitized at the boundary, not rejected,** since plan §4.3's four variants
  are the whole error surface. Labels and sort keys are cut to `max_label_cols` display columns
  and to 15 bytes a column (a drawn cell's symbol), freeing the rest, so width-0 runs are
  bounded too; a NaN weight becomes 1.0 and others clamp into `0..=f32::MAX`; a pin with a NaN
  coordinate is dropped and others clamp into the `i32` cell range; negative zeros become zero.
  `max_label_cols` is the builder's (`DEFAULT_MAX_LABEL_COLS` = 256).
- **M2: small additions to plan §4.2's API.** `Tx::contains_node`/`contains_edge` (an upsert
  needs them, since a failed `add_node` consumes its spec), `GraphView::node_ix`,
  `node_count`, `edge_count`, and `GraphView::new()` as `builder().build()`. `EdgeRef` exposes
  its fields through methods so it can grow. `GraphError` stays exhaustive, as the plan writes
  it. A panicking `update` closure, like an `Err`, commits nothing.
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
  its measured form (since M2 step 2; see its decision), clipped to its bounds and the surface (a
  wide glyph cut by either is dropped). A badge draws `×n` at its cell after its item, clipped to the item's bounds. A glow patches its
  bounds' backgrounds with the style's background (its foreground if it has none), after edges so
  edge glyphs keep theirs.
- **M1: T2 goldens are `insta` snapshots in `catena`'s unit tests** (`src/scene/snapshots/`),
  because scenes are built from crate-private indices until `GraphView` exists. `insta` enters
  with no default features and without `filters` (a deterministic frame needs no redaction, plan
  §16.3; the dependency policy's "with `filters`" is the opposite of that rule, so it is not
  taken). A missing or changed golden fails, locally and in CI; `cargo insta review` accepts.
  The first set renders four primitive scenes (a triangle with a curved edge in all four
  blitters, orthogonal tree connectors with tees and a cross, crossing layers under node text
  with a badge, a shared trunk) at one size each; the §20 fixture goldens at two viewport sizes
  come with the layouts.
- **M1: the sextant table is checked against Unicode's character names,** a 60-entry table
  generated from the Unicode 15.1 database (`BLOCK SEXTANT-<cells>`), not against the encoder's
  own arithmetic; `▌`, `▐`, `█` and a space fill the four patterns Unicode encodes elsewhere.
- **Gates run on the newest stable rustc; CI keeps `toolchain: stable`.** Owner: "just update to
  rust 1.99 so you match latest stable so you don't have conflicts with gh ci." So CI is not
  pinned (plan §17's `check` is "× stable"), and a session switches its local default to the
  newest stable before gating (`rustup update stable && rustup default stable`; the image ships
  1.97, and stable was 1.99.1 at the last update).
  A clippy-lint scope that a release can stop firing takes `allow` with its reason, not
  `expect`, because an unfulfilled expectation fails the gate when clippy's lint set moves.
- **Only `catena` has a `[[bench]]` target; the other crates set `autobenches = false` and
  declare none.** Plan §3 shows `benches/` only under `catena` and §3.1 says "at most one bench
  target"; an empty bench target would cost a link in every `--all-targets` gate. Until criterion
  lands at M2 the bench is a `fn main() {}` placeholder.
- **Dependencies enter when first used.** `sha2` arrives with the SVG port (M1), `proptest` with
  the first property test; `catena-ratatui` takes `ratatui-core` at M0 so the lockfile pins the
  version whose `rust-version` (1.88) sets the MSRV (plan §3).
- **CI's `bench` job prints its table and posts nothing.** Plan §17 adds criterion
  `--save-baseline` with an advisory PR comment past 15%, but the job runs only on pushes to
  `main`, where there is no PR to comment on; and an artifact upload would need a pinned SHA of
  `actions/upload-artifact`, a repository outside this session's GitHub scope to look up. A
  runner has no committed baseline entry, so each run seeds one in the runner and only the
  §15.1 budgets can fail it. Unratified.
- **The zero-allocation test counts with `allocation-counter`, not an allocator of our own.**
  Plan §15.2 asks for a counting `#[global_allocator]` in `catena`'s integration target, but
  `GlobalAlloc` takes an `unsafe impl` and the workspace forbids `unsafe_code`, and tests run in
  parallel threads, so a global count would include other tests' allocations. The crate counts
  per thread and installs its allocator when linked (dev-only, MIT/Apache-2.0, no
  dependencies); only the integration target links it.
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
  next commit, so the port diff is still the record. The seed file is deleted with the port it
  informs: M1's orthogonal routes needed none of it (their glyphs come from an arm table), so
  that is M2's bordered node box (`render_box`'s single and double borders, for
  `NodeShape::Box`).
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
- [x] **Step 4: the scene** — `SceneGraph`, `SceneItem`, `Layer`, `Payload` (A3), both `Route`
  kinds, per-edge routes and shared segments (A3), `CountBadge` (A3), the compositor's OR-merge,
  the label mask, clipping (exact, in the walk); `insta` and the first T2 goldens.
  verify: `cargo test -p catena scene:: && cargo insta test --check`
- [x] **M1 — Raster + scene** — `Surface`, `CellGrid`, `SubCellCanvas`, four blitters,
  compositor with OR-merge, label mask, clipping, `SceneGraph` with both `Route` kinds, the
  §7.1 text-cell rule, the SVG renderer ported onto `CellGrid`; T2 goldens, T3 live; A3's
  shared-geometry room in the scene.
  verify: `cargo test -p catena -- raster:: scene:: && cargo insta test && test ! -e
  catena-testkit/src/svg.seed.rs && test -n "$(git ls-files '*.hash')" && cargo test -p
  catena-testkit perturbing_one_color_fails_the_committed_snapshot` (plan §19 writes the first
  command as `cargo test -p catena raster:: scene::`, which cargo rejects: two filters go after
  `--`)

## M2 — Graph model + force layout + viewport

Plan §19, in the six steps of `## Now`, each green on its own. The milestone closes on its own
`verify:` under `## Later milestones`.

- [x] **Step 1: the graph store** — `GraphView<K>`, `Tx` with all-or-nothing `update`,
  `GraphError`, `EdgeId`, interning into `NodeIx`/`EdgeIx` slots, canonical order, CSR incident
  lists, parallel-edge rank, delta classification; checked against a model graph through random
  transactions. verify: `cargo test -p catena -- --list | grep -c
  'the_store_matches_a_model_through_any_transactions\|a_failed_transaction_leaves_every_slot_id_and_order_as_it_was\|edges_order_by_their_ends_places_then_insertion_with_ranks_per_unordered_pair\|node_edits_classify_by_what_they_change'
  | grep -qx 4 && cargo test -p catena -- graph:: view::`
- [x] **Step 2: `ResolvedMetrics`** — one private `measure` over text cells and the semantic
  zoom table (all six levels live), decorations inside the box, three node forms (glyph, row,
  bordered `Box` with `word_wrap`); `GraphView` measures each commit and keeps only the reshapes
  whose box changed (pin moves via `repinned`, property label edits via `relabeled`); the
  compositor draws forms; invariant N; seed `ui/box_layout.rs` deleted. verify: `cargo test -p
  catena -- --list | grep -c
  'invariant_n_every_form_draws_exactly_its_measured_box\|labels_that_lay_out_alike_measure_alike\|applying_a_delta_keeps_only_the_reshapes_that_changed_a_box\|a_pinned_node_at_its_cap_shortens_its_label\|every_commit_is_measured_and_only_boxes_that_changed_stay_reshaped'
  | grep -qx 5 && cargo test -p catena -- geometry:: scene:: view:: && cargo insta test --check
  && test -z "$(git ls-files seed/ui/box_layout.rs seed/ui/box_layout_tests.rs)"`
- [x] **Step 3: the force layout** — `layout_fr.rs` harvested by the §18 procedure: the FR core
  (`ForceParams`, `Uniform` and `DegreeScaled` repulsion over a weighted quadtree, iterations
  honored, no aspect squash, `fmath`), islands packed left to right, the peripheral ring,
  warm start with neighbor placement and ramp-in, survivors annealing from `converge_eps`;
  invariants F (over the new `fixtures::generated` families) and M (weighted θ = 0 against the
  exact sum). The snapper's half of the seed waits staged in `geometry/snap*.seed.rs`.
  verify: `cargo test -p catena -- --list | grep -c
  'a_warm_relayout_after_five_percent_additions_keeps_nine_in_ten_survivors_within_two_cells\|weighted_bodies_at_theta_zero_match_the_weighted_brute_force_sum\|iterations_are_honored_as_given\|the_starting_circle_is_round\|degree_scaled_repulsion_weights_each_pair_by_both_masses\|islands_pack_left_to_right_largest_first\|isolated_nodes_ring_the_core'
  | grep -qx 7 && cargo test -p catena -- layout::force && cargo test -p catena-testkit generated
  && test -z "$(git ls-files seed/graph/layout_fr.rs seed/graph/layout_fr_tests.rs)"`
- [x] **Relayout per the owner's ruling** (`owner-rulings.md`, "Relayout") — survivors near a
  topology change move as the forces say (no blanket cap), a level change relayouts locally
  around the reshaped nodes, a resize relayouts every node; within the §15.1 budgets,
  measured (the decisions above). The three tests named below were watched red against the
  old engine behind the new `Change` API.
  verify: `cargo test -p catena -- --list | grep -c
  'new_nodes_lead_their_neighbors_to_be_reassessed\|a_level_change_moves_only_nodes_near_reshaped_ones\|a_resize_relays_out_every_node'
  | grep -qx 3 && cargo test -p catena -- layout::force && ! grep -rn "settle" catena/src/layout/force/simulation.rs`
- [x] **Step 4: the snapper and viewport** — `geometry/snap{,_tests}.seed.rs` ported by the
  §18 procedure: isotropic `Fit::Contain` (`Fit::Stretch` opt-in), `cell_aspect` applied once,
  one 50-ring spiral, the enforced 2-row gap, off-screen nodes resolved (ledger rows 7, 9,
  28); the viewport's canonical/derived split with `f64` pan and zoom, zoom-out re-snap and
  anchor compensation; `GraphView` lays out after a topology commit, re-snaps after a geometry
  commit (relayout past 8 pushed), relayouts around reshaped boxes at a level change and in
  full at a resize; invariants A, G and I. verify: `cargo test -p catena -- --list | grep -c
  'invariant_a_no_two_boxes_intersect\|invariant_g_a_relayout_keeps_the_focused_node_where_it_is_drawn\|invariant_i_derived_at_the_snap_zoom_with_no_pan_is_canonical\|an_edit_that_pushes_more_than_eight_nodes_aside_relays_out\|crossing_a_semantic_level_relays_out_around_the_reshaped_nodes\|a_crowding_zoom_out_moves_only_the_nodes_that_meet'
  | grep -qx 6 && cargo test -p catena -- geometry:: view:: && test -z "$(git ls-files
  catena/src/geometry/snap.seed.rs catena/src/geometry/snap_tests.seed.rs)"`
- [x] **Step 5a: draw the layout** — `draw::build` turns the view into a scene: each edge an
  `EdgePath` on `EdgesUnder`, routed straight from its source's anchor to its target's (a
  self-loop is its anchor alone), then each node a `NodeBox` on `Nodes` bounded by its drawn
  box, both in canonical order. `GraphView::scene` returns it; `render(surface)` lays out for
  the surface's size first (plan §7.1) and draws through a kept `Compositor`;
  `render_to_string(cols, rows)` is the no-terminal export; the builder gains `blitter`. Items
  draw in two placeholder styles until §12. Invariants B (one route per edge, anchor to
  anchor), C (every drawn cell inside an item's bounds, and each item alone inside its own) and
  D (any insertion order draws the same frame) on generated graphs, the reserved-box rule
  (ledger row 2) and a T2 golden; each was watched failing against a sabotage (edges dropped,
  edge bounds cut to a cell, node bounds a column short, the layout in slot order).
  verify: `cargo test -p catena -- --list | grep -c
  'invariant_b_every_edge_has_one_route_from_its_source_anchor_to_its_target\|invariant_c_every_drawn_cell_lies_in_an_item_that_drew_it\|invariant_d_any_insertion_order_draws_the_same_frame\|a_node_items_bounds_are_its_measured_box_where_it_is_drawn\|rendering_lays_out_for_the_surface_it_draws_on'
  | grep -qx 5 && cargo test -p catena -- draw:: && cargo insta test --check`
- [x] **Step 5b: T3 snapshots of real scenes** — the community graph at 120 × 40 in braille and
  ascii, and generated family 7 at 120 nodes in braille, drawn through the public API and
  hashed with their colors in `catena/tests/visual/`; each frame was read as text before its
  hash was stored, and a missing hash was seen failing closed. Made on Linux, so CI's macOS and
  Windows `check` legs are the first cross-platform test of plan §11.5's same-frame guarantee.
  At the default zoom the 120-node frame saturates its 4,800 cells, so the snapper pushes some
  boxes past the edge and the renderer clips them (plan §6, "clip, don't drop").
  `FixtureGraph::view` loads a fixture into a `GraphView`. verify: `cargo test -p catena --test
  catena -- scenes:: && test -f catena/tests/visual/community_braille.hash`. CI run
  41910022430 passed on all three platforms with these hashes.
- [x] **Step 5c: the T4 tier** — `seed/tests/functional/test_blink_core.rs` harvested by the
  §18 procedure as `catena-testkit::oscillation`: `diff_count` and `diff_details` over
  `CellGrid`, the A→B→A detector `flickers`, and `CaptureBuf` for M5's PTY tier.
  `catena-testkit::frames` names fixtures (`community`, `generated:<seed>:<n>`, at most 5,000
  nodes) and blitters, and draws them; the `catena-render-hash` bin prints a named frame's hash.
  The cross-run test spawns it twice for five cases (all four blitters) and compares the two
  hashes with each other, with the library's own frame, and with `catena-testkit/tests/hashes/`;
  the community and generated-7 hashes equal their T3 twins. A quiescent view redrawn 50 times
  changes no cell (seen failing with a pan that drifted a cell per frame). Not yet: §16.5's
  `tick(16ms)` between frames and the interaction scripts, which need M3's controller.
  verify: `cargo test -p catena-testkit -- determinism:: oscillation:: && cargo test -p catena
  --test catena -- a_quiescent_view_redrawn_fifty_times_changes_no_cell && test -z "$(git
  ls-files seed/tests)"`
- [x] **Step 5d: invariants E and N on real scenes** — E: every edge of three laid-out
  generated graphs, drawn alone on a braille canvas, lights exactly its pixel walk clipped to
  the frame, 8-connected, its on-frame ends lit, no pixel walked twice (seen failing when the
  canvas dropped one dot; an integration test, since the braille asserts take the testkit's
  copy of `catena`'s types). N: every node of four generated graphs, whose labels mix CJK,
  emoji and combining marks, drawn alone fills exactly the cells its box reserves, boxes that
  cross the frame's edge aside (seen failing with each label's closing bracket dropped). The
  compositor's cell-to-sub-pixel mapping is now one function. verify: `cargo test -p catena
  --test catena -- invariant_e_every_drawn_edge_is_one_unbroken_line_between_its_ends && cargo
  test -p catena --lib -- invariant_n_on_real_scenes_every_box_draws_exactly_its_cells`
- [x] **Step 6a: a steady frame allocates nothing** (plan §15.2) — the view keeps its scene
  and rebuilds it only after an `update`, a pan, a zoom or a new size (`Drawing::current`); the
  compositor already reused its buffers. Measured by `allocation-counter` (see the decision):
  generated family 9 at 200 nodes into a kept 200 × 50 grid, the third frame makes 0
  allocations in every blitter, against 342 (14,848 bytes, braille) before. An edit, a pan and
  a zoom each redraw (each seen failing with its invalidation removed). verify: `cargo test -p
  catena --test catena -- frames:: && cargo test -p catena --lib --
  a_pan_or_a_zoom_redraws_the_kept_scene`
- [x] **Step 6b: the plan §15.1 benches** — `catena/benches/catena/main.rs` on criterion 0.8
  (rust-version 1.86; no rayon or plotters), through the public API over generated family 1:
  `layout/cold_{200,1000}`, `layout/warm_5pct_200`, `frame/{steady,rebuilt}_{200,1000}` into a
  kept 200 × 50 grid, and `update/property_1000`; the binary then prints its peak resident set
  (Linux). On this container (`vm`, 4 × Xeon 2.10 GHz, 16 GiB), medians: cold 200 10.4 ms
  (budget 50), cold 1,000 98.8 ms (400), warm 3.6 ms (10), frame 200 0.94 ms (3), frame 1,000
  7.7 ms (16), property update 0.66 µs (100); peak RSS 14.7 MiB. A steady frame costs what a
  rebuilt one does: the compositor dominates, not the scene. Hit-testing (M3) and the layered
  pipeline (M4) join with their code. verify: `cargo bench -p catena --bench catena -- --list
  | grep -c ': benchmark' | grep -qx 8`
- [x] **Step 6c: the perf ratchet** (plan §15.1) — `scripts/check-perf.sh` runs `cargo xtask
  perf`: the bench target, then every budgeted bench against its §15.1 budget on any machine,
  and every bench and the peak RSS against 1.5× of this machine's entry in the committed
  `bench-baseline.json` (past it, the owner's approval, then `UPDATE_BASELINE=1`). A machine
  with no entry seeds one loudly; an estimate older than the run is never read, so a bench that
  did not run fails as "not measured" (seen, when two benches outside groups filed under
  another directory, which put them in groups). A machine is its host, CPU model, cores and
  GiB of memory, from `/proc` or `sysctl`. The judgment is I/O-free and unit-tested (seeding,
  the ratchet, budgets, reseeding one machine, added benches, JSON round trip, malformed
  baselines); the ratchet and budget tests were seen failing with each bar loosened. This
  container's entry is seeded, and a second run passed at 1.00–1.06×. verify:
  `./scripts/check-perf.sh && grep -q '"host": "vm"' bench-baseline.json`
- [x] **Step 6d: the coverage and bench CI jobs** (plan §17) — `coverage` runs `cargo llvm-cov
  -p catena --summary-only --fail-under-lines 80` on every push and PR (measured here: 97.9% of
  5,088 lines over catena's 41 production files; the flag was seen exiting 0 at 80 and 1 at
  99). `bench` runs `scripts/check-perf.sh` on pushes to `main`. verify: the CI run on the
  commit that adds them shows `coverage` and `bench` with conclusion success (run 41914698918
  on `337deaa`: every job success).
- [x] **Pull joined islands together** (`owner-rulings.md`, "Relayout follow-ups") — the owner:
  "it's not okay for islands to be stretched, they need to pull together." `layout/force/merge`
  moves each island a change joins, but the largest, as one rigid body (sliding and turning)
  until its joining edges rest, before the usual reach-limited relayout; behind the
  crate-private `ForceParams::pull_joined`, off until the owner rules on invariant F (the
  question under `## Owner`). The test joins two grids on their facing and their far sides:
  without the pull, the joined nodes were dragged out of their islands (an own edge grew from
  13.7 to 36.7) and joining edges ended 1.57 median edges long; with it, at most 1.23× and 1.41
  medians. Its bounds (joining edges ≤ 1.5 medians of the islands' own edges, a joined node's
  own edges grow ≤ ⅓) are derived, unratified; the rest length `√(kᵢ kⱼ)` the item first named
  is no yardstick, since FR's repulsion draws every edge longer than it (the islands' median is
  1.37 rest lengths). verify: `cargo test -p catena -- --list | grep -c
  'a_node_joining_two_islands_pulls_them_together\|the_smaller_island_turns_and_slides_until_its_joining_edge_rests\|an_island_with_a_pinned_body_stays_put_even_when_smaller'
  | grep -qx 3 && cargo test -p catena -- layout::force`

## M3 — Interaction + widget + demo

Plan §19, in the seven steps of `## Now`, each green on its own. The milestone closes on its
own `verify:` under `## Later milestones`.

- [x] **Step 1: the input model** (plan §10.1) — `catena::interact`: `InputEvent` (key, mouse
  down, up and move, scroll, resize), `KeyInput` of a `KeyCode` and `Modifiers` flags,
  `Button`, `Dir`, `Action` (pan, zoom, cycle, spatial nav, pin, clear, and A4's
  `ToggleBundling`), and `Keymap`, an ordered table with `bind`, `unbind` and plan §10.1's
  defaults. Choices of mine, unratified: `b` toggles bundling; `=` zooms in beside `+`, since
  it is `+` without Shift on most layouts; a character key drops Shift (its case carries it,
  and terminals disagree on reporting Shift with `N`), while Shift with an arrow stays
  distinct; rebinding a key keeps its place in the order. The Shift test was seen failing with
  that normalization removed. verify: `cargo test -p catena --lib -- interact::`
- [x] **Step 2: hit-testing** (plan §10.2) — `seed/app/types_excerpt.rs` harvested by the §18
  procedure: its `EdgeSpatialGrid` ported as `interact::spatial::SpatialGrid` (any item by
  number, frame cells, buckets kept across rebuilds); its `PanAnimation` and `ZoomAnimation`
  staged byte-exact as `interact/animation.seed.rs` for step 3. `interact::hit::HitMap`
  indexes the kept scene, rebuilt with it, and answers what a cell points at: a node whose
  drawn box holds it, else the nearest anchor within 3 aspect-corrected cells; else edges within
  1.5 cells (a shared segment's members, A3); nodes over edges, then topmost layer, smallest
  bounds and canonical order (ledger row 30). `GraphView::hit` waits on step 3's controller.
  `seed/app/selection_excerpt.rs` read, then deleted. Tested against a brute-force scan over
  every cell; invariant J holds at six zooms spanning all six semantic levels (seen failing
  with a hit map left stale by a zoom). verify: `cargo test -p catena --lib -- interact::
  invariant_j && test -z "$(git ls-files seed/app/types_excerpt.rs
  seed/app/selection_excerpt.rs)"`
- [x] **Step 3a: the glides and navigation helpers** — harvested by the §18 procedure:
  `animation.seed.rs` (the seed's `PanAnimation`, `ZoomAnimation`) ported as
  `interact::animation` over injected time, a 300 ms ease-out-cubic `PanGlide` that applies
  only increments in `f64` cells, and a `ZoomGlide` smoothing with `α = 1 − exp(−dt / 40 ms)`
  that snaps within 0.001; `seed/app/navigation.rs` ported as `interact::navigation`, the
  0.25 key and 0.08 scroll zoom steps on the current target clamped to `[0.1, 4.0]`, the
  three-cell pan-to slack, and `visible_center` counting occluded cells once (ledger row 31).
  `fmath` gains `exp`. Not carried: `force_complete`, the 65% doc comment, the seed's window,
  palette and data-refresh methods, and its `select_next_entity`, which skipped the first
  node when none was selected. The timing tests were seen failing with the seed's fixed 35%
  step. `seed/app/` is gone. verify: `cargo test -p catena --lib -- interact::animation
  interact::navigation && test ! -e seed/app`
- [x] **Step 3b: spatial navigation** (plan §10.3) — `navigation::nearest_toward`: from the
  focused node, candidates whose aspect-corrected offset lies in the ±60° cone around the
  direction score their distance × `1 + 2(1 − cos θ)`, the lowest wins; an empty cone falls back
  to the nearest node ahead in the half-plane; nothing ahead is the boundary; ties go to the
  earlier candidate. Seen failing when the score dropped its angle term. verify: `cargo test -p
  catena --lib -- interact::navigation`
- [x] **Step 3c: the controller** (plan §10.1, §10.3, §10.4) — `interact::controller` and
  `interact::pointer`: `GraphView::{handle, act, tick}` returning `Outcome`s and `Redraw`,
  with `selected`, `selected_edge`, `hovered`, `bundling`, `keymap`, `keymap_mut` and
  `set_occlusions`. A click selects a node, an edge (the first of a shared segment's) or
  nothing; a node drag pins the node under the pointer, keeping the grab offset, and on release
  restores its pin and reports `DragCompleted` with the world point (`Viewport::world_at`, the
  inverse of the fit, zoom and pan); hovering a node or an edge lifts its edges onto `EdgesOver`
  in a highlight (A4), in a T2 golden with a style map and a T3 hash; `n`/`N` cycle the sort
  order, wrapping, and Shift+arrows navigate spatially, both gliding the node to the visible
  center; Enter pins the selected node where it is drawn, or unpins it; Esc clears; `b` flips
  `bundling`, which nothing reads until the radial view; keys and the wheel zoom through the
  glide about the visible center and the pointer; `tick` clamps `dt` to 250 ms and answers
  `Idle` only when the last frame shows the view. The selected node is the anchor-compensation
  focus, and a commit that removes a held node forgets it. Retired: the dead-code expectations
  on `pan_by`, `zoom_about`, the zoom clamps, `fmath::exp`, `animation` and `navigation`; `cell`
  is test-only. Choices of mine, unratified: a `Pan` moves 4 columns or 2 rows the way it
  names; node and edge selection exclude each other; a click selects without centering;
  spatial navigation with nothing selected starts from the visible center; `ToggleBundling`
  reports no outcome while nothing bundles. Each test was seen failing under a sabotage (the
  clamp dropped, the pan reversed, occlusions ignored, `N` from none taking the first node,
  `invalidate` keeping `shown`, a removal not forgotten, the pin not restored, the grab
  dropped, one-ended hover-lift, a keyboard pin at the layout position, a hover move that
  invalidates). verify: `cargo test -p catena --lib -- interact:: view::tests::repinning &&
  cargo test -p catena --test catena -- hover_lift pointer_moving quiescent`
- [x] **Step 4a: the pointer in screen cells** — `Surface::origin` (provided, `(0, 0)`) says
  where a surface sits on the host's screen; `render` records it and `handle` subtracts it, so
  a widget in an area of the terminal takes the terminal's coordinates unchanged, as plan §13's
  loop passes them. Off the surface a press or scroll is ignored and a move hovers nothing; a
  drag past its edge holds at the edge. `KeyInput::new` drops Shift from `BackTab`, which
  crossterm reports with it. The crate root re-exports `Blitter`, `Outcome` and `Redraw`, as
  plan §13's imports have them. Seen failing with drag moves sent through the hover path, and
  with Shift kept on `BackTab`. verify: `cargo test -p catena --lib -- interact::pointer::tests::on_a_region
  interact::pointer::tests::a_drag_past back_tab`
- [x] **Step 4b: the widget crate** (plan §3, §7.1, §10.1, §16.6) — `catena-ratatui`:
  `BufferSurface`, an area of a ratatui `Buffer` as a `Surface` (its corner the origin; colors
  by ratatui's 16 names on the low four bits, `Rgb` and `Indexed` as they are; attributes by
  their shared bits), holding `CellGrid`'s width-2 contract by reading glyph widths, since a
  `Buffer` keeps no spans; `Graph<K>`, a `StatefulWidget` whose state is the `GraphView`; and
  `input::from_crossterm` behind the `crossterm` feature (crossterm 0.29, only `events`): key
  presses and repeats with Shift, Ctrl and Alt, the pointer in terminal cells with a drag as a
  move, the wheel as `±1` notches, resizes; `None` for releases, keys catena has no code for,
  sideways scrolls, focus and pastes. Tests: the adapter against `CellGrid` as the oracle over
  random writes at random offsets, nothing written outside the area (seen failing with the
  release and the shared background each dropped); the widget's frame equals the core's cell
  for cell at two offsets; T5 through `TestBackend`: a golden beside a sidebar, a seeded
  ten-resize storm holding invariants A and C, and a click in terminal cells selecting the
  node drawn there (seen failing with the origin's column dropped). verify: `cargo test -p
  catena-ratatui --all-features && cargo test -p catena-ratatui -- input:: widget::`
- [x] **Step 5: `catena-demo`** (plan §19 M3) — `catena-ratatui`'s `demo` feature builds the
  bin (`src/bin/catena-demo/`): the community fixture and `generated(7, 120)` in the `Graph`
  widget over all but the last row, a status line naming what happened by label, Tab to switch
  graphs, `q` or Ctrl+C to quit, and the mouse live. `NO_COLOR` (set, not empty) builds both
  views with `Blitter::Ascii` and the new `GraphViewBuilder::ascii_glyphs` (lines, boxes and
  node marks in ASCII) and strips every color, keeping attributes. Tests drive its `App`
  through `TestBackend`: every binding of the default table changes the frame (seen failing
  with Enter swallowed), the status line names a label, Tab and the quits, and `NO_COLOR`
  frames are ASCII in the terminal's own colors (seen failing without `ascii_glyphs` and
  without the strip). Watched under a pseudo-terminal at 100 × 30: it draws, `n` puts
  "selected Argonne Research Center" on the status line, `q` exits 0 leaving the alternate
  screen and mouse capture, and with `NO_COLOR` it sends only SGR 0, 7 and 27 and only ASCII.
  verify: `cargo test -p catena-ratatui --features demo --bin catena-demo && cargo test -p
  catena --lib -- ascii_glyphs && cargo build -p catena-ratatui --features demo --bin
  catena-demo`
- [x] **Step 6: the minimap and legend** (plan §3, §18) — `seed/ui/vector_minimap.rs` read,
  then deleted. `GraphView::render_overview(&self, surface)` draws the whole graph small from
  the layout, changing nothing in the view: a sub-cell dot a node (cells shared, no re-spacing),
  the selected node's cell filled and reversed, and under them the rectangle of what the view
  shows, in a placeholder bright cyan; nodes and rectangle are fitted together, aspect-correct,
  in a one-cell margin, so the rectangle is always in sight. `catena-ratatui`'s `widget-extras`
  adds `Minimap` (a `Widget` over `&GraphView`) and `Legend` (host rows of a swatch and a label,
  cut at the edge, never through a wide glyph), each clearing its area to float over the graph;
  `demo` implies it and shows both at the right, passing them to `set_occlusions`. Tests: the
  dots at cells worked by hand from both fits (seen failing with the fit stretched per axis);
  the rectangle frames every dot, shrinks under a zoom and follows a pan; the widgets draw the
  core's overview at an offset and clear what was under them (seen failing without the clear);
  a legend's swatch takes its color (seen failing without); T5's graph, minimap and legend in
  one layout; and the demo centers on the visible center its overlays leave, column 45.04 of
  100, not 49.5 (seen failing with the occlusions dropped). verify: `cargo test -p catena --lib
  -- overview && cargo test -p catena-ratatui --features demo && test ! -e
  seed/ui/vector_minimap.rs`

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

- [ ] **Turn on the joined-islands pull, which costs invariant F (needs the owner)** — your
  ruling ("they need to pull together") and plan §11.3's bound (≥ 90% of survivors within two
  cells after ≤ 5% additions, pooled, no per-run floor) cannot both hold on the seeded suite.
  The pull moves only the smaller islands of a join, but the suite joins islands in 11 of its
  24 runs, moving 147 of 2053 survivors (7.2%) by 130 to 1030 world units: islands pack in one
  row (the open "Fit the islands" question below), so they sit far apart. Pooled F falls from
  92.1% to 87.1%. Options: let F exclude nodes a join moves as one body (they keep their
  shape); lower the bound; or keep the pull off. The threshold is not lowered without you. verify: `grep -n 'pull_joined: true'
  catena/src/layout/force/params.rs && cargo test -p catena -- layout::force`
- [ ] **Reserve the crate names on crates.io** — a real `0.0.0` publish of `catena`,
  `catena-ratatui` and `catena-testkit`; the owner's act, recommended before M1 (plan §0).
  verify: `cargo info catena && cargo info catena-ratatui && cargo info catena-testkit`

## Found while porting

- [ ] **Draw edges as plan §7.3 has them** — step 5a draws every edge as one straight solid
  line between anchors. Still to come: `EdgeClass::Curved`'s 20%-perpendicular bow
  (tessellated, A1) and `Dashed` (arc-length dashes, A1); arrowheads on directed edges;
  crossing hops (the hop math of `seed/graph/render.rs`, ported by the §18 procedure, with
  spatial-grid candidates and the 256-hop budget); and, plan §19 M5's, parallel edges fanned by
  rank with a `CountBadge` past three, and self-loop arcs. Until then a self-loop is invisible
  under its node's mask and parallel edges overlap. verify: `cargo test -p catena -- --list |
  grep -c 'curved_edges_bow\|dashed_edges_dash_by_arc_length\|directed_edges_end_in_an_arrowhead\|crossings_hop'
  | grep -qx 4 && cargo test -p catena -- draw::`
- [ ] **Themes and the style resolver (plan §12)** — `draw::style` is two placeholder styles
  (nodes `Ansi(7)`, edges `Ansi(8)`). Plan §12's `StyleResolver`, `Theme` with its four
  built-ins (`dark`, `light`, `ansi16` ported from `seed/ui/themes/ansi16.rs`, `ascii`) and its
  `GlyphSet` (replacing `BoxGlyphs`, `LineGlyphs` and `NodeGlyphs`) replace them; the T3 scene
  hashes re-baseline then, on purpose. verify: `cargo test -p catena -- style:: && test -z
  "$(git ls-files seed/ui/theme.rs seed/ui/themes)"`

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

- [x] **Give invisible text no line in `word_wrap` (M2 step 2)** — a word made only of width-0
  characters (`"a \u{301}"` at width 1) and the marks of a wide character dropped at width 1
  (`"日\u{301}"`) each come out as a line that draws nothing, so a `Box` label would grow
  phantom rows. Found by the `same_layout` property in M2 step 1, which now counts width-0
  characters and so stays sound either way. Fixed ahead of the bordered node box, `word_wrap`'s
  first caller: a word left with no width is dropped, and the wrap property now also asserts
  that no line but a blank paragraph's measures zero. verify: `cargo test -p catena -- --list | grep -c
  'an_invisible_word_takes_no_line\|a_dropped_wide_characters_marks_go_with_it' | grep -qx 2 &&
  cargo test -p catena -- raster::text`
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
  M4, M5); the `expect`s announce themselves, the `fmath` `allow` will not. M2 step 2 removed
  `raster::text`'s and narrowed `geometry::zoom`'s to `MIN_ZOOM`, `MAX_ZOOM` and
  `SemanticZoomTable::new`. M2 step 4 put the force layout, the snapper and the viewport live,
  removing their modules' expectations and `fmath`'s `allow` (nothing there is dead now, on
  any rustc); what waits is item by item: `GraphView::{cell, resize}` and the box accessors on
  step 5, `GraphView::{pan_by, zoom_about}`, the zoom clamps and `Snap::bounds` on M3, and the
  layered, tree and radial modules on M4 and M5.
  verify: `! grep -rn "dead_code" catena/src`

- [ ] **Fit the islands and the ring to the frame (needs the owner: plan §8.2)** — the
  generated families lay out about ten times wider than the frame: at 120 × 40 cells the
  `Fit::Contain` scale is 0.04 to 0.11, because islands pack in one row (§8.2: "left to
  right") and the ring circles that strip, so a ring of a few nodes sets a square bounding box
  around a long thin core. It also bounds what a resize can do: the force layout is isotropic
  and scale-free, so its shape ignores the frame's aspect, and a resize relayout (owner: "use
  the space properly") mostly relaxes the tension local relayouts left behind. Options: pack
  islands in shelves toward the frame's aspect; an elliptical ring; gravity stronger along the
  frame's short axis (for a log-repelling cloud in an anisotropic harmonic trap the envelope's
  axes go as the inverse ratio of the gravities). verify: `cargo test -p catena --
  layout::force::engine::tests::islands_fill_the_frame` (a generated family at 120 × 40 cells
  laid out at a `Fit::Contain` scale of at least 0.5)
- [ ] **The `png` gallery (plan §16.4)** — `catena-testkit`'s `png` feature: `resvg`
  rasterizes `grid_to_svg` into a non-gating `tests/gallery/*.png`, regenerated on demand. Lands
  once scenes render (M3 at the latest, with the demo); check `resvg`'s `rust-version` against
  the MSRV first. verify: `cargo test -p catena-testkit --features png png`
- [ ] **A builder option for the semantic zoom table** — `SemanticZoomTable::new` validates a
  host's level ceilings and caps but nothing public reaches it, so it still carries a
  dead-code expectation ("lands with zoom input (M3)"). Zoom input has landed: give
  `GraphViewBuilder` a `semantic_zoom(upper, caps)` that returns `None` for a table `new`
  rejects, test it red first, and retire the expectation. verify: `cargo test -p catena --lib
  -- semantic_zoom && ! grep -n 'lands with zoom input' catena/src/geometry/zoom.rs`
- [ ] **The radial view reads `GraphView::bundling`** (owner ruling A4) — the unbundle key
  flips the flag, and nothing draws bundled yet, so it changes no frame and reports no
  outcome. When the radial layout's bundling lands (plan §9.3, §19 M5), β is 0 while it is
  off, the toggle invalidates the drawing and reports `ViewChanged`, and a T2 golden shows both.
  verify: `cargo test -p catena -- bundling_toggle_unbundles`
- [ ] **A click on the minimap moves the view there** — the seed dragged its minimap's
  rectangle (plan §10.1's note); `render_overview` only draws. Give the core the overview's
  inverse (`GraphView::overview_world_at(size, cell)`) and a glide that centers a world point,
  and have the `Minimap` widget's host route a press in its area there; the plan names no
  interaction for the minimap, so the binding is the owner's to confirm. verify: `cargo test
  -p catena --lib -- overview::tests::a_click`

## Later milestones

- [x] **M2 — Graph model + force layout + viewport** — per plan §19. Closed on `main` with:
  `cargo test -p catena` passing 427 tests, the stability suite and the cross-run hash test
  green, `./scripts/check-perf.sh` exiting 0 after seeding this machine's entry, and `cargo
  xtask lint` reporting `lint-allow: none`. verify: `cargo test -p catena` reports ≥ 200
  passing tests, the stability suite and the cross-run hash test are green,
  `./scripts/check-perf.sh` exits 0 and wrote this machine's baseline entry, `cargo xtask lint`
  reports zero `lint-allow` lines for the `fmath` rule
- [ ] **M3 — Interaction + widget + demo** — per plan §19, with A4's unbundle key and hover-lift
  in the default key table. verify: `cargo test --workspace &&
  cargo build -p catena-ratatui --bin catena-demo --features demo && cargo +nightly fuzz run
  fuzz_events -- -runs=10000`
- [ ] **M4 — Layered engine** — per plan §19 (its two test filters moved after `--`, as M1's
  were). verify: `cargo test -p catena -- layered:: tree::` exits 0, goldens exist for the 12-fixture set, edge conservation passes at 512 cases
- [ ] **M5 — Polish + package** — per plan §19, including `seed/` removed and `HARVEST_COMPLETE`
  flipped in `xtask`, and A4's goldens once the radial view renders. verify: `cargo test --workspace --all-features && cargo publish --dry-run
  -p catena -p catena-ratatui -p catena-testkit && test ! -e seed && test -z "$(git ls-files
  '*.seed.rs')"` (plus the PTY job green on linux and macos, benches within §15.1)
