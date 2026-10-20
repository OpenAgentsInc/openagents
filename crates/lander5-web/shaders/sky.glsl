// /lander5, pass 1: the sky-view table. Runs once, into a small RGBA8
// texture (SKY_TABLE_SIZE in src/web.rs), so the frame pays for a
// physically based twilight sky with one texture read instead of the
// atmosphere integral (16 view x 8 light steps) per pixel.

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec3 rd = skyTableDir(fragCoord / iResolution.xy);
    vec3 sky = atmosphereSky(rd, SUN_DIR, VIEW_HEIGHT_KM, SUN_INTENSITY);
    fragColor = vec4(sqrt(clamp(sky / SKY_TABLE_MAX, 0.0, 1.0)), 1.0);
}
