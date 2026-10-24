import type { CompiledMaterial } from "../assets/src/scene.ts";

export const GRASS_GROUND_PROFILE = "quick-grass-v1";
export const GRASS_GROUND_GREEN = [0.05, 0.2, 0.01] as const;

/** The second existing material-tail lane enables the fixed ground profile. */
export function grassGroundMarker(
  material: Pick<CompiledMaterial, "grassGround">,
): number {
  const value = material.grassGround;
  if (value === undefined) return 0;
  if (
    !value ||
    typeof value !== "object" ||
    Array.isArray(value) ||
    Object.keys(value).length !== 1 ||
    value.profile !== GRASS_GROUND_PROFILE
  )
    throw new Error("Choose a supported grass ground profile.");
  return 1;
}

const smoothstep = (low: number, high: number, value: number) => {
  const t = Math.max(0, Math.min(1, (value - low) / (high - low)));
  return t * t * (3 - 2 * t);
};

/** Preserve green ground beyond the blades without a bright camera-relative ring. */
export function grassGroundBase(distance: number): [number, number, number] {
  if (!Number.isFinite(distance) || distance < 0)
    throw new Error("Choose a finite ground distance.");
  // Distant blades expose more soil, but the ground retains the same green hue.
  const ao = 0.25 + 0.2 * smoothstep(25, 100, distance);
  return GRASS_GROUND_GREEN.map((value) => value * ao) as [
    number,
    number,
    number,
  ];
}

// Adapted from Quick_Grass's MIT terrain shader; its license is retained in grass-shader.ts.
export const WGSL_GRASS_GROUND = `
fn quickGrassGroundBase(world:vec3<f32>)->vec3<f32>{
 let surfaceDistance=distance(frame.eye.xyz,world);
 let soilOcclusion=mix(0.25,0.45,smoothstep(25.0,100.0,surfaceDistance));
 return vec3(0.05,0.2,0.01)*soilOcclusion;
}
`;
