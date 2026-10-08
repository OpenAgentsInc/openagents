//! Builds the textured scene and its navigation blockers from the loaded
//! Everglade pack and the layout's placements.
//!
//! Only the models the layout places, and the materials and images they
//! use, are copied into the scene. The pack carries base color only, so
//! every material is a dielectric: rough for wood, plaster, stone, and
//! leaves, and smooth for the blended window glass.
//!
//! A placement may be painted ([`Paint`]): its kit plaster and roof tiles
//! then sample the pack's neutral images (`village/T_Plaster_Luma` and
//! `village/T_RoundTiles_Luma`, admitted for the generated buildings)
//! tinted by the paint's colors, so kit-built houses vary in color without
//! another model or image.
//!
//! A carved placement ([`super::demolition::carve`]) draws its model with
//! every triangle split on the model's block lattice, so the town can take
//! one block's triangles out of the merged cells when it breaks.

use super::demolition::carve::{Lattice, split};
use super::{detail, layout::Placement};
use crate::controller::Footprint;
use crate::pbr::textured::{
    AlphaMode, BaseColorImage, Detail, Primitive, TexturedMaterial, TexturedMesh, TexturedScene,
    TexturedVertex, UNBAKED,
};
use crate::zones::everglade_pack::{self, ZonePack};
use std::collections::BTreeMap;

/// Roughness of every opaque and masked material.
const ROUGH: f32 = 0.85;
/// Roughness of blended glass.
const GLASS: f32 = 0.1;

/// Linear colors that replace a kit model's plaster and roof tiles; `None`
/// keeps the kit's own.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Paint {
    pub plaster: Option<[f32; 3]>,
    pub roof: Option<[f32; 3]>,
}

impl Paint {
    /// A key that orders and compares paints exactly.
    fn key(self) -> [u32; 6] {
        let bits = |c: Option<[f32; 3]>, i: usize| c.map_or(u32::MAX, |c| c[i].to_bits());
        [
            bits(self.plaster, 0),
            bits(self.plaster, 1),
            bits(self.plaster, 2),
            bits(self.roof, 0),
            bits(self.roof, 1),
            bits(self.roof, 2),
        ]
    }
}

/// The kit materials a paint replaces, in the village set or a generated
/// copy of a kit piece, and the neutral image each samples: plaster, then
/// roof tiles. The sixth round's houses (`scripts/blender/town_houses.py`)
/// name theirs `HousePlaster` and `HouseTiles`; the pack names a house's
/// color variant `~2`, `~3`, and so on, which still takes the paint. The
/// market stall's awning takes the plaster color on its cloth and first
/// stripes and the roof color on its second, as plain colors without an
/// image (`layout::paint`).
const PAINTED: [(&str, Coat, Option<&str>); 7] = [
    ("/MI_Plaster", Coat::Plaster, Some("village/T_Plaster_Luma")),
    (
        "/MI_RoundTiles",
        Coat::Roof,
        Some("village/T_RoundTiles_Luma"),
    ),
    (
        "/HousePlaster",
        Coat::Plaster,
        Some("village/T_Plaster_Luma"),
    ),
    ("/HouseTiles", Coat::Roof, Some("village/T_RoundTiles_Luma")),
    ("/Stall_Cloth", Coat::Plaster, None),
    ("/Stall_StripeA", Coat::Plaster, None),
    ("/Stall_StripeB", Coat::Roof, None),
];

/// Which of a paint's two colors a material takes.
#[derive(Clone, Copy)]
enum Coat {
    Plaster,
    Roof,
}

/// Pack indices already copied into the scene.
#[derive(Default)]
pub struct Copied<'a> {
    images: BTreeMap<u16, usize>,
    materials: BTreeMap<(u16, [u32; 6]), usize>,
    /// White materials that a placed model's colors fold into, by image,
    /// alpha mode and cutoff, face culling, and emission.
    folded: BTreeMap<(Option<usize>, u8, u32, bool, u32), usize>,
    #[allow(clippy::type_complexity)]
    meshes: BTreeMap<(&'a str, [u32; 6], Option<[u32; 6]>), (usize, ([f32; 3], [f32; 3]))>,
}

