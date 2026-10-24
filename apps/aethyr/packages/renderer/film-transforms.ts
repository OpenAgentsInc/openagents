import type { CompiledScene } from "../assets/src/scene.ts";
import { buildInstanceBounds } from "./culling.ts";
import { type V3 } from "./math.ts";

/** A local-space rigid delta for one immutable source instance, for one capture. */
export interface FilmSourceTransform {
  mode: "day" | "night";
  instanceIndex: number;
  meshId: string;
  baseModelSha256: string;
  pivot: V3;
  axis: V3;
  angleRadians: number;
}
export function validateFilmTransforms(value: unknown): FilmSourceTransform[] {
  if (!Array.isArray(value) || value.length > 16)
    throw Error("Choose up to 16 source objects to rotate in a frame.");
  const selected = new Set<string>();
  for (const item of value) {
    if (
      !item ||
      typeof item !== "object" ||
      Array.isArray(item) ||
      Object.keys(item).some(
        (key) =>
          ![
            "mode",
            "instanceIndex",
            "meshId",
            "baseModelSha256",
            "pivot",
            "axis",
            "angleRadians",
          ].includes(key),
      ) ||
      !["day", "night"].includes(item.mode) ||
      !Number.isInteger(item.instanceIndex) ||
      item.instanceIndex < 0 ||
      item.instanceIndex >= 65536 ||
      typeof item.meshId !== "string" ||
      !item.meshId.length ||
      item.meshId.length > 2048 ||
      typeof item.baseModelSha256 !== "string" ||
      !/^[a-f0-9]{64}$/.test(item.baseModelSha256) ||
      ![item.pivot, item.axis].every(
        (p) =>
          Array.isArray(p) &&
          p.length === 3 &&
          p.every(
            (n: unknown) =>
              typeof n === "number" &&
              Number.isFinite(n) &&
              Math.abs(n) <= 1000,
          ),
      ) ||
      Math.hypot(...item.axis) < 1e-6 ||
      typeof item.angleRadians !== "number" ||
      !Number.isFinite(item.angleRadians) ||
      Math.abs(item.angleRadians) > 1e6 ||
      selected.has(item.mode + ":" + item.instanceIndex)
    )
      throw Error(
        "Choose distinct source objects with finite rotations and matching saved transforms.",
      );
    selected.add(item.mode + ":" + item.instanceIndex);
  }
  return value as FilmSourceTransform[];
}
/** SHA-256 of exactly sixteen little-endian Float64 model values, without rounding to GPU precision. */
export async function sourceModelSha256(
  model: ArrayLike<number>,
): Promise<string> {
  if (model.length !== 16 || !Array.from(model).every(Number.isFinite))
    throw Error("Choose a finite saved source transform.");
  const bytes = new ArrayBuffer(128),
    view = new DataView(bytes);
  Array.from(model).forEach((n, i) => view.setFloat64(i * 8, n, true));
  const hash = await crypto.subtle.digest("SHA-256", bytes);
  return Array.from(new Uint8Array(hash), (n) =>
    n.toString(16).padStart(2, "0"),
  ).join("");
}
/** Mbase · T(pivot) · R(axis, angle) · T(-pivot); no time integration or source mutation. */
export function rotateSourceModel(
  base: ArrayLike<number>,
  transform: Pick<FilmSourceTransform, "pivot" | "axis" | "angleRadians">,
): Float32Array {
  if (
    base.length !== 16 ||
    !Array.from(base).every(Number.isFinite) ||
    ![transform.pivot, transform.axis].every(
      (p) => Array.isArray(p) && p.length === 3 && p.every(Number.isFinite),
    ) ||
    Math.hypot(...transform.axis) < 1e-6 ||
    !Number.isFinite(transform.angleRadians)
  )
    throw Error("Choose a finite source transform, pivot and rotation axis.");
  const norm = Math.hypot(...transform.axis),
    [x, y, z] = transform.axis.map((n) => n / norm),
    angle = transform.angleRadians % (2 * Math.PI),
    c = Math.cos(angle),
    s = Math.sin(angle),
    t = 1 - c;
  const rotation = [
    t * x * x + c,
    t * x * y + s * z,
    t * x * z - s * y,
    t * x * y - s * z,
    t * y * y + c,
    t * y * z + s * x,
    t * x * z + s * y,
    t * y * z - s * x,
    t * z * z + c,
  ];
  const p = transform.pivot;
  const shift = p.map(
    (n, row) =>
      n -
      rotation[row] * p[0] -
      rotation[3 + row] * p[1] -
      rotation[6 + row] * p[2],
  );
  const result = new Float32Array(16);
  for (let col = 0; col < 3; col++)
    for (let row = 0; row < 3; row++)
      result[col * 4 + row] =
        base[row] * rotation[col * 3] +
        base[4 + row] * rotation[col * 3 + 1] +
        base[8 + row] * rotation[col * 3 + 2];
  for (let row = 0; row < 3; row++)
    result[12 + row] =
      base[12 + row] +
      base[row] * shift[0] +
      base[4 + row] * shift[1] +
      base[8 + row] * shift[2];
  result[15] = 1;
  return result;
}
export async function prepareFilmTransforms(
  scene: CompiledScene,
  value: unknown,
  mode: "day" | "night",
) {
  const transforms = structuredClone(validateFilmTransforms(value));
  return await Promise.all(
    transforms.map(async (transform) => {
      const instance = scene.instances[transform.instanceIndex],
        mesh = instance && scene.meshes[instance.mesh];
      if (
        transform.mode !== mode ||
        scene.scenario !== mode ||
        !mesh ||
        mesh.id !== transform.meshId ||
        instance.model.length !== 16 ||
        !instance.model.every(Number.isFinite) ||
        [3, 7, 11].some((i) => Math.abs(instance.model[i]) > 1e-10) ||
        Math.abs(instance.model[15] - 1) > 1e-10 ||
        (await sourceModelSha256(instance.model)) !== transform.baseModelSha256
      )
        throw Error(
          "The selected source object changed. Choose it again before filming.",
        );
      const m = instance.model,
        axes = [
          [m[0], m[1], m[2]],
          [m[4], m[5], m[6]],
          [m[8], m[9], m[10]],
        ],
        lengths = axes.map((axis) => Math.hypot(...axis)),
        size = Math.max(...lengths);
      if (
        Math.min(...lengths) < 1e-6 ||
        lengths.some((n) => Math.abs(n - size) > size * 1e-5) ||
        [
          [0, 1],
          [0, 2],
          [1, 2],
        ].some(
          ([a, b]) =>
            Math.abs(axes[a].reduce((sum, n, i) => sum + n * axes[b][i], 0)) >
            size * size * 1e-5,
        )
      )
        throw Error(
          "Choose a source object with uniform scale and no shear for a rigid rotation.",
        );
      const model = rotateSourceModel(instance.model, transform);
      if (!Array.from(model).every(Number.isFinite))
        throw Error("The rotated source object is out of range.");
      const bounds = buildInstanceBounds({
        meshes: [mesh],
        instances: [{ ...instance, mesh: 0, model: Array.from(model) }],
      });
      if (Array.from(bounds.aabbs).some((n) => Math.abs(n) > 20000))
        throw Error("Keep the filmed source object within 20,000 meters.");
      return { instanceIndex: transform.instanceIndex, model, bounds };
    }),
  );
}
