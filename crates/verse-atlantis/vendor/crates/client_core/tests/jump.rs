use client_core::controller::PlayerController;
use client_core::input::InputState;

#[test]
fn jump_rises_and_lands() {
    let mut pc = PlayerController::new(glam::vec3(0.0, 0.0, 0.0));
    let mut input = InputState::default();
    // Press jump once
    input.jump_pressed = true;
    let dt = 0.016;
    pc.update(&input, dt, glam::Vec3::Z);
    // Height should be positive immediately after a jump tick
    assert!(pc.pos.y > 0.0, "expected positive height after jump start");
    // Release jump (one-shot)
    input.jump_pressed = false;
    // Simulate up to 2 seconds; should land back on ground (y ~= 0)
    let mut t = 0.0f32;
    while t < 2.0 {
        pc.update(&input, dt, glam::Vec3::Z);
        t += dt;
    }
    assert!(
        pc.pos.y.abs() < 1e-3,
        "expected to land on ground, y={}",
        pc.pos.y
    );
}
