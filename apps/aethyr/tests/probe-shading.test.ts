import test from "node:test";
import assert from "node:assert/strict";
import {
  WGSL_PROBES as compactProbes,
  probeShadingWGSL,
} from "../packages/renderer/probe-shading.ts";
import { halfToFloat } from "../packages/renderer/environment-lighting.ts";
import { WGSL_IBL } from "../packages/renderer/ibl.ts";
import { compactGeneratedWGSL } from "../packages/renderer/wgsl-source.ts";
import { MATERIAL } from "../packages/renderer/shaders.ts";

const WGSL_PROBES = probeShadingWGSL({ compactIdentifiers: false });

const f32 = Math.fround;
const add = (a: number, b: number) => f32(a + b);
const mul = (a: number, b: number) => f32(a * b);

// Run the shipped scalar-channel SH statements, not a second optimized model.
// Each WGSL multiplication and addition rounds independently to f32 here.
const coefficients = (source: string) =>
  source.slice(
    source.indexOf("  var value=vec3(0.0);"),
    source.indexOf("  skyBounce+=max(skyValue"),
  );
const coefficientSource = coefficients(WGSL_PROBES);
function createEvaluator(source: string) {
  const coefficientJS = coefficients(source)
    .replace(/\b(?:var|let)\b/g, "let")
    .replace(/vec3\(0\.0\)/g, "0")
    .replace(/\b(\d+)u\b/g, "$1")
    .replace(/\.rgb/g, "[channel]")
    .replace(/if ([a-zA-Z]+)\{/g, "if ($1){")
    .replace(
      /(value|skyValue|sunValue|localValue)\+=([^;]+);/g,
      (_, lhs, rhs) => {
        const factors = String(rhs).split("*");
        const expression = factors
          .slice(1)
          .reduce((left, right) => `mul(${left},${right})`, factors[0]);
        return `${lhs}=add(${lhs},${expression});`;
      },
    );
  return new Function(
    "probeRecords",
    "bounceRecords",
    "basis",
    "frame",
    "index",
    "count",
    "probeCount",
    "flags",
    "channel",
    "add",
    "mul",
    `const probeBase=index*10,shBase=index*9;
   const constantProbeBase=probeCount*10,constantBounceBase=count*27;
   const [constantSkyEnabled,skyBounceEnabled,constantBounceEnabled,sunBounceEnabled,localBounceEnabled]=flags;
   const needI=flags[5]??true;
   const skyGain=frame.skyLayers.x;
   ${coefficientJS}
   return [value,skyValue,sunValue,localValue];`,
  ) as (
    probeRecords: number[][],
    bounceRecords: number[][],
    basis: number[],
    frame: { skyLayers: { x: number } },
    index: number,
    count: number,
    probeCount: number,
    flags: boolean[],
    channel: number,
    add: (a: number, b: number) => number,
    mul: (a: number, b: number) => number,
  ) => number[];
}
const runShipped = createEvaluator(WGSL_PROBES);
const runUnrolled = createEvaluator(
  probeShadingWGSL({ unrollSH9: true, compactIdentifiers: false }),
);

test("probe SH branch hoisting preserves f32 sums for every lighting-layer combination", () => {
  let seed = 12999;
  const random = () => {
    seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
    return f32((seed / 0xffffffff - 0.5) * 2048);
  };
  const count = 24;
  const probeRecords = Array.from({ length: count * 19 }, () =>
    Array.from({ length: 3 }, random),
  );
  const bounceRecords = Array.from({ length: count * 36 }, () =>
    Array.from({ length: 3 }, random),
  );
  const basis = Array.from({ length: 9 }, () => f32(random() / 1024));
  for (let combination = 0; combination < 32; combination++) {
    const flags = Array.from(
      { length: 5 },
      (_, bit) => !!(combination & (1 << bit)),
    );
    for (const gain of [0, f32(0.13), 1, 3.5]) {
      for (const index of [0, 1, 3, 4, 12, 13, 16, 23]) {
        for (let channel = 0; channel < 3; channel++) {
          // Original coefficient-major evaluator: direct and constant terms
          // alternate within each coefficient before advancing to the next.
          const expected = [0, 0, 0, 0];
          for (let coefficient = 0; coefficient < 9; coefficient++) {
            const term = (
              records: number[][],
              record: number,
              scale = false,
            ) => {
              const value = mul(records[record][channel], basis[coefficient]);
              return scale ? mul(value, gain) : value;
            };
            expected[0] = add(
              expected[0],
              term(probeRecords, index * 10 + 1 + coefficient, true),
            );
            if (flags[0])
              expected[0] = add(
                expected[0],
                term(probeRecords, count * 10 + index * 9 + coefficient),
              );
            if (flags[1]) {
              expected[1] = add(
                expected[1],
                term(bounceRecords, index * 9 + coefficient, true),
              );
              if (flags[2])
                expected[1] = add(
                  expected[1],
                  term(bounceRecords, count * 27 + index * 9 + coefficient),
                );
            }
            if (flags[3])
              expected[2] = add(
                expected[2],
                term(bounceRecords, (count + index) * 9 + coefficient),
              );
            if (flags[4])
              expected[3] = add(
                expected[3],
                term(bounceRecords, (count * 2 + index) * 9 + coefficient),
              );
          }
          assert.deepEqual(
            runShipped(
              probeRecords,
              bounceRecords,
              basis,
              { skyLayers: { x: gain } },
              index,
              count,
              count,
              flags,
              channel,
              add,
              mul,
            ),
            expected,
            `Layer combination ${combination}, gain ${gain}, record ${index}, channel ${channel}`,
          );
          assert.deepEqual(
            runUnrolled(
              probeRecords,
              bounceRecords,
              basis,
              { skyLayers: { x: gain } },
              index,
              count,
              count,
              flags,
              channel,
              add,
              mul,
            ),
            expected,
            "Literal-index unrolling must preserve the original f32 accumulation order.",
          );
          assert.deepEqual(
            runShipped(
              probeRecords,
              bounceRecords,
              basis,
              { skyLayers: { x: gain } },
              index,
              count,
              count,
              [...flags, false],
              channel,
              add,
              mul,
            ),
            [0, ...expected.slice(1)],
            "Skipping replaced primary irradiance must preserve every bounce layer.",
          );
          assert.deepEqual(
            runUnrolled(
              probeRecords,
              bounceRecords,
              basis,
              { skyLayers: { x: gain } },
              index,
              count,
              count,
              [...flags, false],
              channel,
              add,
              mul,
            ),
            [0, ...expected.slice(1)],
          );
        }
      }
    }
  }
});

test("probe SH coefficient loops contain no uniform branches and keep all nine terms", () => {
  assert.ok(coefficientSource.length > 0);
  const loops = [
    ...coefficientSource.matchAll(
      /for\(var coefficient=0u;coefficient<9u;coefficient\+\+\)\{([^}]+)\}/g,
    ),
  ];
  assert.equal(loops.length, 6);
  assert.ok(loops.every(([, body]) => !body.includes("if ")));
  assert.match(WGSL_PROBES, /for\(var corner=0u;corner<8u;corner\+\+\)/);
});

