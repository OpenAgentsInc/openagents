//! Many independent worlds stepped together across threads (issue #11659).
//!
//! The idea comes from Genesis's batched scenes (`scene.build(n_envs=...)`,
//! <https://github.com/Genesis-Embodied-AI/Genesis>, genesis-world
//! `df30b61a`, Apache-2.0); this is our own code. Each world
//! is a full [`World`] with its own bodies, so per-environment parameters
//! are whatever the builder puts in it. A world never reads another, and
//! threads only choose which core steps it, so world `i` of a batch is bit
//! for bit the world a single run builds and steps from the same seed.
//! A browser build steps every world on the caller's thread.

use glam::{DQuat, DVec3};

use crate::{Body, BodyKind, Collider, Field, Shape, World};

/// World `index`'s seed in a batch seeded with `seed` (SplitMix64 of the
/// pair), so any one environment can be rebuilt on its own.
#[must_use]
pub fn env_seed(seed: u64, index: usize) -> u64 {
    mix(seed ^ mix(index as u64 ^ 0x6a09_e667_f3bc_c908))
}

fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// A deterministic uniform stream in [0, 1) for scene builders.
#[derive(Clone, Debug)]
pub struct Draw(pub u64);

impl Draw {
    pub fn unit(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        (mix(self.0) >> 11) as f64 / (1u64 << 53) as f64
    }
    /// Uniform in [lo, hi).
    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.unit()
    }
}

/// N independent worlds and the seeds they were built from.
#[derive(Clone, Debug)]
pub struct BatchWorld {
    worlds: Vec<World>,
    seeds: Vec<u64>,
    /// Worker threads, the caller's among them. Zero uses every core.
    pub threads: usize,
}

impl BatchWorld {
    /// `envs` worlds, world `i` built by `build(env_seed(seed, i))`.
    pub fn new(envs: usize, seed: u64, build: impl Fn(u64) -> World) -> Self {
        let seeds: Vec<u64> = (0..envs).map(|i| env_seed(seed, i)).collect();
        Self::with_seeds(&seeds, build)
    }

    /// One world per seed, in order.
    pub fn with_seeds(seeds: &[u64], build: impl Fn(u64) -> World) -> Self {
        Self {
            worlds: seeds.iter().map(|&s| build(s)).collect(),
            seeds: seeds.to_vec(),
            threads: 0,
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.worlds.len()
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.worlds.is_empty()
    }
    #[must_use]
    pub fn worlds(&self) -> &[World] {
        &self.worlds
    }
    pub fn world_mut(&mut self, env: usize) -> &mut World {
        &mut self.worlds[env]
    }
    #[must_use]
    pub fn seed(&self, env: usize) -> u64 {
        self.seeds[env]
    }

    /// Rebuild one world from a seed (its old one to restart the same
    /// episode, a new one for a fresh draw).
    pub fn reset(&mut self, env: usize, seed: u64, build: impl Fn(u64) -> World) {
        self.worlds[env] = build(seed);
        self.seeds[env] = seed;
    }

    /// Threads a step uses for this batch.
    #[must_use]
    pub fn thread_count(&self) -> usize {
        #[cfg(target_arch = "wasm32")]
        {
            1
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let cores = if self.threads == 0 {
                std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get)
            } else {
                self.threads
            };
            cores.min(self.worlds.len()).max(1)
        }
    }

    /// Step every world once under `field`.
    pub fn step(&mut self, field: &(impl Field + Sync)) {
        self.step_each(|_, world| world.step(field));
    }

    /// Run `f(env, world)` on every world, spread over the batch's threads.
    /// `f` may apply that environment's actions before stepping it.
    pub fn step_each(&mut self, f: impl Fn(usize, &mut World) + Sync) {
        let threads = self.thread_count();
        if threads <= 1 && self.worlds.len() > 1 {
            crate::parallel::serial_here(|| {
                for (env, world) in self.worlds.iter_mut().enumerate() {
                    f(env, world);
                }
            });
            return;
        }
        if threads <= 1 {
            // A batch of one keeps the world's own threads.
            for (env, world) in self.worlds.iter_mut().enumerate() {
                f(env, world);
            }
            return;
        }
        let size = self.worlds.len().div_ceil(threads);
        let parts: Vec<(usize, &mut [World])> = self
            .worlds
            .chunks_mut(size)
            .enumerate()
            .map(|(c, run)| (c * size, run))
            .collect();
        crate::parallel::each(parts, |(start, run)| {
            crate::parallel::serial_here(|| {
                for (k, world) in run.iter_mut().enumerate() {
                    f(start + k, world);
                }
            });
        });
    }

    /// Run `steps` steps of every world, each worker carrying its own
    /// worlds through all of them with no hand-over between steps.
    /// `f(env, step, world)` applies that world's actions and steps it.
    pub fn rollout(&mut self, steps: usize, f: impl Fn(usize, usize, &mut World) + Sync) {
        self.step_each(|env, world| {
            for step in 0..steps {
                f(env, step, world);
            }
        });
    }
}

