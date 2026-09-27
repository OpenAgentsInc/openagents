use glam::vec3;
use server_core::ecs::geom::segment_aabb_enter_t;

#[test]
fn segment_aabb_enter_various_orientations() {
    let min = vec3(-1.0, -1.0, -1.0);
    let max = vec3(1.0, 1.0, 1.0);

    // Axis-aligned pass-through
    assert!(segment_aabb_enter_t(vec3(-2.0, 0.0, 0.0), vec3(2.0, 0.0, 0.0), min, max).is_some());

    // Diagonal pass-through
    assert!(segment_aabb_enter_t(vec3(-2.0, -2.0, -2.0), vec3(2.0, 2.0, 2.0), min, max).is_some());

    // Miss along X
    assert!(segment_aabb_enter_t(vec3(-2.0, 2.0, 0.0), vec3(2.0, 2.0, 0.0), min, max).is_none());
}
