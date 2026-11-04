//! The controller (plan §10.1, §10.3, §10.4): what input does to a view, as a plain state
//! machine with no clock. [`GraphView::handle`] takes an input event, [`GraphView::act`] an
//! action (what the key table maps a key to, or a host's own), and [`GraphView::tick`]
//! advances the pan glide and the zoom smoothing by the time the host passes.
//!
//! It keeps the selected node or edge, the hovered node or edges, a node drag, the two glides,
//! whether bundling is on, and the windows a host draws over the view. The selected node is
//! the one a relayout keeps where it is drawn (plan §6). Held indices are dropped when a
//! commit removes their node, since a slot is reused after it.
//!
//! Choices of mine, unratified: a `Pan` moves the view 4 columns or 2 rows, the way it names
//! (`Pan(Left)` shows what is to the left, so the drawing moves right); the selections of a
//! node and of an edge exclude each other; keyboard selection glides the node to the visible
//! center and a click does not; a pin set from the keyboard pins the node where it is drawn;
//! and spatial navigation with nothing selected starts from the visible center.

use std::time::Duration;

use super::animation::{PanGlide, ZoomGlide};
use super::navigation::{
    KEY_ZOOM_STEP, SCROLL_ZOOM_STEP, nearest_toward, pan_to, step_zoom, visible_center,
};
use super::pointer::Drag;
use super::{Action, Dir, Keymap};
use crate::GraphView;
use crate::draw::Marks;
use crate::geometry::cell::{CellBox, CellPt, SubPt};
use crate::graph::{Delta, DeltaClass, EdgeId, EdgeIx, GraphStore, Key, NodeIx};

/// What an input did that a host may react to (plan §10.1). An index names its node as of the
/// commit the outcome came from; [`GraphView::key`] turns it into the host's key.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum Outcome {
    /// The selected node changed, to this one or to none.
    Selected(Option<NodeIx>),
    /// The pointer came onto this node, or left every node.
    Hovered(Option<NodeIx>),
    /// The selected edge changed, to this one or to none.
    EdgeSelected(Option<EdgeId>),
    /// The node was pinned (`true`) or unpinned.
    Pinned(NodeIx, bool),
    /// A drag of the node ended with it at this world position. Its pin is again as it was
    /// before the drag: a node that was not pinned is free to move with the next layout, and
    /// a host that wants it to stay pins it here.
    DragCompleted(NodeIx, (f64, f64)),
    /// The view panned or zoomed, or began to.
    ViewChanged,
    /// Spatial navigation found no node in this direction.
    Boundary(Dir),
}

/// Whether a host needs to draw a frame (plan §10.4), from [`GraphView::tick`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Redraw {
    /// An animation is running, or the view changed since it was last drawn.
    Needed,
    /// Nothing changed since the last frame drawn.
    Idle,
}

/// The longest step a tick advances animations by, so a host that was suspended does not
/// fast-forward a glide into a teleport (plan §10.4).
pub const MAX_TICK: Duration = Duration::from_millis(250);

/// How far one `Pan` moves the view, in cells: a cell is about twice as tall as wide, so half
/// as many rows as columns.
const PAN_STEP: (f64, f64) = (4.0, 2.0);

/// The controller's state (plan §10.1), which [`GraphView`]'s methods here and in `pointer`
/// drive.
#[derive(Debug, Clone)]
pub(crate) struct Controller {
    keymap: Keymap,
    /// The selected node, which a relayout keeps where it is drawn.
    pub(crate) selected: Option<NodeIx>,
    selected_edge: Option<EdgeId>,
    pub(super) hovered: Option<NodeIx>,
    /// The edges under the pointer, when it is over edges and no node; kept across moves, so
    /// hovering allocates nothing once warm.
    pub(super) hovered_edges: Vec<EdgeIx>,
    pub(super) drag: Option<Drag>,
    pan: Option<PanGlide>,
    zoom: Option<ZoomGlide>,
    bundling: bool,
    occlusions: Vec<CellBox>,
    /// Where the last frame's surface sat on the host's screen ([`Surface::origin`]).
    ///
    /// [`Surface::origin`]: crate::raster::Surface::origin
    pub(crate) origin: (u16, u16),
}