/// A material's look once its paint is applied: the scene image, the
/// linear base color, and the pack's alpha mode and face culling.
#[derive(Clone, Copy)]
struct Look {
    image: Option<usize>,
    base_color: [f32; 4],
    alpha: everglade_pack::AlphaMode,
    double_sided: bool,
    /// Emitted luminance, cd/m², times the base color.
    emissive: f32,
}

/// What a material named `Emit…` emits, cd/m² times its color: the pack
/// carries no emission, so generated flames, embers, runes, and lantern
/// glass mark themselves by name (`scripts/blender/grove_props.py`). At a
/// dusk exposure it reads a few times brighter than a sunlit wall, so the
/// flames bloom.
pub(crate) const EMIT_LUMINANCE: f32 = 6_000.0;

/// Whether pack material `name` (`set/source-name`) glows.
fn emits(name: &str) -> bool {
    name.rsplit('/')
        .next()
        .is_some_and(|n| n.starts_with("Emit"))
}

/// The textured scene of `placements` and the blockers their collision
/// declares.
///
/// # Errors
///
/// Returns a message when a placement names a model the pack lacks, or the
/// scene exceeds the renderer's bounds.
pub fn build(
    pack: &ZonePack,
    placements: &[Placement],
) -> Result<(TexturedScene, Vec<Footprint>), String> {
    build_painted(pack, placements, |_| Paint::default())
}

/// [`build`], with each placement's kit plaster and roof tiles painted by
/// `paint`.
///
/// # Errors
///
/// Returns a message when a placement names a model the pack lacks, or the
/// scene exceeds the renderer's bounds.
pub fn build_painted(
    pack: &ZonePack,
    placements: &[Placement],
    paint: impl Fn(&Placement) -> Paint,
) -> Result<(TexturedScene, Vec<Footprint>), String> {
    let mut scene = TexturedScene {
        switches: detail::SWITCHES.to_vec(),
        ..TexturedScene::default()
    };
    let mut copied = Copied::default();
    let houses=super::house_lod::configure(pack,placements,&mut scene);
    let mut house_details=vec![None;placements.len()];
    for (group,h) in houses.iter().enumerate() {
        for &index in &h.members {
            house_details[index]=Some(Detail::Group {group:group as u16,level:if h.near{3}else{0}});
        }
    }
    let mut blockers = Vec::new();
    let carved = super::demolition::carve::carved(placements);
    let lattice = |placement: &Placement, carved: bool| -> Result<Option<Lattice>, String> {
        if !carved {
            return Ok(None);
        }
        let model = pack
            .model(placement.model)
            .ok_or_else(|| format!("The Everglade pack has no {}", placement.model))?;
        Ok(Some(Lattice::of_model(
            placement.model,
            model.bounds(),
            placement.scale,
        )))
    };
    // The town hides and carves its buildings' pieces by rewriting their
    // merged indices (`demolition::town`), so they merge into their cells;
    // everything else may draw as an instance of a shared mesh.
    let kit = super::demolition::town::kit_members(placements);
    let editable = |index: usize| carved[index] || kit.contains(&index) || house_details[index].is_some();
    let place = |scene: &mut TexturedScene, index: usize, mesh, transform, level| {
        if editable(index) {
            scene.place_detail(mesh, transform, level);
        } else {
            scene.place_instanced(mesh, transform, level);
        }
    };
    for (index, (placement, &carved)) in placements.iter().zip(&carved).enumerate() {
        let colors = paint(placement);
        let cut = lattice(placement, carved)?;
        let (mesh, bounds) = mesh(pack, placement.model, colors, cut, &mut scene, &mut copied)?;
        let (level, _) = detail::plan(pack, placement.model);
        let level=house_details[index].unwrap_or(level);
        place(&mut scene, index, mesh, placement.transform(), level);
        blockers.extend(placement.footprints(bounds));
    }
    // Far levels of detail follow, in layout order (`detail::far_placements`).
    // A far level is not split: its few large triangles hide with the
    // block their middle lies in.
    for (index, (placement, far)) in placements
        .iter()
        .zip(detail::far_placements(pack, placements))
        .enumerate()
    {
        if let Some((far, _)) = far {
            let (mesh, _) = mesh(pack, far, paint(placement), None, &mut scene, &mut copied)?;
            place(
                &mut scene,
                index,
                mesh,
                placement.transform(),
                detail::FAR_DETAIL,
            );
        }
    }
    super::house_lod::append(pack,&houses,&mut scene,&mut copied)?;
    scene.validate()?;
    Ok((scene, blockers))
}

