//! Validates every assembled shader with Naga, the WGSL front end wgpu uses.
//! Browsers validate with Tint at pipeline creation; the browser check covers it.

fn validate(name: &str, source: &str) {
    let module = naga::front::wgsl::parse_str(source).unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(source)));
    naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
        .validate(&module)
        .unwrap_or_else(|e| panic!("{name}: {e:?}"));
}

const COMMON: &str = include_str!("../shaders/common.wgsl");

#[test]
fn every_shader_validates() {
    let forward = include_str!("../shaders/forward.wgsl");
    let cpu = forward
        .replace("//MATERIAL_GROUP", "@group(1) @binding(0) var<uniform> material_index: vec4u;")
        .replace("//MATERIAL_SELECT", "let material = material_index.x; _ = input.material;");
    let gpu = forward.replace("//MATERIAL_SELECT", "let material = input.material;");
    for (name, body) in [
        ("forward cpu", cpu.as_str()),
        ("forward gpu", gpu.as_str()),
        ("cull", include_str!("../shaders/cull.wgsl")),
        ("hiz", include_str!("../shaders/hiz.wgsl")),
        ("visbuffer", include_str!("../shaders/visbuffer.wgsl")),
        ("crowd", include_str!("../shaders/crowd.wgsl")),
        ("post", include_str!("../shaders/post.wgsl")),
    ] {
        validate(name, &format!("{COMMON}\n{body}"));
    }
    let slang = include_str!("../shaders/generated/cull.slang.wgsl");
    if !slang.trim().is_empty() {
        validate("cull (slang)", slang);
    }
}
