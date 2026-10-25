import type { MaterialBundle, MaterialSource } from "./creator-material.ts";
import { ENVIRONMENT_RANGES } from "./protocol.ts";
import type { ProjectDocument } from "./project-document.ts";
/** Creator-owned material hook and project values; never the renderer source. */
export const DEFAULT_EFFECT = `fn oa_material(base:vec3<f32>, position:vec3<f32>, normal:vec3<f32>, uv:vec2<f32>, time:f32)->vec3<f32> {
  return base;
}`;
export type Authoring = {
  environment: Record<string, number>;
  materials: Record<string, Record<string, number>>;
  effect: string | null;
  materialSources?: Record<string, MaterialSource>;
  planetSource?: MaterialSource;
};
export type ProjectEdit =
  | { type: "move"; target: string; position: [number, number, number] }
  | { type: "material"; target: string; parameter: string; value: number }
  | {
      type: "place";
      asset: string;
      position: [number, number, number];
      scale?: number;
    }
  | { type: "remove"; target: string };
export type AuthorAction =
  | {
      type: "restore_project";
      document: ProjectDocument;
      checkpoint?: {
        eye: [number, number, number];
        target: [number, number, number];
        mode: "walk" | "fly";
      } | null;
    }
  | {
      type: "restore_material_project";
      document: ProjectDocument;
      checkpoint?: {
        eye: [number, number, number];
        target: [number, number, number];
        mode: "walk" | "fly";
      } | null;
      materials: MaterialBundle[];
    }
  | { type: "apply_material"; material: MaterialBundle }
  | { type: "propose"; operations: ProjectEdit[] }
  | { type: "draft"; asset: string; source: string }
  | { type: "compile"; asset: string | null; source_sha256?: string | null }
  | { type: "undo" | "redo" };
export type AuthorView = {
  schema: "openagents.verse.authoring.v1";
  revision: number;
  effect_scope: "world";
  can_undo: boolean;
  can_redo: boolean;
  draft: string;
  active_source: string;
  diagnostics: string[];
  materials: { id: string; objects: string[] }[];
  environment: Record<string, number>;
};
export function materialEffect(source: string, rendererSource: string): string {
  if (new TextEncoder().encode(source).length > 4096 || source.includes("\0"))
    throw new Error("Keep the material effect below 4 KiB.");
  // WGSL has no strings or preprocessing. Comments cannot hide executable tokens.
  const code = source
    .replace(/\/\*[\s\S]*?\*\//g, " ")
    .replace(/\/\/[^\n]*/g, " ");
  if (
    !/^\s*fn\s+oa_material\s*\(base\s*:\s*vec3<f32>,\s*position\s*:\s*vec3<f32>,\s*normal\s*:\s*vec3<f32>,\s*uv\s*:\s*vec2<f32>,\s*time\s*:\s*f32\)\s*->\s*vec3<f32>\s*\{[\s\S]*\}\s*$/.test(
      code,
    ) ||
    /@|\b(loop|for|while|discard|enable|override|const_assert|var\s*<)\b/.test(
      code,
    ) ||
    (code.match(/\bfn\b/g)?.length ?? 0) !== 1
  )
    throw new Error(
      "Keep the oa_material inputs and output; use a material function without loops or bindings.",
    );
  const allowed = new Set([
    "oa_material",
    "sin",
    "cos",
    "abs",
    "min",
    "max",
    "clamp",
    "mix",
    "smoothstep",
    "step",
    "fract",
    "floor",
    "ceil",
    "pow",
    "sqrt",
    "dot",
    "length",
    "normalize",
    "select",
    "f32",
    "if",
  ]);
  for (const call of code.matchAll(/\b([A-Za-z_]\w*)\s*\(/g))
    if (!allowed.has(call[1]))
      throw new Error(`The material function cannot call ${call[1]}.`);
  const marker = "var textureAlpha=";
  if (
    rendererSource.split(marker).length !== 2 ||
    !/struct Out\s*\{[^}]*\brawUV\s*:\s*vec2<f32>/.test(rendererSource)
  )
    throw new Error(
      "The renderer material interface changed. Keep the previous effect and reopen the current editor.",
    );
  return (
    rendererSource.replace(
      "var textureAlpha=",
      "base=clamp(oa_material(base,v.world,v.normal.xyz,v.rawUV,frame.eye.w),vec3(0.0),vec3(16.0));var textureAlpha=",
    ) +
    "\n" +
    source
  );
}
export const MATERIAL_PARAMETERS: Record<string, [number, number, number]> = {
  red: [0, 0, 1],
  green: [1, 0, 1],
  blue: [2, 0, 1],
  roughness: [8, 0.04, 1],
  metallic: [9, 0, 1],
  normal: [10, 0, 2],
};
export const ENVIRONMENT_PARAMETERS = ENVIRONMENT_RANGES;
export function checkedValue(
  parameter: string,
  value: number,
  bounds: Record<string, readonly number[]>,
) {
  const range = bounds[parameter];
  const [lo, hi] = range?.slice(-2) ?? [];
  if (!range || !Number.isFinite(value) || value < lo || value > hi)
    throw new Error(
      "Choose a supported material or lighting value within its range.",
    );
}
