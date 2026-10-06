use super::*;

fn node(id: usize, width: u16, height: u16, children: &[usize]) -> TreeNode {
    TreeNode {
        id,
        width,
        height,
        children: children.to_vec(),
    }
}

fn simple_tree() -> Vec<TreeNode> {
    // root(0) → left(1), right(2)
    vec![
        node(0, 10, 3, &[1, 2]),
        node(1, 10, 3, &[]),
        node(2, 10, 3, &[]),
    ]
}

/// A chain root → 1 → 2 → … with the given node heights, every node 4 wide.
fn chain(heights: &[u16]) -> Vec<TreeNode> {
    (0..heights.len())
        .map(|i| {
            let children: &[usize] = if i + 1 < heights.len() { &[i + 1] } else { &[] };
            node(i, 4, heights[i], children)
        })
        .collect()
}

/// Helper: `center_x = pos.x + node.width / 2` (integer midpoint)
fn center_x(pos: &TreePosition, node: &TreeNode) -> i32 {
    i32::from(pos.x) + i32::from(node.width) / 2
}

/// Right edge of a node: x + width.
fn right_edge(pos: &TreePosition, node: &TreeNode) -> i32 {
    i32::from(pos.x) + i32::from(node.width)
}

#[test]
fn tree_single_node_at_origin() {
    let nodes = vec![node(42, 8, 2, &[])];
    let pos = tree_layout(&nodes, 0, 2, 1);
    assert_eq!(pos.len(), 1);
    assert_eq!(pos[0].id, 42);
    assert_eq!(pos[0].x, 0);
    assert_eq!(pos[0].y, 0);
}

#[test]
fn tree_centering_invariant() {
    // Uses equal-width nodes so that center-of-span (what the algorithm computes)
    // and center-of-child-centers (the simplest invariant to state) coincide.
    // For asymmetric trees, the algorithm guarantees the parent center equals the
    // midpoint of the children's combined bounding span.
    let nodes = simple_tree();
    let pos = tree_layout(&nodes, 0, 2, 1);

    let parent_cx = center_x(&pos[0], &nodes[0]);
    let left_cx = center_x(&pos[1], &nodes[1]);
    let right_cx = center_x(&pos[2], &nodes[2]);
    let expected_midpoint = i32::midpoint(left_cx, right_cx);

    assert_eq!(
        parent_cx, expected_midpoint,
        "parent center {parent_cx} != midpoint of children ({left_cx}+{right_cx})/2 = {expected_midpoint}"
    );
}

#[test]
fn tree_sibling_order_preserved() {
    // 5-child root; verify x-coords increase left-to-right
    let mut nodes = vec![node(0, 6, 2, &[1, 2, 3, 4, 5])];
    for i in 1..=5usize {
        nodes.push(node(i, 8, 2, &[]));
    }
    let pos = tree_layout(&nodes, 0, 2, 1);

    let xs: Vec<i16> = nodes[0].children.iter().map(|&c| pos[c].x).collect();
    assert_eq!(xs.len(), 5);
    for w in xs.windows(2) {
        assert!(
            w[0] < w[1],
            "child x-coords not strictly increasing: {xs:?}"
        );
    }
}

#[test]
fn tree_depth_y_uniformity() {
    // 3-level tree: root→2 children, each child→2 grandchildren
    let nodes = vec![
        node(0, 8, 2, &[1, 2]),
        node(1, 8, 2, &[3, 4]),
        node(2, 8, 2, &[5, 6]),
        node(3, 8, 2, &[]),
        node(4, 8, 2, &[]),
        node(5, 8, 2, &[]),
        node(6, 8, 2, &[]),
    ];
    let pos = tree_layout(&nodes, 0, 2, 1);

    // depth 0: node 0
    let y0 = pos[0].y;
    // depth 1: nodes 1, 2
    let y1a = pos[1].y;
    let y1b = pos[2].y;
    // depth 2: nodes 3..6
    let y2s: Vec<i16> = (3..=6).map(|i| pos[i].y).collect();

    assert_eq!(y1a, y1b, "depth-1 nodes must have same y");
    assert!(
        y2s.iter().all(|&y| y == y2s[0]),
        "depth-2 nodes must have same y: {y2s:?}"
    );
    assert!(y0 < y1a, "root must be above depth-1");
    assert!(y1a < y2s[0], "depth-1 must be above depth-2");
}