test("unrolled generation removes dynamic coefficient indices and stays within the hot-edit bound", () => {
  assert.equal(compactProbes, probeShadingWGSL());
  const readable = probeShadingWGSL({
    unrollSH9: true,
    compactIdentifiers: false,
  });
  assert.ok(!readable.includes("var coefficient="));
  assert.ok(!readable.includes("basis[coefficient]"));
  for (let coefficient = 0; coefficient < 9; coefficient++)
    assert.equal(
      (
        coefficients(readable).match(
          new RegExp(`basis\\[${coefficient}u\\]`, "g"),
        ) ?? []
      ).length,
      8,
    );
  const unrolled = compactGeneratedWGSL(probeShadingWGSL({ unrollSH9: true }));
  const core = (source: string) => {
    const start = source.indexOf("fn localSkyLightingComponents(");
    assert.ok(start >= 0);
    let depth = 1,
      end = source.indexOf("{", start) + 1;
    for (; end < source.length && depth; end++) {
      if (source[end] === "{") depth++;
      if (source[end] === "}") depth--;
    }
    assert.equal(depth, 0);
    return source.slice(start, end);
  };
  // Generated-only pruning may remove unreachable compatibility wrappers. The
  // actual shader must retain the complete literal-index evaluator unchanged.
  assert.equal(core(MATERIAL), core(unrolled));
  assert.ok(new TextEncoder().encode(MATERIAL).byteLength <= 65536);
});

test("owned identifier shortening changes only consistent, noncolliding identifiers", () => {
  for (const unrollSH9 of [false, true]) {
    const original = compactGeneratedWGSL(
      probeShadingWGSL({ unrollSH9, compactIdentifiers: false }),
    );
    const compacted = compactGeneratedWGSL(probeShadingWGSL({ unrollSH9 }));
    const tokens = (source: string) =>
      source.match(/[A-Za-z_][A-Za-z_0-9]*|\s+|./g)!;
    const before = tokens(original),
      after = tokens(compacted);
    assert.equal(before.length, after.length);
    const forward = new Map<string, string>(),
      reverse = new Map<string, string>();
    for (let index = 0; index < before.length; index++) {
      if (before[index] === after[index]) continue;
      assert.match(before[index], /^[A-Za-z_][A-Za-z_0-9]*$/);
      assert.match(after[index], /^P[a-z0-9]{1,2}$/);
      assert.ok(!original.match(new RegExp(`\\b${after[index]}\\b`)));
      assert.ok(
        !forward.has(before[index]) ||
          forward.get(before[index]) === after[index],
      );
      assert.ok(
        !reverse.has(after[index]) ||
          reverse.get(after[index]) === before[index],
      );
      forward.set(before[index], after[index]);
      reverse.set(after[index], before[index]);
    }
    for (const stable of [
      "localSkyLighting",
      "localSkyLightingComponents",
      "localSkyIrradiance",
      "AF_PROBE_GATHER",
      "AF_SELECTIVE_GI",
      "irradiance",
      "confidence",
      "specularVisibility",
      "skyBounce",
      "sunBounce",
      "localBounce",
    ])
      assert.ok(!forward.has(stable));
  }
});

