//! A meteor's blast wave: the free-air scaled-distance curves of Kinney and
//! Graham (*Explosive Shocks in Air*, 2nd ed., 1985) for a charge of TNT,
//! the Friedlander pulse through the positive phase, and the Rankine–
//! Hugoniot reflected and dynamic pressures. Distances in m, charges in kg
//! of TNT, pressures in Pa, times in s.

/// Sea-level air: pressure, Pa, sound speed, m/s, density, kg/m³.
pub const AIR_PRESSURE: f64 = 101_325.0;
pub const SOUND: f64 = 340.0;
pub const AIR_DENSITY: f64 = 1.225;
/// TNT's energy, J/kg.
pub const TNT: f64 = 4.184e6;
/// A burst on the ground: the ground reflects the wave, so it is a
/// free-air charge this many times the energy released (UFC 3-340-02's
/// 1.8 for a surface burst).
pub const SURFACE: f64 = 1.8;
/// The side-on overpressure that flattens the scene's brick houses at the
/// edge of a meteor's damage radius, Pa: unreinforced brick fails at
/// 35–70 kPa (Glasstone and Dolan, *The Effects of Nuclear Weapons*,
/// table 5.139); this is the middle.
pub const DAMAGE_OVERPRESSURE: f64 = 50_000.0;
/// The curves are fitted down to this scaled distance, m/kg^⅓; closer
/// than that is inside the fireball, and is held at it.
const NEAREST: f64 = 0.2;

/// Side-on peak overpressure at scaled distance `z`, Pa (Kinney–Graham).
#[must_use]
pub fn side_on(z: f64) -> f64 {
    let z = z.max(NEAREST);
    AIR_PRESSURE * 808.0 * (1.0 + (z / 4.5).powi(2))
        / ((1.0 + (z / 0.048).powi(2)).sqrt()
            * (1.0 + (z / 0.32).powi(2)).sqrt()
            * (1.0 + (z / 1.35).powi(2)).sqrt())
}

/// The positive phase's duration at scaled distance `z` for `charge` kg,
/// s (Kinney–Graham).
#[must_use]
pub fn duration(z: f64, charge: f64) -> f64 {
    let z = z.max(NEAREST);
    1e-3 * charge.cbrt() * 980.0 * (1.0 + (z / 0.54).powi(10))
        / ((1.0 + (z / 0.02).powi(3))
            * (1.0 + (z / 0.74).powi(6))
            * (1.0 + (z / 6.9).powi(2)).sqrt())
}

/// The side-on impulse of the positive phase at scaled distance `z` for
/// `charge` kg, Pa s (Kinney–Graham).
#[must_use]
pub fn impulse(z: f64, charge: f64) -> f64 {
    let z = z.max(NEAREST);
    100.0 * charge.cbrt() * 0.067 * (1.0 + (z / 0.23).powi(4)).sqrt()
        / (z * z * (1.0 + (z / 1.55).powi(3)).cbrt())
}

/// The scaled distance where the side-on overpressure is `pressure`,
/// m/kg^⅓.
#[must_use]
pub fn scaled_distance_for(pressure: f64) -> f64 {
    let (mut lo, mut hi) = (NEAREST, 200.0);
    for _ in 0..80 {
        let mid = (lo + hi) / 2.0;
        if side_on(mid) > pressure {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo + hi) / 2.0
}

/// The free-air TNT charge, kg, whose blast has [`DAMAGE_OVERPRESSURE`]
/// at `radius` m: the meteor that flattens the houses out to `radius`.
#[must_use]
pub fn charge_for(radius: f64) -> f64 {
    (radius.max(0.1) / scaled_distance_for(DAMAGE_OVERPRESSURE)).powi(3)
}

/// Peak reflected overpressure, head on, for side-on `p`, Pa
/// (Rankine–Hugoniot, γ = 1.4).
#[must_use]
pub fn reflected(p: f64) -> f64 {
    2.0 * p * (7.0 * AIR_PRESSURE + 4.0 * p) / (7.0 * AIR_PRESSURE + p)
}

/// Peak dynamic (wind) pressure behind a shock of side-on `p`, Pa.
#[must_use]
pub fn dynamic(p: f64) -> f64 {
    2.5 * p * p / (7.0 * AIR_PRESSURE + p)
}

/// The shock front's speed behind side-on `p`, m/s.
#[must_use]
pub fn shock_speed(p: f64) -> f64 {
    SOUND * (1.0 + 6.0 * p / (7.0 * AIR_PRESSURE)).sqrt()
}

/// The Friedlander pulse p(t) = P (1 − t/T) e^(−b t/T): its decay `b`
/// for a peak `p`, duration `t` and impulse `i` (the impulse is
/// P T (1/b − (1 − e^(−b))/b²)).
fn decay(p: f64, t: f64, i: f64) -> f64 {
    let target = (i / (p * t)).clamp(1e-4, 0.5);
    let ratio = |b: f64| {
        if b < 1e-6 {
            0.5 - b / 6.0
        } else {
            1.0 / b - (1.0 - (-b).exp()) / (b * b)
        }
    };
    let (mut lo, mut hi) = (0.0, 1e4);
    for _ in 0..100 {
        let mid = (lo + hi) / 2.0;
        if ratio(mid) > target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo + hi) / 2.0
}

/// The blast wave of `charge` kg TNT where it reaches `distance` m.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Wave {
    /// When the shock arrives, s after the burst.
    pub arrival: f64,
    /// Peak side-on, reflected and dynamic pressures, Pa.
    pub side_on: f64,
    pub reflected: f64,
    pub dynamic: f64,
    /// The positive phase's duration, s, and side-on impulse, Pa s.
    pub duration: f64,
    pub impulse: f64,
    /// The Friedlander decay.
    pub decay: f64,
    /// The shock's speed, m/s.
    pub speed: f64,
}