impl Default for Controller {
    fn default() -> Self {
        Controller {
            keymap: Keymap::default(),
            selected: None,
            selected_edge: None,
            hovered: None,
            hovered_edges: Vec::new(),
            drag: None,
            pan: None,
            zoom: None,
            bundling: true,
            occlusions: Vec::new(),
            origin: (0, 0),
        }
    }
}

impl Controller {
    /// Drops what a commit made stale: nodes it removed, whose slots will be reused, and, when
    /// it changed edges, the hovered edges, whose indices are reused alike.
    pub(crate) fn forget<K: Key>(&mut self, store: &GraphStore<K>, delta: &Delta) {
        let gone = |ix: Option<NodeIx>| ix.is_some_and(|ix| delta.removed.contains(&ix));
        if gone(self.selected) {
            self.selected = None;
        }
        if gone(self.hovered) {
            self.hovered = None;
        }
        if gone(self.drag.map(|drag| drag.node)) {
            self.drag = None;
        }
        if delta.class == Some(DeltaClass::Topology) {
            self.hovered_edges.clear();
        }
        if self
            .selected_edge
            .is_some_and(|id| store.edge_ix(id).is_none())
        {
            self.selected_edge = None;
        }
    }

    /// What the scene draws marked.
    pub(crate) fn marks<'a, K: Key>(&'a self, store: &GraphStore<K>) -> Marks<'a> {
        Marks {
            selected: self.selected,
            hovered: self.hovered,
            hovered_edges: &self.hovered_edges,
            selected_edge: self.selected_edge.and_then(|id| store.edge_ix(id)),
        }
    }
}

impl<K: Key> GraphView<K> {
    /// Acts on one input event (plan §10.1): a key through the key table, the pointer's
    /// presses, moves and releases, a scroll, a resize. Pointer positions are screen cells:
    /// the last frame's surface origin ([`Surface::origin`]) is subtracted, a press or scroll
    /// off the surface is ignored, the pointer leaving it hovers nothing, and a drag past its
    /// edge holds at the edge. Returns what changed, in order.
    ///
    /// [`Surface::origin`]: crate::raster::Surface::origin
    pub fn handle(&mut self, event: super::InputEvent) -> Vec<Outcome> {
        use super::{Button, InputEvent};
        let mut out = Vec::new();
        match event {
            InputEvent::Key(key) => {
                if let Some(action) = self.controller.keymap.action(key) {
                    self.apply(action, &mut out);
                }
            }
            InputEvent::MouseDown(x, y, Button::Left) => {
                if let Some((x, y)) = self.local(x, y) {
                    self.press(x, y, &mut out);
                }
            }
            InputEvent::MouseUp(x, y, Button::Left) => {
                let (x, y) = self.clamped(x, y);
                self.release(x, y, &mut out);
            }
            InputEvent::MouseMove(x, y) => {
                if self.controller.drag.is_some() {
                    let (x, y) = self.clamped(x, y);
                    self.point(x, y, &mut out);
                } else {
                    let at = self.local(x, y);
                    self.hover(at, &mut out);
                }
            }
            InputEvent::Scroll(notches, x, y) => {
                if let Some((x, y)) = self.local(x, y) {
                    let about = SubPt::new(f64::from(x), f64::from(y));
                    self.zoom_by(f64::from(notches) * SCROLL_ZOOM_STEP, about, &mut out);
                }
            }
            InputEvent::Resize(cols, rows) => {
                let before = self.viewport.grid();
                self.resize(cols, rows);
                if self.viewport.grid() != before {
                    out.push(Outcome::ViewChanged);
                }
            }
            InputEvent::MouseDown(..) | InputEvent::MouseUp(..) => {}
        }
        out
    }

    /// Does what `action` asks, as its key would: the way a host with key handling of its own
    /// drives the view (plan §10.1). Returns what changed, in order.
    pub fn act(&mut self, action: Action) -> Vec<Outcome> {
        let mut out = Vec::new();
        self.apply(action, &mut out);
        out
    }

