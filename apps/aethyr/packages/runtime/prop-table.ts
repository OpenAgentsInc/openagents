/** Derived prop data. The authored document and Rust kernel remain authoritative. */
export type PropVector = [number, number, number];
export interface PropEntity {
  id: string;
  kind: string;
  position: PropVector;
  yaw?: number;
  scale?: number;
  collidable?: boolean;
  halfExtents?: PropVector | null;
  /** Absolute local center; omission preserves the legacy floor-pivot collider. */
  collisionOffset?: PropVector | null;
}
export interface PropHandle {
  slot: number;
  generation: number;
}
export interface PropAabb {
  min: PropVector;
  max: PropVector;
}
export interface PropRecord {
  id: string;
  kind: string;
  handle: PropHandle;
  matrix: Float32Array<ArrayBuffer>;
  physics: PropAabb | null;
  selection: {
    position: PropVector;
    yaw: number;
    scale: number;
    bounds: PropAabb | null;
  };
}
export interface PropWrite {
  readonly id: string;
  readonly slot: number;
  readonly handle: Readonly<PropHandle>;
  /** Read-only borrowed staging view, valid only while its plan is ready. */
  readonly matrix: Float32Array<ArrayBuffer>;
}
export interface PropRemoval {
  readonly id: string;
  readonly slot: number;
  readonly handle: Readonly<PropHandle>;
}
export interface PropPlan {
  readonly baseVersion: number;
  readonly ticket: number;
  readonly capacity: number;
  readonly count: number;
  /** Membership or kind changed; transform-only edits leave draw membership intact. */
  readonly structural: boolean;
  readonly writes: readonly PropWrite[];
  readonly removed: readonly PropRemoval[];
  ready(): boolean;
  activate(): void;
  discard(): void;
  handle(id: string): PropHandle | undefined;
  records(): PropRecord[];
  /** Explicit full-copy path for a complete renderer rebuild. */
  matrixRecords(): Float32Array<ArrayBuffer>;
}
export interface PropTableSnapshot {
  version: number;
  desiredTicket: number;
  liveRecords: number;
  capacity: number;
  recordsValidated: number;
  recordsRebuilt: number;
  matricesRebuilt: number;
  physicsRebuilt: number;
  selectionRebuilt: number;
  /** Typed backing-buffer allocations, including scratch and explicit inspection copies. */
  bytesAllocated: number;
  /** Typed-buffer copies; excludes JavaScript metadata and arithmetic writes. */
  bytesCopied: number;
  /** Dirty matrix payload of accepted plans; actual GPU transfers belong to the renderer. */
  uploadBytes: number;
  structuralChanges: number;
  /** Owned resident and staging typed buffers; excludes JavaScript and GPU memory. */
  heldBytes: number;
}
interface NormalizedProp {
  id: string;
  kind: string;
  position: PropVector;
  yaw: number;
  scale: number;
  collidable: boolean;
  halfExtents?: PropVector;
  collisionOffset?: PropVector;
}
interface Metadata {
  entity: NormalizedProp;
  slot: number;
  generation: number;
}
interface Bank {
  capacity: number;
  matrices: Float32Array<ArrayBuffer>;
  bounds: Float32Array<ArrayBuffer>;
  poses: Float32Array<ArrayBuffer>;
  generations: Uint32Array<ArrayBuffer>;
  flags: Uint8Array<ArrayBuffer>;
}
interface Scratch {
  capacity: number;
  matrices: Float32Array<ArrayBuffer>;
  bounds: Float32Array<ArrayBuffer>;
  poses: Float32Array<ArrayBuffer>;
}
interface Change {
  meta: Metadata;
  matrix: boolean;
  bounds: boolean;
  pose: boolean;
  physics: boolean;
}
interface Staged {
  ticket: number;
  baseVersion: number;
  capacity: number;
  next: Map<string, Metadata>;
  changes: Change[];
  changeBySlot: Map<number, Change>;
  removed: Metadata[];
  structuralChanges: number;
  grown?: Bank;
  used: boolean;
}
const MAX_CAPACITY = 16_384;
const MAX_GENERATION = 0xffff_ffff;
const ACTIVE = 1,
  BOUNDS = 2,
  COLLIDABLE = 4;
