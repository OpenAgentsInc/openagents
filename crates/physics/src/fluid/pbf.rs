//! A deterministic position-based fluid in `f64` (Macklin and Müller 2013),
//! with boundary particles and two-way rigid coupling (Akinci et al. 2012)
//! and optional surface tension and adhesion (Akinci et al. 2013).
//!
//! Every step:
//!
//! 1. External accelerations (gravity, surface tension, adhesion) update
//!    velocities, and positions are predicted.
//! 2. A sorted spatial hash finds each particle's fluid and boundary
//!    neighbours within the kernel radius `h`.
//! 3. A few Jacobi iterations solve the density constraint
//!    `C_i = rho_i / rho_0 - 1 >= 0`: each particle's multiplier `lambda_i`,
//!    then its position correction, with the `s_corr` tensile term.
//!    Boundary particles add density with their volume weights (`psi`), and
//!    the correction they cause is the force the liquid puts on a coupled
//!    body.
//! 4. Velocities come from the position change; XSPH smooths them.
//!
//! Every per-particle pass is a gather that reads only the previous pass,
//! so splitting it over threads changes nothing, and neighbours are visited
//! in a fixed order: a run is bit for bit the same on any thread count.
//! The pressure each particle carries ([`Fluid::pressure`]) is recovered
//! from the multipliers, `p_i = -kappa rho_0 sum(lambda_i) / dt^2`, where
//! `kappa` corrects the kernel-gradient operator on a full lattice.

use std::f64::consts::PI;

use glam::{DQuat, DVec3};

use crate::{BodyId, World, parallel};

/// Liquid parameters. Lengths in metres, densities in kg/m^3.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// Particle radius `r`. Particles start `2r` apart, and the kernel
    /// radius is `h = 4r`, about 30 neighbours at rest.
    pub radius: f64,
    /// Rest density `rho_0`.
    pub rest_density: f64,
    /// Step length, s.
    pub dt: f64,
    /// Density iterations per step.
    pub iterations: u32,
    /// Constraint softening, as a fraction of a full neighbourhood's
    /// gradient sum. Small values keep the liquid stiff.
    pub relaxation: f64,
    /// XSPH viscosity: the fraction of the neighbourhood's mean velocity
    /// each particle takes per step.
    pub viscosity: f64,
    /// XSPH drag toward boundary velocities (a no-slip wall at 1); zero
    /// lets the liquid slide freely.
    pub boundary_viscosity: f64,
    /// Akinci cohesion and curvature coefficient `gamma`; zero turns
    /// surface tension off.
    pub surface_tension: f64,
    /// Akinci adhesion to boundaries `beta`; zero turns it off.
    pub adhesion: f64,
    /// Tensile term strength `k` of `s_corr = -k (W(r)/W(0.2h))^4`, as a
    /// fraction of a full neighbourhood's multiplier scale.
    pub tensile: f64,
    /// Uniform acceleration, m/s^2.
    pub gravity: DVec3,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            radius: 0.00625,
            rest_density: 1000.0,
            dt: 1.0 / 240.0,
            iterations: 6,
            relaxation: 1e-4,
            viscosity: 0.02,
            boundary_viscosity: 0.0,
            surface_tension: 0.0,
            adhesion: 0.0,
            tensile: 0.002,
            gravity: DVec3::new(0.0, -9.81, 0.0),
        }
    }
}

/// An open-topped box the liquid stays in: its floor and four walls are
/// boundary particles, and particles are also kept inside it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tank {
    pub min: DVec3,
    pub max: DVec3,
}

/// Statistics the CPU and GPU solvers are compared on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stats {
    /// Mean of `|rho_i / rho_0 - 1|` over particles more than `h` below
    /// the fill height (whose neighbourhoods are full).
    pub density_error: f64,
    /// Fill height above the tank floor: the liquid's volume over the
    /// floor area, as a settled liquid would stand.
    pub fill_height: f64,
    /// The 95th percentile of particle heights above the floor, plus `r`.
    pub surface: f64,
    pub center_of_mass: DVec3,
    /// Mean particle speed, m/s.
    pub mean_speed: f64,
}

