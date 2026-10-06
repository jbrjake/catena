//! Walker O(n) tree layout algorithm for vertical hierarchy diagrams.
//! Pure math — no App, no ratatui, no Uuid dependency.

#[derive(Debug, Clone)]
pub struct TreeNode {
    pub id: usize,
    pub width: u16,
    pub height: u16,
    pub children: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreePosition {
    pub id: usize,
    pub x: i16,
    pub y: i16,
}

fn compute_subtree_width(nodes: &[TreeNode], idx: usize, h_gap: u16, widths: &mut [f64]) {
    if nodes[idx].children.is_empty() {
        widths[idx] = nodes[idx].width as f64;
        return;
    }
    let mut children_total = 0.0f64;
    let n_children = nodes[idx].children.len();
    for i in 0..n_children {
        let child = nodes[idx].children[i];
        compute_subtree_width(nodes, child, h_gap, widths);
        children_total += widths[child];
    }
    let gaps = (n_children - 1) as f64 * h_gap as f64;
    widths[idx] = (children_total + gaps).max(nodes[idx].width as f64);
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
    positions[idx].x = (center - nodes[idx].width as f64 / 2.0).round() as i16;

    let n_children = nodes[idx].children.len();
    if n_children == 0 {
        return;
    }

    // Children span = sum(child subtree widths) + gaps between them
    let mut children_total = 0.0f64;
    for i in 0..n_children {
        children_total += subtree_w[nodes[idx].children[i]];
    }
    let gaps = (n_children - 1) as f64 * h_gap as f64;
    let children_span = children_total + gaps;

    // Start placing children so their combined centre aligns with the parent centre
    let mut child_left = center - children_span / 2.0;
    for i in 0..n_children {
        let child = nodes[idx].children[i];
        assign_x(nodes, child, child_left, h_gap, subtree_w, positions);
        child_left += subtree_w[child] + h_gap as f64;
    }
}

pub fn tree_layout(nodes: &[TreeNode], root: usize, h_gap: u16, v_gap: u16) -> Vec<TreePosition> {
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
                queue.push_back((child, d + 1));
            }
        }
    }

    // 3. Compute max height at each depth → y offset per depth
    let max_depth = depths.iter().copied().max().unwrap_or(0) as usize;
    let mut max_h = vec![0u16; max_depth + 1];
    for (i, &d) in depths.iter().enumerate() {
        max_h[d as usize] = max_h[d as usize].max(nodes[i].height);
    }
    let mut y_at_depth = vec![0i16; max_depth + 1];
    for d in 1..=max_depth {
        y_at_depth[d] = y_at_depth[d - 1] + max_h[d - 1] as i16 + v_gap as i16;
    }

    // 4. Pre-order DFS: assign x coordinates
    let mut positions = vec![TreePosition { id: 0, x: 0, y: 0 }; n];
    assign_x(nodes, root, 0.0, h_gap, &subtree_w, &mut positions);

    // 5. Apply y from depth table and node IDs
    for i in 0..n {
        positions[i].id = nodes[i].id;
        positions[i].y = y_at_depth[depths[i] as usize];
    }

    // 6. Normalise: shift so minimum x == 0
    let min_x = positions.iter().map(|p| p.x).min().unwrap_or(0);
    if min_x != 0 {
        for p in &mut positions {
            p.x -= min_x;
        }
    }

    positions
}

pub fn route_connectors(
    nodes: &[TreeNode],
    positions: &[TreePosition],
    _root: usize,
    _h_gap: u16,
    offset: (i16, i16),
) -> Vec<Vec<(i32, i32)>> {
    let mut result: Vec<Vec<(i32, i32)>> = vec![vec![]; nodes.len()];

    for (i, node) in nodes.iter().enumerate() {
        if node.children.is_empty() {
            continue;
        }

        let pos = &positions[i];
        let ox = offset.0 as i32;
        let oy = offset.1 as i32;

        let parent_cx = pos.x as i32 + node.width as i32 / 2 + ox;
        let parent_bottom = pos.y as i32 + node.height as i32 + oy;
        let bus_y = parent_bottom + 1;

        let leftmost_child = node.children[0];
        let rightmost_child = *node
            .children
            .last()
            .expect("children is non-empty (guarded by caller)");
        let bus_left =
            positions[leftmost_child].x as i32 + nodes[leftmost_child].width as i32 / 2 + ox;
        let bus_right =
            positions[rightmost_child].x as i32 + nodes[rightmost_child].width as i32 / 2 + ox;

        // Stem (0..1) + Bus (2..3) fixed segments; drops appended below.
        let mut segs: Vec<(i32, i32)> = vec![
            (parent_cx, parent_bottom), // stem start
            (parent_cx, bus_y),         // stem end
            (bus_left, bus_y),          // bus left
            (bus_right, bus_y),         // bus right
        ];

        // Drops: one per child — vertical from bus_y to child top-center
        for &child in &node.children {
            let child_cx = positions[child].x as i32 + nodes[child].width as i32 / 2 + ox;
            let child_top = positions[child].y as i32 + oy;
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
