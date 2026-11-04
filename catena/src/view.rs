//! [`GraphView`], the one object a host holds (plan §4.2, §13): the graph store, every node's
//! measured box, its layout and the viewport it is drawn in, and with later milestones its
//! controller and caches.

use std::collections::BTreeSet;
use std::marker::PhantomData;

use crate::draw::{self, Drawing};
use crate::geometry::cell::{CellPt, SubPt};
use crate::geometry::{
    DEFAULT_ZOOM, Fit, Grid, ResolvedMetrics, SemanticZoomTable, Viewport, placement_order,
};
use crate::graph::{
    Delta, DeltaClass, EdgeId, EdgeRef, GraphError, GraphStore, Key, Limits, NodeIx, Tx,
};
use crate::layout::LayoutKind;
use crate::layout::force::{Change, ForceState, Frame, lay_out};
use crate::raster::{Blitter, CellGrid, Surface};
use crate::scene::{RenderOptions, SceneGraph};

/// The default ceiling on a label's display columns (plan §4.1).
pub const DEFAULT_MAX_LABEL_COLS: u16 = 256;

/// The default cell aspect, a cell's width over its height: about twice as tall as wide
/// (plan §6).
pub const DEFAULT_CELL_ASPECT: f64 = 0.5;

/// The viewport a view lays out for until it is first drawn at another size: the classic
/// terminal.
const DEFAULT_VIEWPORT: (u16, u16) = (80, 24);

/// How many nodes an edit's re-snap may push aside before the layout runs instead (plan
/// §4.2).
const CASCADE_LIMIT: usize = 8;

/// An interactive graph keyed by the host's `K` (plan §4.2).
#[derive(Debug, Clone)]
pub struct GraphView<K: Key> {
    pub(crate) store: GraphStore<K>,
    /// Every node's form at the current semantic zoom level, kept in step with each commit.
    pub(crate) metrics: ResolvedMetrics,
    /// Every commit since the layout last caught up, absorbed in order, its reshapes cut to
    /// the nodes whose measured box changed. The layout catches up at the end of each
    /// `update`, so it is empty between them.
    pending: Delta,
    layout: LayoutKind,
    /// The force layout's world positions and what it keeps between runs.
    force: ForceState,
    pub(crate) viewport: Viewport,
    /// The node a relayout keeps where it is drawn (plan §6, anchor compensation); the
    /// controller (M3) sets it.
    focus: Option<NodeIx>,
    /// How frames are drawn.
    pub(crate) render_options: RenderOptions,
    /// The scene and compositor frames reuse.
    drawing: Drawing,
}

/// Configures a [`GraphView`]; `GraphView::builder().build()` is a complete configuration.
#[derive(Debug, Clone)]
pub struct GraphViewBuilder<K: Key> {
    max_label_cols: u16,
    layout: LayoutKind,
    fit: Fit,
    cell_aspect: f64,
    blitter: Blitter,
    key: PhantomData<fn() -> K>,
}

impl<K: Key> GraphViewBuilder<K> {
    /// Cuts every label and sort key to `cols` display columns as it enters the graph
    /// (default [`DEFAULT_MAX_LABEL_COLS`]), so one hostile label cannot make a frame
    /// O(label).
    #[must_use]
    pub fn max_label_cols(mut self, cols: u16) -> Self {
        self.max_label_cols = cols;
        self
    }

    /// Places nodes with `kind` (default [`LayoutKind::Force`] with default parameters).
    #[must_use]
    pub fn layout(mut self, kind: LayoutKind) -> Self {
        self.layout = kind;
        self
    }

    /// Maps the layout onto the viewport by `fit` (default [`Fit::Contain`]).
    #[must_use]
    pub fn fit(mut self, fit: Fit) -> Self {
        self.fit = fit;
        self
    }

    /// Takes a cell to be `aspect` times as wide as it is tall (default
    /// [`DEFAULT_CELL_ASPECT`]), so the layout's distances keep their proportions on screen. An
    /// aspect that is not finite and positive keeps the default.
    #[must_use]
    pub fn cell_aspect(mut self, aspect: f64) -> Self {
        self.cell_aspect = if aspect.is_finite() && aspect > 0.0 {
            aspect
        } else {
            DEFAULT_CELL_ASPECT
        };
        self
    }

