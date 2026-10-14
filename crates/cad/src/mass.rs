//! Mass properties from the solids' triangles: volume, centre of mass and
//! the inertia tensor, by the divergence theorem over each closed mesh
//! (exact for the triangles; the triangles follow the curved faces to the
//! kernel's display tolerance, so curved parts differ from the exact solid
//! by about that much).

use serde_json::{Value, json};
use solidcraft_engine::geom::Mesh;

/// Volume integrals of a closed mesh, about the origin, in mm.
#[derive(Clone, Copy, Debug, Default)]
pub struct Integrals {
    pub volume: f64,
    /// ∫x, ∫y, ∫z.
    pub first: [f64; 3],
    /// ∫x², ∫y², ∫z², ∫xy, ∫yz, ∫zx.
    pub second: [f64; 6],
}

impl Integrals {
    fn add(&mut self, o: &Integrals, scale: f64) {
        self.volume += o.volume * scale;
        for i in 0..3 {
            self.first[i] += o.first[i] * scale;
        }
        for i in 0..6 {
            self.second[i] += o.second[i] * scale;
        }
    }
}

/// The integrals of one mesh (counter-clockwise seen from outside).
#[must_use]
pub fn integrals(mesh: &Mesh) -> Integrals {
    let mut out = Integrals::default();
    for tri in &mesh.triangles {
        let p = |i: u32| {
            let v = mesh.positions[i as usize];
            [v.x, v.y, v.z]
        };
        let (a, b, c) = (p(tri[0]), p(tri[1]), p(tri[2]));
        // Signed volume of the tetrahedron (origin, a, b, c).
        let v = (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
            + a[2] * (b[0] * c[1] - b[1] * c[0]))
            / 6.0;
        out.volume += v;
        for k in 0..3 {
            out.first[k] += v * (a[k] + b[k] + c[k]) / 4.0;
        }
        let sq = |k: usize| {
            a[k] * a[k] + b[k] * b[k] + c[k] * c[k] + a[k] * b[k] + a[k] * c[k] + b[k] * c[k]
        };
        let cross = |i: usize, j: usize| {
            2.0 * (a[i] * a[j] + b[i] * b[j] + c[i] * c[j])
                + a[i] * b[j]
                + a[j] * b[i]
                + a[i] * c[j]
                + a[j] * c[i]
                + b[i] * c[j]
                + b[j] * c[i]
        };
        out.second[0] += v * sq(0) / 10.0;
        out.second[1] += v * sq(1) / 10.0;
        out.second[2] += v * sq(2) / 10.0;
        out.second[3] += v * cross(0, 1) / 20.0;
        out.second[4] += v * cross(1, 2) / 20.0;
        out.second[5] += v * cross(2, 0) / 20.0;
    }
    out
}

/// Mass properties: grams, millimetres; inertia about the centre of mass
/// in g·mm², world axes.
#[derive(Clone, Copy, Debug, Default)]
pub struct Mass {
    pub volume_mm3: f64,
    pub mass_g: f64,
    pub com: [f64; 3],
    pub inertia: [[f64; 3]; 3],
}

/// Accumulates solids of known density.
#[derive(Default)]
pub struct Sum {
    weighted: Integrals,
    volume: f64,
}

impl Sum {
    /// Adds a mesh of `density` g/cm³.
    pub fn add(&mut self, mesh: &Mesh, density: f64) {
        let i = integrals(mesh);
        // g per mm³.
        self.weighted.add(&i, density / 1000.0);
        self.volume += i.volume;
    }

    pub fn merge(&mut self, o: &Sum) {
        self.weighted.add(&o.weighted, 1.0);
        self.volume += o.volume;
    }

    #[must_use]
    pub fn mass(&self) -> Mass {
        let m = self.weighted.volume;
        if m <= 0.0 {
            return Mass::default();
        }
        let c = [
            self.weighted.first[0] / m,
            self.weighted.first[1] / m,
            self.weighted.first[2] / m,
        ];
        let s = self.weighted.second;
        // About the origin, then moved to the centre of mass.
        let ixx = s[1] + s[2] - m * (c[1] * c[1] + c[2] * c[2]);
        let iyy = s[0] + s[2] - m * (c[0] * c[0] + c[2] * c[2]);
        let izz = s[0] + s[1] - m * (c[0] * c[0] + c[1] * c[1]);
        let ixy = -(s[3] - m * c[0] * c[1]);
        let iyz = -(s[4] - m * c[1] * c[2]);
        let izx = -(s[5] - m * c[2] * c[0]);
        Mass {
            volume_mm3: self.volume,
            mass_g: m,
            com: c,
            inertia: [[ixx, ixy, izx], [ixy, iyy, iyz], [izx, iyz, izz]],
        }
    }
}

