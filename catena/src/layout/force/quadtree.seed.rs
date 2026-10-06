//! Barnes-Hut quadtree for O(n log n) repulsive force approximation.
//!
//! The standard Fruchterman-Reingold all-pairs repulsion is O(n^2) per iteration.
//! For large graphs (hundreds of nodes), this dominates layout time. The quadtree
//! groups distant nodes into clusters whose aggregate repulsive effect can be
//! approximated as a single body, reducing per-iteration cost to O(n log n).
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
        let mid_x = (self.x_min + self.x_max) / 2.0;
        let mid_y = (self.y_min + self.y_max) / 2.0;
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
        let mid_x = (self.x_min + self.x_max) / 2.0;
        let mid_y = (self.y_min + self.y_max) / 2.0;
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
}

/// A single point body in the quadtree.
#[derive(Debug, Clone, Copy)]
struct Body {
    x: f64,
    y: f64,
}

/// Barnes-Hut quadtree for spatial partitioning of 2D point masses.
///
/// Each node stores the aggregate center of mass and total mass (body count)
/// for all bodies in its spatial region. Leaf nodes hold at most one body;
/// inserting a second body triggers subdivision into four quadrants.
pub(crate) struct QuadTree {
    bounds: Bounds,
    /// Single body stored in a leaf node.
    body: Option<Body>,
    /// Aggregate center of mass x-coordinate.
    cx: f64,
    /// Aggregate center of mass y-coordinate.
    cy: f64,
    /// Total number of bodies in this subtree (each body has mass 1).
    total_mass: f64,
    /// Four children: NW, NE, SW, SE. `None` for leaf nodes.
    children: Option<Box<[QuadTree; 4]>>,
}

