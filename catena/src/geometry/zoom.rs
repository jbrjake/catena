//! Semantic zoom — the six label-detail levels a continuous zoom selects between (plan §5),
//! and the zoom range the controller clamps to.
//!
//! The level boundaries are a configurable [`SemanticZoomTable`] whose defaults are the seed's
//! proven thresholds. They are hysteresis-free by decision: a zoom crossing 1.5 flips between
//! levels 2 and 3 exactly at 1.5 in both directions, because a level that depended on history
//! would break "same input, same frame" (plan §11.5).

/// The smallest zoom the controller allows (the seed's `zoom_out` clamp in
/// `seed/app/navigation.rs`).
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "the viewport (M2 step 4) clamps to it")
)]
pub(crate) const MIN_ZOOM: f64 = 0.1;

/// The largest zoom the controller allows (the seed's `zoom_in` clamp).
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "the viewport (M2 step 4) clamps to it")
)]
pub(crate) const MAX_ZOOM: f64 = 4.0;

/// The zoom a view starts at: one world unit to one cell column, which semantic zoom shows at
/// level 2, `[truncated]` labels.
pub(crate) const DEFAULT_ZOOM: f64 = 1.0;

/// One of the six semantic detail levels: 0 draws a single glyph, 5 the full label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct SemanticZoom(u8);

impl SemanticZoom {
    /// The level number, 0 to 5.
    pub(crate) fn index(self) -> u8 {
        self.0
    }
}

/// The zoom thresholds and per-level width caps of semantic zoom.
///
/// Level `i < 5` covers zooms up to and including `upper[i]` (and above `upper[i - 1]`); level 5
/// covers everything above `upper[4]` and has no width cap of its own (only the label limit,
/// `max_label_cols`, bounds it).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SemanticZoomTable {
    upper: [f64; 5],
    caps: [u16; 5],
}

impl Default for SemanticZoomTable {
    /// The seed's thresholds (`zoom_to_semantic`) and caps (`visual_label_width`), plan §5:
    ///
    /// | zoom ≤ | level | node form | width cap |
    /// |---|---|---|---|
    /// | 0.30 | 0 | single glyph | 1 |
    /// | 0.35 | 1 | `[A…]` | 5 |
    /// | 1.5 | 2 | `[truncated]` | 14 |
    /// | 2.5 | 3 | | 22 |
    /// | 3.5 | 4 | | 34 |
    /// | ∞ | 5 | full label | unlimited |
    ///
    /// The caps include the two bracket columns: level 2's 14 shows about 12 label columns.
    fn default() -> Self {
        Self {
            upper: [0.30, 0.35, 1.5, 2.5, 3.5],
            caps: [1, 5, 14, 22, 34],
        }
    }
}

impl SemanticZoomTable {
    /// A custom table: `upper` holds the inclusive zoom ceilings of levels 0 to 4, `caps` their
    /// width caps in display columns. `None` unless the ceilings are finite and strictly
    /// increasing and the caps are at least 1 and non-decreasing, since a level that shows
    /// more detail must not be narrower than the one below it.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "the builder (M2 step 4) takes a custom table")
    )]
    pub(crate) fn new(upper: [f64; 5], caps: [u16; 5]) -> Option<Self> {
        let ceilings_ok =
            upper.iter().all(|u| u.is_finite()) && upper.windows(2).all(|pair| pair[0] < pair[1]);
        let caps_ok = caps[0] >= 1 && caps.windows(2).all(|pair| pair[0] <= pair[1]);
        (ceilings_ok && caps_ok).then_some(Self { upper, caps })
    }

    /// The level a continuous zoom selects. A NaN zoom has no level of its own and reads as the
    /// least detailed, level 0.
    pub(crate) fn level(&self, zoom: f64) -> SemanticZoom {
        if zoom.is_nan() {
            return SemanticZoom(0);
        }
        let below = self
            .upper
            .iter()
            .take_while(|&&ceiling| zoom > ceiling)
            .count();
        SemanticZoom(u8::try_from(below).expect("at most five ceilings"))
    }

    /// The most display columns a node may take at `level`, decorations included; `None` at
    /// level 5, which only `max_label_cols` bounds. Level 0 draws one glyph, so its cap bounds
    /// that glyph's width.
    pub(crate) fn cap(&self, level: SemanticZoom) -> Option<u16> {
        self.caps.get(usize::from(level.index())).copied()
    }
}

#[cfg(test)]
#[path = "zoom_tests.rs"]
mod tests;