    /// Draws edges with `blitter` (default [`Blitter::Braille`]): the trade between
    /// resolution and colors per cell (plan §7.2).
    #[must_use]
    pub fn blitter(mut self, blitter: Blitter) -> Self {
        self.blitter = blitter;
        self
    }

    /// The configured view, with an empty graph.
    #[must_use]
    pub fn build(self) -> GraphView<K> {
        let table = SemanticZoomTable::default();
        let level = table.level(DEFAULT_ZOOM);
        let grid = Grid {
            cols: DEFAULT_VIEWPORT.0,
            rows: DEFAULT_VIEWPORT.1,
            cell_aspect: self.cell_aspect,
        };
        GraphView {
            store: GraphStore::new(self.max_label_cols, Limits::default()),
            metrics: ResolvedMetrics::new(table, level),
            pending: Delta::default(),
            layout: self.layout,
            force: ForceState::default(),
            viewport: Viewport::new(grid, self.fit),
            focus: None,
            render_options: RenderOptions {
                blitter: self.blitter,
                cell_aspect: self.cell_aspect,
                ..RenderOptions::default()
            },
            drawing: Drawing::default(),
        }
    }
}

impl<K: Key> Default for GraphView<K> {
    fn default() -> Self {
        GraphView::builder().build()
    }
}

impl<K: Key> GraphView<K> {
    /// A builder with every default.
    #[must_use]
    pub fn builder() -> GraphViewBuilder<K> {
        GraphViewBuilder {
            max_label_cols: DEFAULT_MAX_LABEL_COLS,
            layout: LayoutKind::default(),
            fit: Fit::default(),
            cell_aspect: DEFAULT_CELL_ASPECT,
            blitter: Blitter::Braille,
            key: PhantomData,
        }
    }

    /// A view with every default: `GraphView::builder().build()`.
    #[must_use]
    pub fn new() -> Self {
        GraphView::default()
    }

    /// Runs one all-or-nothing transaction. When `f` returns `Ok`, everything it did commits
    /// at once, and the layout catches up before this returns (plan §4.2): a node or edge
    /// added or removed relayouts, a box or pin that changed re-snaps, and anything else moves
    /// nothing. When `f` returns `Err` (or panics), nothing it did happened.
    ///
    /// # Errors
    ///
    /// Whatever `f` returns.
    pub fn update<T>(
        &mut self,
        f: impl FnOnce(&mut Tx<'_, K>) -> Result<T, GraphError<K>>,
    ) -> Result<T, GraphError<K>> {
        let (out, mut delta) = self.store.transact(f)?;
        self.metrics.apply(&self.store, &mut delta);
        self.pending.absorb(delta);
        self.catch_up();
        Ok(out)
    }

    /// The key of the node at `ix`, or `None` if that slot holds no node.
    #[must_use]
    pub fn key(&self, ix: NodeIx) -> Option<&K> {
        self.store.key(ix)
    }

    /// The index of the node `key`, valid until the next commit.
    #[must_use]
    pub fn node_ix(&self, key: &K) -> Option<NodeIx> {
        self.store.ix_of(key)
    }

    /// The edge `id`: its ends, rank and spec.
    #[must_use]
    pub fn edge(&self, id: EdgeId) -> Option<EdgeRef<'_, K>> {
        self.store.edge_ref(self.store.edge_ix(id)?)
    }

