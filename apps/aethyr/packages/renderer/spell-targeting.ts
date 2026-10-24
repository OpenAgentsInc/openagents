import {
  add,
  cross,
  direction,
  normalize,
  scale,
  sub,
  type V3,
} from "./math.ts";
import type { DruidAbility } from "../runtime/druid-abilities.ts";

export interface AimCamera {
  eye: V3;
  yaw: number;
  pitch: number;
  fov?: number;
  up?: V3;
}
export interface AimPointer {
  x: number;
  y: number;
  locked: boolean;
}
export interface SpellRay {
  origin: V3;
  direction: V3;
}
export interface SpellTarget {
  point: V3;
  normal: V3;
  distance: number;
  nodeId?: number;
  enemyId?: number;
}
export interface GameplayAimMarker {
  overlay?: boolean;
  point: V3;
  normal: V3;
  radius: number;
}
export const LOCKED_THIRD_PERSON_AIM_Y = 0.22;

/** Match the renderer's perspective and camera basis, including a rolled camera. */
export function pointerCameraRay(
  camera: AimCamera,
  aspect: number,
  pointer: AimPointer,
  thirdPerson: boolean,
): SpellRay {
  if (
    ![aspect, camera.fov ?? 1, pointer.x, pointer.y].every(Number.isFinite) ||
    aspect <= 0 ||
    (camera.fov ?? 1) <= 0 ||
    (camera.fov ?? 1) >= Math.PI
  )
    throw new Error("The world view needs a valid camera and pointer.");
  const x = pointer.locked ? 0 : Math.max(-1, Math.min(1, pointer.x));
  const y = pointer.locked
    ? thirdPerson
      ? LOCKED_THIRD_PERSON_AIM_Y
      : 0
    : Math.max(-1, Math.min(1, pointer.y));
  const forward = direction(camera.yaw, camera.pitch);
  const right = normalize(cross(forward, camera.up ?? [0, 1, 0]));
  const up = normalize(cross(right, forward));
  const tangent = Math.tan((camera.fov ?? 1) / 2);
  return {
    origin: [...camera.eye],
    direction: normalize(
      add(
        forward,
        add(scale(right, x * tangent * aspect), scale(up, y * tangent)),
      ),
    ),
  };
}

export function spellAimRange(ability: DruidAbility): number {
  return ability === "lightning_bolt" ? 30.48 : 80;
}
export function spellAimRadius(ability: DruidAbility): number {
  return ability === "meteor_shower"
    ? 4
    : ability === "thunderbolt"
      ? 3.4
      : 1.8;
}

/** Pick through the rendered camera, then resolve the spell's unobstructed hand ray. */
export function resolveSpellTarget(
  cameraRay: SpellRay,
  origin: V3,
  range: number,
  query: (ray: SpellRay, maxDistance: number) => SpellTarget | undefined,
): { ray: SpellRay; target: SpellTarget } | undefined {
  const cameraRange = range + Math.hypot(...sub(cameraRay.origin, origin));
  let viewed: SpellTarget | undefined;
  // Nearby hits stop before broad-phase selection scans the full spell range.
  for (let offset = 0; offset < cameraRange; offset += 20) {
    viewed = query(
      {
        origin: add(cameraRay.origin, scale(cameraRay.direction, offset)),
        direction: cameraRay.direction,
      },
      Math.max(0.1, Math.min(20.02, cameraRange - offset)),
    );
    if (viewed) break;
  }
  if (!viewed) return undefined;
  const delta = sub(viewed.point, origin);
  const length = Math.hypot(...delta);
  if (length < 0.01 || length > range + 0.002) return undefined;
  const ray: SpellRay = {
    origin: [...origin],
    direction: scale(delta, 1 / length),
  };
  const target = query(ray, Math.min(range, Math.max(0.1, length + 0.02)));
  if (!target || target.distance > range + 0.002) return undefined;
  return {
    ray,
    target: {
      point: target.point,
      normal: target.normal,
      distance: target.distance,
      ...(target.nodeId == null ? {} : { nodeId: target.nodeId }),
      ...(target.enemyId == null ? {} : { enemyId: target.enemyId }),
    },
  };
}
