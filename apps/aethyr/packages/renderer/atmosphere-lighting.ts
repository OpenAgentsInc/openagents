/**
 * Owned RGB single-scattering quadrature; no engine implementation is copied.
 * Physical integrand and phase functions:
 * https://ebruneton.github.io/precomputed_atmospheric_scattering/atmosphere/functions.glsl.html
 * Coefficients use inverse meters, distances use meters, and phase uses 1/sr.
 * RGB lux input yields RGB photometric radiance proxies in cd/m², not a spectral solution.
 */
export const ATMOSPHERE_LIGHTING_PROFILE =
  "owned-rgb-single-scatter-atmosphere-v1";
export type AtmosphereVector = [number, number, number];
export interface AtmosphereProfile {
  planetRadiusMeters: number;
  atmosphereHeightMeters: number;
  rayleighScatteringPerMeter: AtmosphereVector;
  rayleighScaleHeightMeters: number;
  mieScatteringPerMeter: AtmosphereVector;
  mieAbsorptionPerMeter: AtmosphereVector;
  mieScaleHeightMeters: number;
  mieAnisotropy: number;
  absorptionPerMeter: AtmosphereVector;
  absorptionTent: {
    peakAltitudeMeters: number;
    halfWidthMeters: number;
    peakDensity: number;
  };
  skyLuminanceFactor: AtmosphereVector;
}
export interface AtmosphereSun {
  /** Canonical Y-up unit direction from the observer toward the light. */
  direction: AtmosphereVector;
  /** Normal illuminance at the top of the atmosphere; no source Skylight gain. */
  illuminanceLux: number;
  color: AtmosphereVector;
}
export interface AtmosphereQuadrature {
  observerAltitudeMeters?: number;
  viewSamples?: number;
  sunSamples?: number;
}
export interface AtmospherePanorama {
  width: number;
  height: number;
  pixels: Float32Array;
  statistics: {
    milliseconds: number;
    maximumRadiance: number;
    meanUpperHemisphereRadiance: AtmosphereVector;
    horizontalIlluminanceLux: AtmosphereVector;
    clippedChannels: number;
  };
}
const MAX_RADIANCE = 65504;
const dot = (a: AtmosphereVector, b: AtmosphereVector) =>
  a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const length = (v: AtmosphereVector) => Math.sqrt(dot(v, v));
