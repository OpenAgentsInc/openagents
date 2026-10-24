/** Three r124's source light colors are linear numeric RGB, with legacy light units. */
export const MMORPG_SUN_DIRECTION = [-50, 200, -10].map(
  (x) => x / Math.hypot(50, 200, 10),
) as [number, number, number];
export const MMORPG_SUN_COLOR = [128 / 255, 136 / 255, 179 / 255] as const;
export const MMORPG_HEMISPHERE_SKY = [66 / 255, 74 / 255, 117 / 255] as const;
export const MMORPG_HEMISPHERE_GROUND = [
  106 / 255,
  136 / 255,
  181 / 255,
] as const;
export const MMORPG_LIGHT_INTENSITY = 0.7;
export function mmorpgHemisphereSH() {
  const result = new Float32Array(36);
  for (let i = 0; i < 3; i++) {
    result[i] =
      ((MMORPG_HEMISPHERE_SKY[i] + MMORPG_HEMISPHERE_GROUND[i]) *
        0.5 *
        MMORPG_LIGHT_INTENSITY) /
      0.2820947918;
    result[4 + i] =
      ((MMORPG_HEMISPHERE_SKY[i] - MMORPG_HEMISPHERE_GROUND[i]) *
        0.5 *
        MMORPG_LIGHT_INTENSITY) /
      0.4886025119;
  }
  return result;
}
