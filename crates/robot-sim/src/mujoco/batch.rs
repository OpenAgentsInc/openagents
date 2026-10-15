//! Many independent copies of one MuJoCo scene in one process, stepped across
//! threads (issue #11659).
//!
//! The idea comes from Genesis's `scene.build(n_envs=...)` (genesis-world
//! `df30b61a`, Apache-2.0); nothing here is translated from its code. Every
//! environment owns its `mjData`. Environments share the loaded `mjModel`
//! unless their physical parameters are randomised, in which case each gets
//! its own copy (`mj_copyModel`), because MuJoCo keeps mass, friction, and
//! gains on the model.
//!
//! An environment's trajectory depends only on its seed and its actions: it
//! never reads another environment, and threads only decide which core runs
//! it. So environment `i` of a batch of 64 matches, bit for bit, a batch of
//! one built with the same seed (`tests/batch.rs`).

use std::ffi::c_int;
use std::ptr::NonNull;

use super::{Model, Servo, ffi};
use crate::{Error, Result, require};

/// Relative half-widths for per-environment parameter randomisation. Each
/// factor is drawn uniformly from `1 ± width` per body, geom, or servo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Randomize {
    /// Body mass and rotational inertia, scaled together.
    pub mass: f64,
    /// Sliding friction of every geom.
    pub friction: f64,
    /// Position gain of every controlled servo.
    pub gain: f64,
}

/// How a batch is laid out and run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// Worker threads, the caller's among them. Zero uses every core.
    pub threads: usize,
    /// Per-environment parameters; `None` keeps the loaded model's values.
    pub randomize: Option<Randomize>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            threads: 0,
            randomize: None,
        }
    }
}

/// Environment `index`'s seed in a batch seeded with `seed` (SplitMix64 of
/// the pair), so any one environment can be rebuilt on its own.
pub fn env_seed(seed: u64, index: usize) -> u64 {
    mix(seed ^ mix(index as u64 ^ 0x6a09_e667_f3bc_c908))
}

fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// A uniform draw in [-1, 1) from a SplitMix64 stream.
struct Draw(u64);
impl Draw {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let bits = mix(self.0) >> 11;
        bits as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
    }
}

/// One controlled joint, in the template's (name-sorted) order.
#[derive(Clone)]
struct Joint {
    qpos: usize,
    dof: usize,
    actuator: usize,
    servo: Servo,
    limits: Option<(f64, f64)>,
}

struct Env {
    /// This environment's own model when its parameters are randomised.
    model: Option<NonNull<ffi::mjModel>>,
    data: NonNull<ffi::mjData>,
    seed: u64,
    /// Per-joint position gain (randomised) and current target.
    kp: Vec<f64>,
    targets: Vec<f64>,
}

// SAFETY: an `Env` is only ever touched by one thread at a time (each step
// hands every environment to exactly one worker), and the template model it
// may read is never written after the batch is built.
unsafe impl Send for Env {}

#[derive(Clone, Copy)]
struct Shared(*const ffi::mjModel);
// SAFETY: MuJoCo reads a model through `const mjModel*` during `mj_step`;
// several threads may step different `mjData` against one model.
unsafe impl Sync for Shared {}
unsafe impl Send for Shared {}
impl Shared {
    fn get(self) -> *const ffi::mjModel {
        self.0
    }
}

/// Raw views of a run's environments and buffers, shared with workers that
/// each touch only their own rows between barriers.
#[derive(Clone, Copy)]
struct Buffers {
    envs: *mut Env,
    obs: *mut f64,
    contacts: *mut usize,
    actions: *mut f64,
}
// SAFETY: see `Batch::run`: rows are disjoint per worker, and the barrier
// orders every hand-over between the caller and the workers.
unsafe impl Sync for Buffers {}
unsafe impl Send for Buffers {}
impl Buffers {
    // Methods, so closures capture the whole (Sync) struct, not its fields.
    fn envs(&self) -> *mut Env {
        self.envs
    }
    fn obs(&self) -> *mut f64 {
        self.obs
    }
    fn contacts(&self) -> *mut usize {
        self.contacts
    }
    fn actions(&self) -> *mut f64 {
        self.actions
    }
}

/// A barrier that spins briefly before yielding: a step of a small arm is a
/// few microseconds, far less than a sleeping thread takes to wake.
struct SpinBarrier {
    parties: usize,
    arrived: std::sync::atomic::AtomicUsize,
    generation: std::sync::atomic::AtomicUsize,
}

