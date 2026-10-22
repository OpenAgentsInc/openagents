//! The shared pylon computer, the Pylon Bay, the build billboard, and the
//! help desk in the Grid's depth-tested world pass.

use glam::Vec3;
use verse::{
    grid_engine::{Content, Kind},
    grid_frame, grid_pack,
    imported::flat,
};
use verse_engine::{
    assets::{Placement, Topology},
    inventory::{Asset, Binding, fingerprint},
    loading::Prepared,
};

const MODEL: &str = "home/pylons-terminal";

pub fn content() -> Result<Content, String> {
    let mut pack = grid_pack::embedded()?;
    let white = pack
        .textures
        .iter()
        .position(|t| t.file == flat::WHITE_TEXTURE)
        .ok_or("The Grid's white texture is missing")?;
    let mut edges = flat::surface(white, [0.8; 3], Topology::Lines);
    let mut faces = flat::surface(white, [0.002; 3], Topology::Triangles);
    for face in pylons_view::terminal::faces() {
        let corners = face.map(|p| Vec3::from_array(p.map(|v| v as f32)));
        flat::quad(&mut faces, corners);
        for i in 0..4 {
            flat::line(&mut edges, corners[i], corners[(i + 1) % 4]);
        }
    }
    let model = flat::model(MODEL, vec![faces, edges], 2.3);
    let (sha256, bytes) = fingerprint(&model)?;
    let inventory = pack
        .inventory
        .as_mut()
        .ok_or("The Grid's inventory is missing")?;
    let texture = inventory
        .assets
        .iter()
        .find(|a| a.binding == Binding::Texture { slot: white })
        .ok_or("The Grid's white texture is not listed")?
        .id
        .clone();
    let white_id = texture.as_str().to_owned();
    inventory.assets.push(Asset {
        id: verse::imported::inventory::id("verse:model:home/pylons-terminal")?,
        binding: Binding::Model { key: MODEL.into() },
        sha256,
        bytes,
        dependencies: vec![texture],
    });
    pack.models.insert(MODEL.into(), model);
    pack.placements.push(Placement {
        model: MODEL.into(),
        position: [0.; 3],
        rotation: [0., 0., 0., 1.],
        scale: 1.,
    });
    crate::pylon_bay::add(&mut pack, white, &white_id)?;
    crate::build_board::add(&mut pack, white, &white_id)?;
    grid_help_desk::add(&mut pack, white, &white_id, &crate::pylon_bay::tint)?;
    grid_suggestion_box::add(&mut pack, white, &white_id, &crate::pylon_bay::tint)?;
    let statics = grid_frame::statics(&pack);
    let mut files: Vec<(&str, &[u8])> = vec![
        (
            flat::WHITE_TEXTURE,
            include_bytes!("../../../assets/verse/grid/verse-flat-white.png"),
        ),
        (
            grid_pack::PORTAL_VIEW,
            include_bytes!("../../../assets/verse/grid/everglade-portal.png"),
        ),
    ];
    files.extend(grid_help_desk::files());
    let prepared = Prepared::from_bytes(pack, &files, Default::default())?;
    Ok(Content {
        prepared,
        statics,
        kind: Kind::Grid,
    })
}
