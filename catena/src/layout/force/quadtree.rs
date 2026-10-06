//! Barnes-Hut quadtree for O(n log n) repulsive force approximation.
//!
//! The standard Fruchterman-Reingold all-pairs repulsion is O(n^2) per iteration.
//! For large graphs (hundreds of nodes), this dominates layout time. The quadtree
//! groups distant nodes into clusters whose aggregate repulsive effect can be
//! approximated as a single body, reducing per-iteration cost to O(n log n).
//! The force engine uses it only above `bh_threshold` nodes (plan §8.1), because
//! below about a thousand points the tree costs more than brute force.
//!
//! Every body keeps its true position for centre of mass and force. Separating
//! coincident points nudges only the position the tree *routes* the body by, so
//! the nudge changes the tree's shape and never its physics, and with θ = 0 the
//! tree agrees with the brute-force sum (plan §16.2-M).
//!
//! Reference: Barnes & Hut, "A Hierarchical O(N log N) Force-Calculation Algorithm",
//! Nature 324 (1986).

/// Axis-aligned bounding box for a quadtree cell.
#[derive(Debug, Clone, Copy)]
struct Bounds {
    x_min: f64,
    y_min: f64,
    x_max: f64,
    y_max: f64,
}

impl Bounds {
    fn width(&self) -> f64 {
        self.x_max - self.x_min
    }

    fn height(&self) -> f64 {
        self.y_max - self.y_min
    }

    /// Return the quadrant index (0=NW, 1=NE, 2=SW, 3=SE) for a point.
    fn quadrant(&self, x: f64, y: f64) -> usize {
        let mid_x = f64::midpoint(self.x_min, self.x_max);
        let mid_y = f64::midpoint(self.y_min, self.y_max);
        let east = x > mid_x;
        let south = y > mid_y;
        match (south, east) {
            (false, false) => 0, // NW
            (false, true) => 1,  // NE
            (true, false) => 2,  // SW
            (true, true) => 3,   // SE
        }
    }

    fn subdivide(&self) -> [Bounds; 4] {
        let mid_x = f64::midpoint(self.x_min, self.x_max);
        let mid_y = f64::midpoint(self.y_min, self.y_max);
        [
            Bounds {
                x_min: self.x_min,
                y_min: self.y_min,
                x_max: mid_x,
                y_max: mid_y,
            },
            Bounds {
                x_min: mid_x,
                y_min: self.y_min,
                x_max: self.x_max,
                y_max: mid_y,
            },
            Bounds {
                x_min: self.x_min,
                y_min: mid_y,
                x_max: mid_x,
                y_max: self.y_max,
            },
            Bounds {
                x_min: mid_x,
                y_min: mid_y,
                x_max: self.x_max,
                y_max: self.y_max,
            },
        ]
    }

    /// Moves a routing point `EPSILON` toward this cell's centre on each axis, clamped into the
    /// cell, so a jittered point stays inside the bounds it was routed by. (The seed added
    /// `EPSILON` unconditionally, so a point on the max edge left its cell, and a coincident pair
    /// there nested 54 levels deep before floating-point midpoints happened to split it.)
    fn nudge_inward(&self, (x, y): (f64, f64)) -> (f64, f64) {
        (
            nudge_axis(x, self.x_min, self.x_max),
            nudge_axis(y, self.y_min, self.y_max),
        )
    }
}

fn nudge_axis(v: f64, lo: f64, hi: f64) -> f64 {
    let nudged = if v < f64::midpoint(lo, hi) {
        v + EPSILON
    } else {
        v - EPSILON
    };
    nudged.clamp(lo, hi)
}

/// A single point body in the quadtree.
#[derive(Debug, Clone, Copy)]
struct Body {
    /// True position: the only one centre of mass and force ever see.
    x: f64,
    y: f64,
    /// Where the tree files the body. Equal to the true position unless a coincident neighbour
    /// forced a nudge (see [`Bounds::nudge_inward`]).
    route: (f64, f64),
}

impl Body {
    /// FR repulsion `k² / d` from this body on a point at `(x, y)`, with `d` floored at 0.01.
    /// A point within `EPSILON` of the body is the body itself, or coincident with it: no
    /// direction exists, so it feels nothing.
    fn repulsion_on(&self, x: f64, y: f64, k: f64) -> (f64, f64) {
        let dx = x - self.x;
        let dy = y - self.y;
        let dist_sq = dx * dx + dy * dy;
        if dist_sq < EPSILON * EPSILON {
            return (0.0, 0.0);
        }
        let dist = dist_sq.sqrt().max(0.01);
        let force = (k * k) / dist;
        ((dx / dist) * force, (dy / dist) * force)
    }
}

/// Barnes-Hut quadtree for spatial partitioning of 2D point masses.
///
/// Each node stores the aggregate center of mass and total mass (body count)
/// for all bodies in its spatial region. Leaf nodes hold at most one body;
/// inserting a second body triggers subdivision into four quadrants, except at
/// `MAX_DEPTH`, where a leaf keeps every body routed to it.
pub(crate) struct QuadTree {
    bounds: Bounds,
    /// Single body stored in a leaf node.
    body: Option<Body>,
    /// Every body routed to a leaf at `MAX_DEPTH`. Kept individually, so the leaf sums their
    /// forces exactly instead of treating a point as repelled by its own cell's mass.
    bucket: Vec<Body>,
    /// Aggregate center of mass x-coordinate.
    cx: f64,
    /// Aggregate center of mass y-coordinate.
    cy: f64,
    /// Total number of bodies in this subtree (each body has mass 1).
    total_mass: f64,
    /// Four children: NW, NE, SW, SE. `None` for leaf nodes.
    children: Option<Box<[QuadTree; 4]>>,
}

