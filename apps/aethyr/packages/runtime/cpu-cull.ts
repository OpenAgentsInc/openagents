/** Optional CPU experiment. Its results never become live renderer visibility. */
import {
  frustumPlanes,
  frustumVisibility,
  type InstanceBounds,
} from "../renderer/culling.ts";

export const CPU_CULL_LIMITS = Object.freeze({
  instances: 1_000_000,
  workers: 2,
  timeoutMs: 30_000,
});
export interface CpuCullWorker {
  postMessage(message: unknown, transfer: Transferable[]): void;
  addEventListener(type: string, listener: EventListener): void;
  removeEventListener(type: string, listener: EventListener): void;
  terminate(): void;
}
export type CpuCullWorkerRequest =
  | {
      type: "install";
      epoch: number;
      part: number;
      count: number;
      spheres: Float64Array<ArrayBuffer>;
      aabbs: Float64Array<ArrayBuffer>;
    }
  | {
      type: "run";
      epoch: number;
      sequence: number;
      matrix: Float64Array<ArrayBuffer>;
    };
export type CpuCullWorkerReply =
  | { type: "ready"; epoch: number; part: number; count: number }
  | {
      type: "result";
      epoch: number;
      part: number;
      sequence: number;
      count: number;
      mask: ArrayBuffer;
      computeMs: number;
      taskRuns?: number;
    }
  | {
      type: "error";
      epoch: number;
      part: number;
      sequence?: number;
      message: string;
    };
export interface CpuCullBytes {
  /** Explicit typed-buffer copies; excludes JavaScript and browser protocol overhead. */
  copied: number;
  transferredIn: number;
  transferredOut: number;
  /** Coordinator and helper bounds only; returned masks belong to the caller. */
  resident: number;
  /** Peak owned typed buffers, including transient matrices and output masks. */
  peak: number;
}
export interface CpuCullInstallResult {
  datasetId: string;
  generation: number;
  workers: number;
  instances: number;
  timings: { copyMs: number; workerReadyMs: number; totalMs: number };
  bytes: CpuCullBytes;
}
export interface CpuCullResult {
  datasetId: string;
  generation: number;
  sequence: number;
  mode: "serial" | "workers";
  workers: number;
  mask: Uint8Array<ArrayBuffer>;
  timings: {
    kernelMs: number;
    maxKernelMs: number;
    dispatchMs: number;
    roundTripMs: number;
    mergeMs: number;
    totalMs: number;
  };
  bytes: CpuCullBytes;
}
export type CpuCullErrorCode =
  | "invalid-input"
  | "not-installed"
  | "busy"
  | "disposed"
  | "aborted"
  | "superseded"
  | "timeout"
  | "worker-failed"
  | "invalid-result";
