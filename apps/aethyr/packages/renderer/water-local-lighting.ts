import type { SourceLight } from "../assets/src/scene.ts";
import { lightAttenuation, lightColor, lightIntensity } from "./lighting.ts";
import { SCENE_LINEAR_SCALE } from "./radiometry.ts";
export const WATER_LOCAL_LIGHT_LIMIT = 32;
/** Bounded selection for this small transparent slice; local shadowing remains unsupported. */
export function packWaterLocalLights(
  lights: readonly SourceLight[],
  anchors: readonly (readonly number[])[],
) {
  if (
    !Array.isArray(lights) ||
    lights.length > 1024 ||
    !anchors.length ||
    anchors.length > 64 ||
    anchors.some(
      (p) =>
        p.length !== 3 ||
        p.some((v) => !Number.isFinite(v) || Math.abs(v) > 1e6),
    )
  )
    throw Error("Water highlights need bounded lights and world positions.");
  const candidates = (lights as readonly SourceLight[])
    .filter((light) => light.enabled && light.kind !== "directional")
    .map((light) => {
      if (
        (light.kind !== "point" && light.kind !== "spot") ||
        !light.position ||
        light.position.length !== 3 ||
        light.position.some((v) => !Number.isFinite(v) || Math.abs(v) > 1e6) ||
        !Number.isFinite(light.range ?? 10) ||
        (light.range ?? 10) <= 0 ||
        (light.range ?? 10) > 1e5 ||
        !light.color ||
        light.color.length !== 3 ||
        light.color.some((v) => !Number.isFinite(v) || v < 0 || v > 1000)
      )
        throw Error("Water highlights need supported point or spot lights.");
      const intensity = lightIntensity(light),
        color = lightColor(light);
      if (
        !Number.isFinite(intensity) ||
        intensity > 1e9 ||
        color.some((v) => !Number.isFinite(v))
      )
        throw Error("Water highlight strength is out of range.");
      let distance = Infinity;
      for (let i = 0; i < anchors.length; i++) {
        const p = anchors[i];
        distance = Math.min(
          distance,
          Math.hypot(
            p[0] - light.position[0],
            p[1] - light.position[1],
            p[2] - light.position[2],
          ),
        );
      }
      return {
        light,
        intensity,
        color,
        score:
          lightAttenuation(light, distance) *
          intensity *
          Math.max(color[0], color[1], color[2]),
      };
    })
    .filter((x) => x.score > 0)
    .sort((a, b) => b.score - a.score || a.light.id.localeCompare(b.light.id))
    .slice(0, 32);
  const positions = new Float32Array(128),
    colors = new Float32Array(128),
    directions = new Float32Array(128),
    cones = new Float32Array(128);
  for (let i = 0; i < candidates.length; i++) {
    const { light, intensity, color } = candidates[i],
      at = i * 4;
    positions[at] = light.position[0];
    positions[at + 1] = light.position[1];
    positions[at + 2] = light.position[2];
    positions[at + 3] = light.range ?? 10;
    colors[at] = color[0] * intensity * SCENE_LINEAR_SCALE;
    colors[at + 1] = color[1] * intensity * SCENE_LINEAR_SCALE;
    colors[at + 2] = color[2] * intensity * SCENE_LINEAR_SCALE;
    if (light.kind === "spot") {
      const d = light.direction,
        length = Math.hypot(d[0], d[1], d[2]),
        inner = light.innerConeAngleRadians ?? 0,
        outer = light.outerConeAngleRadians ?? Math.PI / 4;
      if (
        d.length !== 3 ||
        d.some((v) => !Number.isFinite(v)) ||
        length < 1e-6 ||
        inner < 0 ||
        outer < inner ||
        outer > Math.PI / 2 ||
        !Number.isFinite(inner) ||
        !Number.isFinite(outer)
      )
        throw Error("Water highlights need bounded spotlight cones.");
      directions[at] = d[0] / length;
      directions[at + 1] = d[1] / length;
      directions[at + 2] = d[2] / length;
      directions[at + 3] = 1;
      cones[at] = Math.cos(inner);
      cones[at + 1] = Math.cos(outer);
    }
    const exponent =
      light.inverseSquaredFalloff === false ? (light.falloffExponent ?? 8) : -1;
    if (!Number.isFinite(exponent) || exponent > 1024)
      throw Error("Water highlight falloff is out of range.");
    cones[at + 3] = exponent;
  }
  return {
    count: candidates.length,
    ids: candidates.map((x) => x.light.id),
    positions,
    colors,
    directions,
    cones,
  };
}
