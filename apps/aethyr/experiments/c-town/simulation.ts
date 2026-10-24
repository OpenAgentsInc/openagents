/** Experimental packed movement. Rust remains the authoritative command validator. */
export type SimulationKind = "c" | "rust-packed";
export type SimulationInput = {
  forward?: number;
  right?: number;
  up?: number;
  yawDelta?: number;
  pitchDelta?: number;
  sprint?: boolean;
  jump?: boolean;
};
export type SimulationCamera = {
  position: number[];
  yaw: number;
  pitch: number;
  mode: "fly" | "walk";
};
type SimExports = WebAssembly.Exports & {
  memory: WebAssembly.Memory;
  sim_state_ptr(): number;
  sim_input_ptr(): number;
  sim_aabbs_ptr(): number;
  sim_solids_ptr(): number;
  sim_supports_ptr(): number;
  sim_ramps_ptr(): number;
  sim_reset(): void;
  sim_set_camera(): number;
  sim_step(): number;
  sim_set_world(a: number, s: number, u: number, r: number): number;
  sim_speed_scale?(): number;
};
type PackedWorld = { arrays: Float32Array[]; counts: number[]; ground: number };
const capacities = [32768, 131072, 65536, 8192];
const strides = [6, 12, 11, 13];
const stateBytes = 64;
const f32 = Math.fround;

function values(v: unknown, length: number): number[] {
  if (
    !Array.isArray(v) ||
    v.length !== length ||
    v.some(
      (x) =>
        typeof x !== "number" || !Number.isFinite(x) || Math.abs(x) > 100000,
    )
  )
    throw new Error("The movement world contains invalid geometry.");
  return v.map(f32);
}
function transform(v: any): number[] {
  if (!v) return [0, 0, 0, 1, 0, 1];
  const position = values(v.position, 3),
    scale = f32(v.scale ?? 1),
    yaw = f32(v.yaw ?? 0);
  if (
    !Number.isFinite(scale) ||
    scale < 0.05 ||
    scale > 20 ||
    !Number.isFinite(yaw)
  )
    throw new Error("The movement world has an invalid transform.");
  return [...position, scale, f32(Math.sin(yaw)), f32(Math.cos(yaw))];
}
function bounds(min: unknown, max: unknown): number[] {
  const lo = values(min, 3),
    hi = values(max, 3);
  if (lo.some((v, i) => v >= hi[i]))
    throw new Error("The movement world contains an empty collider.");
  return [...lo, ...hi];
}

/** Pack only a complete snapshot returned by the existing authoritative kernel. */
export function packSimulationWorld(snapshot: any): PackedWorld {
  if (!snapshot || snapshot.ok !== true)
    throw new Error("A complete working world is required for movement.");
  const ground = f32(snapshot.groundY ?? 0);
  if (!Number.isFinite(ground))
    throw new Error("The movement world has an invalid ground height.");
  const lists: number[][] = [[], [], [], []];
  for (const a of snapshot.colliders ?? [])
    lists[0].push(...bounds(a.min, a.max));
  for (const e of snapshot.entities ?? [])
    if (e.collidable && e.halfExtents) {
      const [x, y, z] = values(e.position, 3),
        [hx, hy, hz] = values(e.halfExtents, 3);
      const scale = f32(e.scale ?? 1),
        yaw = f32(e.yaw ?? 0),
        s = f32(Math.sin(yaw)),
        c = f32(Math.cos(yaw));
      const ex = f32(
        f32(f32(hx * Math.abs(c)) + f32(hz * Math.abs(s))) * scale,
      );
      const ez = f32(
        f32(f32(hz * Math.abs(c)) + f32(hx * Math.abs(s))) * scale,
      );
      lists[0].push(
        ...bounds(
          [f32(x - ex), y, f32(z - ez)],
          [f32(x + ex), f32(y + f32(f32(hy * 2) * scale)), f32(z + ez)],
        ),
      );
    }
  for (const w of snapshot.wallColliders ?? []) {
    if (w.transform && w.localMin && w.localMax)
      lists[1].push(
        ...transform(w.transform),
        ...bounds(w.localMin, w.localMax),
      );
    else lists[0].push(...bounds(w.min, w.max));
  }
  for (const s of snapshot.supports ?? []) {
    const center = values(s.center, 2),
      size = values(s.size, 2),
      floor = f32(s.floorY);
    if (!Number.isFinite(floor) || size.some((v) => v <= 0))
      throw new Error("The movement world has an invalid floor.");
    const t = transform(s.transform);
    lists[2].push(...t, ...center, ...size, floor);
    lists[1].push(
      ...t,
      ...bounds(
        [
          f32(center[0] - size[0] / 2),
          f32(floor - f32(0.12)),
          f32(center[1] - size[1] / 2),
        ],
        [f32(center[0] + size[0] / 2), floor, f32(center[1] + size[1] / 2)],
      ),
    );
  }
  for (const r of snapshot.ramps ?? []) {
    const from = values(r.from, 3),
      to = values(r.to, 3),
      width = f32(r.width);
    const dx = to[0] - from[0],
      dz = to[2] - from[2];
    if (!Number.isFinite(width) || width <= 0 || dx * dx + dz * dz < 0.000001)
      throw new Error("The movement world has invalid stairs.");
    lists[3].push(...transform(r.transform), ...from, ...to, width);
  }
  const counts = lists.map((a, i) => a.length / strides[i]);
  if (counts.some((n, i) => !Number.isInteger(n) || n > capacities[i]))
    throw new Error("The movement world exceeds this experiment's limits.");
  return { arrays: lists.map((a) => new Float32Array(a)), counts, ground };
}

