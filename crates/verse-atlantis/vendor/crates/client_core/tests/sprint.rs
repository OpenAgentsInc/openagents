use client_core::controller::PlayerController;
use client_core::input::InputState;

#[test]
fn sprint_increases_forward_speed() {
    let mut pc = PlayerController::new(glam::vec3(0.0, 0.0, 0.0));
    let mut input = InputState::default();
    let dt = 1.0; // seconds
    input.forward = true;
    // Baseline run
    pc.update(&input, dt, glam::Vec3::Z);
    let base_z = pc.pos.z;
    // Reset and try sprint
    pc.pos = glam::vec3(0.0, 0.0, 0.0);
    input.run = true; // Shift
    pc.update(&input, dt, glam::Vec3::Z);
    let sprint_z = pc.pos.z;
    assert!(
        sprint_z > base_z + 0.01,
        "sprint should move farther than base ({} > {})",
        sprint_z,
        base_z
    );
}

#[test]
fn walk_overrides_sprint() {
    let mut pc = PlayerController::new(glam::vec3(0.0, 0.0, 0.0));
    let mut input = InputState::default();
    input.forward = true;
    // Enable walk mode and sprint input; walk should cap speed
    pc.toggle_walk();
    input.run = true;
    pc.update(&input, 1.0, glam::Vec3::Z);
    // WALK_SPEED = 2.2860 m/s
    assert!(
        pc.pos.z <= 2.2861,
        "walk should cap speed even when sprint pressed: z={}",
        pc.pos.z
    );
}
