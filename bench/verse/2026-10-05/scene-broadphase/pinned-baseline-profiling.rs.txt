//! Optional bounded measurements for one collision scene and its clones.
use super::Stats;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Instant;

#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub struct QueryMetrics {
    pub calls: u64,
    pub errors: u64,
    pub truncated: u64,
    pub nodes: u64,
    pub triangles: u64,
    pub capsule_tests: u64,
    pub wall_seconds: f64,
    pub maximum_wall_seconds: f64,
}
#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub struct QueryProfile {
    pub ray: QueryMetrics,
    pub overlap: QueryMetrics,
    pub sweep: QueryMetrics,
}
#[derive(Clone, Copy)]
pub(super) enum Kind {
    Ray,
    Overlap,
    Sweep,
}
#[derive(Debug, Default)]
pub(super) struct Measurements {
    enabled: AtomicBool,
    data: Mutex<QueryProfile>,
}
impl Measurements {
    pub fn enable(&self) {
        self.enabled.store(true, Ordering::Relaxed);
    }
    pub fn snapshot(&self) -> Option<QueryProfile> {
        if !self.enabled.load(Ordering::Relaxed) {
            return None;
        }
        self.data.lock().ok().map(|data| *data)
    }
    pub fn observe(self: &Arc<Self>, kind: Kind) -> Observation {
        Observation {
            active: self
                .enabled
                .load(Ordering::Relaxed)
                .then(|| (self.clone(), Instant::now())),
            kind,
            result: None,
        }
    }
}
pub(super) struct Observation {
    active: Option<(Arc<Measurements>, Instant)>,
    kind: Kind,
    result: Option<(Stats, bool)>,
}
impl Observation {
    pub fn finish(&mut self, stats: Stats, truncated: bool) {
        self.result = Some((stats, truncated));
    }
}
impl Drop for Observation {
    fn drop(&mut self) {
        let Some((measurements, start)) = &self.active else {
            return;
        };
        let elapsed = start.elapsed().as_secs_f64();
        // Observation never changes query results, including after a poisoned lock.
        let Ok(mut profile) = measurements.data.lock() else {
            return;
        };
        let metric = match self.kind {
            Kind::Ray => &mut profile.ray,
            Kind::Overlap => &mut profile.overlap,
            Kind::Sweep => &mut profile.sweep,
        };
        metric.calls = metric.calls.saturating_add(1);
        metric.wall_seconds += elapsed;
        metric.maximum_wall_seconds = metric.maximum_wall_seconds.max(elapsed);
        if let Some((stats, truncated)) = self.result {
            metric.nodes = metric.nodes.saturating_add(stats.nodes as u64);
            metric.triangles = metric.triangles.saturating_add(stats.triangles as u64);
            metric.capsule_tests = metric
                .capsule_tests
                .saturating_add(stats.capsule_tests as u64);
            metric.truncated = metric.truncated.saturating_add(u64::from(truncated));
        } else {
            metric.errors = metric.errors.saturating_add(1);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scene_profiling_preserves_hits_and_observes_real_query_failures() {
        use crate::queries::{
            Capsule, ColliderKey, Filter, Life, Mesh, MeshCollider, Scene, Usage,
        };
        use glam::DVec3;
        let mut scene = Scene::default();
        scene
            .insert(MeshCollider {
                key: ColliderKey {
                    life: Life {
                        instance: 1,
                        entity: 1,
                        generation: 0,
                    },
                    shape: 0,
                },
                layers: 1,
                usage: Usage::Blocking,
                mesh: Mesh::from_box(DVec3::new(-3., -1., -3.), DVec3::new(3., 0., 3.)).unwrap(),
            })
            .unwrap();
        let filter = Filter::blocking(1);
        let before = scene.ray(DVec3::Y * 2., -DVec3::Y, 4., filter).unwrap();
        assert!(scene.query_profile().is_none());
        scene.enable_profiling();
        let after = scene.ray(DVec3::Y * 2., -DVec3::Y, 4., filter).unwrap();
        assert_eq!(before.hits.len(), after.hits.len());
        for (before, after) in before.hits.iter().zip(&after.hits) {
            assert_eq!(before.collider, after.collider);
            assert_eq!(before.distance, after.distance);
            assert_eq!(before.position, after.position);
        }
        let capsule = Capsule {
            a: DVec3::Y * 0.1,
            b: DVec3::Y,
            radius: 0.35,
        };
        let overlap = scene.overlap(capsule, filter).unwrap();
        let sweep = scene.sweep(capsule, -DVec3::Y, filter).unwrap();
        assert!(scene.ray(DVec3::ZERO, DVec3::ZERO, 1., filter).is_err());
        let metrics = scene.query_profile().unwrap();
        assert_eq!(metrics.ray.calls, 2);
        assert_eq!(metrics.ray.errors, 1);
        assert_eq!(metrics.ray.nodes, after.stats.nodes as u64);
        assert_eq!(metrics.overlap.calls, 1);
        assert_eq!(metrics.overlap.triangles, overlap.stats.triangles as u64);
        assert_eq!(metrics.sweep.calls, 1);
        assert_eq!(metrics.sweep.triangles, sweep.stats.triangles as u64);
        // A clone participates in the same observation scope without altering geometry.
        scene.clone().overlap(capsule, filter).unwrap();
        assert_eq!(scene.query_profile().unwrap().overlap.calls, 2);
    }

    #[test]
    fn measurements_are_optional_bounded_and_count_failed_calls() {
        let measurements = Arc::new(Measurements::default());
        measurements.observe(Kind::Ray).finish(
            Stats {
                nodes: 2,
                triangles: 3,
                ..Default::default()
            },
            false,
        );
        assert!(measurements.snapshot().is_none());
        measurements.enable();
        measurements.observe(Kind::Ray).finish(
            Stats {
                nodes: 2,
                triangles: 3,
                ..Default::default()
            },
            true,
        );
        drop(measurements.observe(Kind::Sweep));
        let profile = measurements.snapshot().unwrap();
        assert_eq!(profile.ray.calls, 1);
        assert_eq!(profile.ray.nodes, 2);
        assert_eq!(profile.ray.triangles, 3);
        assert_eq!(profile.ray.truncated, 1);
        assert_eq!(profile.sweep.calls, 1);
        assert_eq!(profile.sweep.errors, 1);
        assert_eq!(profile.overlap.calls, 0);
        assert!(profile.ray.wall_seconds >= profile.ray.maximum_wall_seconds);
    }
}
