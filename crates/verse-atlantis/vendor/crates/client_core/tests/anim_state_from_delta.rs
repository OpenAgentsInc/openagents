#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnimClip {
    Idle,
    Jog,
    Death,
}

fn anim_state(prev: glam::Vec3, cur: glam::Vec3, alive: bool, dt: f32) -> AnimClip {
    if !alive {
        return AnimClip::Death;
    }
    let v = (cur - prev).length() / dt.max(1e-6);
    if v > 0.2 {
        AnimClip::Jog
    } else {
        AnimClip::Idle
    }
}

#[test]
fn anim_state_from_delta_cases() {
    let dt = 0.1;
    // Small delta → Idle
    assert_eq!(
        anim_state(
            glam::vec3(0.0, 0.0, 0.0),
            glam::vec3(0.01, 0.0, 0.0),
            true,
            dt
        ),
        AnimClip::Idle
    );
    // Larger delta → Jog
    assert_eq!(
        anim_state(
            glam::vec3(0.0, 0.0, 0.0),
            glam::vec3(0.2, 0.0, 0.0),
            true,
            dt
        ),
        AnimClip::Jog
    );
    // Death overrides
    assert_eq!(
        anim_state(
            glam::vec3(0.0, 0.0, 0.0),
            glam::vec3(1.0, 0.0, 0.0),
            false,
            dt
        ),
        AnimClip::Death
    );
}
