// /lander5's scene constants and the sky-view table's mapping, shared by
// both passes (sky.glsl fills the table, grid.glsl reads it).
//
// The table follows Hillaire, "A Scalable and Production Ready Sky and
// Atmosphere Rendering Technique" (EGSR 2020), section 5.3: u is the
// azimuth from the sun (the sky is mirror-symmetric about the sun's
// vertical plane, so 0..pi covers it), v is the elevation, warped by a
// square root so texels crowd the horizon, where the sky changes fastest.

// ---- art controls ----
const vec3  SUN_DIR = vec3(0.5375, -0.0698, -0.8404); // 4 degrees below the horizon, ahead and to the right
const float SUN_INTENSITY = 20.0;
const float VIEW_HEIGHT_KM = 0.002;
// The table holds sqrt(radiance / SKY_TABLE_MAX) in 8 bits a channel.
const float SKY_TABLE_MAX = 0.5;

// Table coordinates in [0,1]^2 for a view direction.
vec2 skyTableUv(vec3 rd) {
    vec2 h = rd.xz;
    float len = length(h);
    float cosAz = (len > 1e-5) ? dot(h / len, normalize(SUN_DIR.xz)) : 1.0;
    float u = acos(clamp(cosAz, -1.0, 1.0)) / PI;
    float e = asin(clamp(rd.y, -1.0, 1.0));
    float v = 0.5 + 0.5 * sign(e) * sqrt(abs(e) / (0.5 * PI));
    return vec2(u, v);
}

// The view direction a table texel stands for (the inverse of skyTableUv).
vec3 skyTableDir(vec2 uv) {
    float az = uv.x * PI + atan(SUN_DIR.x, SUN_DIR.z);
    float s = 2.0 * uv.y - 1.0;
    float e = sign(s) * s * s * 0.5 * PI;
    return vec3(sin(az) * cos(e), sin(e), cos(az) * cos(e));
}
