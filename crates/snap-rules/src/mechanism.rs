//! Forces for the moving joins: pivots, springs, tethers, and cargo arms.
//! A pivot holds the middle of two sides together while allowing rotation.

use crate::V;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Coupling {
    #[default]
    Magnet,
    Hinge,
    Spring,
    Tether,
    Wheel,
    CargoArm,
}

pub const K_PIVOT: f64 = 3000.;
pub const C_PIVOT: f64 = 6.;
pub const K_SPRING: f64 = 35.;
pub const C_SPRING: f64 = 3.;
pub const TETHER_LENGTH: f64 = 0.25;
pub const K_TETHER: f64 = 80.;
pub const C_TETHER: f64 = 3.;
pub const K_ARM: f64 = 12.;
pub const C_ARM: f64 = 1.;
pub const WHEEL_TORQUE: f64 = 0.15;

pub const PARAMETERS: [f64; 10] = [
    K_PIVOT,
    C_PIVOT,
    K_SPRING,
    C_SPRING,
    TETHER_LENGTH,
    K_TETHER,
    C_TETHER,
    K_ARM,
    C_ARM,
    WHEEL_TORQUE,
];

/// Forces on two corresponding side endpoints of A and B. `wheel_drive`
/// is a signed throttle: equal opposite motor torques stay inside the build.
pub fn forces(
    kind: Coupling,
    a: [V; 2],
    b: [V; 2],
    va: [V; 2],
    vb: [V; 2],
    wheel_drive: f64,
) -> [[V; 2]; 2] {
    let mut out = [[V::ZERO; 2]; 2];
    if kind == Coupling::Magnet {
        for n in 0..2 {
            let f = (b[n] - a[n]) * crate::sim::K_MAGNET + (vb[n] - va[n]) * crate::sim::C_MAGNET;
            out[0][n] = f;
            out[1][n] = f * (-1.);
        }
        return out;
    }
    if matches!(kind, Coupling::Spring | Coupling::Tether) {
        for n in 0..2 {
            let d = b[n] - a[n];
            let len = d.len();
            if len < 1e-12 {
                continue;
            }
            let dir = d * (1. / len);
            let speed = (vb[n] - va[n]).dot(dir);
            let force = if kind == Coupling::Spring {
                K_SPRING * len + C_SPRING * speed
            } else if len > TETHER_LENGTH {
                (K_TETHER * (len - TETHER_LENGTH) + C_TETHER * speed).max(0.)
            } else {
                0.
            };
            out[0][n] = dir * force;
            out[1][n] = dir * (-force);
        }
        return out;
    }
    let middle_a = a[0].mid(a[1]);
    let middle_b = b[0].mid(b[1]);
    let speed = vb[0].mid(vb[1]) - va[0].mid(va[1]);
    let f = (middle_b - middle_a) * K_PIVOT + speed * C_PIVOT;
    for n in 0..2 {
        out[0][n] = f * 0.5;
        out[1][n] = f * (-0.5);
    }
    let ea = a[1] - a[0];
    let eb = b[1] - b[0];
    if ea.len2() < 1e-12 || eb.len2() < 1e-12 {
        return out;
    }
    let ua = ea.unit();
    let ub = eb.unit();
    let relative_speed = eb.cross(vb[1] - vb[0]) / eb.len2() - ea.cross(va[1] - va[0]) / ea.len2();
    let torque = match kind {
        Coupling::CargoArm => K_ARM * ua.cross(ub) + C_ARM * relative_speed,
        Coupling::Wheel => WHEEL_TORQUE * wheel_drive.clamp(-1., 1.),
        Coupling::Hinge if ua.dot(ub) < 0. => {
            let sign = if ua.cross(ub) >= 0. { 1. } else { -1. };
            sign * (-ua.dot(ub)) * K_ARM + C_ARM * relative_speed
        }
        _ => 0.,
    };
    let perpendicular = |v: V| V::new(-v.y, v.x);
    let fa = perpendicular(ea) * (torque / ea.len2());
    let fb = perpendicular(eb) * (-torque / eb.len2());
    out[0][0] -= fa;
    out[0][1] += fa;
    out[1][0] -= fb;
    out[1][1] += fb;
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    const SIDE: [V; 2] = [V::new(0., -0.5), V::new(0., 0.5)];
    #[test]
    fn a_hinge_is_free_inside_its_limit_and_an_arm_holds_its_angle() {
        let turned = [
            V::new(-0.25, -0.4330127018922193),
            V::new(0.25, 0.4330127018922193),
        ];
        let still = [V::ZERO; 2];
        assert_eq!(
            forces(Coupling::Hinge, SIDE, turned, still, still, 0.),
            [[V::ZERO; 2]; 2]
        );
        assert!(
            forces(Coupling::Magnet, SIDE, turned, still, still, 0.)
                .iter()
                .flatten()
                .any(|f| f.len() > 0.)
        );
        let arm = forces(Coupling::CargoArm, SIDE, turned, still, still, 0.);
        assert!(arm[0][0].len() > 0.);
        let total = arm.iter().flatten().copied().fold(V::ZERO, |a, b| a + b);
        assert!(total.len() < 1e-12);
    }
    #[test]
    fn a_spring_pulls_and_damps_but_a_slack_tether_does_not_push() {
        let still = [V::ZERO; 2];
        let move_by = |x| SIDE.map(|p| p + V::new(x, 0.));
        let slack = forces(Coupling::Tether, SIDE, move_by(0.1), still, still, 0.);
        assert_eq!(slack, [[V::ZERO; 2]; 2]);
        let spring = forces(Coupling::Spring, SIDE, move_by(0.5), still, still, 0.);
        assert!((spring[0][0].x - K_SPRING * 0.5).abs() < 1e-12);
        let tether = forces(Coupling::Tether, SIDE, move_by(0.5), still, still, 0.);
        assert!((tether[0][0].x - K_TETHER * 0.25).abs() < 1e-12);
    }
    #[test]
    fn a_reaction_wheel_transfers_equal_opposite_torque() {
        let f = forces(Coupling::Wheel, SIDE, SIDE, [V::ZERO; 2], [V::ZERO; 2], 1.);
        let torque = |n: usize| SIDE[0].cross(f[n][0]) + SIDE[1].cross(f[n][1]);
        assert!((torque(0) - WHEEL_TORQUE).abs() < 1e-12);
        assert!((torque(0) + torque(1)).abs() < 1e-12);
    }
}
