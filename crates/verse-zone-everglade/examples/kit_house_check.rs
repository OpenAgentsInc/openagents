//! Exports every actual kit-house recipe for private coplanar acceptance.
//! Usage: VERSE_KIT_PACK=PACK kit_house_check OUTPUT_DIR
//!
//! Writes licensed geometry and images outside the repository. Each house
//! keeps its piece node names, so `scripts/blender/coplanar.py --names`
//! identifies the recipe placements that need adjustment. The JSON report
//! counts triangles and merged materials at 10, 50, and 120 meters.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use serde_json::{Value, json};
use verse_pbr::pbr::textured::{AlphaMode, TexturedScene};
use verse_zone_everglade::zones::{everglade::{layout, scene}, everglade_pack::{self, kit}};

#[derive(Default)]
struct Buffer {
    bytes: Vec<u8>,
    views: Vec<Value>,
    accessors: Vec<Value>,
}

impl Buffer {
    fn add(&mut self, bytes: &[u8], count: usize, kind: &str, component: u32, target: u32) -> usize {
        self.views.push(json!({"buffer":0,"byteOffset":self.bytes.len(),"byteLength":bytes.len(),"target":target}));
        self.bytes.extend_from_slice(bytes);
        self.accessors.push(json!({"bufferView":self.views.len()-1,"componentType":component,"count":count,"type":kind}));
        self.accessors.len()-1
    }
}

