import type { CompiledScene } from "../assets/src/scene.ts";
import {
  FOG_HEIGHT_RATE,
  FOG_REFERENCE_HEIGHT,
  FOG_START_DISTANCE,
  segmentFogOpticalDepth,
} from "./fog.ts";

export interface SkyDome {
  center: [number, number, number];
  radiusMeters: number;
}
export const SKY_DOME_LIMITS = Object.freeze({
  maxRadius: 100_000,
  maxCenter: 1_000_000,
  maxDistance: 100_000,
  maxIndices: 1_000_000,
  sphereExtentTolerance: 0.01,
  uniformScaleTolerance: 0.0001,
});

/** Recover a bounded spherical approximation only for a qualified source sphere. */
export function sourceSkyDome(
  scene: Pick<CompiledScene, "materials" | "meshes" | "instances">,
  materialId: string,
  geometry: { vertices: Float32Array; indices: Uint32Array },
): SkyDome | undefined {
  const selected = scene.instances.filter((instance) => {
    const mesh = scene.meshes[instance.mesh];
    if (!mesh)
      throw new Error("Prepare the source sky mesh before opening it.");
    return mesh.primitives.some((primitive) => {
      const override = instance.materialOverrides[primitive.materialSlot];
      return (
        scene.materials[override >= 0 ? override : primitive.material]?.id ===
        materialId
      );
    });
  });
  if (!selected.length) return undefined;
  if (selected.length !== 1)
    throw new Error("Choose one source skydome for the visible environment.");
  const instance = selected[0],
    mesh = scene.meshes[instance.mesh],
    m = instance.model;
  const { min, max } = mesh.bounds;
  if (
    m.length !== 16 ||
    !m.every(Number.isFinite) ||
    Math.abs(m[3]) + Math.abs(m[7]) + Math.abs(m[11]) > 1e-10 ||
    Math.abs(m[15] - 1) > 1e-10 ||
    min.length !== 3 ||
    max.length !== 3 ||
    !min.every(Number.isFinite) ||
    !max.every(Number.isFinite)
  )
    throw new Error("Prepare finite source skydome bounds and placement.");
  const radii = max.map((v, i) => (v - min[i]) * 0.5);
  const radius = Math.max(...radii),
    scale = [0, 4, 8].map((i) => Math.hypot(m[i], m[i + 1], m[i + 2]));
  const largestScale = Math.max(...scale);
  if (
    radius <= 0 ||
    radii.some(
      (v) =>
        v <= 0 ||
        Math.abs(v - radius) > radius * SKY_DOME_LIMITS.sphereExtentTolerance,
    ) ||
    largestScale <= 0 ||
    scale.some(
      (v) =>
        Math.abs(v - largestScale) >
        largestScale * SKY_DOME_LIMITS.uniformScaleTolerance,
    ) ||
    [
      [0, 4],
      [0, 8],
      [4, 8],
    ].some(
      ([a, b]) =>
        Math.abs(m[a] * m[b] + m[a + 1] * m[b + 1] + m[a + 2] * m[b + 2]) >
        largestScale * largestScale * SKY_DOME_LIMITS.uniformScaleTolerance,
    )
  )
    throw new Error(
      "The source skydome needs a spherical mesh and uniform placement scale.",
    );
  const local = min.map((v, i) => (v + max[i]) * 0.5);
  let count = 0;
  for (const primitive of mesh.primitives) {
    if (
      ![primitive.firstIndex, primitive.indexCount, primitive.baseVertex].every(
        Number.isInteger,
      ) ||
      primitive.firstIndex < 0 ||
      primitive.indexCount <= 0 ||
      primitive.indexCount % 3 !== 0 ||
      primitive.baseVertex < 0 ||
      primitive.firstIndex + primitive.indexCount > geometry.indices.length ||
      (count += primitive.indexCount) > SKY_DOME_LIMITS.maxIndices
    )
      throw new Error(
        "Prepare bounded triangle geometry for the source skydome.",
      );
    for (
      let triangle = primitive.firstIndex;
      triangle < primitive.firstIndex + primitive.indexCount;
      triangle += 3
    ) {
      const centroid = [0, 0, 0];
      for (let corner = 0; corner < 3; corner++) {
        const offset =
          (geometry.indices[triangle + corner] + primitive.baseVertex) * 12;
        const position = [
          geometry.vertices[offset],
          geometry.vertices[offset + 1],
          geometry.vertices[offset + 2],
        ];
        if (
          !position.every(Number.isFinite) ||
          Math.abs(
            Math.hypot(...position.map((v, i) => v - local[i])) - radius,
          ) >
            radius * SKY_DOME_LIMITS.sphereExtentTolerance
        )
          throw new Error(
            "The source sky vertices do not fit the skydome sphere.",
          );
        position.forEach((v, i) => {
          centroid[i] += (v - local[i]) / 3;
        });
      }
      if (
        Math.hypot(...centroid) <
        radius * (1 - SKY_DOME_LIMITS.sphereExtentTolerance)
      )
        throw new Error(
          "The source skydome triangles need a closer spherical fit.",
        );
    }
  }
  const center: [number, number, number] = [0, 1, 2].map(
    (i) =>
      m[i] * local[0] + m[4 + i] * local[1] + m[8 + i] * local[2] + m[12 + i],
  ) as [number, number, number];
  const dome = { center, radiusMeters: radius * largestScale };
  checkedDome(dome);
  return dome;
}

