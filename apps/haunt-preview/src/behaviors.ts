import { walkable, type Behavior, type SceneAsset } from "./project.ts";
import type { Pose } from "./scene.ts";

export type GhostMotion = {
  id: string;
  behavior: Behavior;
  x: number;
  y: number;
  z: number;
  yaw: number;
  homeX: number;
  homeZ: number;
  scale: number;
  elapsed: number;
  phase: number;
};

/** One local simulation owns movement; saved entities keep authored positions. */
export class GhostSimulation {
  motions = new Map<string, GhostMotion>();
  ticks = 0;

  sync(assets: SceneAsset[]) {
    const live = new Set<string>();
    for (const asset of assets) {
      if (asset.kind !== "ghost") continue;
      live.add(asset.id);
      let motion = this.motions.get(asset.id);
      if (!motion) {
        let hash = 0;
        for (const char of asset.id)
          hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
        motion = {
          id: asset.id,
          behavior: asset.behavior ?? "still",
          x: asset.x,
          y: 0,
          z: asset.z,
          yaw: 0,
          homeX: asset.x,
          homeZ: asset.z,
          scale: asset.scale,
          elapsed: 0,
          phase: (hash % 628) / 100,
        };
        this.motions.set(asset.id, motion);
      }
      if (motion.homeX !== asset.x || motion.homeZ !== asset.z) {
        motion.x = asset.x;
        motion.z = asset.z;
      }
      motion.homeX = asset.x;
      motion.homeZ = asset.z;
      motion.scale = asset.scale;
      motion.behavior = asset.behavior ?? "still";
    }
    for (const id of this.motions.keys())
      if (!live.has(id)) this.motions.delete(id);
  }

  step(
    player: Pose,
    seconds: number,
    floor: (x: number, z: number) => boolean = walkable,
    guide?: (
      from: GhostMotion,
      player: Pose,
    ) => { x: number; z: number; stopDistance?: number },
  ) {
    const dt = Number.isFinite(seconds)
      ? Math.max(0, Math.min(0.05, seconds))
      : 0;
    if (!dt) return;
    this.ticks++;
    for (const motion of this.motions.values()) {
      if (motion.behavior === "still") continue;
      motion.elapsed += dt;
      let dx = 0,
        dz = 0;
      if (motion.behavior === "float") {
        const angle = motion.phase + motion.elapsed * 0.7;
        dx = Math.cos(angle) * 0.45 * dt;
        dz = Math.sin(angle) * 0.45 * dt;
      } else {
        const target = guide?.(motion, player) ?? player;
        const x = target.x - motion.x,
          z = target.z - motion.z;
        const distance = Math.hypot(x, z);
        const step = Math.min(
          1.2 * dt,
          Math.max(
            0,
            distance -
              ("stopDistance" in target &&
              typeof target.stopDistance === "number"
                ? target.stopDistance
                : 0.85 + motion.scale * 0.3),
          ),
        );
        if (distance > 0.001) {
          dx = (x / distance) * step;
          dz = (z / distance) * step;
          motion.yaw = Math.atan2(x, z);
        }
      }
      // Slide along room walls and through the connecting corridor.
      const fits = (x: number, z: number) => {
        const radius = motion.scale * 0.3;
        return (
          floor(x - radius, z) &&
          floor(x + radius, z) &&
          floor(x, z - radius) &&
          floor(x, z + radius)
        );
      };
      if (fits(motion.x + dx, motion.z)) motion.x += dx;
      if (fits(motion.x, motion.z + dz)) motion.z += dz;
      const height =
        0.16 + Math.sin(motion.phase + motion.elapsed * 1.7) * 0.12;
      motion.y += (height - motion.y) * Math.min(1, dt * 5);
    }
  }

  positions(ids: string[]) {
    return ids.flatMap((id) => {
      const motion = this.motions.get(id);
      return motion ? [{ id, x: motion.x, z: motion.z }] : [];
    });
  }

  snapshot() {
    return Array.from(
      this.motions.values(),
      ({ id, behavior, x, y, z, yaw }) => ({ id, behavior, x, y, z, yaw }),
    );
  }
}