/// Small offset to separate coincident points during subdivision.
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
            cx: 0.0,
            cy: 0.0,
            total_mass: 0.0,
            children: None,
        }
    }

    /// Insert a body at position (x, y) with mass 1.
    pub(crate) fn insert(&mut self, x: f64, y: f64) {
        self.insert_depth(x, y, 0);
    }

    fn insert_depth(&mut self, x: f64, y: f64, depth: u32) {
        // Safety valve: stop subdividing at extreme depth (coincident points).
        if depth >= MAX_DEPTH {
            let new_mass = self.total_mass + 1.0;
            self.cx = (self.cx * self.total_mass + x) / new_mass;
            self.cy = (self.cy * self.total_mass + y) / new_mass;
            self.total_mass = new_mass;
            return;
        }

        if self.total_mass == 0.0 {
            // Empty leaf: store the body directly.
            self.body = Some(Body { x, y });
            self.cx = x;
            self.cy = y;
            self.total_mass = 1.0;
            return;
        }

        // Update aggregate center of mass.
        let new_mass = self.total_mass + 1.0;
        self.cx = (self.cx * self.total_mass + x) / new_mass;
        self.cy = (self.cy * self.total_mass + y) / new_mass;
        self.total_mass = new_mass;

        if let Some(existing) = self.body.take() {
            // Leaf with one body: subdivide and re-insert both.
            self.subdivide_node();
            // Jitter coincident points so they separate into different quadrants
            // instead of recursing to MAX_DEPTH.
            let mut ex = existing.x;
            let mut ey = existing.y;
            if (ex - x).abs() < f64::EPSILON && (ey - y).abs() < f64::EPSILON {
                ex += EPSILON;
                ey += EPSILON;
            }
            self.insert_into_child(ex, ey, depth);
            self.insert_into_child(x, y, depth);
        } else if self.children.is_some() {
            // Internal node: route into the appropriate child.
            self.insert_into_child(x, y, depth);
        }
    }

    fn subdivide_node(&mut self) {
        let quads = self.bounds.subdivide();
        self.children = Some(Box::new([
            QuadTree::new(
                quads[0].x_min,
                quads[0].y_min,
                quads[0].x_max,
                quads[0].y_max,
            ),
            QuadTree::new(
                quads[1].x_min,
                quads[1].y_min,
                quads[1].x_max,
                quads[1].y_max,
            ),
            QuadTree::new(
                quads[2].x_min,
                quads[2].y_min,
                quads[2].x_max,
                quads[2].y_max,
            ),
            QuadTree::new(
                quads[3].x_min,
                quads[3].y_min,
                quads[3].x_max,
                quads[3].y_max,
            ),
        ]));
    }

    fn insert_into_child(&mut self, x: f64, y: f64, depth: u32) {
        if let Some(children) = &mut self.children {
            let idx = self.bounds.quadrant(x, y);
            children[idx].insert_depth(x, y, depth + 1);
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

        let dx = x - self.cx;
        let dy = y - self.cy;
        let dist_sq = dx * dx + dy * dy;

        // Skip self-interaction: leaf body at (nearly) the same position as query.
        if self.children.is_none() && self.body.is_some() && dist_sq < EPSILON * EPSILON {
            return (0.0, 0.0);
        }

        let dist = dist_sq.sqrt().max(0.01);
        let cell_size = self.bounds.width().max(self.bounds.height());

        let is_leaf = self.children.is_none();
        let far_enough = cell_size / dist < theta;

        if is_leaf || far_enough {
            // Approximate: treat entire subtree as one point mass.
            // FR repulsive force: f = k^2 / d, scaled by mass (body count).
            let force = self.total_mass * (k * k) / dist;
            return ((dx / dist) * force, (dy / dist) * force);
        }

        // Cell too close — recurse into children for accuracy.
        let mut fx = 0.0;
        let mut fy = 0.0;
        if let Some(children) = &self.children {
            for child in children.iter() {
                let (cfx, cfy) = child.compute_force(x, y, theta, k);
                fx += cfx;
                fy += cfy;
            }
        }
        (fx, fy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn center_of_mass_single_body() {
        let mut tree = QuadTree::new(0.0, 0.0, 100.0, 100.0);
        tree.insert(25.0, 75.0);
        assert_eq!(tree.total_mass, 1.0);
        assert!((tree.cx - 25.0).abs() < 1e-10);
        assert!((tree.cy - 75.0).abs() < 1e-10);
    }

    #[test]
    fn center_of_mass_two_bodies() {
        let mut tree = QuadTree::new(0.0, 0.0, 100.0, 100.0);
        tree.insert(20.0, 40.0);
        tree.insert(80.0, 60.0);
        assert_eq!(tree.total_mass, 2.0);
        assert!((tree.cx - 50.0).abs() < 1e-10);
        assert!((tree.cy - 50.0).abs() < 1e-10);
    }

    #[test]
    fn center_of_mass_three_bodies() {
        let mut tree = QuadTree::new(0.0, 0.0, 100.0, 100.0);
        tree.insert(10.0, 10.0);
        tree.insert(20.0, 20.0);
        tree.insert(30.0, 30.0);
        assert_eq!(tree.total_mass, 3.0);
        assert!((tree.cx - 20.0).abs() < 1e-10);
        assert!((tree.cy - 20.0).abs() < 1e-10);
    }

    #[test]
    fn force_pushes_bodies_apart() {
        let mut tree = QuadTree::new(0.0, 0.0, 100.0, 100.0);
        tree.insert(40.0, 50.0);
        tree.insert(60.0, 50.0);

        let k = 10.0;
        let theta = 0.0; // exact calculation

        // Force on a body at (40, 50) should push it leftward (away from 60, 50).
        let (fx, fy) = tree.compute_force(40.0, 50.0, theta, k);
        assert!(fx < 0.0, "body at x=40 should be pushed left, got fx={fx}");
        assert!(fy.abs() < 1e-6, "no vertical force expected, got fy={fy}");

        // Force on a body at (60, 50) should push it rightward.
        let (fx, fy) = tree.compute_force(60.0, 50.0, theta, k);
        assert!(fx > 0.0, "body at x=60 should be pushed right, got fx={fx}");
        assert!(fy.abs() < 1e-6, "no vertical force expected, got fy={fy}");
    }

    #[test]
    fn force_magnitude_decreases_with_distance() {
        let mut tree = QuadTree::new(0.0, 0.0, 200.0, 200.0);
        tree.insert(100.0, 100.0);

        let k = 10.0;
        let theta = 0.0;

        let (fx_near, _) = tree.compute_force(110.0, 100.0, theta, k);
        let (fx_far, _) = tree.compute_force(150.0, 100.0, theta, k);
        assert!(
            fx_near.abs() > fx_far.abs(),
            "near force ({}) should exceed far force ({})",
            fx_near.abs(),
            fx_far.abs()
        );
    }

    #[test]
    fn identical_positions_no_panic() {
        let mut tree = QuadTree::new(0.0, 0.0, 100.0, 100.0);
        // Insert multiple bodies at the exact same position.
        for _ in 0..10 {
            tree.insert(50.0, 50.0);
        }
        assert_eq!(tree.total_mass, 10.0);

        // Force computation should not panic or return NaN/Infinity.
        let (fx, fy) = tree.compute_force(50.0, 50.0, 0.8, 10.0);
        assert!(fx.is_finite(), "fx should be finite, got {fx}");
        assert!(fy.is_finite(), "fy should be finite, got {fy}");
    }

    #[test]
    fn theta_approximation_reasonable() {
        // Compare exact (theta=0) vs approximate (theta=0.8) for a distant cluster.
        let mut tree = QuadTree::new(0.0, 0.0, 1000.0, 1000.0);
        for i in 0..50 {
            tree.insert(100.0 + (i as f64) * 2.0, 100.0 + (i as f64) * 2.0);
        }

        let k = 20.0;
        let query = (800.0, 800.0);

        let (ex, ey) = tree.compute_force(query.0, query.1, 0.0, k);
        let (ax, ay) = tree.compute_force(query.0, query.1, 0.8, k);

        // Approximation should have the same sign and similar magnitude.
        assert!(
            ex.signum() == ax.signum(),
            "force x signs differ: exact={ex}, approx={ax}"
        );
        assert!(
            ey.signum() == ay.signum(),
            "force y signs differ: exact={ey}, approx={ay}"
        );

        let exact_mag = (ex * ex + ey * ey).sqrt();
        let approx_mag = (ax * ax + ay * ay).sqrt();
        let ratio = approx_mag / exact_mag;
        assert!(
            ratio > 0.5 && ratio < 2.0,
            "approximation ratio {ratio:.2} out of range (exact={exact_mag:.2}, approx={approx_mag:.2})"
        );
    }

    #[test]
    fn empty_tree_zero_force() {
        let tree = QuadTree::new(0.0, 0.0, 100.0, 100.0);
        let (fx, fy) = tree.compute_force(50.0, 50.0, 0.8, 10.0);
        assert_eq!(fx, 0.0);
        assert_eq!(fy, 0.0);
    }
}
