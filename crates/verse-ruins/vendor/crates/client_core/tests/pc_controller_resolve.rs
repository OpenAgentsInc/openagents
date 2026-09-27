#![deny(warnings, clippy::all, clippy::pedantic)]

use client_core::systems::pc_controller::{RawButtons, ResolveParams, resolve};

fn params() -> ResolveParams {
    ResolveParams {
        dt: 0.1,
        turn_speed_rad_per_s: 180f32.to_radians(),
    }
}

#[test]
fn ad_swing_camera_not_strafe_or_turn() {
    let out_a = resolve(
        RawButtons {
            a: true,
            ..Default::default()
        },
        params(),
    );
    assert!(out_a.cam_yaw_delta > 0.0);
    assert!(!out_a.intents.strafe_left && !out_a.intents.strafe_right);
    let out_d = resolve(
        RawButtons {
            d: true,
            ..Default::default()
        },
        params(),
    );
    assert!(out_d.cam_yaw_delta < 0.0);
}

#[test]
fn ad_become_strafes_when_rmb_held() {
    // A with RMB → strafe right, no camera swing
    let out = resolve(
        RawButtons {
            rmb: true,
            a: true,
            ..Default::default()
        },
        params(),
    );
    assert!(out.intents.strafe_right && !out.intents.strafe_left);
    assert_eq!(out.cam_yaw_delta, 0.0);
    // D with RMB → strafe left, no camera swing
    let out = resolve(
        RawButtons {
            rmb: true,
            d: true,
            ..Default::default()
        },
        params(),
    );
    assert!(out.intents.strafe_left && !out.intents.strafe_right);
    assert_eq!(out.cam_yaw_delta, 0.0);
}

#[test]
fn sprint_requires_forward_only() {
    let out = resolve(
        RawButtons {
            w: true,
            shift: true,
            ..Default::default()
        },
        params(),
    );
    assert!(out.intents.run);
    let out = resolve(
        RawButtons {
            w: true,
            shift: true,
            q: true,
            ..Default::default()
        },
        params(),
    );
    assert!(!out.intents.run, "strafing cancels sprint");
    let out = resolve(
        RawButtons {
            w: true,
            shift: true,
            s: true,
            ..Default::default()
        },
        params(),
    );
    assert!(!out.intents.run, "backpedal cancels sprint");
}

#[test]
fn click_move_requires_both_buttons() {
    let out = resolve(
        RawButtons {
            lmb: true,
            rmb: true,
            ..Default::default()
        },
        params(),
    );
    assert!(out.intents.click_move_forward);
    let out = resolve(
        RawButtons {
            lmb: true,
            ..Default::default()
        },
        params(),
    );
    assert!(!out.intents.click_move_forward);
}