    /// How many nodes the graph has.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.store.node_count()
    }

    /// How many edges the graph has.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.store.edge_count()
    }

    /// Where node `ix`'s anchor, the middle of its box, is drawn now.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "the controller (M3) hit-tests against anchors")
    )]
    pub(crate) fn cell(&self, ix: NodeIx) -> Option<CellPt> {
        self.viewport.cell(ix)
    }

    /// The scene as the view draws it now (plan §7.4): every node's box and every edge's
    /// route, in viewport cells.
    #[must_use]
    pub fn scene(&self) -> SceneGraph {
        let mut scene = SceneGraph::new();
        draw::build(&self.store, &self.metrics, &self.viewport, &mut scene);
        scene
    }

    /// Draws the graph onto `surface`, which sets the viewport: a surface of a new size lays
    /// out for it first (plan §7.1). Total: any graph draws, clipped to the surface.
    pub fn render<S: Surface + ?Sized>(&mut self, surface: &mut S) {
        let (cols, rows) = surface.size();
        self.resize(cols, rows);
        let Drawing { scene, compositor } = &mut self.drawing;
        draw::build(&self.store, &self.metrics, &self.viewport, scene);
        let metrics = &self.metrics;
        compositor.render(scene, surface, &self.render_options, draw::style, |ix| {
            metrics.form(ix)
        });
    }

    /// The graph drawn into `cols × rows` cells, as text: one line per row, trailing blanks
    /// trimmed. The no-terminal export (plan §7.1), and the substrate of golden tests.
    pub fn render_to_string(&mut self, cols: u16, rows: u16) -> String {
        let mut grid = CellGrid::new(cols, rows);
        self.render(&mut grid);
        grid.to_string()
    }

    /// Moves the view by `(dx, dy)` cells; no layout runs (plan §11.2).
    #[cfg_attr(not(test), expect(dead_code, reason = "the controller (M3) pans"))]
    pub(crate) fn pan_by(&mut self, delta: (f64, f64)) {
        self.viewport.pan_by(delta);
    }

    /// Zooms to `zoom`, keeping the view point `about` where it is. Crossing a semantic level
    /// relayouts around the nodes whose boxes collapsed or expanded (owner, "Relayout");
    /// within a level no layout runs, and a zoom out re-snaps only the nodes whose boxes would
    /// meet (plan §6).
    #[cfg_attr(not(test), expect(dead_code, reason = "the controller (M3) zooms"))]
    pub(crate) fn zoom_about(&mut self, zoom: f64, about: SubPt) {
        self.viewport.zoom_about(zoom, about);
        let level = self.metrics.level_at(self.viewport.zoom());
        let reshaped = self.metrics.set_level(&self.store, level);
        if reshaped.is_empty() {
            let order = placement_order(&self.store);
            self.viewport.resnap_if_crowded(&self.metrics, &order);
        } else {
            self.relayout(Change::Reshaped(&reshaped));
        }
    }

    /// Draws into `cols × rows` cells from now on: a full relayout (owner, "Relayout"), when
    /// the size changed.
    pub(crate) fn resize(&mut self, cols: u16, rows: u16) {
        let grid = Grid {
            cols,
            rows,
            ..self.viewport.grid()
        };
        if grid != self.viewport.grid() {
            self.viewport.resize(grid);
            self.relayout(Change::Resize);
        }
    }

    /// Lays out what the commits since the layout last caught up changed.
    fn catch_up(&mut self) {
        let pending = std::mem::take(&mut self.pending);
        match pending.class {
            Some(DeltaClass::Topology) => self.relayout(Change::Topology(&pending.added)),
            Some(DeltaClass::Geometry) => self.resnap(&pending),
            Some(DeltaClass::Property) | None => {}
        }
    }

    /// Re-snaps after boxes or pins changed, placing a moved pin's node at it; a re-snap that
    /// pushes more than `CASCADE_LIMIT` nodes aside relayouts around the changed boxes
    /// instead (plan §4.2).
    fn resnap(&mut self, delta: &Delta) {
        for &ix in &delta.repinned {
            let pin = self.store.node(ix).and_then(|node| node.spec.pinned);
            if let (Some(pin), Some(at)) = (pin, self.force.positions.get_mut(ix.slot())) {
                *at = Some(pin);
            }
        }
        let changed: BTreeSet<NodeIx> = delta.reshaped.union(&delta.repinned).copied().collect();
        let order = placement_order(&self.store);
        let pushed = self.viewport.resnap(
            &changed,
            &self.force.positions,
            &self.metrics,
            &order,
            self.focus,
        );
        if pushed.len() > CASCADE_LIMIT {
            self.relayout(Change::Reshaped(&changed));
        }
    }

    /// Runs the layout after `change` and snaps the result, keeping the focused node where it
    /// is drawn.
    fn relayout(&mut self, change: Change<'_>) {
        let grid = self.viewport.grid();
        let frame = Frame {
            area: (
                f64::from(grid.cols),
                f64::from(grid.rows) / grid.cell_aspect,
            ),
            cell_aspect: grid.cell_aspect,
        };
        let LayoutKind::Force(params) = &self.layout;
        lay_out(
            params,
            &self.store,
            &self.metrics,
            frame,
            change,
            &mut self.force,
        );
        let order = placement_order(&self.store);
        self.viewport
            .refit(&self.force.positions, &self.metrics, &order, self.focus);
    }
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
