import type { CompiledMaterial } from "../assets/src/scene.ts";

export const GI_MATERIAL_PROFILE = "source-diffuse-material-v1";
/** Normal orientation and decoding are outside prepared diffuse transport. */
export function giMaterialIdentityValue(material: CompiledMaterial) {
  const {
    normalConvention: _normalConvention,
    normalEncoding: _normalEncoding,
    ...transportMaterial
  } = material;
  return transportMaterial;
}
export const GI_MATERIAL_LIMITATIONS = Object.freeze([
  "Diffuse transport uses a Lambert approximation with metallic and dielectric energy rejection.",
  "Material maps use base-level bilinear filtering without a ray footprint or mip selection.",
  "Normal-map detail and view-dependent terrain parallax do not change transport geometry.",
  "Vegetation fuzz, subsurface transmission, and wind are omitted.",
  "Emissive surfaces, blended glass, flames, and surface decals do not contribute bounced light.",
]);

type V2 = [number, number];
type V3 = [number, number, number];
type V4 = [number, number, number, number];
export type GiTextureRole = "color" | "data";
export interface GiTexture {
  width: number;
  height: number;
  /** Straight-alpha RGBA8. Treat the pixels as immutable during a bake. */
  pixels: Uint8Array;
}
export type GiTextureSampler = (url: string, role: GiTextureRole, uv: V2) => V4;
export interface GiMaterialContext {
  /** Raw mesh UV0, before material tiling or object-scale routing. */
  uv: V2;
  uv1: V2;
  vertexColor: V4;
  world: V3;
  geometricNormal: V3;
  objectScale: V3;
}
export interface GiCoverage {
  /** Sampled alpha; compare against cutoff only when masked is true. */
  coverage: number;
  masked: boolean;
  cutoff: number;
}
export interface GiMaterialSample extends GiCoverage {
  /** Linear diffuse reflectance, without a 1/pi factor or incident illumination. */
  reflectance: V3;
  doubleSided: boolean;
  approximations: string[];
}

const MAX_SCALAR = 1e12;
const MAX_IMAGES = 2048;
const MAX_TEXELS = 67_108_864;
const WHITE: V4 = [1, 1, 1, 1];
const PACKED_DEFAULT: V4 = [0, 1, 1, 1];
const srgb = Float64Array.from({ length: 256 }, (_, byte) => {
  const value = byte / 255;
  return value <= 0.04045
    ? value / 12.92
    : Math.pow((value + 0.055) / 1.055, 2.4);
});
const clamp = (value: number, low = 0, high = 1) =>
  Math.min(high, Math.max(low, value));
const mix = (a: number, b: number, weight: number) => a + (b - a) * weight;
const rgb = (value: readonly number[]): V3 => [value[0], value[1], value[2]];
const mix3 = (a: V3, b: V3, weight: number): V3 => [
  mix(a[0], b[0], weight),
  mix(a[1], b[1], weight),
  mix(a[2], b[2], weight),
];
const multiply = (a: readonly number[], b: readonly number[]): V3 => [
  a[0] * b[0],
  a[1] * b[1],
  a[2] * b[2],
];
function finite(value: number, label: string): number {
  if (!Number.isFinite(value) || Math.abs(value) > MAX_SCALAR)
    throw new RangeError(`${label} must be finite and at most ${MAX_SCALAR}.`);
  return value;
}
function vector(value: readonly number[], count: number, label: string) {
  if (!value || value.length !== count)
    throw new RangeError(`${label} must contain ${count} values.`);
  for (let i = 0; i < count; i++) finite(value[i], label);
}
function textureURL(value: string) {
  if (typeof value !== "string" || value.length === 0 || value.length > 4096)
    throw new RangeError(
      "GI material map URLs must contain 1 to 4096 characters.",
    );
}

