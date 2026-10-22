import { box, face, buildGeometry } from "./scene.ts";
import { TownFloor } from "./town.ts";
type V3 = [number, number, number];
const wood: V3 = [0.48, 0.3, 0.16],
  tile: V3 = [0.45, 0.18, 0.1],
  plaster: V3 = [0.68, 0.62, 0.46];
/** Floors, stairs and roof extensions complement the kit's textured walls. */
export function townGeometry(floor: TownFloor, proxies = false) {
  return buildGeometry(() => {
    box(0, -0.2, 0, 96, 0.3, 96, [0.31, 0.42, 0.23]);
    box(0, -0.025, 4, 40, 0.06, 7, [0.57, 0.53, 0.44]);
    for (const e of floor.entities.filter(
      (e) => e.kind === "cottage" || e.kind === "townhouse",
    ))
      box(
        e.x - 2 * e.scale,
        -0.015,
        e.z + 9 * e.scale,
        2 * e.scale,
        0.06,
        8 * e.scale,
        [0.57, 0.53, 0.44],
      );
    for (const { entity: e, level, plan } of floor.levels) {
      const s = e.scale,
        y = level * 4 * s;
      const rect = (l: number, r: number, t: number, b: number) => {
        if (r - l < 0.02 || b - t < 0.02) return;
        box(
          e.x + ((l + r) / 2) * s,
          y - 0.08 * s,
          e.z + ((t + b) / 2) * s,
          (r - l) * s,
          0.16 * s,
          (b - t) * s,
          [0.58, 0.39, 0.22],
        );
        for (let x = l + 0.3; x < r; x += 0.6)
          box(
            e.x + x * s,
            y + 0.003,
            e.z + ((t + b) / 2) * s,
            0.012 * s,
            0.012,
            (b - t) * s,
            [0.36, 0.25, 0.16],
          );
      };
      for (const room of plan.rooms) {
        const l = room.x - room.width / 2,
          r = room.x + room.width / 2,
          t = room.z - room.depth / 2,
          b = room.z + room.depth / 2;
        if (level && e.kind === "townhouse") {
          rect(l, Math.min(r, 2.3), t, b);
          rect(Math.max(l, 3.75), r, t, b);
          rect(Math.max(l, 2.3), Math.min(r, 3.75), t, Math.min(b, -4.05));
          rect(Math.max(l, 2.3), Math.min(r, 3.75), Math.max(t, 3.05), b);
        } else rect(l, r, t, b);
        const extension = room.id.startsWith("room-");
        if (extension || level === floor.town.buildings[e.id].floors.length - 1)
          box(
            e.x + room.x * s,
            y + 3.96 * s,
            e.z + room.z * s,
            room.width * s,
            0.08 * s,
            room.depth * s,
            plaster,
          );
        if (extension) {
          const x0 = e.x + (l - 0.2) * s,
            x1 = e.x + (r + 0.2) * s,
            z0 = e.z + (t - 0.2) * s,
            z1 = e.z + (b + 0.2) * s,
            zm = (z0 + z1) / 2,
            ey = y + 4 * s,
            ridge = ey + 1.2 * s;
          face(
            [
              [x0, ey, z0],
              [x1, ey, z0],
              [x1, ridge, zm],
              [x0, ridge, zm],
            ],
            [0, 0.7, -0.7],
            tile,
          );
          face(
            [
              [x0, ridge, zm],
              [x1, ridge, zm],
              [x1, ey, z1],
              [x0, ey, z1],
            ],
            [0, 0.7, 0.7],
            tile,
          );
          if (level)
            for (const x of [l + 0.15, r - 0.15])
              for (const z of [t + 0.15, b - 0.15])
                box(e.x + x * s, y / 2, e.z + z * s, 0.2 * s, y, 0.2 * s, wood);
        }
      }
    }
    for (const e of floor.entities.filter((e) => e.kind === "townhouse"))
      for (let n = 0; n < 28; n++) {
        const s = e.scale,
          h = ((n + 1) / 7) * s,
          z = 3 - (n + 0.5) / 4;
        box(e.x + 3.025 * s, h / 2, e.z + z * s, 1.45 * s, h, 0.25 * s, wood);
        if (n % 4 === 0)
          box(
            e.x + 2.35 * s,
            h + 0.5 * s,
            e.z + z * s,
            0.06 * s,
            s,
            0.06 * s,
            [0.32, 0.2, 0.12],
          );
      }
    if (proxies)
      for (const w of floor.walls) {
        const horizontal = w.z1 === w.z2;
        box(
          (w.x1 + w.x2) / 2,
          w.y + w.height / 2,
          (w.z1 + w.z2) / 2,
          horizontal ? w.x2 - w.x1 : 0.2,
          w.height,
          horizontal ? 0.2 : w.z2 - w.z1,
          plaster,
        );
      }
  });
}