export class PackedSimulation {
  private reloadGeneration = 0;
  private world: PackedWorld = {
    arrays: strides.map(() => new Float32Array()),
    counts: [0, 0, 0, 0],
    ground: 0,
  };
  private constructor(
    readonly kind: SimulationKind,
    private e: SimExports,
  ) {
    e.sim_reset();
  }

  static async create(kind: SimulationKind, bytes: BufferSource) {
    const { instance } = await WebAssembly.instantiate(bytes, {
      env: {
        af_sin: (v: number) => Math.sin(v),
        af_cos: (v: number) => Math.cos(v),
        af_rem: (a: number, b: number) => a % b,
      },
    });
    const e = instance.exports as SimExports;
    for (const name of [
      "sim_state_ptr",
      "sim_input_ptr",
      "sim_aabbs_ptr",
      "sim_solids_ptr",
      "sim_supports_ptr",
      "sim_ramps_ptr",
      "sim_reset",
      "sim_set_camera",
      "sim_step",
      "sim_set_world",
    ])
      if (typeof e[name] !== "function")
        throw new Error("The movement module has an unsupported interface.");
    if (!(e.memory instanceof WebAssembly.Memory))
      throw new Error("The movement module has no memory.");
    const result = new PackedSimulation(kind, e);
    if (
      result.view().getUint32(0, true) !== 1 ||
      result.view().getUint32(4, true) !== stateBytes
    )
      throw new Error("The movement module uses a different state version.");
    return result;
  }
  private view() {
    return new DataView(
      this.e.memory.buffer,
      this.e.sim_state_ptr(),
      stateBytes,
    );
  }
  private installWorld(world: PackedWorld) {
    const pointers = [
      this.e.sim_aabbs_ptr(),
      this.e.sim_solids_ptr(),
      this.e.sim_supports_ptr(),
      this.e.sim_ramps_ptr(),
    ];
    for (let i = 0; i < pointers.length; i++)
      new Float32Array(
        this.e.memory.buffer,
        pointers[i],
        world.arrays[i].length,
      ).set(world.arrays[i]);
    if (
      !this.e.sim_set_world(
        world.counts[0],
        world.counts[1],
        world.counts[2],
        world.counts[3],
      )
    )
      throw new Error("The movement module rejected this world.");
    this.view().setFloat32(32, world.ground, true);
    this.world = world;
  }
  syncWorld(snapshot: any) {
    this.installWorld(packSimulationWorld(snapshot));
  }
  validateWorld(snapshot: any) {
    packSimulationWorld(snapshot);
  }
  initializeWorld(snapshot: any) {
    const world = packSimulationWorld(snapshot);
    this.e.sim_reset();
    this.installWorld(world);
    this.setCamera(snapshot.player, snapshot.player.grounded);
  }
  setCamera(camera: SimulationCamera, grounded?: boolean) {
    const previous = this.snapshot(),
      v = this.view();
    const p = values(camera.position, 3);
    for (let i = 0; i < 3; i++) v.setFloat32(8 + i * 4, p[i], true);
    v.setFloat32(20, camera.yaw, true);
    v.setFloat32(24, camera.pitch, true);
    v.setUint32(
      52,
      camera.mode === "walk" ? 1 : camera.mode === "fly" ? 0 : 2,
      true,
    );
    if (!this.e.sim_set_camera()) {
      this.restore(previous);
      throw new Error("Choose an open position above the ground.");
    }
    if (grounded !== undefined)
      this.view().setUint32(48, grounded ? 1 : 0, true);
  }
  replaceContentCamera(camera: SimulationCamera, grounded: boolean) {
    const saved = new DataView(this.snapshot().buffer);
    this.setCamera(camera, grounded);
    const v = this.view();
    // The command authority resets velocity, but preserves tick timing and jump edges.
    v.setFloat32(36, saved.getFloat32(36, true), true);
    v.setUint32(56, saved.getUint32(56, true), true);
    v.setUint32(60, saved.getUint32(60, true), true);
  }
  writeInput(elapsed: number, input: SimulationInput = {}) {
    const v = new DataView(this.e.memory.buffer, this.e.sim_input_ptr(), 32);
    const fields = [
      input.forward ?? 0,
      input.right ?? 0,
      input.up ?? 0,
      input.yawDelta ?? 0,
      input.pitchDelta ?? 0,
      elapsed,
    ];
    for (let i = 0; i < fields.length; i++)
      v.setFloat32(i * 4, fields[i], true);
    v.setUint32(24, input.sprint ? 1 : 0, true);
    v.setUint32(28, input.jump ? 1 : 0, true);
  }
  /** Benchmark helper: step already-written packed input without allocating a reply. */
  advance() {
    if (!this.e.sim_step())
      throw new Error("Movement values must be finite and within range.");
  }
  step(elapsed: number, input: SimulationInput = {}) {
    this.writeInput(elapsed, input);
    this.advance();
    return this.reply();
  }
  reply() {
    const v = this.view(),
      position = [8, 12, 16].map((offset) => v.getFloat32(offset, true));
    return {
      ok: true,
      type: "step",
      tick: v.getUint32(44, true),
      droppedSeconds: v.getFloat32(40, true),
      player: {
        position,
        eye: [position[0], f32(position[1] + f32(1.62)), position[2]],
        yaw: v.getFloat32(20, true),
        pitch: v.getFloat32(24, true),
        grounded: !!v.getUint32(48, true),
        mode: v.getUint32(52, true) ? "walk" : "fly",
      },
    };
  }
  snapshot() {
    return new Uint8Array(
      this.e.memory.buffer,
      this.e.sim_state_ptr(),
      stateBytes,
    ).slice();
  }
  restore(bytes: Uint8Array) {
    if (bytes.byteLength !== stateBytes)
      throw new Error("The saved movement state has the wrong size.");
    const v = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    if (v.getUint32(0, true) !== 1 || v.getUint32(4, true) !== stateBytes)
      throw new Error("The saved movement state uses a different version.");
    for (let o = 8; o <= 40; o += 4)
      if (!Number.isFinite(v.getFloat32(o, true)))
        throw new Error("The saved movement state contains an invalid number.");
    for (let o = 48; o <= 60; o += 4)
      if (v.getUint32(o, true) > 1)
        throw new Error("The saved movement state has an invalid flag.");
    if (
      v.getFloat32(36, true) < 0 ||
      v.getFloat32(36, true) > 1 / 60 + 1e-6 ||
      v.getFloat32(40, true) < 0
    )
      throw new Error("The saved movement clock is invalid.");
    new Uint8Array(
      this.e.memory.buffer,
      this.e.sim_state_ptr(),
      stateBytes,
    ).set(bytes);
  }
  /** Build a candidate first. A failed instantiate or restore leaves this module active. */
  async reload(bytes: BufferSource) {
    const generation = ++this.reloadGeneration;
    const candidate = await PackedSimulation.create(this.kind, bytes);
    if (generation !== this.reloadGeneration)
      throw new Error("A newer movement module was requested.");
    // These operations run without an await, so input cannot advance between them.
    candidate.installWorld(this.world);
    candidate.restore(this.snapshot());
    this.e = candidate.e;
  }
  getSpeedScale() {
    return this.e.sim_speed_scale?.() ?? 1;
  }
  memoryBytes() {
    return this.e.memory.buffer.byteLength;
  }
}

export const createPackedSimulation = PackedSimulation.create;
