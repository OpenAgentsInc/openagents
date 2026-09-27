#![deny(warnings, clippy::all, clippy::pedantic)]

use client_core::systems::auto_face::{AutoFaceParams, auto_face_step, register_cam_change};

fn rad(deg: f32) -> f32 {
    deg.to_radians()
}

/// After a large swing (>90°), we should:
///  - rotate immediately (no delay) while |diff| > 90°
///  - when |diff| <= 90°, wait ~delay, then continue to finish the turn
#[test]
fn finishes_turn_after_large_swing_release() {
    let delay = 0.25f32;
    let turn_speed = rad(180.0); // 180°/s
    let dt = 0.1f32;

    // Character yaw starts at 0°, camera is swung to +120° by user input.
    let cam_yaw = rad(120.0);
    let mut cur_yaw = 0.0f32;

    // Register the user‑driven change at t=0 (A/D or mouse orbit).
    let mut prev_cam = 0.0f32;
    let mut changed_at = 0.0f32;
    register_cam_change(&mut prev_cam, &mut changed_at, cam_yaw, 0.0);

    // Phase 1: "panic" — |diff| > 90° ⇒ rotate immediately, no delay.
    let mut t = 0.0f32;
    loop {
        t += dt;
        let p = AutoFaceParams {
            last_change_at: changed_at,
            now: t,
            delay_s: delay,
            turning: false,
            turn_speed_rad_per_s: turn_speed,
            dt,
            panic_threshold_rad: std::f32::consts::FRAC_PI_2,
            trail_by_threshold: true,
            hysteresis_rad: 0.15,
        };
        let next = auto_face_step(cur_yaw, cam_yaw, p);
        cur_yaw = next;
        let mut diff = cam_yaw - cur_yaw;
        while diff > std::f32::consts::PI {
            diff -= std::f32::consts::TAU;
        }
        while diff < -std::f32::consts::PI {
            diff += std::f32::consts::TAU;
        }
        if diff.abs() <= std::f32::consts::FRAC_PI_2 {
            break; // left "panic"
        }
        // Keep it bounded for safety.
        assert!(t < 3.0, "did not exit panic fast enough");
    }

    // Phase 2: emulate renderer anchoring the post‑panic delay at the time we exited panic.
    let panic_exit_time = t;
    // During the delay window, no change:
    let hold = auto_face_step(
        cur_yaw,
        cam_yaw,
        AutoFaceParams {
            last_change_at: panic_exit_time,
            now: panic_exit_time + delay * 0.5,
            delay_s: delay,
            turning: false,
            turn_speed_rad_per_s: turn_speed,
            dt,
            panic_threshold_rad: std::f32::consts::FRAC_PI_2,
            trail_by_threshold: true,
            hysteresis_rad: 0.15,
        },
    );
    assert!(
        (hold - cur_yaw).abs() < 1e-6,
        "shouldn’t rotate before delay elapses"
    );

    // After the delay elapses, we should continue rotating and finish alignment.
    let mut now = panic_exit_time + delay + 1e-4;
    let mut yaw = cur_yaw;
    for _ in 0..12 {
        yaw = auto_face_step(
            yaw,
            cam_yaw,
            AutoFaceParams {
                last_change_at: panic_exit_time,
                now,
                delay_s: delay,
                turning: false,
                turn_speed_rad_per_s: turn_speed,
                dt,
                panic_threshold_rad: std::f32::consts::FRAC_PI_2,
                trail_by_threshold: true,
                hysteresis_rad: 0.15,
            },
        );
        now += dt;
    }
    let mut final_diff = cam_yaw - yaw;
    while final_diff > std::f32::consts::PI {
        final_diff -= std::f32::consts::TAU;
    }
    while final_diff < -std::f32::consts::PI {
        final_diff += std::f32::consts::TAU;
    }
    assert!(
        final_diff.abs() < rad(3.0),
        "should finish facing camera (|diff| < 3°)"
    );
}

/// Regression guard: if some caller keeps "touching" the anchor every frame
/// (the bug we saw when smoothing fed anchor updates), the character never
/// reaches the finish phase. This demonstrates why anchors must only be
/// updated on **user input**.
#[test]
fn anchor_resets_every_frame_causes_stall() {
    let delay = 0.25f32;
    let turn_speed = rad(180.0);
    let dt = 0.05f32;
    let cam_yaw = rad(120.0);
    let mut cur_yaw = 0.0f32;
    let mut changed_at = 0.0f32;
    let mut prev_cam = 0.0f32;
    register_cam_change(&mut prev_cam, &mut changed_at, cam_yaw, 0.0);

    // BUGGY caller behavior: updates anchor to "now" each step.
    let mut now = 0.0f32;
    for _ in 0..80 {
        now += dt;
        changed_at = now; // <- this is the anti‑pattern
        cur_yaw = auto_face_step(
            cur_yaw,
            cam_yaw,
            AutoFaceParams {
                last_change_at: changed_at,
                now,
                delay_s: delay,
                turning: false,
                turn_speed_rad_per_s: turn_speed,
                dt,
                panic_threshold_rad: std::f32::consts::FRAC_PI_2,
                trail_by_threshold: true,
                hysteresis_rad: 0.15,
            },
        );
    }
    let mut diff = cam_yaw - cur_yaw;
    while diff > std::f32::consts::PI {
        diff -= std::f32::consts::TAU;
    }
    while diff < -std::f32::consts::PI {
        diff += std::f32::consts::TAU;
    }
    // We prove we are still roughly "stuck" near the threshold instead of finishing.
    assert!(
        diff.abs() > rad(10.0),
        "with constant anchor resets we shouldn't have finished; this guards the fix"
    );
}
