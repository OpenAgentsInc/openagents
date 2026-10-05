//! Low-precision ephemeris and the bright-star catalogue for the physical sky.
//!
//! The scene uses the rotating Sun–Earth frame of `verse_lagrange`: rotating
//! x points from the Sun to the Earth, y along the Earth's orbital motion, and
//! z to ecliptic north. Scene axes are `(y, z, x)` of that frame, so −Z faces
//! the Sun and +Y is ecliptic north.
//!
//! Formulas follow Meeus, *Astronomical Algorithms* (2nd ed.), chapters 12
//! (sidereal time), 22 (obliquity), and 25 (solar coordinates, low accuracy).

use glam::{DMat3, DVec3, Mat3, Vec3};

/// Julian date of J2000.0 (2000 January 1, 12:00 TT).
pub const J2000: f64 = 2_451_545.0;
/// Julian date of the mission epoch, 2026 September 27, 12:00 UTC.
pub const EPOCH: f64 = 2_461_311.0;
/// Mean obliquity of the ecliptic at the epoch, radians (23.4367°).
pub const OBLIQUITY: f64 = 0.409_046_3;

/// Julian date for a proleptic Gregorian calendar date and UTC hour.
#[must_use]
pub fn julian_date(year: i32, month: u32, day: u32, hour: f64) -> f64 {
    let (y, m) = if month <= 2 {
        (year - 1, month + 12)
    } else {
        (year, month)
    };
    let a = (f64::from(y) / 100.0).floor();
    let b = 2.0 - a + (a / 4.0).floor();
    (365.25 * (f64::from(y) + 4716.0)).floor()
        + (30.6001 * f64::from(m + 1)).floor()
        + f64::from(day)
        + b
        - 1524.5
        + hour / 24.0
}

/// The Earth's heliocentric ecliptic longitude at `jd`, radians. This is the
/// Sun's geocentric true longitude plus 180°.
#[must_use]
pub fn earth_longitude(jd: f64) -> f64 {
    let t = (jd - J2000) / 36_525.0;
    let l0 = 280.466_46 + 36_000.769_83 * t;
    let m = (357.529_11 + 35_999.050_29 * t).to_radians();
    let c = (1.914_602 - 0.004_817 * t) * m.sin()
        + 0.019_993 * (2.0 * m).sin()
        + 0.000_289 * (3.0 * m).sin();
    (l0 + c + 180.0).rem_euclid(360.0).to_radians()
}

/// Greenwich mean sidereal time at `jd`, radians.
#[must_use]
pub fn sidereal_time(jd: f64) -> f64 {
    let d = jd - J2000;
    (280.460_618_37 + 360.985_647_366_29 * d)
        .rem_euclid(360.0)
        .to_radians()
}

/// Rotating-frame vector to scene axes.
#[must_use]
pub fn to_scene(v: DVec3) -> DVec3 {
    DVec3::new(v.y, v.z, v.x)
}

/// Columns map equatorial J2000 unit vectors into scene axes, when the Earth's
/// heliocentric longitude is `longitude` radians.
#[must_use]
pub fn celestial(longitude: f64) -> DMat3 {
    // Equatorial to ecliptic: rotate by −ε about x.
    let equatorial_to_ecliptic = DMat3::from_rotation_x(-OBLIQUITY);
    // Ecliptic to rotating: rotating x is the Earth's heliocentric direction.
    let ecliptic_to_rotating = DMat3::from_rotation_z(-longitude);
    let rotating_to_scene = DMat3::from_cols(DVec3::Z, DVec3::X, DVec3::Y);
    rotating_to_scene * ecliptic_to_rotating * equatorial_to_ecliptic
}

/// The Earth's body axes in scene coordinates at `jd`: x toward longitude 0
/// on the equator, z toward the north pole. `longitude` is the heliocentric
/// longitude the rotating frame uses at that instant.
#[must_use]
pub fn earth_axes(jd: f64, longitude: f64) -> DMat3 {
    celestial(longitude) * DMat3::from_rotation_z(sidereal_time(jd))
}

/// A tidally locked Moon: x toward the Earth, z toward ecliptic north.
#[must_use]
pub fn moon_axes(moon_to_earth: DVec3) -> DMat3 {
    let x = moon_to_earth.normalize();
    let north = DVec3::Y;
    let y = north.cross(x).normalize();
    let z = x.cross(y);
    DMat3::from_cols(x, y, z)
}

/// Converts a double-precision basis for the GPU.
#[must_use]
pub fn single(m: DMat3) -> Mat3 {
    m.as_mat3()
}

/// One catalogue star.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Star {
    /// Unit direction in equatorial J2000 coordinates.
    pub dir: Vec3,
    /// Illuminance at the observer, lux.
    pub illuminance: f32,
    /// Linear color with unit luminance, relative to 5,800 K white.
    pub color: [f32; 3],
}

