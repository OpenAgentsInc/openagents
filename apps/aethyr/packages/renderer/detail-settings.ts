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
  /**
   * Projected height, in pixels, below which a battle fighter draws as a
   * silhouette. 8 at full detail, never above the 16-pixel readability floor.
   */
  crowdSilhouettePixels: number;
}

/** Today's renderer: every value Detail 1 produces. */
export const FULL_RENDERER_DETAIL: Readonly<RendererDetail> = Object.freeze({
  renderScale: 1,
  shadowRefreshMeters: 2,
  shadowRefreshDot: 0.995,
  foliageReach: 1,
  crowdReach: 1,
  crowdSilhouettePixels: 8,
});

/** `crowd_tier_distance`'s floor; the silhouette threshold reaches 16 px there. */
const CROWD_REACH_FLOOR = 0.4;

const within = (v: number, lo: number, hi: number) =>
  typeof v === "number" && Number.isFinite(v) && v >= lo && v <= hi;

export function validateRendererDetail(value: RendererDetail): RendererDetail {
  if (
    !value ||
    !within(value.renderScale, 0.25, 1) ||
    !within(value.shadowRefreshMeters, 0.5, 32) ||
    !within(value.shadowRefreshDot, 0.9, 1) ||
    !within(value.foliageReach, 0.1, 1) ||
    !within(value.crowdReach, 0.1, 1) ||
    !within(value.crowdSilhouettePixels, 8, 16)
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
  const crowd = at("crowd_tier_distance");
  // Linear from 8 px at full reach to 16 px at the knob's floor, clamped so a
  // reach below the floor never pushes larger fighters into silhouettes.
  const silhouette = Math.min(
    1,
    Math.max(0, (1 - crowd) / (1 - CROWD_REACH_FLOOR)),
  );
  return validateRendererDetail({
    renderScale: at("render_scale"),
    shadowRefreshMeters: 2 + 6 * (1 - shadow),
    shadowRefreshDot: 0.995 - 0.015 * (1 - shadow),
    foliageReach: at("foliage_density"),
    crowdReach: crowd,
    crowdSilhouettePixels: 8 + 8 * silhouette,
  });
}
