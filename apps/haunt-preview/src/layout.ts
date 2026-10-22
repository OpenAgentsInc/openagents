import type { Pose } from "./scene.ts";

export const SIDES = ["north", "east", "south", "west"] as const;
export type Side = (typeof SIDES)[number];
export type Room = {
  id: string;
  name: string;
  x: number;
  z: number;
  width: number;
  depth: number;
  outdoor?: boolean;
};
export type Layout = { rooms: Room[]; removedWalls: string[] };
export type Wall = {
  id: string;
  roomId: string;
  name: string;
  side: Side;
  x1: number;
  z1: number;
  x2: number;
  z2: number;
  exterior: boolean;
  removed: boolean;
};
export const DEFAULT_LAYOUT: Layout = {
  rooms: [
    { id: "hall", name: "Lantern Hall", x: 0, z: 1.5, width: 7, depth: 13 },
    { id: "passage", name: "Passage", x: 0, z: -7, width: 2.5, depth: 4 },
    { id: "gallery", name: "Gallery", x: 0, z: -13, width: 8, depth: 8 },
  ],
  removedWalls: [],
};
export const MAX_ROOMS = 16;
const EPS = 0.0001;
const bounds = (r: Room) => ({
  left: r.x - r.width / 2,
  right: r.x + r.width / 2,
  top: r.z - r.depth / 2,
  bottom: r.z + r.depth / 2,
});
const overlap = (a: number, b: number, c: number, d: number) =>
  Math.min(b, d) - Math.max(a, c);
function adjacent(a: Room, b: Room) {
  const p = bounds(a),
    q = bounds(b);
  return (
    ((Math.abs(p.right - q.left) < EPS || Math.abs(p.left - q.right) < EPS) &&
      overlap(p.top, p.bottom, q.top, q.bottom) >= 1.8) ||
    ((Math.abs(p.bottom - q.top) < EPS || Math.abs(p.top - q.bottom) < EPS) &&
      overlap(p.left, p.right, q.left, q.right) >= 1.8)
  );
}

export function readLayout(value: unknown): Layout {
  const p = value as Layout;
  if (
    !p ||
    !Array.isArray(p.rooms) ||
    !p.rooms.length ||
    p.rooms.length > MAX_ROOMS ||
    !Array.isArray(p.removedWalls) ||
    p.removedWalls.length > MAX_ROOMS * 4
  )
    throw new Error("Keep the house between 1 and 16 connected rooms.");
  const ids = new Set<string>();
  for (const r of p.rooms) {
    if (
      !r ||
      typeof r.id !== "string" ||
      !/^[a-zA-Z0-9-]{1,64}$/.test(r.id) ||
      ids.has(r.id) ||
      typeof r.name !== "string" ||
      !r.name.trim() ||
      r.name.length > 60 ||
      (r.outdoor !== undefined && typeof r.outdoor !== "boolean") ||
      ![r.x, r.z, r.width, r.depth].every(Number.isFinite) ||
      r.width < 2 ||
      r.depth < 2 ||
      r.width > (r.outdoor ? 32 : 16) ||
      r.depth > (r.outdoor ? 32 : 16) ||
      Math.abs(r.x) + r.width / 2 > 64 ||
      Math.abs(r.z) + r.depth / 2 > 64
    )
      throw new Error("Choose finite room sizes inside the house limits.");
    ids.add(r.id);
  }
  for (let i = 0; i < p.rooms.length; i++)
    for (let j = i + 1; j < p.rooms.length; j++) {
      const a = bounds(p.rooms[i]),
        b = bounds(p.rooms[j]);
      if (
        overlap(a.left, a.right, b.left, b.right) > EPS &&
        overlap(a.top, a.bottom, b.top, b.bottom) > EPS
      )
        throw new Error(
          "That room would overlap another room. Choose a different wall.",
        );
    }
  const connected = new Set([p.rooms[0].id]);
  for (let pass = 0; pass < p.rooms.length; pass++)
    for (const a of p.rooms)
      if (connected.has(a.id))
        for (const b of p.rooms) if (adjacent(a, b)) connected.add(b.id);
  if (connected.size !== p.rooms.length)
    throw new Error("Connect each room to the house with a walkable doorway.");
  const validSides = new Set(
    p.rooms.flatMap((r) => SIDES.map((s) => `${r.id}:${s}`)),
  );
  if (
    new Set(p.removedWalls).size !== p.removedWalls.length ||
    p.removedWalls.some((id) => typeof id !== "string" || !validSides.has(id))
  )
    throw new Error("Choose an existing wall.");
  return {
    rooms: p.rooms.map((r) => ({
      id: r.id,
      name: r.name,
      x: r.x,
      z: r.z,
      width: r.width,
      depth: r.depth,
      ...(r.outdoor === undefined ? {} : { outdoor: r.outdoor }),
    })),
    removedWalls: [...p.removedWalls],
  };
}

