//! Bounded source geometry for rebuilding an exact scoped query scene.
use super::*;

pub const SNAPSHOT_COLLIDERS: usize = 4096;
pub const SNAPSHOT_TRIANGLES: usize = 16384;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneSnapshot {
    pub instance: u64,
    pub colliders: Vec<ShapeSnapshot>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShapeSnapshot {
    pub key: ColliderKey,
    pub layers: u32,
    pub usage: Usage,
    pub pose: Pose,
    pub geometry: GeometrySnapshot,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
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
                    triangles: shape.mesh.triangles.clone(),
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
