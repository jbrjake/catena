//! Centered-parent tree layout for vertical hierarchy diagrams (plan §9.3).
//! Pure math — no App, no ratatui, no Uuid dependency.
//!
//! Three passes, O(n) and deterministic: post-order subtree widths, BFS depths, pre-order x.
//! Each parent is centred over the combined span of its children's subtrees. This is naive
//! subtree-width packing, *not* Walker or Buchheim (the seed's header claimed "Walker O(n)",
//! but there are no contours or threads), so a narrow subtree beside a wide one reserves its
//! full width. Fine at terminal scale; Buchheim is a post-1.0 item.
//!
//! Coordinates are `i16` cells and saturate at its range rather than overflowing. The input
//! must be a tree: the spanning-forest guard for cycles and shared children lands at M4
//! (plan §9.3, ledger row 34).

#[derive(Debug, Clone)]
pub(crate) struct TreeNode {
    pub id: usize,
    pub width: u16,
    pub height: u16,
    pub children: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TreePosition {
    pub id: usize,
    pub x: i16,
    pub y: i16,
}

/// `v` rounded to the nearest cell, saturating at the `i16` range.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a float-to-int cast saturates, which is the intent: off-range cells clamp"
)]
fn round_to_cell(v: f64) -> i16 {
    v.round() as i16
}

/// `n` as `i16`, saturating.
fn saturating_i16(n: u16) -> i16 {
    i16::try_from(n).unwrap_or(i16::MAX)
}

/// The total horizontal gap between `n_children` side by side.
fn gaps_between(n_children: usize, h_gap: u16) -> f64 {
    let gap_count = u32::try_from(n_children.saturating_sub(1)).unwrap_or(u32::MAX);
    f64::from(gap_count) * f64::from(h_gap)
}

fn compute_subtree_width(nodes: &[TreeNode], idx: usize, h_gap: u16, widths: &mut [f64]) {
    if nodes[idx].children.is_empty() {
        widths[idx] = f64::from(nodes[idx].width);
        return;
    }
    let mut children_total = 0.0f64;
    let n_children = nodes[idx].children.len();
    for i in 0..n_children {
        let child = nodes[idx].children[i];
        compute_subtree_width(nodes, child, h_gap, widths);
        children_total += widths[child];
    }
    let gaps = gaps_between(n_children, h_gap);
    widths[idx] = (children_total + gaps).max(f64::from(nodes[idx].width));
}

fn assign_x(
    nodes: &[TreeNode],
    idx: usize,
    left_edge: f64,
    h_gap: u16,
    subtree_w: &[f64],
    positions: &mut [TreePosition],
) {
    let sw = subtree_w[idx];
    let center = left_edge + sw / 2.0;
    // Node's top-left x = center - half node width
    positions[idx].x = round_to_cell(center - f64::from(nodes[idx].width) / 2.0);

    let n_children = nodes[idx].children.len();
    if n_children == 0 {
        return;
    }

    // Children span = sum(child subtree widths) + gaps between them
    let mut children_total = 0.0f64;
    for i in 0..n_children {
        children_total += subtree_w[nodes[idx].children[i]];
    }
    let gaps = gaps_between(n_children, h_gap);
    let children_span = children_total + gaps;

    // Start placing children so their combined centre aligns with the parent centre
    let mut child_left = center - children_span / 2.0;
    for i in 0..n_children {
        let child = nodes[idx].children[i];
        assign_x(nodes, child, child_left, h_gap, subtree_w, positions);
        child_left += subtree_w[child] + f64::from(h_gap);
    }
}

