/**
 * One storage scale for linear scene radiance. Camera exposure must divide this
 * out again; applying it to only some emitters changes their relative energy.
 */
export const SCENE_LINEAR_SCALE = 0.01;

/**
 * Owned fixed calibration hypotheses, not recovered vendor camera exposures.
 * Extended EV4 with an inherited +1 stop is one possible source interpretation;
 * the original project's exposure configuration remains unknown.
 */
export const OWNED_EXPOSURE = Object.freeze({ day: 0.125, night: 0.125 });

/** Procedural sky values have no source photometric units; this is an owned profile. */
export const GENERATED_SKY_RADIANCE_SCALE = 8;