/** Matches normalized repeat addressing and sRGB GPU filtering at mip level zero. */
export function createGiTextureSampler(
  images: ReadonlyMap<string, GiTexture>,
): GiTextureSampler {
  if (images.size > MAX_IMAGES)
    throw new RangeError("The GI material image collection exceeds 2048 maps.");
  const admitted = new Map<string, GiTexture>();
  let texels = 0;
  for (const [url, image] of images) {
    textureURL(url);
    if (
      !Number.isInteger(image.width) ||
      !Number.isInteger(image.height) ||
      image.width < 1 ||
      image.height < 1 ||
      image.width > 4096 ||
      image.height > 4096 ||
      !(image.pixels instanceof Uint8Array) ||
      image.pixels.length !== image.width * image.height * 4
    )
      throw new RangeError(
        "GI material images require bounded RGBA8 dimensions.",
      );
    texels += image.width * image.height;
    if (texels > MAX_TEXELS)
      throw new RangeError(
        "GI material images exceed 256 MiB of RGBA8 pixels.",
      );
    admitted.set(url, {
      width: image.width,
      height: image.height,
      pixels: image.pixels,
    });
  }
  return (url, role, uv) => {
    vector(uv, 2, "GI material UV");
    if (role !== "color" && role !== "data")
      throw new RangeError("Unknown GI material texture role.");
    const image = admitted.get(url);
    if (!image) throw new Error(`GI material map was not prepared: ${url}`);
    const { width, height, pixels } = image;
    const x = (uv[0] - Math.floor(uv[0])) * width - 0.5;
    const y = (uv[1] - Math.floor(uv[1])) * height - 0.5;
    const ix = Math.floor(x),
      iy = Math.floor(y),
      fx = x - ix,
      fy = y - iy;
    const x0 = (ix + width) % width,
      x1 = (x0 + 1) % width,
      y0 = (iy + height) % height,
      y1 = (y0 + 1) % height;
    const indices = [
      (y0 * width + x0) * 4,
      (y0 * width + x1) * 4,
      (y1 * width + x0) * 4,
      (y1 * width + x1) * 4,
    ];
    const result: V4 = [0, 0, 0, 0];
    for (let c = 0; c < 4; c++) {
      const decode = (index: number) =>
        role === "color" && c < 3
          ? srgb[pixels[index + c]]
          : pixels[index + c] / 255;
      result[c] = mix(
        mix(decode(indices[0]), decode(indices[1]), fx),
        mix(decode(indices[2]), decode(indices[3]), fx),
        fy,
      );
    }
    return result;
  };
}

type Family = "generic" | "layers" | "paint" | "terrain" | "vegetation";
function family(material: CompiledMaterial): Family {
  if (!["OPAQUE", "MASK", "BLEND"].includes(material.alphaMode))
    throw new RangeError("Unknown GI material alpha mode.");
  if (material.packedChannels !== "metallic-r-roughness-g-occlusion-b")
    throw new RangeError("Unknown GI material packed-channel profile.");
  const profiles = [
    ["layers", "ue-mm-blend-2-v1"],
    ["paint", "ue-mm-blend-1-v1"],
    ["terrain", "ue-mm-blend-3-v1"],
    ["vegetation", "ue-mm-vegetation-1-v1"],
  ] as const;
  let selected: Family = "generic";
  // The fragment shader applies these profiles in this order.
  for (const [key, expected] of profiles) {
    if (material[key]) {
      if (material[key].model !== expected)
        throw new RangeError(`Unknown GI material ${key} profile.`);
      selected = key;
    }
  }
  return selected;
}
function excluded(material: CompiledMaterial) {
  return material.alphaMode === "BLEND" || !!material.glass || !!material.flame;
}
const roleFor = (space: "srgb" | "linear"): GiTextureRole => {
  if (space !== "srgb" && space !== "linear")
    throw new RangeError("Unknown GI material source color space.");
  return space === "srgb" ? "color" : "data";
};

