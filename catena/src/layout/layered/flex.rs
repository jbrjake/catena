//! CSS-flexbox-like space distribution for responsive TUI layouts.
//!
//! Pure integer math on `u16` values — no ratatui dependency.
//! Distributes available space across items respecting min/preferred/max constraints.
//! The layered engine uses it to allocate layer bands and routing channels (plan §9.1 step 6).

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
/// 1. Give every item its `min`, in order. If `available < sum(min)`, the first item whose
///    `min` no longer fits gets whatever remains and every item after it gets zero; the items
///    before it keep their full `min`.
/// 2. Distribute remaining space toward `preferred`, proportionally.
/// 3. If space remains after preferred, distribute toward `max`, proportionally.
/// 4. Handle rounding remainders by giving leftover to items that can still accept it, from
///    the last item backward.
/// 5. Compute contiguous offsets.
///
/// The sizes sum to `min(available, sum(max))`, and every item gets `min ≤ size ≤ max` whenever
/// `available` covers the minimums (plan §16.2 invariant L).
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

    // Phase 1: Give every item its min. The first that no longer fits takes the remainder.
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

    // Phase 5: Compute contiguous offsets. The sizes sum to at most `available`, so no
    // offset overflows.
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

    let total_want: u32 = wants.iter().map(|&w| u32::from(w)).sum();
    if total_want == 0 {
        return budget;
    }

    let to_give = u32::from(budget).min(total_want);

    // Proportional distribution with truncation (floor).
    let mut given = 0u16;
    for (i, &want) in wants.iter().enumerate() {
        if want == 0 {
            continue;
        }
        // want * to_give <= 65535² fits in u32, and the share is at most to_give <= budget.
        let share = u16::try_from(u32::from(want) * to_give / total_want)
            .expect("a share never exceeds the u16 budget");
        // Clamp to the item's max.
        let clamped = share.min(items[i].max.saturating_sub(sizes[i]));
        sizes[i] += clamped;
        given += clamped;
    }

    budget = budget.saturating_sub(given);

    // Rounding remainder (fewer columns than there are items): from the last item backward,
    // each item with headroom takes as much of it as it can hold. (The seed's comment said "1
    // extra to each item"; the code has always let the last item take the whole remainder.)
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
    let total: u32 = item_sizes.iter().map(|&s| u32::from(s)).sum();

    if total >= u32::from(available) {
        // Pack from left with no gaps.
        let mut offsets = Vec::with_capacity(item_sizes.len());
        let mut x = 0u16;
        for &size in item_sizes {
            offsets.push(x);
            x = x.saturating_add(size);
        }
        return offsets;
    }

    let remaining = u32::from(available) - total;
    let n_gaps = u32::try_from(item_sizes.len())
        .unwrap_or(u32::MAX)
        .saturating_add(1);
    let gap = u16::try_from(remaining / n_gaps).expect("a gap is at most `available`");

    // Every offset stays below `available`: gap * (n + 1) + total <= available.
    let mut offsets = Vec::with_capacity(item_sizes.len());
    let mut x = gap;
    for &size in item_sizes {
        offsets.push(x);
        x += size + gap;
    }

    offsets
}

#[cfg(test)]
#[path = "flex_tests.rs"]
mod tests;