/// The scene mesh of pack model `name` in `colors`, split on `lattice`
/// when it is carved, copied once, and the model's bounds.
pub(crate) fn mesh<'a>(
    pack: &'a ZonePack,
    name: &'a str,
    colors: Paint,
    lattice: Option<Lattice>,
    scene: &mut TexturedScene,
    copied: &mut Copied<'a>,
) -> Result<(usize, ([f32; 3], [f32; 3])), String> {
    let cut = lattice.map(|l| {
        let [a, b, c] = l.size.to_array().map(f32::to_bits);
        let [d, e, f] = l.min.to_array().map(f32::to_bits);
        [a, b, c, d, e, f]
    });
    if let Some(entry) = copied.meshes.get(&(name, colors.key(), cut)) {
        return Ok(*entry);
    }
    let model = pack
        .model(name)
        .ok_or_else(|| format!("The Everglade pack has no {name}"))?;
    let mesh = copy_model(pack, model, scene, copied, colors, lattice.as_ref())?;
    let entry = (mesh, model.bounds());
    copied.meshes.insert((name, colors.key(), cut), entry);
    Ok(entry)
}

/// Copies a placed model. Each primitive's base color, after its paint, is
/// folded into its vertex colors, and its material is the white one for its
/// image, alpha, and face culling, so models that differ only in color, such
/// as the generated buildings' tinted plaster and tiles and the painted
/// houses, share a material and merge into the same batch of each cell.
fn copy_model(
    pack: &ZonePack,
    model: &everglade_pack::Model,
    scene: &mut TexturedScene,
    copied: &mut Copied<'_>,
    paint: Paint,
    lattice: Option<&Lattice>,
) -> Result<usize, String> {
    let mut primitives = Vec::with_capacity(model.primitives.len());
    for primitive in &model.primitives {
        let look = look(pack, primitive.material, scene, copied, paint)?;
        let key = match look.alpha {
            everglade_pack::AlphaMode::Opaque => (0, 0),
            everglade_pack::AlphaMode::Mask { cutoff } => (1, cutoff.to_bits()),
            everglade_pack::AlphaMode::Blend => (2, 0),
        };
        let key = (
            look.image,
            key.0,
            key.1,
            look.double_sided,
            look.emissive.to_bits(),
        );
        let material = match copied.folded.get(&key) {
            Some(&material) => material,
            None => {
                let material = add_material(
                    scene,
                    &Look {
                        base_color: [1.0; 4],
                        ..look
                    },
                );
                copied.folded.insert(key, material);
                material
            }
        };
        let tint = look.base_color;
        let mut vertices: Vec<TexturedVertex> = primitive
            .vertices
            .iter()
            .map(|v| TexturedVertex {
                pos: v.position,
                normal: v.normal,
                uv: v.uv,
                color: std::array::from_fn(|c| {
                    (f32::from(v.color[c]) * tint[c]).round().clamp(0.0, 255.0) as u8
                }),
                light: UNBAKED,
            })
            .collect();
        let indices = match lattice {
            Some(lattice) => split(&mut vertices, &primitive.indices, lattice),
            None => primitive.indices.clone(),
        };
        primitives.push(Primitive {
            vertices,
            indices,
            material,
        });
    }
    Ok(scene.add_mesh(TexturedMesh { primitives }))
}

pub fn copy_material(
    pack: &ZonePack,
    index: u16,
    scene: &mut TexturedScene,
    copied: &mut Copied<'_>,
) -> Result<usize, String> {
    copy_painted(pack, index, scene, copied, Paint::default())
}

/// [`copy_material`], with a kit plaster or roof-tile material replaced by
/// its neutral image in `paint`'s color.
pub fn copy_painted(
    pack: &ZonePack,
    index: u16,
    scene: &mut TexturedScene,
    copied: &mut Copied<'_>,
    paint: Paint,
) -> Result<usize, String> {
    let key = (index, paint_key(pack, index, paint)?);
    if let Some(&material) = copied.materials.get(&key) {
        return Ok(material);
    }
    let look = look(pack, index, scene, copied, paint)?;
    let material = add_material(scene, &look);
    copied.materials.insert(key, material);
    Ok(material)
}