/** Collects only maps used for diffuse color, metal rejection, or alpha tests. */
export function collectGiMaterialTextures(
  material: CompiledMaterial,
): { url: string; role: GiTextureRole }[] {
  const selected = family(material);
  if (excluded(material)) return [];
  const refs = new Map<string, { url: string; role: GiTextureRole }>();
  const add = (url: string | undefined, role: GiTextureRole) => {
    if (!url) return;
    textureURL(url);
    refs.set(`${role}\0${url}`, { url, role });
  };
  if (selected === "generic") {
    add(material.baseColorUrl, "color");
    add(material.mraUrl, "data");
  } else if (selected === "vegetation") {
    const v = material.vegetation!;
    add(v.baseColorUrl, "color");
    add(v.mraUrl, "data");
    if (v.mask.enabled) add(v.mask.url, roleFor(v.mask.sourceColorSpace));
  } else {
    const profile = material[selected]!;
    for (const slot of profile.slots) {
      add(slot.baseColorUrl, "color");
      add(slot.mraUrl, "data");
    }
    if (selected === "layers") {
      const blend = material.layers!.blend;
      if (blend.height.mode === "maskRed")
        add(blend.height.url, roleFor(blend.height.sourceColorSpace));
      add(
        blend.worldSubtract.url,
        roleFor(blend.worldSubtract.sourceColorSpace),
      );
    } else if (selected === "paint") {
      const p = material.paint!;
      add(p.height.url, roleFor(p.height.sourceColorSpace));
      add(p.overlay.url, roleFor(p.overlay.sourceColorSpace));
    } else {
      const t = material.terrain!;
      add(t.height.url, roleFor(t.height.sourceColorSpace));
      add(t.wetness.url, roleFor(t.wetness.sourceColorSpace));
    }
  }
  return [...refs.values()];
}

function map(
  sample: GiTextureSampler,
  url: string | undefined,
  role: GiTextureRole,
  uv: V2,
): V4 {
  if (!url) return role === "color" ? WHITE : PACKED_DEFAULT;
  const result = sample(url, role, uv);
  vector(result, 4, "GI material map sample");
  if (result.some((value) => value < 0 || value > 1))
    throw new RangeError("GI material map samples must be in [0, 1].");
  return result;
}

/** Alpha-only path for traversing cutout geometry without evaluating its color graph. */
export function evaluateGiCoverage(
  material: CompiledMaterial,
  ctx: GiMaterialContext,
  sample: GiTextureSampler,
): GiCoverage {
  const selected = family(material);
  const cutoff = finite(
    selected === "vegetation"
      ? material.vegetation!.alphaCutoff
      : material.alphaCutoff,
    "GI material alpha cutoff",
  );
  if (cutoff < 0 || cutoff > 1)
    throw new RangeError("GI material alpha cutoff must be in [0, 1].");
  if (excluded(material)) return { coverage: 0, masked: false, cutoff };
  if (material.alphaMode !== "MASK")
    return { coverage: 1, masked: false, cutoff };
  vector(ctx.uv, 2, "GI material UV");
  let coverage: number;
  if (selected === "vegetation") {
    coverage = map(
      sample,
      material.vegetation!.baseColorUrl,
      "color",
      ctx.uv,
    )[3];
  } else {
    vector(material.baseColorFactor, 4, "GI material base color factor");
    vector(ctx.vertexColor, 4, "GI vertex color");
    vector(material.uvScale, 2, "GI material UV scale");
    const uv: V2 = [
      ctx.uv[0] * material.uvScale[0],
      ctx.uv[1] * material.uvScale[1],
    ];
    // Layer albedo alpha is height data; those profiles bind a white generic map.
    const alpha =
      selected === "generic"
        ? map(sample, material.baseColorUrl, "color", uv)[3]
        : 1;
    coverage =
      alpha *
      material.baseColorFactor[3] *
      (material.vertexOpacity ? ctx.vertexColor[3] : 1);
  }
  return {
    coverage: clamp(finite(coverage, "GI material coverage")),
    masked: true,
    cutoff,
  };
}