impl SpinBarrier {
    fn new(parties: usize) -> Self {
        Self {
            parties,
            arrived: std::sync::atomic::AtomicUsize::new(0),
            generation: std::sync::atomic::AtomicUsize::new(0),
        }
    }
    fn wait(&self) {
        use std::sync::atomic::Ordering::{AcqRel, Acquire, Release};
        let generation = self.generation.load(Acquire);
        if self.arrived.fetch_add(1, AcqRel) + 1 == self.parties {
            self.arrived.store(0, Release);
            self.generation.fetch_add(1, Release);
            return;
        }
        let mut spins = 0u32;
        while self.generation.load(Acquire) == generation {
            if spins < 20_000 {
                std::hint::spin_loop();
                spins += 1;
            } else {
                std::thread::yield_now();
            }
        }
    }
}

/// N copies of one scene. Actions are joint position targets for the
/// template's controlled joints, in name order, `envs × joints` long.
pub struct Batch<'a> {
    template: &'a Model,
    joints: Vec<Joint>,
    names: Vec<String>,
    envs: Vec<Env>,
    threads: usize,
    randomize: Option<Randomize>,
}

impl Drop for Batch<'_> {
    fn drop(&mut self) {
        let api = &self.template.runtime.api;
        for env in &self.envs {
            // SAFETY: each data and copied model is owned by this batch alone.
            unsafe {
                (api.delete_data)(env.data.as_ptr());
                if let Some(model) = env.model {
                    (api.delete_model)(model.as_ptr());
                }
            }
        }
    }
}

impl<'a> Batch<'a> {
    /// `envs` environments seeded from `seed` ([`env_seed`]).
    pub fn new(template: &'a Model, envs: usize, seed: u64, settings: Settings) -> Result<Self> {
        let seeds: Vec<u64> = (0..envs).map(|i| env_seed(seed, i)).collect();
        Self::with_seeds(template, &seeds, settings)
    }