export class CpuCullError extends Error {
  constructor(
    readonly code: CpuCullErrorCode,
    message: string,
  ) {
    super(message);
    this.name = "CpuCullError";
  }
}
interface Dataset {
  id: string;
  generation: number;
  bounds: InstanceBounds;
}
interface Slot {
  port: CpuCullWorker;
  part: number;
  start: number;
  count: number;
  message: EventListener;
  failure: EventListener;
}
interface Active {
  kind: "install" | "run";
  epoch: number;
  sequence: number;
  pending: Set<number>;
  results: Map<number, { mask: Uint8Array<ArrayBuffer>; computeMs: number }>;
  cleanup(): void;
  reject(error: Error): void;
  ready(): void;
}
const invalid = (message: string): never => {
  throw new CpuCullError("invalid-input", message);
};
function integer(value: unknown, min: number, max: number): value is number {
  return (
    typeof value === "number" &&
    Number.isSafeInteger(value) &&
    value >= min &&
    value <= max
  );
}
function timeout(value: number | undefined) {
  const result = value ?? 5000;
  if (!integer(result, 1, CPU_CULL_LIMITS.timeoutMs))
    invalid("Choose a CPU probe timeout from 1 to 30,000 milliseconds.");
  return result;
}
function copiedBounds(bounds: InstanceBounds): InstanceBounds {
  if (
    !bounds ||
    !integer(bounds.count, 0, CPU_CULL_LIMITS.instances) ||
    !(bounds.spheres instanceof Float64Array) ||
    !(bounds.aabbs instanceof Float64Array) ||
    !(bounds.spheres.buffer instanceof ArrayBuffer) ||
    !(bounds.aabbs.buffer instanceof ArrayBuffer) ||
    bounds.spheres.length !== bounds.count * 4 ||
    bounds.aabbs.length !== bounds.count * 6
  )
    invalid(
      "Use complete, ordinary typed bounds for at most one million instances.",
    );
  const spheres = new Float64Array(bounds.spheres),
    aabbs = new Float64Array(bounds.aabbs);
  for (let i = 0; i < bounds.count; i++) {
    if (
      ![
        ...spheres.subarray(i * 4, i * 4 + 4),
        ...aabbs.subarray(i * 6, i * 6 + 6),
      ].every(Number.isFinite) ||
      spheres[i * 4 + 3] < 0 ||
      [0, 1, 2].some((axis) => aabbs[i * 6 + axis] > aabbs[i * 6 + axis + 3])
    )
      invalid("Use finite spheres and ordered instance bounds.");
  }
  return { count: bounds.count, spheres, aabbs };
}
function matrixCopy(matrix: ArrayLike<number>) {
  try {
    const copy = Float64Array.from(matrix);
    frustumPlanes(copy);
    return copy;
  } catch {
    return invalid("Use a complete finite camera matrix for CPU measurement.");
  }
}

