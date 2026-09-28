//! Zone transition admission and the Ruins, Lagrange 1, and Physics Lab
//! simulations.

use super::{
    Control, Intent, Lab, Lagrange, LoadState, PortalProjection, Ruins, Snapshot, ZoneId, assets,
};
use crate::{
    controller::{InputState, PlayerController},
    runtime::WorldRuntime,
};
use glam::{DVec3, Vec3};
use serde_json::{Value, json};
use verse_lagrange::{Input, PartState};

/// The display name of the OpenAgents app's bare world.
pub const GRID_LABEL: &str = "The Grid";

impl WorldRuntime {
    pub(crate) fn update_player(&mut self, input: &InputState, dt: f32) {
        if let Some(ruins) = &mut self.zone_state.ruins {
            ruins.move_player(&mut self.player, input, dt);
        } else if let Some(lagrange) = &mut self.zone_state.lagrange {
            lagrange.move_player(&mut self.player, input, self.camera.pitch, dt);
        } else {
            self.player
                .update(input, dt, &self.world.blockers, self.zone_half());
        }
    }

    /// Configure a host-owned cache directory. This does not read or fetch assets.
    pub fn configure_zone_cache(&mut self, path: std::path::PathBuf) {
        self.zone_cancel_loading();
        self.zone_state.loader = Some(assets::Loader::new(path));
    }
    pub fn is_plaza(&self) -> bool {
        self.zone == ZoneId::Plaza
    }
    pub fn zone_half(&self) -> f32 {
        self.zone.half_extent()
    }
    pub fn zone_loading(&self) -> bool {
        self.zone_state.loading == LoadState::Loading
    }
    pub fn zone_cancel_loading(&mut self) {
        if let Some(loader) = &mut self.zone_state.loader {
            loader.cancel();
        }
        self.zone_state.loading = LoadState::Idle;
        self.zone_state.progress = 0.0;
        self.zone_state.error = None;
    }
    /// Poll only while the native surface is active, before its presence tick.
    /// A true result invalidates the old world GPU buffers and pointer capture.
    pub fn zone_tick(&mut self) -> bool {
        let event = self
            .zone_state
            .loader
            .as_mut()
            .and_then(assets::Loader::poll);
        if !self.zone_loading() {
            return false;
        }
        match event {
            Some(assets::LoadEvent::Progress { received, total }) => {
                self.zone_state.progress = if total == 0 {
                    0.0
                } else {
                    (received as f32 / total as f32).clamp(0.0, 1.0)
                };
            }
            Some(assets::LoadEvent::Ready(assets)) => {
                self.install_ruins(*assets);
                return true;
            }
            Some(assets::LoadEvent::Failed(error)) => {
                self.zone_state.error = Some(error.chars().take(180).collect());
                self.zone_state.loading = LoadState::Failed;
            }
            None => {}
        }
        false
    }
    /// Install already verified artwork. Offline tools use the same decoder.
    pub fn install_ruins(&mut self, assets: assets::LoadedAssets) {
        // A repeated completion cannot replace the saved plaza return pose.
        if !self.is_plaza() {
            return;
        }
        self.zone_state.plaza_pose = Some((self.player.pos, self.player.yaw));
        let ruins = match Ruins::new(assets) {
            Ok(ruins) => ruins,
            Err(error) => {
                self.zone_state.error = Some(error);
                self.zone_state.loading = LoadState::Failed;
                return;
            }
        };
        self.world = ruins.world();
        self.zone_state.ruins = Some(ruins);
        self.zone = ZoneId::Ruins;
        self.zone_state.loading = LoadState::Idle;
        self.zone_state.error = None;
        self.zone_state.progress = 1.0;
        self.zone_revision = self.zone_revision.saturating_add(1);
        let _ = self.set_spawn(Ruins::spawn(), 0.0);
        self.camera = crate::camera::FollowCamera::default();
    }
    /// Enter the procedurally built L1 station. Nothing is downloaded.
    /// Finishes any pending zone light bake (offline captures).
    pub fn settle_zone_light(&mut self) {
        if let Some(lagrange) = &mut self.zone_state.lagrange {
            lagrange.settle_light();
        }
    }