    /// One environment per seed, in order.
    pub fn with_seeds(template: &'a Model, seeds: &[u64], settings: Settings) -> Result<Self> {
        require(
            (1..=65_536).contains(&seeds.len()),
            "Run between 1 and 65,536 environments.",
        )?;
        if let Some(r) = settings.randomize {
            require(
                [r.mass, r.friction, r.gain]
                    .iter()
                    .all(|w| w.is_finite() && (0.0..=0.9).contains(w)),
                "Keep randomisation widths between 0 and 0.9.",
            )?;
        }
        let names: Vec<String> = template.controlled.keys().cloned().collect();
        let joints = template
            .controlled
            .values()
            .map(|c| Joint {
                qpos: c.qpos,
                dof: c.dof,
                actuator: c.actuator,
                servo: c.servo.clone(),
                limits: template.limits(c),
            })
            .collect();
        let threads = if settings.threads == 0 {
            std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get)
        } else {
            settings.threads
        };
        let mut batch = Self {
            template,
            joints,
            names,
            envs: Vec::with_capacity(seeds.len()),
            threads,
            randomize: settings.randomize,
        };
        for &seed in seeds {
            let env = batch.make(seed)?;
            batch.envs.push(env);
            batch.reset(batch.envs.len() - 1)?;
        }
        Ok(batch)
    }

    fn make(&self, seed: u64) -> Result<Env> {
        let api = &self.template.runtime.api;
        let mut kp: Vec<f64> = self.joints.iter().map(|j| j.servo.kp).collect();
        let model = match self.randomize {
            None => None,
            Some(r) => {
                // SAFETY: copying into a fresh model allocates a new owner.
                let copy =
                    unsafe { (api.copy_model)(std::ptr::null_mut(), self.template.model.as_ptr()) };
                let mut copy = NonNull::new(copy)
                    .ok_or_else(|| Error::Library("The robot couldn't be copied".into()))?;
                let mut draw = Draw(seed);
                // SAFETY: the copy is a live 3.3.7 model owned here; every
                // index below is within its own nbody, ngeom, and nu arrays.
                unsafe {
                    let m = copy.as_mut();
                    for body in 1..m.nbody as usize {
                        let f = 1.0 + r.mass * draw.next();
                        *m.body_mass.add(body) *= f;
                        for k in 0..3 {
                            *m.body_inertia.add(body * 3 + k) *= f;
                        }
                    }
                    for geom in 0..m.ngeom as usize {
                        *m.geom_friction.add(geom * 3) *= 1.0 + r.friction * draw.next();
                    }
                    for (joint, gain) in self.joints.iter().zip(&mut kp) {
                        *gain *= 1.0 + r.gain * draw.next();
                        *m.actuator_gainprm.add(joint.actuator * 10) = *gain;
                        *m.actuator_biasprm.add(joint.actuator * 10 + 1) = -*gain;
                    }
                }
                Some(copy)
            }
        };
        let source = model.map_or(self.template.model.as_ptr(), NonNull::as_ptr);
        // SAFETY: makeData receives a live model and returns data owned here.
        let data = NonNull::new(unsafe { (api.make)(source) }).ok_or_else(|| {
            if let Some(model) = model {
                // SAFETY: the copy is still uniquely owned on this error path.
                unsafe { (api.delete_model)(model.as_ptr()) };
            }
            Error::Library("The robot's state couldn't be allocated".into())
        })?;
        if model.is_some() {
            // SAFETY: recompute derived constants (subtree masses, inverse
            // weights) of this environment's own model and data.
            unsafe { (api.set_const)(source, data.as_ptr()) };
        }
        Ok(Env {
            model,
            data,
            seed,
            targets: Vec::new(),
            kp,
        })
    }

    pub fn len(&self) -> usize {
        self.envs.len()
    }
    pub fn is_empty(&self) -> bool {
        self.envs.is_empty()
    }
    /// Worker threads a step uses.
    pub fn threads(&self) -> usize {
        self.threads
    }
    /// Controlled joint names, the order of every action row.
    pub fn joints(&self) -> &[String] {
        &self.names
    }
    /// Values per observation row: `qpos` then `qvel`.
    pub fn observation_width(&self) -> usize {
        let (nq, nv, _) = self.template.dimensions();
        nq + nv
    }
    pub fn seed(&self, env: usize) -> u64 {
        self.envs[env].seed
    }
    /// The joint limits of each controlled joint, if limited.
    pub fn limits(&self) -> Vec<Option<(f64, f64)>> {
        self.joints.iter().map(|j| j.limits).collect()
    }
    /// The initial target of each controlled joint (its start pose).
    pub fn home(&self) -> Vec<f64> {
        self.joints
            .iter()
            .map(|j| self.template.initial_qpos[j.qpos])
            .collect()
    }

    fn model_of(&self, env: usize) -> *const ffi::mjModel {
        self.envs[env]
            .model
            .map_or(self.template.model.as_ptr().cast_const(), |m| {
                m.as_ptr().cast_const()
            })
    }

    /// Put one environment back at the start pose, keeping its parameters.
    pub fn reset(&mut self, env: usize) -> Result<()> {
        require(
            env < self.envs.len(),
            "Choose an environment in this batch.",
        )?;
        let api = self.template.runtime.api.clone();
        let model = self.model_of(env) as *mut ffi::mjModel;
        let initial = &self.template.initial_qpos;
        let targets: Vec<f64> = self.joints.iter().map(|j| initial[j.qpos]).collect();
        let e = &mut self.envs[env];
        // SAFETY: this environment's matching model and data; qpos holds nq
        // entries and ctrl nu entries, both checked when joints were bound.
        unsafe {
            (api.reset)(model, e.data.as_ptr());
            let d = e.data.as_mut();
            std::ptr::copy_nonoverlapping(initial.as_ptr(), d.qpos, initial.len());
            for (joint, target) in self.joints.iter().zip(&targets) {
                *d.ctrl.add(joint.actuator) = *target;
            }
            (api.forward)(model, e.data.as_ptr());
        }
        e.targets = targets;
        Ok(())
    }

    /// Re-draw one environment's parameters from a new seed and reset it.
    pub fn reseed(&mut self, env: usize, seed: u64) -> Result<()> {
        require(
            env < self.envs.len(),
            "Choose an environment in this batch.",
        )?;
        let fresh = self.make(seed)?;
        let old = std::mem::replace(&mut self.envs[env], fresh);
        let api = &self.template.runtime.api;
        // SAFETY: the replaced environment's data and model are owned here.
        unsafe {
            (api.delete_data)(old.data.as_ptr());
            if let Some(model) = old.model {
                (api.delete_model)(model.as_ptr());
            }
        }
        self.reset(env)
    }

    /// Advance every environment one step toward `actions` (`envs × joints`
    /// position targets). Each servo moves at most its speed limit toward
    /// its target, the same control law as [`Model`]'s single step. For
    /// many steps, [`Self::run`] keeps its workers between steps.
    pub fn step(&mut self, actions: &[f64]) -> Result<()> {
        require(
            actions.len() == self.envs.len() * self.joints.len(),
            "Give one action row per environment.",
        )?;
        self.run(1, |_, _, _, out| out.copy_from_slice(actions))
    }

    fn check(&self, actions: &[f64]) -> Result<()> {
        for row in actions.chunks(self.joints.len().max(1)) {
            for (value, joint) in row.iter().zip(&self.joints) {
                require(
                    value.is_finite()
                        && joint
                            .limits
                            .is_none_or(|(lo, hi)| *value >= lo && *value <= hi),
                    "Keep robot actions inside the joint limits.",
                )?;
            }
        }
        Ok(())
    }

    /// Run `steps` control steps on workers that live for the whole call.
    /// Before each step `control(step, observations, contacts, actions)`
    /// runs on the caller's thread: it reads every environment's last
    /// observation row ([`Self::observe`]'s layout) and contact count, and
    /// writes this step's action rows. The workers then step their own
    /// environments in parallel and write the new observations back. This
    /// is the shape of a learning loop with one central policy.
    pub fn run(
        &mut self,
        steps: usize,
        mut control: impl FnMut(usize, &[f64], &[usize], &mut [f64]),
    ) -> Result<()> {
        let n = self.envs.len();
        let width = self.joints.len();
        let ow = self.observation_width();
        let nq = self.template.dimensions().0;
        let mut obs = vec![0.0; n * ow];
        self.observe(&mut obs)?;
        let mut contacts: Vec<usize> = (0..n).map(|e| self.contacts(e)).collect();
        let mut actions: Vec<f64> = self.envs.iter().flat_map(|e| e.targets.clone()).collect();
        let threads = self.threads.min(n).max(1);
        let size = n.div_ceil(threads);
        let chunks = n.div_ceil(size);
        let template = Shared(self.template.model.as_ptr());
        let api = &*self.template.runtime.api;
        let joints = &self.joints;
        let dt = self.template.timestep;
        let world = Buffers {
            envs: self.envs.as_mut_ptr(),
            obs: obs.as_mut_ptr(),
            contacts: contacts.as_mut_ptr(),
            actions: actions.as_mut_ptr(),
        };
        let failed = std::sync::atomic::AtomicUsize::new(usize::MAX);
        let stop = std::sync::atomic::AtomicBool::new(false);
        let barrier = SpinBarrier::new(chunks);
        // Step environments `range` of the shared buffers.
        let work = |range: std::ops::Range<usize>| {
            for index in range {
                // SAFETY: the barrier gives this worker exclusive use of the
                // environments and buffer rows in its range for this phase.
                unsafe {
                    let env = &mut *world.envs().add(index);
                    let row = std::slice::from_raw_parts(world.actions().add(index * width), width);
                    if !advance(api, template, joints, dt, env, row) {
                        failed.fetch_min(index, std::sync::atomic::Ordering::Relaxed);
                    }
                    let out = std::slice::from_raw_parts_mut(world.obs().add(index * ow), ow);
                    *world.contacts().add(index) = read(env, nq, out);
                }
            }
        };
        let mut result = Ok(());
        std::thread::scope(|scope| {
            for c in 1..chunks {
                let (work, barrier, stop) = (&work, &barrier, &stop);
                scope.spawn(move || {
                    loop {
                        barrier.wait();
                        if stop.load(std::sync::atomic::Ordering::Acquire) {
                            break;
                        }
                        work(c * size..((c + 1) * size).min(n));
                        barrier.wait();
                    }
                });
            }
            for step in 0..steps {
                // SAFETY: every worker is parked at the barrier, so the
                // caller alone reads observations and writes actions.
                let (o, k, a) = unsafe {
                    (
                        std::slice::from_raw_parts(world.obs(), n * ow),
                        std::slice::from_raw_parts(world.contacts(), n),
                        std::slice::from_raw_parts_mut(world.actions(), n * width),
                    )
                };
                control(step, o, k, a);
                if let Err(e) = self.check(a) {
                    result = Err(e);
                    break;
                }
                barrier.wait();
                work(0..size.min(n));
                barrier.wait();
                let bad = failed.load(std::sync::atomic::Ordering::Relaxed);
                if bad != usize::MAX {
                    result = Err(Error::Invalid(format!(
                        "Environment {bad} became unstable. Try a smaller simulation step."
                    )));
                    break;
                }
            }
            stop.store(true, std::sync::atomic::Ordering::Release);
            barrier.wait();
        });
        result
    }

    /// Run `steps` steps of every environment with a per-environment
    /// policy and no hand-over between steps: each worker carries its own
    /// environments through all their steps. `policy(env, step,
    /// observation, action)` sees one environment's last observation row
    /// and writes its action row. Use it when each environment decides on
    /// its own (scripted control, a policy per worker); a central policy
    /// that sees every environment at once needs [`Self::run`].
    pub fn rollout(
        &mut self,
        steps: usize,
        policy: impl Fn(usize, usize, &[f64], &mut [f64]) + Sync,
    ) -> Result<()> {
        let n = self.envs.len();
        let width = self.joints.len();
        let ow = self.observation_width();
        let nq = self.template.dimensions().0;
        let template = Shared(self.template.model.as_ptr());
        let api = &*self.template.runtime.api;
        let joints = &self.joints;
        let dt = self.template.timestep;
        let threads = self.threads.min(n).max(1);
        let size = n.div_ceil(threads);
        let failed = std::sync::atomic::AtomicUsize::new(usize::MAX);
        let limits = |row: &[f64]| {
            row.iter()
                .zip(joints)
                .all(|(v, j)| v.is_finite() && j.limits.is_none_or(|(lo, hi)| *v >= lo && *v <= hi))
        };
        let work = |start: usize, envs: &mut [Env]| {
            let mut obs = vec![0.0; ow];
            let mut action = vec![0.0; width];
            for (k, env) in envs.iter_mut().enumerate() {
                let index = start + k;
                for step in 0..steps {
                    // SAFETY: this worker alone owns `env` for the rollout.
                    unsafe { read(env, nq, &mut obs) };
                    action.copy_from_slice(&env.targets);
                    policy(index, step, &obs, &mut action);
                    // SAFETY: as above; the row was checked against limits.
                    if !limits(&action)
                        || !unsafe { advance(api, template, joints, dt, env, &action) }
                    {
                        failed.fetch_min(index, std::sync::atomic::Ordering::Relaxed);
                        break;
                    }
                }
            }
        };
        if threads == 1 {
            work(0, &mut self.envs);
        } else {
            std::thread::scope(|scope| {
                let mut parts = self.envs.chunks_mut(size).enumerate();
                let first = parts.next();
                for (c, envs) in parts {
                    let work = &work;
                    scope.spawn(move || work(c * size, envs));
                }
                if let Some((_, envs)) = first {
                    work(0, envs);
                }
            });
        }
        match failed.load(std::sync::atomic::Ordering::Relaxed) {
            usize::MAX => Ok(()),
            env => Err(Error::Invalid(format!(
                "Environment {env} left the joint limits or became unstable."
            ))),
        }
    }

    /// Write every environment's `qpos` then `qvel` into `out`, one row of
    /// [`Self::observation_width`] values per environment.
    pub fn observe(&self, out: &mut [f64]) -> Result<()> {
        let (nq, nv, _) = self.template.dimensions();
        require(
            out.len() == self.envs.len() * (nq + nv),
            "Give one observation row per environment.",
        )?;
        for (env, row) in self.envs.iter().zip(out.chunks_mut(nq + nv)) {
            // SAFETY: qpos and qvel hold nq and nv entries in every data made
            // from this template or its copies.
            unsafe {
                let d = env.data.as_ref();
                std::ptr::copy_nonoverlapping(d.qpos, row.as_mut_ptr(), nq);
                std::ptr::copy_nonoverlapping(d.qvel, row[nq..].as_mut_ptr(), nv);
            }
        }
        Ok(())
    }

    /// Active contacts in one environment after its last step.
    pub fn contacts(&self, env: usize) -> usize {
        // SAFETY: ncon is a plain count in this environment's live data.
        let n: c_int = unsafe { self.envs[env].data.as_ref().ncon };
        n.max(0) as usize
    }

    /// Simulated seconds of one environment.
    pub fn time(&self, env: usize) -> f64 {
        // SAFETY: time is a plain value in this environment's live data.
        unsafe { self.envs[env].data.as_ref().time }
    }

    /// The position of one controlled joint's degree of freedom velocity
    /// index, for reading `qvel` rows.
    pub fn joint_columns(&self) -> Vec<(usize, usize)> {
        self.joints.iter().map(|j| (j.qpos, j.dof)).collect()
    }

    /// Total mass of one environment's bodies (the randomised value).
    pub fn total_mass(&self, env: usize) -> f64 {
        let model = self.model_of(env);
        // SAFETY: body_mass holds nbody entries of this environment's model.
        unsafe {
            let m = &*model;
            (0..m.nbody as usize).map(|b| *m.body_mass.add(b)).sum()
        }
    }
}

