export type Vec3 = [number, number, number];
export type World = {
  id: string;
  name: string;
  category: string;
  description: string;
  position: Vec3;
  radius: number;
  color: Vec3;
  ring: number;
};
export const worlds: World[] = [
  {
    id: "openagents",
    name: "OpenAgents",
    category: "Company world · Locked",
    description:
      "Our home in the Verse. The Gym, Pylons, and more will open here for their communities.",
    position: [0, 0, 0],
    radius: 1.5,
    color: [0.56, 0.69, 0.78],
    ring: 3.3,
  },
  {
    id: "blue-rush",
    name: "Blue Rush",
    category: "Creator studio",
    description:
      "Stories, games, and worlds with a little wonder. Explore the studio behind the Haunted House.",
    position: [-8.4, 0.9, -2.8],
    radius: 0.78,
    color: [0.22, 0.63, 0.8],
    ring: 1.9,
  },
  {
    id: "haunted-house",
    name: "Haunted House",
    category: "A world by Blue Rush",
    description:
      "Step into a spooky little house. A place to play, experiment, and make something your own.",
    position: [-5.7, -0.7, 4.3],
    radius: 0.69,
    color: [0.82, 0.44, 0.24],
    ring: 1.55,
  },
  {
    id: "creator-worlds",
    name: "Creator worlds",
    category: "Future destination",
    description:
      "A place for worlds made by people like you. This part of the Verse is still taking shape.",
    position: [7.7, 0.4, -4],
    radius: 0.82,
    color: [0.53, 0.42, 0.79],
    ring: 2.05,
  },
  {
    id: "new-horizons",
    name: "New horizons",
    category: "Future destination",
    description:
      "More room for imagination. New destinations will appear as the Verse grows.",
    position: [8.4, -0.8, 5.2],
    radius: 0.62,
    color: [0.28, 0.66, 0.61],
    ring: 1.55,
  },
  {
    id: "far-field",
    name: "The far field",
    category: "Future destination",
    description:
      "An open horizon for what comes next. For now, enjoy the view.",
    position: [-1.7, 1.2, -8.4],
    radius: 0.59,
    color: [0.76, 0.68, 0.44],
    ring: 1.5,
  },
];
export const sub = (a: Vec3, b: Vec3): Vec3 => [
  a[0] - b[0],
  a[1] - b[1],
  a[2] - b[2],
];
export const dot = (a: Vec3, b: Vec3) =>
  a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
export const cross = (a: Vec3, b: Vec3): Vec3 => [
  a[1] * b[2] - a[2] * b[1],
  a[2] * b[0] - a[0] * b[2],
  a[0] * b[1] - a[1] * b[0],
];
export const norm = (a: Vec3): Vec3 => {
  const l = Math.hypot(...a) || 1;
  return a.map((x) => x / l) as Vec3;
};
export function camera(
  yaw: number,
  pitch: number,
  distance: number,
  target: Vec3,
  aspect: number,
) {
  const eye: Vec3 = [
    target[0] + Math.sin(yaw) * Math.cos(pitch) * distance,
    target[1] + Math.sin(pitch) * distance,
    target[2] + Math.cos(yaw) * Math.cos(pitch) * distance,
  ];
  const z = norm(sub(eye, target)),
    x = norm(cross([0, 1, 0], z)),
    y = cross(z, x);
  const view = new Float32Array([
    x[0],
    y[0],
    z[0],
    0,
    x[1],
    y[1],
    z[1],
    0,
    x[2],
    y[2],
    z[2],
    0,
    -dot(x, eye),
    -dot(y, eye),
    -dot(z, eye),
    1,
  ]);
  const f = 1 / Math.tan(0.57),
    n = 0.1,
    far = 160;
  const proj = new Float32Array([
    f / aspect,
    0,
    0,
    0,
    0,
    f,
    0,
    0,
    0,
    0,
    far / (n - far),
    -1,
    0,
    0,
    (n * far) / (n - far),
    0,
  ]);
  const matrix = new Float32Array(16);
  for (let col = 0; col < 4; col++)
    for (let row = 0; row < 4; row++)
      for (let k = 0; k < 4; k++)
        matrix[col * 4 + row] += proj[k * 4 + row] * view[col * 4 + k];
  return { eye, matrix };
}
export function project(point: Vec3, matrix: Float32Array) {
  const v = [...point, 1];
  const p = [0, 0, 0, 0];
  for (let r = 0; r < 4; r++)
    for (let k = 0; k < 4; k++) p[r] += matrix[k * 4 + r] * v[k];
  return {
    x: (p[0] / p[3]) * 0.5 + 0.5,
    y: 0.5 - (p[1] / p[3]) * 0.5,
    visible: p[3] > 0.1 && p[2] > 0,
  };
}
export function geometry() {
  const triangles: number[] = [],
    lines: number[] = [];
  function vertex(out: number[], p: Vec3, n: Vec3, c: Vec3, kind: number) {
    out.push(...p, ...n, ...c, kind);
  }
  function sphere(
    center: Vec3,
    r: number,
    color: Vec3,
    kind: number,
    steps = 36,
    rows = 20,
  ) {
    const point = (a: number, b: number): Vec3 => [
      Math.cos(a) * Math.sin(b),
      Math.cos(b),
      Math.sin(a) * Math.sin(b),
    ];
    for (let j = 0; j < rows; j++)
      for (let i = 0; i < steps; i++) {
        const a = (i / steps) * Math.PI * 2,
          b = (j / rows) * Math.PI;
        const corners = [
          point(a, b),
          point(a + (Math.PI * 2) / steps, b),
          point(a, b + Math.PI / rows),
          point(a + (Math.PI * 2) / steps, b + Math.PI / rows),
        ];
        for (const idx of [0, 2, 1, 1, 2, 3]) {
          const n = corners[idx];
          vertex(
            triangles,
            [center[0] + n[0] * r, center[1] + n[1] * r, center[2] + n[2] * r],
            n,
            color,
            kind,
          );
        }
      }
  }
  for (let w = 0; w < worlds.length; w++) {
    const world = worlds[w];
    sphere(world.position, world.radius, world.color, w);
    for (let ring = 0; ring < 3; ring++)
      for (let i = 0; i < 144; i++) {
        const radius = world.ring + ring * 0.25;
        for (const phase of [
          (i / 144) * Math.PI * 2,
          ((i + 1) / 144) * Math.PI * 2,
        ])
          vertex(
            lines,
            [
              world.position[0] + Math.cos(phase) * radius,
              world.position[1] - 0.25 + Math.sin(phase) * radius * 0.13,
              world.position[2] + Math.sin(phase) * radius,
            ],
            [0, 1, 0],
            world.color,
            10 + ring,
          );
      }
    for (let i = 0; i < 32; i++) {
      const a = (i / 32) * Math.PI * 2 + w * 0.37,
        r = world.ring + 0.5 + (i % 3) * 0.17;
      sphere(
        [
          world.position[0] + Math.cos(a) * r,
          world.position[1] - 0.1 + Math.sin(a) * 0.15,
          world.position[2] + Math.sin(a) * r,
        ],
        i % 7 === 0 ? 0.055 : 0.026,
        world.color,
        20,
        5,
        3,
      );
    }
    if (w > 0)
      for (const p of [[0, 0, 0], world.position] as Vec3[])
        vertex(lines, p, [0, 1, 0], [0.31, 0.42, 0.49], 12);
  }
  return {
    triangles: new Float32Array(triangles),
    lines: new Float32Array(lines),
  };
}