const f32 = Math.fround;
const record = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);
function vector(value: unknown, label: string): PropVector {
  if (
    !Array.isArray(value) ||
    value.length !== 3 ||
    value.some(
      (n) =>
        typeof n !== "number" ||
        !Number.isFinite(f32(n)) ||
        Math.abs(f32(n)) > 100_000,
    )
  )
    throw new Error(`Choose a finite ${label} within the supported world.`);
  return value.map(f32) as PropVector;
}
function normalize(value: unknown): NormalizedProp {
  if (
    !record(value) ||
    typeof value.id !== "string" ||
    !/^[a-zA-Z0-9_.:-]{1,96}$/.test(value.id) ||
    typeof value.kind !== "string" ||
    value.kind.length < 1 ||
    new TextEncoder().encode(value.kind).length > 128 ||
    (value.collidable !== undefined && typeof value.collidable !== "boolean")
  )
    throw new Error("Choose props with valid, distinct IDs and asset names.");
  const position = vector(value.position, "prop position");
  const yaw = value.yaw === undefined ? 0 : value.yaw;
  const scale = value.scale === undefined ? 1 : value.scale;
  if (
    typeof yaw !== "number" ||
    !Number.isFinite(f32(yaw)) ||
    typeof scale !== "number" ||
    !Number.isFinite(f32(scale)) ||
    f32(scale) < f32(0.05) ||
    f32(scale) > 20
  )
    throw new Error(
      "Choose a finite prop rotation and a size from 0.05 to 20.",
    );
  const halfExtents =
    value.halfExtents == null
      ? undefined
      : vector(value.halfExtents, "collision size");
  if (halfExtents?.some((n) => n <= 0 || n > 1000))
    throw new Error("Choose positive collision half sizes up to 1,000 meters.");
  const collisionOffset =
    value.collisionOffset == null
      ? undefined
      : vector(value.collisionOffset, "collision center");
  if (value.collidable && !halfExtents)
    throw new Error("Solid props need collision half sizes.");
  return {
    id: value.id,
    kind: value.kind,
    position,
    yaw: f32(yaw),
    scale: f32(scale),
    collidable: value.collidable === true,
    ...(halfExtents ? { halfExtents } : {}),
    ...(collisionOffset ? { collisionOffset } : {}),
  };
}
const sameVector = (a?: PropVector, b?: PropVector) =>
  a === b || (!!a && !!b && a.every((n, i) => n === b[i]));
const samePose = (a: NormalizedProp, b: NormalizedProp) =>
  sameVector(a.position, b.position) && a.yaw === b.yaw && a.scale === b.scale;
const sameBounds = (a: NormalizedProp, b: NormalizedProp) =>
  samePose(a, b) &&
  sameVector(a.halfExtents, b.halfExtents) &&
  sameVector(a.collisionOffset, b.collisionOffset);
const sameEntity = (a: NormalizedProp, b: NormalizedProp) =>
  a.id === b.id &&
  a.kind === b.kind &&
  sameBounds(a, b) &&
  a.collidable === b.collidable;
const bytes = (bank: Scratch | Bank) =>
  bank.matrices.byteLength +
  bank.bounds.byteLength +
  bank.poses.byteLength +
  ("generations" in bank
    ? bank.generations.byteLength + bank.flags.byteLength
    : 0);
function powerOfTwo(value: number) {
  return (
    Number.isSafeInteger(value) &&
    value > 0 &&
    value <= MAX_CAPACITY &&
    (value & (value - 1)) === 0
  );
}
const handle = (meta: Metadata): PropHandle => ({
  slot: meta.slot,
  generation: meta.generation,
});