/// A rigid body the liquid pushes and is pushed by: boundary particles on
/// the surface of a box collider, in the body frame.
#[derive(Clone, Debug)]
struct Coupled {
    body: BodyId,
    half: DVec3,
    local: Vec<DVec3>,
    psi: Vec<f64>,
}

/// The smoothing kernels for one `h`.
#[derive(Clone, Copy, Debug)]
struct Kernel {
    h: f64,
    h2: f64,
    poly6: f64,
    spiky: f64,
    cohesion: f64,
    adhesion: f64,
}

impl Kernel {
    fn new(h: f64) -> Self {
        Self {
            h,
            h2: h * h,
            poly6: 315.0 / (64.0 * PI * h.powi(9)),
            spiky: -45.0 / (PI * h.powi(6)),
            cohesion: 32.0 / (PI * h.powi(9)),
            adhesion: 0.007 / h.powf(3.25),
        }
    }
    fn w(&self, r2: f64) -> f64 {
        if r2 >= self.h2 {
            0.0
        } else {
            let d = self.h2 - r2;
            self.poly6 * d * d * d
        }
    }
    /// Spiky gradient at `x = x_i - x_j`.
    fn grad(&self, x: DVec3) -> DVec3 {
        let r = x.length();
        if r >= self.h || r <= 1e-12 {
            DVec3::ZERO
        } else {
            let d = self.h - r;
            x * (self.spiky * d * d / r)
        }
    }
    /// Akinci 2013 cohesion spline.
    fn cohesion(&self, r: f64) -> f64 {
        let h = self.h;
        if r >= h || r <= 0.0 {
            0.0
        } else {
            let a = (h - r).powi(3) * r.powi(3);
            if 2.0 * r > h {
                self.cohesion * a
            } else {
                self.cohesion * (2.0 * a - h.powi(6) / 64.0)
            }
        }
    }
    /// Akinci 2013 adhesion spline.
    fn adhesion(&self, r: f64) -> f64 {
        let h = self.h;
        if r >= h || 2.0 * r <= h {
            0.0
        } else {
            self.adhesion * (-4.0 * r * r / h + 6.0 * r - 2.0 * h).max(0.0).powf(0.25)
        }
    }
}

/// A sorted spatial hash: points sorted by their cell, with each occupied
/// cell's run, so queries visit points in a fixed order.
#[derive(Clone, Debug, Default)]
struct Grid {
    cell: f64,
    order: Vec<u32>,
    cells: Vec<(u64, u32, u32)>,
}

fn key(c: [i64; 3]) -> u64 {
    let f = |v: i64| ((v + (1 << 20)) as u64) & ((1 << 21) - 1);
    (f(c[0]) << 42) | (f(c[1]) << 21) | f(c[2])
}

