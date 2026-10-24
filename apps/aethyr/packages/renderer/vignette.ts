import type {
  SourceCamera,
  SourceVignetteOverride,
} from "../assets/src/scene.ts";

/** Retain explicit camera settings without activating inherited values. */
export function selectedCameraVignette(
  camera: Pick<SourceCamera, "vignetteOverride"> | undefined,
): SourceVignetteOverride | undefined {
  const source = camera?.vignetteOverride;
  if (!source || source.shape !== "cosine-fourth") return undefined;
  if (
    !Number.isFinite(source.intensity) ||
    source.intensity < 0 ||
    source.intensity > 1 ||
    !Number.isFinite(source.blendWeight) ||
    source.blendWeight < 0 ||
    source.blendWeight > 1
  )
    throw new RangeError(
      "Camera vignette settings must be between zero and one",
    );
  return { ...source };
}

/** Only complete source-camera overrides have a qualified inherited-state boundary. */
export function cameraVignetteIntensity(
  source: SourceVignetteOverride | undefined,
): number {
  const selected = selectedCameraVignette({ vignetteOverride: source });
  return selected?.blendWeight === 1 ? selected.intensity : 0;
}

/** Cosine-fourth lens falloff in a sensor plane normalized by its diagonal. */
export function vignetteWeight(
  uv: readonly [number, number],
  aspect: number,
  intensity: number,
): number {
  if (
    uv.length !== 2 ||
    !uv.every(Number.isFinite) ||
    !Number.isFinite(aspect) ||
    aspect <= 0 ||
    !Number.isFinite(intensity) ||
    intensity < 0 ||
    intensity > 1
  )
    throw new RangeError(
      "Use finite lens coordinates, a positive aspect, and intensity from zero to one",
    );
  const x = 2 * uv[0] - 1,
    y = 2 * uv[1] - 1;
  const r2 = (2 * (aspect * aspect * x * x + y * y)) / (1 + aspect * aspect);
  const d = 1 + intensity * intensity * r2;
  return 1 / (d * d);
}

export const WGSL_VIGNETTE = `fn vignetteWeight(uv:vec2<f32>,aspect:f32,intensity:f32)->f32{let p=2.0*uv-vec2(1.0);let a2=aspect*aspect;let r2=2.0*(a2*p.x*p.x+p.y*p.y)/(1.0+a2);let d=1.0+intensity*intensity*r2;return 1.0/(d*d);}`;
