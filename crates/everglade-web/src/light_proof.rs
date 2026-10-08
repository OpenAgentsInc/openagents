//! Private geometry comparison data for the test-compiled browser harness.
//! Production builds do not include this module or its query switch.
use js_sys::{Array, Object, Reflect, Uint8Array};
use verse::runtime::WorldRuntime;
use wasm_bindgen::JsValue;

pub(super) fn export(runtime: &WorldRuntime) -> Result<(), String> {
    let scene = runtime
        .world
        .mesh
        .textured
        .as_ref()
        .ok_or("no textured town")?;
    let merged = scene.merge()?;
    let proof = Object::new();
    let set = |name: &str, value: &JsValue| {
        Reflect::set(&proof, &JsValue::from_str(name), value)
            .map(|_| ())
            .map_err(|error| format!("scene proof: {error:?}"))
    };
    let mut vertices = Vec::with_capacity(merged.vertices.len() * 36);
    for vertex in &merged.vertices {
        for value in vertex.pos.iter().chain(&vertex.normal).chain(&vertex.uv) {
            vertices.extend_from_slice(&value.to_le_bytes());
        }
        vertices.extend_from_slice(&vertex.color);
    }
    set("vertices", &Uint8Array::from(vertices.as_slice()))?;
    drop(vertices);
    let indices: Vec<u8> = merged
        .indices
        .iter()
        .flat_map(|index| index.to_le_bytes())
        .collect();
    set("indices", &Uint8Array::from(indices.as_slice()))?;
    drop(indices);
    let images = Array::new();
    for image in &scene.images {
        images.push(&Uint8Array::from(image.rgba.as_slice()));
    }
    set("images", &images)?;
    let report = serde_json::json!({
        "scene": verse::pbr::baked_layers::hex(&verse::pbr::baked_layers::scene_digest(scene, &merged)),
        "vertices": merged.vertices.len(), "vertex_stride":36,
        "indices": merged.indices.len(), "architecture":"wasm32", "debug_assertions":cfg!(debug_assertions),
        "batches": merged.batches.iter().map(|b| serde_json::json!({
            "material":b.material,"first":b.first,"count":b.count,"level":format!("{:?}",b.level)
        })).collect::<Vec<_>>(),
        "materials": scene.materials.iter().map(|m| format!("{m:?}")).collect::<Vec<_>>(),
        "images": scene.images.iter().map(|image| serde_json::json!({
            "width":image.width,"height":image.height
        })).collect::<Vec<_>>(),
        "dirt_index": scene.images.iter().position(|image| image.name=="everglade/ground/dirt"),
    });
    set("report", &JsValue::from_str(&report.to_string()))?;
    Reflect::set(
        &js_sys::global(),
        &JsValue::from_str("__verseLightProof"),
        &proof,
    )
    .map(|_| ())
    .map_err(|error| format!("scene proof: {error:?}"))
}
