//! Procedural spell geometry compiled without a graphics device.
use glam::Vec3;
use verse_engine::assets::Pack;
fn particle_quad() -> (Vec<verse_engine::assets::Vertex>, Vec<u32>) {
    use verse_engine::assets::Vertex;
    let vertices = [
        ([-1.0, -1.0, 0.0], [0.0, 1.0]),
        ([1.0, -1.0, 0.0], [1.0, 1.0]),
        ([1.0, 1.0, 0.0], [1.0, 0.0]),
        ([-1.0, 1.0, 0.0], [0.0, 0.0]),
    ]
    .into_iter()
    .map(|(position, uv)| Vertex {
        position,
        normal: [0.0, 0.0, 1.0],
        uv,
        joints: [0; 4],
        weights: [1.0, 0.0, 0.0, 0.0],
    })
    .collect();
    (vertices, vec![0, 1, 2, 0, 2, 3])
}
fn particle_texture(pack: &mut Pack, dir: &std::path::Path, name: &str) -> Result<usize, String> {
    use sha2::{Digest, Sha256};
    let file = format!("{name}.png");
    if let Some(i) = pack.textures.iter().position(|t| t.file == file) {
        return Ok(i);
    }
    let bytes = std::fs::read(dir.join(&file))
        .map_err(|e| format!("Missing particle texture {file}: {e}"))?;
    let reader = png::Decoder::new(std::io::Cursor::new(&bytes))
        .read_info()
        .map_err(|e| e.to_string())?;
    let i = pack.textures.len();
    pack.textures.push(verse_engine::assets::Texture {
        file,
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        width: reader.info().width,
        height: reader.info().height,
    });
    Ok(i)
}
pub fn add_effect_models(pack: &mut Pack, dir: &std::path::Path) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    use verse_engine::assets::{Model, Surface, Texture, Vertex};
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .map_err(|e| e.to_string())?
            .write_image_data(&[255; 4])
            .map_err(|e| e.to_string())?;
    }
    std::fs::write(dir.join("verse-effect-white.png"), &bytes).map_err(|e| e.to_string())?;
    let texture = pack.textures.len();
    pack.textures.push(Texture {
        file: "verse-effect-white.png".into(),
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        width: 1,
        height: 1,
    });
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for row in 0..=12 {
        for col in 0..=16 {
            let y = row as f32 / 12.0 * std::f32::consts::PI;
            let a = col as f32 / 16.0 * std::f32::consts::TAU;
            let p = Vec3::new(y.sin() * a.cos(), y.cos(), y.sin() * a.sin());
            vertices.push(Vertex {
                position: p.into(),
                normal: p.into(),
                uv: [0.5, 0.5],
                joints: [0; 4],
                weights: [1.0, 0.0, 0.0, 0.0],
            });
        }
    }
    for row in 0..12 {
        for col in 0..16 {
            let a = row * 17 + col;
            indices.extend_from_slice(&[a, a + 1, a + 17, a + 1, a + 18, a + 17]);
        }
    }
    for (name, color) in [
        ("effect-fire", [1.0, 0.15, 0.015]),
        ("effect-force", [0.18, 0.25, 1.0]),
        ("effect-impact", [1.0, 0.14, 0.015]),
        ("effect-mist", [0.5, 0.8, 1.0]),
        ("effect-web", [0.7, 0.8, 0.9]),
        ("effect-grease", [0.15, 0.12, 0.07]),
        ("effect-shadow", [0.7, 0.05, 1.0]),
        ("effect-light", [1.0, 0.85, 0.45]),
    ] {
        let sprite = match name {
            "effect-fire" | "effect-impact" => Some("particle-fire"),
            "effect-force" | "effect-light" => Some("particle-arcane"),
            "effect-mist" => Some("particle-smoke"),
            "effect-web" => Some("particle-web"),
            "effect-grease" => Some("particle-smoke"),
            "effect-shadow" => Some("particle-shadow"),
            _ => None,
        };
        let texture = if let Some(sprite) = sprite {
            particle_texture(pack, dir, sprite)?
        } else {
            texture
        };
        let (mesh, triangles) = if sprite.is_some() {
            particle_quad()
        } else {
            (vertices.clone(), indices.clone())
        };
        pack.models.insert(
            name.into(),
            Model {
                graph: None,
                markers: Vec::new(),
                states: Default::default(),
                skin: None,
                source: format!(
                    "verse/{}/{name}",
                    if ["effect-web", "effect-grease"].contains(&name) {
                        "ground"
                    } else if sprite.is_some() {
                        "particles"
                    } else {
                        "procedural"
                    }
                ),
                source_sha256: format!("{:x}", Sha256::digest(name.as_bytes())),
                height: 2.0,
                bones: vec![],
                clips: vec![],
                attachments: vec![],
                surfaces: vec![Surface {
                    material: Default::default(),
                    vertices: mesh,
                    indices: triangles,
                    texture,
                    blend: if ["effect-grease", "effect-web", "effect-mist"].contains(&name) {
                        2
                    } else {
                        3
                    },
                    emissive: true,
                    topology: Default::default(),
                    unlit: false,
                    tint: if name == "effect-grease" {
                        [0.1, 0.075, 0.04]
                    } else if sprite.is_some() {
                        [1.0; 3]
                    } else {
                        color
                    },
                }],
            },
        );
    }
    for (name, image, blend, tint) in [
        ("particle-smoke", "particle-smoke", 2, [0.28, 0.22, 0.2]),
        ("particle-spark", "particle-spark", 3, [1.0, 0.65, 0.2]),
    ] {
        let texture = particle_texture(pack, dir, image)?;
        let (vertices, indices) = particle_quad();
        pack.models.insert(
            name.into(),
            Model {
                graph: None,
                markers: Vec::new(),
                states: Default::default(),
                skin: None,
                source: format!("verse/particles/{name}"),
                source_sha256: format!("{:x}", Sha256::digest(name.as_bytes())),
                height: 2.0,
                bones: vec![],
                clips: vec![],
                attachments: vec![],
                surfaces: vec![Surface {
                    material: Default::default(),
                    vertices,
                    indices,
                    texture,
                    blend,
                    emissive: true,
                    topology: Default::default(),
                    unlit: false,
                    tint,
                }],
            },
        );
    }
    let texture = particle_texture(pack, dir, "particle-ribbon")?;
    let (mesh, triangles) = particle_quad();
    pack.models.insert(
        "effect-ribbon".into(),
        Model {
            graph: None,
            markers: Vec::new(),
            states: Default::default(),
            skin: None,
            source: "verse/ribbon/effect-ribbon".into(),
            source_sha256: format!("{:x}", Sha256::digest(b"effect-ribbon")),
            height: 2.0,
            bones: vec![],
            clips: vec![],
            attachments: vec![],
            surfaces: vec![Surface {
                material: Default::default(),
                vertices: mesh,
                indices: triangles,
                texture,
                blend: 3,
                emissive: true,
                topology: Default::default(),
                unlit: false,
                tint: [1.0; 3],
            }],
        },
    );
    // Thin luminous rings keep the shield transparent around the character.
    for (name, planes, color) in [
        ("effect-rune", 1, [0.8, 0.04, 0.65]),
        ("effect-shield", 3, [0.08, 0.6, 1.0]),
    ] {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        for plane in 0..planes {
            let base = vertices.len() as u32;
            for segment in 0..=64 {
                let angle = segment as f32 / 64.0 * std::f32::consts::TAU;
                for radius in [0.98, 1.0] {
                    let (x, z) = (angle.cos() * radius, angle.sin() * radius);
                    let p = match plane {
                        0 => Vec3::new(x, 0.0, z),
                        1 => Vec3::new(x, z, 0.0),
                        _ => Vec3::new(0.0, x, z),
                    };
                    vertices.push(Vertex {
                        position: p.into(),
                        normal: Vec3::Y.into(),
                        uv: [0.5, 0.5],
                        joints: [0; 4],
                        weights: [1.0, 0.0, 0.0, 0.0],
                    });
                }
            }
            for segment in 0..64 {
                let i = base + segment * 2;
                indices.extend_from_slice(&[i, i + 1, i + 2, i + 1, i + 3, i + 2]);
            }
        }
        pack.models.insert(
            name.into(),
            Model {
                graph: None,
                markers: Vec::new(),
                states: Default::default(),
                skin: None,
                source: format!("verse/procedural/{name}"),
                source_sha256: format!("{:x}", Sha256::digest(name.as_bytes())),
                height: 2.0,
                bones: vec![],
                clips: vec![],
                attachments: vec![],
                surfaces: vec![Surface {
                    material: Default::default(),
                    vertices,
                    indices,
                    texture,
                    blend: 3,
                    emissive: true,
                    topology: Default::default(),
                    unlit: false,
                    tint: color,
                }],
            },
        );
    }
    let texture = particle_texture(pack, dir, "particle-rune")?;
    let (quad, triangles) = particle_quad();
    pack.models.get_mut("effect-rune").unwrap().surfaces[0] = Surface {
        material: Default::default(),
        vertices: quad,
        indices: triangles,
        texture,
        blend: 3,
        emissive: true,
        topology: Default::default(),
        unlit: false,
        tint: [0.65, 0.15, 0.85],
    };
    pack.models.get_mut("effect-rune").unwrap().source = "verse/ground/effect-rune".into();
    let mut wave = pack.models["effect-rune"].clone();
    wave.source = "verse/ground/effect-wave".into();
    wave.surfaces[0].tint = [0.2, 0.55, 1.0];
    pack.models.insert("effect-wave".into(), wave);
    let shell = &mut pack.models.get_mut("effect-shield").unwrap().surfaces[0];
    shell.vertices = vertices;
    shell.indices = indices;
    pack.validate()
}
