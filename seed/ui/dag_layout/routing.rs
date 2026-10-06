//! Phase 4: Edge routing — merge bars, fork bars, vertical pipes, crossing detection.

#![allow(dead_code)]

use super::models::*;

/// Build a horizontal merge bar: multiple parents converge to one child.
///
/// Parents are sorted by column. The bar runs from leftmost to rightmost parent.
/// Junctions: └ at leftmost, ┘ at rightmost, ┴ at interior parents, ┬ at child center.
pub(super) fn build_merge_bar(
    parent_centers: &[(usize, usize)], // (node_id, center_col)
    child_center: usize,
    bar_row: usize,
    color: EdgeColor,
) -> Vec<EdgeSegment> {
    let mut segments: Vec<EdgeSegment> = Vec::new();

    if parent_centers.is_empty() || parent_centers.len() == 1 {
        return segments;
    }

    let mut sorted: Vec<(usize, usize)> = parent_centers.to_vec();
    sorted.sort_by_key(|&(_, col)| col);

    let leftmost = sorted.first().expect("sorted non-empty after guard").1;
    let rightmost = sorted.last().expect("sorted non-empty after guard").1;

    segments.push(EdgeSegment::HBar {
        row: bar_row,
        col_start: leftmost,
        col_end: rightmost,
        color,
    });

    for (i, &(_, col)) in sorted.iter().enumerate() {
        let is_left = i == 0;
        let is_right = i == sorted.len() - 1;
        let is_child = col == child_center;

        let kind = if is_child {
            JunctionKind::Cross
        } else if is_left {
            JunctionKind::CornerUR // └
        } else if is_right {
            JunctionKind::CornerUL // ┘
        } else {
            JunctionKind::TeeUp // ┴
        };

        segments.push(EdgeSegment::Junction {
            col,
            row: bar_row,
            kind,
            color,
        });
    }

    let child_is_parent = sorted.iter().any(|&(_, col)| col == child_center);
    if !child_is_parent {
        segments.push(EdgeSegment::Junction {
            col: child_center,
            row: bar_row,
            kind: JunctionKind::TeeDown,
            color,
        });
    }

    segments
}

/// Build a horizontal fork bar: one parent splits to multiple children.
pub(super) fn build_fork_bar(
    parent_center: usize,
    child_centers: &[(usize, usize)], // (node_id, center_col)
    bar_row: usize,
    color: EdgeColor,
) -> Vec<EdgeSegment> {
    let mut segments: Vec<EdgeSegment> = Vec::new();

    if child_centers.len() <= 1 {
        return segments;
    }

    let mut sorted: Vec<(usize, usize)> = child_centers.to_vec();
    sorted.sort_by_key(|&(_, col)| col);

    let leftmost = sorted.first().expect("sorted non-empty after guard").1;
    let rightmost = sorted.last().expect("sorted non-empty after guard").1;

    segments.push(EdgeSegment::HBar {
        row: bar_row,
        col_start: leftmost,
        col_end: rightmost,
        color,
    });

    let parent_is_child = sorted.iter().any(|&(_, col)| col == parent_center);
    if !parent_is_child && parent_center >= leftmost && parent_center <= rightmost {
        segments.push(EdgeSegment::Junction {
            col: parent_center,
            row: bar_row,
            kind: JunctionKind::TeeUp,
            color,
        });
    }

    for (i, &(_, col)) in sorted.iter().enumerate() {
        let is_left = i == 0;
        let is_right = i == sorted.len() - 1;
        let is_parent = col == parent_center;

        let kind = if is_parent {
            JunctionKind::Cross
        } else if is_left {
            JunctionKind::CornerDR // ┌
        } else if is_right {
            JunctionKind::CornerDL // ┐
        } else {
            JunctionKind::TeeDown // ┬
        };

        segments.push(EdgeSegment::Junction {
            col,
            row: bar_row,
            kind,
            color,
        });
    }

    segments
}