pub(crate) fn tree_layout(
    nodes: &[TreeNode],
    root: usize,
    h_gap: u16,
    v_gap: u16,
) -> Vec<TreePosition> {
    let n = nodes.len();
    if n == 0 {
        return vec![];
    }

    // 1. Post-order DFS: compute subtree widths (f64 for precision)
    let mut subtree_w = vec![0.0f64; n];
    compute_subtree_width(nodes, root, h_gap, &mut subtree_w);

    // 2. BFS: compute depth of each node
    let mut depths = vec![0u16; n];
    {
        let mut queue = std::collections::VecDeque::new();
        queue.push_back((root, 0u16));
        while let Some((idx, d)) = queue.pop_front() {
            depths[idx] = d;
            for &child in &nodes[idx].children {
                queue.push_back((child, d.saturating_add(1)));
            }
        }
    }

    // 3. Compute max height at each depth → y offset per depth
    let max_depth = usize::from(depths.iter().copied().max().unwrap_or(0));
    let mut max_h = vec![0u16; max_depth + 1];
    for (i, &d) in depths.iter().enumerate() {
        let d = usize::from(d);
        max_h[d] = max_h[d].max(nodes[i].height);
    }
    // Saturating: a tall tree clamps its deepest rows at i16::MAX instead of overflowing
    // (a panic in debug builds) or wrapping a child above its parent (in release).
    let mut y_at_depth = vec![0i16; max_depth + 1];
    for d in 1..=max_depth {
        y_at_depth[d] = y_at_depth[d - 1]
            .saturating_add(saturating_i16(max_h[d - 1]))
            .saturating_add(saturating_i16(v_gap));
    }

    // 4. Pre-order DFS: assign x coordinates
    let mut positions = vec![TreePosition { id: 0, x: 0, y: 0 }; n];
    assign_x(nodes, root, 0.0, h_gap, &subtree_w, &mut positions);

    // 5. Apply y from depth table and node IDs
    for i in 0..n {
        positions[i].id = nodes[i].id;
        positions[i].y = y_at_depth[usize::from(depths[i])];
    }

    // 6. Normalise: shift so minimum x == 0
    let min_x = positions.iter().map(|p| p.x).min().unwrap_or(0);
    if min_x != 0 {
        for p in &mut positions {
            p.x = p.x.saturating_sub(min_x);
        }
    }

    positions
}

/// Orthogonal connector points for every parent: a stem down from its bottom centre to a bus
/// row, the bus across to the outermost children, and a drop to each child's top centre.
/// `result[i]` is empty for a leaf; otherwise it holds points in pairs, each pair one
/// axis-aligned segment: stem `[0..2]`, bus `[2..4]`, then one drop per child. `offset` shifts
/// every point.
pub(crate) fn route_connectors(
    nodes: &[TreeNode],
    positions: &[TreePosition],
    offset: (i16, i16),
) -> Vec<Vec<(i32, i32)>> {
    let mut result: Vec<Vec<(i32, i32)>> = vec![vec![]; nodes.len()];
    let ox = i32::from(offset.0);
    let oy = i32::from(offset.1);
    let center_x = |i: usize| i32::from(positions[i].x) + i32::from(nodes[i].width) / 2 + ox;

    for (i, node) in nodes.iter().enumerate() {
        let (Some(&leftmost_child), Some(&rightmost_child)) =
            (node.children.first(), node.children.last())
        else {
            continue;
        };

        let parent_cx = center_x(i);
        let parent_bottom = i32::from(positions[i].y) + i32::from(node.height) + oy;
        let bus_y = parent_bottom + 1;

        let bus_left = center_x(leftmost_child);
        let bus_right = center_x(rightmost_child);

        // Stem (0..1) + Bus (2..3) fixed segments; drops appended below.
        let mut segs: Vec<(i32, i32)> = vec![
            (parent_cx, parent_bottom), // stem start
            (parent_cx, bus_y),         // stem end
            (bus_left, bus_y),          // bus left
            (bus_right, bus_y),         // bus right
        ];

        // Drops: one per child — vertical from bus_y to child top-center
        for &child in &node.children {
            let child_cx = center_x(child);
            let child_top = i32::from(positions[child].y) + oy;
            segs.push((child_cx, bus_y));
            segs.push((child_cx, child_top));
        }

        result[i] = segs;
    }

    result
}

#[cfg(test)]
#[path = "tree_layout_tests.rs"]
mod tests;