/** Shared wall segments drive rendering, aiming, and floor collision. */
export class HouseFloor {
  layout: Layout;
  surfaces: Wall[] = [];
  walls: Wall[];
  links = new Map<
    string,
    { to: Room; x: number; z: number; nx: number; nz: number }[]
  >();
  constructor(layout: Layout = DEFAULT_LAYOUT) {
    this.layout = layout;
    for (const r of layout.rooms)
      for (const side of SIDES) {
        const b = bounds(r),
          horizontal = side === "north" || side === "south";
        const plane =
          side === "north"
            ? b.top
            : side === "south"
              ? b.bottom
              : side === "west"
                ? b.left
                : b.right;
        const start = horizontal ? b.left : b.top,
          end = horizontal ? b.right : b.bottom;
        const neighbors: { start: number; end: number; other: Room }[] = [];
        for (const other of layout.rooms) {
          if (other === r) continue;
          const q = bounds(other);
          const opposite =
            side === "north"
              ? q.bottom
              : side === "south"
                ? q.top
                : side === "west"
                  ? q.right
                  : q.left;
          const lo = Math.max(start, horizontal ? q.left : q.top),
            hi = Math.min(end, horizontal ? q.right : q.bottom);
          if (Math.abs(plane - opposite) < EPS && hi - lo > EPS)
            neighbors.push({ start: lo, end: hi, other });
        }
        const cuts = [start, end];
        for (const n of neighbors) {
          const mid = (n.start + n.end) / 2,
            half = Math.min(1.1, (n.end - n.start) / 2);
          cuts.push(n.start, n.end, mid - half, mid + half);
        }
        cuts.sort((a, b) => a - b);
        for (let i = 1; i < cuts.length; i++) {
          const lo = cuts[i - 1],
            hi = cuts[i],
            mid = (lo + hi) / 2;
          if (hi - lo < EPS) continue;
          const neighbor = neighbors.find(
            (n) => mid > n.start - EPS && mid < n.end + EPS,
          );
          if (neighbor) {
            const center = (neighbor.start + neighbor.end) / 2,
              half = Math.min(1.1, (neighbor.end - neighbor.start) / 2);
            if (
              Math.abs(mid - center) < half - EPS ||
              layout.rooms.indexOf(r) > layout.rooms.indexOf(neighbor.other)
            )
              continue;
          }
          const id = `${r.id}:${side}`;
          this.surfaces.push({
            id,
            roomId: r.id,
            name: `${r.name} · ${side} wall`,
            side,
            x1: horizontal ? lo : plane,
            z1: horizontal ? plane : lo,
            x2: horizontal ? hi : plane,
            z2: horizontal ? plane : hi,
            exterior: !neighbor,
            removed: layout.removedWalls.includes(id),
          });
        }
      }
    this.walls = this.surfaces.filter((w) => !w.removed);
    for (const room of layout.rooms) {
      const a = bounds(room),
        links: { to: Room; x: number; z: number; nx: number; nz: number }[] =
          [];
      for (const other of layout.rooms)
        if (other !== room && adjacent(room, other)) {
          const b = bounds(other);
          if (
            Math.abs(a.right - b.left) < EPS ||
            Math.abs(a.left - b.right) < EPS
          )
            links.push({
              to: other,
              x: Math.abs(a.right - b.left) < EPS ? a.right : a.left,
              z: (Math.max(a.top, b.top) + Math.min(a.bottom, b.bottom)) / 2,
              nx: other.x > room.x ? 1 : -1,
              nz: 0,
            });
          else
            links.push({
              to: other,
              x: (Math.max(a.left, b.left) + Math.min(a.right, b.right)) / 2,
              z: Math.abs(a.bottom - b.top) < EPS ? a.bottom : a.top,
              nx: 0,
              nz: other.z > room.z ? 1 : -1,
            });
        }
      this.links.set(room.id, links);
    }
  }
  canStand = (x: number, z: number, radius = 0.24): boolean => {
    if (!Number.isFinite(x) || !Number.isFinite(z)) return false;
    const onFloor = (px: number, pz: number) =>
      this.layout.rooms.some((r) => {
        const b = bounds(r);
        return (
          px > b.left - EPS &&
          px < b.right + EPS &&
          pz > b.top - EPS &&
          pz < b.bottom + EPS
        );
      });
    if (
      !onFloor(x, z) ||
      !onFloor(x - radius, z) ||
      !onFloor(x + radius, z) ||
      !onFloor(x, z - radius) ||
      !onFloor(x, z + radius)
    )
      return false;
    for (const w of this.walls) {
      const px = Math.max(
        Math.min(w.x1, w.x2),
        Math.min(Math.max(w.x1, w.x2), x),
      );
      const pz = Math.max(
        Math.min(w.z1, w.z2),
        Math.min(Math.max(w.z1, w.z2), z),
      );
      if (Math.hypot(px - x, pz - z) < radius + 0.09) return false;
    }
    return true;
  };
  guide = (
    from: { x: number; z: number },
    player: { x: number; z: number },
  ): { x: number; z: number; stopDistance?: number } => {
    const contains = (r: Room, p: { x: number; z: number }) => {
      const b = bounds(r);
      return (
        p.x >= b.left - EPS &&
        p.x <= b.right + EPS &&
        p.z >= b.top - EPS &&
        p.z <= b.bottom + EPS
      );
    };
    const sources = this.layout.rooms.filter((r) => contains(r, from)),
      targets = this.layout.rooms.filter((r) => contains(r, player));
    if (
      sources.some((r) => targets.includes(r)) ||
      !sources.length ||
      !targets.length
    )
      return player;
    const queue = [
        {
          room: sources[0],
          first: undefined as { x: number; z: number } | undefined,
        },
      ],
      seen = new Set<string>();
    for (let index = 0; index < queue.length; index++) {
      const entry = queue[index];
      if (targets.includes(entry.room) && entry.first)
        return { ...entry.first, stopDistance: 0.06 };
      if (seen.has(entry.room.id)) continue;
      seen.add(entry.room.id);
      for (const link of this.links.get(entry.room.id) ?? [])
        if (!seen.has(link.to.id))
          queue.push({
            room: link.to,
            first: entry.first ?? {
              x: link.x + link.nx * 0.8,
              z: link.z + link.nz * 0.8,
            },
          });
    }
    return player;
  };
  pick(
    pose: Pose,
    turn = 0,
  ): (Wall & { distance: number; hitX: number; hitZ: number }) | undefined {
    const dx = Math.sin(pose.yaw + turn),
      dz = -Math.cos(pose.yaw + turn);
    let picked:
      (Wall & { distance: number; hitX: number; hitZ: number }) | undefined;
    for (const w of this.surfaces) {
      if (w.removed && !w.exterior) continue;
      const horizontal = w.z1 === w.z2,
        direction = horizontal ? dz : dx;
      if (Math.abs(direction) < EPS) continue;
      const distance =
        ((horizontal ? w.z1 : w.x1) - (horizontal ? pose.z : pose.x)) /
        direction;
      if (
        distance < 0.05 ||
        distance > 80 ||
        (picked && distance >= picked.distance)
      )
        continue;
      const at = horizontal ? pose.x + dx * distance : pose.z + dz * distance;
      const min = horizontal ? Math.min(w.x1, w.x2) : Math.min(w.z1, w.z2),
        max = horizontal ? Math.max(w.x1, w.x2) : Math.max(w.z1, w.z2);
      const height = 1.55 + Math.tan(pose.pitch) * distance;
      if (at >= min - EPS && at <= max + EPS && height > 0.05 && height < 3.6)
        picked = {
          ...w,
          distance,
          hitX: pose.x + dx * distance,
          hitZ: pose.z + dz * distance,
        };
    }
    return picked;
  }
}

