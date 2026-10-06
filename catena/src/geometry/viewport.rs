//! The viewport (plan §6): the canonical snap, and the pan and zoom every frame derives from
//! it.
//!
//! A layout is snapped once into canonical cells at `ref_zoom`, independent of the pan; each
//! frame derives the cells it draws from them in O(n) ([`derive`]), so pan and zoom never run
//! a layout. Pan and zoom are `f64` state, rounded once, in the derive step (ledger row 32:
//! the seed rounded the pan on every zoom tick, so a long glide drifted).
//!
//! Three things move canonical cells. A refit snaps a new layout afresh, keeping the focused
//! node where it was drawn by moving the pan (anchor compensation). A re-snap places nodes
//! whose box or pin changed again, under the last refit's transform, with every other node
//! starting from its cell, so only what a changed box pushes moves. And a zoom out, which
//! shrinks the spacing while boxes keep their width, re-resolves the nodes whose drawn boxes
//! now meet (the zoom-out re-snap).

use std::collections::BTreeSet;

use super::ResolvedMetrics;
use super::cell::{CellBox, CellPt, SubPt};
use super::snap::{Fit, Grid, MARGIN, Snap, Transform, derive, half_up, resolve};
use super::zoom::{DEFAULT_ZOOM, MAX_ZOOM, MIN_ZOOM};
use crate::graph::NodeIx;

/// The viewport's state: its grid, its zoom and pan, and the canonical snap they derive from.
#[derive(Debug, Clone)]
pub(crate) struct Viewport {
    grid: Grid,
    fit: Fit,
    zoom: f64,
    /// In cells, added after zooming.
    pan: (f64, f64),
    /// The zoom the canonical snap ran at.
    ref_zoom: f64,
    /// The last refit's map from world space to cells; `None` before the first.
    transform: Option<Transform>,
    snap: Snap,
}

impl Viewport {
    /// A viewport over `grid` at the default zoom, with no pan and nothing snapped.
    pub(crate) fn new(grid: Grid, fit: Fit) -> Self {
        Viewport {
            grid,
            fit,
            zoom: DEFAULT_ZOOM,
            pan: (0.0, 0.0),
            ref_zoom: DEFAULT_ZOOM,
            transform: None,
            snap: Snap::default(),
        }
    }

    /// The grid the viewport covers.
    pub(crate) fn grid(&self) -> Grid {
        self.grid
    }

    /// The zoom now.
    pub(crate) fn zoom(&self) -> f64 {
        self.zoom
    }

    /// The zoom the canonical snap ran at.
    pub(crate) fn ref_zoom(&self) -> f64 {
        self.ref_zoom
    }

    /// The canonical snap.
    pub(crate) fn canonical(&self) -> &Snap {
        &self.snap
    }

    /// Where node `ix`'s anchor is drawn now.
    pub(crate) fn cell(&self, ix: NodeIx) -> Option<CellPt> {
        let canonical = self.snap.anchors.get(ix.slot()).copied().flatten()?;
        Some(derive(canonical, self.ratio(), self.pan))
    }

