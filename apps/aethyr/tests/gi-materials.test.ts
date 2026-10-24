import assert from "node:assert/strict";
import test from "node:test";
import type {
  CompiledMaterial,
  PaintLayerSlot,
} from "../packages/assets/src/scene.ts";
import {
  collectGiMaterialTextures,
  createGiTextureSampler,
  evaluateGiCoverage,
  evaluateGiMaterial,
  GI_MATERIAL_LIMITATIONS,
  GI_MATERIAL_PROFILE,
  type GiMaterialContext,
  type GiTextureSampler,
} from "../packages/renderer/gi-materials.ts";

const source = {
  master: "owned-fixture",
  graphSha256: "owned",
  parameters: {},
};
type V4 = [number, number, number, number];
function material(): CompiledMaterial {
  return {
    id: "owned",
    name: "Owned diffuse fixture",
    baseColorFactor: [1, 1, 1, 1],
    emissiveFactor: [0, 0, 0],
    roughness: 1,
    metallic: 0,
    normalScale: 1,
    uvScale: [1, 1],
    alphaMode: "OPAQUE",
    alphaCutoff: 0.5,
    doubleSided: false,
    packedChannels: "metallic-r-roughness-g-occlusion-b",
  };
}
function context(): GiMaterialContext {
  return {
    uv: [0.25, 0.5],
    uv1: [0.8, 0.2],
    vertexColor: [0.5, 0.5, 0.5, 0.5],
    world: [2, 6, 10],
    geometricNormal: [0, 1, 0],
    objectScale: [4, 5, 6],
  };
}
function slot(name: string, colorFactor: V4 = [1, 1, 1, 1]): PaintLayerSlot {
  return {
    baseColorUrl: name,
    normalUrl: `${name}-normal`,
    mraUrl: `${name}-mra`,
    colorFactor,
    emissiveIntensity: 0,
    emissiveColorSpace: "linear",
  };
}
function sampler(values: Record<string, V4>): GiTextureSampler {
  return (url) => {
    assert.ok(values[url], `Unexpected map ${url}`);
    return values[url];
  };
}
function close(actual: readonly number[], expected: readonly number[]) {
  assert.equal(actual.length, expected.length);
  actual.forEach((value, c) =>
    assert.ok(
      Math.abs(value - expected[c]) < 1e-10,
      `${value} != ${expected[c]}`,
    ),
  );
}
function painted(): CompiledMaterial {
  return {
    ...material(),
    paint: {
      model: "ue-mm-blend-1-v1",
      slots: [slot("a"), slot("b")],
      uv: { mode: "objectYZ", scale: [19, 21] },
      height: {
        url: "height",
        sourceColorSpace: "linear",
        uvScale: [2, 3],
        contrast: 0,
      },
      overlay: {
        url: "overlay",
        sourceColorSpace: "srgb",
        uvSet: 1,
        useAlpha: true,
        colorIntensity: 1,
        roughnessIntensity: 0,
      },
      baseRoughnessIntensity: 1,
      source,
    },
  };
}

test("RGBA8 sampling decodes sRGB before bilinear interpolation and keeps alpha linear", () => {
  const sample = createGiTextureSampler(
    new Map([
      [
        "image",
        {
          width: 2,
          height: 1,
          pixels: new Uint8Array([0, 128, 255, 0, 255, 128, 0, 128]),
        },
      ],
    ]),
  );
  const middle = sample("image", "color", [0.5, 0.5]);
  close(middle, [0.5, ((128 / 255 + 0.055) / 1.055) ** 2.4, 0.5, 64 / 255]);
  close(sample("image", "data", [0.5, 0.5]), [0.5, 128 / 255, 0.5, 64 / 255]);
  close(
    sample("image", "color", [-0.75, 8.5]),
    sample("image", "color", [0.25, 0.5]),
  );
  close(sample("image", "color", [0, 0.5]), middle);
  close(sample("image", "color", [1, 0.5]), middle);
});

