import test from "node:test";
import assert from "node:assert/strict";
import {
  LOCAL_FOG_WGSL,
  LOCAL_FOG_LIMITS,
} from "../packages/renderer/local-fog.ts";
import { FOG_SEGMENT_WGSL } from "../packages/renderer/fog.ts";

function body(source: string, name: string): string {
  const start = source.indexOf(`fn ${name}(`);
  assert.ok(start >= 0, `Missing function ${name}.`);
  const first = source.indexOf("{", start);
  let depth = 1,
    end = first + 1;
  for (; end < source.length && depth; end++) {
    if (source[end] === "{") depth++;
    if (source[end] === "}") depth--;
  }
  assert.equal(depth, 0);
  return source.slice(first + 1, end - 1);
}
const compact = (source: string) => source.replace(/\s+/g, "");

test("fog retains the same sample count, capped resolution, positions, attenuation, and integration order", () => {
  assert.deepEqual(
    [
      LOCAL_FOG_LIMITS.steps,
      LOCAL_FOG_LIMITS.maxWidth,
      LOCAL_FOG_LIMITS.maxHeight,
    ],
    [24, 640, 360],
  );
  const integrate = body(LOCAL_FOG_WGSL, "integrateFS");
  assert.match(
    integrate,
    /let steps=u32\(fog\.volume\.w\);let ds=\(end-fog\.volume\.z\)\/f32\(steps\)/,
  );
  assert.match(
    integrate,
    /for\(var i=0u;i<steps;i\+\+\)\{let distance=fog\.volume\.z\+\(f32\(i\)\+0\.5\)\*ds;let p=frame\.eye\.xyz\+ray\*distance/,
  );
  assert.match(
    integrate,
    /result\+=\(fogIncident\(p,ray,cluster\)\+direct\+fogSkyIncident\(pixel,distance\)\)\*sigma\*fog\.medium\.w\*transmittance\*ds/,
  );
  assert.match(integrate, /result\*fogPreVolumeTransmittance\(ray\)/);
  assert.equal((integrate.match(/fogWorld\(/g) ?? []).length, 1);
  assert.equal((integrate.match(/fogDepth\(/g) ?? []).length, 1);
  const loop = integrate.indexOf("for(var i=");
  for (const operation of [
    "let logDensity=",
    "let logEyeDensity=",
    "fogDirectionalRadiance(ray)",
    "fogClusterRay(pixel)",
  ])
    assert.ok(integrate.indexOf(operation) < loop);
});

test("factored fog optical depth retains every original logarithm, exponential branch, and clamp", () => {
  const original = body(FOG_SEGMENT_WGSL, "segmentFogOpticalDepth");
  const optimized = body(LOCAL_FOG_WGSL, "fogStepTau");
  const originalAverage = original
    .slice(original.indexOf("let k="), original.indexOf("let logTau="))
    .replace(/\bheightRate\b/g, "fog.medium.y")
    .replace(/\bdirectionY\b/g, "ray.y");
  const optimizedAverage = optimized.slice(
    optimized.indexOf("let k="),
    optimized.indexOf("let logTau="),
  );
  assert.equal(compact(optimizedAverage), compact(originalAverage));
  assert.equal(
    compact(optimized.slice(optimized.indexOf("if logTau>="))),
    compact(original.slice(original.indexOf("if logTau>="))),
  );
  assert.match(optimized, /logEyeDensity\+log\(travel\)\+logAverage/);
  const integrate = body(LOCAL_FOG_WGSL, "integrateFS");
  assert.match(
    integrate,
    /logEyeDensity=logDensity-fog\.medium\.y\*\(frame\.eye\.y\+ray\.y\*fog\.volume\.z-fog\.medium\.z\)/,
  );
  assert.ok(
    integrate.indexOf("fog.medium.x<=0.0") <
      integrate.indexOf("let logDensity="),
  );
  assert.match(
    body(LOCAL_FOG_WGSL, "fogDensity"),
    /exp\(clamp\(logDensity-fog\.medium\.y\*\(p\.y-fog\.medium\.z\),-80\.0,11\.08214255\)\)/,
  );
});

test("f32 midpoint samples never precede the volume start and preserve the hoisted optical-depth prefix", () => {
  const f = Math.fround;
  const add = (a: number, b: number) => f(a + b),
    sub = (a: number, b: number) => f(a - b);
  const mul = (a: number, b: number) => f(a * b),
    log = (a: number) => f(Math.log(a));
  const prefix = (
    density: number,
    heightRate: number,
    eye: number,
    direction: number,
    start: number,
    reference: number,
  ) =>
    sub(
      log(density),
      mul(heightRate, sub(add(eye, mul(direction, start)), reference)),
    );
  let checked = 0;
  for (const density of [f(1e-40), f(0.0001), f(0.1), 10000])
    for (const heightRate of [0, f(0.027725887), 10])
      for (const eye of [-20000, 0, 2, 20000])
        for (const direction of [-1, f(-1e-8), 0, f(1e-8), 1])
          for (const [start, end] of [
            [0, 60],
            [1, 60],
            [999, 1000],
            [999, 999.00006103515625],
          ]) {
            const cached = prefix(
              density,
              heightRate,
              eye,
              direction,
              start,
              1,
            );
            const ds = f(sub(end, start) / 24);
            for (let index = 0; index < 24; index++) {
              const distance = add(start, mul(add(index, 0.5), ds));
              assert.ok(distance >= start);
              const originalStart = Math.min(start, distance);
              assert.equal(
                cached,
                prefix(density, heightRate, eye, direction, originalStart, 1),
              );
              checked++;
            }
          }
  assert.ok(checked > 20000);
});

test("fog rejects out-of-range lights before loading unused light lanes and keeps source-order sums", () => {
  const incident = body(LOCAL_FOG_WGSL, "fogIncident");
  const reject = incident.indexOf("if distance>=position.w{continue;}");
  assert.ok(reject > 0);
  for (const vector of ["record+4u", "record+8u", "record+12u"])
    assert.ok(incident.indexOf(`fogVector(${vector})`) > reject);
  assert.match(incident, /for\(var entry=0u;entry<iterations;entry\+\+\)/);
  assert.match(incident, /index=clusters\[c\.indices\+start\+entry\]/);
  assert.match(incident, /let iterations=select\(count,c\.total,overflow\)/);
  assert.match(
    incident,
    /radiance\+=max\(emission\.rgb\*emission\.w,vec3\(0\.0\)\)\*attenuation\*strength\*visibility\*fogPhase/,
  );
  assert.ok(!incident.includes("floor(pixel/"));
  assert.ok(!/clusters\[\d+u?\]/.test(incident));
});

test("directional fog reuses original shadow coordinates and hoists only the constant radiance prefix", () => {
  for (const [name, map] of [
    ["fogSunVisibility", "fogSunNear"],
    ["fogSunFarVisibility", "fogSunFar"],
  ]) {
    const fn = body(LOCAL_FOG_WGSL, name);
    assert.equal((fn.match(/shadowProjection\(/g) ?? []).length, 1);
    assert.match(
      fn,
      new RegExp(
        `textureSampleCompareLevel\\(${map},fogShadowSampler,q\\.uv,q\\.depth-0\\.00005\\)`,
      ),
    );
  }
  const directional = body(LOCAL_FOG_WGSL, "fogDirectionalRadiance");
  assert.match(
    directional,
    /max\(frame\.sunColor\.rgb,vec3\(0\.0\)\)\*frame\.sun\.w\*fog\.flags\.z\*fogPhase\(clamp\(dot\(l,ray\),-1\.0,1\.0\)\)/,
  );
  assert.ok(!directional.includes("fogSunVisibility("));
  const integrate = body(LOCAL_FOG_WGSL, "integrateFS");
  assert.match(
    integrate,
    /if any\(directional>vec3\(0\.0\)\)\{direct=directional\*fogSunVisibility\(p\);\}/,
  );
});
