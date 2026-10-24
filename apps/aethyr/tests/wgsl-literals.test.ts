import assert from "node:assert/strict";
import test from "node:test";
import { compactGeneratedWGSL } from "../packages/renderer/wgsl-source.ts";
import { WGSL_FILMIC } from "../packages/renderer/filmic.ts";
import { WGSL_DISPLAY_LUT } from "../packages/renderer/display-lut.ts";
import { WGSL_VIGNETTE } from "../packages/renderer/vignette.ts";
import { WGSL_WATER_TURBULENCE } from "../packages/renderer/water-turbulence.ts";
import { WGSL_WATER_AIR_SCATTERING } from "../packages/renderer/water-air-scattering.ts";
import { FOG_SEGMENT_WGSL } from "../packages/renderer/fog.ts";
import { WGSL_TERRAIN_FUNCTIONS } from "../packages/renderer/terrain.ts";
import { WGSL_LAYER_FUNCTIONS } from "../packages/renderer/layers.ts";
import { WGSL_PAINT_FUNCTIONS } from "../packages/renderer/paint.ts";
import { WGSL_GLASS_FUNCTIONS } from "../packages/renderer/glass.ts";

test("owned literal shaders are compact before JavaScript delivery", () => {
  for (const [name, source] of Object.entries({
    WGSL_FILMIC,
    WGSL_DISPLAY_LUT,
    WGSL_VIGNETTE,
    WGSL_WATER_TURBULENCE,
    WGSL_WATER_AIR_SCATTERING,
    FOG_SEGMENT_WGSL,
    WGSL_TERRAIN_FUNCTIONS,
    WGSL_LAYER_FUNCTIONS,
    WGSL_PAINT_FUNCTIONS,
    WGSL_GLASS_FUNCTIONS,
  }))
    assert.equal(source, compactGeneratedWGSL(source), name);
});