test("bilinear filtering wraps both axes at texel centers", () => {
  const sample = createGiTextureSampler(
    new Map([
      [
        "quad",
        {
          width: 2,
          height: 2,
          pixels: new Uint8Array([
            255, 0, 0, 255, 0, 255, 0, 0, 0, 0, 255, 128, 255, 255, 255, 64,
          ]),
        },
      ],
    ]),
  );
  close(sample("quad", "data", [0.25, 0.25]), [1, 0, 0, 1]);
  close(sample("quad", "data", [0.5, 0.5]), [
    0.5,
    0.5,
    0.5,
    (255 + 128 + 64) / 1020,
  ]);
  close(sample("quad", "data", [0, 0]), sample("quad", "data", [0.5, 0.5]));
});

test("texture admission snapshots dimensions and enforces the 256 MiB decoded budget", () => {
  const image = {
    width: 1,
    height: 1,
    pixels: new Uint8Array([255, 0, 0, 255]),
  };
  const sample = createGiTextureSampler(new Map([["one", image]]));
  image.width = 5;
  close(sample("one", "data", [0.5, 0.5]), [1, 0, 0, 1]);
  const large = {
    width: 4096,
    height: 4096,
    pixels: new Uint8Array(4096 * 4096 * 4),
  };
  assert.throws(
    () =>
      createGiTextureSampler(
        new Map(Array.from({ length: 5 }, (_, i) => [`map-${i}`, large])),
      ),
    /256 MiB/,
  );
});

test("generic materials use actual color factors, MRA metal, UV tiling, and energy rejection", () => {
  const m = material();
  m.baseColorUrl = "base";
  m.mraUrl = "packed";
  m.baseColorFactor = [0.5, 1, 0.25, 1];
  m.metallic = 0.5;
  m.uvScale = [2, 3];
  const calls: number[][] = [];
  const sample: GiTextureSampler = (url, role, uv) => {
    calls.push(uv);
    return url === "base" ? [0.5, 0.7, 0.9, 0.2] : [0.4, 0.9, 0.1, 1];
  };
  const result = evaluateGiMaterial(m, context(), sample);
  close(result.reflectance, [0.192, 0.5376, 0.1728]);
  assert.equal(result.coverage, 1);
  assert.ok(calls.every((uv) => uv[0] === 0.5 && uv[1] === 1.5));
  m.metallic = 1;
  close(
    evaluateGiMaterial(
      m,
      context(),
      sampler({ base: [1, 1, 1, 1], packed: [1, 0, 1, 1] }),
    ).reflectance,
    [0, 0, 0],
  );
  assert.equal(result.approximations.length, GI_MATERIAL_LIMITATIONS.length);
  assert.equal(GI_MATERIAL_PROFILE, "source-diffuse-material-v1");
});