    /// Advances the pan glide and the zoom smoothing by `dt`, clamped to [`MAX_TICK`] (plan
    /// §10.4): the only way time enters the library. A `dt` of zero advances nothing.
    /// Returns whether a frame needs drawing.
    pub fn tick(&mut self, dt: Duration) -> Redraw {
        let dt = dt.min(MAX_TICK);
        if !dt.is_zero() {
            if let Some(mut glide) = self.controller.pan.take() {
                let (step, running) = glide.advance(dt);
                self.pan_by(step);
                self.controller.pan = running.then_some(glide);
            }
            if let Some(glide) = self.controller.zoom {
                let (zoom, done) = glide.step(self.viewport.zoom(), dt);
                self.zoom_about(zoom, glide.about);
                if done {
                    self.controller.zoom = None;
                }
            }
        }
        let animating = self.controller.pan.is_some() || self.controller.zoom.is_some();
        if animating || !self.drawing.shown {
            Redraw::Needed
        } else {
            Redraw::Idle
        }
    }

    /// The selected node.
    #[must_use]
    pub fn selected(&self) -> Option<NodeIx> {
        self.controller.selected
    }

    /// The selected edge.
    #[must_use]
    pub fn selected_edge(&self) -> Option<EdgeId> {
        self.controller.selected_edge
    }

    /// The node under the pointer.
    #[must_use]
    pub fn hovered(&self) -> Option<NodeIx> {
        self.controller.hovered
    }

    /// Whether edge bundling is on (owner ruling A4's toggle). No layout bundles edges yet;
    /// the radial view's will read this.
    #[must_use]
    pub fn bundling(&self) -> bool {
        self.controller.bundling
    }

    /// The key table [`GraphView::handle`] maps keys through.
    #[must_use]
    pub fn keymap(&self) -> &Keymap {
        &self.controller.keymap
    }

    /// The key table, to rebind (plan §10.1).
    pub fn keymap_mut(&mut self) -> &mut Keymap {
        &mut self.controller.keymap
    }

    /// Tells the view which of its cells the host draws other windows over, so centering
    /// aims at the middle of what stays visible.
    pub fn set_occlusions(&mut self, windows: &[CellBox]) {
        self.controller.occlusions.clear();
        self.controller.occlusions.extend_from_slice(windows);
    }

    fn apply(&mut self, action: Action, out: &mut Vec<Outcome>) {
        match action {
            Action::Pan(dir) => {
                self.controller.pan = None;
                let (dx, dy) = PAN_STEP;
                self.pan_by(match dir {
                    Dir::Left => (dx, 0.0),
                    Dir::Right => (-dx, 0.0),
                    Dir::Up => (0.0, dy),
                    Dir::Down => (0.0, -dy),
                });
                out.push(Outcome::ViewChanged);
            }
            Action::ZoomIn => self.zoom_by(KEY_ZOOM_STEP, self.visible_center(), out),
            Action::ZoomOut => self.zoom_by(-KEY_ZOOM_STEP, self.visible_center(), out),
            Action::CycleNext => self.cycle(true, out),
            Action::CyclePrev => self.cycle(false, out),
            Action::NavTo(dir) => self.nav_to(dir, out),
            Action::TogglePin => self.toggle_pin(out),
            Action::Clear => self.choose(None, None, out),
            Action::ToggleBundling => self.controller.bundling = !self.controller.bundling,
        }
    }

    /// Selects `node` or `edge` (at most one is `Some`), reporting each selection that
    /// changed.
    pub(super) fn choose(
        &mut self,
        node: Option<NodeIx>,
        edge: Option<EdgeId>,
        out: &mut Vec<Outcome>,
    ) {
        if self.controller.selected != node {
            self.controller.selected = node;
            self.drawing.invalidate();
            out.push(Outcome::Selected(node));
        }
        if self.controller.selected_edge != edge {
            self.controller.selected_edge = edge;
            self.drawing.invalidate();
            out.push(Outcome::EdgeSelected(edge));
        }
    }

    /// Starts or retargets the zoom glide `step` beyond its target, about the view point
    /// `about`. A step the clamp swallows changes nothing.
    fn zoom_by(&mut self, step: f64, about: SubPt, out: &mut Vec<Outcome>) {
        let target = self
            .controller
            .zoom
            .map_or_else(|| self.viewport.zoom(), |glide| glide.target);
        let next = step_zoom(target, step);
        #[expect(clippy::float_cmp, reason = "a clamped step returns the clamp exactly")]
        let unchanged = next == target;
        if unchanged {
            return;
        }
        self.controller.zoom = Some(ZoomGlide {
            target: next,
            about,
        });
        out.push(Outcome::ViewChanged);
    }

