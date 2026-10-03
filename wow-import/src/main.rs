//! Convert private 1.12.1 assets into the Verse imported-world schema.
use anyhow::{Context, Result};
use benilla_formats as f;
use glam::{Quat, Vec3};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap},
    io::Cursor,
    path::PathBuf,
};
use verse_wow::assets::*;
const REV: &str = "cf891dc3756a36dc0af4376f861ffb0c847ba5e9";
struct Importer {
    chain: f::Chain,
    dir: PathBuf,
    pack: Pack,
    textures: HashMap<String, usize>,
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
impl Importer {
    fn rgba(&mut self, key: &str, w: u32, h: u32, rgba: &[u8]) -> Result<usize> {
        if let Some(i) = self.textures.get(key) {
            return Ok(*i);
        }
        let i = self.pack.textures.len();
        let file = format!("texture-{i:03}.png");
        let path = self.dir.join(&file);
        let mut png = png::Encoder::new(std::fs::File::create(&path)?, w, h);
        png.set_color(png::ColorType::Rgba);
        png.set_depth(png::BitDepth::Eight);
        png.write_header()?.write_image_data(rgba)?;
        self.pack.textures.push(Texture {
            file,
            sha256: hash(&std::fs::read(path)?),
            width: w,
            height: h,
        });
        self.textures.insert(key.into(), i);
        Ok(i)
    }
    fn texture(&mut self, path: Option<&str>) -> Result<usize> {
        let Some(path) = path else {
            return self.rgba("white", 1, 1, &[255; 4]);
        };
        let key = path.to_ascii_lowercase();
        if let Some(i) = self.textures.get(&key) {
            return Ok(*i);
        }
        let (w, h, rgba) = f::read_texture_rgba(&mut self.chain, path)
            .with_context(|| format!("texture {path}"))?;
        self.rgba(&key, w, h, &rgba)
    }
    fn model(
        &mut self,
        key: &str,
        path: &str,
        skins: &[Option<String>],
        human: bool,
    ) -> Result<()> {
        if self.pack.models.contains_key(key) {
            return Ok(());
        }
        let path = path.to_ascii_lowercase().replace(".mdx", ".m2");
        let bytes = self.chain.read_file(&path)?;
        let mut subs = if path.ends_with(".wmo") {
            f::load_wmo(&mut self.chain, &path)?
        } else {
            f::load_m2_mesh_skinned(&mut self.chain, &path, skins)?
        };
        let mut body = None;
        if human {
            let sections = f::CharSections::load(&mut self.chain)?;
            let atlas = sections
                .composite_body(&mut self.chain, 1, 0, 0, 0, 0, 0, 0, [None; 8], None, false)?
                .context("human body atlas")?
                .into_rgba8();
            body = Some(self.rgba("human-body", atlas.width, atlas.height, &atlas.mips[0])?);
            let sets = f::CharacterGeosets::load(&mut self.chain)?.visible_geosets(
                1,
                0,
                0,
                0,
                &f::EquipGeosets::default(),
            );
            subs.retain(|s| sets.contains(&s.geoset_id));
        }
        let mut surfaces = Vec::new();
        let mut height = 0.0f32;
        for mut sub in subs {
            if sub.texture.is_none() && sub.char_slot==Some(f::CharSkinSlot::Object){
                sub.texture=skins.iter().flatten().next().map(|skin|format!("{}\\{}.blp",path.rsplit_once('\\').map_or("",|p|p.0),skin));
            }
            if matches!(sub.blend, f::ModelBlend::Mod | f::ModelBlend::Mod2x) {
                continue;
            }
            let texture = if human && sub.char_slot == Some(f::CharSkinSlot::Body) {
                body.unwrap()
            } else {
                self.texture(sub.texture.as_deref())?
            };
            let vertices = sub
                .positions
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    height = height.max(p[2]);
                    Vertex {
                        position: *p,
                        normal: sub.normals.get(i).copied().unwrap_or([0.0, 0.0, 1.0]),
                        uv: sub.uvs.get(i).copied().unwrap_or([0.0; 2]),
                        joints: sub.joints.get(i).copied().unwrap_or([0; 4]).map(u32::from),
                        weights: sub.weights.get(i).copied().unwrap_or([1.0, 0.0, 0.0, 0.0]),
                    }
                })
                .collect();
            let blend = if sub.additive {
                3
            } else {
                match sub.blend {
                    f::ModelBlend::Opaque => 0,
                    f::ModelBlend::AlphaTest => 1,
                    _ => 2,
                }
            };
            let tint = sub
                .rgb_anim
                .as_ref()
                .or_else(|| sub.rgb_seq.as_ref().and_then(|seq| seq.seq(None)))
                .map_or([1.0; 3], |rgb| rgb.sample(0.0));
            surfaces.push(Surface {
                vertices,
                indices: sub.indices,
                texture,
                blend,
                emissive: sub.emissive,
                tint,
            });
        }
        let (bones, clips) = if path.ends_with(".m2") {
            let bones = f::parse_m2_skeleton(&bytes)?
                .bones
                .into_iter()
                .map(|b| Bone {
                    parent: b.parent,
                    pivot: b.pivot,
                })
                .collect();
            let clips = f::parse_m2_animations(&bytes)
                .into_iter()
                .map(|a| Clip {
                    id: a.anim_id,
                    duration: a.duration,
                    bones: a
                        .bones
                        .into_iter()
                        .map(|b| BoneKeys {
                            bone: usize::from(b.bone),
                            translation: b.translation,
                            rotation: b.rotation,
                            scale: b.scale,
                        })
                        .collect(),
                })
                .collect();
            (bones, clips)
        } else {
            (vec![], vec![])
        };
        let attachments = if path.ends_with(".m2") {
            f::parse_m2_attachments(&bytes)?
                .into_iter()
                .map(|a| Attachment {
                    id: a.id,
                    bone: usize::from(a.bone),
                    position: a.position,
                })
                .collect()
        } else {
            vec![]
        };
        eprintln!("{key}: {} surfaces", surfaces.len());
        self.pack.models.insert(
            key.into(),
            Model {
                source: path,
                source_sha256: hash(&bytes),
                surfaces,
                bones,
                clips,
                height,
                attachments,
            },
        );
        Ok(())
    }
}
fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let data = PathBuf::from(args.next().context("Expected a client Data directory")?);
    let dir = PathBuf::from(args.next().context("Expected a private output directory")?);
    std::fs::create_dir_all(&dir)?;
    let mut import = Importer {
        chain: f::open_chain(&data)?,
        dir: dir.clone(),
        pack: Pack {
            version: 1,
            source_revision: REV.into(),
            models: BTreeMap::new(),
            textures: vec![],
            placements: vec![],
        },
        textures: HashMap::new(),
    };
    let maps = f::load_map_catalog(&mut import.chain)?;
    let map = maps.directory(289).context("Scholomance map directory")?;
    let wdt = import
        .chain
        .read_file(&format!("World\\Maps\\{map}\\{map}.wdt"))?;
    let wdt = f::WdtReader::new(Cursor::new(wdt), f::WowVersion::Classic).read()?;
    eprintln!("map directory {map}");
    let fallback = if wdt.global_wmo().is_none() {
        let tiles = f::MapTiles::load(&mut import.chain, map)?;
        let mut found = Vec::new();
        for (x, y) in tiles.existing_in_radius(-15.0, 141.0, 1) {
            let tile = f::load_tile_mesh(&mut import.chain, map, x, y)?;
            found.extend(tile.wmos);
        }
        eprintln!("ADT WMOs: {found:?}");
        let w = found.into_iter().next().context("Dungeon ADT WMO")?;
        Some(f::GlobalWmo {
            model: w.model,
            position: w.position,
            rotation: w.rotation,
            doodad_set: w.doodad_set,
            name_set: w.name_set,
        })
    } else {
        None
    };
    let wmo = wdt
        .global_wmo()
        .or(fallback.as_ref())
        .context("Scholomance WMO")?;
    eprintln!(
        "room {} position {:?} rotation {:?}",
        wmo.model, wmo.position, wmo.rotation
    );
    let [rx, ry, rz] = wmo.rotation.map(f32::to_radians);
    let rot = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)
        * Quat::from_rotation_y(ry - std::f32::consts::PI)
        * Quat::from_rotation_z(-rx)
        * Quat::from_rotation_x(rz - std::f32::consts::FRAC_PI_2);
    import.pack.placements.push(Placement {
        model: "room".into(),
        position: wmo.position,
        rotation: rot.to_array(),
        scale: 1.0,
    });
    import.model("room", &wmo.model, &[], false)?;
    let root = f::parse_wmo_root(&import.chain.read_file(&wmo.model)?)?;
    for (i, p) in root.doodads().iter().enumerate() {
        let world = (Vec3::from(wmo.position) + rot * Vec3::from(p.position)).to_array();
        if (world[0] + 15.0).abs() > 35.0
            || (world[1] - 141.0).abs() > 35.0
            || (world[2] - 83.9).abs() > 15.0
        {
            continue;
        }
        let key = format!("prop-{i}");
        import.model(&key, &p.model, &[], false)?;
        import.pack.placements.push(Placement {
            model: key,
            position: world,
            rotation: (rot * Quat::from_array(p.orientation)).to_array(),
            scale: p.scale,
        });
    }
    let creatures = f::load_creature_catalog(&mut import.chain)?;
    for (key, id) in [("claude", 10691), ("cultist", 11157), ("adventurer", 49)] {
        let model = creatures.model(id).context("Creature display")?;
        import.model(key, &model.model_path, &model.textures, key == "adventurer")?;
    }
    let items = f::load_item_display_catalog(&mut import.chain)?;
    let bow = items.get(20723).context("Bow display")?;
    let path = bow.model[0].as_ref().context("Bow model")?;
    let path = if path.contains('\\') {
        path.clone()
    } else {
        format!("Item\\ObjectComponents\\Weapon\\{path}")
    };
    import.model("bow", &path, &[bow.model_texture[0].clone()], false)?;
    let anim = f::load_anim_data_catalog(&mut import.chain)?;
    for id in [40, 64, 65, 68, 85, 87, 105, 109] {
        eprintln!("animation {id}: {:?}", anim.name(id));
    }
    eprintln!("arrow display: {:?}", items.get(5996));
    let arrow=items.get(5996).context("Arrow display")?;
    let arrow_path=format!("Item\\ObjectComponents\\Ammo\\{}",arrow.model[1].as_ref().context("Arrow model")?);
    import.model("arrow",&arrow_path,&[arrow.model_texture[1].clone()],false)?;
    import.pack.validate().map_err(anyhow::Error::msg)?;
    serde_json::to_writer(std::fs::File::create(dir.join("pack.json"))?, &import.pack)?;
    println!(
        "{} models, {} textures, {} placed props",
        import.pack.models.len(),
        import.pack.textures.len(),
        import.pack.placements.len()
    );
    Ok(())
}