function vector(value: readonly number[], name: string): void {
  if (value.length !== 3 || !value.every(Number.isFinite))
    throw new Error(`Use a finite ${name} vector.`);
}
function checkedDome(dome: SkyDome): void {
  vector(dome.center, "skydome center");
  if (
    dome.center.some((v) => Math.abs(v) > SKY_DOME_LIMITS.maxCenter) ||
    !Number.isFinite(dome.radiusMeters) ||
    dome.radiusMeters <= 0 ||
    dome.radiusMeters > SKY_DOME_LIMITS.maxRadius
  )
    throw new Error("Keep the source skydome within supported world bounds.");
}

/** Nearest positive ray intersection; inside observers reach the far sphere surface. */
export function skyDomeDistance(
  dome: SkyDome,
  eye: readonly number[],
  direction: readonly number[],
): number {
  checkedDome(dome);
  vector(eye, "eye");
  vector(direction, "sky direction");
  if (eye.some((v) => Math.abs(v) > SKY_DOME_LIMITS.maxCenter))
    throw new Error("Keep the sky observer within supported world bounds.");
  const length = Math.hypot(...direction);
  if (length < 1e-12) throw new Error("Choose a nonzero sky direction.");
  const ray = direction.map((v) => v / length),
    offset = eye.map((v, i) => v - dome.center[i]);
  const b = offset.reduce((v, n, i) => v + n * ray[i], 0),
    c = offset.reduce((v, n) => v + n * n, 0) - dome.radiusMeters ** 2,
    discriminant = b * b - c;
  if (discriminant < 0) return 0;
  const root = Math.sqrt(discriminant),
    near = -b - root,
    far = -b + root;
  return Math.min(
    SKY_DOME_LIMITS.maxDistance,
    near > 1e-6 ? near : Math.max(0, far),
  );
}

/**
 * Analytic extinction for the selected segment, without any in-scattering.
 * A later source volume composition can set startDistance to its far boundary;
 * near-volume extinction and light must then be composed separately, once.
 */
export function skyFogTransmittance(
  eyeHeight: number,
  directionY: number,
  distance: number,
  densityAtReference: number,
  startDistance = FOG_START_DISTANCE,
): number {
  if (
    ![eyeHeight, directionY, distance, densityAtReference, startDistance].every(
      Number.isFinite,
    ) ||
    Math.abs(eyeHeight) > 20_000 ||
    Math.abs(directionY) > 1 ||
    distance < 0 ||
    distance > SKY_DOME_LIMITS.maxDistance ||
    densityAtReference < 0 ||
    densityAtReference > 10_000 ||
    startDistance < 0 ||
    startDistance > SKY_DOME_LIMITS.maxDistance
  )
    throw new Error("Use a supported finite sky fog segment.");
  return Math.exp(
    -segmentFogOpticalDepth(
      eyeHeight,
      directionY,
      Math.min(startDistance, distance),
      distance,
      densityAtReference,
      FOG_HEIGHT_RATE,
      FOG_REFERENCE_HEIGHT,
    ),
  );
}

/** Standalone math; the host chooses the source volume/analytic composition. */
export const WGSL_SKY_DOME = `
fn skyDomeDistance(dome:vec4<f32>,eye:vec3<f32>,direction:vec3<f32>)->f32{
 if dome.w<=0.0||dot(direction,direction)<1e-20{return 0.0;}
 let ray=normalize(direction);let offset=eye-dome.xyz;let b=dot(offset,ray);let c=dot(offset,offset)-dome.w*dome.w;let discriminant=b*b-c;
 if discriminant<0.0{return 0.0;}
 let root=sqrt(discriminant);let near=-b-root;let far=-b+root;
 return min(${SKY_DOME_LIMITS.maxDistance}.0,select(max(0.0,far),near,near>1e-6));
}
`;