/// The bright-star catalogue shipped with the zone (`LGSTARS1` format:
/// magic, little-endian `u32` count, then right ascension and declination in
/// radians, V magnitude, and B−V as four `f32` per star).
///
/// # Errors
///
/// Returns a message when the bytes are not a well-formed catalogue.
pub fn parse_stars(bytes: &[u8]) -> Result<Vec<Star>, String> {
    let body = bytes
        .strip_prefix(b"LGSTARS1")
        .ok_or("star catalogue has no LGSTARS1 header")?;
    let count = body
        .get(..4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
        .ok_or("star catalogue has no count")?;
    let records = &body[4..];
    if count > 20_000 || records.len() != count * 16 {
        return Err("star catalogue length does not match its count".into());
    }
    let f = |chunk: &[u8], i: usize| {
        f32::from_le_bytes([
            chunk[i * 4],
            chunk[i * 4 + 1],
            chunk[i * 4 + 2],
            chunk[i * 4 + 3],
        ])
    };
    records
        .chunks_exact(16)
        .map(|r| {
            let (ra, dec, mag, bv) = (f(r, 0), f(r, 1), f(r, 2), f(r, 3));
            if ![ra, dec, mag, bv].iter().all(|x| x.is_finite()) {
                return Err("star catalogue holds a nonfinite value".into());
            }
            Ok(Star {
                dir: Vec3::new(dec.cos() * ra.cos(), dec.cos() * ra.sin(), dec.sin()),
                illuminance: magnitude_illuminance(mag),
                color: blackbody_color(color_temperature(bv)),
            })
        })
        .collect()
}

/// Illuminance of a star of visual magnitude `mag`, lux. Magnitude zero is
/// about 2.1 × 10⁻⁶ lux outside the atmosphere.
#[must_use]
pub fn magnitude_illuminance(mag: f32) -> f32 {
    10f32.powf(-0.4 * (mag + 14.18))
}

/// Effective temperature from B−V (Ballesteros 2012), kelvin.
#[must_use]
pub fn color_temperature(bv: f32) -> f32 {
    let bv = bv.clamp(-0.4, 2.0);
    4600.0 * (1.0 / (0.92 * bv + 1.7) + 1.0 / (0.92 * bv + 0.62))
}

/// Planck radiance at red, green, and blue sample wavelengths, normalized to
/// unit Rec. 709 luminance and divided by the same for 5,800 K sunlight, so
/// the Sun is white and hotter stars are blue.
#[must_use]
pub fn blackbody_color(kelvin: f32) -> [f32; 3] {
    fn planck(nm: f32, kelvin: f32) -> f32 {
        let l = nm * 1e-9;
        // Constant factors cancel in the ratios below.
        1.0 / (l.powi(5) * ((0.014_388 / (l * kelvin)).exp() - 1.0))
    }
    let rgb = |k: f32| {
        let c = [planck(610.0, k), planck(550.0, k), planck(465.0, k)];
        let y = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
        c.map(|x| x / y)
    };
    let (star, sun) = (rgb(kelvin), rgb(5_800.0));
    let c = [star[0] / sun[0], star[1] / sun[1], star[2] / sun[2]];
    let y = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    c.map(|x| x / y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_and_j2000_are_the_published_julian_dates() {
        assert!((julian_date(2000, 1, 1, 12.0) - J2000).abs() < 1e-9);
        assert!((julian_date(2026, 9, 27, 12.0) - EPOCH).abs() < 1e-9);
    }

    #[test]
    fn the_earth_is_past_the_september_equinox_at_the_epoch() {
        // At the September equinox the Sun is at 180°, the Earth at 0°.
        let longitude = earth_longitude(EPOCH).to_degrees();
        assert!((3.0..6.0).contains(&longitude), "{longitude}");
    }

    #[test]
    fn the_celestial_basis_is_a_rotation_that_keeps_ecliptic_north_up() {
        let m = celestial(earth_longitude(EPOCH));
        assert!((m.determinant() - 1.0).abs() < 1e-12);
        // The ecliptic pole in equatorial coordinates is (0, −sin ε, cos ε).
        let pole = m * DVec3::new(0.0, -OBLIQUITY.sin(), OBLIQUITY.cos());
        assert!(pole.distance(DVec3::Y) < 1e-12, "{pole}");
    }

    #[test]
    fn the_sun_lies_in_the_scene_minus_z_direction_in_celestial_terms() {
        // The Sun's equatorial direction at the epoch, from its ecliptic
        // longitude (Earth longitude − 180°) on the ecliptic.
        let longitude = earth_longitude(EPOCH);
        let sun_ecliptic = DVec3::new(
            (longitude + std::f64::consts::PI).cos(),
            (longitude + std::f64::consts::PI).sin(),
            0.0,
        );
        let sun_equatorial = DMat3::from_rotation_x(OBLIQUITY) * sun_ecliptic;
        let scene = celestial(longitude) * sun_equatorial;
        assert!(scene.distance(-DVec3::Z) < 1e-12, "{scene}");
    }

    #[test]
    fn star_colors_run_from_blue_to_red_and_magnitudes_scale() {
        let hot = blackbody_color(color_temperature(-0.3));
        let cool = blackbody_color(color_temperature(1.6));
        assert!(hot[2] > hot[0] && cool[0] > cool[2]);
        let sun = blackbody_color(5_800.0);
        assert!(sun.iter().all(|c| (c - 1.0).abs() < 1e-5), "{sun:?}");
        let ratio = magnitude_illuminance(0.0) / magnitude_illuminance(5.0);
        assert!((ratio - 100.0).abs() < 0.01);
    }

    #[test]
    fn malformed_catalogues_are_refused() {
        assert!(parse_stars(b"nope").is_err());
        let mut bytes = b"LGSTARS1".to_vec();
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(&[0; 16]);
        assert!(parse_stars(&bytes).is_err());
    }
}
