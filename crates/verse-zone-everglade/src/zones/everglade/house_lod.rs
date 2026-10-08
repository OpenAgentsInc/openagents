//! Whole-house distance levels over the original collision and destruction pieces.

use super::{layout::{self, Placement, kit_house::KitHouse}, scene};
use crate::pbr::textured::{Detail, DetailGroup, TexturedScene};
use crate::zones::everglade_pack::{ZonePack, kit};
use glam::{Mat4, Quat, Vec3};

pub const SWITCHES: [f32; 2] = [40.0,80.0];
pub const TRIANGLES: [u64; 3] = [10_000,3_000,800];
pub const DRAWS: [usize; 3] = [12,5,1];

#[must_use]
pub fn houses() -> Vec<KitHouse> {
    layout::city::kit_houses().into_iter().map(|(_,h)| h).chain(layout::first_town_houses()).collect()
}

/// The same bays and piece choices, in a stable local frame.
#[must_use]
pub fn normalized(mut house: KitHouse) -> KitHouse {
    let [x,z] = house.front().0;
    let (s,c) = house.facing.sin_cos();
    let u = (x-house.center[0])*c-(z-house.center[1])*s;
    house.center=[0.0;2];
    house.facing=0.0;
    house.door_at=Some([(u*1000.0).round()/1000.0,house.depth/2.0]);
    house
}

/// A recipe digest. Geometry and images stay in the private build.
#[must_use]
pub fn key(house: KitHouse) -> String {
    let mut pieces=Vec::new();
    normalized(house).raise(&mut pieces);
    let mut hash=0xcbf29ce484222325u64;
    for p in pieces {
        let geometry=[p.at[0],p.at[1],super::height(p.at[0],p.at[1])+p.lift,p.yaw,p.scale];
        for byte in p.model.bytes().chain([0]).chain(geometry.into_iter().flat_map(|v| ((v*10_000.0).round() as i32).to_le_bytes())) {
            hash=(hash^u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    format!("house-{hash:016x}")
}

#[must_use]
pub fn model(house: KitHouse, level: usize) -> String {
    format!("kit/{}-{}",key(house),["near","middle","far"][level])
}

#[must_use]
pub fn transform(house: KitHouse) -> Mat4 {
    Mat4::from_rotation_translation(Quat::from_rotation_y(house.facing),Vec3::new(house.center[0],house.floor()-normalized(house).floor(),house.center[1]))
}

/// Schema bounds, independent of access to the licensed pieces.
#[must_use]
pub fn bounds(house: KitHouse) -> (Vec3,Vec3) {
    let mut pieces=Vec::new();
    normalized(house).raise(&mut pieces);
    let (mut low,mut high)=(Vec3::splat(f32::INFINITY),Vec3::splat(f32::NEG_INFINITY));
    for placement in pieces {
        let p=kit::piece_of(placement.model).expect("house schema uses kit pieces");
        for i in 0..8 {
            let corner=Vec3::from(std::array::from_fn(|a| if i&(1<<a)==0 {p.min[a]}else{p.max[a]}));
            let world=placement.transform().transform_point3(corner);
            low=low.min(world);
            high=high.max(world);
        }
    }
    (low,high)
}

/// Original placement indices that belong to complete canonical recipes.
/// Their whole-house level replaces any legacy per-piece far submission.
pub(crate) fn member_indices(placements: &[Placement]) -> std::collections::BTreeSet<usize> {
    houses().into_iter().flat_map(|house| {
        let mut expected=Vec::new();
        house.raise(&mut expected);
        placements.windows(expected.len()).position(|p| p==expected)
            .into_iter().flat_map(move |start| start..start+expected.len())
    }).collect()
}

pub(crate) struct HouseGroup {
    pub house: KitHouse,
    pub members: Vec<usize>,
    pub near: bool,
    pub reduced: bool,
}

/// Adds groups only for complete canonical recipes, leaving other placements alone.
pub(crate) fn configure(pack: &ZonePack, placements: &[Placement], paints: &[scene::Paint], world: &mut TexturedScene) -> Vec<HouseGroup> {
    let mut groups=Vec::new();
    for house in houses() {
        let mut expected=Vec::new();
        house.raise(&mut expected);
        let Some(start)=placements.windows(expected.len()).position(|p| p==expected) else {continue};
        let members: Vec<_>=(start..start+expected.len()).collect();
        // Atlases include the canonical kit tints. A custom paint stays on
        // the original pieces, so it is neither lost nor applied twice.
        let canonical=members.iter().all(|&i| paints[i]==scene::Paint::default());
        let near=canonical && pack.model(&model(house,0)).is_some();
        let reduced=canonical && pack.model(&model(house,1)).is_some() && pack.model(&model(house,2)).is_some();
        world.detail_groups.push(DetailGroup { anchor:house.center,switches:if reduced{SWITCHES}else{[1e9,2e9]},fallback:if near{3}else{0} });
        groups.push(HouseGroup {house,members,near,reduced});
    }
    groups
}

/// Appends private whole-house levels after all original placement indices.
pub(crate) fn append<'p>(pack: &'p ZonePack, groups: &[HouseGroup], world: &mut TexturedScene, copied: &mut scene::Copied<'p>) -> Result<(),String> {
    for (group,h) in groups.iter().enumerate() {
        for level in 0..3 {
            if (level==0 && !h.near) || (level>0 && !h.reduced) {continue}
            let name=model(h.house,level);
            let own=pack.model(&name).ok_or_else(||format!("The house level is absent: {name}"))?;
            let (mesh,_) = scene::mesh(pack,&own.name,scene::Paint::default(),None,world,copied)?;
            world.place_detail(mesh,transform(h.house),Detail::Group {group:group as u16,level:level as u8});
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_exports_preserve_facing_and_the_terrain_floor_baseline() {
        for house in houses() {
            let mut original=Vec::new();
            let mut local=Vec::new();
            house.raise(&mut original);
            normalized(house).raise(&mut local);
            assert_eq!(original.len(),local.len());
            let to_world=transform(house);
            let to_local=to_world.inverse();
            for (world,local) in original.into_iter().zip(local) {
                assert_eq!(world.model,local.model);
                let expected=world.transform();
                let actual=to_world*local.transform();
                assert!(expected.to_cols_array().into_iter().zip(actual.to_cols_array()).all(|(a,b)| (a-b).abs()<0.002),"{} {}",house.name,world.model);
                let roundtrip=to_world*(to_local*expected);
                assert!(expected.to_cols_array().into_iter().zip(roundtrip.to_cols_array()).all(|(a,b)| (a-b).abs()<0.002));
            }
        }
    }

    #[test]
    fn recipe_keys_ignore_location_but_preserve_the_selected_door_bays() {
        for house in houses() {
            assert_eq!(key(house),key(normalized(house)),"{}",house.name);
            let (low,high)=bounds(house);
            assert!(low.is_finite() && high.is_finite() && low.cmplt(high).all());
        }
    }
}
