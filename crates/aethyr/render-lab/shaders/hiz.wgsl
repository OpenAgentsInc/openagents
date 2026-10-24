// Hierarchical depth: each texel stores the farthest depth of the texels it
// covers, so an object nearer than nothing in its footprint is hidden.
// Windows are 3x3 rather than 2x2 so odd sizes stay conservative.

@group(0) @binding(0) var depth: texture_depth_2d;
@group(0) @binding(1) var depth_ms: texture_depth_multisampled_2d;
@group(0) @binding(2) var src: texture_2d<f32>;
@group(0) @binding(3) var dst: texture_storage_2d<r32float, write>;

@compute @workgroup_size(8, 8)
fn first(@builtin(global_invocation_id) id: vec3u) {
  let out_dims = textureDimensions(dst);
  if (any(id.xy >= out_dims)) { return; }
  let in_dims = textureDimensions(depth);
  var z = 0.0;
  for (var y = 0u; y < 3u; y++) {
    for (var x = 0u; x < 3u; x++) {
      let p = min(id.xy * 2u + vec2u(x, y), in_dims - 1u);
      z = max(z, textureLoad(depth, p, 0));
    }
  }
  textureStore(dst, id.xy, vec4f(z, 0.0, 0.0, 0.0));
}

@compute @workgroup_size(8, 8)
fn first_msaa(@builtin(global_invocation_id) id: vec3u) {
  let out_dims = textureDimensions(dst);
  if (any(id.xy >= out_dims)) { return; }
  let in_dims = textureDimensions(depth_ms);
  var z = 0.0;
  for (var y = 0u; y < 3u; y++) {
    for (var x = 0u; x < 3u; x++) {
      let p = min(id.xy * 2u + vec2u(x, y), in_dims - 1u);
      for (var s = 0; s < 4; s++) {
        z = max(z, textureLoad(depth_ms, p, s));
      }
    }
  }
  textureStore(dst, id.xy, vec4f(z, 0.0, 0.0, 0.0));
}

@compute @workgroup_size(8, 8)
fn down(@builtin(global_invocation_id) id: vec3u) {
  let out_dims = textureDimensions(dst);
  if (any(id.xy >= out_dims)) { return; }
  let in_dims = textureDimensions(src, 0);
  var z = 0.0;
  for (var y = 0u; y < 3u; y++) {
    for (var x = 0u; x < 3u; x++) {
      let p = min(id.xy * 2u + vec2u(x, y), in_dims - 1u);
      z = max(z, textureLoad(src, p, 0).r);
    }
  }
  textureStore(dst, id.xy, vec4f(z, 0.0, 0.0, 0.0));
}
