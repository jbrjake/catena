//! CSS-flexbox-like space distribution for responsive TUI layouts.
//!
//! Pure integer math on `u16` values — no ratatui dependency.
//! Distributes available space across items respecting min/preferred/max constraints.

/// A sizing constraint for one flex item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FlexItem {
    pub min: u16,
    pub preferred: u16,
    pub max: u16,
}

/// Result of flex distribution: where each item starts and how big it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FlexResult {
    pub offset: u16,
    pub size: u16,
}

/// Distribute `available` columns across `items`, respecting min/preferred/max.
///
/// Algorithm:
/// 1. Give every item its `min`. If available < sum(min), truncate the last item.
/// 2. Distribute remaining space toward `preferred`, proportionally.
/// 3. If space remains after preferred, distribute toward `max`, proportionally.
/// 4. Handle rounding remainders by giving leftover to the last item that can accept it.
/// 5. Compute contiguous offsets.
pub(crate) fn flex_distribute(available: u16, items: &[FlexItem]) -> Vec<FlexResult> {
    for item in items {
        debug_assert!(
            item.min <= item.preferred && item.preferred <= item.max,
            "FlexItem invariant violated: min={} preferred={} max={}",
            item.min,
            item.preferred,
            item.max
        );
    }

    if items.is_empty() {
        return Vec::new();
    }

    let n = items.len();
    let mut sizes = vec![0u16; n];

    // Phase 1: Give every item its min. Truncate last if insufficient.
    let mut remaining = available;
    for (i, item) in items.iter().enumerate() {
        if remaining >= item.min {
            sizes[i] = item.min;
            remaining -= item.min;
        } else {
            sizes[i] = remaining;
            remaining = 0;
            // All subsequent items get 0 (already initialized).
            break;
        }
    }

    // Phase 2: Distribute remaining space toward preferred, proportionally.
    if remaining > 0 {
        remaining = distribute_toward(items, &mut sizes, remaining, |item| item.preferred);
    }

    // Phase 3: Distribute remaining space toward max, proportionally.
    if remaining > 0 {
        remaining = distribute_toward(items, &mut sizes, remaining, |item| item.max);
    }

    // Phase 4: Give any leftover (from rounding or all items at max) to the last
    // item that can still accept space.
    if remaining > 0 {
        for i in (0..n).rev() {
            let headroom = items[i].max.saturating_sub(sizes[i]);
            if headroom > 0 {
                let give = remaining.min(headroom);
                sizes[i] += give;
                remaining -= give;
                if remaining == 0 {
                    break;
                }
            }
        }
    }

    // Phase 5: Compute contiguous offsets.
    let mut results = Vec::with_capacity(n);
    let mut offset = 0u16;
    for &size in &sizes {
        results.push(FlexResult { offset, size });
        offset += size;
    }

    results
}

/// Distribute `budget` toward each item's `target` (preferred or max),
/// proportional to each item's remaining want. Returns unspent budget.
fn distribute_toward(
    items: &[FlexItem],
    sizes: &mut [u16],
    mut budget: u16,
    target_fn: impl Fn(&FlexItem) -> u16,
) -> u16 {
    // Compute how much each item still wants to reach its target.
    let wants: Vec<u16> = items
        .iter()
        .zip(sizes.iter())
        .map(|(item, &sz)| target_fn(item).saturating_sub(sz))
        .collect();

    let total_want: u32 = wants.iter().map(|&w| w as u32).sum();
    if total_want == 0 {
        return budget;
    }

    let to_give = (budget as u32).min(total_want);

    // Proportional distribution with truncation (floor).
    let mut given = 0u32;
    for (i, &want) in wants.iter().enumerate() {
        if want == 0 {
            continue;
        }
        let share = ((want as u32) * to_give / total_want) as u16;
        // Clamp to the item's max.
        let clamped = share.min(items[i].max.saturating_sub(sizes[i]));
        sizes[i] += clamped;
        given += clamped as u32;
    }

    budget = budget.saturating_sub(given as u16);

    // Rounding remainder: give 1 extra to each item that still has headroom,
    // iterating from the last item backward.
    if budget > 0 {
        for i in (0..items.len()).rev() {
            if budget == 0 {
                break;
            }
            let headroom = target_fn(&items[i]).saturating_sub(sizes[i]);
            if headroom > 0 {
                let give = headroom.min(budget);
                sizes[i] += give;
                budget -= give;
            }
        }
    }

    budget
}

