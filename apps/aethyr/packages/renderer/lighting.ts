import type { SourceLight } from "../assets/src/scene.ts";
import { colorTemperatureRec709 } from "./color-temperature.ts";
import { normalize, type V3 } from "./math.ts";
/** Three-band black-body approximation, normalized to the 6500 K white point. */
export function temperatureTint(kelvin: number): V3 {
  const temperature = Math.min(12000, Math.max(1000, kelvin));
  const values = [620e-9, 540e-9, 460e-9].map(
    (wavelength) =>
      Math.expm1(0.01438776877 / (wavelength * 6500)) /
      Math.expm1(0.01438776877 / (wavelength * temperature)),
  );
  const maximum = Math.max(...values);
  return values.map((value) => value / maximum) as V3;
}
/** Incident-light strength before the shared scene scale and camera exposure. */
export function lightIntensity(light: SourceLight): number {
  if (!Number.isFinite(light.intensity) || light.intensity < 0)
    throw new Error("Use a finite, nonnegative light intensity.");
  if (light.kind === "directional" || light.inverseSquaredFalloff === false)
    return light.intensity;
  if (light.intensityUnits === "candelas") return light.intensity;
  if (light.intensityUnits === "unitless") return light.intensity * 0.0016;
  if (light.intensityUnits === "lumens") {
    const angle = light.outerConeAngleRadians ?? Math.PI / 4;
    if (
      light.kind === "spot" &&
      (!Number.isFinite(angle) || angle <= 0 || angle > Math.PI)
    )
      throw new Error("Use a positive, finite spotlight cone angle.");
    return (
      light.intensity /
      (light.kind === "spot"
        ? 2 * Math.PI * (1 - Math.cos(angle))
        : 4 * Math.PI)
    );
  }
  throw new Error("This light intensity unit is not supported.");
}

/** Source-derived radial profiles; source sphere size needs separate BRDF support. */
export function lightAttenuation(
  light: SourceLight,
  distanceMeters: number,
): number {
  const range = light.range ?? 10;
  if (
    !Number.isFinite(distanceMeters) ||
    distanceMeters < 0 ||
    !Number.isFinite(range) ||
    range <= 0
  )
    throw new Error("Use a finite light distance and a positive range.");
  const normalizedSquared = (distanceMeters / range) ** 2;
  if (light.inverseSquaredFalloff === false) {
    const exponent = light.falloffExponent ?? 8;
    if (!Number.isFinite(exponent) || exponent < 0)
      throw new Error("Use a finite, nonnegative light falloff exponent.");
    return Math.max(1 - normalizedSquared, 0) ** Math.max(exponent, 0.0001);
  }
  const window = Math.max(1 - normalizedSquared ** 2, 0) ** 2;
  return window / (distanceMeters ** 2 + 0.0001);
}
export function lightColor(light: SourceLight): V3 {
  validateLightTemperatureModel(light);
  const tint = light.useTemperature
    ? light.temperatureModel === "cie-planckian-rec709-v1"
      ? colorTemperatureRec709(light.temperatureKelvin ?? 6500)
      : temperatureTint(light.temperatureKelvin ?? 6500)
    : [1, 1, 1];
  return light.color.map((value, i) => value * tint[i]) as V3;
}
/** Reject unknown prepared temperature models before interpreting light RGB. */
export function validateLightTemperatureModel(
  light: Pick<SourceLight, "temperatureModel">,
): void {
  if (
    light.temperatureModel !== undefined &&
    light.temperatureModel !== "cie-planckian-rec709-v1"
  )
    throw new Error("This light color temperature model is not supported.");
}
export function sunAngles(light: SourceLight): {
  sunAzimuth: number;
  sunElevation: number;
} {
  const direction = normalize(light.direction.map((n) => -n) as V3);
  return {
    sunAzimuth:
      ((Math.atan2(direction[0], direction[2]) * 180) / Math.PI + 360) % 360,
    sunElevation: (Math.asin(direction[1]) * 180) / Math.PI,
  };
}
