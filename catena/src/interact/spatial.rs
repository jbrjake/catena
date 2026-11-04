//! The bucket grid that hit-testing searches (plan §10.2), ported from the seed's
//! `EdgeSpatialGrid`: the frame is cut into buckets of 16 columns by 8 rows, and each bucket
//! lists the items whose bounds reach it, so a query under the pointer looks at a handful of
//! items instead of all of them.
//!
//! Changes from the seed: it indexes any item by number (scene items, nodes and edges alike)
//! instead of `Uuid` edges; bounds come as cell boxes, already inflated by the caller's hit
//! margin; coordinates are the frame's own, so there is no origin; and rebuilding keeps every
//! bucket's allocation, so a frame that rebuilds the grid at the same size allocates nothing.
//! The seed's `EdgeScreenPos` gives way to the scene's own routes.

use crate::geometry::cell::CellBox;
use crate::raster::MAX_CELLS;

/// A bucket's width, in columns (the seed's).
const BUCKET_W: u16 = 16;
/// A bucket's height, in rows (the seed's).
const BUCKET_H: u16 = 8;

/// Items by the buckets their bounds reach.
#[derive(Debug, Clone, Default)]
pub(crate) struct SpatialGrid {
    cols: u16,
    rows: u16,
    /// Row-major: `buckets[row * cols + col]` lists items in insertion order.
    buckets: Vec<Vec<u32>>,
}

impl SpatialGrid {
    /// Indexes `items`, each an id and its bounds, over a `width × height` frame. Bounds past
    /// the frame are clipped to it; an item wholly off it is not indexed.
    pub(crate) fn rebuild(
        &mut self,
        (width, height): (u16, u16),
        items: impl IntoIterator<Item = (u32, CellBox)>,
    ) {
        // Only what can be drawn is indexed: a frame past `MAX_CELLS` keeps its width and loses
        // rows, as a `CellGrid` and a canvas do, so a resize to any `u16` size stays bounded.
        let drawn = MAX_CELLS / usize::from(width.max(1));
        let height = height.min(u16::try_from(drawn).unwrap_or(u16::MAX));
        self.cols = width.div_ceil(BUCKET_W);
        self.rows = height.div_ceil(BUCKET_H);
        let len = usize::from(self.cols) * usize::from(self.rows);
        self.buckets.truncate(len);
        for bucket in &mut self.buckets {
            bucket.clear();
        }
        self.buckets.resize_with(len, Vec::new);
        if len == 0 {
            return;
        }
        let (right, bottom) = (i64::from(width), i64::from(height));
        for (id, b) in items {
            let (x0, y0) = (i64::from(b.x).max(0), i64::from(b.y).max(0));
            let (x1, y1) = (b.right().min(right), b.bottom().min(bottom));
            if b.width == 0 || b.height == 0 || x0 >= x1 || y0 >= y1 {
                continue;
            }
            let bucket = |v: i64, size: u16| usize::try_from(v / i64::from(size)).unwrap_or(0);
            let (c0, c1) = (bucket(x0, BUCKET_W), bucket(x1 - 1, BUCKET_W));
            let (r0, r1) = (bucket(y0, BUCKET_H), bucket(y1 - 1, BUCKET_H));
            for r in r0..=r1 {
                for c in c0..=c1 {
                    self.buckets[r * usize::from(self.cols) + c].push(id);
                }
            }
        }
    }

    /// The items whose bounds may reach cell `(x, y)`, in insertion order; none off the frame.
    pub(crate) fn query(&self, x: u16, y: u16) -> &[u32] {
        let (c, r) = (x / BUCKET_W, y / BUCKET_H);
        if c >= self.cols || r >= self.rows {
            return &[];
        }
        &self.buckets[usize::from(r) * usize::from(self.cols) + usize::from(c)]
    }
}

#[cfg(test)]
#[path = "spatial_tests.rs"]
mod tests;
