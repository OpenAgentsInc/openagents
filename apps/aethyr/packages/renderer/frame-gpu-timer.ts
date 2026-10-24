import type { RendererGpuSlot } from "./feature-contract.ts";

const MAX_PASSES = 256;
const QUERY_COUNT = MAX_PASSES * 2;
const QUERY_BYTES = QUERY_COUNT * 8;
type PassRecord = {
  label: string;
  kind: "render" | "compute";
  begin: number;
  end: number;
  nonempty: boolean;
  closed: boolean;
};
type SlotState = {
  generation: number;
  next: number;
  passes: PassRecord[];
  total: number;
  overflow: number;
  foreign: number;
  hdrBegin: boolean;
  hdrEnd: boolean;
  duplicateHdr: boolean;
};
export interface FrameGpuTimingStatus {
  scope: "render-compute-pass-envelope";
  availability:
    "not-sampled" | "pending" | "available" | "partial" | "unavailable";
  observedFrames: number;
  validFrames: number;
  unavailableFrames: number;
  pendingFrames: number;
  lastFrame: number | null;
  excludes: string[];
  lastCoverage: null | {
    passes: number;
    nonemptyPasses: number;
    measuredPasses: number;
    invalidPasses: number;
    overflow: number;
    foreignTimestampPasses: number;
    labels: string[];
    reasons: string[];
    /** Per-pass GPU durations can overlap; their sum is not frame wall time. */
    durations: {
      label: string;
      kind: "render" | "compute";
      nonempty: boolean;
      ms: number | null;
      startMs: number | null;
      endMs: number | null;
      reason: string | null;
    }[];
  };
}

/** Asynchronous timing of actual passes; never inserts synthetic GPU work. */
export class FrameGpuTimer {
  private slots: RendererGpuSlot[];
  private states = new WeakMap<RendererGpuSlot, SlotState>();
  private values = new Map<number, number>();
  private frameValues = new Map<number, number>();
  private generation = 0;
  private disposed = false;
  private observed = 0;
  private valid = 0;
  private unavailable = 0;
  private pending = 0;
  private lastFrame: number | null = null;
  private lastCoverage: FrameGpuTimingStatus["lastCoverage"] = null;