#[test]
fn tree_no_overlap_wide_nodes() {
    // 4 siblings with wide labels; bounding boxes must not overlap
    let nodes = vec![
        node(0, 6, 2, &[1, 2, 3, 4]),
        node(1, 20, 2, &[]),
        node(2, 20, 2, &[]),
        node(3, 20, 2, &[]),
        node(4, 20, 2, &[]),
    ];
    let pos = tree_layout(&nodes, 0, 2, 1);

    // Check all pairs of siblings at depth 1
    let children = &nodes[0].children;
    for i in 0..children.len() {
        for j in (i + 1)..children.len() {
            let (ci, cj) = (children[i], children[j]);
            let right_of_i = right_edge(&pos[ci], &nodes[ci]);
            assert!(
                right_of_i <= i32::from(pos[cj].x),
                "nodes {ci} and {cj} overlap: right_of_{ci}={right_of_i} > left_of_{cj}={}",
                pos[cj].x
            );
        }
    }
}

#[test]
fn tree_deep_chain_spacing() {
    // 10-node straight chain: root→1→2→…→9
    let nodes = chain(&[2; 10]);
    let pos = tree_layout(&nodes, 0, 2, 1);

    // Each level y must be strictly greater than the previous
    for i in 1..10usize {
        assert!(
            pos[i].y > pos[i - 1].y,
            "depth-{i} y={} must exceed depth-{} y={}",
            pos[i].y,
            i - 1,
            pos[i - 1].y
        );
    }
}

#[test]
fn tree_deterministic() {
    let nodes = simple_tree();
    let pos1 = tree_layout(&nodes, 0, 2, 1);
    let pos2 = tree_layout(&nodes, 0, 2, 1);
    assert_eq!(pos1, pos2, "tree_layout must be deterministic");
}

#[test]
fn tree_large_asymmetric_no_overlap() {
    // root(0) with left chain 0→1→2→3→4→5 and right subtree 10 with 5 wide children
    let mut nodes = vec![node(0, 8, 2, &[1, 10])];
    for i in 1..=4 {
        nodes.push(node(i, 8, 2, &[i + 1]));
    }
    nodes.push(node(5, 8, 2, &[]));
    for i in 6..=9 {
        nodes.push(node(i, 8, 2, &[])); // unreachable, harmless
    }
    nodes.push(node(10, 12, 3, &[11, 12, 13, 14, 15]));
    for (i, width) in [(11, 6), (12, 14), (13, 8), (14, 10), (15, 6)] {
        nodes.push(node(i, width, 2, &[]));
    }
    let pos = tree_layout(&nodes, 0, 2, 1);

    // Verify siblings 11..15 (children of node 10) don't overlap
    let siblings = [11usize, 12, 13, 14, 15];
    for i in 0..siblings.len() {
        for j in (i + 1)..siblings.len() {
            let (ci, cj) = (siblings[i], siblings[j]);
            let right_of_i = right_edge(&pos[ci], &nodes[ci]);
            assert!(
                right_of_i <= i32::from(pos[cj].x),
                "nodes {ci} and {cj} overlap: right_of_{ci}={right_of_i} > left_of_{cj}={}",
                pos[cj].x
            );
        }
    }
}