/// Like build_merge_bar, but one parent may continue downward (fork+merge).
/// `continues_down` is the node_id of the parent whose pipe continues below the bar.
pub(super) fn build_merge_bar_with_continuation(
    parent_centers: &[(usize, usize)],
    child_center: usize,
    bar_row: usize,
    color: EdgeColor,
    continues_down: Option<usize>,
) -> Vec<EdgeSegment> {
    let mut segments: Vec<EdgeSegment> = Vec::new();

    if parent_centers.len() <= 1 {
        return segments;
    }

    let mut sorted: Vec<(usize, usize)> = parent_centers.to_vec();
    sorted.sort_by_key(|&(_, col)| col);

    let leftmost = sorted.first().expect("sorted non-empty after guard").1;
    let rightmost = sorted.last().expect("sorted non-empty after guard").1;

    segments.push(EdgeSegment::HBar {
        row: bar_row,
        col_start: leftmost,
        col_end: rightmost,
        color,
    });

    for (i, &(nid, col)) in sorted.iter().enumerate() {
        let is_left = i == 0;
        let is_right = i == sorted.len() - 1;
        let continues = continues_down == Some(nid);

        let kind = if continues && is_left {
            JunctionKind::TeeRight // ├
        } else if continues && is_right {
            JunctionKind::TeeLeft // ┤
        } else if continues {
            JunctionKind::Cross // ┼
        } else if is_left {
            JunctionKind::CornerUR // └
        } else if is_right {
            JunctionKind::CornerUL // ┘
        } else {
            JunctionKind::TeeUp // ┴
        };

        segments.push(EdgeSegment::Junction {
            col,
            row: bar_row,
            kind,
            color,
        });
    }

    let child_is_parent = sorted.iter().any(|&(_, col)| col == child_center);
    if !child_is_parent {
        segments.push(EdgeSegment::Junction {
            col: child_center,
            row: bar_row,
            kind: JunctionKind::TeeDown,
            color,
        });
    }

    segments
}

// ── Phase 4b: Vertical pipes and crossing detection ─────────────────────────

/// Build vertical pipe segments from box bottoms down to a target row.
pub(super) fn build_vertical_pipes(
    sources: &[(usize, usize, usize)], // (node_id, center_col, bottom_row)
    target_row: usize,
) -> Vec<EdgeSegment> {
    let mut segments = Vec::new();
    for &(_nid, col, bot) in sources {
        let start = bot + 1;
        if start < target_row {
            segments.push(EdgeSegment::VPipe {
                col,
                row_start: start,
                row_end: target_row.saturating_sub(1),
                color: EdgeColor::Neutral,
            });
        }
    }
    segments
}