    pub fn install_lagrange(&mut self) {
        if !self.is_plaza() {
            return;
        }
        self.zone_state.plaza_pose = Some((self.player.pos, self.player.yaw));
        self.zone_cancel_loading();
        let mut zone = Lagrange::new();
        // From the Grid, the station's guides and overlays are neutral.
        zone.neutral = self.is_bare();
        zone.tick();
        self.world = if self.is_bare() {
            Lagrange::neutral_world()
        } else {
            Lagrange::world()
        };
        self.zone_state.lagrange = Some(zone);
        self.zone = ZoneId::Lagrange1;
        self.zone_state.progress = 1.0;
        self.zone_revision = self.zone_revision.saturating_add(1);
        let _ = self.set_spawn(Lagrange::spawn(), Lagrange::spawn_yaw());
        self.camera = crate::camera::FollowCamera::default();
    }
    /// Enter the generated Physics Lab. Nothing is downloaded.
    pub fn install_lab(&mut self) {
        if !self.is_plaza() {
            return;
        }
        self.zone_state.plaza_pose = Some((self.player.pos, self.player.yaw));
        self.zone_cancel_loading();
        self.world = Lab::world();
        self.zone_state.lab = Some(Lab::new());
        self.zone = ZoneId::PhysicsLab;
        self.zone_state.progress = 1.0;
        self.zone_revision = self.zone_revision.saturating_add(1);
        let _ = self.set_spawn(Lab::spawn(), Lab::spawn_yaw());
        self.camera = crate::camera::FollowCamera::default();
    }
    /// The nearest portal in this zone and its destination.
    fn nearest_portal(&self) -> (ZoneId, Vec3) {
        let at = self.player.pos;
        self.zone
            .portals()
            .into_iter()
            .min_by(|a, b| {
                let da = (a.1 - at).x.hypot((a.1 - at).z);
                let db = (b.1 - at).x.hypot((b.1 - at).z);
                da.total_cmp(&db)
            })
            .unwrap_or((ZoneId::Plaza, self.zone.portal()))
    }
    fn portal_in_reach(&self, at: Vec3) -> bool {
        let offset = self.player.pos - at;
        offset.x.hypot(offset.z) <= 6.0
            && (offset.y.abs() < 3.0 || self.is_plaza() && offset.y < 3.0)
    }
    pub fn zone_intent(&mut self, intent: Intent) -> Result<(), String> {
        let result = self.apply_zone_intent(intent);
        if let Err(error) = &result {
            self.zone_state.error = Some(error.chars().take(180).collect());
        }
        result
    }
    fn apply_zone_intent(&mut self, intent: Intent) -> Result<(), String> {
        match intent {
            Intent::Enter | Intent::Retry => {
                if !self.is_plaza() || self.zone_loading() || !self.zone_portal(1.0).near {
                    return Err("Approach a portal".into());
                }
                let destination = if intent == Intent::Retry {
                    self.zone_state.destination
                } else {
                    self.nearest_portal().0
                };
                self.zone_state.destination = destination;
                if destination == ZoneId::Lagrange1 {
                    self.cancel_navigation();
                    self.doors.cancel_transient();
                    self.install_lagrange();
                    return Ok(());
                }
                if destination == ZoneId::PhysicsLab {
                    self.cancel_navigation();
                    self.doors.cancel_transient();
                    self.install_lab();
                    return Ok(());
                }
                super::Manifest::ruins()?;
                let loader = self
                    .zone_state
                    .loader
                    .as_mut()
                    .ok_or("Zone storage is unavailable")?;
                if !loader.request() {
                    return Err("Finishing the previous load; try again".into());
                }
                self.cancel_navigation();
                self.doors.cancel_transient();
                self.zone_state.loading = LoadState::Loading;
                self.zone_state.progress = 0.0;
                self.zone_state.error = None;
            }
            Intent::Cancel => self.zone_cancel_loading(),
            Intent::Return => {
                if self.is_plaza() {
                    return Err("You are already in the plaza".into());
                }
                self.zone_cancel_loading();
                // Dropping a zone releases its decoded frames and simulation.
                self.zone_state.ruins = None;
                self.zone_state.lagrange = None;
                self.zone_state.lab = None;
                self.zone = ZoneId::Plaza;
                self.zone_revision = self.zone_revision.saturating_add(1);
                if let Some(gate) = self.grid_gate() {
                    // Back on the Grid in front of its portal, facing away,
                    // with the ball and blocks where they were left.
                    self.world = crate::world::bare();
                    self.zone_state.plaza_pose = None;
                    let (pos, yaw) = gate.front();
                    self.place_player(pos, yaw)?;
                } else {
                    self.world = crate::world::build();
                    let (pos, yaw) = self
                        .zone_state
                        .plaza_pose
                        .take()
                        .unwrap_or((crate::world::SPAWN, 0.0));
                    self.set_spawn(pos, yaw)?;
                }
                self.zone_state.gate_cooldown = super::gate::COOLDOWN;
                self.camera = crate::camera::FollowCamera::default();
            }
            Intent::Firebolt | Intent::MagicMissile | Intent::Fireball => {
                let spell = match intent {
                    Intent::Firebolt => verse_ruins::Spell::Firebolt,
                    Intent::MagicMissile => verse_ruins::Spell::MagicMissile,
                    _ => verse_ruins::Spell::Fireball,
                };
                let direction = self.player.forward();
                let origin = self.player.pos + Vec3::Y * 1.4 + direction * 0.25;
                let ruins = self.zone_state.ruins.as_mut().ok_or("Enter Ruins first")?;
                ruins
                    .simulation
                    .cast(spell, origin.to_array(), direction.to_array())?;
                self.zone_state.error = None;
            }
            Intent::Forces => {
                let lagrange = self
                    .zone_state
                    .lagrange
                    .as_mut()
                    .ok_or("Enter Lagrange 1 first")?;
                lagrange.overlay = !lagrange.overlay;
                lagrange.tick();
            }
            Intent::Camera => {
                let lagrange = self
                    .zone_state
                    .lagrange
                    .as_mut()
                    .ok_or("Enter Lagrange 1 first")?;
                lagrange.art = !lagrange.art;
                lagrange.tick();
            }
            Intent::Grab | Intent::Release => {
                let lagrange = self
                    .zone_state
                    .lagrange
                    .as_mut()
                    .ok_or("Enter Lagrange 1 first")?;
                lagrange.station.apply(if intent == Intent::Grab {
                    Input::Grab
                } else {
                    Input::Release
                })?;
                lagrange.tick();
                self.zone_state.error = None;
            }
            Intent::KnobPrev
            | Intent::KnobNext
            | Intent::Decrease
            | Intent::Increase
            | Intent::Reset
            | Intent::Pause
            | Intent::Step => {
                let lab = self
                    .zone_state
                    .lab
                    .as_mut()
                    .ok_or("Enter the Physics Lab first")?;
                match intent {
                    Intent::KnobPrev => lab.cycle_knob(false),
                    Intent::KnobNext => lab.cycle_knob(true),
                    Intent::Decrease => lab.adjust(false),
                    Intent::Increase => lab.adjust(true),
                    Intent::Reset => lab.reset(),
                    Intent::Pause => lab.toggle_pause(),
                    _ => lab.single_step(),
                }
                self.zone_state.error = None;
            }
        }
        Ok(())
    }
    /// Applies a NIP-MV zone command from an authorized operator to the
    /// loaded simulation and returns what changed.
    ///
    /// Verbs: `fly X,Y,Z` or `fly LANDMARK`, `grab`, `release`, `stop`,
    /// `status`, and `parts`. `install` and `wait` are headless-only, since
    /// the desktop simulation runs in real time. Anything else is refused.
    ///
    /// # Errors
    ///
    /// Returns a message when the command names another zone, no zone is
    /// loaded, or the simulation refuses the verb.
    pub fn zone_command(&mut self, command: &crate::mv::Command) -> Result<Value, String> {
        use crate::mv::Arg;
        if command.zone != self.zone.world_id() {
            return Err(format!(
                "zone `{}` is not loaded; this operator is in `{}`",
                command.zone,
                self.zone.world_id()
            ));
        }
        let lagrange = self
            .zone_state
            .lagrange
            .as_mut()
            .ok_or("no simulation zone is loaded")?;
        let station = &mut lagrange.station;
        let result = match command.cmd.as_str() {
            "fly" => {
                let target = match command.args.as_slice() {
                    [Arg::Text(name)] => station
                        .landmark(name)
                        .ok_or_else(|| format!("`{name}` is not a landmark or part"))?,
                    [Arg::Number(x), Arg::Number(y), Arg::Number(z)] => DVec3::new(*x, *y, *z),
                    _ => return Err("fly takes X,Y,Z or a landmark name".into()),
                };
                let target = if station.snapshot().carrying.is_some() {
                    target - (station.hands() - station.astronaut().pos)
                } else {
                    target
                };
                station.apply(Input::FlyTo { target })?;
                json!({ "flying_to": target.to_array(), "from": station.astronaut().pos.to_array() })
            }
            "grab" => {
                let kind = station.apply(Input::Grab)?.ok_or("nothing was grabbed")?;
                json!({ "grabbed": kind.name() })
            }
            "release" => {
                let kind = station
                    .apply(Input::Release)?
                    .ok_or("nothing was released")?;
                let installed = station
                    .parts
                    .iter()
                    .any(|part| part.kind == kind && part.state == PartState::Installed);
                json!({ "released": kind.name(), "installed": installed })
            }
            "stop" => {
                station.apply(Input::Stop)?;
                json!({ "stopped": true })
            }
            "status" => serde_json::to_value(station.snapshot()).map_err(|e| e.to_string())?,
            "parts" => json!(
                station
                    .parts
                    .iter()
                    .map(|part| json!({
                        "kind": part.kind.name(),
                        "state": format!("{:?}", part.state).to_lowercase(),
                        "pos": station.body(part).pos.to_array(),
                    }))
                    .collect::<Vec<_>>()
            ),
            "install" | "wait" => {
                return Err(format!(
                    "`{}` runs only in the headless simulator; send fly, grab, release, and stop",
                    command.cmd
                ));
            }
            other => return Err(format!("unknown zone command `{other}`")),
        };
        lagrange.tick();
        self.zone_state.error = None;
        Ok(result)
    }
    /// Map status and marker while an EVA pack autopilot owns map taps.
    #[must_use]
    pub fn eva_map_status(&self) -> Option<(&'static str, Option<[f32; 2]>)> {
        let station = &self.zone_state.lagrange.as_ref()?.station;
        Some(match station.target {
            Some(t) => ("Flying", Some([t.x as f32, t.z as f32])),
            None => ("Choose a point to fly to", None),
        })
    }
    /// Fly the EVA pack toward a map point at the current altitude.
    pub(crate) fn lagrange_fly_to(&mut self, destination: [f32; 2]) -> Option<Result<(), String>> {
        let lagrange = self.zone_state.lagrange.as_mut()?;
        let y = lagrange.station.astronaut().pos.y;
        Some(
            lagrange
                .station
                .apply(Input::FlyTo {
                    target: glam::DVec3::new(
                        f64::from(destination[0]),
                        y,
                        f64::from(destination[1]),
                    ),
                })
                .map(|_| ()),
        )
    }
    pub fn zone_snapshot(&self, aspect: f32) -> Snapshot {
        let portal = self.zone_portal(aspect);
        let mut controls = Vec::new();
        let mut add = |id, label, action, enabled| {
            controls.push(Control {
                id,
                label: String::from(label),
                action,
                enabled,
            })
        };
        let combat = self.zone_state.ruins.as_ref().map(|f| f.snapshot.clone());
        let caption = if self.zone_loading() {
            add("cancel", "Cancel", Intent::Cancel, true);
            format!(
                "Loading Ruins · {}%",
                (self.zone_state.progress * 100.0) as u32
            )
        } else if self.zone_state.loading == LoadState::Failed {
            add("retry", "Retry", Intent::Retry, portal.near);
            add("cancel", "Dismiss", Intent::Cancel, true);
            "Ruins could not load".into()
        } else if self.zone == ZoneId::Ruins {
            if let Some(c) = &combat {
                for ability in &c.abilities {
                    let (id, label, intent) = match ability.id {
                        verse_ruins::Spell::Firebolt => ("firebolt", "Firebolt", Intent::Firebolt),
                        verse_ruins::Spell::MagicMissile => {
                            ("magic_missile", "Missile", Intent::MagicMissile)
                        }
                        verse_ruins::Spell::Fireball => ("fireball", "Fireball", Intent::Fireball),
                    };
                    add(id, label, intent, ability.ready);
                }
                add("return", "Plaza", Intent::Return, true);
                if c.player.hp <= 0 {
                    "Defeated · return to Plaza".into()
                } else {
                    format!(
                        "HP {} / {}   Mana {} / {}",
                        c.player.hp, c.player.max_hp, c.player.mana, c.player.max_mana
                    )
                }
            } else {
                String::new()
            }
        } else if let Some(lagrange) = &self.zone_state.lagrange {
            let s = lagrange.station.snapshot();
            if s.carrying.is_some() {
                add(
                    "release",
                    if s.latch_ready { "Latch" } else { "Release" },
                    Intent::Release,
                    true,
                );
            } else {
                add("grab", "Grab", Intent::Grab, s.can_grab);
            }
            add(
                "forces",
                if lagrange.overlay {
                    "Hide forces"
                } else {
                    "Forces"
                },
                Intent::Forces,
                true,
            );
            add(
                "camera",
                if lagrange.art { "Photo" } else { "Art" },
                Intent::Camera,
                true,
            );
            add("return", self.return_label(), Intent::Return, true);
            let status = if let Some(kind) = s.carrying {
                match s.latch_distance_m {
                    Some(_) if s.latch_ready => format!("{} aligned · latch", kind.name()),
                    Some(d) => format!("{} · {:.0} kg · jig {:.1} m", kind.name(), kind.mass(), d),
                    None => kind.name().to_owned(),
                }
            } else if let Some(message) = &s.message {
                message.clone()
            } else if let Some(next) = s.next_part {
                format!("Next: {} at the depot", next.name().to_lowercase())
            } else {
                "Keel frame complete".into()
            };
            let status = if s.keeping_active && s.carrying.is_none() {
                format!(
                    "Day {:.0} · keeping burn · {:.2} m/s total",
                    s.orbit.mission_days, s.orbit.keeping_dv_m_s
                )
            } else {
                status
            };
            let mut sensed = format!("{:.2} g · spin {:.0}°/s", s.g_load, s.spin_deg_s);
            if let Some(ahead) = s.proximity_m {
                sensed.push_str(&format!(" · {ahead:.1} m ahead"));
            }
            if lagrange.overlay {
                sensed.push_str(&format!(
                    " · step {:.2} ms · {} awake",
                    s.step_ms, s.awake_bodies
                ));
            }
            if s.impact_n > 1.0 {
                sensed.push_str(&format!(" · impact {:.0} N", s.impact_n));
            }
            format!(
                "Earth {:.2}M km · N2 {:.1} kg · {:.1} m/s\n{sensed}\n{}",
                s.orbit.earth_distance_km / 1.0e6,
                s.propellant_kg,
                s.speed_m_s,
                status
            )
        } else if let Some(lab) = &self.zone_state.lab {
            add("knob_prev", "Prev", Intent::KnobPrev, true);
            add("knob_next", "Next", Intent::KnobNext, true);
            add("decrease", "-", Intent::Decrease, true);
            add("increase", "+", Intent::Increase, true);
            add("reset", "Reset", Intent::Reset, true);
            add(
                "pause",
                if lab.paused { "Run" } else { "Pause" },
                Intent::Pause,
                true,
            );
            add("step", "Step", Intent::Step, true);
            add("return", "Plaza", Intent::Return, true);
            Lab::caption(&lab.snapshot())
        } else if portal.near && portal.visible {
            if self.nearest_portal().0 == ZoneId::Lagrange1 {
                add("enter", "Enter L1", Intent::Enter, true);
                "Lagrange 1 · Sun–Earth L1 station".into()
            } else if self.nearest_portal().0 == ZoneId::PhysicsLab {
                add("enter", "Enter Lab", Intent::Enter, true);
                "Physics Lab · live rigid-body sandbox".into()
            } else {
                add(
                    "enter",
                    "Enter Ruins",
                    Intent::Enter,
                    self.zone_state.loader.is_some(),
                );
                "Ruins · load this zone".into()
            }
        } else {
            String::new()
        };
        Snapshot {
            id: self.zone,
            label: self.zone_label(),
            state: self.zone_state.loading,
            progress: self.zone_state.progress,
            error: self.zone_state.error.clone(),
            portal,
            controls,
            combat,
            station: self
                .zone_state
                .lagrange
                .as_ref()
                .map(|l| l.station.snapshot()),
            lab: self.zone_state.lab.as_ref().map(Lab::snapshot),
            caption,
        }
    }
    fn zone_portal(&self, aspect: f32) -> PortalProjection {
        let at = self.nearest_portal().1;
        let offset = self.player.pos - at;
        let distance = offset.x.hypot(offset.z);
        let anchor = at + Vec3::new(0.0, 2.5, -0.3);
        let view = self.view(aspect);
        let clip = view.view_proj * anchor.extend(1.0);
        let mut p = PortalProjection {
            // The Grid's own portal is walked through, not tapped.
            near: !(self.is_bare() && self.is_plaza()) && self.portal_in_reach(at),
            visible: false,
            screen_x: 0.5,
            screen_y: 0.5,
            distance,
        };
        if aspect.is_finite() && aspect > 0.0 && clip.is_finite() && clip.w > 0.0 {
            let ndc = clip.truncate() / clip.w;
            p.screen_x = (ndc.x * 0.5 + 0.5).clamp(0.0, 1.0);
            p.screen_y = (0.5 - ndc.y * 0.5).clamp(0.0, 1.0);
            p.visible = (-1.0..=1.0).contains(&ndc.x)
                && (-1.0..=1.0).contains(&ndc.y)
                && (0.0..=1.0).contains(&ndc.z);
        }
        if p.visible && p.near {
            let delta = anchor - view.eye;
            let length = delta.length();
            let direction = delta.normalize_or_zero();
            p.visible =
                !crate::runtime::mesh_occludes(&self.world.mesh, view.eye, direction, length)
                    && !self.zone_dynamic_occludes(view.eye, direction, length);
        }
        p
    }
    /// Pick a nearby visible portal opening; an occluding surface blocks entry.
    pub fn zone_hit(&self, aspect: f32, x: f32, y: f32) -> bool {
        self.zone_hit_with_entities(aspect, x, y, &crate::mesh::Mesh::default())
    }
    /// Include the currently presented remote entities when admitting a tap.
    pub fn zone_hit_with_entities(
        &self,
        aspect: f32,
        x: f32,
        y: f32,
        entities: &crate::mesh::Mesh,
    ) -> bool {
        let portal = self.nearest_portal().1;
        // The Grid's portal is walked through; nothing on the Grid is tapped.
        if self.is_bare() && self.is_plaza() || !self.portal_in_reach(portal) || self.zone_loading()
        {
            return false;
        }
        let view = self.view(aspect);
        let Some((eye, direction)) = crate::runtime::viewport_ray(&view, aspect, x, y) else {
            return false;
        };
        let at = portal + Vec3::new(0.0, 2.25, -0.3);
        if direction.z.abs() < 0.0001 {
            return false;
        }
        let distance = (at.z - eye.z) / direction.z;
        let hit = eye + direction * distance;
        let clip = view.view_proj * hit.extend(1.0);
        distance > 0.0
            && (hit.x - at.x).abs() <= 1.75
            && (hit.y - at.y).abs() <= 2.1
            && clip.w > 0.0
            && (0.0..=1.0).contains(&(clip.z / clip.w))
            && !crate::runtime::mesh_occludes(&self.world.mesh, eye, direction, distance)
            && !self.zone_dynamic_occludes(eye, direction, distance)
            && !crate::runtime::mesh_occludes(entities, eye, direction, distance)
    }
    fn zone_dynamic_occludes(&self, eye: Vec3, direction: Vec3, distance: f32) -> bool {
        if let Some(ruins) = &self.zone_state.ruins {
            crate::runtime::mesh_occludes(ruins.dynamic(), eye, direction, distance)
        } else if let Some(lagrange) = &self.zone_state.lagrange {
            crate::runtime::mesh_occludes(lagrange.dynamic(), eye, direction, distance)
        } else if let Some(lab) = &self.zone_state.lab {
            crate::runtime::mesh_occludes(lab.dynamic(), eye, direction, distance)
        } else {
            crate::runtime::mesh_occludes(&self.dynamic_mesh(), eye, direction, distance)
        }
    }
    /// The Grid's walk-in portal while on the Grid; none elsewhere.
    #[must_use]
    pub fn grid_gate(&self) -> Option<super::Gate> {
        self.ball().map(|ball| super::Gate::grid(&ball.layout()))
    }