    /// The surface cell under screen cell `(x, y)`; `None` off the surface.
    fn local(&self, x: u16, y: u16) -> Option<(u16, u16)> {
        let (ox, oy) = self.controller.origin;
        let grid = self.viewport.grid();
        let (x, y) = (x.checked_sub(ox)?, y.checked_sub(oy)?);
        (x < grid.cols && y < grid.rows).then_some((x, y))
    }

    /// The surface cell nearest screen cell `(x, y)`.
    fn clamped(&self, x: u16, y: u16) -> (u16, u16) {
        let (ox, oy) = self.controller.origin;
        let grid = self.viewport.grid();
        (
            x.saturating_sub(ox).min(grid.cols.saturating_sub(1)),
            y.saturating_sub(oy).min(grid.rows.saturating_sub(1)),
        )
    }

    /// The middle of what the host's windows leave visible of the view.
    fn visible_center(&self) -> SubPt {
        let grid = self.viewport.grid();
        visible_center((grid.cols, grid.rows), &self.controller.occlusions)
    }

    /// Glides node `ix` to the visible center, unless it is within three cells of it.
    fn center(&mut self, ix: NodeIx, out: &mut Vec<Outcome>) {
        let Some(anchor) = self.viewport.cell(ix) else {
            return;
        };
        if let Some(total) = pan_to(anchor, self.visible_center()) {
            self.controller.pan = Some(PanGlide::new(total));
            out.push(Outcome::ViewChanged);
        }
    }

    /// Selects the node after (or before) the selected one in sort order, wrapping, or the
    /// first (or last) with none selected, and centers it.
    fn cycle(&mut self, forward: bool, out: &mut Vec<Outcome>) {
        let order = self.store.nodes_in_order();
        let len = order.len();
        if len == 0 {
            return;
        }
        let at = self
            .controller
            .selected
            .and_then(|s| order.iter().position(|&ix| ix == s));
        let next = match (at, forward) {
            (None, true) => 0,
            (None, false) => len - 1,
            (Some(i), true) => (i + 1) % len,
            (Some(i), false) => (i + len - 1) % len,
        };
        let ix = order[next];
        self.choose(Some(ix), None, out);
        self.center(ix, out);
    }

    /// Selects the nearest node in `dir` from the selected one, or from the visible center
    /// with none selected, and centers it; reports the boundary when there is none.
    fn nav_to(&mut self, dir: Dir, out: &mut Vec<Outcome>) {
        let from = self
            .controller
            .selected
            .and_then(|ix| self.viewport.cell(ix))
            .unwrap_or_else(|| self.visible_center().cell());
        let (nodes, cells): (Vec<NodeIx>, Vec<CellPt>) = self
            .store
            .nodes_in_order()
            .iter()
            .filter_map(|&ix| Some((ix, self.viewport.cell(ix)?)))
            .unzip();
        let aspect = self.viewport.grid().cell_aspect;
        match nearest_toward(from, dir, &cells, aspect) {
            Some(i) => {
                self.choose(Some(nodes[i]), None, out);
                self.center(nodes[i], out);
            }
            None => out.push(Outcome::Boundary(dir)),
        }
    }

    /// Pins the selected node where it is drawn, or unpins it.
    fn toggle_pin(&mut self, out: &mut Vec<Outcome>) {
        let Some(ix) = self.controller.selected else {
            return;
        };
        let Some(key) = self.store.key(ix).cloned() else {
            return;
        };
        let pinned = self.store.node(ix).is_some_and(|n| n.spec.pinned.is_some());
        let pin = if pinned {
            None
        } else {
            let drawn = self.viewport.cell(ix).map(SubPt::from);
            let Some(world) = drawn.and_then(|at| self.viewport.world_at(at)) else {
                return;
            };
            Some(world)
        };
        if self
            .update(|tx| tx.set_node(&key, |n| n.pinned = pin))
            .is_ok()
        {
            out.push(Outcome::Pinned(ix, !pinned));
        }
    }
}

#[cfg(test)]
#[path = "controller_tests.rs"]
pub(super) mod tests;
