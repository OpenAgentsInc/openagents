import { STOCK_MATERIAL_FEATURES } from "./material-specialization.ts";
import {
  rasterFrontFace,
  type RasterOrientation,
} from "./raster-orientation.ts";

export const STOCK_MATERIAL_FEATURE_MASK = 0xffff;
export const STOCK_SURFACE_SKY_FLAG = 0x40000;
export const STOCK_RASTER_MATERIAL_FLAGS = Object.freeze({
  masked: 0x10000,
  doubleSided: 0x20000,
});
export const STOCK_RASTER_FLAGS = Object.freeze({
  planarClip: 1,
  masked: 2,
  doubleSided: 4,
  hardwareCulled: 8,
});
export const STOCK_RASTER_DYNAMIC = 0xffffffff;

/** Pack the same alpha and facing facts used by the material uniform. */
export function stockRasterMaterialFlags(
  masked: boolean,
  doubleSided: boolean,
): number {
  if (typeof masked !== "boolean" || typeof doubleSided !== "boolean")
    throw new Error("Choose explicit mask and double-sided material flags.");
  return (
    (masked ? STOCK_RASTER_MATERIAL_FLAGS.masked : 0) |
    (doubleSided ? STOCK_RASTER_MATERIAL_FLAGS.doubleSided : 0)
  );
}

export interface StockRasterPolicy {
  rasterFlags: number;
  cullMode: "back" | "none";
  frontFace: "ccw" | "cw";
  depthWriteEnabled: boolean;
  alphaToCoverageEnabled: boolean;
}

/** Masked coverage and custom shaders can shade beyond the hard depth prepass. */
export function deriveStockRasterPolicy(
  features: number | undefined,
  blend: boolean,
  baseFront: "ccw" | "cw",
  orientation: RasterOrientation,
): StockRasterPolicy {
  const allowed =
    STOCK_MATERIAL_FEATURE_MASK |
    STOCK_RASTER_MATERIAL_FLAGS.masked |
    STOCK_RASTER_MATERIAL_FLAGS.doubleSided |
    STOCK_SURFACE_SKY_FLAG;
  if (
    (features !== undefined &&
      (!Number.isSafeInteger(features) ||
        features < 0 ||
        features > allowed ||
        (features & ~allowed) !== 0)) ||
    typeof blend !== "boolean" ||
    !["ccw", "cw"].includes(baseFront) ||
    !["positive", "negative", "degenerate"].includes(orientation)
  )
    throw new Error(
      "Choose complete stock material and source raster settings.",
    );
  const dynamic = features === undefined;
  const masked =
    !dynamic && (features & STOCK_RASTER_MATERIAL_FLAGS.masked) !== 0;
  const doubleSided =
    !dynamic && (features & STOCK_RASTER_MATERIAL_FLAGS.doubleSided) !== 0;
  const hardwareCulled =
    !dynamic && !doubleSided && orientation !== "degenerate";
  const planar = baseFront === "cw";
  return {
    rasterFlags: dynamic
      ? STOCK_RASTER_DYNAMIC
      : (planar ? STOCK_RASTER_FLAGS.planarClip : 0) |
        (masked ? STOCK_RASTER_FLAGS.masked : 0) |
        (doubleSided ? STOCK_RASTER_FLAGS.doubleSided : 0) |
        (hardwareCulled ? STOCK_RASTER_FLAGS.hardwareCulled : 0),
    cullMode: hardwareCulled ? "back" : "none",
    frontFace: hardwareCulled
      ? rasterFrontFace(baseFront, orientation)
      : baseFront,
    depthWriteEnabled: !blend && (dynamic || planar || masked),
    alphaToCoverageEnabled: !blend && (dynamic || masked),
  };
}

/** Reuse identical GPU descriptors across source orientations. */
export function stockRasterPipelineKey(
  features: number,
  blend: boolean,
  additive: boolean,
  baseFront: "ccw" | "cw",
  orientation: RasterOrientation,
): string {
  const policy = deriveStockRasterPolicy(
    features,
    blend,
    baseFront,
    orientation,
  );
  return `${features}:${Number(blend)}:${Number(additive)}:${policy.rasterFlags}:${policy.cullMode}:${policy.frontFace}`;
}

/** Opaque stock depth needs no alpha sample or fragment discard. */
export function deriveStockDepthPolicy(
  features: number | undefined,
  orientation: RasterOrientation,
): {
  fragmentRequired: boolean;
  cullMode: "back" | "none";
  frontFace: "ccw" | "cw";
} {
  const raster = deriveStockRasterPolicy(features, false, "ccw", orientation);
  const masked =
    features === undefined ||
    (features &
      (STOCK_RASTER_MATERIAL_FLAGS.masked |
        STOCK_MATERIAL_FEATURES.vegetation)) !==
      0;
  const fragmentRequired =
    masked ||
    (raster.cullMode === "none" &&
      (features! & STOCK_RASTER_MATERIAL_FLAGS.doubleSided) === 0);
  return fragmentRequired
    ? { fragmentRequired: true, cullMode: "none", frontFace: "ccw" }
    : {
        fragmentRequired: false,
        cullMode: raster.cullMode,
        frontFace: raster.frontFace,
      };
}