/// The Robot Lab SO-101 table scene with a 5 cm cube in reach, the
/// contact-rich environment the batch tests and benchmark use. `source` is
/// the pinned SO-101 description folder (`crates/robot-description/assets/so101`).
pub fn so101_table(runtime: &super::Runtime, source: &std::path::Path) -> Result<Model> {
    use robot_lab::document::{
        Camera, Geometry, Object, Pose, Reference, RobotInstance, Scene, Shape,
    };
    let (pin, robot) =
        robot_description::pin::load_pinned(source).map_err(|e| Error::Invalid(e.to_string()))?;
    let object = |id: &str, position, size, mass| Object {
        id: id.into(),
        geometry: Geometry {
            pose: Pose {
                position,
                ..Pose::default()
            },
            shape: Shape::Box { size },
            rgba: [0.8, 0.1, 0.1, 1.0],
        },
        mesh: None,
        collider: Some(Shape::Box { size }),
        mass,
        friction: 0.7,
    };
    let scene = Scene {
        name: "SO-101 table".into(),
        robots: vec![RobotInstance {
            id: "arm".into(),
            robot: Reference {
                document: "so101".into(),
                version: "a".repeat(64),
            },
            pose: Pose {
                position: [0.0, 0.75, 0.0],
                ..Pose::default()
            },
            joints: robot.movable().map(|j| (j.name.clone(), 0.0)).collect(),
        }],
        objects: vec![
            object("table", [0.0, 0.73, 0.0], [1.2, 0.04, 0.75], 0.0),
            object("cube", [0.22, 0.775, -0.1], [0.05; 3], 0.05),
        ],
        lights: vec![],
        cameras: vec![Camera {
            id: "front".into(),
            pose: Pose::default(),
            width: 64,
            height: 64,
            intrinsics: [50.0, 50.0, 32.0, 32.0],
            robot_link: None,
        }],
    };
    runtime.arm_scene(
        source,
        &pin.description,
        &scene,
        1.0 / 240.0,
        Servo {
            kp: 100.0,
            kd: 4.0,
            effort: 2.4,
            speed: 2.0,
        },
    )
}