fn floats(values: impl Iterator<Item=f32>) -> Vec<u8> {
    values.flat_map(f32::to_le_bytes).collect()
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn export(dir: &Path, name: &str, scene: &TexturedScene, placements: &[layout::Placement]) -> Result<(), String> {
    let mut buffer = Buffer::default();
    let mut meshes = Vec::new();
    for mesh in &scene.meshes {
        let mut primitives = Vec::new();
        for p in &mesh.primitives {
            let positions = buffer.add(&floats(p.vertices.iter().flat_map(|v| v.pos)), p.vertices.len(), "VEC3", 5126, 34962);
            let low: Vec<f32> = (0..3).map(|a| p.vertices.iter().map(|v| v.pos[a]).fold(f32::INFINITY, f32::min)).collect();
            let high: Vec<f32> = (0..3).map(|a| p.vertices.iter().map(|v| v.pos[a]).fold(f32::NEG_INFINITY, f32::max)).collect();
            buffer.accessors[positions]["min"] = json!(low);
            buffer.accessors[positions]["max"] = json!(high);
            let normals = buffer.add(&floats(p.vertices.iter().flat_map(|v| v.normal)), p.vertices.len(), "VEC3", 5126, 34962);
            let uvs = buffer.add(&floats(p.vertices.iter().flat_map(|v| v.uv)), p.vertices.len(), "VEC2", 5126, 34962);
            let colors = buffer.add(&p.vertices.iter().flat_map(|v| v.color).collect::<Vec<_>>(), p.vertices.len(), "VEC4", 5121, 34962);
            buffer.accessors[colors]["normalized"] = json!(true);
            let indices = buffer.add(&p.indices.iter().flat_map(|i| i.to_le_bytes()).collect::<Vec<_>>(), p.indices.len(), "SCALAR", 5125, 34963);
            primitives.push(json!({"attributes":{"POSITION":positions,"NORMAL":normals,"TEXCOORD_0":uvs,"COLOR_0":colors},"indices":indices,"material":p.material}));
        }
        meshes.push(json!({"primitives":primitives}));
    }
    let mut materials = Vec::new();
    for (i, m) in scene.materials.iter().enumerate() {
        let mut material = json!({"name":format!("material-{i}"),"doubleSided":m.double_sided,"pbrMetallicRoughness":{"baseColorFactor":m.base_color,"metallicFactor":m.metallic,"roughnessFactor":m.roughness}});
        if let Some(image) = m.image { material["pbrMetallicRoughness"]["baseColorTexture"] = json!({"index":image}); }
        match m.alpha {
            AlphaMode::Opaque => material["alphaMode"] = json!("OPAQUE"),
            AlphaMode::Mask { cutoff } => { material["alphaMode"] = json!("MASK"); material["alphaCutoff"] = json!(cutoff); },
            AlphaMode::Blend => material["alphaMode"] = json!("BLEND"),
        }
        materials.push(material);
    }
    let mut images = Vec::new();
    for (i, image) in scene.images.iter().enumerate() {
        let filename = format!("{name}-image-{i}.png");
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, image.width, image.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header().map_err(|e| e.to_string())?.write_image_data(&image.rgba).map_err(|e| e.to_string())?;
        write(&dir.join(&filename), &bytes)?;
        images.push(json!({"uri":filename}));
    }
    let nodes: Vec<_> = scene.placements.iter().take(placements.len()).enumerate().map(|(i,p)|
        json!({"name":format!("{i}:{}", placements[i].model),"mesh":p.mesh,"matrix":p.transform.to_cols_array()})).collect();
    let binary = format!("{name}.bin");
    let doc = json!({"asset":{"version":"2.0","generator":"OpenAgents kit-house acceptance"},"scene":0,
        "scenes":[{"nodes":(0..nodes.len()).collect::<Vec<_>>()}],"nodes":nodes,"meshes":meshes,"materials":materials,
        "textures":(0..images.len()).map(|i| json!({"source":i})).collect::<Vec<_>>(),"images":images,
        "buffers":[{"uri":binary,"byteLength":buffer.bytes.len()}],"bufferViews":buffer.views,"accessors":buffer.accessors});
    write(&dir.join(binary), &buffer.bytes)?;
    write(&dir.join(format!("{name}.gltf")), &serde_json::to_vec_pretty(&doc).map_err(|e| e.to_string())?)
}

fn main() -> Result<(), String> {
    let output = PathBuf::from(std::env::args().nth(1).ok_or("Expected an output directory")?);
    std::fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().map_err(|e| e.to_string())?;
    let dir = output.canonicalize().map_err(|e| e.to_string())?;
    if dir.starts_with(&repo) { return Err("Licensed house exports must stay outside the repository".into()); }
    let path = repo.join(everglade_pack::PACK_DIRECTORY).join(format!("{}.vtp", everglade_pack::PACK_SHA256));
    let pack = everglade_pack::ZonePack::load_local(&path)?;
    if !kit::installed(&pack) { return Err("House acceptance requires the real private kit".into()); }
    let mut houses: Vec<_> = layout::city::kit_houses().into_iter().map(|(_,h)| h).collect();
    houses.extend(layout::first_town_houses());
    for (index, mut house) in houses.into_iter().enumerate() {
        let (s, c) = house.facing.sin_cos();
        let relative_door = house.door_at.map(|[x,z]| {
            let (dx, dz) = (x-house.center[0], z-house.center[1]);
            [dx*c-dz*s, dx*s+dz*c]
        });
        house.center = [0.0; 2];
        house.facing = 0.0;
        house.door_at = relative_door;
        let mut placements = Vec::new();
        house.raise(&mut placements);
        let (scene, _) = scene::build_painted(&pack, &placements, layout::paint)?;
        let merged = scene.merge()?;
        let levels: Vec<_> = [10.0, 50.0, 120.0].map(|distance| {
            let eye = glam::Vec3::new(0.0, house.floor()+2.0, distance);
            let batches: Vec<_> = merged.batches.iter().filter(|b| b.level.drawn_from(eye)).collect();
            json!({"distance":distance,"triangles":batches.iter().map(|b| u64::from(b.count/3)).sum::<u64>(),
                "draws":batches.len(),"materials":batches.iter().map(|b| b.material).collect::<BTreeSet<_>>().len()})
        }).into_iter().collect();
        let name = format!("house-{index:02}");
        export(&dir, &name, &scene, &placements)?;
        println!("{}", json!({"house":house.name,"file":format!("{name}.gltf"),"width":house.width,"depth":house.depth,"stories":house.stories,"levels":levels}));
    }
    Ok(())
}
