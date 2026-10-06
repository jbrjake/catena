# TODO

The worklist (`CLAUDE.md`, override 1). An item closes when its `verify:` command is observed
to exit 0. Milestones and their gates come from plan §19.

## Now

M0 step 4, flex: `git mv seed/ui/flex_layout.rs catena/src/layout/layered/flex.seed.rs`
(commit 1), then port (commit 2) as `flex.rs`: correct the overflow comment (the first item that
no longer fits the remainder gets it, later items get zero — red first: a test pinning that
with the overflow in the middle item), keep all seed tests, and add invariant L (plan §16.2) as
a property test over random item sets checked against a slow oracle. `proptest` enters the
workspace as a dev-dependency here.

## Decisions

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
- [ ] **Harvest flex** → `catena/src/layout/layered/flex.rs`, overflow comment corrected and
  pinned. verify: `cargo test -p catena flex && test -z "$(git ls-files seed/ui/flex_layout.rs)"`
- [ ] **Harvest `word_wrap`** → `catena/src/raster/text.rs`, in display columns with the §18
  semantics. verify: `cargo test -p catena text`
- [ ] **Harvest tree_layout + tests** → `catena/src/layout/tree/`, header fixed, unused parameters
  removed. verify: `cargo test -p catena tree && test -z "$(git ls-files seed/graph/tree_layout.rs
  seed/graph/tree_layout_tests.rs)"`
- [ ] **Testkit fixtures** — `fixtures/community.json` moved verbatim, plus its loader.
  verify: `cargo test -p catena-testkit && test -z "$(git ls-files seed/fixtures)"`
- [ ] **Stage the SVG renderer** for its M1 port. verify: `test -f catena-testkit/src/svg.seed.rs
  && test -z "$(git ls-files seed/tests/visual)"`
- [ ] **M0 gate** — verify: `./scripts/regression.sh && cargo test -p xtask && test -z "$(git
  ls-files seed/graph/braille.rs seed/graph/braille_tests.rs seed/graph/quadtree.rs
  seed/graph/chord.rs seed/graph/chord_tests.rs seed/graph/zoom.rs seed/ui/flex_layout.rs
  seed/graph/tree_layout.rs seed/graph/tree_layout_tests.rs seed/fixtures seed/tests/visual)" &&
  test -f CONTRIBUTING.md -a -f TODO.md && grep -q Signed-off-by .github/workflows/*.yml`

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

- [ ] **Drop the harvest's dead-code attributes once callers land** — the M0 ports are wired
  to nothing yet, so their modules carry `cfg_attr(not(test), expect(dead_code))`, and `fmath`
  an `allow` (rustc 1.88 does not report it, 1.97 does). Each goes when its engine lands (M2,
  M4, M5); the `expect`s announce themselves, the `fmath` `allow` will not.
  verify: `! grep -rn "dead_code" catena/src`

## Later milestones

- [ ] **M1 — Raster + scene** — `Surface`, `CellGrid`, `SubCellCanvas`, four blitters,
  compositor with OR-merge, label mask, clipping, `SceneGraph` with both `Route` kinds, the
  §7.1 text-cell rule, the SVG renderer ported onto `CellGrid`; T2 goldens, T3 live.
  verify: `cargo test -p catena raster:: scene:: && cargo insta test && test ! -e
  catena-testkit/src/svg.seed.rs` (plus a committed `.hash` set and a test that perturbs one
  color and watches T3 fail)
- [ ] **M2 — Graph model + force layout + viewport** — per plan §19.
  verify: `cargo test -p catena` reports ≥ 200 passing tests, the stability suite and the
  cross-run hash test are green, `./scripts/check-perf.sh` exits 0 and wrote this machine's
  baseline entry, `cargo xtask lint` reports zero `lint-allow` lines for the `fmath` rule
- [ ] **M3 — Interaction + widget + demo** — per plan §19. verify: `cargo test --workspace &&
  cargo build -p catena-ratatui --bin catena-demo --features demo && cargo +nightly fuzz run
  fuzz_events -- -runs=10000`
- [ ] **M4 — Layered engine** — per plan §19. verify: `cargo test -p catena layered:: tree::`
  exits 0, goldens exist for the 12-fixture set, edge conservation passes at 512 cases
- [ ] **M5 — Polish + package** — per plan §19, including `seed/` removed and `HARVEST_COMPLETE`
  flipped in `xtask`. verify: `cargo test --workspace --all-features && cargo publish --dry-run
  -p catena -p catena-ratatui -p catena-testkit && test ! -e seed && test -z "$(git ls-files
  '*.seed.rs')"` (plus the PTY job green on linux and macos, benches within §15.1)
