//! What the shell draws from a house: each placed prop as a mesh or a
//! card with its transform and state, the yard's scenery props (the fence
//! along its sides and the trees beyond), the lighting each area is under,
//! and the route a scripted walk takes from the arrival out the back door.

use std::collections::VecDeque;

use glam::{Mat4, Quat, Vec3};
use haunt_assets::{Catalog, Kind, Surface};
use haunt_rules::doc::{Area, AreaKind, House, OUTSIDE, Wall};

/// One thing to draw.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    /// The prop's id, or a made-up one for scenery.
    pub id: String,
    pub area: String,
    pub asset: String,
    pub kind: Kind,
    pub state: String,
    /// Where it stands (x, y, z) and which way it faces, degrees.
    pub at: Vec3,
    pub yaw: f32,
    /// Its size from the library, m.
    pub size: [f32; 3],
}

impl Placed {
    /// Its model matrix: stood at `at`, turned to `yaw`.
    #[must_use]
    pub fn model(&self) -> Mat4 {
        Mat4::from_rotation_translation(Quat::from_rotation_y(self.yaw.to_radians()), self.at)
    }

    /// Which mesh draws it: the asset and, for a state that changes its
    /// shape, the state.
    #[must_use]
    pub fn mesh_key(&self) -> String {
        let shaped = haunt_assets::meshes::shaped_states(&self.asset);
        if shaped.contains(&self.state.as_str()) {
            format!("{}#{}", self.asset, self.state)
        } else {
            format!("{}#", self.asset)
        }
    }
}

/// The props of an area, placed in the world.
#[must_use]
pub fn props(area: &Area, catalog: &Catalog) -> Vec<Placed> {
    let mut out = Vec::new();
    for p in &area.props {
        let Some(asset) = catalog.asset(&p.asset) else {
            continue;
        };
        let [x, z] = area.world(p.at);
        let ceiling = area.ceiling.unwrap_or(0.0);
        let y = match asset.surface {
            // Hanging things are modelled from their top down to y = 0, so
            // the shell stands them at their stated height.
            Surface::Floor | Surface::Wall | Surface::Ceiling => p.y,
            Surface::Sky => 0.0,
        };
        let _ = ceiling;
        out.push(Placed {
            id: p.id.clone(),
            area: area.id.clone(),
            asset: asset.id.clone(),
            kind: asset.kind,
            state: p.state.clone().unwrap_or_else(|| asset.states[0].clone()),
            at: Vec3::new(x, y, z),
            yaw: p.yaw,
            size: asset.footprint,
        });
    }
    out
}

/// Scenery the yard stands in: a fence down its sides and along the
/// front either side of the gate, and gnarled trees outside it, so the
/// road you face on arrival runs between them.
#[must_use]
pub fn yard_scenery(area: &Area) -> Vec<Placed> {
    let mut out = Vec::new();
    if area.kind != AreaKind::Yard {
        return out;
    }
    let fence = |id: String, x: f32, z: f32, yaw: f32| Placed {
        id,
        area: area.id.clone(),
        asset: "iron-fence".into(),
        kind: Kind::Mesh,
        state: "still".into(),
        at: Vec3::new(x, 0.0, z),
        yaw,
        size: [3.0, 0.15, 1.6],
    };
    let mut k = 0;
    let mut z = area.z0() + 1.5;
    while z < area.z1() - 1.0 {
        out.push(fence(format!("fence-{k}"), area.x0() + 0.1, z, 90.0));
        out.push(fence(format!("fence-{k}b"), area.x1() - 0.1, z, 90.0));
        z += 3.0;
        k += 1;
    }
    // Trees outside, thick to the sides and lining the road south.
    let tree = |id: String, x: f32, z: f32, yaw: f32| Placed {
        id,
        area: area.id.clone(),
        asset: "gnarled-tree".into(),
        kind: Kind::Mesh,
        state: "still".into(),
        at: Vec3::new(x, 0.0, z),
        yaw,
        size: [1.0, 1.0, 5.5],
    };
    let mid = area.centre()[0];
    for i in 0..9 {
        let z = area.z0() - 4.0 - i as f32 * 7.0;
        let wobble = ((i * 37) % 5) as f32 * 0.6;
        out.push(tree(
            format!("road-w{i}"),
            mid - 4.5 - wobble,
            z,
            i as f32 * 41.0,
        ));
        out.push(tree(
            format!("road-e{i}"),
            mid + 4.5 + wobble,
            z - 3.0,
            i as f32 * 67.0,
        ));
    }
    for i in 0..6 {
        let z = area.z0() + 2.0 + i as f32 * 3.6;
        out.push(tree(
            format!("side-w{i}"),
            area.x0() - 3.0 - ((i * 3) % 4) as f32,
            z,
            i as f32 * 53.0,
        ));
        out.push(tree(
            format!("side-e{i}"),
            area.x1() + 3.0 + ((i * 5) % 4) as f32,
            z,
            i as f32 * 29.0,
        ));
    }
    out
}

