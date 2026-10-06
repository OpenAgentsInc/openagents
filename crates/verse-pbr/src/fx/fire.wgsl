// Optical integration adapted from Daniel Greenheck, threejs-fire-pro (MIT).
// See assets/verse/fx/LICENSE-fire-pro.txt.
fn verse_fire_volume(xy: vec2<f32>, world: vec3<f32>, time: f32, mask: f32, count: u32, fire_noise: texture_3d<f32>, fire_lut: texture_2d<f32>, fire_sampler: sampler, lut_sampler: sampler) -> vec4<f32> {
    let radial = dot(xy, xy);
    if radial >= 1.0 || mask < 0.001 { return vec4<f32>(0.0); }
    let reach = sqrt(1.0 - radial);
    let ds = 2.0 * reach / f32(count);
    // Jitter the sample inside each interval; preserve both integration bounds.
    let jitter = fract(52.9829189 * fract(dot(xy * 512.0, vec2<f32>(0.06711056, 0.00583715))));
    var transmittance = 1.0;
    var radiance = vec3<f32>(0.0);
    let flow = vec3<f32>(0.07, -0.28, 0.04) * time;
    for (var step = 0u; step < count; step++) {
        let z = -reach + (f32(step) + jitter) * ds;
        let p = vec3<f32>(xy, z);
        let n = textureSampleLevel(fire_noise, fire_sampler, p * 0.17 + world * 0.035 + flow, 0.0).r;
        let detail = textureSampleLevel(fire_noise, fire_sampler, p * 0.43 - flow * 1.6, 0.0).r;
        let body = max(0.0, 1.0 - dot(p, p));
        let density = smoothstep(0.1, 0.72, body + (n - 0.5) * 0.9 + (detail - 0.5) * 0.35) * mask;
        let heat = clamp(body * 0.65 + density * 0.35, 0.0, 1.0);
        let kelvin = mix(950.0, 3300.0, heat);
        let thermal = textureSampleLevel(fire_lut, lut_sampler, vec2<f32>((kelvin - 500.0) / 9500.0, 0.5), 0.0);
        let extinction = density * 2.5;
        let tau = extinction * ds;
        // The small-tau series avoids cancellation and preserves thin slices.
        var integral = ds * (1.0 - 0.5 * tau + tau * tau / 6.0);
        if tau >= 0.001 { integral = (1.0 - exp(-tau)) / max(extinction, 0.00001); }
        let emission = thermal.rgb * density * (0.2 + 1.5 * heat * heat);
        radiance += transmittance * emission * integral;
        transmittance *= 1.0 - min(1.0, extinction * integral);
        if transmittance < 0.008 { break; }
    }
    return vec4<f32>(radiance, 1.0 - transmittance);
}