/** One resident table and one reusable staging bank; no per-frame UI state or world authority. */
export class RuntimePropTable {
  readonly #maxCapacity: number;
  #bank: Bank;
  #scratch: Scratch;
  #byId = new Map<string, Metadata>();
  #slots: (Metadata | undefined)[];
  #pending?: Staged;
  #version = 0;
  #ticket = 0;
  #counters = {
    recordsValidated: 0,
    recordsRebuilt: 0,
    matricesRebuilt: 0,
    physicsRebuilt: 0,
    selectionRebuilt: 0,
    bytesAllocated: 0,
    bytesCopied: 0,
    uploadBytes: 0,
    structuralChanges: 0,
  };
  constructor(
    options: { initialCapacity?: number; maxCapacity?: number } = {},
  ) {
    const initial = options.initialCapacity ?? 32;
    this.#maxCapacity = options.maxCapacity ?? MAX_CAPACITY;
    if (
      !powerOfTwo(initial) ||
      !powerOfTwo(this.#maxCapacity) ||
      initial > this.#maxCapacity
    )
      throw new Error("Choose power-of-two prop capacities from 1 to 16,384.");
    this.#bank = this.#allocate(initial);
    this.#scratch = this.#allocateScratch(initial);
    this.#slots = new Array(initial);
  }
  snapshot(): PropTableSnapshot {
    return {
      version: this.#version,
      desiredTicket: this.#ticket,
      liveRecords: this.#byId.size,
      capacity: this.#bank.capacity,
      ...this.#counters,
      heldBytes:
        bytes(this.#bank) +
        bytes(this.#scratch) +
        (this.#pending?.grown ? bytes(this.#pending.grown) : 0),
    };
  }
  handle(id: string): PropHandle | undefined {
    const meta = this.#byId.get(id);
    return meta && handle(meta);
  }
  resolve(candidate: PropHandle): PropRecord | undefined {
    if (
      !candidate ||
      !Number.isSafeInteger(candidate.slot) ||
      candidate.slot < 0 ||
      candidate.slot >= this.#bank.capacity ||
      !Number.isSafeInteger(candidate.generation) ||
      candidate.generation < 1 ||
      candidate.generation > MAX_GENERATION
    )
      return undefined;
    const meta = this.#slots[candidate.slot];
    return meta &&
      meta.generation === candidate.generation &&
      this.#bank.generations[candidate.slot] === candidate.generation &&
      this.#bank.flags[candidate.slot] & ACTIVE
      ? this.#copy(meta)
      : undefined;
  }
  records(): PropRecord[] {
    return [...this.#byId.values()]
      .sort((a, b) => a.slot - b.slot)
      .map((meta) => this.#copy(meta));
  }
  prepare(entities: readonly PropEntity[]): PropPlan {
    const ticket = ++this.#ticket;
    if (this.#pending) this.#pending.grown = undefined;
    this.#pending = undefined;
    if (!Array.isArray(entities) || entities.length > this.#maxCapacity)
      throw new Error("This prop table has reached its capacity.");
    const normalized = new Map<string, NormalizedProp>();
    for (const candidate of entities) {
      const entity = normalize(candidate);
      this.#counters.recordsValidated++;
      if (normalized.has(entity.id))
        throw new Error("Use each authored prop ID once.");
      normalized.set(entity.id, entity);
    }
    const available: number[] = [];
    for (let slot = 0; slot < this.#bank.capacity; slot++)
      if (
        (!this.#slots[slot] || !normalized.has(this.#slots[slot]!.entity.id)) &&
        this.#bank.generations[slot] < MAX_GENERATION
      )
        available.push(slot);
    let nextSlot = this.#bank.capacity,
      freeIndex = 0,
      capacity = this.#bank.capacity;
    const next = new Map<string, Metadata>(),
      changes: Change[] = [];
    const removed = [...this.#byId.values()].filter(
      (meta) => !normalized.has(meta.entity.id),
    );
    let structuralChanges = removed.length;
    for (const entity of normalized.values()) {
      const previous = this.#byId.get(entity.id);
      if (previous && sameEntity(previous.entity, entity)) {
        next.set(entity.id, previous);
        continue;
      }
      let slot: number, generation: number;
      if (previous) {
        slot = previous.slot;
        generation = previous.generation;
      } else {
        slot = available[freeIndex++] ?? nextSlot++;
        if (slot >= this.#maxCapacity)
          throw new Error("This prop table has no reusable slots left.");
        generation = (this.#bank.generations[slot] ?? 0) + 1;
        while (capacity <= slot) capacity *= 2;
        structuralChanges++;
      }
      if (previous && previous.entity.kind !== entity.kind) structuralChanges++;
      const meta = { entity, slot, generation };
      const pose = !previous || !samePose(previous.entity, entity);
      const bounds = !previous || !sameBounds(previous.entity, entity);
      changes.push({
        meta,
        matrix: pose,
        pose,
        bounds,
        physics: bounds || previous?.entity.collidable !== entity.collidable,
      });
      next.set(entity.id, meta);
    }
    if (capacity > this.#scratch.capacity)
      this.#scratch = this.#allocateScratch(capacity);
    const grown =
      capacity > this.#bank.capacity
        ? this.#allocate(capacity, this.#bank)
        : undefined;
    const staged: Staged = {
      ticket,
      baseVersion: this.#version,
      capacity,
      next,
      changes,
      changeBySlot: new Map(
        changes.map((change) => [change.meta.slot, change]),
      ),
      removed,
      structuralChanges,
      grown,
      used: false,
    };
    // All inputs are validated before any derived scratch record is written.
    for (const change of changes) this.#derive(change);
    this.#pending = staged;
    const ready = () =>
      !staged.used &&
      this.#pending === staged &&
      this.#ticket === staged.ticket &&
      this.#version === staged.baseVersion;
    const requireReady = () => {
      if (!ready())
        throw new Error(
          "A newer prop change replaced this candidate, or it was already used.",
        );
    };
    const writes = Object.freeze(
      changes
        .filter((change) => change.matrix)
        .map(({ meta }) =>
          Object.freeze({
            id: meta.entity.id,
            slot: meta.slot,
            handle: Object.freeze(handle(meta)),
            get matrix() {
              requireReady();
              return thisTable.#scratch.matrices.subarray(
                meta.slot * 16,
                meta.slot * 16 + 16,
              );
            },
          }),
        ),
    );
    const removals = Object.freeze(
      removed.map((meta) =>
        Object.freeze({
          id: meta.entity.id,
          slot: meta.slot,
          handle: Object.freeze(handle(meta)),
        }),
      ),
    );
    const thisTable = this;
    const plan: PropPlan = {
      baseVersion: staged.baseVersion,
      ticket,
      capacity,
      count: next.size,
      structural: structuralChanges > 0,
      get writes() {
        requireReady();
        return writes;
      },
      removed: removals,
      ready,
      handle: (id) => {
        requireReady();
        const meta = next.get(id);
        return meta && handle(meta);
      },
      records: () => {
        requireReady();
        return [...next.values()]
          .sort((a, b) => a.slot - b.slot)
          .map((meta) => this.#copyStaged(meta, staged));
      },
      matrixRecords: () => {
        requireReady();
        return this.#matrixSnapshot(staged);
      },
      activate: () => {
        requireReady();
        const bank = staged.grown ?? this.#bank;
        for (const meta of removed) {
          bank.flags[meta.slot] = 0;
          bank.matrices.fill(0, meta.slot * 16, meta.slot * 16 + 16);
          bank.bounds.fill(0, meta.slot * 6, meta.slot * 6 + 6);
          bank.poses.fill(0, meta.slot * 5, meta.slot * 5 + 5);
        }
        for (const change of changes) {
          const { meta } = change,
            slot = meta.slot;
          if (change.matrix) {
            bank.matrices.set(
              this.#scratch.matrices.subarray(slot * 16, slot * 16 + 16),
              slot * 16,
            );
            this.#counters.bytesCopied += 64;
          }
          if (change.bounds) {
            bank.bounds.set(
              this.#scratch.bounds.subarray(slot * 6, slot * 6 + 6),
              slot * 6,
            );
            this.#counters.bytesCopied += 24;
          }
          if (change.pose) {
            bank.poses.set(
              this.#scratch.poses.subarray(slot * 5, slot * 5 + 5),
              slot * 5,
            );
            this.#counters.bytesCopied += 20;
          }
          bank.generations[slot] = meta.generation;
          bank.flags[slot] =
            ACTIVE |
            (meta.entity.halfExtents ? BOUNDS : 0) |
            (meta.entity.collidable ? COLLIDABLE : 0);
        }
        this.#bank = bank;
        this.#byId = next;
        this.#slots.length = capacity;
        for (const meta of removed) this.#slots[meta.slot] = undefined;
        for (const change of changes)
          this.#slots[change.meta.slot] = change.meta;
        this.#version++;
        this.#counters.uploadBytes += writes.length * 64;
        this.#counters.structuralChanges += structuralChanges;
        staged.used = true;
        staged.grown = undefined;
        this.#pending = undefined;
      },
      discard: () => {
        if (!staged.used) {
          staged.used = true;
          staged.grown = undefined;
          if (this.#pending === staged) this.#pending = undefined;
        }
      },
    };
    return Object.freeze(plan);
  }
  #allocateScratch(capacity: number): Scratch {
    const scratch = {
      capacity,
      matrices: new Float32Array(capacity * 16),
      bounds: new Float32Array(capacity * 6),
      poses: new Float32Array(capacity * 5),
    };
    this.#counters.bytesAllocated += bytes(scratch);
    return scratch;
  }
  #allocate(capacity: number, previous?: Bank): Bank {
    const bank = {
      capacity,
      matrices: new Float32Array(capacity * 16),
      bounds: new Float32Array(capacity * 6),
      poses: new Float32Array(capacity * 5),
      generations: new Uint32Array(capacity),
      flags: new Uint8Array(capacity),
    };
    this.#counters.bytesAllocated += bytes(bank);
    if (previous) {
      bank.matrices.set(previous.matrices);
      bank.bounds.set(previous.bounds);
      bank.poses.set(previous.poses);
      bank.generations.set(previous.generations);
      bank.flags.set(previous.flags);
      this.#counters.bytesCopied += bytes(previous);
    }
    return bank;
  }
  #derive(change: Change) {
    const { entity: e, slot } = change.meta;
    this.#counters.recordsRebuilt++;
    const c = change.matrix || change.bounds ? Math.cos(e.yaw) : 0,
      s = change.matrix || change.bounds ? Math.sin(e.yaw) : 0;
    if (change.matrix) {
      const matrix = this.#scratch.matrices.subarray(slot * 16, slot * 16 + 16);
      matrix.set([
        c * e.scale,
        0,
        -s * e.scale,
        0,
        0,
        e.scale,
        0,
        0,
        s * e.scale,
        0,
        c * e.scale,
        0,
        ...e.position,
        1,
      ]);
      this.#counters.matricesRebuilt++;
    }
    if (change.pose) {
      this.#scratch.poses.set([...e.position, e.yaw, e.scale], slot * 5);
      this.#counters.selectionRebuilt++;
    }
    if (change.bounds) {
      const values = this.#scratch.bounds.subarray(slot * 6, slot * 6 + 6);
      if (!e.halfExtents) values.fill(0);
      else {
        const half = e.halfExtents,
          offset = e.collisionOffset ?? [0, half[1], 0];
        const fc = f32(c),
          fs = f32(s);
        const center: PropVector = [
          f32(
            e.position[0] +
              f32(f32(f32(offset[0] * fc) + f32(offset[2] * fs)) * e.scale),
          ),
          f32(e.position[1] + f32(offset[1] * e.scale)),
          f32(
            e.position[2] +
              f32(f32(f32(-offset[0] * fs) + f32(offset[2] * fc)) * e.scale),
          ),
        ];
        const extent: PropVector = [
          f32(
            f32(f32(half[0] * Math.abs(fc)) + f32(half[2] * Math.abs(fs))) *
              e.scale,
          ),
          f32(half[1] * e.scale),
          f32(
            f32(f32(half[2] * Math.abs(fc)) + f32(half[0] * Math.abs(fs))) *
              e.scale,
          ),
        ];
        for (let axis = 0; axis < 3; axis++) {
          values[axis] = f32(center[axis] - extent[axis]);
          values[axis + 3] = f32(center[axis] + extent[axis]);
          if (
            !Number.isFinite(values[axis]) ||
            !Number.isFinite(values[axis + 3]) ||
            values[axis] >= values[axis + 3]
          )
            throw new Error("This prop has no representable collision bounds.");
        }
      }
      if (!change.pose) this.#counters.selectionRebuilt++;
    }
    if (change.physics) this.#counters.physicsRebuilt++;
  }
  #matrixSnapshot(staged: Staged) {
    const matrix = new Float32Array(staged.capacity * 16);
    this.#counters.bytesAllocated += matrix.byteLength;
    matrix.set(this.#bank.matrices);
    this.#counters.bytesCopied += this.#bank.matrices.byteLength;
    for (const meta of staged.removed)
      matrix.fill(0, meta.slot * 16, meta.slot * 16 + 16);
    for (const change of staged.changes)
      if (change.matrix) {
        matrix.set(
          this.#scratch.matrices.subarray(
            change.meta.slot * 16,
            change.meta.slot * 16 + 16,
          ),
          change.meta.slot * 16,
        );
        this.#counters.bytesCopied += 64;
      }
    return matrix;
  }
  #copy(meta: Metadata): PropRecord {
    return this.#record(
      meta,
      this.#bank.matrices,
      this.#bank.bounds,
      this.#bank.poses,
    );
  }
  #copyStaged(meta: Metadata, staged: Staged): PropRecord {
    const change = staged.changeBySlot.get(meta.slot);
    return this.#record(
      meta,
      change?.matrix ? this.#scratch.matrices : this.#bank.matrices,
      change?.bounds ? this.#scratch.bounds : this.#bank.bounds,
      change?.pose ? this.#scratch.poses : this.#bank.poses,
    );
  }
  #record(
    meta: Metadata,
    matrices: Float32Array<ArrayBuffer>,
    bounds: Float32Array<ArrayBuffer>,
    poses: Float32Array<ArrayBuffer>,
  ): PropRecord {
    const slot = meta.slot,
      matrix = matrices.slice(slot * 16, slot * 16 + 16);
    this.#counters.bytesAllocated += matrix.byteLength;
    this.#counters.bytesCopied += matrix.byteLength;
    const aabb = (): PropAabb => ({
      min: Array.from(bounds.subarray(slot * 6, slot * 6 + 3)) as PropVector,
      max: Array.from(
        bounds.subarray(slot * 6 + 3, slot * 6 + 6),
      ) as PropVector,
    });
    return {
      id: meta.entity.id,
      kind: meta.entity.kind,
      handle: handle(meta),
      matrix,
      physics:
        meta.entity.collidable && meta.entity.halfExtents ? aabb() : null,
      selection: {
        position: Array.from(
          poses.subarray(slot * 5, slot * 5 + 3),
        ) as PropVector,
        yaw: poses[slot * 5 + 3],
        scale: poses[slot * 5 + 4],
        bounds: meta.entity.halfExtents ? aabb() : null,
      },
    };
  }
}
