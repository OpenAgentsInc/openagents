//! Auto-face helpers: align player yaw to camera yaw after a short delay.

/// Register a camera yaw change; updates `prev_cam_yaw` and `changed_at` when yaw delta exceeds a small threshold.
pub fn register_cam_change(prev_cam_yaw: &mut f32, changed_at: &mut f32, cam_yaw: f32, now: f32) {
    let mut d = cam_yaw - *prev_cam_yaw;
    while d > std::f32::consts::PI {
        d -= std::f32::consts::TAU;
    }
    while d < -std::f32::consts::PI {
        d += std::f32::consts::TAU;
    }
    if d.abs() > 0.02 {
        *prev_cam_yaw = cam_yaw;
        *changed_at = now;
    }
}

/// Parameters for a single auto-face integration step.
#[derive(Clone, Copy, Debug)]
pub struct AutoFaceParams {
    pub last_change_at: f32,
    pub now: f32,
    pub delay_s: f32,
    pub turning: bool,
    pub turn_speed_rad_per_s: f32,
    pub dt: f32,
    /// If the camera deviates more than this, ignore delay and begin turning.
    pub panic_threshold_rad: f32,
    /// When panicking, trail the camera by exactly `panic_threshold_rad` while turning.
    pub trail_by_threshold: bool,
    /// How far inside the threshold we should trail to ensure we leave panic.
    /// Example: threshold = 90°, hysteresis = 8.5° → trail to 81.5°.
    pub hysteresis_rad: f32,
}

/// One step of auto-face behavior: after `delay_s` since last camera change and if not `turning`,
/// rotate `cur_yaw` toward `cam_yaw` by at most `turn_speed_rad_per_s * dt`.
#[must_use]
pub fn auto_face_step(cur_yaw: f32, cam_yaw: f32, p: AutoFaceParams) -> f32 {
    let mut diff = cam_yaw - cur_yaw;
    while diff > std::f32::consts::PI {
        diff -= std::f32::consts::TAU;
    }
    while diff < -std::f32::consts::PI {
        diff += std::f32::consts::TAU;
    }
    let diff_abs = diff.abs();
    let panic = p.panic_threshold_rad > 0.0 && diff_abs > p.panic_threshold_rad;
    if p.turning || ((!panic) && (p.now - p.last_change_at) < p.delay_s) {
        return cur_yaw;
    }
    // Choose target yaw: either camera yaw (normal) or trail inside threshold when panicking
    let target_yaw = if panic && p.trail_by_threshold {
        let trail = (p.panic_threshold_rad - p.hysteresis_rad).max(0.0);
        cam_yaw - trail * diff.signum()
    } else {
        cam_yaw
    };
    let mut diff = target_yaw - cur_yaw;
    while diff > std::f32::consts::PI {
        diff -= std::f32::consts::TAU;
    }
    while diff < -std::f32::consts::PI {
        diff += std::f32::consts::TAU;
    }
    let diff_abs = diff.abs();
    if diff_abs <= 0.10 {
        return cur_yaw;
    }
    let step = (p.turn_speed_rad_per_s * p.dt).min(diff_abs);
    cur_yaw + diff.signum() * step
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_auto_face_before_delay() {
        let cur = 0.0f32; // facing +Z
        let cam = std::f32::consts::FRAC_PI_2; // camera 90° right
        let yaw = auto_face_step(
            cur,
            cam,
            AutoFaceParams {
                last_change_at: 1.0,
                now: 1.1,
                delay_s: 0.5,
                turning: false,
                turn_speed_rad_per_s: 3.1415926,
                dt: 0.016,
                panic_threshold_rad: std::f32::consts::FRAC_PI_2,
                trail_by_threshold: true,
                hysteresis_rad: 0.15,
            },
        );
        // only 0.1s elapsed < 0.5s delay → unchanged
        assert!((yaw - cur).abs() < 1e-6);
    }

    #[test]
    fn rotates_after_delay() {
        let cur = 0.0f32; // facing +Z
        let cam = std::f32::consts::FRAC_PI_2; // camera 90° right
        let mut yaw = cur;
        // register change at t=0
        let mut prev = 0.0;
        let mut changed_at = 0.0;
        register_cam_change(&mut prev, &mut changed_at, cam, 0.0);
        // Advance to after delay and step
        yaw = auto_face_step(
            yaw,
            cam,
            AutoFaceParams {
                last_change_at: changed_at,
                now: 0.6,
                delay_s: 0.5,
                turning: false,
                turn_speed_rad_per_s: 3.1415926,
                dt: 0.1,
                panic_threshold_rad: std::f32::consts::FRAC_PI_2,
                trail_by_threshold: true,
                hysteresis_rad: 0.15,
            },
        );
        assert!(yaw > 0.0);
    }

    #[test]
    fn panic_triggers_rotation_without_delay() {
        let cur = 0.0f32; // facing +Z
        let cam = std::f32::consts::PI; // camera 180° behind
        let yaw = auto_face_step(
            cur,
            cam,
            AutoFaceParams {
                last_change_at: 0.0,
                now: 0.1,
                delay_s: 0.5,
                turning: false,
                turn_speed_rad_per_s: 3.1415926,
                dt: 0.1,
                panic_threshold_rad: std::f32::consts::FRAC_PI_2,
                trail_by_threshold: true,
                hysteresis_rad: 0.15,
            },
        );
        assert!(yaw.abs() > 0.0);
    }
}