/// The route of a walk through the house: from the arrival, through each
/// doorway of the shortest door chain to the way out, and out.
#[must_use]
pub fn route(house: &House) -> Vec<[f32; 2]> {
    let Some(start) = house.arrival_point() else {
        return Vec::new();
    };
    // Breadth first over areas, remembering the door taken.
    let mut came: Vec<(String, Option<(String, usize)>)> = vec![(house.arrival.area.clone(), None)];
    let mut queue = VecDeque::from([house.arrival.area.clone()]);
    while let Some(id) = queue.pop_front() {
        if id == house.exit.area {
            break;
        }
        let Some(area) = house.area(&id) else {
            continue;
        };
        for (i, d) in area.doors.iter().enumerate() {
            if d.to == OUTSIDE || came.iter().any(|(a, _)| *a == d.to) {
                continue;
            }
            came.push((d.to.clone(), Some((id.clone(), i))));
            queue.push_back(d.to.clone());
        }
    }
    let mut doors = Vec::new();
    let mut at = house.exit.area.clone();
    while let Some((_, Some((from, i)))) = came.iter().find(|(a, _)| *a == at).cloned() {
        doors.push((from.clone(), i));
        at = from;
    }
    doors.reverse();
    let mut out = vec![start];
    for (area_id, i) in doors {
        let area = house.area(&area_id).expect("an area on the route");
        let door = &area.doors[i];
        out.push(area.door_inside(door, 0.9));
        out.push(area.door_inside(door, -0.9));
    }
    if let Some(area) = house.area(&house.exit.area)
        && let Some(door) = area.doors.iter().find(|d| d.id == house.exit.door)
    {
        out.push(area.door_inside(door, 0.9));
        out.push(area.door_inside(door, -1.5));
    }
    out
}

/// Assets shown on one library page.
pub const LIBRARY_PAGE: usize = 6;

/// The library's assets in the order the contact sheet shows them: the
/// cards, then the meshes, then the effects.
#[must_use]
pub fn library_assets(catalog: &Catalog) -> Vec<&haunt_assets::Asset> {
    let mut out = Vec::new();
    for kind in [Kind::Card, Kind::Mesh, Kind::Effect] {
        out.extend(catalog.assets.iter().filter(|a| a.kind == kind));
    }
    out
}

/// The library's pages.
#[must_use]
pub fn library_pages(catalog: &Catalog) -> usize {
    library_assets(catalog).len().div_ceil(LIBRARY_PAGE)
}

