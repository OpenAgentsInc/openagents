/** The renderer's quality switches that follow Detail (ADR 0007). */
export interface RendererDetail {
  /** Drawing-buffer size over the viewport size given to `resize`. */
  renderScale: number;
  /** Camera travel, in meters, before the sun shadow cascades refit. */
  shadowRefreshMeters: number;
  /** Heading change (as a dot product) before the cascades refit. */
  shadowRefreshDot: number;
  /** Grass draw distance over the manifest's `maxDistance`. */
  foliageReach: number;
  /** Battle crowd LOD and draw distances over their configured values. */
  crowdReach: number;
}

/** Today's renderer: every value Detail 1 produces. */
export const FULL_RENDERER_DETAIL: Readonly<RendererDetail> = Object.freeze({
  renderScale: 1,
  shadowRefreshMeters: 2,
  shadowRefreshDot: 0.995,
  foliageReach: 1,
  crowdReach: 1,
});

const within = (v: number, lo: number, hi: number) =>
  typeof v === "number" && Number.isFinite(v) && v >= lo && v <= hi;

export function validateRendererDetail(value: RendererDetail): RendererDetail {
  if (
    !value ||
    !within(value.renderScale, 0.25, 1) ||
    !within(value.shadowRefreshMeters, 0.5, 32) ||
    !within(value.shadowRefreshDot, 0.9, 1) ||
    !within(value.foliageReach, 0.1, 1) ||
    !within(value.crowdReach, 0.1, 1)
  )
    throw new Error("Choose supported detail settings.");
  return value;
}

/**
 * Knob values to renderer settings. `shadow_quality` 1 keeps today's refit
 * thresholds; 0 refits after 8 m or about 10 degrees of turn.
 */
export function rendererDetailFromKnobs(
  values: readonly { knob: string; value: number }[],
): RendererDetail {
  const at = (knob: string) => values.find((v) => v.knob === knob)?.value ?? 1;
  const shadow = at("shadow_quality");
  return validateRendererDetail({
    renderScale: at("render_scale"),
    shadowRefreshMeters: 2 + 6 * (1 - shadow),
    shadowRefreshDot: 0.995 - 0.015 * (1 - shadow),
    foliageReach: at("foliage_density"),
    crowdReach: at("crowd_tier_distance"),
  });
}