function unit(v: AtmosphereVector): AtmosphereVector {
  if (!Array.isArray(v) || v.length !== 3 || v.some((x) => !Number.isFinite(x)))
    throw new Error("Use a finite three-component direction.");
  const n = length(v);
  if (n < 1e-12) throw new Error("Use a nonzero direction.");
  return v.map((x) => x / n) as AtmosphereVector;
}
function bounded(
  value: number,
  minimum: number,
  maximum: number,
  name: string,
): void {
  if (!Number.isFinite(value) || value < minimum || value > maximum)
    throw new Error(
      `${name} must be finite and between ${minimum} and ${maximum}.`,
    );
}
function rgb(value: AtmosphereVector, maximum: number, name: string): void {
  if (!Array.isArray(value) || value.length !== 3)
    throw new Error(`${name} needs three components.`);
  for (const x of value) bounded(x, 0, maximum, name);
}
export function validateAtmosphere(
  profile: AtmosphereProfile,
  sun?: AtmosphereSun,
): void {
  bounded(profile.planetRadiusMeters, 1_000, 1_000_000_000, "Planet radius");
  bounded(
    profile.atmosphereHeightMeters,
    1,
    profile.planetRadiusMeters,
    "Atmosphere height",
  );
  for (const key of [
    "rayleighScaleHeightMeters",
    "mieScaleHeightMeters",
  ] as const)
    bounded(profile[key], 1, 1_000_000, key);
  for (const key of [
    "rayleighScatteringPerMeter",
    "mieScatteringPerMeter",
    "mieAbsorptionPerMeter",
    "absorptionPerMeter",
  ] as const)
    rgb(profile[key], 1, key);
  bounded(profile.mieAnisotropy, -0.99, 0.99, "Mie anisotropy");
  bounded(
    profile.absorptionTent.peakAltitudeMeters,
    0,
    1_000_000,
    "Absorption peak altitude",
  );
  bounded(
    profile.absorptionTent.halfWidthMeters,
    1,
    1_000_000,
    "Absorption half width",
  );
  bounded(profile.absorptionTent.peakDensity, 0, 1, "Absorption peak density");
  rgb(profile.skyLuminanceFactor, 16, "Sky luminance factor");
  if (sun) {
    unit(sun.direction);
    bounded(sun.illuminanceLux, 0, 1_000_000, "Sun illuminance");
    rgb(sun.color, 16, "Sun color");
  }
}
function quadrature(profile: AtmosphereProfile, options: AtmosphereQuadrature) {
  const altitude = options.observerAltitudeMeters ?? 2;
  bounded(
    altitude,
    0,
    profile.atmosphereHeightMeters - 0.001,
    "Observer altitude",
  );
  const view = options.viewSamples ?? 64,
    sun = options.sunSamples ?? 32;
  for (const [value, name] of [
    [view, "View samples"],
    [sun, "Sun samples"],
  ] as const) {
    bounded(value, 4, 256, name);
    if (!Number.isInteger(value))
      throw new Error(`${name} must be an integer.`);
  }
  return { altitude, view, sun };
}
/** Both roots of |origin + t*direction|² = radius², using a stable quadratic root. */
function sphereRoots(
  origin: AtmosphereVector,
  direction: AtmosphereVector,
  radius: number,
): [number, number] | null {
  const b = dot(origin, direction),
    c = dot(origin, origin) - radius * radius;
  const discriminant = b * b - c;
  if (discriminant < 0) return null;
  const root = Math.sqrt(discriminant),
    q = -b - (b >= 0 ? root : -root);
  if (q === 0) return [0, 0];
  const a = q,
    other = c / q;
  return a < other ? [a, other] : [other, a];
}
function altitudeAt(
  profile: AtmosphereProfile,
  point: AtmosphereVector,
): number {
  const r = length(point),
    radius = profile.planetRadiusMeters;
  return (dot(point, point) - radius * radius) / (r + radius);
}
function density(profile: AtmosphereProfile, height: number): AtmosphereVector {
  if (height < -0.001 || height > profile.atmosphereHeightMeters + 0.001)
    return [0, 0, 0];
  const h = Math.max(0, height),
    tent = profile.absorptionTent;
  return [
    Math.exp(-h / profile.rayleighScaleHeightMeters),
    Math.exp(-h / profile.mieScaleHeightMeters),
    tent.peakDensity *
      Math.max(
        0,
        1 - Math.abs(h - tent.peakAltitudeMeters) / tent.halfWidthMeters,
      ),
  ];
}
function extinction(
  profile: AtmosphereProfile,
  d: AtmosphereVector,
): AtmosphereVector {
  return [0, 1, 2].map(
    (c) =>
      profile.rayleighScatteringPerMeter[c] * d[0] +
      (profile.mieScatteringPerMeter[c] + profile.mieAbsorptionPerMeter[c]) *
        d[1] +
      profile.absorptionPerMeter[c] * d[2],
  ) as AtmosphereVector;
}
function pointAt(
  origin: AtmosphereVector,
  direction: AtmosphereVector,
  t: number,
): AtmosphereVector {
  return [
    origin[0] + direction[0] * t,
    origin[1] + direction[1] * t,
    origin[2] + direction[2] * t,
  ];
}
function integrateDepth(
  profile: AtmosphereProfile,
  origin: AtmosphereVector,
  direction: AtmosphereVector,
  distance: number,
  samples: number,
): AtmosphereVector {
  const step = distance / samples,
    tau: AtmosphereVector = [0, 0, 0];
  for (let i = 0; i < samples; i++) {
    const e = extinction(
      profile,
      density(
        profile,
        altitudeAt(profile, pointAt(origin, direction, (i + 0.5) * step)),
      ),
    );
    for (let c = 0; c < 3; c++) tau[c] += e[c] * step;
  }
  return tau;
}
/** Dimensionless Beer–Lambert optical depth; useful for independent unit checks. */
export function atmosphereOpticalDepth(
  profile: AtmosphereProfile,
  origin: AtmosphereVector,
  direction: AtmosphereVector,
  distanceMeters: number,
  samples = 64,
): AtmosphereVector {
  validateAtmosphere(profile);
  rgb(
    origin.map(Math.abs) as AtmosphereVector,
    2_000_000_000,
    "Origin magnitude",
  );
  bounded(distanceMeters, 0, 2_000_000_000, "Optical path");
  bounded(samples, 4, 256, "Optical samples");
  if (!Number.isInteger(samples))
    throw new Error("Optical samples must be an integer.");
  return integrateDepth(
    profile,
    origin,
    unit(direction),
    distanceMeters,
    samples,
  );
}
export function rayleighPhase(cosine: number): number {
  const mu = Math.min(1, Math.max(-1, cosine));
  return (3 * (1 + mu * mu)) / (16 * Math.PI);
}
/** Normalized Cornette–Shanks aerosol phase; the finite g bound avoids a singular lobe. */
export function miePhase(cosine: number, anisotropy: number): number {
  bounded(anisotropy, -0.99, 0.99, "Mie anisotropy");
  const mu = Math.min(1, Math.max(-1, cosine)),
    g = anisotropy;
  return (
    (3 * (1 - g * g) * (1 + mu * mu)) /
    (8 * Math.PI * (2 + g * g) * (1 + g * g - 2 * g * mu) ** 1.5)
  );
}
function sunlightTransmittance(
  profile: AtmosphereProfile,
  point: AtmosphereVector,
  direction: AtmosphereVector,
  samples: number,
): AtmosphereVector {
  const ground = sphereRoots(point, direction, profile.planetRadiusMeters);
  if (
    (ground && ground[0] >= -1e-6) ||
    (altitudeAt(profile, point) < 1e-6 && dot(point, direction) < 0)
  )
    return [0, 0, 0];
  const top = sphereRoots(
    point,
    direction,
    profile.planetRadiusMeters + profile.atmosphereHeightMeters,
  );
  if (!top || top[1] <= 0) return [1, 1, 1];
  return integrateDepth(profile, point, direction, top[1], samples).map((tau) =>
    Math.exp(-tau),
  ) as AtmosphereVector;
}
function scatter(
  profile: AtmosphereProfile,
  sun: AtmosphereSun,
  direction: AtmosphereVector,
  settings: ReturnType<typeof quadrature>,
): AtmosphereVector {
  // This is a lighting-only map: the lower hemisphere and planet boundary emit nothing.
  if (direction[1] <= 0 || sun.illuminanceLux === 0) return [0, 0, 0];
  const origin: AtmosphereVector = [
    0,
    profile.planetRadiusMeters + settings.altitude,
    0,
  ];
  const top = sphereRoots(
    origin,
    direction,
    profile.planetRadiusMeters + profile.atmosphereHeightMeters,
  );
  if (!top || top[1] <= 0) return [0, 0, 0];
  const sunDirection = unit(sun.direction),
    mu = dot(direction, sunDirection);
  const pR = rayleighPhase(mu),
    pM = miePhase(mu, profile.mieAnisotropy);
  const step = top[1] / settings.view,
    tau: AtmosphereVector = [0, 0, 0],
    result: AtmosphereVector = [0, 0, 0];
  for (let i = 0; i < settings.view; i++) {
    const point = pointAt(origin, direction, (i + 0.5) * step);
    const d = density(profile, altitudeAt(profile, point)),
      e = extinction(profile, d);
    const tSun = sunlightTransmittance(
      profile,
      point,
      sunDirection,
      settings.sun,
    );
    for (let c = 0; c < 3; c++) {
      const source =
        profile.rayleighScatteringPerMeter[c] * d[0] * pR +
        profile.mieScatteringPerMeter[c] * d[1] * pM;
      // Integrate extinction analytically within each constant-density cell.
      const opticalCell = e[c] * step,
        lengthWeight = e[c] === 0 ? step : -Math.expm1(-opticalCell) / e[c];
      result[c] += Math.exp(-tau[c]) * tSun[c] * source * lengthWeight;
      tau[c] += opticalCell;
    }
  }
  return result.map(
    (x, c) =>
      x * sun.illuminanceLux * sun.color[c] * profile.skyLuminanceFactor[c],
  ) as AtmosphereVector;
}
export function singleScatteredRadiance(
  profile: AtmosphereProfile,
  sun: AtmosphereSun,
  direction: AtmosphereVector,
  options: AtmosphereQuadrature = {},
): AtmosphereVector {
  validateAtmosphere(profile, sun);
  return scatter(
    profile,
    sun,
    unit(direction),
    quadrature(profile, options),
  ).map((x) => Math.min(MAX_RADIANCE, x)) as AtmosphereVector;
}
/** The existing environment convention is u=atan2(x,z)/2π, v=acos(y)/π. */
export function renderAtmospherePanorama(
  profile: AtmosphereProfile,
  sun: AtmosphereSun,
  width = 128,
  height = 64,
  options: AtmosphereQuadrature = {},
): AtmospherePanorama {
  validateAtmosphere(profile, sun);
  if (
    !Number.isInteger(width) ||
    !Number.isInteger(height) ||
    width < 16 ||
    width > 256 ||
    height !== width / 2
  )
    throw new Error("Use a 2:1 atmosphere panorama from 16×8 to 256×128.");
  const settings = quadrature(profile, options);
  if (width * height * settings.view * settings.sun > 70_000_000)
    throw new Error(
      "The atmosphere quadrature exceeds 70 million sun-path samples.",
    );
  const started = performance.now(),
    pixels = new Float32Array(width * height * 4);
  const mean: AtmosphereVector = [0, 0, 0],
    horizontal: AtmosphereVector = [0, 0, 0];
  let weight = 0,
    maximumRadiance = 0,
    clippedChannels = 0;
  for (let y = 0; y < height; y++) {
    const phi = ((y + 0.5) * Math.PI) / height,
      sinPhi = Math.sin(phi),
      cosPhi = Math.cos(phi);
    const solidAngle =
      ((2 * Math.PI) / width) *
      (Math.cos((y * Math.PI) / height) -
        Math.cos(((y + 1) * Math.PI) / height));
    for (let x = 0; x < width; x++) {
      const theta = ((x + 0.5) * 2 * Math.PI) / width;
      const value = scatter(
        profile,
        sun,
        [sinPhi * Math.sin(theta), cosPhi, sinPhi * Math.cos(theta)],
        settings,
      );
      for (let c = 0; c < 3; c++) {
        const v = Math.min(MAX_RADIANCE, value[c]);
        if (!Number.isFinite(v) || v < 0)
          throw new Error("Atmosphere quadrature produced invalid radiance.");
        clippedChannels += Number(value[c] > MAX_RADIANCE);
        pixels[(y * width + x) * 4 + c] = v;
        maximumRadiance = Math.max(maximumRadiance, v);
        if (cosPhi > 0) {
          mean[c] += v * solidAngle;
          horizontal[c] += v * cosPhi * solidAngle;
        }
      }
      if (cosPhi > 0) weight += solidAngle;
      pixels[(y * width + x) * 4 + 3] = 1;
    }
  }
  return {
    width,
    height,
    pixels,
    statistics: {
      milliseconds: performance.now() - started,
      maximumRadiance,
      meanUpperHemisphereRadiance: mean.map(
        (x) => x / weight,
      ) as AtmosphereVector,
      horizontalIlluminanceLux: horizontal,
      clippedChannels,
    },
  };
}