export function extendLayout(
  layout: Layout,
  wallId: string,
  size: number,
  wing = false,
  at?: { x: number; z: number },
  outdoor = false,
): Layout {
  if (
    !Number.isFinite(size) ||
    size < 3 ||
    size > (outdoor ? 32 : 10) ||
    (outdoor && wing)
  )
    throw new Error("Choose a room size between 3 and 10 meters.");
  const next = structuredClone(layout),
    floor = new HouseFloor(layout);
  const parts = floor.surfaces
    .filter((w) => w.id === wallId && w.exterior)
    .sort(
      (a, b) =>
        Math.hypot(b.x2 - b.x1, b.z2 - b.z1) -
        Math.hypot(a.x2 - a.x1, a.z2 - a.z1),
    );
  const wall = parts[0];
  if (!wall)
    throw new Error("Choose an outside wall or opening to extend the house.");
  const nx = wall.side === "east" ? 1 : wall.side === "west" ? -1 : 0,
    nz = wall.side === "south" ? 1 : wall.side === "north" ? -1 : 0;
  if (at && ![at.x, at.z].every(Number.isFinite))
    throw new Error("Choose a point on the wall.");
  const horizontal = wall.z1 === wall.z2;
  const lo = horizontal ? wall.x1 : wall.z1,
    hi = horizontal ? wall.x2 : wall.z2;
  const middle = at
    ? Math.max(lo + 0.5, Math.min(hi - 0.5, horizontal ? at.x : at.z))
    : (lo + hi) / 2;
  const cx = horizontal ? middle : wall.x1,
    cz = horizontal ? wall.z1 : middle;
  for (let i = 0; i < (wing ? 3 : 1); i++)
    next.rooms.push({
      id: `room-${crypto.randomUUID()}`,
      ...(outdoor ? { outdoor: true } : {}),
      name: outdoor
        ? "Courtyard"
        : wing
          ? `Wing ${layout.rooms.length + 1} · room ${i + 1}`
          : `Room ${next.rooms.length + 1}`,
      x: cx + nx * size * (i + 0.5),
      z: cz + nz * size * (i + 0.5),
      width: size,
      depth: size,
    });
  return readLayout(next);
}