test("two-layer color uses source vertex red, layer alpha height, and world-aligned subtraction", () => {
  const m = material();
  m.baseColorFactor = [0, 1, 0, 0];
  m.layers = {
    model: "ue-mm-blend-2-v1",
    slots: [slot("a", [1, 0, 0, 1]), slot("b", [0, 0, 1, 1])],
    uv: { mode: "objectXY", scale: [20, 30] },
    blend: {
      vertexChannel: "r",
      phase: 0.75,
      contrast: 0,
      height: {
        mode: "layer2Alpha",
        customIntensity: 1,
        sourceColorSpace: "linear",
      },
      worldSubtract: {
        url: "world",
        sourceColorSpace: "linear",
        sizeMeters: 2,
        intensity: 0.5,
        power: 1,
        channel: "r",
        projection: "ue-world-aligned-v1",
      },
    },
    detailNormal: { url: "detail", uvScale: [1, 1], flatness: 0 },
    colorIntensity: 1,
    source,
  };
  const calls: { url: string; uv: number[] }[] = [];
  const data = sampler({
    a: [1, 1, 1, 1],
    b: [1, 1, 1, 0.25],
    "a-mra": [0.2, 1, 1, 1],
    "b-mra": [0.6, 1, 1, 1],
    world: [0.5, 0, 0, 1],
  });
  const result = evaluateGiMaterial(m, context(), (url, role, uv) => {
    calls.push({ url, uv });
    return data(url, role, uv);
  });
  close(result.reflectance, [0.288, 0, 0.288]);
  close(calls.find((v) => v.url === "a")!.uv, [1, 3]);
  assert.deepEqual(
    calls.filter((v) => v.url === "world").map((v) => v.uv),
    [
      [-1, -3],
      [-5, -3],
      [-1, -5],
    ],
  );
  m.layers.blend.height = {
    mode: "maskRed",
    url: "height",
    sourceColorSpace: "srgb",
    customIntensity: 10,
  };
  m.layers.blend.worldSubtract.intensity = 0;
  const masked = evaluateGiMaterial(
    m,
    context(),
    sampler({
      a: [1, 1, 1, 1],
      b: [1, 1, 1, 1],
      "a-mra": [0, 1, 1, 1],
      "b-mra": [0, 1, 1, 1],
      world: [0, 0, 0, 1],
      height: [0.5, 0, 0, 1],
    }),
  );
  close(masked.reflectance, [0.48, 0, 0.48]);
});

test("paint follows blue layer blending and green UV1 overlay with linear alpha gating", () => {
  const m = painted();
  const calls: { url: string; role: string; uv: number[] }[] = [];
  const data = sampler({
    a: [0.2, 0.4, 0.6, 0],
    b: [0.8, 0.6, 0.4, 0],
    "a-mra": [0, 1, 1, 1],
    "b-mra": [0, 1, 1, 1],
    height: [0.5, 0, 0, 1],
    overlay: [0.25, 0.75, 0.5, 0.4],
  });
  const sample: GiTextureSampler = (url, role, uv) => {
    calls.push({ url, role, uv });
    return data(url, role, uv);
  };
  close(
    evaluateGiMaterial(m, context(), sample).reflectance,
    [0.432, 0.528, 0.48],
  );
  assert.deepEqual(calls.find((v) => v.url === "height")!.uv, [3, 7.5]);
  assert.deepEqual(
    calls.find((v) => v.url === "overlay"),
    { url: "overlay", role: "color", uv: [0.8, 0.2] },
  );
  m.paint!.overlay.useAlpha = false;
  close(
    evaluateGiMaterial(m, context(), sample).reflectance,
    [0.36, 0.6, 0.48],
  );
});

test("terrain reproduces three-layer vertex weights and wetness color without parallax", () => {
  const m = material();
  m.terrain = {
    model: "ue-mm-blend-3-v1",
    slots: [slot("a"), slot("b"), slot("c")],
    uv: { mode: "mesh", scale: [2, 4] },
    height: {
      url: "height",
      sourceColorSpace: "linear",
      uvScale: [1, 1],
      contrast: 0,
      useCustom: false,
      customIntensity: 1,
    },
    wetness: {
      url: "wet",
      sourceColorSpace: "linear",
      colorIntensity: 2,
      roughnessIntensity: 1,
    },
    parallax: {
      enabled: true,
      url: "omitted-parallax",
      sourceColorSpace: "linear",
      referencePlane: 0.5,
      heightRatio: 0.1,
      uvSet: 0,
    },
    baseRoughnessIntensity: 1,
    source,
  };
  const data = sampler({
    a: [0.2, 0.4, 0.6, 1],
    b: [0.8, 0.6, 0.4, 0.25],
    c: [0.1, 0.2, 0.3, 1],
    "a-mra": [0, 0.25, 1, 1],
    "b-mra": [0.2, 0.8, 1, 1],
    "c-mra": [0.4, 0.6, 1, 1],
    height: [0.5, 0, 0, 1],
    wet: [0.05, 0.1, 0.15, 0.5],
  });
  const ctx = context();
  ctx.vertexColor = [0, 0, 1, 1];
  close(
    evaluateGiMaterial(m, ctx, data).reflectance,
    [0.425, 0.35, 0.275].map((v) => v * 0.576),
  );
  ctx.vertexColor = [0, 1, 1, 1];
  close(
    evaluateGiMaterial(m, ctx, data).reflectance,
    [0.1, 0.2, 0.3].map((v) => v * 0.576),
  );
  ctx.vertexColor = [1, 1, 1, 1];
  close(
    evaluateGiMaterial(m, ctx, data).reflectance,
    [0.8, 0.6, 0.4].map((v) => v * 0.768),
  );
  m.terrain.height.useCustom = true;
  ctx.vertexColor = [0.5, 1, 0.5, 1];
  // Custom height .6875 drives blue; raw height .5 independently drives red.
  close(
    evaluateGiMaterial(m, ctx, data).reflectance,
    [0.35625, 0.36875, 0.38125].map((v) => v * 0.702),
  );
});

