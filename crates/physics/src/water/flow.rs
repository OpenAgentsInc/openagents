//! Currents: a coarse grid of horizontal velocities, and a build-time
//! generator of divergence-free potential flow along a river.
//!
//! The generator solves Laplace's equation for the stream function `ψ` on
//! the grid (Batchelor, *An Introduction to Fluid Dynamics*, 1967, §2.2 and
//! §2.7): `ψ` is 0 on the right bank and the discharge `Q` on the left
//! bank (nodes outside hold the profile extended past the bank), at a constant on each obstacle, and at the uniform
//! lateral profile across both ends; interior nodes relax by successive
//! over-relaxation. The velocity is the curl of `ψ`, `u = (∂ψ/∂z,
//! −∂ψ/∂x)`, taken with central differences, so the discrete divergence
//! of the field is zero to rounding away from the banks, and the flux between
//! the banks is `Q` at every cross section: the water speeds up where the
//! river narrows and slows where it widens, and parts around rocks.

use glam::DVec2;
use serde::{Deserialize, Serialize};

use super::body::Course;

/// Horizontal velocities at the nodes of a regular grid in the (x, z)
/// plane, sampled bilinearly. Outside the grid the water is still.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FlowGrid {
    /// The node at index (0, 0), m.
    pub origin: DVec2,
    /// Node spacing, m.
    pub cell: f64,
    /// Nodes along x and z.
    pub nx: usize,
    pub nz: usize,
    /// Row-major by z: node (i, j) is `velocity[j * nx + i]`, m/s.
    pub velocity: Vec<DVec2>,
}

/// A round obstacle in a river, such as a rock or a pier.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Obstacle {
    pub center: DVec2,
    pub radius: f64,
}

impl FlowGrid {
    /// A grid of still water covering `min` to `max`.
    #[must_use]
    pub fn still(min: DVec2, max: DVec2, cell: f64) -> Self {
        let nx = ((max.x - min.x) / cell).ceil() as usize + 1;
        let nz = ((max.y - min.y) / cell).ceil() as usize + 1;
        Self {
            origin: min,
            cell,
            nx,
            nz,
            velocity: vec![DVec2::ZERO; nx * nz],
        }
    }

    /// A uniform current everywhere on the grid.
    #[must_use]
    pub fn uniform(min: DVec2, max: DVec2, cell: f64, velocity: DVec2) -> Self {
        let mut grid = Self::still(min, max, cell);
        grid.velocity.fill(velocity);
        grid
    }

    /// The node at (i, j).
    #[must_use]
    pub fn node(&self, i: usize, j: usize) -> DVec2 {
        self.velocity[j * self.nx + i]
    }

    /// The node's position, m.
    #[must_use]
    pub fn position(&self, i: usize, j: usize) -> DVec2 {
        self.origin + DVec2::new(i as f64, j as f64) * self.cell
    }

    /// The current at `p`, bilinear between nodes.
    #[must_use]
    pub fn sample(&self, p: DVec2) -> DVec2 {
        let g = (p - self.origin) / self.cell;
        if g.x < 0.0
            || g.y < 0.0
            || g.x > (self.nx - 1) as f64
            || g.y > (self.nz - 1) as f64
            || self.nx < 2
            || self.nz < 2
        {
            return DVec2::ZERO;
        }
        let i = (g.x.floor() as usize).min(self.nx - 2);
        let j = (g.y.floor() as usize).min(self.nz - 2);
        let (fx, fz) = (g.x - i as f64, g.y - j as f64);
        let a = self.node(i, j).lerp(self.node(i + 1, j), fx);
        let b = self.node(i, j + 1).lerp(self.node(i + 1, j + 1), fx);
        a.lerp(b, fz)
    }

    /// The central-difference divergence at an interior node, 1/s.
    #[must_use]
    pub fn divergence(&self, i: usize, j: usize) -> f64 {
        let h2 = 2.0 * self.cell;
        (self.node(i + 1, j).x - self.node(i - 1, j).x) / h2
            + (self.node(i, j + 1).y - self.node(i, j - 1).y) / h2
    }

