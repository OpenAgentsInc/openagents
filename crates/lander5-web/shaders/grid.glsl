// /lander5, pass 2: the Grid at twilight. An infinite plane of filtered
// white lines on the near-black field, light pulses running along the
// major lines in white, a physically based sky read from the table
// (iChannel0, filled by sky.glsl), aerial perspective, and analytic height
// haze lit by the sun below the horizon and by the grid itself, all drawn
// in grayscale.
// Units are meters; the atmosphere works in kilometers.

#define DEBUG_MODE 0   // 0 final | 1 plane distance | 2 reflection direction | 3 cell ids | 4 filter footprint | 5 masks (lines, glow, pulses) | 6 light terms (emission, reflection, haze)
#ifndef CAMERA_TIME
#define CAMERA_TIME iTime
#endif

// ---- art controls ----
const float CELL = 4.0;              // minor line spacing, m
const float MAJOR = 8.0;             // minor cells per major cell
const float MINOR_WIDTH = 0.012;     // minor line width, fraction of a cell
const float MAJOR_WIDTH = 0.0035;     // major line width, fraction of a major cell
const float GLOW_RADIUS = 0.6;       // major line glow falloff, m
const float CAMERA_HEIGHT = 3.2;     // m
const float CAMERA_SPEED = 2.5;      // m/s, forward drift
const float HORIZON_UV = 0.16;       // where the horizon sits, in uv.y
const float FOCAL = 1.75;
const float EXPOSURE = 0.6;
const vec3  LINE_COLOR = vec3(0.84, 0.84, 0.84); // linear; #ededed
const vec3  PULSE_COLOR = vec3(1.0, 1.0, 1.0); // linear white
const float LINE_LIGHT = 1.6;
const float PULSE_LIGHT = 10.0;
const float AERIAL = 6.0;            // aerial perspective strength (haze above clear air)
const float HAZE_DENSITY = 0.0016;   // ground haze, per m
const float HAZE_FALLOFF = 0.12;     // per m of height
const float STAR_DENSITY = 0.0035;   // fraction of sky cells with a star

vec3 skyTable(vec3 rd) {
    vec3 s = textureLod(iChannel0, skyTableUv(rd), 0.0).rgb;
    return s * s * SKY_TABLE_MAX;
}

// Distance in m from x to the nearest line of a family with the given spacing.
float lineDistance(float x, float spacing) {
    return abs(fract(x / spacing + 0.5) - 0.5) * spacing;
}

// Glow exp(-d / r) around lines, filtered: once the pixel footprint covers
// the glow, it fades to the glow's mean over a period, so it never aliases.
float filteredGlow(float d, float spacing, float footprint) {
    float r = GLOW_RADIUS;
    float glow = exp(-d / r);
    float mean = (2.0 * r / spacing) * (1.0 - exp(-0.5 * spacing / r));
    return mix(glow, mean, smoothstep(0.3 * r, 3.0 * r, footprint));
}

// Box-filtered coverage of one family of lines (period 1, width 1/n), the
// one-axis term of the library's filteredGrid.
float lineCoverage(float x, float w, float n) {
    w = max(w, 1e-4);
    float a = x + 0.5 * w, b = x - 0.5 * w;
    return (floor(a) + min(fract(a) * n, 1.0) - floor(b) - min(fract(b) * n, 1.0)) / (n * w);
}

// Light pulses running along the major lines of one direction. `along` is
// the position along the lines and `across` the position across them (m);
// `w` is the footprint across. A pulse lights the line itself and its glow,
// so it fades with the line where the line drops below a pixel.
float pulses(float along, float across, float w, float seed) {
    float spacing = CELL * MAJOR;
    float id = floor(across / spacing + 0.5);
    float h = hash12(vec2(id, seed));
    if (h > 0.7) return 0.0;
    float period = 70.0 + 110.0 * hash12(vec2(id, seed + 17.0));
    float speed = (6.0 + 10.0 * h) * ((hash12(vec2(id, seed + 31.0)) < 0.5) ? -1.0 : 1.0);
    float head = mod(along - speed * iTime - period * h * 7.0, period);
    float tail = (speed > 0.0) ? period - head : head;
    float streak = exp(-tail / 12.0) * smoothstep(0.0, 0.6, tail);
    float line = lineCoverage(across / spacing + 0.5 * MAJOR_WIDTH, w / spacing, 1.0 / MAJOR_WIDTH);
    float r = GLOW_RADIUS;
    float glow = exp(-lineDistance(across, spacing) / r) * (1.0 - smoothstep(0.3 * r, 3.0 * r, w));
    return streak * (line + 0.5 * glow);
}

// Stars on a jittered lattice in (azimuth, elevation), drawn as one-pixel points.
float stars(vec3 rd, float pixelAngle) {
    vec2 a = vec2(atan(rd.x, -rd.z), asin(clamp(rd.y, -1.0, 1.0))) * 180.0;
    vec2 cell = floor(a);
    float h = hash12(cell);
    if (h > STAR_DENSITY) return 0.0;
    h /= STAR_DENSITY;
    vec2 at = cell + hash22(cell + 101.0);
    vec2 d = (a - at) / 180.0 / pixelAngle;
    float bright = pow(hash12(cell + 7.0), 6.0) * 0.6 + 0.02;
    float twinkle = 0.75 + 0.25 * sin(iTime * (1.0 + 3.0 * h) + 6.2832 * h);
    return bright * twinkle * exp(-dot(d, d) * 1.6);
}