impl Wave {
    #[must_use]
    pub fn at(distance: f64, charge: f64) -> Self {
        let scale = charge.max(1e-6).cbrt();
        let z = distance / scale;
        let p = side_on(z);
        let t = duration(z, charge);
        let i = impulse(z, charge);
        // Arrival: the shock's speed integrated out from the burst.
        let n = 48;
        let arrival = (0..n)
            .map(|k| {
                let r = distance * (k as f64 + 0.5) / n as f64;
                distance / n as f64 / shock_speed(side_on(r / scale))
            })
            .sum();
        Wave {
            arrival,
            side_on: p,
            reflected: reflected(p),
            dynamic: dynamic(p),
            duration: t,
            impulse: i,
            decay: decay(p, t, i),
            speed: shock_speed(p),
        }
    }

    /// The pulse's shape at `t` s after arrival: 1 at the front, 0 at the
    /// end of the positive phase.
    #[must_use]
    pub fn shape(&self, t: f64) -> f64 {
        if t < 0.0 || t >= self.duration {
            return 0.0;
        }
        let s = t / self.duration;
        (1.0 - s) * (-self.decay * s).exp()
    }

    /// The net pressure pushing a part of drag coefficient `drag` whose
    /// face clears in `clearing` s, at `t` s after arrival, Pa: the
    /// reflected overpressure on its face over the back's side-on, falling
    /// to nothing as the wave diffracts round it, or the wind's drag,
    /// whichever is greater (UFC 3-340-02's front-face method).
    #[must_use]
    pub fn net(&self, t: f64, clearing: f64, drag: f64) -> f64 {
        let shape = self.shape(t);
        if shape <= 0.0 {
            return 0.0;
        }
        let diffraction =
            (self.reflected - self.side_on) * shape * (1.0 - t / clearing.max(1e-6)).max(0.0);
        let wind = drag * self.dynamic * shape * shape;
        diffraction.max(wind)
    }

    /// The net pressure's impulse between `t0` and `t1` s after arrival,
    /// Pa s: Simpson's rule on the positive phase in that window, split
    /// where the face clears (the near-field pulse is a sharp spike).
    #[must_use]
    pub fn net_impulse(&self, t0: f64, t1: f64, clearing: f64, drag: f64) -> f64 {
        let (a, b) = (t0.max(0.0), t1.min(self.duration));
        if b <= a {
            return 0.0;
        }
        let simpson = |a: f64, b: f64| {
            if b <= a {
                return 0.0;
            }
            let n = 256;
            let h = (b - a) / n as f64;
            let f = |t: f64| self.net(t, clearing, drag);
            let mut sum = f(a) + f(b);
            for k in 1..n {
                sum += f(a + h * k as f64) * if k % 2 == 1 { 4.0 } else { 2.0 };
            }
            sum * h / 3.0
        };
        let split = clearing.clamp(a, b);
        simpson(a, split) + simpson(split, b)
    }

    /// The face's clearing time for a part whose face's smaller half
    /// side is `half` m: three times it over the shock's speed.
    #[must_use]
    pub fn clearing(&self, half: f64) -> f64 {
        3.0 * half / self.speed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_curves_match_kinney_and_grahams_tables() {
        // The fit's own values (Kinney–Graham eq. 6-2): about 10 atm at
        // 1 m/kg^⅓, 2 atm at 2, 0.29 atm at 5 and 0.1 atm at 10.
        for (z, atm) in [(1.0, 9.96), (2.0, 2.05), (5.0, 0.289), (10.0, 0.0985)] {
            assert!((side_on(z) / AIR_PRESSURE / atm - 1.0).abs() < 0.01, "{z}");
        }
        // The Friedlander decay reproduces the impulse.
        let w = Wave::at(5.2, charge_for(5.2));
        let n = 4000;
        let h = w.duration / n as f64;
        let i: f64 = (0..n)
            .map(|k| w.side_on * w.shape((k as f64 + 0.5) * h) * h)
            .sum();
        assert!((i / w.impulse - 1.0).abs() < 0.02, "{i} vs {}", w.impulse);
        assert!((w.side_on - DAMAGE_OVERPRESSURE).abs() < 100.0);
    }
}
