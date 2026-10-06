use super::*;

fn simple_tree() -> Vec<TreeNode> {
    // root(0) → left(1), right(2)
    vec![
        TreeNode {
            id: 0,
            width: 10,
            height: 3,
            children: vec![1, 2],
        },
        TreeNode {
            id: 1,
            width: 10,
            height: 3,
            children: vec![],
        },
        TreeNode {
            id: 2,
            width: 10,
            height: 3,
            children: vec![],
        },
    ]
}

/// Helper: node center_x = pos.x + node.width / 2  (integer midpoint)
fn center_x(pos: &TreePosition, node: &TreeNode) -> i32 {
    pos.x as i32 + node.width as i32 / 2
}

#[test]
fn tree_single_node_at_origin() {
    let nodes = vec![TreeNode {
        id: 42,
        width: 8,
        height: 2,
        children: vec![],
    }];
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
    let expected_midpoint = (left_cx + right_cx) / 2;

    assert_eq!(
        parent_cx, expected_midpoint,
        "parent center {parent_cx} != midpoint of children ({left_cx}+{right_cx})/2 = {expected_midpoint}"
    );
}

#[test]
fn tree_sibling_order_preserved() {
    // 5-child root; verify x-coords increase left-to-right
    let mut nodes = vec![TreeNode {
        id: 0,
        width: 6,
        height: 2,
        children: vec![1, 2, 3, 4, 5],
    }];
    for i in 1..=5usize {
        nodes.push(TreeNode {
            id: i,
            width: 8,
            height: 2,
            children: vec![],
        });
    }
    let pos = tree_layout(&nodes, 0, 2, 1);

    let xs: Vec<i16> = nodes[0].children.iter().map(|&c| pos[c].x).collect();
    for w in xs.windows(2) {
        assert!(
            w[0] < w[1],
            "child x-coords not strictly increasing: {:?}",
            xs
        );
    }
}

#[test]
fn tree_depth_y_uniformity() {
    // 3-level tree: root→2 children, each child→2 grandchildren
    let nodes = vec![
        TreeNode {
            id: 0,
            width: 8,
            height: 2,
            children: vec![1, 2],
        },
        TreeNode {
            id: 1,
            width: 8,
            height: 2,
            children: vec![3, 4],
        },
        TreeNode {
            id: 2,
            width: 8,
            height: 2,
            children: vec![5, 6],
        },
        TreeNode {
            id: 3,
            width: 8,
            height: 2,
            children: vec![],
        },
        TreeNode {
            id: 4,
            width: 8,
            height: 2,
            children: vec![],
        },
        TreeNode {
            id: 5,
            width: 8,
            height: 2,
            children: vec![],
        },
        TreeNode {
            id: 6,
            width: 8,
            height: 2,
            children: vec![],
        },
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
        "depth-2 nodes must have same y: {:?}",
        y2s
    );
    assert!(y0 < y1a, "root must be above depth-1");
    assert!(y1a < y2s[0], "depth-1 must be above depth-2");
}

#[test]
fn tree_no_overlap_wide_nodes() {
    // 4 siblings with wide labels; bounding boxes must not overlap
    let nodes = vec![
        TreeNode {
            id: 0,
            width: 6,
            height: 2,
            children: vec![1, 2, 3, 4],
        },
        TreeNode {
            id: 1,
            width: 20,
            height: 2,
            children: vec![],
        },
        TreeNode {
            id: 2,
            width: 20,
            height: 2,
            children: vec![],
        },
        TreeNode {
            id: 3,
            width: 20,
            height: 2,
            children: vec![],
        },
        TreeNode {
            id: 4,
            width: 20,
            height: 2,
            children: vec![],
        },
    ];
    let pos = tree_layout(&nodes, 0, 2, 1);

    // Check all pairs of siblings at depth 1
    let children = &nodes[0].children;
    for i in 0..children.len() {
        for j in (i + 1)..children.len() {
            let ci = children[i];
            let cj = children[j];
            let pi = &pos[ci];
            let pj = &pos[cj];
            let ni = &nodes[ci];
            let right_of_i = pi.x + ni.width as i16;
            assert!(
                right_of_i <= pj.x,
                "nodes {ci} and {cj} overlap: right_of_{ci}={right_of_i} > left_of_{cj}={}",
                pj.x
            );
        }
    }
}

#[test]
fn tree_deep_chain_spacing() {
    // 10-node straight chain: root→1→2→…→9
    let nodes: Vec<TreeNode> = (0..10)
        .map(|i| TreeNode {
            id: i,
            width: 6,
            height: 2,
            children: if i < 9 { vec![i + 1] } else { vec![] },
        })
        .collect();
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
    let nodes: Vec<TreeNode> = vec![
        TreeNode {
            id: 0,
            width: 10,
            height: 3,
            children: vec![1, 2],
        },
        TreeNode {
            id: 1,
            width: 10,
            height: 3,
            children: vec![],
        },
        TreeNode {
            id: 2,
            width: 10,
            height: 3,
            children: vec![],
        },
    ];
    let pos1 = tree_layout(&nodes, 0, 2, 1);
    let pos2 = tree_layout(&nodes, 0, 2, 1);
    assert_eq!(pos1, pos2, "tree_layout must be deterministic");
}

