//! CAD placement polling and Verse runtime integration.

#[cfg(not(target_arch = "wasm32"))]
use super::ZoneId;
use crate::pbr::LitVertex;
use glam::Vec3;
use std::path::PathBuf;
#[cfg(not(target_arch = "wasm32"))]
use std::{path::Path, time::SystemTime};
pub use verse_cad_sync::cad_place::*;

/// The models placed in the current zone, kept in step with the file.
#[derive(Default)]
pub struct Placements {
    /// What was last read: the zone and the file's time and length.
    #[cfg(not(target_arch = "wasm32"))]
    seen: Option<(ZoneId, PathBuf, Option<SystemTime>, u64)>,
    #[cfg(not(target_arch = "wasm32"))]
    pub source: Option<PathBuf>,
    pub supplied: bool,
    pub sync: Option<(String, u64)>,
    clock: f32,
    pub placed: Vec<Placed>,
    /// Placements that couldn't stand, in plain words.
    pub problems: Vec<String>,
}

impl Placements {
    /// Whether it's time to look at the file again, `dt` seconds on.
    pub fn due(&mut self, dt: f32) -> bool {
        self.clock += dt;
        if self.clock >= POLL_SECONDS {
            self.clock = 0.0;
            return true;
        }
        false
    }

    /// Reads `file` again when it or the zone changed, standing every
    /// placement for `zone`. Returns whether anything changed.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn refresh(
        &mut self,
        zone: ZoneId,
        file: &Path,
        ground: impl Fn(f32, f32) -> f32,
        default_at: Vec3,
    ) -> bool {
        self.source = Some(file.to_owned());
        self.supplied = false;
        let meta = std::fs::metadata(file).ok();
        let stamp = (
            zone,
            file.to_owned(),
            meta.as_ref().and_then(|m| m.modified().ok()),
            meta.as_ref().map_or(0, std::fs::Metadata::len),
        );
        if self.seen.as_ref() == Some(&stamp) {
            return false;
        }
        self.seen = Some(stamp);
        self.placed.clear();
        self.problems.clear();
        let list = match read(file) {
            Ok(list) => list,
            Err(e) => {
                self.problems.push(e);
                return true;
            }
        };
        let mut triangles = 0;
        let mut keys = std::collections::BTreeSet::new();
        for placement in list.iter().filter(|p| p.zone == zone.world_id()) {
            if keys.len() >= 20 || !keys.insert(placement.key().to_owned()) {
                self.problems
                    .push("Use different names for up to 20 placed models.".into());
                continue;
            }
            match stand(placement, &ground, default_at) {
                Ok(mut placed) if triangles + placed.vertices.len() / 3 <= MAX_TRIANGLES => {
                    if let Some(same) = self.placed.iter().find(|p| p.geometry == placed.geometry) {
                        placed.model = same.model.clone();
                    }
                    triangles += placed.vertices.len() / 3;
                    self.placed.push(placed);
                }
                Ok(_) => self.problems.push(format!(
                    "{} v{} would take {zone:?} past {MAX_TRIANGLES} placed triangles",
                    placement.doc, placement.version
                )),
                Err(e) => self.problems.push(e),
            }
        }
        for problem in &self.problems {
            eprintln!("verse: a CAD model can't stand here: {problem}");
        }
        true
    }

    /// Every placed model's triangles.
    #[must_use]
    pub fn lit(&self) -> Vec<LitVertex> {
        self.placed
            .iter()
            .flat_map(|p| p.vertices.iter().copied())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROVER: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/verse/cad/rover.glb"
    );

    fn file(dir: &Path, placements: &serde_json::Value) -> PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join("placements.json");
        std::fs::write(
            &path,
            serde_json::json!({"kind": KIND, "placements": placements}).to_string(),
        )
        .unwrap();
        path
    }

    #[test]
    fn runtime_keeps_copies_moving_and_removes_replaced_bodies() {
        let mut runtime = crate::runtime::WorldRuntime::new();
        runtime.install_everglade(super::super::everglade_tests::pack());
        let bytes = std::fs::read(ROVER).unwrap();
        let p:Placement=serde_json::from_value(serde_json::json!({"id":"copy","doc":"rover","version":1,"zone":"verse-everglade","glb":"remote","at":[0.,3.,-28.]})).unwrap();
        runtime
            .install_cad_models(vec![(p.clone(), bytes.clone())])
            .unwrap();
        let initial = runtime.cad_report("copy").unwrap().positions[0][1];
        for _ in 0..15 {
            runtime.tick(&crate::controller::InputState::default(), 1. / 120.);
        }
        let falling = runtime.cad_report("copy").unwrap().positions;
        assert!(falling[0][1] < initial - 0.02);
        runtime
            .install_cad_models(vec![(p.clone(), bytes.clone())])
            .unwrap();
        assert_eq!(runtime.cad_report("copy").unwrap().positions, falling);
        let mut newer = p;
        newer.version = 2;
        runtime.install_cad_models(vec![(newer, bytes)]).unwrap();
        assert!(runtime.cad_report("copy").unwrap().positions[0][1] > falling[0][1] + 0.02);
        runtime.install_cad_models(Vec::new()).unwrap();
        assert!(runtime.cad_report("copy").is_none());
        assert_eq!(runtime.cad_placed().len(), 0);
    }

    #[test]
    fn two_rovers_stand_at_the_requested_spawn_positions() {
        let mut runtime = crate::runtime::WorldRuntime::new();
        runtime.install_everglade(super::super::everglade_tests::pack());
        let bytes = std::fs::read(ROVER).unwrap();
        let models=[-2.5,2.5].into_iter().enumerate().map(|(i,x)| {
            let p:Placement=serde_json::from_value(serde_json::json!({"id":format!("copy-{i}"),"doc":"rover","version":38,"zone":"verse-everglade","glb":"remote","at":[x,0.,-28.]})).unwrap();(p,bytes.clone())
        }).collect();
        runtime.install_cad_models(models).unwrap();
        for _ in 0..120 {
            runtime.tick(&crate::controller::InputState::default(), 1. / 60.);
        }
        for id in ["copy-0", "copy-1"] {
            assert_eq!(runtime.cad_report(id).unwrap().broken, 0, "{id}");
            let placed = runtime
                .cad_placed()
                .iter()
                .find(|p| p.placement.key() == id)
                .unwrap();
            let index = placed
                .model
                .solids
                .iter()
                .position(|s| s.instance == "chassis")
                .unwrap();
            let rotation =
                glam::DQuat::from_array(runtime.cad_report(id).unwrap().orientations[index])
                    * placed.model.solids[index].frame.inverse();
            assert!(
                (rotation * glam::DVec3::Y).y > 0.8,
                "{id} tipped over without a load"
            );
        }
    }

    #[test]
    fn a_placed_version_stands_on_the_ground_and_a_newer_one_replaces_it() {
        let dir = std::env::temp_dir().join(format!("verse-cad-place-{}", std::process::id()));
        let ground = |x: f32, z: f32| 0.1 * x + 0.05 * z;
        let path = file(
            &dir,
            &serde_json::json!([
                {"doc": "rover", "version": 38, "zone": "verse-everglade", "glb": ROVER, "at": [4.0, 0.0, -2.0], "heading_deg": 90.0},
                {"doc": "rover", "version": 38, "zone": "verse-coast", "glb": ROVER}
            ]),
        );
        let mut places = Placements::default();
        assert!(places.refresh(ZoneId::Everglade, &path, ground, Vec3::ZERO));
        assert_eq!(places.placed.len(), 1, "{:?}", places.problems);
        let rover = &places.placed[0];
        assert_eq!(rover.origin, Vec3::new(4.0, 0.3, -2.0));
        // The rover v38 is 909 × 1122 × 980 mm (X forward, Z up, Y left).
        assert!((rover.size.y - 1.122).abs() < 0.01, "{}", rover.size);
        let lowest = rover
            .vertices
            .iter()
            .map(|v| v.pos[1])
            .fold(f32::MAX, f32::min);
        assert!((lowest - 0.3).abs() < 1e-4, "{lowest}");
        // Turned a quarter: its length now lies along Z.
        let (lo, hi) = rover.vertices.iter().fold(
            (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
            |(lo, hi), v| (lo.min(Vec3::from(v.pos)), hi.max(Vec3::from(v.pos))),
        );
        assert!(((hi.z - lo.z) - 0.909).abs() < 0.01, "{lo} {hi}");
        assert!(!places.refresh(ZoneId::Everglade, &path, ground, Vec3::ZERO));
        // Another zone sees only its own.
        assert!(places.refresh(ZoneId::Coast, &path, ground, Vec3::new(1.0, 2.0, 3.0)));
        assert_eq!(places.placed[0].origin, Vec3::new(1.0, 2.0, 3.0));
        // A newer version replaces the old in the file; the zone follows.
        std::thread::sleep(std::time::Duration::from_millis(20));
        file(
            &dir,
            &serde_json::json!([{"doc": "rover", "version": 39, "zone": "verse-everglade", "glb": ROVER, "scale": 0.5}]),
        );
        assert!(places.refresh(ZoneId::Everglade, &path, ground, Vec3::ZERO));
        assert_eq!(places.placed.len(), 1);
        assert_eq!(places.placed[0].placement.version, 39);
        assert!((places.placed[0].size.y - 0.561).abs() < 0.01);
        // A missing glTF is a plain problem, not a crash.
        file(
            &dir,
            &serde_json::json!([{"doc": "gone", "version": 1, "zone": "verse-everglade", "glb": dir.join("none.glb")}]),
        );
        assert!(places.refresh(ZoneId::Everglade, &path, ground, Vec3::ZERO));
        assert!(places.placed.is_empty() && places.problems.len() == 1);
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[cfg(test)]
mod shared_tests {
    use super::*;
    use crate::{
        controller::InputState,
        runtime::WorldRuntime,
        zones::{
            Intent,
            everglade::cad_physics::sync::{Object, Snapshot},
        },
    };
    fn client() -> WorldRuntime {
        let mut r = WorldRuntime::new();
        r.install_everglade(super::super::everglade_tests::pack());
        let placement=serde_json::from_value(serde_json::json!({"id":"shared-rover","doc":"rover","version":1,"generation":"first","zone":"verse-everglade","glb":"/cad-world/rover.glb","at":[0.,0.,-28.]})).unwrap();
        r.install_cad_models(vec![(
            placement,
            include_bytes!("../../../../assets/verse/cad/rover.glb").to_vec(),
        )])
        .unwrap();
        r
    }
    fn snapshot(r: &mut WorldRuntime, revision: u64) -> Snapshot {
        let bindings = r.cad_bindings();
        let states = r.everglade_zone_mut().unwrap().cad_site_mut().cad_states();
        Snapshot {
            account: "fixture-alice".into(),
            revision,
            objects: bindings
                .into_iter()
                .map(|binding| Object {
                    state: states[&binding.id].clone(),
                    binding,
                })
                .collect(),
        }
    }
    #[test]
    fn two_native_views_follow_each_solid_and_reject_foreign_or_stale_snapshots() {
        let mut authority = client();
        let mut a = client();
        let mut b = client();
        a.enable_cad_sync("fixture-alice".into());
        b.enable_cad_sync("fixture-alice".into());
        authority.set_spawn(Vec3::new(0., 0., -30.), 0.).unwrap();
        let initial = snapshot(&mut authority, 1);
        for _ in 0..30 {
            authority.tick(
                &InputState {
                    forward: true,
                    ..Default::default()
                },
                1. / 60.,
            );
        }
        authority.set_spawn(Vec3::new(0., 0., -32.), 0.).unwrap();
        authority.zone_intent(Intent::ReverseGravity).unwrap();
        for _ in 0..90 {
            authority.tick(&InputState::default(), 1. / 60.);
        }
        let center = Vec3::from_array(
            authority.cad_report("shared-rover").unwrap().positions[0].map(|x| x as f32),
        );
        authority
            .everglade_zone_mut()
            .unwrap()
            .cad_site_mut()
            .explode(center, 8., 0, 68.);
        for _ in 0..10 {
            authority.tick(&InputState::default(), 1. / 60.);
        }
        let shared = snapshot(&mut authority, 2);
        assert_ne!(shared.objects[0].state, initial.objects[0].state);
        assert!(shared.objects[0].state.broken.iter().any(|b| *b));
        for r in [&mut a, &mut b] {
            r.apply_cad_snapshot(&shared).unwrap();
            let geometry = &r.cad_placed()[0].vertices;
            assert!(!geometry.is_empty());
            assert!(
                r.dynamic_mesh()
                    .lit
                    .windows(geometry.len())
                    .any(|drawn| drawn == geometry),
                "the frame must include the CAD solids"
            );
            let report = r.cad_report("shared-rover").unwrap();
            r.tick(&InputState::default(), 1. / 60.);
            assert_eq!(
                r.cad_report("shared-rover").unwrap().positions,
                report.positions
            );
            assert!(r.apply_cad_snapshot(&initial).is_err());
            let mut foreign = shared.clone();
            foreign.account = "another-account".into();
            assert!(r.apply_cad_snapshot(&foreign).is_err());
            let mut stale = shared.clone();
            stale.objects[0].binding.version = 0;
            assert!(r.apply_cad_snapshot(&stale).is_err());
        }
        assert_eq!(
            a.cad_report("shared-rover").unwrap().positions,
            b.cad_report("shared-rover").unwrap().positions
        );
        assert_eq!(
            a.cad_report("shared-rover").unwrap().orientations,
            b.cad_report("shared-rover").unwrap().orientations
        );
        let mut reconnect = client();
        reconnect.enable_cad_sync("fixture-alice".into());
        reconnect.apply_cad_snapshot(&shared).unwrap();
        assert_eq!(
            reconnect.cad_report("shared-rover").unwrap().positions,
            a.cad_report("shared-rover").unwrap().positions
        );
    }
}