/// Small offset to separate coincident points during subdivision, and the radius within which
/// two points count as coincident for force.
const EPSILON: f64 = 1e-4;

/// Maximum subdivision depth — prevents stack overflow from pathological inputs
/// where many bodies share (nearly) the same position.
const MAX_DEPTH: u32 = 64;

impl QuadTree {
    /// Create an empty quadtree covering the given bounding box.
    pub(crate) fn new(x_min: f64, y_min: f64, x_max: f64, y_max: f64) -> Self {
        Self {
            bounds: Bounds {
                x_min,
                y_min,
                x_max,
                y_max,
            },
            body: None,
            bucket: Vec::new(),
            cx: 0.0,
            cy: 0.0,
            total_mass: 0.0,
            children: None,
        }
    }

    /// Insert a body at position (x, y) with mass 1.
    pub(crate) fn insert(&mut self, x: f64, y: f64) {
        self.insert_depth(
            Body {
                x,
                y,
                route: (x, y),
            },
            0,
        );
    }

    fn add_mass(&mut self, body: Body) {
        let new_mass = self.total_mass + 1.0;
        self.cx = (self.cx * self.total_mass + body.x) / new_mass;
        self.cy = (self.cy * self.total_mass + body.y) / new_mass;
        self.total_mass = new_mass;
    }

    fn insert_depth(&mut self, body: Body, depth: u32) {
        // Safety valve: stop subdividing at extreme depth (coincident points).
        if depth >= MAX_DEPTH {
            self.add_mass(body);
            self.bucket.push(body);
            return;
        }

        if self.total_mass == 0.0 {
            // Empty leaf: store the body directly.
            self.body = Some(body);
            self.cx = body.x;
            self.cy = body.y;
            self.total_mass = 1.0;
            return;
        }

        self.add_mass(body);

        if let Some(mut existing) = self.body.take() {
            // Leaf with one body: subdivide and re-insert both.
            self.subdivide_node();
            // Nudge a coincident routing point so the pair separates into different quadrants
            // instead of recursing to MAX_DEPTH.
            let (ex, ey) = existing.route;
            let (x, y) = body.route;
            if (ex - x).abs() < f64::EPSILON && (ey - y).abs() < f64::EPSILON {
                existing.route = self.bounds.nudge_inward(existing.route);
            }
            self.insert_into_child(existing, depth);
            self.insert_into_child(body, depth);
        } else if self.children.is_some() {
            // Internal node: route into the appropriate child.
            self.insert_into_child(body, depth);
        }
    }

    fn subdivide_node(&mut self) {
        let [nw, ne, sw, se] = self.bounds.subdivide();
        let child = |b: Bounds| QuadTree::new(b.x_min, b.y_min, b.x_max, b.y_max);
        self.children = Some(Box::new([child(nw), child(ne), child(sw), child(se)]));
    }

    fn insert_into_child(&mut self, body: Body, depth: u32) {
        if let Some(children) = &mut self.children {
            let (x, y) = body.route;
            let idx = self.bounds.quadrant(x, y);
            children[idx].insert_depth(body, depth + 1);
        }
    }

    /// Compute the repulsive force on a body at (x, y) from this subtree.
    ///
    /// `theta` controls the accuracy/speed tradeoff (0.0 = exact, higher = faster).
    /// A cell is approximated as a point mass when `cell_size / distance < theta`.
    /// `k` is the Fruchterman-Reingold optimal distance parameter.
    pub(crate) fn compute_force(&self, x: f64, y: f64, theta: f64, k: f64) -> (f64, f64) {
        if self.total_mass == 0.0 {
            return (0.0, 0.0);
        }

        let Some(children) = &self.children else {
            // Leaf: its one body, or its MAX_DEPTH bucket, summed exactly.
            return self
                .body
                .iter()
                .chain(&self.bucket)
                .map(|b| b.repulsion_on(x, y, k))
                .fold((0.0, 0.0), |(fx, fy), (bx, by)| (fx + bx, fy + by));
        };

        let dx = x - self.cx;
        let dy = y - self.cy;
        let dist = (dx * dx + dy * dy).sqrt().max(0.01);
        let cell_size = self.bounds.width().max(self.bounds.height());

        if cell_size / dist < theta {
            // Approximate: treat entire subtree as one point mass.
            // FR repulsive force: f = k^2 / d, scaled by mass (body count).
            let force = self.total_mass * (k * k) / dist;
            return ((dx / dist) * force, (dy / dist) * force);
        }

        // Cell too close — recurse into children for accuracy.
        let mut fx = 0.0;
        let mut fy = 0.0;
        for child in children.iter() {
            let (cfx, cfy) = child.compute_force(x, y, theta, k);
            fx += cfx;
            fy += cfy;
        }
        (fx, fy)
    }
}

#[cfg(test)]
#[path = "quadtree_tests.rs"]
mod tests;