/// Find points where vertical pipes cross horizontal bars.
/// Returns Cross junctions at each intersection.
pub(super) fn detect_crossings(hbars: &[EdgeSegment], vpipes: &[EdgeSegment]) -> Vec<EdgeSegment> {
    let mut crossings = Vec::new();
    for hbar in hbars {
        if let EdgeSegment::HBar {
            row,
            col_start,
            col_end,
            ..
        } = hbar
        {
            for vpipe in vpipes {
                if let EdgeSegment::VPipe {
                    col,
                    row_start,
                    row_end,
                    color: vcolor,
                } = vpipe
                {
                    if *col > *col_start
                        && *col < *col_end
                        && *row >= *row_start
                        && *row <= *row_end
                    {
                        crossings.push(EdgeSegment::Junction {
                            col: *col,
                            row: *row,
                            kind: JunctionKind::Cross,
                            color: *vcolor,
                        });
                    }
                }
            }
        }
    }
    crossings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_merge_bar_two_parents() {
        let parent_centers = vec![(0, 10), (1, 40)];
        let child_center = 25;
        let bar_row = 5;

        let segments = build_merge_bar(&parent_centers, child_center, bar_row, EdgeColor::Neutral);

        let has_corner_ur = segments.iter().any(|s| {
            matches!(
                s,
                EdgeSegment::Junction {
                    col: 10,
                    kind: JunctionKind::CornerUR,
                    ..
                }
            )
        });
        let has_tee_down = segments.iter().any(|s| {
            matches!(
                s,
                EdgeSegment::Junction {
                    col: 25,
                    kind: JunctionKind::TeeDown,
                    ..
                }
            )
        });
        let has_corner_ul = segments.iter().any(|s| {
            matches!(
                s,
                EdgeSegment::Junction {
                    col: 40,
                    kind: JunctionKind::CornerUL,
                    ..
                }
            )
        });
        assert!(has_corner_ur, "Should have └ at leftmost parent");
        assert!(has_tee_down, "Should have ┬ at child center");
        assert!(has_corner_ul, "Should have ┘ at rightmost parent");
    }

    #[test]
    fn route_fork_bar_one_parent_two_children() {
        let parent_center = 25;
        let child_centers = vec![(2, 10), (3, 40)];
        let bar_row = 8;

        let segments = build_fork_bar(parent_center, &child_centers, bar_row, EdgeColor::Neutral);

        let has_corner_dr = segments.iter().any(|s| {
            matches!(
                s,
                EdgeSegment::Junction {
                    col: 10,
                    kind: JunctionKind::CornerDR,
                    ..
                }
            )
        });
        let has_tee_up = segments.iter().any(|s| {
            matches!(
                s,
                EdgeSegment::Junction {
                    col: 25,
                    kind: JunctionKind::TeeUp,
                    ..
                }
            )
        });
        let has_corner_dl = segments.iter().any(|s| {
            matches!(
                s,
                EdgeSegment::Junction {
                    col: 40,
                    kind: JunctionKind::CornerDL,
                    ..
                }
            )
        });
        assert!(has_corner_dr, "Should have ┌ at leftmost child");
        assert!(has_tee_up, "Should have ┴ at parent center");
        assert!(has_corner_dl, "Should have ┐ at rightmost child");
    }

    #[test]
    fn build_vertical_pipes_between_tiers() {
        let box_bottoms = vec![(0, 15, 5)];
        let bar_row = 8;
        let segments = build_vertical_pipes(&box_bottoms, bar_row);

        let has_vpipe = segments.iter().any(|s| {
            matches!(
                s,
                EdgeSegment::VPipe {
                    col: 15,
                    row_start: 6,
                    row_end: 7,
                    ..
                }
            )
        });
        assert!(
            has_vpipe,
            "Should have vertical pipe from box bottom+1 to bar_row-1"
        );
    }

    #[test]
    fn detect_crossing_marks_cross_junction() {
        let bar = EdgeSegment::HBar {
            row: 5,
            col_start: 5,
            col_end: 20,
            color: EdgeColor::Outgoing,
        };
        let pipe = EdgeSegment::VPipe {
            col: 12,
            row_start: 3,
            row_end: 7,
            color: EdgeColor::Incoming,
        };

        let crossings = detect_crossings(&[bar], &[pipe]);
        assert_eq!(crossings.len(), 1);
        assert!(matches!(
            crossings[0],
            EdgeSegment::Junction {
                col: 12,
                row: 5,
                kind: JunctionKind::Cross,
                ..
            }
        ));
    }

    #[test]
    fn build_vertical_pipes_no_gap() {
        let box_bottoms = vec![(0, 15, 8)];
        let bar_row = 8;
        let segments = build_vertical_pipes(&box_bottoms, bar_row);
        assert!(segments.is_empty(), "No pipe when there's no gap");
    }

    #[test]
    fn detect_crossings_no_intersection() {
        let bar = EdgeSegment::HBar {
            row: 10,
            col_start: 5,
            col_end: 20,
            color: EdgeColor::Neutral,
        };
        let pipe = EdgeSegment::VPipe {
            col: 12,
            row_start: 3,
            row_end: 7,
            color: EdgeColor::Neutral,
        };

        let crossings = detect_crossings(&[bar], &[pipe]);
        assert!(
            crossings.is_empty(),
            "No crossing when pipe doesn't reach bar row"
        );
    }

    #[test]
    fn detect_crossings_pipe_at_bar_endpoint() {
        let bar = EdgeSegment::HBar {
            row: 5,
            col_start: 5,
            col_end: 20,
            color: EdgeColor::Neutral,
        };
        let pipe = EdgeSegment::VPipe {
            col: 5,
            row_start: 3,
            row_end: 7,
            color: EdgeColor::Neutral,
        };

        let crossings = detect_crossings(&[bar], &[pipe]);
        assert!(crossings.is_empty(), "No crossing at bar endpoint");
    }

    #[test]
    fn route_fork_merge_tee_right() {
        let parent_centers = vec![(0, 10), (1, 40)];
        let child_center = 25;

        let segments = build_merge_bar_with_continuation(
            &parent_centers,
            child_center,
            5,
            EdgeColor::Neutral,
            Some(0),
        );

        let bob_junction = segments
            .iter()
            .find(|s| matches!(s, EdgeSegment::Junction { col: 10, .. }));
        assert!(
            matches!(
                bob_junction,
                Some(EdgeSegment::Junction {
                    kind: JunctionKind::TeeRight,
                    ..
                })
            ),
            "Bob should have ├ (continues down + branch right)"
        );
    }
}
