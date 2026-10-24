/** Owned photographic starting points, independently adjustable in the lighting panel. */
export interface ReferenceCameraLook {
  profile: "owned-reference-camera-look-v1";
  exposure: number;
  skyBackgroundIntensity: number;
}
const supported = new Set([
  "MT-02",
  "MT-03",
  "MT-05",
  "MT-06",
  "MT-07",
  "MT-08",
  "MT-09",
  "MT-10",
  "MT-11",
  "MT-12",
  "MT-13",
  "MT-14",
  "MT-15",
  "MT-16",
  "MT-17",
  "MT-18",
]);

/** These are comparison grades, not recovered source-camera exposure values. */
export function referenceCameraLook(
  id: string,
  mode: "day" | "night",
): ReferenceCameraLook | undefined {
  if (!supported.has(id)) return undefined;
  const exposure =
    mode === "night" ? 1 : id === "MT-09" || id === "MT-16" ? 4 : 3;
  return {
    profile: "owned-reference-camera-look-v1",
    exposure,
    // Fit clear daytime sky independently of surface exposure and lighting.
    skyBackgroundIntensity: (mode === "day" ? 0.28 : 1) / exposure,
  };
}
