/**
 * Original low-poly character figures, built only from the renderer's owned
 * unit cube. No asset files: each figure is a list of coloured boxes in local
 * metres with its feet at y = 0 and its front toward +z.
 */
import type { NpcFigure, NpcPose } from "../runtime/npc.ts";
import { modelMatrix, multiply, type V3 } from "./math.ts";

/** A mostly brown palette with orange and black accents. */
export const NPC_COLORS = {
  black: { base: [0.035, 0.03, 0.03], emissive: [0, 0, 0] },
  brown: { base: [0.34, 0.2, 0.1], emissive: [0, 0, 0] },
  darkBrown: { base: [0.17, 0.1, 0.055], emissive: [0, 0, 0] },
  tan: { base: [0.6, 0.45, 0.28], emissive: [0, 0, 0] },
  skin: { base: [0.66, 0.48, 0.34], emissive: [0, 0, 0] },
  orange: { base: [0.86, 0.36, 0.06], emissive: [0, 0, 0] },
  ember: { base: [1, 0.5, 0.1], emissive: [0.9, 0.35, 0.05] },
  sheet: { base: [0.82, 0.78, 0.68], emissive: [0.08, 0.075, 0.06] },
} as const;
export type NpcColor = keyof typeof NPC_COLORS;
export interface NpcPart {
  min: V3;
  max: V3;
  color: NpcColor;
}
const box = (min: V3, max: V3, color: NpcColor): NpcPart => ({
  min,
  max,
  color,
});
/** A part and its mirror across x = 0. */
const pair = (min: V3, max: V3, color: NpcColor): NpcPart[] => [
  box(min, max, color),
  box([-max[0], min[1], min[2]], [-min[0], max[1], max[2]], color),
];

export const NPC_FIGURE_PARTS: Readonly<Record<NpcFigure, NpcPart[]>> = {
  cat: [
    box([-0.12, 0.12, -0.24], [0.12, 0.34, 0.18], "black"),
    ...pair([0.05, 0, 0.08], [0.1, 0.14, 0.14], "black"),
    ...pair([0.05, 0, -0.22], [0.1, 0.14, -0.16], "black"),
    box([-0.11, 0.26, 0.12], [0.11, 0.46, 0.32], "black"),
    ...pair([0.04, 0.46, 0.18], [0.1, 0.55, 0.24], "black"),
    ...pair([0.035, 0.36, 0.32], [0.075, 0.4, 0.33], "ember"),
    box([-0.025, 0.31, 0.32], [0.025, 0.34, 0.335], "orange"),
    box([-0.03, 0.26, -0.3], [0.03, 0.62, -0.24], "black"),
    box([-0.13, 0.25, 0.08], [0.13, 0.28, 0.13], "orange"),
  ],
  witch: [
    box([-0.3, 0, -0.24], [0.3, 0.82, 0.24], "black"),
    box([-0.24, 0.82, -0.17], [0.24, 1.32, 0.17], "black"),
    box([-0.25, 0.8, -0.18], [0.25, 0.87, 0.18], "orange"),
    ...pair([0.24, 0.86, -0.08], [0.34, 1.3, 0.08], "black"),
    ...pair([0.25, 0.76, -0.06], [0.33, 0.86, 0.06], "skin"),
    box([-0.13, 1.32, -0.13], [0.13, 1.6, 0.13], "skin"),
    ...pair([0.04, 1.45, 0.13], [0.08, 1.49, 0.14], "black"),
    box([-0.02, 1.4, 0.13], [0.02, 1.45, 0.19], "skin"),
    box([-0.32, 1.58, -0.32], [0.32, 1.62, 0.32], "black"),
    box([-0.16, 1.62, -0.16], [0.16, 1.69, 0.16], "orange"),
    box([-0.15, 1.69, -0.15], [0.15, 1.8, 0.15], "black"),
    box([-0.1, 1.8, -0.1], [0.1, 1.92, 0.1], "black"),
    box([-0.05, 1.92, -0.07], [0.05, 2.04, 0.03], "black"),
    box([-0.025, 2.0, -0.12], [0.025, 2.06, -0.04], "black"),
    box([0.36, 0, 0.1], [0.4, 1.25, 0.14], "brown"),
  ],
  ghost: [
    box([-0.3, 0, -0.26], [0.3, 0.22, 0.26], "sheet"),
    box([-0.27, 0.22, -0.23], [0.27, 0.7, 0.23], "sheet"),
    box([-0.23, 0.7, -0.2], [0.23, 1.02, 0.2], "sheet"),
    box([-0.15, 1.02, -0.14], [0.15, 1.14, 0.14], "sheet"),
    ...pair([0.27, 0.42, -0.07], [0.42, 0.62, 0.07], "sheet"),
    ...pair([0.06, 0.8, 0.2], [0.13, 0.92, 0.21], "black"),
    box([-0.05, 0.66, 0.2], [0.05, 0.74, 0.21], "black"),
    ...pair([0.08, -0.08, -0.12], [0.2, 0, 0.12], "sheet"),
  ],
  gardener: [
    ...pair([0.05, 0, -0.08], [0.15, 0.78, 0.08], "darkBrown"),
    box([-0.22, 0.78, -0.14], [0.22, 1.36, 0.14], "brown"),
    box([-0.2, 0.62, 0.14], [0.2, 1.24, 0.16], "tan"),
    ...pair([0.22, 0.86, -0.07], [0.32, 1.32, 0.07], "brown"),
    ...pair([0.23, 0.76, -0.06], [0.31, 0.86, 0.06], "skin"),
    box([-0.16, 1.32, -0.1], [0.16, 1.38, 0.12], "orange"),
    box([-0.13, 1.38, -0.12], [0.13, 1.64, 0.13], "skin"),
    ...pair([0.04, 1.5, 0.13], [0.08, 1.54, 0.14], "black"),
    box([-0.34, 1.62, -0.34], [0.34, 1.66, 0.34], "tan"),
    box([-0.15, 1.66, -0.15], [0.15, 1.78, 0.15], "tan"),
    box([-0.155, 1.66, -0.155], [0.155, 1.7, 0.155], "orange"),
  ],
  ranger: [
    ...pair([0.05, 0, -0.09], [0.16, 0.3, 0.11], "black"),
    ...pair([0.05, 0.3, -0.08], [0.15, 0.8, 0.08], "darkBrown"),
    box([-0.24, 0.62, -0.15], [0.24, 1.38, 0.15], "brown"),
    box([-0.25, 0.8, -0.16], [0.25, 0.86, 0.16], "black"),
    ...pair([0.24, 0.86, -0.07], [0.34, 1.34, 0.07], "brown"),
    ...pair([0.25, 0.76, -0.06], [0.33, 0.86, 0.06], "skin"),
    box([-0.17, 1.32, -0.12], [0.17, 1.4, 0.14], "orange"),
    box([-0.13, 1.4, -0.12], [0.13, 1.66, 0.13], "skin"),
    ...pair([0.04, 1.52, 0.13], [0.08, 1.56, 0.14], "black"),
    box([-0.3, 1.64, -0.3], [0.3, 1.68, 0.3], "darkBrown"),
    box([-0.16, 1.68, -0.16], [0.16, 1.82, 0.16], "darkBrown"),
    box([-0.03, 0.7, -0.24], [0.03, 1.5, -0.18], "tan"),
  ],
};
/** Height of a figure's name label above its feet. */
export const NPC_LABEL_HEIGHT: Readonly<Record<NpcFigure, number>> = {
  cat: 0.85,
  witch: 2.35,
  ghost: 1.5,
  gardener: 2.05,
  ranger: 2.05,
};