    /// The label of the control that leaves a zone.
    fn return_label(&self) -> &'static str {
        if self.is_bare() { GRID_LABEL } else { "Plaza" }
    }

    /// The display name of where the player is. The bare world's grid is
    /// "The Grid"; Coder's plaza and every zone keep their own names.
    #[must_use]
    pub fn zone_label(&self) -> &'static str {
        if self.is_bare() && self.is_plaza() {
            GRID_LABEL
        } else {
            self.zone.label()
        }
    }

    /// The walk-in arches in the neutral palette: the Grid's portal to
    /// Lagrange 1, or a zone's return arch lettered for the Grid.
    pub(crate) fn grid_portal_mesh(&self) -> crate::mesh::Mesh {
        let elapsed = self.zone_state.elapsed;
        if self.is_plaza() {
            return self
                .grid_gate()
                .map_or_else(crate::mesh::Mesh::default, |gate| {
                    gate.mesh(ZoneId::Plaza, ZoneId::Lagrange1.sign(), elapsed)
                });
        }
        let mut mesh = crate::mesh::Mesh::default();
        for (_, at) in self.zone.portals() {
            mesh.extend(&super::Gate::fixed(at).mesh(self.zone, "THE GRID", elapsed));
        }
        mesh
    }

    /// On the Grid, walking through its portal enters Lagrange 1, and in
    /// Lagrange 1 flying through the return arch comes back. Feet moved
    /// from `from` to the player's position this frame. Coder's plaza
    /// keeps its tapped arches and buttons.
    pub(crate) fn walk_through_portals(&mut self, from: Vec3, dt: f32) {
        if !self.is_bare() {
            return;
        }
        self.zone_state.gate_cooldown = (self.zone_state.gate_cooldown - dt).max(0.0);
        if self.zone_state.gate_cooldown > 0.0 || self.zone_loading() {
            return;
        }
        let to = self.player.pos;
        if self.is_plaza() {
            if self.grid_gate().is_some_and(|gate| gate.crossed(from, to)) {
                self.cancel_navigation();
                self.doors.cancel_transient();
                self.zone_state.destination = ZoneId::Lagrange1;
                self.install_lagrange();
                self.zone_state.gate_cooldown = super::gate::COOLDOWN;
            }
        } else if self
            .zone
            .portals()
            .iter()
            .any(|&(_, at)| super::Gate::fixed(at).crossed(from, to))
        {
            let _ = self.zone_intent(Intent::Return);
        }
    }

    pub(crate) fn ruins_tick(&mut self, dt: f32, _previous: PlayerController) {
        self.zone_state.elapsed = (self.zone_state.elapsed + dt) % 1000.0;
        if let Some(ruins) = &mut self.zone_state.ruins
            && let Err(error) = ruins.tick(dt, &self.player)
        {
            self.zone_state.error = Some(error);
        }
        if let Some(lagrange) = &mut self.zone_state.lagrange {
            lagrange.tick();
        }
        if let Some(lab) = &mut self.zone_state.lab {
            lab.tick(dt);
        }
    }
    pub(crate) fn zone_dynamic_mesh(&self) -> crate::mesh::Mesh {
        let elapsed = self.zone_state.elapsed;
        let mut mesh = if self.is_bare() {
            self.grid_portal_mesh()
        } else {
            super::portal_mesh(self.zone, elapsed)
        };
        if let Some(ruins) = &self.zone_state.ruins {
            mesh.extend(ruins.dynamic());
        }
        if let Some(lagrange) = &self.zone_state.lagrange {
            mesh.extend(lagrange.dynamic());
        }
        if let Some(lab) = &self.zone_state.lab {
            mesh.extend(lab.dynamic());
            // The lab has no suit of its own; the plaza character walks it.
            mesh.extend(&crate::avatar::mesh(&self.player, &self.gait));
        }
        mesh
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{controller::InputState, mesh::Mesh};

    fn ruins_world() -> WorldRuntime {
        let animation = || assets::AnimatedMesh {
            frames: vec![Mesh::default()],
            frame_seconds: 0.1,
        };
        let mut world = WorldRuntime::new();
        world.install_ruins(assets::LoadedAssets {
            tree: Mesh::default(),
            wizard_still: Mesh::default(),
            wizard: animation(),
            zombie: animation(),
            zombie_walk: animation(),
        });
        world
    }

    #[test]
    fn ruins_runs_immediately_and_fireball_spends_mana_while_moving() {
        let mut world = ruins_world();
        let initial = world.zone_snapshot(1.0).combat.unwrap();
        assert_eq!(
            initial.actors.iter().filter(|a| a.kind == "zombie").count(),
            35
        );
        assert_eq!(world.player.pos, Ruins::spawn());
        let moving = InputState {
            forward: true,
            ..Default::default()
        };
        world.zone_intent(Intent::Fireball).unwrap();
        world.tick(&moving, 0.05);
        let cast = world.zone_snapshot(1.0).combat.unwrap();
        assert!(cast.player.mana < initial.player.mana);
        assert!(
            cast.abilities
                .iter()
                .find(|a| a.id == verse_ruins::Spell::Fireball)
                .unwrap()
                .cooldown_remaining
                > 0.0
        );
        assert!(world.player.pos.z > 0.0);
        assert!(
            cast.projectiles
                .iter()
                .any(|p| p.kind == verse_ruins::Spell::Fireball)
        );
        for _ in 0..80 {
            world.tick(&moving, 0.05);
        }
        let later = world.zone_snapshot(1.0).combat.unwrap();
        assert!(world.player.pos.z > 20.0, "no per-turn movement allowance");
        assert!(
            initial
                .actors
                .iter()
                .filter(|a| a.kind == "zombie")
                .any(|a| later.actors.iter().any(|b| b.id == a.id && b.pos != a.pos))
        );
        assert!(
            later
                .abilities
                .iter()
                .find(|a| a.id == verse_ruins::Spell::Fireball)
                .unwrap()
                .ready
        );
        assert!(
            !world
                .zone_snapshot(1.0)
                .controls
                .iter()
                .any(|c| c.id.contains("turn") && c.id != "return")
        );
    }

    #[test]
    fn ruins_uses_source_strafe_and_terrain_camera_and_stops_defeated_players() {
        let mut world = ruins_world();
        let moving = InputState {
            left: true,
            mouse_look: true,
            ..Default::default()
        };
        world.tick(&moving, 0.05);
        assert!(world.player.pos.x.abs() > 0.1);
        assert_eq!(world.player.yaw, 0.0);
        world.camera.distance = crate::camera::MIN_DISTANCE;
        world.camera.pitch = -0.6;
        let view = world.view(1.0);
        let floor = verse_ruins::scene::Terrain::bundled().height(view.eye.x, view.eye.z);
        assert!(view.eye.y < 0.0, "camera follows negative terrain");
        assert!(view.eye.y >= floor + 0.399);
        world.zone_state.ruins.as_mut().unwrap().snapshot.player.hp = 0;
        let before = world.player.pos;
        world.update_player(&moving, 0.05);
        assert_eq!(world.player.pos, before);
        assert!(world.zone_snapshot(1.0).caption.contains("Defeated"));
        assert!(
            world
                .zone_snapshot(1.0)
                .controls
                .iter()
                .find(|c| c.id == "return")
                .unwrap()
                .enabled
        );
    }

    #[test]
    fn return_drops_combat_and_restores_the_plaza_pose() {
        let mut world = WorldRuntime::new();
        let original = world.player;
        let mut ruins = ruins_world();
        world.install_ruins(ruins.zone_state.ruins.take().unwrap().assets);
        world.zone_intent(Intent::Firebolt).unwrap();
        world.tick(&InputState::default(), 0.05);
        world.zone_intent(Intent::Return).unwrap();
        assert_eq!(world.player, original);
        assert!(world.zone_snapshot(1.0).combat.is_none());
        assert!(world.zone_state.ruins.is_none());
    }

    fn command(zone: &str, cmd: &str, args: Vec<crate::mv::Arg>) -> crate::mv::Command {
        crate::mv::Command {
            v: 1,
            zone: zone.into(),
            cmd: cmd.into(),
            args,
            t: 0,
            id: "c1".into(),
        }
    }

    #[test]
    fn zone_command_refuses_without_a_simulation_or_in_another_zone() {
        let mut world = WorldRuntime::new();
        let error = world
            .zone_command(&command("lagrange-1-v1", "status", vec![]))
            .unwrap_err();
        assert!(error.contains("not loaded"), "{error}");
        world.install_lagrange();
        let error = world
            .zone_command(&command("plaza", "status", vec![]))
            .unwrap_err();
        assert!(error.contains("not loaded"), "{error}");
    }

    #[test]
    fn zone_command_flies_grabs_and_refuses_headless_verbs() {
        use crate::mv::Arg;
        let mut world = WorldRuntime::new();
        world.install_lagrange();
        let zone = "lagrange-1-v1";
        let before = world
            .zone_state
            .lagrange
            .as_ref()
            .unwrap()
            .station
            .astronaut()
            .pos;
        let flown = world
            .zone_command(&command(zone, "fly", vec![Arg::Text("depot".into())]))
            .unwrap();
        assert!(flown["flying_to"].is_array());
        assert!(
            world
                .zone_state
                .lagrange
                .as_ref()
                .unwrap()
                .station
                .target
                .is_some()
        );
        world.zone_command(&command(zone, "stop", vec![])).unwrap();
        assert!(
            world
                .zone_state
                .lagrange
                .as_ref()
                .unwrap()
                .station
                .target
                .is_none()
        );
        let parts = world.zone_command(&command(zone, "parts", vec![])).unwrap();
        assert_eq!(parts.as_array().unwrap().len(), 6);
        let status = world
            .zone_command(&command(zone, "status", vec![]))
            .unwrap();
        assert!(status.is_object());
        for verb in ["install", "wait", "explode"] {
            let error = world
                .zone_command(&command(zone, verb, vec![]))
                .unwrap_err();
            assert!(!error.is_empty(), "{verb}");
        }
        let error = world
            .zone_command(&command(zone, "fly", vec![Arg::Text("nowhere".into())]))
            .unwrap_err();
        assert!(error.contains("nowhere"), "{error}");
        assert!(
            world
                .zone_state
                .lagrange
                .as_ref()
                .unwrap()
                .station
                .astronaut()
                .pos
                .is_finite(),
            "refused commands leave the station intact (started at {before})"
        );
    }
}
