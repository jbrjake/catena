# Owner rulings that amend the plan

`initial-catena-plan.md` stays as written. Each ruling below is quoted in the owner's words; where
one conflicts with the plan, the ruling wins. `TODO.md` tracks the work each one creates and
records, under `## Decisions`, every choice a ruling leaves open.

## Toolchain

> just update to rust 1.99 so you match latest stable so you don't have conflicts with gh ci.

## Where work lands

> dude that's the work you did this session. you need to merge it onto main. i never want you to
> leave work on your random branches.

A session still develops on the branch its harness names, but that branch is a transit lane, not
a home: after every push of it, `main` is fast-forwarded to it and pushed, and once merged the
branch is deleted. If `main` has moved so that a fast-forward is impossible, the branch is rebased
onto `main` (it is the session's own branch) and the gate re-run before `main` moves. The plan
says nothing about branches, so this overrides no plan text.

## Course correction: four ideas lifted from a bundling design

> This message is the ruling. Don't edit docs/design/initial-catena-plan.md. Where this message
> conflicts with the plan, this message wins. Track each item in TODO.md with a verify: command,
> and record any choice left open here in ## Decisions with its reason. Work has already started:
> apply each item where it fits, and if the code it touches has already landed, change it in a
> follow-up commit.
>
> A1. The sampler is a tessellator.
> §18 merges the four Bézier loops into one sampler. Go one step further:
> - One function turns a curve into a polyline: quadratic, cubic, or a chain of either. Keep the
>   seed's density, steps = (chord/2).clamp(10, 200).
> - Every raster primitive (solid, dashed, hop-gapped) then works on polylines.
> - Dash by arc length along the polyline, not by sample index. This removes the defect the seed
>   notes in draw_dashed_bezier (dashes coarsen on curves under ~20 px). Don't carry that note
>   forward as a doc comment.
>
> Tests:
> - Invariant E (8-connected, exact endpoints, no duplicates) holds on tessellated polylines.
> - Dash and gap lengths, measured along the polyline, are within ±1 sub-pixel of the pattern,
>   including on curves under 20 px.
>
> A2. Chord edges are G1, not C⁰.
> Do this in its own commit, after the verbatim chord port. In generate_bundled_edges, an
> inter-group edge is two quadratics:
> - (src, b_src_comm, b_root)
> - (b_root, b_tgt_comm, tgt)
>
> They kink at b_root whenever the two controls aren't collinear with it. Make each a cubic and
> match tangents:
> - J = b_root, d = unit(b_tgt_comm − b_src_comm). If that vector is degenerate, use
>   unit(tgt − src).
> - The cubics are (src, b_src_comm, J − a·d, J) and (J, J + a·d, b_tgt_comm, tgt).
> - a is the waist knob: the control-arm length at the junction. A short arm pinches the waist; a
>   long arm loosens it. Expose it next to β, with a tuned default.
>
> Tests:
> - The tangent angle at J is below 1e-9 rad.
> - At β = 0 the chain is still collinear, and the existing limit tests stay green.
> - Replace the seed's C⁰ test; don't just delete it.
>
> Use quadratic and cubic Béziers only. No hyperbeziers: at most a later stretch goal, and nothing
> may depend on them.
>
> A3. Leave room in the scene for shared geometry.
> A later bundling feature would draw each shared segment once, so one segment would stand for
> many edges. Make that addable without a rewrite:
> - Mark Payload #[non_exhaustive].
> - Give each edge a route: either one EdgePath, or an ordered chain of segment ids.
> - A segment carries its member EdgeIx set. Hit-testing a shared segment returns that set.
> - Restate invariant B in route terms now: every input edge maps to exactly one route, and that
>   route's segments connect its source anchor to its target anchor.
> - Make the parallel-edge ×n badge (§7.3) a general CountBadge decoration that any scene item can
>   carry.
>
> A4. Bundling is never the only path to an element (radial view).
> Bundled drawings make single-element tasks harder: it's hard to tell which edge goes where.
> - Unbundle toggle: one key sets β = 0.
> - Hover-lift: hovering a node or an edge redraws its edges on EdgesOver, highlighted.
>
> Add both to the default key table and to the T2/T3 goldens.

### Where the course correction overrides the plan

| Plan text | Overridden by |
|---|---|
| §7.2, §18 (braille row): the seed's dash-phase comment "moves into the doc comment verbatim" | A1: dashes by arc length; the note is not carried |
| §9.3, §18 (chord row): "C⁰-continuous chained quadratics", "keep … the C⁰/limit tests" | A2: inter-group edges are two G1 cubics; the C⁰ test is replaced by a tangent test |
| §7.4: `Payload` and its `EdgePath{ix, route}` | A3: `Payload` is `#[non_exhaustive]`; an edge's route is one `EdgePath` or a chain of shared segments |
| §16.2-B: "every input edge yields exactly one scene path" | A3: every input edge maps to exactly one route, whose segments connect its anchors |
| §7.3: the parallel-edge "`×n` count badge at the midpoint" | A3: a general `CountBadge` decoration any scene item can carry |
| §10.1: the default key table | A4: gains the unbundle toggle; hover gains hover-lift |