#[test]
fn tree_large_asymmetric_no_overlap() {
    // root(0) with left chain 0→1→2→3→4→5 and right subtree 10 with 5 wide children
    let nodes: Vec<TreeNode> = vec![
        TreeNode {
            id: 0,
            width: 8,
            height: 2,
            children: vec![1, 10],
        },
        TreeNode {
            id: 1,
            width: 8,
            height: 2,
            children: vec![2],
        },
        TreeNode {
            id: 2,
            width: 8,
            height: 2,
            children: vec![3],
        },
        TreeNode {
            id: 3,
            width: 8,
            height: 2,
            children: vec![4],
        },
        TreeNode {
            id: 4,
            width: 8,
            height: 2,
            children: vec![5],
        },
        TreeNode {
            id: 5,
            width: 8,
            height: 2,
            children: vec![],
        },
        TreeNode {
            id: 6,
            width: 8,
            height: 2,
            children: vec![],
        }, // unreachable, harmless
        TreeNode {
            id: 7,
            width: 8,
            height: 2,
            children: vec![],
        },
        TreeNode {
            id: 8,
            width: 8,
            height: 2,
            children: vec![],
        },
        TreeNode {
            id: 9,
            width: 8,
            height: 2,
            children: vec![],
        },
        TreeNode {
            id: 10,
            width: 12,
            height: 3,
            children: vec![11, 12, 13, 14, 15],
        },
        TreeNode {
            id: 11,
            width: 6,
            height: 2,
            children: vec![],
        },
        TreeNode {
            id: 12,
            width: 14,
            height: 2,
            children: vec![],
        },
        TreeNode {
            id: 13,
            width: 8,
            height: 2,
            children: vec![],
        },
        TreeNode {
            id: 14,
            width: 10,
            height: 2,
            children: vec![],
        },
        TreeNode {
            id: 15,
            width: 6,
            height: 2,
            children: vec![],
        },
    ];
    let pos = tree_layout(&nodes, 0, 2, 1);

    // Verify siblings 11..15 (children of node 10) don't overlap
    let siblings = [11usize, 12, 13, 14, 15];
    for i in 0..siblings.len() {
        for j in (i + 1)..siblings.len() {
            let ci = siblings[i];
            let cj = siblings[j];
            let right_of_i = pos[ci].x + nodes[ci].width as i16;
            assert!(
                right_of_i <= pos[cj].x,
                "nodes {ci} and {cj} overlap: right_of_{ci}={right_of_i} > left_of_{cj}={}",
                pos[cj].x
            );
        }
    }
}

#[test]
fn tree_connector_from_parent_bottom_center() {
    // simple_tree: root(0)→left(1), right(2), all width=10 height=3
    let nodes: Vec<TreeNode> = vec![
        TreeNode {
            id: 0,
            width: 10,
            height: 3,
            children: vec![1, 2],
        },
        TreeNode {
            id: 1,
            width: 10,
            height: 3,
            children: vec![],
        },
        TreeNode {
            id: 2,
            width: 10,
            height: 3,
            children: vec![],
        },
    ];
    let pos = tree_layout(&nodes, 0, 2, 1);
    let connectors = route_connectors(&nodes, &pos, 0, 2, (0, 0));

    // Parent (node 0) should have non-empty connector segments
    assert!(
        !connectors[0].is_empty(),
        "parent must have connector segments"
    );

    // Stem start = parent bottom-center
    let parent_cx = pos[0].x as i32 + nodes[0].width as i32 / 2;
    let parent_bottom = pos[0].y as i32 + nodes[0].height as i32;
    assert_eq!(
        connectors[0][0],
        (parent_cx, parent_bottom),
        "connector must start at parent bottom-center ({parent_cx}, {parent_bottom})"
    );
}

#[test]
fn tree_connector_to_child_top_center() {
    let nodes: Vec<TreeNode> = vec![
        TreeNode {
            id: 0,
            width: 10,
            height: 3,
            children: vec![1, 2],
        },
        TreeNode {
            id: 1,
            width: 10,
            height: 3,
            children: vec![],
        },
        TreeNode {
            id: 2,
            width: 10,
            height: 3,
            children: vec![],
        },
    ];
    let pos = tree_layout(&nodes, 0, 2, 1);
    let connectors = route_connectors(&nodes, &pos, 0, 2, (0, 0));

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
        let child_cx = pos[child].x as i32 + nodes[child].width as i32 / 2;
        let child_top = pos[child].y as i32;
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
    let nodes: Vec<TreeNode> = vec![
        TreeNode {
            id: 0,
            width: 10,
            height: 3,
            children: vec![1, 2],
        },
        TreeNode {
            id: 1,
            width: 10,
            height: 3,
            children: vec![],
        },
        TreeNode {
            id: 2,
            width: 10,
            height: 3,
            children: vec![],
        },
    ];
    let pos = tree_layout(&nodes, 0, 2, 1);
    let connectors = route_connectors(&nodes, &pos, 0, 2, (0, 0));

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
