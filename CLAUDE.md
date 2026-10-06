# catena

A Rust workspace (`catena`, `catena-ratatui`, `catena-testkit`) that draws interactive
node-and-edge graphs in terminal UIs: four layouts, four blitters, and live updates that keep
unchanged nodes still. It is a pure library, with no async, I/O, clock reads or domain meaning;
the host owns the event loop.

## Commands

```bash
./scripts/smoke.sh        # commit gate (pre-commit hook)
./scripts/regression.sh   # push gate (pre-push hook) and CI's check job
./scripts/check-perf.sh   # perf ratchet against this machine's bench-baseline.json entry
cargo xtask lint          # hygiene greps, file length, the build-scaffold rules
cargo insta review        # accept T2 golden changes
CATENA_UPDATE_SNAPSHOTS=1 cargo test   # re-baseline T3 hashes (each package's tests/visual/)
```

`check-perf.sh` lands at M2, insta with M1's scene step. Gate on the newest stable rustc, the
one CI's `check` runs (owner ruling); a container's `stable` can lag, so check `rustc -V`.

## Overrides

1. Work, "the repo's meshwork store": the worklist is `TODO.md`, with `## Now` (the next step,
   startable cold), `## Decisions` (each with its why), then items `- [ ] **title** — context`,
   each carrying a `verify:` command whose observed exit 0 closes it. Because sessions here run in
   the cloud without the meshwork binary (owner: "this needs to be completely standalone for the
   cloud session").
2. Hygiene, "never add license, author or copyright fields": every crate carries
   `license = "AGPL-3.0-only"` and the root carries `LICENSE`, because the license is an owner
   ruling (AGPL-3.0-only plus sold commercial exceptions, plan §3).
3. Hygiene, "pin load-bearing deps `=x.y.z`": published crates declare semver ranges
   (`ratatui-core = "0.1"`), because an exact pin in a library forces every downstream app onto
   one version. The committed `Cargo.lock` and CI's MSRV job carry reproducibility.

## Standing decisions

- `docs/design/initial-catena-plan.md` is the design authority. Its §21 is ruled, so departing
  from it is a question for the owner. `docs/design/owner-rulings.md` amends it without editing
  it, and wins where they conflict.
- Work lands on `main`: every push of a session branch is followed by fast-forwarding `main` to
  it and pushing `main`, and a merged branch is deleted, because the owner rules: "i never want
  you to leave work on your random branches."
- `seed/` is harvest input: never compiled, never edited in place. A file leaves by `git mv` to
  `<target>.seed.rs`, then a separate port commit (plan §18), because the port diff is the record
  of what changed.
- `catena` never names a ratatui type, so the core works without a terminal.
- Time enters only through `tick(dt)`, the core draws no random numbers (the testkit's generators
  use a seeded SplitMix64), and transcendental math goes through `fmath`, because determinism
  across runs and platforms is a published guarantee (plan §11).
- Outside PRs merge only with DCO sign-off and the contributor license grant, because one
  ungranted merge ends the commercial-exception arm.
- `cargo publish` runs only with `--dry-run`. Releases are the owner's act.

## Boundaries

- Never in the library: async, I/O, logging, wall-clock reads, analytics, file-format parsing,
  pixel protocols, windowing (plan §2.2).

## Pointers

- `docs/design/initial-catena-plan.md`: design, gates (§17), seed manifest (§18), milestones
  (§19), fixtures (§20), decisions (§21).
- `docs/design/owner-rulings.md`: the owner's later rulings, quoted, with what each overrides.
- `TODO.md`: the worklist.

<!-- baseline:begin — pasted from portfolio/CLAUDE-BASELINE.md; edit it there, never here -->
# Engineering baseline

## Evidence
Done means you watched it work, this session, on this tree: an exit 0, or `gh run view <id>` showing
`conclusion=success`. "Should pass", "likely fixed" and "CI in flight" are not reports. Say what you
didn't verify and whether you used `--no-verify`. A number carries what it was measured on (pin,
corpus, machine, arm; batch or incremental), and a doc's sentence about a result is not the result:
read the artifact.

## Owner intent
Quote rulings in the owner's words. What you derive from one is unratified until the owner confirms
it, so read back what you will and won't build before building. Dropping anything the design treats as
core is a question, not a decision. When the owner points at something as the model, its mechanism is
the spec.

## Work
The worklist is the repo's meshwork store, used as the meshwork skill says. File work the moment you
find it: a harness task list isn't in git and dies with the session.

## Tests
Red first: no production code without a failing test behind it. A test must be able to fail, so no
tautologies, no "didn't panic", no mocking away the layer under test, and assert counts before
equality. Check every fast path against a slow, obviously correct oracle. External input gets
adversarial fixtures. Reproduce a bug in a failing test before fixing it. A flaky test is a bug, and a
threshold is never lowered to pass.

## Gates
Commit gate (`scripts/smoke.sh`): under 60 s, scoped to the diff. Push gate (`scripts/regression.sh`):
deterministic correctness only (strict lint, build, all tests with `--include-ignored`). Benchmarks run
on demand or in CI, never in a git hook. Enable hooks with `git config core.hooksPath .githooks`.
Never re-run a gate on an unchanged tree. Gates fail closed and skip loudly.

## Performance
Criterion benches against a committed per-machine `bench-baseline.json`: median of 7+ runs after
warm-up, plus peak RSS. A regression past 1.5× needs the owner's approval. Run A/Bs in one sitting
against a control, and count rather than time where you can.

## Hygiene
Conventional Commits (`type(scope): subject`), atomic, with docs in the same commit. Commit lockfiles,
pin load-bearing deps `=x.y.z`, and keep generated data deterministic. Never add license, author or
copyright fields. Secrets come from the environment, and untrusted sizes get a ceiling. Edit files with
Edit/Write, never `sed -i` or heredocs. Cite code by symbol or `// ANCHOR: <name>`, never by line number.

## This file
Above this baseline, a repo's CLAUDE.md says only what the tree can't: what the repo is, its commands,
the owner's standing decisions (each with a one-clause why), its boundaries, pointers, and numbered
overrides of this baseline. Current state only: no dates, history or incident stories. Under 4 KB.

## Rust
Give a new workspace this setup with its first crate: `[profile.dev] debug = "line-tables-only"` and
`split-debuginfo = "unpacked"`, `[profile.dev.package."*"] debug = false`, one integration-test target
per crate (`tests/<crate>/main.rs`, with each file a `mod`), and `CARGO_TARGET_DIR` outside
`~/Documents`. Warnings fail the gate, not the build: the gate runs
`cargo clippy --all-targets -- -D warnings` and `RUSTDOCFLAGS="-D warnings" cargo doc`. Never export
`RUSTFLAGS` (that forks `target/`), and never put `-D warnings` in `.cargo/config.toml` or in source.
Lints go in `[workspace.lints]` (clippy `all` deny, `pedantic` warn), and `#[allow]` goes at the
narrowest scope with a reason. Every `unsafe` block states its invariant in `// SAFETY:`. Use
`thiserror` in libraries and `anyhow` at entry points, with no `.unwrap()` outside tests. Files target
500 lines, with a hard ceiling of 750.
<!-- baseline:end -->