vec3 render(vec2 fragCoord) {
    vec2 uv = (2.0 * fragCoord - iResolution.xy) / iResolution.y;

    // camera: low over the floor, drifting forward with a slow sway
    // (CAMERA_TIME holds it still where a page defines it)
    float camTime = CAMERA_TIME;
    float z = -mod(camTime * CAMERA_SPEED, CELL * MAJOR * 128.0);
    vec3 ro = vec3(9.0 + 3.0 * sin(camTime * 0.045), CAMERA_HEIGHT, z);
    vec3 ta = ro + vec3(0.12 * sin(camTime * 0.03), -HORIZON_UV / FOCAL, -1.0);
    mat3 cam = cameraMatrix(ro, ta, 0.0);
    vec3 rd = cameraRay(cam, uv, FOCAL);
    float pixelAngle = 2.0 / (iResolution.y * FOCAL);

    // The floor is hit analytically for every pixel (sky pixels get a far,
    // unused hit), so the filter footprint comes from derivatives taken
    // outside any branch.
    float tPlane = iPlane(ro, rd, vec3(0.0, 1.0, 0.0), 0.0);
    bool floorHit = tPlane > 0.0;
    float t = floorHit ? tPlane : 1e5;
    vec2 p = ro.xz + rd.xz * t;
    vec2 w = fwidth(p);
    float footprint = max(w.x, w.y);

#if DEBUG_MODE == 1
    return floorHit ? vec3(fract(log2(t))) : vec3(0.0, 0.0, 0.2);
#elif DEBUG_MODE == 4
    return floorHit ? debugHeat(clamp(footprint / CELL, 0.0, 1.0)) : vec3(0.0);
#endif

    vec3 skyDir = normalize(vec3(rd.x, max(rd.y, 0.0), rd.z));
    vec3 col;
    float haze;
    vec3 emission = vec3(0.0), reflection = vec3(0.0);
    if (floorHit) {
        // lines: minor and major families, box-filtered over the pixel footprint
        vec2 q = p / CELL;
        float minor = filteredGrid(q + 0.5 * MINOR_WIDTH, w / CELL, 1.0 / MINOR_WIDTH);
        vec2 qm = p / (CELL * MAJOR);
        float major = filteredGrid(qm + 0.5 * MAJOR_WIDTH, w / (CELL * MAJOR), 1.0 / MAJOR_WIDTH);
        float spacing = CELL * MAJOR;
        float glow = max(filteredGlow(lineDistance(p.x, spacing), spacing, w.x),
                         filteredGlow(lineDistance(p.y, spacing), spacing, w.y));
        float pulse = pulses(p.x, p.y, w.y, 3.0) + pulses(p.y, p.x, w.x, 11.0);
        // detail fades out where lines crowd below a pixel, leaving their average
        float distanceFade = exp(-t / 900.0);

#if DEBUG_MODE == 3
        return debugIdColor(dot(floor(qm), vec2(1.0, 57.0)));
#elif DEBUG_MODE == 5
        return vec3(max(minor, major), glow, pulse);
#endif

        emission = LINE_COLOR * LINE_LIGHT * (0.14 * minor + major + 0.05 * glow) * distanceFade
                 + PULSE_COLOR * PULSE_LIGHT * pulse * distanceFade;

        // a dark glossy floor reflecting the sky
        vec3 n = vec3(0.0, 1.0, 0.0);
        vec3 r = reflect(rd, n);
#if DEBUG_MODE == 2
        return 0.5 + 0.5 * r;
#endif
        float NoV = max(dot(n, -rd), 0.0);
        reflection = envBRDFApprox(vec3(0.04), NoV, 0.3) * skyTable(r) + 0.02 * skyTable(n);

        // aerial perspective: the air between camera and floor, filled with the horizon sky
        vec3 airT = exp(-t * 0.001 * AERIAL * atmosphereExtinction(atmosphereDensity(0.0)));
        col = (emission + reflection) * airT + skyTable(skyDir) * (1.0 - airT);
        haze = heightFog(ro, rd, t, HAZE_DENSITY, HAZE_FALLOFF);
    } else {
        col = skyTable(rd) + vec3(0.9, 0.92, 1.0) * stars(rd, pixelAngle) * smoothstep(0.02, 0.25, rd.y);
        haze = heightFog(ro, rd, 4000.0, HAZE_DENSITY, HAZE_FALLOFF);
    }

    // haze: lit by the sky's horizon, forward-scattered twilight toward the
    // sun, and a faint glow from the grid below it
    float mu = dot(rd, SUN_DIR);
    vec3 hazeLight = skyTable(skyDir) * (0.6 + 4.0 * PI * 0.35 * phaseCornetteShanks(mu, 0.7))
                   + LINE_COLOR * 0.012;
    col = mix(col, hazeLight, haze);

#if DEBUG_MODE == 6
    return vec3(luminance(emission), luminance(reflection) * 20.0, haze);
#endif

    // grayscale: the twilight's light without its color, white to grays to
    // near-black, in keeping with the white grid
    return vec3(luminance(col)) * vignette(uv, FOCAL * 1.3);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec3 col = render(fragCoord);
#if DEBUG_MODE == 0
    col = linearToSrgb(tonemapAgX(col * EXPOSURE)) + ditherTPDF(fragCoord, iFrame);
#endif
    fragColor = vec4(col, 1.0);
}
