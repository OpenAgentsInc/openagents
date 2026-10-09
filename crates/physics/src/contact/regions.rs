//! A large island's contacts solved by regions (issue #10937).
//!
//! A meteor swarm's rubble lies in one island of a thousand or more
//! contacts that touch from house to house, so splitting by island leaves
//! one thread doing most of the work. An island at least [`REGION_ROWS`]
//! contacts large and without joints is cut instead into [`REGIONS`] slabs
//! of its moving bodies, equal in count, along its longer horizontal side.
//! Each pass of the solve sweeps every slab's own contacts, slab after slab,
//! then the contacts between slabs. That order depends only on the island,
//! never on the threads the machine has, so every machine reaches the same
//! result bit for bit; where threads are free, the slabs sweep at once
//! (they share no body that moves) and wait for each other at each pass's
//! seam.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use glam::DVec3;

use super::{Motion, Row, solve_row};

/// The fewest contacts in an island that solves by regions.
pub(super) const REGION_ROWS: usize = 1024;

/// How many slabs a large island is cut into.
pub(super) const REGIONS: usize = 4;

/// One slab: its moving bodies' motions (then copies of the fixed bodies
/// its contacts touch, which never change), and its contacts, with each
/// one's place among the island's.
struct Slab {
    motions: Vec<Motion>,
    moving: Vec<usize>,
    rows: Vec<Row>,
    places: Vec<usize>,
}

/// The contacts between slabs, on copies of the bodies they touch.
struct Seam {
    motions: Vec<Motion>,
    /// Each copied body's slab and its index there.
    from: Vec<(usize, usize)>,
    rows: Vec<Row>,
    places: Vec<usize>,
}

impl Seam {
    /// Sweeps the seam's contacts once, from and back into `slabs`.
    fn sweep(&mut self, slabs: &mut [&mut Slab]) {
        for (k, &(slab, at)) in self.from.iter().enumerate() {
            self.motions[k] = slabs[slab].motions[at];
        }
        for row in &mut self.rows {
            solve_row(row, &mut self.motions);
        }
        for (k, &(slab, at)) in self.from.iter().enumerate() {
            slabs[slab].motions[at] = self.motions[k];
        }
    }
}

/// Solves one island's `rows` (each with its place) by regions, writing
/// its bodies' motions back into `motions`. `positions` are the bodies'
/// positions.
pub(super) fn solve(
    iterations: u32,
    motions: &mut [Motion],
    rows: &mut [(usize, Row)],
    positions: &[DVec3],
) {
    let moves = |i: usize| motions[i].inverse_mass > 0.0;
    let mut bodies: Vec<usize> = rows
        .iter()
        .flat_map(|(_, r)| [r.a, r.b])
        .filter(|&i| moves(i))
        .collect();
    bodies.sort_unstable();
    bodies.dedup();
    if bodies.is_empty() {
        return;
    }
    let (lo, hi) = bodies.iter().fold(
        (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY)),
        |(lo, hi), &i| (lo.min(positions[i]), hi.max(positions[i])),
    );
    let axis = if hi.x - lo.x >= hi.z - lo.z { 0 } else { 2 };
    bodies.sort_by(|&p, &q| {
        positions[p][axis]
            .total_cmp(&positions[q][axis])
            .then(p.cmp(&q))
    });
    let mut slabs: Vec<Slab> = (0..REGIONS)
        .map(|_| Slab {
            motions: Vec::new(),
            moving: Vec::new(),
            rows: Vec::new(),
            places: Vec::new(),
        })
        .collect();
    // Each moving body's slab and index there: slabs of about equal work,
    // a body's work its contacts.
    let mut contacts: HashMap<usize, usize> = HashMap::with_capacity(bodies.len());
    for (_, r) in rows.iter() {
        for end in [r.a, r.b] {
            if moves(end) {
                *contacts.entry(end).or_default() += 1;
            }
        }
    }
    let total: usize = contacts.values().sum();
    let mut home: HashMap<usize, (usize, usize)> = HashMap::with_capacity(bodies.len());
    let mut before = 0;
    for &body in &bodies {
        let slab = (before * REGIONS / total.max(1)).min(REGIONS - 1);
        before += contacts[&body];
        let s = &mut slabs[slab];
        home.insert(body, (slab, s.motions.len()));
        s.motions.push(motions[body]);
        s.moving.push(body);
    }
    let mut fixed: Vec<HashMap<usize, usize>> = vec![HashMap::new(); REGIONS];
    let mut seam = Seam {
        motions: Vec::new(),
        from: Vec::new(),
        rows: Vec::new(),
        places: Vec::new(),
    };
    let mut seam_of: HashMap<usize, usize> = HashMap::new();
    for (place, row) in rows.iter() {
        let (a, b) = (home.get(&row.a).copied(), home.get(&row.b).copied());
        let mut row = *row;
        match (a, b) {
            (Some((sa, ia)), Some((sb, ib))) if sa != sb => {
                let [ka, kb] = [(row.a, (sa, ia)), (row.b, (sb, ib))].map(|(end, (slab, at))| {
                    *seam_of.entry(end).or_insert_with(|| {
                        seam.from.push((slab, at));
                        seam.motions.push(motions[end]);
                        seam.from.len() - 1
                    })
                });
                (row.a, row.b) = (ka, kb);
                seam.rows.push(row);
                seam.places.push(*place);
            }
            _ => {
                let slab = a.or(b).map_or(0, |(slab, _)| slab);
                let s = &mut slabs[slab];
                let [la, lb] = [row.a, row.b].map(|end| match home.get(&end) {
                    Some(&(_, at)) => at,
                    None => *fixed[slab].entry(end).or_insert_with(|| {
                        s.motions.push(motions[end]);
                        s.motions.len() - 1
                    }),
                });
                (row.a, row.b) = (la, lb);
                s.rows.push(row);
                s.places.push(*place);
            }
        }
    }
    let work = slabs.iter().map(|s| s.rows.len()).sum::<usize>();
    let threads = crate::parallel::threads(work, super::SPLIT_ROWS).min(REGIONS);
    if threads <= 1 {
        for _ in 0..iterations {
            for slab in &mut slabs {
                for row in &mut slab.rows {
                    solve_row(row, &mut slab.motions);
                }
            }
            seam.sweep(&mut slabs.iter_mut().collect::<Vec<_>>());
        }
    } else {
        slabs = sweep_together(iterations, slabs, &mut seam);
    }
    // The bodies' motions and the contacts back in their places.
    for slab in &slabs {
        for (k, &body) in slab.moving.iter().enumerate() {
            motions[body] = slab.motions[k];
        }
    }
    let mut solved: HashMap<usize, Row> = HashMap::with_capacity(rows.len());
    for slab in slabs {
        solved.extend(slab.places.into_iter().zip(slab.rows));
    }
    solved.extend(seam.places.into_iter().zip(seam.rows));
    for (place, row) in rows.iter_mut() {
        let (a, b) = (row.a, row.b);
        *row = solved[place];
        row.a = a;
        row.b = b;
    }
}

