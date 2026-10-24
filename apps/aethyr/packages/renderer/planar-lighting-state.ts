/** Private renderer detector; inputs use the existing fixed frame layout. */
export function planarLightingChanged(
  previous: Float32Array,
  uniform: Float32Array,
): boolean {
  for (let i = 0; i < 556; i++) {
    const next = uniform[i + 52],
      old = previous[i];
    // String(number) equates signed zero and every NaN payload.
    if (next !== old && (next === next || old === old)) {
      for (let j = 0; j < 556; j++) previous[j] = uniform[j + 52];
      return true;
    }
  }
  return false;
}