/// A room to show the library in, and one page of assets standing in a
/// row across it: the contact sheet.
#[must_use]
pub fn library(catalog: &Catalog, page: usize) -> (House, Vec<Placed>) {
    let mut house = haunt_rules::v1();
    let mut room = house.area("parlor").cloned().expect("the parlor");
    room.id = "library".into();
    room.rect = [0.0, 0.0, 24.0, 30.0];
    room.ceiling = Some(7.0);
    room.light = haunt_rules::doc::Light {
        preset: "candle".into(),
        dim: 0.0,
        ambient: Some(haunt_rules::doc::Ambient {
            color: "paper".into(),
            level: 0.75,
        }),
        lights: Vec::new(),
    };
    room.doors.clear();
    room.props.clear();
    room.triggers.clear();
    room.sound.bed = None;
    house.areas = vec![room];
    house.arrival.area = "library".into();
    house.arrival.at = [12.0, 7.0];
    house.arrival.yaw = 0.0;
    house.exit.area = "library".into();
    let assets = library_assets(catalog);
    let shown: Vec<&&haunt_assets::Asset> = assets
        .iter()
        .skip(page * LIBRARY_PAGE)
        .take(LIBRARY_PAGE)
        .collect();
    let placed = shown
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let x = 12.0 + (i as f32 - (shown.len() as f32 - 1.0) / 2.0) * 1.9;
            let y = match a.surface {
                Surface::Ceiling => 0.8,
                Surface::Wall => 0.5,
                _ => 0.0,
            };
            Placed {
                id: format!("library-{}", a.id),
                area: "library".into(),
                asset: a.id.clone(),
                kind: a.kind,
                state: a.states[0].clone(),
                at: Vec3::new(x, y, 14.0),
                yaw: 180.0,
                size: a.footprint,
            }
        })
        .collect();
    (house, placed)
}

/// The way a wall faces into its area, as a yaw.
#[must_use]
pub fn facing_in(wall: Wall) -> f32 {
    wall.opposite().outward_yaw()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_route_runs_through_all_six_areas_and_out_the_back() {
        let house = haunt_rules::v1();
        let path = route(&house);
        let areas: Vec<String> = path
            .iter()
            .filter_map(|p| haunt_rules::walk::area_at(&house, *p).map(|a| a.id.clone()))
            .fold(Vec::new(), |mut seen, id| {
                if seen.last() != Some(&id) {
                    seen.push(id);
                }
                seen
            });
        assert_eq!(
            areas,
            [
                "yard",
                "entry-hall",
                "parlor",
                "long-hall",
                "kitchen",
                "back-hall"
            ]
        );
        assert!(haunt_rules::walk::left_by_exit(
            &house,
            *path.last().unwrap()
        ));
    }

    #[test]
    fn a_walker_following_the_route_gets_out() {
        let house = haunt_rules::v1();
        let walls = haunt_rules::walk::Walls::of(&house, haunt_assets::catalog());
        let mut w = crate::walker::Walker::new(house.arrival_point().unwrap(), 180.0);
        let path = route(&house);
        let mut next = 1;
        for _ in 0..60 * 120 {
            if next >= path.len() {
                break;
            }
            let target = path[next];
            w.face(target, 400.0, 1.0 / 60.0);
            w.step(
                &walls,
                crate::walker::Move {
                    forward: 1.0,
                    ..Default::default()
                },
                1.0 / 60.0,
            );
            if (w.pos[0] - target[0]).hypot(w.pos[1] - target[1]) < 0.4 {
                next += 1;
            }
        }
        assert_eq!(next, path.len(), "stuck at {:?}", w.pos);
        assert!(haunt_rules::walk::left_by_exit(&house, w.pos));
    }

    #[test]
    fn the_library_pages_show_every_asset_once() {
        let catalog = haunt_assets::catalog();
        let pages = library_pages(catalog);
        let mut seen = Vec::new();
        for page in 0..pages {
            let (house, placed) = library(catalog, page);
            assert_eq!(house.areas.len(), 1);
            assert!(placed.len() <= LIBRARY_PAGE && !placed.is_empty());
            seen.extend(placed.into_iter().map(|p| p.asset));
        }
        assert_eq!(seen.len(), catalog.assets.len());
    }

    #[test]
    fn props_stand_where_the_document_says() {
        let house = haunt_rules::v1();
        let parlor = house.area("parlor").unwrap();
        let placed = props(parlor, haunt_assets::catalog());
        let chair = placed.iter().find(|p| p.id == "armchair-1").unwrap();
        assert_eq!(chair.at, Vec3::new(9.2, 0.0, 38.3));
        assert_eq!(chair.mesh_key(), "armchair#");
        let yard = house.area("yard").unwrap();
        let pumpkin = props(yard, haunt_assets::catalog())
            .into_iter()
            .find(|p| p.id == "pumpkin-right")
            .unwrap();
        assert_eq!(pumpkin.mesh_key(), "jack-o-lantern#sly");
        assert!(yard_scenery(yard).len() > 20);
    }
}