export interface NpcInstanceLayout {
  /** Figure index and part index for each instance, grouped by colour. */
  order: [number, number][];
  groups: { color: NpcColor; first: number; count: number }[];
}
export function npcInstanceLayout(
  figures: readonly NpcFigure[],
): NpcInstanceLayout {
  const order: [number, number][] = [];
  const groups: NpcInstanceLayout["groups"] = [];
  for (const color of Object.keys(NPC_COLORS) as NpcColor[]) {
    const first = order.length;
    figures.forEach((figure, f) =>
      NPC_FIGURE_PARTS[figure].forEach((part, p) => {
        if (part.color === color) order.push([f, p]);
      }),
    );
    if (order.length > first)
      groups.push({ color, first, count: order.length - first });
  }
  return { order, groups };
}

/** The animated root of one figure: idle float for the ghost, then its emote. */
export function npcRoot(pose: NpcPose, time: number): number[] {
  const p = pose.emoteProgress;
  const wave = Math.sin(Math.PI * p);
  let yaw = pose.yaw,
    lift = 0,
    tilt = 0;
  if (pose.figure === "ghost") lift += 0.08 * Math.sin(time * 2.1);
  if (pose.emote === "hop") lift += 0.35 * wave;
  if (pose.emote === "spin") yaw += Math.PI * 2 * p;
  if (pose.emote === "wave") yaw += 0.35 * Math.sin(Math.PI * 4 * p);
  if (pose.emote === "bow") tilt = 0.5 * wave;
  const [x, y, z] = pose.position;
  // modelMatrix turns local +z toward (sin yaw, cos yaw), matching atan2(dx, dz).
  const root = modelMatrix([x, y + lift, z], yaw, 1);
  if (!tilt) return root;
  const c = Math.cos(tilt),
    s = Math.sin(tilt);
  return Array.from(
    multiply(root, [1, 0, 0, 0, 0, c, s, 0, 0, -s, c, 0, 0, 0, 0, 1]),
  );
}
export function npcInstanceMatrices(
  layout: NpcInstanceLayout,
  figures: readonly NpcFigure[],
  poses: readonly NpcPose[],
  time: number,
): Float32Array {
  const out = new Float32Array(layout.order.length * 16);
  const roots = poses.map((pose) => npcRoot(pose, time));
  layout.order.forEach(([f, p], i) => {
    const part = NPC_FIGURE_PARTS[figures[f]][p];
    const w = part.max.map((v, k) => Math.max(0.005, v - part.min[k]));
    const local = [
      w[0],
      0,
      0,
      0,
      0,
      w[1],
      0,
      0,
      0,
      0,
      w[2],
      0,
      (part.min[0] + part.max[0]) / 2,
      (part.min[1] + part.max[1]) / 2,
      (part.min[2] + part.max[2]) / 2,
      1,
    ];
    out.set(multiply(roots[f], local), i * 16);
  });
  return out;
}