function routedUV(
  profile: {
    uv: { mode: "mesh" | "objectXY" | "objectYZ"; scale: V2 };
  },
  ctx: GiMaterialContext,
): V2 {
  vector(profile.uv.scale, 2, "GI profile UV scale");
  const mode = profile.uv.mode;
  if (mode === "objectXY")
    return [
      ctx.uv[0] * Math.abs(ctx.objectScale[0]),
      ctx.uv[1] * Math.abs(ctx.objectScale[2]),
    ];
  if (mode === "objectYZ")
    return [
      ctx.uv[0] * Math.abs(ctx.objectScale[2]),
      ctx.uv[1] * Math.abs(ctx.objectScale[1]),
    ];
  if (mode !== "mesh") throw new RangeError("Unknown GI material UV routing.");
  return [ctx.uv[0] * profile.uv.scale[0], ctx.uv[1] * profile.uv.scale[1]];
}
function heightAlpha(height: number, phase: number, contrast: number) {
  finite(phase, "GI material blend phase");
  finite(contrast, "GI material height contrast");
  return clamp(clamp(height - 1 + 2 * phase) * (1 + 2 * contrast) - contrast);
}
function overlay(base: V3, blend: V3): V3 {
  return base.map((v, c) =>
    v > 0.5 ? 1 - 2 * (1 - v) * (1 - blend[c]) : 2 * v * blend[c],
  ) as V3;
}
function slot(
  value: { baseColorUrl: string; mraUrl: string; colorFactor: V4 },
  uv: V2,
  sample: GiTextureSampler,
) {
  vector(value.colorFactor, 4, "GI layer color factor");
  const color = map(sample, value.baseColorUrl, "color", uv);
  const packed = map(sample, value.mraUrl, "data", uv);
  return { color, base: multiply(color, value.colorFactor), packed };
}