#[test]
fn tree_connector_from_parent_bottom_center() {
    // simple_tree: root(0)→left(1), right(2), all width=10 height=3
    let nodes = simple_tree();
    let pos = tree_layout(&nodes, 0, 2, 1);
    let connectors = route_connectors(&nodes, &pos, (0, 0));

    // Parent (node 0) should have non-empty connector segments
    assert!(
        !connectors[0].is_empty(),
        "parent must have connector segments"
    );

    // Stem start = parent bottom-center
    let parent_cx = center_x(&pos[0], &nodes[0]);
    let parent_bottom = i32::from(pos[0].y) + i32::from(nodes[0].height);
    assert_eq!(
        connectors[0][0],
        (parent_cx, parent_bottom),
        "connector must start at parent bottom-center ({parent_cx}, {parent_bottom})"
    );
}

#[test]
fn tree_connector_to_child_top_center() {
    let nodes = simple_tree();
    let pos = tree_layout(&nodes, 0, 2, 1);
    let connectors = route_connectors(&nodes, &pos, (0, 0));

    // For each child of node 0, verify the drop segment ends at child top-center.
    // Encoding: indices 0..1 = stem, 2..3 = bus, then pairs for each drop.
    // Drop for child[0] is at indices 4..5, child[1] at 6..7.
    for (drop_idx, &child) in nodes[0].children.iter().enumerate() {
        let seg_end_idx = 5 + drop_idx * 2;
        assert!(
            connectors[0].len() > seg_end_idx,
            "connector for parent 0 must have segment for child {child}"
        );
        let drop_end = connectors[0][seg_end_idx];
        let child_cx = center_x(&pos[child], &nodes[child]);
        let child_top = i32::from(pos[child].y);
        assert_eq!(
            drop_end,
            (child_cx, child_top),
            "drop for child {child} must end at ({child_cx}, {child_top})"
        );
    }
}

#[test]
fn tree_connector_is_orthogonal() {
    // All connector segment endpoint pairs must be axis-aligned (dx=0 OR dy=0)
    let nodes = simple_tree();
    let pos = tree_layout(&nodes, 0, 2, 1);
    let connectors = route_connectors(&nodes, &pos, (0, 0));

    assert_eq!(connectors[0].len(), 8, "stem, bus and two drops");
    for (node_idx, segs) in connectors.iter().enumerate() {
        for chunk in segs.chunks(2) {
            if let [a, b] = chunk {
                assert!(
                    a.0 == b.0 || a.1 == b.1,
                    "diagonal segment in connectors[{node_idx}]: {a:?} → {b:?}"
                );
            }
        }
    }
}

#[test]
fn tree_connector_offset_shifts_every_point() {
    let nodes = simple_tree();
    let pos = tree_layout(&nodes, 0, 2, 1);
    let plain = route_connectors(&nodes, &pos, (0, 0));
    let shifted = route_connectors(&nodes, &pos, (3, -2));
    assert_eq!(plain.len(), shifted.len());
    for (p, s) in plain.iter().zip(&shifted) {
        assert_eq!(p.len(), s.len());
        for (&(px, py), &(sx, sy)) in p.iter().zip(s) {
            assert_eq!((sx, sy), (px + 3, py - 2));
        }
    }
}

// ── Port regressions: i16 arithmetic saturates (plan §4.3 totality) ────────

#[test]
fn a_tree_taller_than_i16_clamps_its_deepest_rows() {
    // 30000 + 1 + 30000 rows overflowed the seed's i16 sum: a panic in debug builds.
    let pos = tree_layout(&chain(&[30_000, 30_000, 2]), 0, 2, 1);
    let ys: Vec<i16> = pos.iter().map(|p| p.y).collect();
    assert_eq!(ys, [0, 30_001, i16::MAX]);
}

#[test]
fn a_node_taller_than_i16_never_puts_its_child_above_it() {
    // The seed cast a 40000-row height to i16 (-25536) and placed the child at y = -25535.
    let pos = tree_layout(&chain(&[40_000, 2]), 0, 2, 1);
    let ys: Vec<i16> = pos.iter().map(|p| p.y).collect();
    assert_eq!(ys, [0, i16::MAX]);
}
