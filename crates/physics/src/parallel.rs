//! Spreading a step's independent work over threads (issue #10937).
//!
//! Every use keeps the step's result bit for bit what one thread computes:
//! work is split only where its parts share nothing they write, and parts
//! come back in their original order. A browser build
//! (`wasm32-unknown-unknown`) has no threads, so there everything runs on
//! the caller's thread.

/// The most threads a step uses, the caller's among them.
pub(crate) const MOST: usize = 8;

/// Tests run every step on one thread while this is set, to compare.
#[cfg(test)]
pub(crate) static SERIAL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// How many threads to split `work` units across, at least `per_thread`
/// units each: one when the work is small or threads are unavailable.
pub(crate) fn threads(work: usize, per_thread: usize) -> usize {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (work, per_thread);
        1
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        #[cfg(test)]
        if SERIAL.load(std::sync::atomic::Ordering::Relaxed) {
            return 1;
        }
        if work < per_thread.saturating_mul(2) {
            return 1;
        }
        let cores = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
        cores.min(MOST).min(work / per_thread.max(1)).max(1)
    }
}

/// `f` of each of `parts`, in their order, the first on the caller's
/// thread and the rest on scoped threads.
pub(crate) fn each<P: Send, R: Send>(parts: Vec<P>, f: impl Fn(P) -> R + Sync) -> Vec<R> {
    #[cfg(target_arch = "wasm32")]
    {
        parts.into_iter().map(f).collect()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        if parts.len() <= 1 {
            return parts.into_iter().map(f).collect();
        }
        let f = &f;
        std::thread::scope(|scope| {
            let mut parts = parts.into_iter();
            let first = parts.next();
            let handles: Vec<_> = parts.map(|part| scope.spawn(move || f(part))).collect();
            let mut out = Vec::with_capacity(handles.len() + 1);
            out.extend(first.map(f));
            for handle in handles {
                match handle.join() {
                    Ok(result) => out.push(result),
                    Err(panic) => std::panic::resume_unwind(panic),
                }
            }
            out
        })
    }
}

/// `f` of each item of `items`, in order, over contiguous runs of at least
/// `per_thread` items on up to [`MOST`] threads.
pub(crate) fn map<T: Sync, R: Send>(
    items: &[T],
    per_thread: usize,
    f: impl Fn(&T) -> R + Sync,
) -> Vec<R> {
    let n = threads(items.len(), per_thread);
    if n <= 1 {
        return items.iter().map(&f).collect();
    }
    let size = items.len().div_ceil(n);
    each(items.chunks(size).collect(), |run: &[T]| {
        run.iter().map(&f).collect::<Vec<R>>()
    })
    .into_iter()
    .flatten()
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Body, Collider, Shape, Uniform, World};
    use glam::{DQuat, DVec3};

    /// Forty piles of tumbling boxes on a fixed floor, or with `rubble`
    /// one field of boxes all touching, stepped a second.
    fn piles(serial: bool, rubble: bool) -> (World, usize) {
        let mut world = World::new(1.0 / 60.0);
        let floor = world.add(
            Body::new(1.0, DVec3::ONE, DVec3::new(0.0, -0.5, 0.0))
                .with_kind(crate::BodyKind::Static),
        );
        let half = DVec3::new(60.0, 0.5, 60.0);
        world.add_collider(Collider::new(floor, Shape::Cuboid { half }));
        if rubble {
            // Two courses, the upper one bridging the lower's joints, so
            // every box is one island.
            for n in 0..400 + 361 {
                let lean = f64::from(n) * 0.61;
                let (row, x, z, y) = if n < 400 {
                    (20, n % 20, n / 20, 0.31)
                } else {
                    (19, (n - 400) % 19, (n - 400) / 19, 0.92)
                };
                let shift = if row == 19 { 0.5 } else { 0.0 };
                let mut body = Body::new(
                    2.0,
                    DVec3::splat(0.3),
                    DVec3::new(
                        f64::from(x) * 1.01 + shift,
                        y + 0.02 * lean.sin(),
                        f64::from(z) * 1.01 + shift,
                    ),
                );
                body.orientation = DQuat::from_rotation_y(0.01 * lean.cos());
                let id = world.add(body);
                let half = DVec3::new(0.5, 0.3, 0.5);
                world.add_collider(Collider::new(id, Shape::Cuboid { half }));
            }
        }
        for pile in 0..if rubble { 0 } else { 40 } {
            let (x, z) = (
                f64::from(pile % 8) * 6.0 - 21.0,
                f64::from(pile / 8) * 6.0 - 12.0,
            );
            for level in 0..6 {
                let lean = f64::from(pile * 7 + level) * 0.37;
                let mut body = Body::new(
                    2.0,
                    DVec3::splat(0.3),
                    DVec3::new(x + lean.sin() * 0.2, 0.3 + f64::from(level) * 0.62, z),
                );
                body.orientation = DQuat::from_rotation_y(lean);
                let id = world.add(body);
                let half = DVec3::new(0.5, 0.3, 0.4);
                world.add_collider(Collider::new(id, Shape::Cuboid { half }));
            }
        }
        SERIAL.store(serial, std::sync::atomic::Ordering::Relaxed);
        let mut most = 0;
        for _ in 0..60 {
            world.step(&Uniform(DVec3::new(0.0, -9.81, 0.0)));
            most = most.max(world.stats.contact_points);
        }
        SERIAL.store(false, std::sync::atomic::Ordering::Relaxed);
        (world, most)
    }

    #[test]
    fn a_step_split_over_threads_is_the_step_on_one_bit_for_bit() {
        // Many small islands, then one island solved by regions.
        for rubble in [false, true] {
            let (serial, contacts) = piles(true, rubble);
            let (split, _) = piles(false, rubble);
            assert!(contacts > if rubble { 2048 } else { 512 }, "{contacts}");
            for (a, b) in serial.bodies().iter().zip(split.bodies()) {
                for (x, y) in [(a.pos, b.pos), (a.vel, b.vel), (a.omega, b.omega)] {
                    assert_eq!(
                        x.to_array().map(f64::to_bits),
                        y.to_array().map(f64::to_bits)
                    );
                }
                let (p, q) = (a.orientation.to_array(), b.orientation.to_array());
                assert_eq!(p.map(f64::to_bits), q.map(f64::to_bits));
                assert!(a.pos.is_finite() && a.pos.y > -0.6, "{rubble}: {}", a.pos);
            }
        }
    }

    #[test]
    fn mapping_over_threads_keeps_the_order() {
        let items: Vec<u32> = (0..10_000).collect();
        let out = map(&items, 100, |x| x * 3);
        assert_eq!(out, items.iter().map(|x| x * 3).collect::<Vec<_>>());
        assert_eq!(threads(10, 100), 1);
    }
}