    /// Every placed node's box as drawn now.
    pub(crate) fn boxes<'a>(
        &'a self,
        metrics: &'a ResolvedMetrics,
    ) -> impl Iterator<Item = (NodeIx, CellBox)> + 'a {
        self.snap.boxes(metrics).filter_map(move |(ix, canonical)| {
            let form = metrics.form(ix)?;
            let at = self.cell(ix)?;
            let anchor = form.anchor();
            Some((
                ix,
                CellBox::new(
                    at.x.saturating_sub(anchor.x),
                    at.y.saturating_sub(anchor.y),
                    canonical.width,
                    canonical.height,
                ),
            ))
        })
    }

    /// Covers a grid of another size. What it covers is the caller's to lay out and refit.
    pub(crate) fn resize(&mut self, grid: Grid) {
        self.grid = grid;
    }

    /// Fits the world positions `world` (node centers, by slot) afresh and snaps them at the
    /// current zoom, placing nodes in `order`. Node `keep`, if drawn before and after, is drawn
    /// where it was: the pan moves by the difference (plan §6, anchor compensation).
    pub(crate) fn refit(
        &mut self,
        world: &[Option<(f64, f64)>],
        metrics: &ResolvedMetrics,
        order: &[NodeIx],
        keep: Option<NodeIx>,
    ) {
        let before = keep.and_then(|ix| self.cell(ix));
        self.transform = Transform::of(world, metrics, self.grid, self.fit);
        let wanted = self.transform.map_or_else(
            || vec![None; world.len()],
            |transform| transform.wanted(world, metrics, self.zoom),
        );
        self.snap = resolve(&wanted, metrics, order);
        self.ref_zoom = self.zoom;
        self.compensate(keep, before);
    }

    /// Snaps again under the last refit's transform after the nodes `changed` took new boxes
    /// or positions: they go where `world` puts them, every other node starts from its cell,
    /// and collisions resolve in `order`. Returns the nodes moved off where they started,
    /// which is the cascade plan §4.2 counts. Refits when nothing was fitted yet.
    pub(crate) fn resnap(
        &mut self,
        changed: &BTreeSet<NodeIx>,
        world: &[Option<(f64, f64)>],
        metrics: &ResolvedMetrics,
        order: &[NodeIx],
        keep: Option<NodeIx>,
    ) -> Vec<NodeIx> {
        let Some(transform) = self.transform else {
            self.refit(world, metrics, order, keep);
            return Vec::new();
        };
        let before = keep.and_then(|ix| self.cell(ix));
        let fitted = transform.wanted(world, metrics, self.ref_zoom);
        let wanted: Vec<Option<SubPt>> = fitted
            .iter()
            .enumerate()
            .map(|(slot, fit)| {
                let ix = NodeIx::new(u32::try_from(slot).ok()?);
                let kept = self.snap.anchors.get(slot).copied().flatten();
                match kept {
                    Some(cell) if !changed.contains(&ix) => Some(SubPt::from(cell)),
                    _ => *fit,
                }
            })
            .collect();
        self.snap = resolve(&wanted, metrics, order);
        self.compensate(keep, before);
        self.snap.displaced.clone()
    }

    /// Moves the view by `(dx, dy)` cells. A pan that is not finite is ignored.
    pub(crate) fn pan_by(&mut self, (dx, dy): (f64, f64)) {
        if dx.is_finite() && dy.is_finite() {
            self.pan = (self.pan.0 + dx, self.pan.1 + dy);
        }
    }

    /// Zooms to `zoom`, clamped to the zoom range, keeping the view point `about` where it is:
    /// `pan′ = pan − (about − margin − pan)(ratio − 1)` (plan §6). A NaN zoom is ignored.
    pub(crate) fn zoom_about(&mut self, zoom: f64, about: SubPt) {
        if zoom.is_nan() {
            return;
        }
        let zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        let ratio = zoom / self.zoom;
        let margin = f64::from(MARGIN);
        self.pan = (
            self.pan.0 - (about.x - margin - self.pan.0) * (ratio - 1.0),
            self.pan.1 - (about.y - margin - self.pan.1) * (ratio - 1.0),
        );
        self.zoom = zoom;
    }

    /// The zoom-out re-snap (plan §6): below the snap's zoom the spacing shrinks while boxes
    /// keep their width, so drawn boxes can meet. Re-resolves the drawn cells in `order` and,
    /// if any node had to move, makes the result the canonical set at the current zoom.
    /// Returns the nodes moved; every other node is drawn exactly where it was.
    pub(crate) fn resnap_if_crowded(
        &mut self,
        metrics: &ResolvedMetrics,
        order: &[NodeIx],
    ) -> Vec<NodeIx> {
        if self.zoom >= self.ref_zoom {
            return Vec::new();
        }
        // Drawn cells less the pan's whole cells: derived anew with the same pan, a node that
        // keeps its cell is drawn where it was, since `derive` rounds half up.
        let whole = (half_up(self.pan.0), half_up(self.pan.1));
        let wanted: Vec<Option<SubPt>> = (0..self.snap.anchors.len())
            .map(|slot| {
                let ix = NodeIx::new(u32::try_from(slot).ok()?);
                let at = self.cell(ix)?;
                Some(SubPt::from(CellPt::new(
                    at.x.saturating_sub(whole.0),
                    at.y.saturating_sub(whole.1),
                )))
            })
            .collect();
        let snap = resolve(&wanted, metrics, order);
        if snap.displaced.is_empty() {
            return Vec::new();
        }
        let moved = snap.displaced.clone();
        self.snap = snap;
        self.ref_zoom = self.zoom;
        moved
    }

    /// The zoom now over the snap's.
    fn ratio(&self) -> f64 {
        self.zoom / self.ref_zoom
    }

    /// Moves the pan so node `keep`, drawn at `before`, is drawn there again.
    fn compensate(&mut self, keep: Option<NodeIx>, before: Option<CellPt>) {
        if let (Some(ix), Some(before)) = (keep, before)
            && let Some(now) = self.cell(ix)
        {
            self.pan_by((
                f64::from(before.x) - f64::from(now.x),
                f64::from(before.y) - f64::from(now.y),
            ));
        }
    }
}

#[cfg(test)]
#[path = "viewport_tests.rs"]
mod tests;