/** Owns immutable CPU copies and at most two resident helpers, with no shared memory. */
export class CpuCullExperiment {
  private readonly workerCount: 0 | 1 | 2;
  private readonly factory: () => CpuCullWorker;
  private dataset?: Dataset;
  private slots: Slot[] = [];
  private active?: Active;
  private epoch = 0;
  private sequence = 0;
  private disposed = false;
  private workersReady = false;
  private peak = 0;
  private ignoredReplies = 0;
  constructor(
    options: { workers?: 0 | 1 | 2; workerFactory?: () => CpuCullWorker } = {},
  ) {
    const count = options.workers ?? 2;
    if (![0, 1, 2].includes(count))
      invalid("Choose zero, one, or two CPU helpers.");
    this.workerCount = count;
    this.factory =
      options.workerFactory ??
      (() =>
        new Worker(new URL("./cpu-cull.worker.ts", import.meta.url), {
          type: "module",
        }));
  }
  private usable() {
    if (this.disposed)
      throw new CpuCullError("disposed", "This CPU probe is closed.");
  }
  private resident() {
    const own = this.dataset ? this.dataset.bounds.count * 80 : 0;
    return own + (this.workersReady ? own : 0);
  }
  private metrics(
    copied: number,
    transferredIn: number,
    transferredOut: number,
    transient = 0,
  ): CpuCullBytes {
    const resident = this.resident();
    this.peak = Math.max(this.peak, resident + transient);
    return { copied, transferredIn, transferredOut, resident, peak: this.peak };
  }
  private terminate() {
    for (const slot of this.slots) {
      slot.port.removeEventListener("message", slot.message);
      slot.port.removeEventListener("error", slot.failure);
      slot.port.removeEventListener("messageerror", slot.failure);
      slot.port.terminate();
    }
    this.slots = [];
    this.workersReady = false;
  }
  private fail(error: Error) {
    const active = this.active;
    this.active = undefined;
    this.terminate();
    if (active) {
      active.cleanup();
      active.reject(error);
    }
  }
  cancel() {
    this.fail(new CpuCullError("aborted", "CPU probe canceled."));
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.fail(new CpuCullError("disposed", "CPU probe closed."));
    this.dataset = undefined;
  }
  snapshot() {
    return {
      installed: Boolean(this.dataset),
      disposed: this.disposed,
      active: Boolean(this.active),
      datasetId: this.dataset?.id,
      generation: this.dataset?.generation,
      instances: this.dataset?.bounds.count ?? 0,
      configuredWorkers: this.workerCount,
      workers: this.workersReady ? this.slots.length : 0,
      sequence: this.sequence,
      residentBytes: this.resident(),
      peakBytes: this.peak,
      ignoredReplies: this.ignoredReplies,
    };
  }
  private watch(
    active: Active,
    signal: AbortSignal | undefined,
    timeoutMs: number,
  ) {
    const abort = () => {
      if (this.active === active)
        this.fail(new CpuCullError("aborted", "CPU probe canceled."));
    };
    const timer = setTimeout(() => {
      if (this.active === active)
        this.fail(
          new CpuCullError(
            "timeout",
            "The CPU helpers did not finish in time.",
          ),
        );
    }, timeoutMs);
    active.cleanup = () => {
      clearTimeout(timer);
      signal?.removeEventListener("abort", abort);
    };
    signal?.addEventListener("abort", abort, { once: true });
  }
  private receive(slot: Slot, data: unknown) {
    const active = this.active;
    if (!data || typeof data !== "object") {
      if (active)
        this.fail(
          new CpuCullError(
            "invalid-result",
            "A CPU helper returned an invalid message.",
          ),
        );
      return;
    }
    const message = data as Partial<CpuCullWorkerReply>;
    if (
      !active ||
      !message ||
      message.epoch !== active.epoch ||
      message.part !== slot.part ||
      (active.kind === "run" &&
        "sequence" in message &&
        message.sequence !== active.sequence)
    ) {
      this.ignoredReplies++;
      return;
    }
    if (!active.pending.has(slot.part)) {
      this.ignoredReplies++;
      return;
    }
    if (message.type === "error") {
      this.fail(
        new CpuCullError(
          "worker-failed",
          "A CPU helper could not finish the probe.",
        ),
      );
      return;
    }
    if (active.kind === "install") {
      if (message.type !== "ready" || message.count !== slot.count) {
        this.fail(
          new CpuCullError(
            "invalid-result",
            "A CPU helper returned an invalid installation result.",
          ),
        );
        return;
      }
    } else {
      if (
        message.type !== "result" ||
        message.sequence !== active.sequence ||
        message.count !== slot.count ||
        !(message.mask instanceof ArrayBuffer) ||
        message.mask.byteLength !== slot.count ||
        typeof message.computeMs !== "number" ||
        !Number.isFinite(message.computeMs) ||
        message.computeMs < 0
      ) {
        this.fail(
          new CpuCullError(
            "invalid-result",
            "A CPU helper returned invalid visibility data.",
          ),
        );
        return;
      }
      const mask = new Uint8Array(message.mask);
      if (!mask.every((n) => n === 0 || n === 1)) {
        this.fail(
          new CpuCullError(
            "invalid-result",
            "A CPU helper returned invalid visibility flags.",
          ),
        );
        return;
      }
      active.results.set(slot.part, { mask, computeMs: message.computeMs });
    }
    active.pending.delete(slot.part);
    if (!active.pending.size) active.ready();
  }
  async install(
    input: { datasetId: string; generation: number; bounds: InstanceBounds },
    options: { signal?: AbortSignal; timeoutMs?: number } = {},
  ): Promise<CpuCullInstallResult> {
    this.usable();
    const limit = timeout(options.timeoutMs);
    if (options.signal?.aborted)
      throw new CpuCullError("aborted", "CPU probe canceled.");
    if (
      !input ||
      typeof input.datasetId !== "string" ||
      !/^[A-Za-z0-9_.:-]{1,128}$/.test(input.datasetId) ||
      !integer(input.generation, 0, Number.MAX_SAFE_INTEGER)
    )
      invalid("Choose a bounded CPU dataset ID and generation.");
    const started = performance.now(),
      copyStarted = performance.now();
    const bounds = copiedBounds(input.bounds),
      copyMs = performance.now() - copyStarted;
    if (this.active)
      this.fail(
        new CpuCullError(
          "superseded",
          "A newer CPU dataset replaced this probe.",
        ),
      );
    else this.terminate();
    this.dataset = {
      id: input.datasetId,
      generation: input.generation,
      bounds,
    };
    this.sequence = 0;
    const epoch = ++this.epoch,
      bytes = bounds.count * 80;
    this.peak = Math.max(this.peak, bytes * (this.workerCount ? 2 : 1));
    const result = (workerReadyMs: number): CpuCullInstallResult => ({
      datasetId: input.datasetId,
      generation: input.generation,
      workers: this.workerCount,
      instances: bounds.count,
      timings: { copyMs, workerReadyMs, totalMs: performance.now() - started },
      bytes: this.metrics(
        bytes * (this.workerCount ? 2 : 1),
        this.workerCount ? bytes : 0,
        0,
      ),
    });
    if (!this.workerCount) return result(0);
    const workerStarted = performance.now();
    return new Promise((resolve, reject) => {
      const active: Active = {
        kind: "install",
        epoch,
        sequence: 0,
        pending: new Set(Array.from({ length: this.workerCount }, (_, i) => i)),
        results: new Map(),
        cleanup() {},
        reject,
        ready: () => {
          if (this.active !== active) return;
          this.workersReady = true;
          this.active = undefined;
          active.cleanup();
          resolve(result(performance.now() - workerStarted));
        },
      };
      this.active = active;
      this.watch(active, options.signal, limit);
      try {
        for (let part = 0; part < this.workerCount; part++) {
          const start = Math.floor((bounds.count * part) / this.workerCount),
            end = Math.floor((bounds.count * (part + 1)) / this.workerCount);
          const port = this.factory();
          const slot: Slot = {
            port,
            part,
            start,
            count: end - start,
            message: ((event: MessageEvent) =>
              this.receive(slot, event.data)) as EventListener,
            failure: (() =>
              this.fail(
                new CpuCullError(
                  "worker-failed",
                  "A CPU helper stopped before finishing.",
                ),
              )) as EventListener,
          };
          this.slots.push(slot);
          port.addEventListener("message", slot.message);
          port.addEventListener("error", slot.failure);
          port.addEventListener("messageerror", slot.failure);
          const spheres = bounds.spheres.slice(start * 4, end * 4),
            aabbs = bounds.aabbs.slice(start * 6, end * 6);
          port.postMessage(
            {
              type: "install",
              epoch,
              part,
              count: slot.count,
              spheres,
              aabbs,
            } satisfies CpuCullWorkerRequest,
            [spheres.buffer, aabbs.buffer],
          );
        }
      } catch {
        this.fail(
          new CpuCullError("worker-failed", "The CPU helpers could not open."),
        );
      }
    });
  }
  async run(input: {
    matrix: ArrayLike<number>;
    mode?: "serial" | "workers";
    sequence?: number;
    signal?: AbortSignal;
    timeoutMs?: number;
  }): Promise<CpuCullResult> {
    this.usable();
    if (!this.dataset)
      throw new CpuCullError(
        "not-installed",
        "Install CPU bounds before running a probe.",
      );
    if (this.active)
      throw new CpuCullError("busy", "Let the current CPU probe finish first.");
    if (input.signal?.aborted)
      throw new CpuCullError("aborted", "CPU probe canceled.");
    const limit = timeout(input.timeoutMs),
      mode = input.mode ?? (this.workerCount ? "workers" : "serial");
    if (!["serial", "workers"].includes(mode))
      invalid("Choose serial or worker CPU measurement.");
    const started = performance.now(),
      matrix = matrixCopy(input.matrix),
      dataset = this.dataset;
    const sequence = input.sequence ?? this.sequence + 1;
    if (!integer(sequence, this.sequence + 1, Number.MAX_SAFE_INTEGER))
      invalid("Use increasing CPU probe sequence numbers.");
    if (mode === "workers" && !this.workersReady)
      throw new CpuCullError(
        "not-installed",
        "Install resident CPU helpers before measuring workers.",
      );
    this.sequence = sequence;
    if (mode === "serial") {
      const kernelStarted = performance.now(),
        mask = new Uint8Array(dataset.bounds.count);
      frustumVisibility(dataset.bounds, matrix, mask);
      const kernelMs = performance.now() - kernelStarted;
      if (input.signal?.aborted)
        throw new CpuCullError("aborted", "CPU probe canceled.");
      if (performance.now() - started > limit)
        throw new CpuCullError(
          "timeout",
          "The serial CPU probe exceeded its time limit.",
        );
      return {
        datasetId: dataset.id,
        generation: dataset.generation,
        sequence,
        mode,
        workers: 0,
        mask,
        timings: {
          kernelMs,
          maxKernelMs: kernelMs,
          dispatchMs: 0,
          roundTripMs: 0,
          mergeMs: 0,
          totalMs: performance.now() - started,
        },
        bytes: this.metrics(
          matrix.byteLength,
          0,
          0,
          mask.byteLength + matrix.byteLength,
        ),
      };
    }
    return new Promise((resolve, reject) => {
      let dispatchMs = 0;
      const dispatchStarted = performance.now();
      const active: Active = {
        kind: "run",
        epoch: this.epoch,
        sequence,
        pending: new Set(this.slots.map((slot) => slot.part)),
        results: new Map(),
        cleanup() {},
        reject,
        ready: () => {
          if (this.active !== active) return;
          const roundTripMs = performance.now() - dispatchStarted,
            mergeStarted = performance.now();
          const parts = this.slots.map((slot) =>
            active.results.get(slot.part)!,
          );
          const mask =
            parts.length === 1
              ? parts[0].mask
              : new Uint8Array(dataset.bounds.count);
          if (parts.length > 1)
            this.slots.forEach((slot, i) =>
              mask.set(parts[i].mask, slot.start),
            );
          const mergeMs = performance.now() - mergeStarted;
          this.active = undefined;
          active.cleanup();
          resolve({
            datasetId: dataset.id,
            generation: dataset.generation,
            sequence,
            mode,
            workers: this.slots.length,
            mask,
            timings: {
              kernelMs: parts.reduce((n, part) => n + part.computeMs, 0),
              maxKernelMs: Math.max(...parts.map((part) => part.computeMs)),
              dispatchMs,
              roundTripMs,
              mergeMs,
              totalMs: performance.now() - started,
            },
            bytes: this.metrics(
              matrix.byteLength * (this.slots.length + 1) +
                (parts.length > 1 ? mask.byteLength : 0),
              matrix.byteLength * this.slots.length,
              dataset.bounds.count,
              dataset.bounds.count * (parts.length > 1 ? 2 : 1) +
                matrix.byteLength * (this.slots.length + 1),
            ),
          });
        },
      };
      this.active = active;
      this.watch(active, input.signal, limit);
      try {
        for (const slot of this.slots) {
          const view = matrix.slice();
          slot.port.postMessage(
            {
              type: "run",
              epoch: this.epoch,
              sequence,
              matrix: view,
            } satisfies CpuCullWorkerRequest,
            [view.buffer],
          );
        }
        dispatchMs = performance.now() - dispatchStarted;
      } catch {
        this.fail(
          new CpuCullError(
            "worker-failed",
            "The CPU probe could not reach its helpers.",
          ),
        );
      }
    });
  }
}
