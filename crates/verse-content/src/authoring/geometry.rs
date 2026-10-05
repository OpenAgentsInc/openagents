//! Parametric author geometry compiled into the engine's static mesh contract.
use super::*;
use serde::{Deserialize, Serialize};
use verse_engine::assets::{Model, Surface, Vertex};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoxGeometry {
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub texture: usize,
    pub tint: [f32; 3],
}
pub fn compile(key: &str, shape: &BoxGeometry, texture_count: usize) -> Result<Model> {
    let min = glam::Vec3::from_array(shape.min);
    let max = glam::Vec3::from_array(shape.max);
    if key.is_empty()
        || key.len() > 96
        || !key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/-_".contains(&b))
        || !min.is_finite()
        || !max.is_finite()
        || !min.cmplt(max).all()
        || min.abs().max_element() > 1000.
        || max.abs().max_element() > 1000.
        || shape.texture >= texture_count
        || shape
            .tint
            .iter()
            .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
    {
        return Err(Diagnostic::at(
            "document.json",
            format!("primitives.{key}"),
            "Use a bounded model key, finite ordered bounds within 1000 meters, an admitted texture slot, and tint channels from 0 to 1",
        ));
    }
    let inverse = crate::basis().inverse();
    let mut vertices = vec![];
    let mut indices = vec![];
    let faces = [
        (
            glam::Vec3::X,
            [
                [max.x, min.y, min.z],
                [max.x, max.y, min.z],
                [max.x, max.y, max.z],
                [max.x, min.y, max.z],
            ],
        ),
        (
            -glam::Vec3::X,
            [
                [min.x, min.y, max.z],
                [min.x, max.y, max.z],
                [min.x, max.y, min.z],
                [min.x, min.y, min.z],
            ],
        ),
        (
            glam::Vec3::Y,
            [
                [min.x, max.y, min.z],
                [min.x, max.y, max.z],
                [max.x, max.y, max.z],
                [max.x, max.y, min.z],
            ],
        ),
        (
            -glam::Vec3::Y,
            [
                [min.x, min.y, max.z],
                [min.x, min.y, min.z],
                [max.x, min.y, min.z],
                [max.x, min.y, max.z],
            ],
        ),
        (
            glam::Vec3::Z,
            [
                [max.x, min.y, max.z],
                [max.x, max.y, max.z],
                [min.x, max.y, max.z],
                [min.x, min.y, max.z],
            ],
        ),
        (
            -glam::Vec3::Z,
            [
                [min.x, min.y, min.z],
                [min.x, max.y, min.z],
                [max.x, max.y, min.z],
                [max.x, min.y, min.z],
            ],
        ),
    ];
    for (normal, points) in faces {
        let start = vertices.len() as u32;
        for (position, uv) in points
            .into_iter()
            .zip([[0., 0.], [0., 1.], [1., 1.], [1., 0.]])
        {
            vertices.push(Vertex {
                position: inverse.transform_point3(position.into()).to_array(),
                normal: inverse.transform_vector3(normal).normalize().to_array(),
                uv,
                joints: [0; 4],
                weights: [0.; 4],
            });
        }
        // The source basis preserves orientation.
        indices.extend([start, start + 1, start + 2, start, start + 2, start + 3]);
    }
    let source_sha256 = workspace::hash(&serde_json::to_vec(shape).map_err(|e| {
        Diagnostic::at("document.json", format!("primitives.{key}"), e.to_string())
    })?);
    Ok(Model {
        graph: None,
        markers: vec![],
        states: Default::default(),
        skin: None,
        source: "verse/authoring/box".into(),
        source_sha256,
        surfaces: vec![Surface {
            vertices,
            indices,
            texture: shape.texture,
            material: Default::default(),
            blend: 0,
            emissive: false,
            tint: shape.tint,
            topology: Default::default(),
            unlit: false,
        }],
        bones: vec![],
        clips: vec![],
        height: max.y - min.y,
        attachments: vec![],
    })
}