test("vegetation retains source color masks and specular energy while omitting view-dependent fuzz", () => {
  const m = material();
  m.alphaMode = "MASK";
  m.vertexOpacity = true;
  m.baseColorFactor = [0, 0, 0, 0];
  m.vegetation = {
    model: "ue-mm-vegetation-1-v1",
    baseColorUrl: "leaf",
    normalUrl: "omitted-normal",
    mraUrl: "packed",
    mask: {
      url: "mask",
      sourceColorSpace: "linear",
      enabled: true,
      color: [0.5, 1, 0.25],
    },
    tint: [1, 0.5, 0.25],
    color: {
      luminance: [0.2, 0.7, 0.1],
      desaturation: 0.5,
      contrast: 1,
      intensity: 2,
      overlayColor: [1, 1, 1],
      overlayStrength: 0,
    },
    gradient: { axis: "v", intensity: 0.4 },
    aoAffectsColorIntensity: 0.8,
    normalFlatness: 0,
    roughnessIntensity: 1,
    specularAmount: 0.4,
    fuzz: { coreDarkness: 1e5, power: 1e5, edgeBrightness: 1e5 },
    subsurface: { color: [1, 1, 1], intensity: 100 },
    alphaCutoff: 0.4,
    twoSided: true,
    lightingModel: "two-sided-foliage",
    sourceWindEnabled: true,
    source,
  };
  const data = sampler({
    leaf: [0.2, 0.4, 0.6, 0.3],
    packed: [0.25, 1, 0.5, 1],
    mask: [0.5, 0, 0, 1],
  });
  const result = evaluateGiMaterial(m, context(), data);
  close(
    result.reflectance,
    [0.1632, 0.4632, 0.2552].map((v) => v * 0.74514),
  );
  assert.deepEqual(
    {
      coverage: result.coverage,
      masked: result.masked,
      cutoff: result.cutoff,
      doubleSided: result.doubleSided,
    },
    { coverage: 0.3, masked: true, cutoff: 0.4, doubleSided: true },
  );
  assert.deepEqual(
    evaluateGiCoverage(m, context(), (url) => {
      assert.equal(url, "leaf");
      return [0, 0, 0, 0.2];
    }),
    { coverage: 0.2, masked: true, cutoff: 0.4 },
  );
});