/** Evaluates the owned source color graphs without camera-dependent lighting. */
export function evaluateGiMaterial(
  material: CompiledMaterial,
  ctx: GiMaterialContext,
  sample: GiTextureSampler,
): GiMaterialSample {
  const selected = family(material);
  const coverage = evaluateGiCoverage(material, ctx, sample);
  const approximations = [...GI_MATERIAL_LIMITATIONS];
  const doubleSided =
    selected === "vegetation"
      ? material.vegetation!.twoSided
      : material.doubleSided;
  if (excluded(material))
    return { ...coverage, reflectance: [0, 0, 0], doubleSided, approximations };
  vector(ctx.uv, 2, "GI material UV");
  vector(ctx.uv1, 2, "GI material UV1");
  vector(ctx.vertexColor, 4, "GI vertex color");
  vector(ctx.world, 3, "GI material world position");
  vector(ctx.geometricNormal, 3, "GI geometric normal");
  vector(ctx.objectScale, 3, "GI object scale");
  let base: V3;
  let metal: number;
  let dielectric = 0.04;
  if (selected === "layers") {
    const p = material.layers!;
    const uv = routedUV(p, ctx);
    const a = slot(p.slots[0], uv, sample),
      b = slot(p.slots[1], uv, sample);
    const w = p.blend.worldSubtract;
    const size = Math.max(
      Math.abs(finite(w.sizeMeters, "GI world mask size")),
      1e-8,
    );
    const pos = [
      -ctx.world[0] / size,
      -ctx.world[2] / size,
      -ctx.world[1] / size,
    ];
    const nLength = Math.hypot(...ctx.geometricNormal);
    if (nLength < 1e-12)
      throw new RangeError("GI geometric normal must be nonzero.");
    const mask = (uv: V2) =>
      map(sample, w.url, roleFor(w.sourceColorSpace), uv)[0];
    const alongX = clamp(3 * Math.abs(ctx.geometricNormal[0] / nLength) - 1);
    const alongY = clamp(3 * Math.abs(ctx.geometricNormal[1] / nLength) - 1);
    const worldMask = mix(
      mix(mask([pos[0], pos[2]]), mask([pos[1], pos[2]]), alongX),
      mask([pos[0], pos[1]]),
      alongY,
    );
    const power = finite(w.power, "GI world mask power");
    const powerBase =
      worldMask * finite(w.intensity, "GI world mask intensity");
    const subtract = clamp(
      power === 0
        ? 1
        : powerBase <= 0 && power > 0
          ? 0
          : Math.pow(Math.max(powerBase, 1e-8), power),
    );
    const h = p.blend.height;
    if (h.mode !== "layer2Alpha" && h.mode !== "maskRed")
      throw new RangeError("Unknown GI layer height mode.");
    const height = clamp(
      (h.mode === "layer2Alpha"
        ? (1 - b.color[3]) *
          finite(h.customIntensity, "GI layer height intensity")
        : map(sample, h.url, roleFor(h.sourceColorSpace), uv)[0]) - subtract,
    );
    const weight =
      heightAlpha(height, p.blend.phase, p.blend.contrast) *
      heightAlpha(height, ctx.vertexColor[0], p.blend.contrast);
    base = mix3(a.base, b.base, weight).map(
      (v) => v * finite(p.colorIntensity, "GI layer color intensity"),
    ) as V3;
    metal = mix(a.packed[0], b.packed[0], weight);
  } else if (selected === "paint") {
    const p = material.paint!;
    const uv = routedUV(p, ctx);
    vector(p.height.uvScale, 2, "GI paint height UV scale");
    const height = map(
      sample,
      p.height.url,
      roleFor(p.height.sourceColorSpace),
      [uv[0] * p.height.uvScale[0], uv[1] * p.height.uvScale[1]],
    )[0];
    const blue = heightAlpha(height, ctx.vertexColor[2], p.height.contrast);
    const green = heightAlpha(height, ctx.vertexColor[1], p.height.contrast);
    const a = slot(p.slots[0], uv, sample),
      b = slot(p.slots[1], uv, sample);
    base = mix3(a.base, b.base, blue);
    const paint = map(
      sample,
      p.overlay.url,
      roleFor(p.overlay.sourceColorSpace),
      ctx.uv1,
    );
    const gate = p.overlay.useAlpha ? paint[3] : 1;
    base = mix3(
      base,
      mix3(
        base,
        overlay(rgb(paint), base),
        finite(p.overlay.colorIntensity, "GI paint overlay intensity"),
      ),
      gate * green,
    );
    metal = mix(a.packed[0], b.packed[0], blue);
  } else if (selected === "terrain") {
    const p = material.terrain!;
    const uv = routedUV(p, ctx);
    const a = slot(p.slots[0], uv, sample),
      b = slot(p.slots[1], uv, sample),
      c = slot(p.slots[2], uv, sample);
    vector(p.height.uvScale, 2, "GI terrain height UV scale");
    const rawHeight = map(
      sample,
      p.height.url,
      roleFor(p.height.sourceColorSpace),
      [uv[0] * p.height.uvScale[0], uv[1] * p.height.uvScale[1]],
    )[0];
    const height = p.height.useCustom
      ? mix(
          rawHeight,
          1 - b.color[3],
          (1 - b.color[3]) *
            finite(
              p.height.customIntensity,
              "GI terrain custom height intensity",
            ),
        )
      : rawHeight;
    const blue = heightAlpha(height, ctx.vertexColor[2], p.height.contrast);
    const red = heightAlpha(rawHeight, ctx.vertexColor[0], p.height.contrast);
    const green = heightAlpha(height, ctx.vertexColor[1], p.height.contrast);
    const blueColor = mix3(a.base, b.base, blue);
    const redColor = mix3(c.base, blueColor, red);
    const wet = map(
      sample,
      p.wetness.url,
      roleFor(p.wetness.sourceColorSpace),
      uv,
    );
    const wetColor = mix3(
      rgb(wet),
      blueColor,
      a.packed[1] *
        finite(p.wetness.colorIntensity, "GI terrain wetness intensity"),
    );
    base = mix3(wetColor, redColor, green);
    metal = mix(c.packed[0], mix(a.packed[0], b.packed[0], blue), red);
  } else if (selected === "vegetation") {
    const p = material.vegetation!;
    const color = map(sample, p.baseColorUrl, "color", ctx.uv);
    const packed = map(sample, p.mraUrl, "data", ctx.uv);
    vector(p.tint, 3, "GI vegetation tint");
    vector(p.color.luminance, 3, "GI vegetation luminance");
    vector(p.color.overlayColor, 3, "GI vegetation overlay color");
    const tinted = multiply(color, p.tint);
    const luminance = tinted.reduce(
      (sum, v, c) => sum + v * p.color.luminance[c],
      0,
    );
    const desaturated = mix3(
      tinted,
      [luminance, luminance, luminance],
      finite(p.color.desaturation, "GI vegetation desaturation"),
    );
    const overlaid = overlay(desaturated, p.color.overlayColor).map((v) =>
      clamp(v),
    ) as V3;
    const adjusted = mix3(
      desaturated,
      overlaid,
      finite(p.color.overlayStrength, "GI vegetation overlay strength"),
    );
    const contrast = finite(p.color.contrast, "GI vegetation contrast");
    const intensity = finite(
      p.color.intensity,
      "GI vegetation color intensity",
    );
    const ao =
      packed[2] *
      finite(p.aoAffectsColorIntensity, "GI vegetation color AO intensity");
    if (p.gradient.axis !== "u" && p.gradient.axis !== "v")
      throw new RangeError("Unknown GI vegetation gradient axis.");
    const gradient =
      ctx.uv[p.gradient.axis === "v" ? 1 : 0] *
      finite(p.gradient.intensity, "GI vegetation gradient intensity");
    base = adjusted.map(
      (v) =>
        clamp(
          (contrast === 0 ? 1 : Math.pow(Math.max(v, 0), contrast)) *
            intensity *
            ao,
        ) *
        (1 - gradient),
    ) as V3;
    if (p.mask.enabled) {
      vector(p.mask.color, 3, "GI vegetation mask color");
      const mask = map(
        sample,
        p.mask.url,
        roleFor(p.mask.sourceColorSpace),
        ctx.uv,
      )[0];
      const masked = overlay(rgb(color), p.mask.color).map((v) =>
        clamp(v),
      ) as V3;
      base = mix3(base, masked, mask);
    }
    const specular =
      mix(0.15, 0.5, color[2] * ctx.vertexColor[1] ** 2) *
      finite(p.specularAmount, "GI vegetation specular amount");
    dielectric = clamp(specular) * 0.08;
    metal = packed[0];
  } else {
    vector(material.uvScale, 2, "GI material UV scale");
    vector(material.baseColorFactor, 4, "GI material base color factor");
    const uv: V2 = [
      ctx.uv[0] * material.uvScale[0],
      ctx.uv[1] * material.uvScale[1],
    ];
    base = multiply(
      map(sample, material.baseColorUrl, "color", uv),
      material.baseColorFactor,
    );
    metal =
      finite(material.metallic, "GI material metallic factor") *
      (material.mraUrl ? map(sample, material.mraUrl, "data", uv)[0] : 1);
  }
  finite(metal, "GI material metalness");
  vector(base, 3, "GI evaluated base color");
  const diffuse = (1 - clamp(metal)) * (1 - dielectric);
  const reflectance = base.map((value) => clamp(value) * diffuse) as V3;
  return { ...coverage, reflectance, doubleSided, approximations };
}