    /// Potential flow along `course` around `obstacles`, on a grid of
    /// `cell` spacing, carrying `speed` m/s on average across the course's
    /// first width (the discharge per unit depth is `speed` times that
    /// width).
    #[must_use]
    pub fn river(course: &Course, obstacles: &[Obstacle], cell: f64, speed: f64) -> Self {
        let reach = course.widths.iter().fold(0.0f64, |m, w| m.max(*w)) * 0.5 + 2.0 * cell;
        let (lo, hi) = course.points.iter().fold(
            (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY)),
            |(lo, hi), p| (lo.min(*p), hi.max(*p)),
        );
        let mut grid = Self::still(lo - reach, hi + reach, cell);
        let discharge = speed * course.widths[0];
        let (nx, nz) = (grid.nx, grid.nz);
        let mut psi = vec![0.0; nx * nz];
        let mut free = vec![false; nx * nz];
        for j in 0..nz {
            for i in 0..nx {
                let p = grid.position(i, j);
                let station = course.locate(p);
                // The uniform lateral profile, extended linearly past the
                // banks so the zero and full streamlines fall on the banks
                // themselves rather than on the nearest nodes outside.
                let lateral = |offset: f64| {
                    discharge * (offset + station.half_width) / (2.0 * station.half_width)
                };
                let k = j * nx + i;
                psi[k] = lateral(station.offset);
                let edge = i == 0 || j == 0 || i == nx - 1 || j == nz - 1;
                let inside = !station.beyond && station.offset.abs() < station.half_width;
                if let Some(rock) = obstacles.iter().find(|o| p.distance(o.center) <= o.radius) {
                    // A streamline wraps the rock: hold it at the value the
                    // undisturbed flow has at its center.
                    let at = course.locate(rock.center);
                    psi[k] = discharge
                        * ((at.offset + at.half_width) / (2.0 * at.half_width)).clamp(0.0, 1.0);
                } else {
                    free[k] = inside && !edge;
                }
            }
        }
        // Successive over-relaxation in a fixed order, so the result is
        // the same on every machine.
        let tolerance = discharge.abs().max(1e-12) * 1e-10;
        for _ in 0..20_000 {
            let mut change: f64 = 0.0;
            for j in 1..nz - 1 {
                for i in 1..nx - 1 {
                    let k = j * nx + i;
                    if !free[k] {
                        continue;
                    }
                    let average = 0.25 * (psi[k - 1] + psi[k + 1] + psi[k - nx] + psi[k + nx]);
                    let delta = 1.85 * (average - psi[k]);
                    psi[k] += delta;
                    change = change.max(delta.abs());
                }
            }
            if change < tolerance {
                break;
            }
        }
        let h2 = 2.0 * cell;
        for j in 1..nz - 1 {
            for i in 1..nx - 1 {
                let k = j * nx + i;
                grid.velocity[k] = DVec2::new(
                    (psi[k + nx] - psi[k - nx]) / h2,
                    -(psi[k + 1] - psi[k - 1]) / h2,
                );
            }
        }
        // Nodes just past a bank take their free neighbors' mean, so a
        // bilinear sample near the bank carries the full stream, not half.
        let mut banked = grid.velocity.clone();
        for j in 1..nz - 1 {
            for i in 1..nx - 1 {
                let k = j * nx + i;
                if free[k] {
                    continue;
                }
                let around = [k - 1, k + 1, k - nx, k + nx];
                let n = around.iter().filter(|&&a| free[a]).count();
                if n > 0 {
                    banked[k] = around
                        .iter()
                        .filter(|&&a| free[a])
                        .fold(DVec2::ZERO, |s, &a| s + grid.velocity[a])
                        / n as f64;
                }
            }
        }
        grid.velocity = banked;
        grid
    }
}