test("alpha-only traversal multiplies generic and vertex alpha without evaluating paint graphs", () => {
  const m = material();
  m.baseColorUrl = "base";
  m.alphaMode = "MASK";
  m.baseColorFactor[3] = 0.8;
  m.vertexOpacity = true;
  const alpha = evaluateGiCoverage(
    m,
    context(),
    sampler({ base: [0, 0, 0, 0.4] }),
  );
  close([alpha.coverage], [0.16]);
  assert.equal(alpha.masked, true);
  m.vertexOpacity = false;
  close(
    [
      evaluateGiCoverage(m, context(), sampler({ base: [0, 0, 0, 0.4] }))
        .coverage,
    ],
    [0.32],
  );
  const p = painted();
  p.alphaMode = "MASK";
  p.vertexOpacity = true;
  p.baseColorFactor[3] = 0.8;
  close(
    [
      evaluateGiCoverage(p, context(), () => {
        throw new Error("No paint map belongs in alpha coverage");
      }).coverage,
    ],
    [0.4],
  );
  p.alphaMode = "OPAQUE";
  assert.deepEqual(
    evaluateGiCoverage(p, context(), () => {
      throw Error("No lookup");
    }),
    { coverage: 1, masked: false, cutoff: 0.5 },
  );
});

test("texture collection keeps role identity and excludes normals, emission, parallax, and blend", () => {
  const m = painted();
  m.normalUrl = "omitted-normal";
  m.emissiveUrl = "omitted-emission";
  m.paint!.slots[0].emissiveUrl = "omitted-slot-emission";
  m.paint!.slots[0].emissiveIntensity = 100;
  m.paint!.overlay.url = "a";
  m.paint!.height.url = "a";
  assert.deepEqual(collectGiMaterialTextures(m), [
    { url: "a", role: "color" },
    { url: "a-mra", role: "data" },
    { url: "b", role: "color" },
    { url: "b-mra", role: "data" },
    { url: "a", role: "data" },
  ]);
  m.alphaMode = "BLEND";
  assert.deepEqual(collectGiMaterialTextures(m), []);
  const result = evaluateGiMaterial(m, context(), () => {
    throw Error("No lookup");
  });
  close(result.reflectance, [0, 0, 0]);
  assert.equal(result.coverage, 0);
});

test("invalid images, unknown profiles, missing maps, and nonfinite data fail explicitly", () => {
  assert.throws(
    () =>
      createGiTextureSampler(
        new Map([
          ["bad", { width: 4097, height: 1, pixels: new Uint8Array(4) }],
        ]),
      ),
    /bounded RGBA8/,
  );
  assert.throws(
    () =>
      createGiTextureSampler(
        new Map([
          ["bad", { width: 1.5, height: 1, pixels: new Uint8Array(4) }],
        ]),
      ),
    /bounded RGBA8/,
  );
  const sample = createGiTextureSampler(
    new Map([
      [
        "one",
        { width: 1, height: 1, pixels: new Uint8Array([255, 255, 255, 255]) },
      ],
    ]),
  );
  assert.throws(() => sample("absent", "color", [0, 0]), /not prepared/);
  assert.throws(() => sample("one", "color", [NaN, 0]), /finite/);
  const m = material();
  const ctx = context();
  m.metallic = NaN;
  assert.throws(() => evaluateGiMaterial(m, ctx, sample), /finite/);
  m.metallic = 0;
  m.alphaCutoff = -0.01;
  assert.throws(() => evaluateGiCoverage(m, ctx, sample), /cutoff.*\[0, 1\]/);
  m.alphaCutoff = 1.01;
  assert.throws(() => evaluateGiCoverage(m, ctx, sample), /cutoff.*\[0, 1\]/);
  m.alphaCutoff = NaN;
  assert.throws(() => evaluateGiCoverage(m, ctx, sample), /finite/);
  m.alphaCutoff = 0.5;
  ctx.world[0] = Infinity;
  assert.throws(() => evaluateGiMaterial(m, ctx, sample), /finite/);
  ctx.world[0] = 0;
  m.baseColorUrl = "one";
  assert.throws(
    () => evaluateGiMaterial(m, ctx, () => [NaN, 0, 0, 1]),
    /finite/,
  );
  const p = painted();
  (p.paint as unknown as { model: string }).model = "unknown-v9";
  assert.throws(
    () => evaluateGiMaterial(p, ctx, sample),
    /Unknown GI material paint/,
  );
  assert.throws(
    () => collectGiMaterialTextures(p),
    /Unknown GI material paint/,
  );
});