impl Grid {
    fn coord(&self, p: DVec3) -> [i64; 3] {
        let c = (p / self.cell).floor();
        [c.x as i64, c.y as i64, c.z as i64]
    }
    fn build(cell: f64, points: &[DVec3]) -> Self {
        let mut grid = Self {
            cell,
            ..Self::default()
        };
        let mut keyed: Vec<(u64, u32)> = points
            .iter()
            .enumerate()
            .map(|(i, p)| (key(grid.coord(*p)), i as u32))
            .collect();
        keyed.sort_unstable();
        grid.order = keyed.iter().map(|k| k.1).collect();
        let mut start = 0;
        while start < keyed.len() {
            let mut end = start + 1;
            while end < keyed.len() && keyed[end].0 == keyed[start].0 {
                end += 1;
            }
            grid.cells.push((keyed[start].0, start as u32, end as u32));
            start = end;
        }
        grid
    }
    /// Points within `h` of `p`, in cell then index order.
    fn near(&self, p: DVec3, points: &[DVec3], h2: f64, out: &mut Vec<u32>) {
        out.clear();
        let c = self.coord(p);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let k = key([c[0] + dx, c[1] + dy, c[2] + dz]);
                    if let Ok(at) = self.cells.binary_search_by_key(&k, |e| e.0) {
                        let (_, s, e) = self.cells[at];
                        for &j in &self.order[s as usize..e as usize] {
                            if (points[j as usize] - p).length_squared() < h2 {
                                out.push(j);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Per-particle neighbour lists, flattened.
#[derive(Clone, Debug, Default)]
struct Neighbours {
    start: Vec<u32>,
    list: Vec<u32>,
}

impl Neighbours {
    fn of(&self, i: usize) -> &[u32] {
        &self.list[self.start[i] as usize..self.start[i + 1] as usize]
    }
    fn gather(per: Vec<Vec<u32>>) -> Self {
        let mut start = Vec::with_capacity(per.len() + 1);
        let mut list = Vec::new();
        start.push(0);
        for p in per {
            list.extend(p);
            start.push(list.len() as u32);
        }
        Self { start, list }
    }
}

/// A position-based liquid.
#[derive(Clone, Debug)]
pub struct Fluid {
    pub settings: Settings,
    kernel: Kernel,
    /// Mass per particle, calibrated so a full lattice sits at rest density.
    mass: f64,
    /// A full neighbourhood's gradient sum, the multiplier scale.
    gradient_scale: f64,
    /// The lattice gradient moment, which turns multipliers into pressure.
    gradient_moment: f64,
    pos: Vec<DVec3>,
    vel: Vec<DVec3>,
    density: Vec<f64>,
    pressure: Vec<f64>,
    tank: Option<Tank>,
    /// Fixed boundary particles (tank walls and static shapes) and their
    /// volume weights.
    walls: Vec<DVec3>,
    wall_psi: Vec<f64>,
    coupled: Vec<Coupled>,
    neighbours: Neighbours,
    /// Steps taken.
    pub tick: u64,
}

impl Fluid {
    #[must_use]
    pub fn new(settings: Settings, tank: Option<Tank>) -> Self {
        let r = settings.radius;
        let kernel = Kernel::new(4.0 * r);
        let d = 2.0 * r;
        // A full lattice neighbourhood: its kernel sum sets the mass, its
        // gradient sum the multiplier scale.
        let (mut w, mut g2, mut g, mut moment) = (0.0, 0.0, DVec3::ZERO, 0.0);
        let n = (kernel.h / d).ceil() as i32;
        for x in -n..=n {
            for y in -n..=n {
                for z in -n..=n {
                    let p = DVec3::new(f64::from(x), f64::from(y), f64::from(z)) * d;
                    w += kernel.w(p.length_squared());
                    let gr = kernel.grad(p);
                    g += gr;
                    g2 += gr.length_squared();
                    moment -= p.dot(gr);
                }
            }
        }
        let mass = settings.rest_density / w;
        let m = mass / settings.rest_density;
        let gradient_scale = m * m * (g.length_squared() + g2);
        // How far the kernel-gradient sum is from a true gradient on a full
        // lattice: `-(1/3) sum V x_ij . grad W_ij`, 1 for an exact operator.
        let gradient_moment = m * moment / 3.0;
        let mut fluid = Self {
            settings,
            kernel,
            mass,
            gradient_scale,
            gradient_moment,
            pos: Vec::new(),
            vel: Vec::new(),
            density: Vec::new(),
            pressure: Vec::new(),
            tank,
            walls: Vec::new(),
            wall_psi: Vec::new(),
            coupled: Vec::new(),
            neighbours: Neighbours::default(),
            tick: 0,
        };
        if let Some(tank) = tank {
            fluid.add_walls(tank);
        }
        fluid
    }

    /// How far a tank's boundary particles sit behind its faces. At rest
    /// the liquid's particle centres settle about `2.4 r` from a boundary
    /// layer (measured at the floor of a 0.2 m column), and a particle's
    /// volume reaches `r` past its centre, so `1.42 r` puts the liquid's
    /// edge on the face and the fill height comes out right.
    fn wall_standoff(&self) -> f64 {
        1.42 * self.settings.radius
    }

    /// The same for a coupled body's surface. Near the free surface the
    /// liquid presses less and stands further off, so a floating body's
    /// layer sits deeper, at `1.7 r`; this was fitted once at r = 6.25 mm
    /// so boxes of 250, 500 and 750 kg/m^3 float at their Archimedes draft
    /// within 3% (`tests::a_floating_box_reaches_its_archimedes_draft`).
    fn body_standoff(&self) -> f64 {
        1.7 * self.settings.radius
    }

    /// Boundary particles just outside the tank's floor and walls
    /// ([`Self::wall_standoff`]), so the liquid's edge stands on its faces.
    fn add_walls(&mut self, tank: Tank) {
        let d = 2.0 * self.settings.radius;
        let lo = tank.min - DVec3::splat(self.wall_standoff());
        let hi = tank.max + DVec3::splat(self.wall_standoff());
        let count = |a: f64, b: f64| ((b - a) / d).round().max(1.0) as i64;
        let (nx, ny, nz) = (count(lo.x, hi.x), count(lo.y, hi.y), count(lo.z, hi.z));
        let at = |i: i64, n: i64, a: f64, b: f64| a + (b - a) * i as f64 / n as f64;
        let mut points = Vec::new();
        for i in 0..=nx {
            for k in 0..=nz {
                points.push(DVec3::new(
                    at(i, nx, lo.x, hi.x),
                    lo.y,
                    at(k, nz, lo.z, hi.z),
                ));
            }
        }
        for j in 1..=ny {
            let y = at(j, ny, lo.y, hi.y);
            for i in 0..=nx {
                points.push(DVec3::new(at(i, nx, lo.x, hi.x), y, lo.z));
                points.push(DVec3::new(at(i, nx, lo.x, hi.x), y, hi.z));
            }
            for k in 1..nz {
                points.push(DVec3::new(lo.x, y, at(k, nz, lo.z, hi.z)));
                points.push(DVec3::new(hi.x, y, at(k, nz, lo.z, hi.z)));
            }
        }
        self.add_boundary(points);
    }

    /// Fixed boundary particles, such as a sampled static part. Spacing
    /// about `2r` keeps the liquid out.
    pub fn add_boundary(&mut self, points: Vec<DVec3>) {
        let psi = self.volumes(&points);
        self.walls.extend(points);
        self.wall_psi.extend(psi);
    }

    /// Akinci boundary volume weights: `rho_0 / sum_k W(x_b - x_k)` over
    /// the same boundary's particles.
    fn volumes(&self, points: &[DVec3]) -> Vec<f64> {
        let grid = Grid::build(self.kernel.h, points);
        let mut near = Vec::new();
        points
            .iter()
            .map(|p| {
                grid.near(*p, points, self.kernel.h2, &mut near);
                let sum: f64 = near
                    .iter()
                    .map(|&k| self.kernel.w((points[k as usize] - *p).length_squared()))
                    .sum();
                self.settings.rest_density / sum
            })
            .collect()
    }

    /// Couple a body with a box collider of half extents `half`: its surface
    /// gets boundary particles, and each step the liquid's push on them is
    /// applied to it as a force and torque.
    pub fn couple_box(&mut self, body: BodyId, half: DVec3) {
        let d = 2.0 * self.settings.radius;
        // Particles sit inside the faces ([`Self::body_standoff`]), so the
        // liquid's edge meets the faces.
        let skin = half;
        let half = (half - DVec3::splat(self.body_standoff())).max(DVec3::splat(1e-6));
        let mut local = Vec::new();
        let n = (half * 2.0 / d).round().max(DVec3::ONE);
        let (nx, ny, nz) = (n.x as i64, n.y as i64, n.z as i64);
        for i in 0..=nx {
            for j in 0..=ny {
                for k in 0..=nz {
                    if i == 0 || j == 0 || k == 0 || i == nx || j == ny || k == nz {
                        local.push(DVec3::new(
                            -half.x + 2.0 * half.x * i as f64 / nx as f64,
                            -half.y + 2.0 * half.y * j as f64 / ny as f64,
                            -half.z + 2.0 * half.z * k as f64 / nz as f64,
                        ));
                    }
                }
            }
        }
        let psi = self.volumes(&local);
        self.coupled.push(Coupled {
            body,
            half: skin,
            local,
            psi,
        });
    }

    /// Fill `[min, max]` with particles on a `2r` lattice, moving at `vel`.
    pub fn add_block(&mut self, min: DVec3, max: DVec3, vel: DVec3) {
        let r = self.settings.radius;
        let d = 2.0 * r;
        let n = ((max - min) / d).floor();
        for i in 0..n.x as i64 {
            for j in 0..n.y as i64 {
                for k in 0..n.z as i64 {
                    self.push(
                        min + DVec3::splat(r) + DVec3::new(i as f64, j as f64, k as f64) * d,
                        vel,
                    );
                }
            }
        }
    }

    /// Fill a ball with particles on a `2r` lattice, moving at `vel`.
    pub fn add_ball(&mut self, center: DVec3, radius: f64, vel: DVec3) {
        let d = 2.0 * self.settings.radius;
        let n = (radius / d).ceil() as i64;
        for i in -n..=n {
            for j in -n..=n {
                for k in -n..=n {
                    let p = DVec3::new(i as f64, j as f64, k as f64) * d;
                    if p.length() <= radius {
                        self.push(center + p, vel);
                    }
                }
            }
        }
    }

    /// Add one particle.
    pub fn push(&mut self, pos: DVec3, vel: DVec3) {
        self.pos.push(pos);
        self.vel.push(vel);
        self.density.push(self.settings.rest_density);
        self.pressure.push(0.0);
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.pos.len()
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pos.is_empty()
    }
    #[must_use]
    pub fn positions(&self) -> &[DVec3] {
        &self.pos
    }
    #[must_use]
    pub fn velocities(&self) -> &[DVec3] {
        &self.vel
    }
    /// Each particle's density at its last solve, kg/m^3.
    #[must_use]
    pub fn densities(&self) -> &[f64] {
        &self.density
    }
    /// Each particle's pressure from its last step's multipliers, Pa.
    #[must_use]
    pub fn pressure(&self) -> &[f64] {
        &self.pressure
    }
    /// Mass per particle, kg.
    #[must_use]
    pub fn particle_mass(&self) -> f64 {
        self.mass
    }
    /// The kernel radius `h`.
    #[must_use]
    pub fn kernel_radius(&self) -> f64 {
        self.kernel.h
    }
    #[must_use]
    pub fn tank(&self) -> Option<Tank> {
        self.tank
    }
    /// Fixed boundary particles (walls and static parts).
    #[must_use]
    pub fn boundary(&self) -> &[DVec3] {
        &self.walls
    }
    /// Each fixed boundary particle's volume weight `psi`, kg.
    #[must_use]
    pub fn boundary_volumes(&self) -> &[f64] {
        &self.wall_psi
    }
    /// The constraint softening the solve adds to each multiplier's
    /// denominator: `relaxation` times a full neighbourhood's gradient sum.
    #[must_use]
    pub fn softening(&self) -> f64 {
        self.settings.relaxation * self.gradient_scale
    }
    /// The tensile term's strength in multiplier units.
    #[must_use]
    pub fn tensile_strength(&self) -> f64 {
        self.settings.tensile / self.gradient_scale
    }

    /// Advance one step. Coupled bodies are read from `world` and receive
    /// the liquid's force and torque for its next step; step the world
    /// after the liquid, with the same `dt`.
    pub fn step(&mut self, world: Option<&mut World>) {
        let s = self.settings;
        let k = self.kernel;
        let dt = s.dt;
        let n = self.pos.len();
        let m = self.mass;
        let rho0 = s.rest_density;

        // Boundary particles this step: walls, then each coupled body's.
        let mut bpos = self.walls.clone();
        let mut bpsi = self.wall_psi.clone();
        let mut bvel = vec![DVec3::ZERO; bpos.len()];
        let mut owner: Vec<u32> = vec![u32::MAX; bpos.len()];
        let mut poses = Vec::new();
        for (c, coupled) in self.coupled.iter().enumerate() {
            let (pose, motion) = world.as_ref().map_or(
                ((DVec3::ZERO, DQuat::IDENTITY), (DVec3::ZERO, DVec3::ZERO)),
                |w| {
                    let b = &w.bodies()[coupled.body.0 as usize];
                    ((b.pos, b.orientation), (b.vel, b.omega_world()))
                },
            );
            poses.push(pose);
            for (l, psi) in coupled.local.iter().zip(&coupled.psi) {
                let arm = pose.1 * *l;
                bpos.push(pose.0 + arm);
                bvel.push(motion.0 + motion.1.cross(arm));
                bpsi.push(*psi);
                owner.push(c as u32);
            }
        }
        let bgrid = Grid::build(k.h, &bpos);

        // 1. External accelerations from the last step's neighbourhoods.
        let accel = self.forces(&bpos, &bpsi, &bgrid);
        let pos0 = self.pos.clone();
        let mut x: Vec<DVec3> = (0..n)
            .map(|i| {
                let v = self.vel[i] + accel[i] * dt;
                self.vel[i] = v;
                pos0[i] + v * dt
            })
            .collect();
        if let Some(tank) = self.tank {
            for p in &mut x {
                *p = clamp_into(*p, tank, s.radius);
            }
        }

        // 2. Neighbours at the predicted positions.
        let grid = Grid::build(k.h, &x);
        let index: Vec<usize> = (0..n).collect();
        let found = parallel::map(&index, 256, |&i| {
            let mut f = Vec::new();
            grid.near(x[i], &x, k.h2, &mut f);
            f.retain(|&j| j as usize != i);
            let mut b = Vec::new();
            bgrid.near(x[i], &bpos, k.h2, &mut b);
            (f, b)
        });
        let (fluid_near, bound_near): (Vec<_>, Vec<_>) = found.into_iter().unzip();
        let fluid_near = Neighbours::gather(fluid_near);
        let bound_near = Neighbours::gather(bound_near);

        // 3. Density iterations.
        let eps = s.relaxation * self.gradient_scale;
        let mq = m / rho0;
        let w_dq = k.w((0.2 * k.h).powi(2));
        let tensile = s.tensile * self.gradient_scale.recip();
        let mut lambda_sum = vec![0.0; n];
        let mut force = vec![(DVec3::ZERO, DVec3::ZERO); self.coupled.len()];
        let mut density = vec![rho0; n];
        for _ in 0..s.iterations.max(1) {
            let solved = parallel::map(&index, 256, |&i| {
                let xi = x[i];
                let mut rho = m * k.w(0.0);
                let mut gi = DVec3::ZERO;
                let mut g2 = 0.0;
                for &j in fluid_near.of(i) {
                    let r = xi - x[j as usize];
                    rho += m * k.w(r.length_squared());
                    let g = k.grad(r) * mq;
                    gi += g;
                    g2 += g.length_squared();
                }
                for &b in bound_near.of(i) {
                    let r = xi - bpos[b as usize];
                    rho += bpsi[b as usize] * k.w(r.length_squared());
                    gi += k.grad(r) * (bpsi[b as usize] / rho0);
                }
                let c = (rho / rho0 - 1.0).max(0.0);
                (rho, -c / (gi.length_squared() + g2 + eps))
            });
            let lambda: Vec<f64> = solved.iter().map(|s| s.1).collect();
            for (i, (rho, l)) in solved.iter().enumerate() {
                density[i] = *rho;
                lambda_sum[i] += l;
            }
            let moved = parallel::map(&index, 256, |&i| {
                let xi = x[i];
                let mut dp = DVec3::ZERO;
                for &j in fluid_near.of(i) {
                    let r = xi - x[j as usize];
                    let corr = if tensile > 0.0 {
                        -tensile * (k.w(r.length_squared()) / w_dq).powi(4)
                    } else {
                        0.0
                    };
                    dp += k.grad(r) * (mq * (lambda[i] + lambda[j as usize] + corr));
                }
                let mut pushes = Vec::new();
                for &b in bound_near.of(i) {
                    let r = xi - bpos[b as usize];
                    // A boundary particle mirrors the particle's own
                    // multiplier (Akinci 2012's mirrored pressure).
                    let push = k.grad(r) * (bpsi[b as usize] / rho0 * 2.0 * lambda[i]);
                    dp += push;
                    if owner[b as usize] != u32::MAX {
                        pushes.push((b, push));
                    }
                }
                (dp, pushes)
            });
            for (i, (dp, pushes)) in moved.into_iter().enumerate() {
                x[i] += dp;
                for (b, push) in pushes {
                    // The body takes the opposite of the momentum this
                    // correction gives the particle, spread over the step.
                    let f = -push * (m / (dt * dt));
                    let c = owner[b as usize] as usize;
                    force[c].0 += f;
                    force[c].1 += (bpos[b as usize] - poses[c].0).cross(f);
                }
            }
            for (c, coupled) in self.coupled.iter().enumerate() {
                for p in &mut x {
                    if let Some(out) = outside_box(*p, poses[c], coupled.half) {
                        let f = -(out - *p) * (m / (dt * dt));
                        force[c].0 += f;
                        force[c].1 += (*p - poses[c].0).cross(f);
                        *p = out;
                    }
                }
            }
            if let Some(tank) = self.tank {
                for p in &mut x {
                    *p = clamp_into(*p, tank, s.radius);
                }
            }
        }

        // 4. Velocities, then XSPH viscosity among particles and against
        // boundaries, whose drag a coupled body feels in return.
        let vel: Vec<DVec3> = (0..n).map(|i| (x[i] - pos0[i]) / dt).collect();
        let (c, cb) = (s.viscosity, s.boundary_viscosity);
        if c > 0.0 || cb > 0.0 {
            let smoothed = parallel::map(&index, 256, |&i| {
                let mut dv = DVec3::ZERO;
                for &j in fluid_near.of(i) {
                    let j = j as usize;
                    dv +=
                        (vel[j] - vel[i]) * (m / density[j] * k.w((x[i] - x[j]).length_squared()));
                }
                let mut drags = Vec::new();
                let mut db = DVec3::ZERO;
                if cb > 0.0 {
                    for &b in bound_near.of(i) {
                        let b = b as usize;
                        let w = bpsi[b] / rho0 * k.w((x[i] - bpos[b]).length_squared());
                        let pull = (bvel[b] - vel[i]) * (w * cb);
                        db += pull;
                        if owner[b] != u32::MAX {
                            drags.push((b, pull));
                        }
                    }
                }
                (vel[i] + dv * c + db, drags)
            });
            self.vel = Vec::with_capacity(n);
            for (v, drags) in smoothed {
                self.vel.push(v);
                for (b, pull) in drags {
                    let f = -pull * (m / dt);
                    let o = owner[b] as usize;
                    force[o].0 += f;
                    force[o].1 += (bpos[b] - poses[o].0).cross(f);
                }
            }
        } else {
            self.vel = vel;
        }
        self.pos = x;
        self.density = density;
        // The solve's pressure acts through the kernel-gradient operator,
        // which on a full lattice is `gradient_moment` times a true gradient.
        let to_pascal = -rho0 * self.gradient_moment / (dt * dt);
        self.pressure = lambda_sum.iter().map(|l| l * to_pascal).collect();
        self.neighbours = fluid_near;
        if let Some(world) = world {
            for (coupled, (f, t)) in self.coupled.iter().zip(force) {
                let body = &mut world.bodies_mut()[coupled.body.0 as usize];
                body.apply_force(f);
                body.apply_torque(t);
            }
        }
        self.tick += 1;
    }

    /// Surface tension, adhesion and gravity accelerations at the current
    /// positions, over the last step's fluid neighbours.
    fn forces(&self, bpos: &[DVec3], bpsi: &[f64], bgrid: &Grid) -> Vec<DVec3> {
        let s = self.settings;
        let n = self.pos.len();
        let tension = s.surface_tension > 0.0 && self.neighbours.start.len() == n + 1;
        let adhesion = s.adhesion > 0.0;
        if !tension && !adhesion {
            return vec![s.gravity; n];
        }
        let k = self.kernel;
        let m = self.mass;
        let rho0 = s.rest_density;
        let index: Vec<usize> = (0..n).collect();
        let normals = if tension {
            parallel::map(&index, 256, |&i| {
                let mut nrm = DVec3::ZERO;
                for &j in self.neighbours.of(i) {
                    let j = j as usize;
                    nrm += k.grad(self.pos[i] - self.pos[j]) * (m / self.density[j]);
                }
                nrm * k.h
            })
        } else {
            Vec::new()
        };
        parallel::map(&index, 256, |&i| {
            let mut a = s.gravity;
            let xi = self.pos[i];
            if tension {
                for &j in self.neighbours.of(i) {
                    let j = j as usize;
                    let r = xi - self.pos[j];
                    let len = r.length();
                    if len <= 1e-12 {
                        continue;
                    }
                    let correction = 2.0 * rho0 / (self.density[i] + self.density[j]);
                    let cohesion = -r / len * (s.surface_tension * m * k.cohesion(len));
                    let curvature = -(normals[i] - normals[j]) * s.surface_tension;
                    a += (cohesion + curvature) * correction;
                }
            }
            if adhesion {
                let mut near = Vec::new();
                bgrid.near(xi, bpos, k.h2, &mut near);
                for &b in &near {
                    let r = xi - bpos[b as usize];
                    let len = r.length();
                    if len > 1e-12 {
                        a -= r / len * (s.adhesion * bpsi[b as usize] * k.adhesion(len));
                    }
                }
            }
            a
        })
    }

    /// The comparison statistics ([`Stats`]); needs a tank.
    #[must_use]
    pub fn stats(&self) -> Stats {
        let n = self.pos.len().max(1) as f64;
        let rho0 = self.settings.rest_density;
        let r = self.settings.radius;
        let (floor, area) = self.tank.map_or((0.0, 1.0), |t| {
            (t.min.y, (t.max.x - t.min.x) * (t.max.z - t.min.z))
        });
        let fill = self.pos.len() as f64 * self.mass / rho0 / area;
        let mut heights: Vec<f64> = self.pos.iter().map(|p| p.y - floor).collect();
        heights.sort_by(f64::total_cmp);
        let surface = heights
            .get(((heights.len() as f64) * 0.95) as usize)
            .map_or(0.0, |h| h + r);
        let deep: Vec<f64> = self
            .pos
            .iter()
            .zip(&self.density)
            .filter(|(p, _)| p.y - floor < surface - self.kernel.h)
            .map(|(_, d)| (d / rho0 - 1.0).abs())
            .collect();
        Stats {
            density_error: deep.iter().sum::<f64>() / deep.len().max(1) as f64,
            fill_height: fill,
            surface,
            center_of_mass: self.pos.iter().copied().sum::<DVec3>() / n,
            mean_speed: self.vel.iter().map(|v| v.length()).sum::<f64>() / n,
        }
    }
}

/// Keep a particle centre at least `r` inside the tank's floor and walls.
fn clamp_into(p: DVec3, tank: Tank, r: f64) -> DVec3 {
    DVec3::new(
        p.x.clamp(tank.min.x + r, tank.max.x - r),
        p.y.max(tank.min.y + r),
        p.z.clamp(tank.min.z + r, tank.max.z - r),
    )
}

/// If `p` is inside a posed box, the nearest point on its surface.
fn outside_box(p: DVec3, pose: (DVec3, DQuat), half: DVec3) -> Option<DVec3> {
    let local = pose.1.inverse() * (p - pose.0);
    let grown = half;
    let depth = grown - local.abs();
    if depth.min_element() <= 0.0 {
        return None;
    }
    let mut out = local;
    if depth.x <= depth.y && depth.x <= depth.z {
        out.x = grown.x.copysign(local.x);
    } else if depth.y <= depth.z {
        out.y = grown.y.copysign(local.y);
    } else {
        out.z = grown.z.copysign(local.z);
    }
    Some(pose.0 + pose.1 * out)
}

#[cfg(test)]
#[path = "pbf_tests.rs"]
mod tests;
