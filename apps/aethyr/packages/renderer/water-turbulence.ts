import {
  validateWaterAirModel,
  type WaterAirModel,
} from "./water-air-scattering.ts";
import { MAX_WORLD_ANIMATION_TIME } from "../runtime/world-clock.ts";
/** Owned periodic surface model. Values are design choices, not source-system recovery. */
export interface WaterTurbulence {
  geometryAmplitudeMeters: number;
  microAmplitudeMeters: number;
  airScatteringPerMeter: number;
  airAlbedo: number;
  airModel?: WaterAirModel;
}
export const OWNED_WATER_TURBULENCE: Readonly<WaterTurbulence> = Object.freeze({
  geometryAmplitudeMeters: 0.004,
  microAmplitudeMeters: 0.0015,
  airScatteringPerMeter: 20,
  airAlbedo: 0.85,
});
export function validateWaterTurbulence(p: WaterTurbulence) {
  for (const [value, maximum] of [
    [p.geometryAmplitudeMeters, 0.015],
    [p.microAmplitudeMeters, 0.005],
    [p.airScatteringPerMeter, 200],
    [p.airAlbedo, 1],
  ])
    if (!Number.isFinite(value) || value < 0 || value > maximum)
      throw Error("Use bounded water surface and air-scattering settings.");
  if (p.airModel !== undefined) validateWaterAirModel(p.airModel);
  return p;
}
// Integer temporal frequencies make the eight-second wrap continuous in value and derivative.
// Geometry is the long wave; the two shorter waves model unresolved microstructure only.
const WAVES = [
  {
    direction: [0.7, 0.2, 0.68556546],
    wavelength: 0.18,
    weight: 1,
    cycles: 7,
    phase: 0.3,
  },
  {
    direction: [-0.3, 0.9, 0.31622777],
    wavelength: 0.035,
    weight: 0.72,
    cycles: 11,
    phase: 1.7,
  },
  {
    direction: [0.8, 0.4, -0.4472136],
    wavelength: 0.017,
    weight: 0.28,
    cycles: 17,
    phase: 4.1,
  },
] as const;
/** Analytic world-space height and its gradient, independent of call/capture order. */
export function waterSurfaceField(
  position: readonly number[],
  time: number,
  amplitude: number,
  micro = false,
) {
  if (
    position.length !== 3 ||
    position.some((x) => !Number.isFinite(x) || Math.abs(x) > 1e6) ||
    !Number.isFinite(time) ||
    time < 0 ||
    time > MAX_WORLD_ANIMATION_TIME ||
    !Number.isFinite(amplitude) ||
    amplitude < 0 ||
    amplitude > 0.015
  )
    throw Error("Use finite water coordinates, age and displacement.");
  const gradient = [0, 0, 0];
  let height = 0;
  for (const wave of micro ? WAVES.slice(1) : WAVES.slice(0, 1)) {
    const k = wave.direction.map((x) => (x * 2 * Math.PI) / wave.wavelength),
      phase =
        k.reduce((s, x, i) => s + x * position[i], wave.phase) -
        (2 * Math.PI * wave.cycles * (time % 8)) / 8,
      a = amplitude * wave.weight;
    height += Math.sin(phase) * a;
    for (let i = 0; i < 3; i++) gradient[i] += Math.cos(phase) * a * k[i];
  }
  return { height, gradient };
}
/** Extinction gives a bounded scattering fraction; it is not an emission or alpha gain. */
export function waterAirFraction(
  pathMeters: number,
  coverage: number,
  scatteringPerMeter: number,
) {
  if (
    !Number.isFinite(pathMeters) ||
    pathMeters < 0 ||
    pathMeters > 1 ||
    !Number.isFinite(coverage) ||
    coverage < 0 ||
    coverage > 1 ||
    !Number.isFinite(scatteringPerMeter) ||
    scatteringPerMeter < 0 ||
    scatteringPerMeter > 200
  )
    throw Error("Use bounded air coverage and optical depth.");
  return -Math.expm1(-pathMeters * coverage * scatteringPerMeter);
}
export const WGSL_WATER_TURBULENCE = `fn waterWave(position:vec3<f32>,time:f32,amplitude:f32,direction:vec3<f32>,wavelength:f32,cycles:f32,phase:f32)->vec4<f32>{let k=direction*(6.283185307/wavelength);let angle=dot(k,position)+phase-6.283185307*cycles*(time%8.0)/8.0;return vec4(sin(angle)*amplitude,cos(angle)*amplitude*k);}fn waterGeometryField(position:vec3<f32>,time:f32,amplitude:f32)->vec4<f32>{return waterWave(position,time,amplitude,vec3(.7,.2,.68556546),.18,7.0,.3);}fn waterMicroField(position:vec3<f32>,time:f32,amplitude:f32)->vec4<f32>{return waterWave(position,time,amplitude*.72,vec3(-.3,.9,.31622777),.035,11.0,1.7)+waterWave(position,time,amplitude*.28,vec3(.8,.4,-.44721360),.017,17.0,4.1);}fn waterAirFraction(path:f32,coverage:f32,scattering:f32)->f32{return 1.0-exp(-path*coverage*scattering);}`;
