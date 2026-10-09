//! Bounded source geometry for rebuilding an exact scoped query scene.
use super::*;

pub const SNAPSHOT_COLLIDERS: usize = 4096;
pub const SNAPSHOT_TRIANGLES: usize = 16384;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneSnapshot {
    pub instance: u64,
    pub colliders: Vec<ShapeSnapshot>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShapeSnapshot {
    pub key: ColliderKey,
    pub layers: u32,
    pub usage: Usage,
    pub pose: Pose,
    pub geometry: GeometrySnapshot,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum GeometrySnapshot {
    Box { min: DVec3, max: DVec3 },
    Triangles { triangles: Vec<Triangle> },
    Capsule { a: DVec3, b: DVec3, radius: f64 },
}
impl SceneSnapshot {
    /// Checks source bounds without allocating compiled mesh hierarchies.
    pub fn validate(&self, instance: u64) -> Result<(), String> {
        if self.instance != instance || self.colliders.len() > SNAPSHOT_COLLIDERS {
            return Err("Invalid collision snapshot scope or collider budget".into());
        }
        let mut keys = std::collections::BTreeSet::new();
        let mut triangles = 0usize;
        for shape in &self.colliders {
            if shape.key.life.instance != instance || !keys.insert(shape.key) {
                return Err("Duplicate or foreign collision snapshot identity".into());
            }
            valid_point(shape.pose.position)?;
            if !shape.pose.rotation.is_finite()
                || (shape.pose.rotation.length_squared() - 1.).abs() > 1e-8
            {
                return Err("Invalid collision snapshot pose".into());
            }
            match &shape.geometry {
                GeometrySnapshot::Box { min, max } => {
                    valid_point(*min)?;
                    valid_point(*max)?;
                    if !min.cmplt(*max).all() {
                        return Err("Invalid collision snapshot box".into());
                    }
                }
                GeometrySnapshot::Capsule { a, b, radius } => {
                    Capsule {
                        a: *a,
                        b: *b,
                        radius: *radius,
                    }
                    .validate()?;
                }
                GeometrySnapshot::Triangles { triangles: source } => {
                    triangles = triangles
                        .checked_add(source.len())
                        .ok_or("Collision snapshot triangle budget exceeded")?;
                    if source.is_empty() || triangles > SNAPSHOT_TRIANGLES {
                        return Err("Collision snapshot triangle budget exceeded".into());
                    }
                    for triangle in source {
                        for point in triangle.0 {
                            valid_point(point)?;
                        }
                        if (triangle.0[1] - triangle.0[0])
                            .cross(triangle.0[2] - triangle.0[0])
                            .length_squared()
                            < 1e-20
                        {
                            return Err("Degenerate collision snapshot triangle".into());
                        }
                    }
                }
            }
        }
        Ok(())
    }
    pub fn compile(&self, instance: u64) -> Result<Scene, String> {
        self.validate(instance)?;
        let mut scene = Scene::default();
        for shape in &self.colliders {
            if let GeometrySnapshot::Capsule { a, b, radius } = &shape.geometry {
                scene.insert_capsule(CapsuleCollider {
                    key: shape.key,
                    layers: shape.layers,
                    usage: shape.usage,
                    capsule: Capsule {
                        a: *a,
                        b: *b,
                        radius: *radius,
                    },
                })?;
            } else {
                let mesh = match &shape.geometry {
                    GeometrySnapshot::Box { min, max } => Mesh::from_box(*min, *max)?,
                    GeometrySnapshot::Triangles { triangles } => Mesh::compile(triangles.clone())?,
                    GeometrySnapshot::Capsule { .. } => unreachable!(),
                };
                scene.insert(MeshCollider {
                    key: shape.key,
                    layers: shape.layers,
                    usage: shape.usage,
                    mesh,
                })?;
            }
            scene.set_pose(shape.key, shape.pose)?;
        }
        Ok(scene)
    }
}
impl Scene {
    pub fn snapshot(&self, instance: u64) -> Result<SceneSnapshot, String> {
        if self.colliders.len() + self.capsules.len() > SNAPSHOT_COLLIDERS
            || self
                .colliders
                .values()
                .filter(|c| c.mesh.solid_box.is_none())
                .map(|c| c.mesh.triangles.len())
                .sum::<usize>()
                > SNAPSHOT_TRIANGLES
        {
            return Err("Collision snapshot source budget exceeded".into());
        }
        let mut colliders = Vec::with_capacity(self.colliders.len() + self.capsules.len());
        for shape in self.colliders.values() {
            let geometry = if let Some(bounds) = shape.mesh.solid_box {
                GeometrySnapshot::Box {
                    min: bounds.min,
                    max: bounds.max,
                }
            } else {
                GeometrySnapshot::Triangles {
                    triangles: shape.mesh.triangles.to_vec(),
                }
            };
            colliders.push(ShapeSnapshot {
                key: shape.key,
                layers: shape.layers,
                usage: shape.usage,
                pose: self.pose(shape.key).unwrap(),
                geometry,
            });
        }
        for shape in self.capsules.values() {
            colliders.push(ShapeSnapshot {
                key: shape.key,
                layers: shape.layers,
                usage: shape.usage,
                pose: self.pose(shape.key).unwrap(),
                geometry: GeometrySnapshot::Capsule {
                    a: shape.capsule.a,
                    b: shape.capsule.b,
                    radius: shape.capsule.radius,
                },
            });
        }
        colliders.sort_by_key(|shape| shape.key);
        let snapshot = SceneSnapshot {
            instance,
            colliders,
        };
        snapshot.validate(instance)?;
        Ok(snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(entity: u64) -> ColliderKey {
        ColliderKey {
            life: Life {
                instance: 7,
                entity,
                generation: 3,
            },
            shape: 0,
        }
    }
    fn scene() -> Scene {
        let mut scene = Scene::default();
        scene
            .insert(MeshCollider {
                key: key(0),
                layers: 1,
                usage: Usage::Blocking,
                mesh: Mesh::from_box(DVec3::new(-20., -1., -20.), DVec3::new(20., 0., 20.))
                    .unwrap(),
            })
            .unwrap();
        scene
            .insert(MeshCollider {
                key: key(10),
                layers: 1,
                usage: Usage::Blocking,
                mesh: Mesh::compile(vec![Triangle([DVec3::ZERO, DVec3::Y * 3., DVec3::Z * 3.])])
                    .unwrap(),
            })
            .unwrap();
        scene
            .set_pose(
                key(10),
                Pose {
                    position: DVec3::X * 2.,
                    rotation: DQuat::from_rotation_y(0.2),
                },
            )
            .unwrap();
        scene
            .insert_capsule(CapsuleCollider {
                key: key(11),
                layers: 2,
                usage: Usage::Blocking,
                capsule: Capsule {
                    a: DVec3::Y * 0.3,
                    b: DVec3::Y * 1.5,
                    radius: 0.3,
                },
            })
            .unwrap();
        scene
            .set_pose(
                key(11),
                Pose {
                    position: DVec3::X,
                    rotation: DQuat::IDENTITY,
                },
            )
            .unwrap();
        scene
    }
    #[test]
    fn regional_blockers_include_old_new_and_transformed_bounds() {
        for geometry in [
            GeometrySnapshot::Box {
                min: DVec3::ZERO,
                max: DVec3::ONE,
            },
            GeometrySnapshot::Triangles {
                triangles: vec![Triangle([DVec3::ZERO, DVec3::Y, DVec3::Z])],
            },
        ] {
            let mut source = SceneSnapshot {
                instance: 7,
                colliders: vec![ShapeSnapshot {
                    key: key(10),
                    layers: 1,
                    usage: Usage::Blocking,
                    pose: Pose {
                        position: DVec3::X * 100.,
                        rotation: DQuat::from_rotation_y(0.7),
                    },
                    geometry,
                }],
            };
            let mut cache = SceneCache::new(7);
            cache.update(&source).unwrap();
            let min = DVec3::splat(-2.);
            let max = DVec3::splat(2.);
            source.colliders[0].pose.position.x += 1.;
            assert!(cache.blocking_geometry_matches_in(&source, min, max));
            let mut near = source.clone();
            near.colliders[0].pose.position = DVec3::ZERO;
            assert!(!cache.blocking_geometry_matches_in(&near, min, max));
            cache.update(&near).unwrap();
            assert!(!cache.blocking_geometry_matches_in(&source, min, max));
            let empty = SceneSnapshot {
                instance: 7,
                colliders: vec![],
            };
            assert!(!cache.blocking_geometry_matches_in(&empty, min, max));
            cache.update(&empty).unwrap();
            assert!(!cache.blocking_geometry_matches_in(&near, min, max));
            assert!(cache.blocking_geometry_matches_in(&source, min, max));
        }
        let source = SceneSnapshot {
            instance: 7,
            colliders: vec![ShapeSnapshot {
                key: key(10),
                layers: 1,
                usage: Usage::Blocking,
                pose: Pose {
                    position: DVec3::ZERO,
                    rotation: DQuat::from_rotation_z(std::f64::consts::FRAC_PI_4),
                },
                geometry: GeometrySnapshot::Box {
                    min: DVec3::ZERO,
                    max: DVec3::ONE,
                },
            }],
        };
        let cache = SceneCache::new(7);
        assert!(!cache.blocking_geometry_matches_in(
            &source,
            DVec3::new(-0.8, 0., 0.),
            DVec3::new(-0.1, 1., 1.),
        ));
    }

    #[test]
    fn cached_capsule_pose_updates_and_shape_replacement_keep_queries_current() {
        let mut source = scene().snapshot(7).unwrap();
        let mut cache = SceneCache::new(7);
        cache.update(&source).unwrap();
        let key = source
            .colliders
            .iter()
            .find(|s| matches!(s.geometry, GeometrySnapshot::Capsule { .. }))
            .unwrap()
            .key;
        source
            .colliders
            .iter_mut()
            .find(|s| s.key == key)
            .unwrap()
            .pose
            .position = DVec3::X * 10.;
        cache.update(&source).unwrap();
        let query = Capsule {
            a: DVec3::new(10., 0.3, 0.),
            b: DVec3::new(10., 1.5, 0.),
            radius: 0.3,
        };
        assert!(
            cache
                .scene()
                .overlap(query, Filter::blocking(7))
                .unwrap()
                .hits
                .iter()
                .any(|h| h.collider == key)
        );
        let shape = source.colliders.iter_mut().find(|s| s.key == key).unwrap();
        shape.geometry = GeometrySnapshot::Box {
            min: DVec3::ZERO,
            max: DVec3::ONE,
        };
        shape.pose.position = DVec3::X * 30.;
        cache.update(&source).unwrap();
        assert!(
            !cache
                .scene()
                .overlap(query, Filter::blocking(7))
                .unwrap()
                .hits
                .iter()
                .any(|h| h.collider == key)
        );
        assert!(!cache.scene().world_capsules.contains_key(&key));
    }

    #[test]
    fn snapshot_round_trip_preserves_solid_boxes_meshes_capsules_and_poses() {
        let original = scene();
        let snapshot = original.snapshot(7).unwrap();
        let bytes = serde_json::to_vec(&snapshot).unwrap();
        let snapshot: SceneSnapshot = serde_json::from_slice(&bytes).unwrap();
        let restored = snapshot.compile(7).unwrap();
        assert_eq!(
            serde_json::to_vec(&restored.snapshot(7).unwrap()).unwrap(),
            bytes
        );
        let capsule = Capsule {
            a: DVec3::Y * 0.3,
            b: DVec3::Y * 1.5,
            radius: 0.3,
        };
        for delta in [DVec3::X * 4., -DVec3::Y * 2., DVec3::Z * 4.] {
            let a = original.sweep(capsule, delta, Filter::blocking(7)).unwrap();
            let b = restored.sweep(capsule, delta, Filter::blocking(7)).unwrap();
            assert_eq!(format!("{a:?}"), format!("{b:?}"));
        }
        let inside = Capsule {
            a: DVec3::new(0., -0.5, 0.),
            b: DVec3::new(0., -0.5, 0.),
            radius: 0.1,
        };
        assert_eq!(
            format!(
                "{:?}",
                original.overlap(inside, Filter::blocking(7)).unwrap()
            ),
            format!(
                "{:?}",
                restored.overlap(inside, Filter::blocking(7)).unwrap()
            )
        );
        let mut changed = restored;
        changed.remove_capsule(key(11));
        let replaced = changed.snapshot(7).unwrap().compile(7).unwrap();
        assert_eq!(replaced.capsule_keys().count(), 0);
    }
    #[test]
    fn cached_pose_updates_reuse_meshes_and_replacement_is_atomic() {
        let original = scene();
        let mut source = original.snapshot(7).unwrap();
        let mut cache = SceneCache::new(7);
        assert_eq!(cache.update(&source).unwrap(), source.colliders.len());
        let mesh = cache.scene.colliders[&key(0)].mesh.nodes.as_ptr();
        source
            .colliders
            .iter_mut()
            .find(|s| s.key == key(10))
            .unwrap()
            .pose
            .position
            .x += 1.;
        assert_eq!(cache.update(&source).unwrap(), 0);
        assert_eq!(cache.scene.colliders[&key(0)].mesh.nodes.as_ptr(), mesh);
        assert_eq!(cache.scene.snapshot(7).unwrap(), source);
        let mut bad = source.clone();
        bad.colliders.push(source.colliders[0].clone());
        assert!(cache.update(&bad).is_err());
        assert_eq!(cache.scene.snapshot(7).unwrap(), source);
        source.colliders.retain(|s| s.key != key(11));
        assert_eq!(cache.update(&source).unwrap(), 0);
        assert_eq!(cache.scene.capsule_keys().count(), 0);
        source.colliders[0].geometry = GeometrySnapshot::Box {
            min: DVec3::new(-10., -1., -10.),
            max: DVec3::new(10., 0., 10.),
        };
        assert_eq!(cache.update(&source).unwrap(), 1);
        assert_eq!(cache.scene.snapshot(7).unwrap(), source);
    }
    #[test]
    fn rejects_foreign_duplicate_invalid_and_over_budget_geometry() {
        let base = scene().snapshot(7).unwrap();
        assert!(base.compile(8).is_err());
        let mut duplicate = base.clone();
        duplicate.colliders.push(duplicate.colliders[0].clone());
        assert!(duplicate.compile(7).is_err());
        let mut foreign = base.clone();
        foreign.colliders[0].key.life.instance = 8;
        assert!(foreign.compile(7).is_err());
        let mut invalid = base.clone();
        invalid.colliders[0].pose.rotation = DQuat::from_xyzw(0., 0., 0., 0.);
        assert!(invalid.compile(7).is_err());
        let mut invalid = base.clone();
        invalid.colliders[0].geometry = GeometrySnapshot::Box {
            min: DVec3::ONE,
            max: DVec3::ZERO,
        };
        assert!(invalid.compile(7).is_err());
        let mut invalid = base.clone();
        invalid.colliders[0].geometry = GeometrySnapshot::Capsule {
            a: DVec3::ZERO,
            b: DVec3::ONE,
            radius: -1.,
        };
        assert!(invalid.compile(7).is_err());
        let mut invalid = base.clone();
        invalid.colliders[0].geometry = GeometrySnapshot::Triangles {
            triangles: vec![Triangle([DVec3::ZERO; 3])],
        };
        assert!(invalid.compile(7).is_err());
        let mut invalid = base;
        invalid.colliders[0].geometry = GeometrySnapshot::Triangles {
            triangles: vec![Triangle([DVec3::ZERO, DVec3::Y, DVec3::Z]); SNAPSHOT_TRIANGLES + 1],
        };
        assert!(invalid.compile(7).is_err());
    }
}

/// Reuses compiled geometry while admitting complete scoped pose replacements.
pub struct SceneCache {
    instance: u64,
    source: std::collections::BTreeMap<ColliderKey, ShapeSnapshot>,
    scene: Scene,
}
impl SceneCache {
    pub fn new(instance: u64) -> Self {
        Self {
            instance,
            source: Default::default(),
            scene: Default::default(),
        }
    }
    pub fn scene(&self) -> &Scene {
        &self.scene
    }
    /// Classifies an admitted shape without copying its collision geometry.
    pub fn is_capsule(&self, key: ColliderKey) -> bool {
        self.source
            .get(&key)
            .is_some_and(|shape| matches!(shape.geometry, GeometrySnapshot::Capsule { .. }))
    }
    /// Compares fixed geometry, including poses and identities, before an update.
    /// Capsule motion is separate; `update` still validates the complete snapshot.
    pub fn fixed_geometry_matches(&self, snapshot: &SceneSnapshot) -> bool {
        if snapshot.instance != self.instance {
            return false;
        }
        let fixed =
            |shape: &&ShapeSnapshot| !matches!(shape.geometry, GeometrySnapshot::Capsule { .. });
        let next = snapshot.colliders.iter().filter(fixed);
        next.clone().count() == self.source.values().filter(fixed).count()
            && next
                .into_iter()
                .all(|shape| self.source.get(&shape.key) == Some(shape))
    }
    /// Compares fixed character blockers; other query usages do not affect movement.
    /// Capsule poses remain separate, and `update` validates every shape.
    pub fn blocking_geometry_matches(&self, snapshot: &SceneSnapshot) -> bool {
        if snapshot.instance != self.instance {
            return false;
        }
        let blocking = |shape: &&ShapeSnapshot| {
            shape.usage == Usage::Blocking
                && !matches!(shape.geometry, GeometrySnapshot::Capsule { .. })
        };
        let mut next = snapshot.colliders.iter().filter(blocking);
        next.clone().count() == self.source.values().filter(blocking).count()
            && next.all(|shape| self.source.get(&shape.key) == Some(shape))
    }
    /// Compares fixed blockers that can intersect a conservative world-space region.
    /// Both old and new bounds participate; `update` still validates every shape.
    pub fn blocking_geometry_matches_in(
        &self,
        snapshot: &SceneSnapshot,
        min: DVec3,
        max: DVec3,
    ) -> bool {
        if snapshot.instance != self.instance
            || snapshot.colliders.len() > SNAPSHOT_COLLIDERS
            || !min.is_finite()
            || !max.is_finite()
            || !min.cmple(max).all()
        {
            return false;
        }
        let triangles = snapshot.colliders.iter().try_fold(0usize, |count, shape| {
            count.checked_add(match &shape.geometry {
                GeometrySnapshot::Triangles { triangles } => triangles.len(),
                _ => 0,
            })
        });
        if triangles.is_none_or(|count| count > SNAPSHOT_TRIANGLES) {
            return false;
        }
        let intersects = |shape: &ShapeSnapshot| {
            if shape.usage != Usage::Blocking {
                return false;
            }
            let mut low = DVec3::splat(f64::INFINITY);
            let mut high = DVec3::splat(f64::NEG_INFINITY);
            let mut include = |point: DVec3| {
                let point = shape.pose.point(point);
                low = low.min(point);
                high = high.max(point);
            };
            match &shape.geometry {
                GeometrySnapshot::Capsule { .. } => return false,
                GeometrySnapshot::Box { min, max } => {
                    for x in [min.x, max.x] {
                        for y in [min.y, max.y] {
                            for z in [min.z, max.z] {
                                include(DVec3::new(x, y, z));
                            }
                        }
                    }
                }
                GeometrySnapshot::Triangles { triangles } => {
                    for triangle in triangles {
                        for point in triangle.0 {
                            include(point);
                        }
                    }
                }
            }
            low.cmple(max).all() && high.cmpge(min).all()
        };
        let next: std::collections::BTreeMap<_, _> = snapshot
            .colliders
            .iter()
            .map(|shape| (shape.key, shape))
            .collect();
        if next.len() != snapshot.colliders.len() {
            return false;
        }
        self.source.values().all(|old| {
            let new = next.get(&old.key).copied();
            new == Some(old) || (!intersects(old) && new.is_none_or(|new| !intersects(new)))
        }) && next
            .values()
            .all(|new| self.source.contains_key(&new.key) || !intersects(new))
    }
    /// Moves admitted fixed shapes to newer poses without recompiling them.
    /// Unknown keys and capsules are skipped; returns how many moved.
    ///
    /// # Errors
    ///
    /// Returns a message for an invalid pose, before any shape moves.
    pub fn set_poses(&mut self, poses: &[(ColliderKey, Pose)]) -> Result<usize, String> {
        for (_, pose) in poses {
            valid_point(pose.position)?;
            if !pose.rotation.is_finite() || (pose.rotation.length_squared() - 1.).abs() > 1e-8 {
                return Err("Invalid collision pose".into());
            }
        }
        let mut moved = 0;
        for (key, pose) in poses {
            let Some(shape) = self.source.get_mut(key) else {
                continue;
            };
            if matches!(shape.geometry, GeometrySnapshot::Capsule { .. }) || shape.pose == *pose {
                continue;
            }
            shape.pose = *pose;
            self.scene.set_pose(*key, *pose)?;
            moved += 1;
        }
        Ok(moved)
    }
    /// Returns the number of recompiled shapes. Validation and compilation precede mutation.
    pub fn update(&mut self, snapshot: &SceneSnapshot) -> Result<usize, String> {
        snapshot.validate(self.instance)?;
        let mut meshes = std::collections::BTreeMap::new();
        let mut capsules = std::collections::BTreeMap::new();
        let mut changed = std::collections::BTreeMap::new();
        for shape in &snapshot.colliders {
            if self.source.get(&shape.key).is_some_and(|old| {
                old.geometry == shape.geometry
                    && old.layers == shape.layers
                    && old.usage == shape.usage
            }) {
                continue;
            }
            match &shape.geometry {
                GeometrySnapshot::Capsule { a, b, radius } => {
                    capsules.insert(
                        shape.key,
                        CapsuleCollider {
                            key: shape.key,
                            layers: shape.layers,
                            usage: shape.usage,
                            capsule: Capsule {
                                a: *a,
                                b: *b,
                                radius: *radius,
                            },
                        },
                    );
                }
                geometry => {
                    let mesh = match geometry {
                        GeometrySnapshot::Box { min, max } => Mesh::from_box(*min, *max)?,
                        GeometrySnapshot::Triangles { triangles } => {
                            Mesh::compile(triangles.clone())?
                        }
                        GeometrySnapshot::Capsule { .. } => unreachable!(),
                    };
                    meshes.insert(
                        shape.key,
                        MeshCollider {
                            key: shape.key,
                            layers: shape.layers,
                            usage: shape.usage,
                            mesh,
                        },
                    );
                }
            }
            changed.insert(shape.key, shape.clone());
        }
        let count = changed.len();
        let keys: std::collections::BTreeSet<_> =
            snapshot.colliders.iter().map(|s| s.key).collect();
        self.source.retain(|key, _| keys.contains(key));
        self.scene
            .colliders
            .retain(|key, _| keys.contains(key) && !changed.contains_key(key));
        self.scene
            .capsules
            .retain(|key, _| keys.contains(key) && !changed.contains_key(key));
        self.scene.colliders.extend(meshes);
        self.scene.capsules.extend(capsules);
        let capsules = &self.scene.capsules;
        self.scene
            .world_capsules
            .retain(|key, _| capsules.contains_key(key));
        self.source.extend(changed);
        self.scene.poses.retain(|key, _| keys.contains(key));
        for shape in &snapshot.colliders {
            self.scene.set_pose(shape.key, shape.pose)?;
            self.source.get_mut(&shape.key).unwrap().pose = shape.pose;
        }
        Ok(count)
    }
}

#[cfg(test)]
mod blocking_cache_tests {
    use super::*;

    #[test]
    fn nonblocking_changes_preserve_character_geometry_but_blockers_invalidate_it() {
        let shape = |entity, usage| ShapeSnapshot {
            key: ColliderKey {
                life: Life {
                    instance: 7,
                    entity,
                    generation: 0,
                },
                shape: 0,
            },
            layers: 1,
            usage,
            pose: Pose::default(),
            geometry: GeometrySnapshot::Box {
                min: DVec3::ZERO,
                max: DVec3::ONE,
            },
        };
        let source = SceneSnapshot {
            instance: 7,
            colliders: vec![shape(1, Usage::Blocking), shape(2, Usage::Selection)],
        };
        let mut cache = SceneCache::new(7);
        cache.update(&source).unwrap();
        assert!(cache.blocking_geometry_matches(&source));
        for usage in [Usage::Selection, Usage::Trigger, Usage::Damage] {
            let mut changed = source.clone();
            changed.colliders[1].usage = usage;
            changed.colliders[1].pose.position.x = 10.;
            assert!(!cache.fixed_geometry_matches(&changed));
            assert!(cache.blocking_geometry_matches(&changed));
        }
        let mut changed = source.clone();
        changed.colliders.pop();
        assert!(cache.blocking_geometry_matches(&changed));
        changed.colliders.push(shape(3, Usage::Selection));
        assert!(cache.blocking_geometry_matches(&changed));
        changed.colliders[1].usage = Usage::Blocking;
        assert!(!cache.blocking_geometry_matches(&changed));
        let mut changed = source.clone();
        changed.colliders[0].pose.position.x = 1.;
        assert!(!cache.blocking_geometry_matches(&changed));
        changed = source.clone();
        changed.colliders[0].usage = Usage::Selection;
        assert!(!cache.blocking_geometry_matches(&changed));
        changed = source.clone();
        changed.colliders.remove(0);
        assert!(!cache.blocking_geometry_matches(&changed));
        changed = source.clone();
        changed.instance = 8;
        assert!(!cache.blocking_geometry_matches(&changed));
    }
}
