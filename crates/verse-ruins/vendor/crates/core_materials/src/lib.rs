//! core_materials: static material palette for physically-plausible parameters.
//!
//! Scope
//! - Simple registry of materials with density (kg/m^3) and display albedo.
//! - Lightweight ID lookup by name for CLI/config friendliness.
//! - Helper to compute debris mass from voxel size and density.
//!
//! Initial palette (P0, uniform per proxy):
//! - stone:    ρ = 2400 kg/m^3
//! - wood:     ρ = 500  kg/m^3
//! - steel:    ρ = 7850 kg/m^3
//! - concrete: ρ = 2400 kg/m^3
//! - glass:    ρ = 2500 kg/m^3
//! - dirt:     ρ = 1600 kg/m^3
//!
//! Notes
//! - P0 uses a uniform material per voxel proxy. A follow-up may add an optional per-voxel `u8`
//!   palette for mixed materials.

use core_units::{Length, Mass, cube_volume_m3};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MaterialId(pub u16);

#[derive(Clone, Copy, Debug)]
pub struct MaterialInfo {
    pub name: &'static str,
    pub density_kg_m3: f64,
    pub albedo: [f32; 3],
}

/// Initial palette approved for P0.
pub static MATERIALS: &[MaterialInfo] = &[
    MaterialInfo {
        name: "stone",
        density_kg_m3: 2400.0,
        albedo: [0.55, 0.55, 0.55],
    },
    MaterialInfo {
        name: "wood",
        density_kg_m3: 500.0,
        albedo: [0.45, 0.30, 0.15],
    },
    MaterialInfo {
        name: "steel",
        density_kg_m3: 7850.0,
        albedo: [0.50, 0.50, 0.55],
    },
    MaterialInfo {
        name: "concrete",
        density_kg_m3: 2400.0,
        albedo: [0.60, 0.60, 0.60],
    },
    MaterialInfo {
        name: "glass",
        density_kg_m3: 2500.0,
        albedo: [0.70, 0.85, 0.90],
    },
    MaterialInfo {
        name: "dirt",
        density_kg_m3: 1600.0,
        albedo: [0.35, 0.25, 0.20],
    },
];

/// Look up a material by case-insensitive name.
#[inline]
pub fn find_material_id(name: &str) -> Option<MaterialId> {
    let n = name.trim().to_ascii_lowercase();
    MATERIALS
        .iter()
        .position(|m| m.name.eq_ignore_ascii_case(&n))
        .map(|idx| MaterialId(idx as u16))
}

/// Fetch material info by id.
#[inline]
pub fn get(mat: MaterialId) -> Option<&'static MaterialInfo> {
    MATERIALS.get(mat.0 as usize)
}

/// Compute mass for a single voxel cube of edge `voxel_m` (meters) for a given material.
#[inline]
pub fn mass_for_voxel(mat: MaterialId, voxel_m: Length) -> Option<Mass> {
    let m = get(mat)?;
    let vol = cube_volume_m3(voxel_m); // m^3
    Some(Mass(m.density_kg_m3 * vol))
}

impl From<u16> for MaterialId {
    #[inline]
    fn from(v: u16) -> Self {
        MaterialId(v)
    }
}

impl From<MaterialId> for usize {
    #[inline]
    fn from(v: MaterialId) -> Self {
        v.0 as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_and_get_material() {
        let id = find_material_id("Stone").expect("find stone");
        let info = get(id).unwrap();
        assert_eq!(info.name, "stone");
        assert!((info.density_kg_m3 - 2400.0).abs() < 1e-9);
    }

    #[test]
    fn mass_scales_with_density_and_voxel_size() {
        let stone = find_material_id("stone").unwrap();
        let wood = find_material_id("wood").unwrap();
        let v = Length(0.25); // 0.25 m → 0.015625 m^3
        let m_stone = mass_for_voxel(stone, v).unwrap();
        let m_wood = mass_for_voxel(wood, v).unwrap();
        let vol = 0.25f64 * 0.25 * 0.25;
        assert!((f64::from(m_stone) - 2400.0 * vol).abs() < 1e-9);
        assert!((f64::from(m_wood) - 500.0 * vol).abs() < 1e-9);
        assert!(f64::from(m_stone) > f64::from(m_wood));
    }
}
