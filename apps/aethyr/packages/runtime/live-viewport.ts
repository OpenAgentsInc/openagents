/** Layout can briefly report zero while an embedded player is hidden. */
export function liveViewport(
  width: number,
  height: number,
): { width: number; height: number } | undefined {
  for (const [name, value] of [
    ["width", width],
    ["height", height],
  ] as const) {
    if (!Number.isInteger(value) || value < 0 || value > 16384)
      throw new Error(`Use viewport ${name} between 1 and 16384.`);
  }
  // Keep the last/default positive size so loading never waits for visibility.
  if (width === 0 || height === 0) return undefined;
  return { width, height };
}