test("both visibility queries reuse the same atlas tile and retain the original interpolation", () => {
  const lighting = WGSL_PROBES.slice(
    WGSL_PROBES.indexOf("fn localSkyLightingComponents("),
  );
  assert.equal((lighting.match(/index%columns/g) ?? []).length, 1);
  assert.ok(
    lighting.indexOf("let reflectedOct=") < lighting.indexOf("for(var corner="),
  );
  assert.match(lighting, /distanceMomentsAt\(tile,probeOct\(direction\)\)/);
  assert.match(lighting, /distanceMomentsAt\(tile,reflectedOct\)/);
  const sampler = WGSL_PROBES.slice(
    WGSL_PROBES.indexOf("fn distanceMomentsAt("),
    WGSL_PROBES.indexOf("fn distanceMoments("),
  );
  assert.equal((sampler.match(/textureLoad\(probeMoments/g) ?? []).length, 4);
  assert.equal(
    (
      sampler.match(
        /textureGather\([01],probeMoments,environmentSampler,gatherUV\)/g,
      ) ?? []
    ).length,
    2,
  );
  assert.equal(
    (
      sampler.match(
        /return mix\(mix\(a,b,fraction\.x\),mix\(c,d,fraction\.x\),fraction\.y\)/g,
      ) ?? []
    ).length,
    2,
  );
  assert.match(WGSL_PROBES, /override AF_PROBE_GATHER:bool=true/);
  assert.match(sampler, /if AF_PROBE_GATHER\{/);
  assert.match(
    sampler,
    /let gatherUV=\(vec2<f32>\(lo\)\+vec2\(1\.0\)\)\/vec2<f32>\(textureDimensions\(probeMoments\)\)/,
  );
  assert.ok(!sampler.includes("textureSample"));
});

test("selective GI skips only completely replaced irradiance and accepted local reflections", () => {
  assert.match(WGSL_PROBES, /override AF_SELECTIVE_GI:bool=true/);
  assert.match(
    WGSL_PROBES,
    /return localSkyLightingComponents\(world,normal,geometric,view,true,true\)/,
  );
  assert.match(
    WGSL_PROBES,
    /if !needI&&!needS&&all\(frame\.bounceSettings\.xyz<=vec3\(0\.5\)\)/,
  );
  assert.match(
    WGSL_PROBES,
    /if needS\{specularVisibility\+=probeSkyVisibility/,
  );
  assert.match(
    WGSL_IBL,
    /!AF_SELECTIVE_GI\|\|refinedSky\.w!=1\.0,!AF_SELECTIVE_GI\|\|!\(refinedReflection\.w>0\.5\)/,
  );
  assert.match(
    WGSL_IBL,
    /if !AF_SELECTIVE_GI\|\|!\(refinedReflection\.w>0\.5\)\{[\s\S]*?atan2[\s\S]*?textureSampleLevel\(environmentReflection/,
  );
  const mix = (a: number, b: number, t: number) =>
    add(mul(a, f32(1 - t)), mul(b, t));
  for (const confidence of [0, f32(0.33333334), 1]) {
    for (const refinedWeight of [0, 0.5, f32(0.99999994), 1]) {
      for (const global of [0, f32(0.0001), 3, 4096]) {
        for (const probe of [0, f32(0.731), 1, 65504]) {
          const refined = f32(1.234567);
          const expected = mix(
            mix(global, probe, confidence),
            refined,
            refinedWeight,
          );
          const actual = mix(
            mix(global, refinedWeight === 1 ? 0 : probe, confidence),
            refined,
            refinedWeight,
          );
          assert.equal(actual, expected);
        }
      }
    }
  }
});

test("centered gathers select the original four texels across every atlas edge and tile seam", () => {
  const corners = [0, 0.0000001, 0.125, 0.5, 0.99999994, 1];
  let queries = 0;
  for (const count of [8, 27, 512, 4096]) {
    for (const columns of [1, 3, 7, 64]) {
      const width = columns * 10;
      const height = Math.ceil(count / columns) * 10;
      const indices = new Set([
        0,
        count - 1,
        Math.min(count - 1, columns - 1),
        Math.min(count - 1, columns),
      ]);
      for (const index of indices) {
        for (const u of corners) {
          for (const v of corners) {
            const pixel = [
              add(add((index % columns) * 10, 1), mul(u, 8)) - 0.5,
              add(add(Math.floor(index / columns) * 10, 1), mul(v, 8)) - 0.5,
            ].map(f32);
            const lo = pixel.map(Math.floor);
            const dimensions = [width, height];
            const gatherLo = lo.map((value, axis) => {
              const uv = f32(add(value, 1) / dimensions[axis]);
              return Math.floor(f32(mul(uv, dimensions[axis]) - 0.5));
            });
            assert.deepEqual(gatherLo, lo);
            assert.ok(lo[0] >= 0 && lo[0] + 1 < width);
            assert.ok(lo[1] >= 0 && lo[1] + 1 < height);
            // No address mode can wrap into another tile: both neighbors stay
            // within this probe's ten-texel tile, including octahedral borders.
            assert.equal(Math.floor(lo[0] / 10), index % columns);
            assert.equal(Math.floor((lo[0] + 1) / 10), index % columns);
            assert.equal(Math.floor(lo[1] / 10), Math.floor(index / columns));
            assert.equal(
              Math.floor((lo[1] + 1) / 10),
              Math.floor(index / columns),
            );
            queries++;
          }
        }
      }
    }
  }
  assert.ok(queries > 1500);
});

test("shipped gather swizzles retain f32 moments and visibility at zero-variance boundaries", () => {
  const sampler = WGSL_PROBES.slice(
    WGSL_PROBES.indexOf("fn distanceMomentsAt("),
    WGSL_PROBES.indexOf("fn distanceMoments("),
  );
  const swizzles = [
    ...sampler.matchAll(/let ([abcd])=vec2\(r\.([xyzw]),g\.([xyzw])\)/g),
  ];
  assert.equal(swizzles.length, 4);
  // W3C textureGather order: bottom-left, bottom-right, top-right, top-left.
  const components = { x: 2, y: 3, z: 1, w: 0 };
  const mix = (a: number, b: number, t: number) =>
    add(mul(a, f32(1 - t)), mul(b, t));
  const interpolate = (corners: number[][], u: number, v: number) =>
    [0, 1].map((channel) =>
      mix(
        mix(corners[0][channel], corners[1][channel], u),
        mix(corners[2][channel], corners[3][channel], u),
        v,
      ),
    );
  const visibility = ([mean, second]: number[], distance: number) => {
    const variance = Math.max(f32(second - mul(mean, mean)), 0.0001);
    const gap = f32(distance - mean);
    return distance <= mean
      ? 1
      : f32(Math.pow(f32(variance / add(variance, mul(gap, gap))), 3));
  };
  const skyVisibility = ([mean, second]: number[]) => {
    if (mean >= 128 - 0.001) return 1;
    const variance = Math.max(f32(second - mul(mean, mean)), 0);
    const gap = f32(128 - mean);
    return f32(variance / Math.max(add(variance, mul(gap, gap)), 0.000001));
  };
  const fixtures = [
    [
      [0, 0],
      [0, 0],
      [0, 0],
      [0, 0],
    ],
    [
      [1, 1],
      [1, 1],
      [1, 1],
      [1, 1],
    ],
    [
      [1, 1],
      [1.0009765625, 1.001953125],
      [1, 1],
      [1.0009765625, 1.001953125],
    ],
    [
      [0.5, 0.25],
      [128, 16384],
      [128, 16384],
      [0.5, 0.25],
    ],
    // Distinct half-float subnormals expose swapped gather lanes too.
    [0x0001, 0x0002, 0x0400, 0x3c00].map((bits) => [
      halfToFloat(bits),
      halfToFloat(bits + 1),
    ]),
  ];
  for (const original of fixtures) {
    const gathered = swizzles.map(([, , r, g]) => [
      original[components[r as keyof typeof components]][0],
      original[components[g as keyof typeof components]][1],
    ]);
    assert.deepEqual(gathered, original);
    for (const u of [0, 0.00001, 0.125, 0.33333334, 0.5, 0.99999994, 1]) {
      for (const v of [0, 0.00001, 0.125, 0.33333334, 0.5, 0.99999994, 1]) {
        const expected = interpolate(original, f32(u), f32(v));
        const actual = interpolate(gathered, f32(u), f32(v));
        assert.deepEqual(actual, expected);
        for (const gap of [-0.0001, 0, 0.0001, 0.01, 0.1, 1]) {
          const distance = f32(expected[0] + gap);
          assert.equal(
            visibility(actual, distance),
            visibility(expected, distance),
          );
        }
        assert.equal(skyVisibility(actual), skyVisibility(expected));
      }
    }
  }
});