fn round(x: f64, places: i32) -> f64 {
    let k = 10f64.powi(places);
    (x * k).round() / k
}

/// `x` to `digits` significant digits, so a 30 g toy and a 90 kg rover
/// both keep their precision (fixed decimals rounded a toy's inertia to 0).
fn significant(x: f64, digits: i32) -> f64 {
    if x == 0.0 || !x.is_finite() {
        return 0.0;
    }
    let places = digits - 1 - x.abs().log10().floor() as i32;
    round(x, places.clamp(0, 15))
}

impl Mass {
    /// As JSON in SI-ish units people read: kg (and g), mm, kg·m², each to
    /// six significant digits.
    #[must_use]
    pub fn json(&self) -> Value {
        let inertia: Vec<Vec<f64>> = self
            .inertia
            .iter()
            .map(|row| row.iter().map(|x| significant(x * 1e-9, 6)).collect())
            .collect();
        json!({
            "volume_cm3": significant(self.volume_mm3 / 1000.0, 6),
            "mass_kg": significant(self.mass_g / 1000.0, 6),
            "mass_g": significant(self.mass_g, 6),
            "center_of_mass_mm": self.com.map(|x| round(x, 1)),
            "inertia_kg_m2": inertia,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solidcraft_engine::geom::Vec3;

    /// A box from `lo` to `hi` as 12 outward triangles.
    fn cuboid(lo: [f64; 3], hi: [f64; 3]) -> Mesh {
        let mut positions = Vec::new();
        for i in 0..8 {
            positions.push(Vec3::new(
                if i & 1 == 0 { lo[0] } else { hi[0] },
                if i & 2 == 0 { lo[1] } else { hi[1] },
                if i & 4 == 0 { lo[2] } else { hi[2] },
            ));
        }
        let triangles = vec![
            [0, 2, 1],
            [1, 2, 3], // bottom (z lo)
            [4, 5, 6],
            [5, 7, 6], // top
            [0, 1, 4],
            [1, 5, 4], // y lo
            [2, 6, 3],
            [3, 6, 7], // y hi
            [0, 4, 2],
            [2, 4, 6], // x lo
            [1, 3, 5],
            [3, 7, 5], // x hi
        ];
        Mesh {
            positions,
            triangles,
            ..Mesh::default()
        }
    }

    #[test]
    fn a_box_has_the_textbook_mass_properties() {
        // 100 × 40 × 20 mm of aluminium, off the origin.
        let mesh = cuboid([10.0, -20.0, 5.0], [110.0, 20.0, 25.0]);
        let mut sum = Sum::default();
        sum.add(&mesh, 2.7);
        let m = sum.mass();
        assert!((m.volume_mm3 - 80_000.0).abs() < 1e-6);
        assert!((m.mass_g - 216.0).abs() < 1e-9);
        assert!(
            (m.com[0] - 60.0).abs() < 1e-9
                && m.com[1].abs() < 1e-9
                && (m.com[2] - 15.0).abs() < 1e-9
        );
        let ixx = 216.0 * (40.0f64.powi(2) + 20.0f64.powi(2)) / 12.0;
        let izz = 216.0 * (100.0f64.powi(2) + 40.0f64.powi(2)) / 12.0;
        assert!(
            (m.inertia[0][0] - ixx).abs() < 1e-6 * ixx,
            "{:?}",
            m.inertia
        );
        assert!((m.inertia[2][2] - izz).abs() < 1e-6 * izz);
        assert!(m.inertia[0][1].abs() < 1e-6);
        let j = m.json();
        assert_eq!(j["mass_kg"], 0.216);
        assert_eq!(j["mass_g"], 216.0);
    }

    #[test]
    fn a_small_part_keeps_its_mass_and_inertia() {
        // A 10 mm ABS cube: 1.04 g, Ixx = m(b² + c²)/12 ≈ 17.33 g·mm²,
        // which fixed decimals of kg·m² rounded to 0.
        let mesh = cuboid([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]);
        let mut sum = Sum::default();
        sum.add(&mesh, 1.04);
        let j = sum.mass().json();
        assert_eq!(j["mass_g"], 1.04);
        assert_eq!(j["mass_kg"], 0.00104);
        let ixx = j["inertia_kg_m2"][0][0].as_f64().unwrap();
        assert!((ixx - 1.733_33e-8).abs() < 1e-12, "{ixx}");
        assert_eq!(significant(87.940_123_4, 6), 87.9401);
        assert_eq!(significant(0.0, 6), 0.0);
    }
}