/// The benchmark's native scene: `bodies` boxes, capsules and spheres
/// dropped from seeded heights into a walled bin, so they tumble, collide
/// and stack for the first seconds. Sleep is off, so nothing is skipped
/// while it settles.
#[must_use]
pub fn pile(seed: u64, bodies: usize) -> World {
    let mut world = World::new(1.0 / 240.0);
    world.sleep.enabled = false;
    let mut draw = Draw(seed);
    let fixed = |world: &mut World, pos: DVec3, half: DVec3| {
        let id = world.add(Body::new(1.0, DVec3::ONE, pos).with_kind(BodyKind::Static));
        world.add_collider(Collider::new(id, Shape::Cuboid { half }));
    };
    fixed(
        &mut world,
        DVec3::new(0.0, -0.5, 0.0),
        DVec3::new(2.0, 0.5, 2.0),
    );
    for (x, z) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
        let pos = DVec3::new(x * 1.1, 1.0, z * 1.1);
        let half = if x == 0.0 {
            DVec3::new(1.2, 1.0, 0.1)
        } else {
            DVec3::new(0.1, 1.0, 1.2)
        };
        fixed(&mut world, pos, half);
    }
    for n in 0..bodies {
        // A 4 x 4 grid per layer, jittered, so the drop is short and busy.
        let (cell, layer) = (n % 16, n / 16);
        let pos = DVec3::new(
            (cell % 4) as f64 * 0.42 - 0.63 + draw.range(-0.05, 0.05),
            0.3 + 0.38 * layer as f64,
            (cell / 4) as f64 * 0.42 - 0.63 + draw.range(-0.05, 0.05),
        );
        let mass = draw.range(0.5, 2.0);
        let (shape, inertia) = match n % 3 {
            0 => {
                let half = DVec3::new(
                    draw.range(0.08, 0.16),
                    draw.range(0.08, 0.16),
                    draw.range(0.08, 0.16),
                );
                (Shape::Cuboid { half }, Body::box_inertia(mass, half * 2.0))
            }
            1 => {
                let radius = draw.range(0.08, 0.14);
                (
                    Shape::Sphere { radius },
                    DVec3::splat(0.4 * mass * radius * radius),
                )
            }
            _ => {
                let (radius, half_height) = (draw.range(0.05, 0.09), draw.range(0.08, 0.16));
                let l = 2.0 * (half_height + radius);
                (
                    Shape::Capsule {
                        radius,
                        half_length: half_height,
                    },
                    DVec3::new(
                        mass * (3.0 * radius * radius + l * l) / 12.0,
                        mass * (3.0 * radius * radius + l * l) / 12.0,
                        0.5 * mass * radius * radius,
                    ),
                )
            }
        };
        let mut body = Body::new(mass, inertia, pos);
        body.orientation = DQuat::from_euler(
            glam::EulerRot::XYZ,
            draw.range(-3.0, 3.0),
            draw.range(-3.0, 3.0),
            draw.range(-3.0, 3.0),
        );
        body.vel = DVec3::new(draw.range(-1.0, 1.0), 0.0, draw.range(-1.0, 1.0));
        let id = world.add(body);
        world.add_collider(Collider::new(id, shape));
    }
    world
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Uniform;

    fn bits(world: &World) -> Vec<u64> {
        world
            .bodies()
            .iter()
            .flat_map(|b| {
                let mut v: Vec<f64> = b.pos.to_array().to_vec();
                v.extend(b.vel.to_array());
                v.extend(b.omega.to_array());
                v.extend(b.orientation.to_array());
                v
            })
            .map(f64::to_bits)
            .collect()
    }

    #[test]
    fn each_world_of_a_threaded_batch_of_64_matches_its_single_run() {
        let gravity = Uniform(DVec3::new(0.0, -9.81, 0.0));
        let mut batch = BatchWorld::new(64, 11_659, |s| pile(s, 50));
        batch.threads = 8;
        let steps = 240;
        let mut traces: Vec<Vec<Vec<u64>>> = vec![Vec::new(); 64];
        for _ in 0..steps {
            batch.step(&gravity);
            for (t, w) in traces.iter_mut().zip(batch.worlds()) {
                t.push(bits(w));
            }
        }
        let contacts: usize = batch.worlds().iter().map(|w| w.contacts.len()).sum();
        assert!(contacts > 64 * 30, "{contacts}");
        // Ten of the 64, spread across the threads' chunks, run alone.
        for env in [0, 7, 8, 15, 23, 31, 40, 47, 56, 63] {
            let mut alone = pile(batch.seed(env), 50);
            for (step, expected) in traces[env].iter().enumerate() {
                alone.step(&gravity);
                assert_eq!(&bits(&alone), expected, "env {env} step {step}");
            }
        }
    }

    #[test]
    fn the_pile_is_active_and_seeds_differ() {
        let a = pile(env_seed(1, 0), 50);
        let b = pile(env_seed(1, 1), 50);
        assert_ne!(bits(&a), bits(&b));
        let mut batch = BatchWorld::with_seeds(&[5, 5], |s| pile(s, 50));
        batch.step(&Uniform(DVec3::new(0.0, -9.81, 0.0)));
        assert_eq!(bits(&batch.worlds()[0]), bits(&batch.worlds()[1]));
        batch.reset(1, 6, |s| pile(s, 50));
        assert_eq!(batch.worlds()[1].tick, 0);
        assert_eq!(batch.seed(1), 6);
    }
}