/// Compute start offsets for `item_sizes` centered with equal gaps.
///
/// Implements `justify-content: space-evenly`: the gap before the first
/// item, between each pair of items, and after the last item are all equal.
///
/// If items exceed available space, they pack from offset 0 with no gaps.
pub(crate) fn flex_center_with_gaps(available: u16, item_sizes: &[u16]) -> Vec<u16> {
    if item_sizes.is_empty() {
        return Vec::new();
    }

    // Use u32 intermediate to avoid overflow when many items are summed.
    let total: u32 = item_sizes.iter().map(|&s| s as u32).sum();

    if total >= available as u32 {
        // Pack from left with no gaps.
        let mut offsets = Vec::with_capacity(item_sizes.len());
        let mut x = 0u16;
        for &size in item_sizes {
            offsets.push(x);
            x = x.saturating_add(size);
        }
        return offsets;
    }

    let remaining = (available as u32) - total;
    let n_gaps = (item_sizes.len() as u32) + 1;
    let gap = (remaining / n_gaps) as u16;

    let mut offsets = Vec::with_capacity(item_sizes.len());
    let mut x = gap;
    for &size in item_sizes {
        offsets.push(x);
        x += size + gap;
    }

    offsets
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distribute_basic_three_items() {
        // 3 items each min=5/pref=10/max=15 in 40 cols.
        // Each gets at least preferred. Total == 40.
        let items = vec![
            FlexItem {
                min: 5,
                preferred: 10,
                max: 15,
            },
            FlexItem {
                min: 5,
                preferred: 10,
                max: 15,
            },
            FlexItem {
                min: 5,
                preferred: 10,
                max: 15,
            },
        ];
        let results = flex_distribute(40, &items);
        assert_eq!(results.len(), 3);
        for r in &results {
            assert!(
                r.size >= 10,
                "each item should get at least preferred (10), got {}",
                r.size
            );
        }
        let total: u16 = results.iter().map(|r| r.size).sum();
        assert_eq!(total, 40);
    }

    #[test]
    fn distribute_insufficient_space() {
        // available (12) < sum mins (15). First two get min, last gets remainder.
        let items = vec![
            FlexItem {
                min: 5,
                preferred: 10,
                max: 15,
            },
            FlexItem {
                min: 5,
                preferred: 10,
                max: 15,
            },
            FlexItem {
                min: 5,
                preferred: 10,
                max: 15,
            },
        ];
        let results = flex_distribute(12, &items);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].size, 5);
        assert_eq!(results[1].size, 5);
        assert_eq!(results[2].size, 2);
    }

    #[test]
    fn distribute_exact_min() {
        // available == sum mins. Each gets exactly min.
        let items = vec![
            FlexItem {
                min: 5,
                preferred: 10,
                max: 15,
            },
            FlexItem {
                min: 5,
                preferred: 10,
                max: 15,
            },
            FlexItem {
                min: 5,
                preferred: 10,
                max: 15,
            },
        ];
        let results = flex_distribute(15, &items);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].size, 5);
        assert_eq!(results[1].size, 5);
        assert_eq!(results[2].size, 5);
    }

    #[test]
    fn distribute_single_item() {
        // 1 item min=5/pref=20/max=30 in 25 cols. Gets size=25.
        let items = vec![FlexItem {
            min: 5,
            preferred: 20,
            max: 30,
        }];
        let results = flex_distribute(25, &items);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].offset, 0);
        assert_eq!(results[0].size, 25);
    }

    #[test]
    fn distribute_empty() {
        let results = flex_distribute(100, &[]);
        assert!(results.is_empty());
    }

    #[test]
    fn distribute_offsets_are_contiguous() {
        // For any result, item[i].offset == item[i-1].offset + item[i-1].size.
        let items = vec![
            FlexItem {
                min: 3,
                preferred: 8,
                max: 20,
            },
            FlexItem {
                min: 5,
                preferred: 12,
                max: 25,
            },
            FlexItem {
                min: 4,
                preferred: 10,
                max: 18,
            },
            FlexItem {
                min: 2,
                preferred: 6,
                max: 10,
            },
        ];
        let results = flex_distribute(50, &items);
        assert_eq!(results.len(), 4);
        assert_eq!(results[0].offset, 0);
        for i in 1..results.len() {
            assert_eq!(
                results[i].offset,
                results[i - 1].offset + results[i - 1].size,
                "offset[{}] should be contiguous with previous item",
                i,
            );
        }
    }

    #[test]
    fn distribute_respects_max() {
        // Even with 100 cols, items with max=12 don't exceed 12.
        let items = vec![
            FlexItem {
                min: 5,
                preferred: 10,
                max: 12,
            },
            FlexItem {
                min: 5,
                preferred: 10,
                max: 12,
            },
            FlexItem {
                min: 5,
                preferred: 10,
                max: 12,
            },
        ];
        let results = flex_distribute(100, &items);
        assert_eq!(results.len(), 3);
        for r in &results {
            assert!(r.size <= 12, "item size {} exceeds max 12", r.size);
        }
    }

    // --- flex_center_with_gaps tests ---

    #[test]
    fn center_gaps_three_items() {
        // 3 items of size 10 in 70 cols. Total=30, remaining=40, 4 gaps of 10.
        // Positions: 10, 30, 50.
        let offsets = flex_center_with_gaps(70, &[10, 10, 10]);
        assert_eq!(offsets, vec![10, 30, 50]);
    }

    #[test]
    fn center_gaps_single_item() {
        // 1 item of size 20 in 60 cols. remaining=40, 2 gaps of 20.
        // Position: 20.
        let offsets = flex_center_with_gaps(60, &[20]);
        assert_eq!(offsets, vec![20]);
    }

    #[test]
    fn center_gaps_items_fill_space() {
        // Items exactly fill available (30 cols, 3x10). Gaps=0.
        // Positions: 0, 10, 20.
        let offsets = flex_center_with_gaps(30, &[10, 10, 10]);
        assert_eq!(offsets, vec![0, 10, 20]);
    }

    #[test]
    fn center_gaps_items_exceed_space() {
        // Items exceed available (20 cols, 3x10). Flush left.
        // Positions: 0, 10, 20.
        let offsets = flex_center_with_gaps(20, &[10, 10, 10]);
        assert_eq!(offsets, vec![0, 10, 20]);
    }

    #[test]
    fn center_gaps_empty() {
        // Empty items returns empty vec.
        let offsets = flex_center_with_gaps(50, &[]);
        assert!(offsets.is_empty());
    }

    #[test]
    fn center_gaps_unequal_sizes() {
        // 3 items: 8, 12, 6 in 50 cols. Total=26, remaining=24, 4 gaps of 6.
        // Positions: 6, 20, 38.
        let offsets = flex_center_with_gaps(50, &[8, 12, 6]);
        assert_eq!(offsets, vec![6, 20, 38]);
    }
}