/// One control step of one environment toward `row`: the speed-limited
/// servo law of [`Model`]'s step, then `mj_step`. False if the step did
/// not advance time by `dt` (MuJoCo reset an unstable state).
///
/// # Safety
/// The caller has exclusive use of `env`, and `template` is the model its
/// data was made from when it has no copy of its own.
unsafe fn advance(
    api: &super::Api,
    template: Shared,
    joints: &[Joint],
    dt: f64,
    env: &mut Env,
    row: &[f64],
) -> bool {
    let model = env
        .model
        .map_or(template.get(), |m| m.as_ptr().cast_const());
    // SAFETY: per the contract above; qpos and ctrl are sized by the model.
    unsafe {
        let d = env.data.as_mut();
        let before = d.time;
        for (j, (joint, target)) in joints.iter().zip(row).enumerate() {
            let q = *d.qpos.add(joint.qpos);
            let (kp, kd) = (env.kp[j], joint.servo.kd);
            let velocity = (kp * (target - q) / kd).clamp(-joint.servo.speed, joint.servo.speed);
            *d.ctrl.add(joint.actuator) = q + velocity * kd / kp;
        }
        (api.step)(model, d);
        env.targets.clear();
        env.targets.extend_from_slice(row);
        (d.time - before - dt).abs() <= 1e-9
    }
}

/// Copy one environment's `qpos` then `qvel` into `out`; returns its
/// contact count.
///
/// # Safety
/// The caller has exclusive or read-only use of `env`'s data.
unsafe fn read(env: &Env, nq: usize, out: &mut [f64]) -> usize {
    // SAFETY: qpos and qvel hold nq and `out.len() - nq` entries.
    unsafe {
        let d = env.data.as_ref();
        std::ptr::copy_nonoverlapping(d.qpos, out.as_mut_ptr(), nq);
        std::ptr::copy_nonoverlapping(d.qvel, out[nq..].as_mut_ptr(), out.len() - nq);
        d.ncon.max(0) as usize
    }
}