/// The slabs swept at once, one thread each, meeting at every pass's seam.
fn sweep_together(iterations: u32, slabs: Vec<Slab>, seam: &mut Seam) -> Vec<Slab> {
    let slabs: Vec<Mutex<Slab>> = slabs.into_iter().map(Mutex::new).collect();
    let meet = Meeting::new(slabs.len());
    let slabs_ref = &slabs;
    let meet_ref = &meet;
    let pass = |slab: usize| {
        let mut s = slabs_ref[slab]
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Slab { motions, rows, .. } = &mut *s;
        for row in rows.iter_mut() {
            solve_row(row, motions);
        }
    };
    let pass_ref = &pass;
    std::thread::scope(|scope| {
        for slab in 1..slabs_ref.len() {
            scope.spawn(move || {
                for _ in 0..iterations {
                    pass_ref(slab);
                    meet_ref.wait();
                    meet_ref.wait();
                }
            });
        }
        for _ in 0..iterations {
            pass_ref(0);
            meet_ref.wait();
            {
                let mut guards: Vec<_> = slabs_ref
                    .iter()
                    .map(|s| s.lock().unwrap_or_else(std::sync::PoisonError::into_inner))
                    .collect();
                let mut all: Vec<&mut Slab> = guards.iter_mut().map(|g| &mut **g).collect();
                seam.sweep(&mut all);
            }
            meet_ref.wait();
        }
    });
    slabs
        .into_iter()
        .map(|s| {
            s.into_inner()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        })
        .collect()
}

/// A reusable meeting point for a fixed number of threads: each waits until
/// all have arrived. A pass of the solve is tens of microseconds, so the
/// threads spin briefly before yielding.
struct Meeting {
    count: AtomicUsize,
    round: AtomicUsize,
    threads: usize,
}

impl Meeting {
    fn new(threads: usize) -> Self {
        Self {
            count: AtomicUsize::new(0),
            round: AtomicUsize::new(0),
            threads,
        }
    }

    fn wait(&self) {
        let round = self.round.load(Ordering::Acquire);
        if self.count.fetch_add(1, Ordering::AcqRel) + 1 == self.threads {
            self.count.store(0, Ordering::Relaxed);
            self.round.fetch_add(1, Ordering::Release);
            return;
        }
        let mut spins = 0_u32;
        while self.round.load(Ordering::Acquire) == round {
            if spins < 20_000 {
                std::hint::spin_loop();
                spins += 1;
            } else {
                std::thread::yield_now();
            }
        }
    }
}