  constructor(private device: GPUDevice) {
    this.slots = Array.from({ length: 8 }, () => ({
      queries: device.createQuerySet({ type: "timestamp", count: QUERY_COUNT }),
      resolve: device.createBuffer({
        size: QUERY_BYTES,
        usage: GPUBufferUsage.QUERY_RESOLVE | GPUBufferUsage.COPY_SRC,
      }),
      read: device.createBuffer({
        size: QUERY_BYTES,
        usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
      }),
      busy: false,
    }));
  }
  acquire(): RendererGpuSlot | undefined {
    if (this.disposed) return undefined;
    const slot = this.slots.find((value) => !value.busy);
    if (slot) {
      slot.busy = true;
      this.states.set(slot, {
        generation: this.generation,
        next: 0,
        passes: [],
        total: 0,
        overflow: 0,
        foreign: 0,
        hdrBegin: false,
        hdrEnd: false,
        duplicateHdr: false,
      });
    }
    return slot;
  }
  writes(
    slot?: RendererGpuSlot,
    phase?: "begin" | "end",
  ): GPURenderPassTimestampWrites | undefined {
    return slot
      ? {
          querySet: slot.queries,
          ...(phase === "end" ? {} : { beginningOfPassWriteIndex: 2 }),
          ...(phase === "begin" ? {} : { endOfPassWriteIndex: 3 }),
        }
      : undefined;
  }
  instrumentEncoder(
    encoder: GPUCommandEncoder,
    slot?: RendererGpuSlot,
  ): GPUCommandEncoder {
    if (!slot) return encoder;
    const state = this.states.get(slot);
    if (!state)
      throw new Error(
        "Acquire a timing slot before observing graphics passes.",
      );
    const cached = new Map<PropertyKey, unknown>();
    return new Proxy(encoder, {
      get: (target, key) => {
        if (cached.has(key)) return cached.get(key);
        const value = Reflect.get(target, key, target);
        if (typeof value !== "function") return value;
        const method =
          key === "beginRenderPass" || key === "beginComputePass"
            ? (...args: unknown[]) => {
                const kind = key === "beginRenderPass" ? "render" : "compute";
                return this.beginPass(target, value, kind, args, slot, state);
              }
            : value.bind(target);
        cached.set(key, method);
        return method;
      },
      set: (target, key, value) => Reflect.set(target, key, value, target),
    });
  }
  private indices(state: SlotState, count: number) {
    const values: number[] = [];
    let next = state.next;
    while (values.length < count && next < QUERY_COUNT) {
      if (next !== 2 && next !== 3) values.push(next);
      next++;
    }
    if (values.length !== count) return undefined;
    state.next = next;
    return values;
  }
  private beginPass(
    target: GPUCommandEncoder,
    method: (...args: any[]) => any,
    kind: "render" | "compute",
    args: unknown[],
    slot: RendererGpuSlot,
    state: SlotState,
  ) {
    const original = args[0] as
      GPURenderPassDescriptor | GPUComputePassDescriptor | undefined;
    // Preserve the native error for an invalid required render descriptor.
    if (kind === "render" && !original)
      return Reflect.apply(method, target, args);
    const descriptor = original ?? {};
    state.total++;
    const existing = descriptor.timestampWrites;
    const hdrBegin =
      existing?.querySet === slot.queries &&
      existing.beginningOfPassWriteIndex === 2;
    const hdrEnd =
      existing?.querySet === slot.queries && existing.endOfPassWriteIndex === 3;
    const foreign =
      existing &&
      (existing.querySet !== slot.queries ||
        (existing.beginningOfPassWriteIndex !== undefined && !hdrBegin) ||
        (existing.endOfPassWriteIndex !== undefined && !hdrEnd));
    if (foreign) {
      state.foreign++;
      return Reflect.apply(method, target, args);
    }
    if (state.total > MAX_PASSES) {
      state.overflow++;
      return Reflect.apply(method, target, args);
    }
    const free = this.indices(state, 2 - Number(hdrBegin) - Number(hdrEnd));
    if (!free) {
      state.overflow++;
      return Reflect.apply(method, target, args);
    }
    if ((hdrBegin && state.hdrBegin) || (hdrEnd && state.hdrEnd))
      state.duplicateHdr = true;
    state.hdrBegin ||= hdrBegin;
    state.hdrEnd ||= hdrEnd;
    const begin = hdrBegin ? 2 : free.shift()!;
    const end = hdrEnd ? 3 : free.shift()!;
    const timestampWrites = {
      querySet: slot.queries,
      beginningOfPassWriteIndex: begin,
      endOfPassWriteIndex: end,
    };
    // Dictionary getters and inherited fields retain their normal receiver.
    const observedDescriptor = new Proxy(
      {},
      {
        get: (_object, key) =>
          key === "timestampWrites"
            ? timestampWrites
            : Reflect.get(descriptor, key, descriptor),
        has: (_object, key) => key === "timestampWrites" || key in descriptor,
        ownKeys: () => [
          ...new Set([...Reflect.ownKeys(descriptor), "timestampWrites"]),
        ],
        getOwnPropertyDescriptor: (_object, key) =>
          key === "timestampWrites" || Object.hasOwn(descriptor, key)
            ? {
                value:
                  key === "timestampWrites"
                    ? timestampWrites
                    : Reflect.get(descriptor, key, descriptor),
                enumerable: true,
                configurable: true,
                writable: false,
              }
            : undefined,
      },
    );
    const pass = Reflect.apply(method, target, [
      observedDescriptor,
      ...args.slice(1),
    ]);
    const render = descriptor as GPURenderPassDescriptor;
    const attachments = kind === "render" ? render.colorAttachments : undefined;
    const clear =
      kind === "render" &&
      ((Array.isArray(attachments) &&
        attachments.some((item) => item?.loadOp === "clear")) ||
        render.depthStencilAttachment?.depthLoadOp === "clear" ||
        render.depthStencilAttachment?.stencilLoadOp === "clear");
    const record: PassRecord = {
      label: String(descriptor.label ?? kind).slice(0, 96),
      kind,
      begin,
      end,
      nonempty: clear,
      closed: false,
    };
    state.passes.push(record);
    const cached = new Map<PropertyKey, unknown>();
    return new Proxy(pass, {
      get: (object, key) => {
        if (cached.has(key)) return cached.get(key);
        const value = Reflect.get(object, key, object);
        if (typeof value !== "function") return value;
        const bound = (...parameters: unknown[]) => {
          const result = Reflect.apply(value, object, parameters);
          if (key === "end") record.closed = true;
          if (key === "draw" || key === "drawIndexed")
            record.nonempty ||=
              Number(parameters[0]) > 0 && Number(parameters[1] ?? 1) > 0;
          if (
            key === "drawIndirect" ||
            key === "drawIndexedIndirect" ||
            key === "dispatchWorkgroupsIndirect"
          )
            record.nonempty = true;
          if (key === "executeBundles")
            record.nonempty ||=
              !Array.isArray(parameters[0]) || parameters[0].length > 0;
          if (key === "dispatchWorkgroups")
            record.nonempty ||=
              parameters.slice(0, 3).every((value) => Number(value ?? 1) > 0) &&
              Number(parameters[0]) > 0;
          return result;
        };
        cached.set(key, bound);
        return bound;
      },
      set: (object, key, value) => Reflect.set(object, key, value, object),
    });
  }
  resolve(encoder: GPUCommandEncoder, slot?: RendererGpuSlot) {
    if (!slot) return;
    encoder.resolveQuerySet(slot.queries, 0, QUERY_COUNT, slot.resolve, 0);
    encoder.copyBufferToBuffer(slot.resolve, 0, slot.read, 0, QUERY_BYTES);
  }
  submitted(frame: number, slot?: RendererGpuSlot) {
    if (!slot) return;
    const state = this.states.get(slot);
    if (!state) return;
    const generation = state.generation;
    if (generation === this.generation) this.pending++;
    void slot.read
      .mapAsync(GPUMapMode.READ)
      .then(() => {
        try {
          if (this.disposed || generation !== this.generation) return;
          const values = new BigUint64Array(slot.read.getMappedRange());
          this.record(frame, state, values);
        } finally {
          slot.read.unmap();
        }
      })
      .catch(() => {
        if (!this.disposed && generation === this.generation) {
          this.observed++;
          this.unavailable++;
          this.coverage(frame, state, 0, state.passes.length, ["map-failed"]);
        }
      })
      .finally(() => {
        if (generation === this.generation)
          this.pending = Math.max(0, this.pending - 1);
        slot.busy = false;
      });
  }
  private record(frame: number, state: SlotState, values: BigUint64Array) {
    this.observed++;
    const reasons: string[] = [];
    if (state.overflow) reasons.push("pass-capacity-exceeded");
    if (state.foreign) reasons.push("foreign-timestamp-writes");
    if (state.duplicateHdr) reasons.push("duplicate-hdr-timestamps");
    const hdrBegin = values[2],
      hdrEnd = values[3];
    const hdrValid =
      state.hdrBegin &&
      state.hdrEnd &&
      !state.duplicateHdr &&
      hdrBegin > 0n &&
      hdrEnd >= hdrBegin;
    const hdr = hdrValid ? Number(hdrEnd - hdrBegin) / 1e6 : null;
    if (hdr !== null && Number.isFinite(hdr)) this.values.set(frame, hdr);
    else reasons.push("missing-hdr-timestamps");
    let first: bigint | undefined,
      last: bigint | undefined,
      measured = 0,
      invalid = 0,
      active = 0;
    const durations: NonNullable<
      FrameGpuTimingStatus["lastCoverage"]
    >["durations"] = [];
    for (const pass of state.passes) {
      active += Number(pass.nonempty);
      const begin = values[pass.begin],
        end = values[pass.end];
      const valid = pass.closed && begin > 0n && end >= begin;
      durations.push({
        label: pass.label,
        kind: pass.kind,
        nonempty: pass.nonempty,
        ms: valid ? Number(end - begin) / 1e6 : null,
        startMs: null,
        endMs: null,
        reason: valid
          ? null
          : !pass.closed
            ? "pass-not-ended"
            : "invalid-timestamps",
      });
      if (!valid) {
        if (pass.nonempty) invalid++;
        continue;
      }
      measured++;
      first = first === undefined || begin < first ? begin : first;
      last = last === undefined || end > last ? end : last;
    }
    if (first !== undefined)
      for (let index = 0; index < durations.length; index++) {
        const duration = durations[index],
          pass = state.passes[index];
        if (duration.ms !== null) {
          duration.startMs = Number(values[pass.begin] - first) / 1e6;
          duration.endMs = Number(values[pass.end] - first) / 1e6;
        }
      }
    if (!active) reasons.push("no-active-passes");
    if (invalid) reasons.push("invalid-nonempty-pass-timestamps");
    const full =
      first !== undefined && last !== undefined
        ? Number(last - first) / 1e6
        : null;
    if (full === null || !Number.isFinite(full) || full <= 0)
      reasons.push("invalid-pass-envelope");
    if (full !== null && hdr !== null && full + 0.05 < hdr)
      reasons.push("envelope-shorter-than-hdr");
    if (!reasons.length && full !== null) {
      this.frameValues.set(frame, full);
      this.valid++;
    } else this.unavailable++;
    this.coverage(frame, state, measured, invalid, reasons, durations);
    if (this.values.size > 20_000)
      this.values.delete(this.values.keys().next().value!);
    if (this.frameValues.size > 20_000)
      this.frameValues.delete(this.frameValues.keys().next().value!);
  }
  private coverage(
    frame: number,
    state: SlotState,
    measured: number,
    invalid: number,
    reasons: string[],
    durations?: NonNullable<FrameGpuTimingStatus["lastCoverage"]>["durations"],
  ) {
    if (this.lastFrame !== null && frame < this.lastFrame) return;
    this.lastFrame = frame;
    this.lastCoverage = {
      passes: state.total,
      nonemptyPasses: state.passes.filter((pass) => pass.nonempty).length,
      measuredPasses: measured,
      invalidPasses: invalid,
      overflow: state.overflow,
      foreignTimestampPasses: state.foreign,
      labels: state.passes.map((pass) => pass.label),
      reasons,
      durations:
        durations ??
        state.passes.map((pass) => ({
          label: pass.label,
          kind: pass.kind,
          nonempty: pass.nonempty,
          ms: null,
          startMs: null,
          endMs: null,
          reason: "readback-unavailable",
        })),
    };
  }
  status(): FrameGpuTimingStatus {
    return {
      scope: "render-compute-pass-envelope",
      observedFrames: this.observed,
      validFrames: this.valid,
      unavailableFrames: this.unavailable,
      pendingFrames: this.pending,
      lastFrame: this.lastFrame,
      availability: this.observed
        ? this.valid
          ? this.unavailable
            ? "partial"
            : "available"
          : "unavailable"
        : this.pending
          ? "pending"
          : "not-sampled",
      excludes: [
        "Copies and uploads outside passes",
        "Query resolution and readback",
        "Browser composition and physical presentation",
      ],
      lastCoverage: this.lastCoverage
        ? structuredClone(this.lastCoverage)
        : null,
    };
  }
  sampleFrame(frame: number) {
    return this.frameValues.get(frame) ?? null;
  }
  sample(frame: number) {
    return this.values.get(frame) ?? null;
  }
  clear() {
    this.generation++;
    this.values.clear();
    this.frameValues.clear();
    this.observed = this.valid = this.unavailable = this.pending = 0;
    this.lastFrame = null;
    this.lastCoverage = null;
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.clear();
    this.slots.forEach((slot) => {
      slot.queries.destroy();
      slot.resolve.destroy();
      slot.read.destroy();
    });
  }
}