/// The paint color that applies to pack material `index`, if any, and the
/// neutral image it samples, if any.
#[allow(clippy::type_complexity)]
fn painted(
    pack: &ZonePack,
    index: u16,
    paint: Paint,
) -> Result<Option<([f32; 3], Option<&'static str>)>, String> {
    let source = pack
        .materials
        .get(usize::from(index))
        .ok_or("The Everglade pack names a missing material")?;
    let base = source.name.split('~').next().unwrap_or_default();
    Ok(
        match PAINTED.iter().enumerate().find(|(k, (name, ..))| {
            source.name.ends_with(name) || (*k >= 2 && base.ends_with(name))
        }) {
            Some((_, (_, Coat::Plaster, image))) => paint.plaster.map(|c| (c, *image)),
            Some((_, (_, Coat::Roof, image))) => paint.roof.map(|c| (c, *image)),
            None => None,
        },
    )
}

/// The part of a paint that changes pack material `index`.
fn paint_key(pack: &ZonePack, index: u16, paint: Paint) -> Result<[u32; 6], String> {
    Ok(
        painted(pack, index, paint)?.map_or([u32::MAX; 6], |(c, _)| {
            let mut key = [u32::MAX; 6];
            key[..3].copy_from_slice(&c.map(f32::to_bits));
            key
        }),
    )
}

/// Pack material `index` under `paint`, with its image copied into the
/// scene.
fn look(
    pack: &ZonePack,
    index: u16,
    scene: &mut TexturedScene,
    copied: &mut Copied<'_>,
    paint: Paint,
) -> Result<Look, String> {
    let source = pack
        .materials
        .get(usize::from(index))
        .ok_or("The Everglade pack names a missing material")?;
    let (image, base_color) = match painted(pack, index, paint)? {
        Some((c, None)) => (None, [c[0], c[1], c[2], source.base_color[3]]),
        Some((c, Some(luma))) => {
            let texture = pack
                .textures
                .iter()
                .position(|t| t.name == luma)
                .ok_or_else(|| format!("The Everglade pack has no {luma}"))?;
            let texture = u16::try_from(texture).map_err(|_| "Too many textures")?;
            (
                Some(copy_image(pack, texture, scene, copied)?),
                [c[0], c[1], c[2], source.base_color[3]],
            )
        }
        None => (
            match source.texture {
                Some(texture) => Some(copy_image(pack, texture, scene, copied)?),
                None => None,
            },
            source.base_color,
        ),
    };
    Ok(Look {
        image,
        base_color,
        alpha: source.alpha,
        double_sided: source.double_sided,
        emissive: if emits(&source.name) {
            EMIT_LUMINANCE
        } else {
            0.0
        },
    })
}

/// Adds the dielectric material `look` describes.
fn add_material(scene: &mut TexturedScene, look: &Look) -> usize {
    let (alpha, roughness) = match look.alpha {
        everglade_pack::AlphaMode::Opaque => (AlphaMode::Opaque, ROUGH),
        everglade_pack::AlphaMode::Mask { cutoff } => (AlphaMode::Mask { cutoff }, ROUGH),
        everglade_pack::AlphaMode::Blend => (AlphaMode::Blend, GLASS),
    };
    scene.add_material(TexturedMaterial {
        image: look.image,
        base_color: look.base_color,
        metallic: 0.0,
        roughness,
        alpha,
        double_sided: look.double_sided,
        emissive: look.emissive,
    })
}

fn copy_image(
    pack: &ZonePack,
    index: u16,
    scene: &mut TexturedScene,
    copied: &mut Copied<'_>,
) -> Result<usize, String> {
    if let Some(&image) = copied.images.get(&index) {
        return Ok(image);
    }
    let texture = pack
        .textures
        .get(usize::from(index))
        .ok_or("The Everglade pack names a missing texture")?;
    let image = scene.add_image(BaseColorImage {
        name: texture.name.clone(),
        width: texture.width,
        height: texture.height,
        rgba: texture.rgba.clone(),
    });
    copied.images.insert(index, image);
    Ok(image)
}
