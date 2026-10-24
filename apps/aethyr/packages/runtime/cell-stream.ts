/** Verified cell staging. Rendering and collision activate together in one host commit. */
export interface StreamResource {
  url: string;
  sha256: string;
  bytes: number;
  /** Cook-time gzip estimate, not a measurement of transferred HTTP bytes. */
  gzipBytes: number;
}
export interface StreamCell {
  id: string;
  quality: "staged-flat-outside-entry" | "source";
  bounds: { min: [number, number]; max: [number, number] };
  scenes: { day: string; night: string };
  resources: StreamResource[];
}
export interface StreamPackageManifest {
  schema: "aethyrforge.stream-package.v1";
  abi: 1;
  identityScope: "stream-descriptor-v1";
  /** Hash of the complete canonical descriptor, excluding this hash and its scope label. */
  sha256: string;
  entryCell: string;
  cells: StreamCell[];
  files: StreamResource[];
  totals: {
    rawBytes: number;
    gzipBytes: number;
    uniqueRawBytes: number;
    uniqueGzipBytes: number;
  };
}
export interface CellStreamOptions {
  fetch?: typeof fetch;
  maxResourceBytes?: number;
  maxHeldBytes?: number;
  concurrency?: number;
  maxCells?: number;
  maxResources?: number;
  now?: () => number;
}
export interface CellStreamPrepared {
  /** Synchronous, atomic host activation; a throw must leave the old cell active. */
  commit(): void;
  /** Idempotently release this candidate, or this old cell after replacement. */
  dispose(): void;
  /** Optional host-measured decoding time. Async preparation is timed separately. */
  decodeMs?: number;
}
export interface CellStreamInputs {
  cell: StreamCell;
  generation: number;
  /** Read-only borrowed verified bytes; do not mutate or retain after disposal. */
  resources: ReadonlyMap<string, Uint8Array<ArrayBuffer>>;
  signal: AbortSignal;
}
export interface CellStreamLoadOptions {
  signal?: AbortSignal;
  /** Prepare graphics and collision without changing the live cell. */
  prepare(
    inputs: CellStreamInputs,
  ): Promise<CellStreamPrepared> | CellStreamPrepared;
}
export type CellStreamResult =
  | { ok: true; cellId: string; generation: number; cache: "cold" | "warm" }
  | {
      ok: false;
      cellId: string;
      generation: number;
      reason: "canceled" | "stale" | "failed";
      message: string;
    };
export interface CellStreamSnapshot {
  activeCell: string | null;
  bounds: StreamCell["bounds"] | null;
  generation: number;
  pendingLoads: number;
  cachedResources: number;
  /** Cache plus reserved download buffers; excludes browser, decoded, and GPU memory. */
  heldBytes: number;
  cacheBytes: number;
  inFlightBytes: number;
  highWaterBytes: number;
  loads: number;
  coldLoads: number;
  warmLoads: number;
  networkRequests: number;
  /** Decoded response-body bytes consumed, including failed work; not wire bytes. */
  requestedBytes: number;
  cacheHits: number;
  hashMs: number;
  decodeMs: number;
  prepareMs: number;
  failures: number;
  cancellations: number;
  cleanupErrors: number;
  disposed: boolean;
}

const MiB = 1024 * 1024;
const defaults = {
  maxResourceBytes: 256 * MiB,
  maxHeldBytes: 2048 * MiB,
  concurrency: 6,
  maxCells: 128,
  maxResources: 16_384,
};
type Limits = typeof defaults;
const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);
const integer = (
  value: unknown,
  max = Number.MAX_SAFE_INTEGER,
): value is number =>
  typeof value === "number" &&
  Number.isSafeInteger(value) &&
  value >= 0 &&
  value <= max;
const identifier = (value: unknown): value is string =>
  typeof value === "string" && /^[a-zA-Z][a-zA-Z0-9_-]{0,63}$/.test(value);
function resourceURL(value: unknown): value is string {
  return (
    typeof value === "string" &&
    value.length <= 512 &&
    /^\/(?:private-assets|starter)\/[a-zA-Z0-9_./-]+$/.test(value) &&
    value
      .split("/")
      .slice(1)
      .every((part) => part !== "" && part !== "." && part !== "..")
  );
}
function resource(value: unknown, limits: Limits): StreamResource {
  if (
    !isRecord(value) ||
    !resourceURL(value.url) ||
    typeof value.sha256 !== "string" ||
    !/^[a-f0-9]{64}$/.test(value.sha256) ||
    !integer(value.bytes, limits.maxResourceBytes) ||
    !integer(value.gzipBytes, limits.maxResourceBytes * 2 + 1024)
  )
    throw new Error(
      "Choose bounded local resources with complete hashes and sizes.",
    );
  return {
    url: value.url,
    sha256: value.sha256,
    bytes: value.bytes,
    gzipBytes: value.gzipBytes,
  };
}
const sameResource = (a: StreamResource, b: StreamResource) =>
  a.url === b.url &&
  a.sha256 === b.sha256 &&
  a.bytes === b.bytes &&
  a.gzipBytes === b.gzipBytes;
const cacheKey = (value: StreamResource) =>
  `${value.url}\0${value.sha256}\0${value.bytes}`;

/** Validate a bounded copy. Hash verification happens before the first load. */
export function validateStreamPackage(
  value: unknown,
  overrides: Pick<
    CellStreamOptions,
    "maxCells" | "maxResources" | "maxResourceBytes"
  > = {},
): StreamPackageManifest {
  const limits = {
    ...defaults,
    maxCells: overrides.maxCells ?? defaults.maxCells,
    maxResources: overrides.maxResources ?? defaults.maxResources,
    maxResourceBytes: overrides.maxResourceBytes ?? defaults.maxResourceBytes,
  };
  if (
    !integer(limits.maxCells, defaults.maxCells) ||
    limits.maxCells < 1 ||
    !integer(limits.maxResources, defaults.maxResources) ||
    limits.maxResources < 1 ||
    !integer(limits.maxResourceBytes) ||
    limits.maxResourceBytes < 1
  )
    throw new Error("Choose positive bounded package limits.");
  if (
    !isRecord(value) ||
    value.schema !== "aethyrforge.stream-package.v1" ||
    value.abi !== 1 ||
    value.identityScope !== "stream-descriptor-v1" ||
    typeof value.sha256 !== "string" ||
    !/^[a-f0-9]{64}$/.test(value.sha256) ||
    !identifier(value.entryCell) ||
    !Array.isArray(value.cells) ||
    value.cells.length < 1 ||
    value.cells.length > limits.maxCells ||
    !Array.isArray(value.files) ||
    value.files.length < 1 ||
    value.files.length > limits.maxResources ||
    !isRecord(value.totals) ||
    !["rawBytes", "gzipBytes", "uniqueRawBytes", "uniqueGzipBytes"].every(
      (key) => integer((value.totals as Record<string, unknown>)[key]),
    )
  )
    throw new Error("Open a compatible, bounded cell package.");
  const files = value.files.map((file) => resource(file, limits));
  const byURL = new Map(files.map((file) => [file.url, file]));
  if (byURL.size !== files.length)
    throw new Error("List each package resource URL once.");
  const byHash = new Map<string, StreamResource>();
  for (const file of files) {
    const same = byHash.get(file.sha256);
    if (
      same &&
      (same.bytes !== file.bytes || same.gzipBytes !== file.gzipBytes)
    )
      throw new Error("Give identical resource contents matching sizes.");
    byHash.set(file.sha256, file);
  }
  const sum = (items: Iterable<StreamResource>, key: "bytes" | "gzipBytes") =>
    [...items].reduce((total, file) => total + file[key], 0);
  if (
    value.totals.rawBytes !== sum(files, "bytes") ||
    value.totals.gzipBytes !== sum(files, "gzipBytes") ||
    value.totals.uniqueRawBytes !== sum(byHash.values(), "bytes") ||
    value.totals.uniqueGzipBytes !== sum(byHash.values(), "gzipBytes")
  )
    throw new Error("The package resource totals do not match its inventory.");
  const seen = new Set<string>();
  let resourceCount = 0;
  const cells: StreamCell[] = value.cells.map((candidate: unknown) => {
    if (
      !isRecord(candidate) ||
      !identifier(candidate.id) ||
      seen.has(candidate.id) ||
      !["staged-flat-outside-entry", "source"].includes(
        candidate.quality as string,
      ) ||
      !isRecord(candidate.bounds) ||
      !isRecord(candidate.scenes) ||
      !Array.isArray(candidate.resources) ||
      candidate.resources.length < 1 ||
      candidate.resources.length > limits.maxResources
    )
      throw new Error(
        "Choose distinct cells with complete bounds, scenes, and resources.",
      );
    const { min, max } = candidate.bounds;
    if (
      ![min, max].every(
        (point) =>
          Array.isArray(point) &&
          point.length === 2 &&
          point.every(
            (n) =>
              typeof n === "number" &&
              Number.isSafeInteger(n) &&
              Math.abs(n) <= 1_000_000,
          ),
      ) ||
      (min as number[]).some((n, i) => n >= (max as number[])[i])
    )
      throw new Error("Choose finite, increasing cell bounds.");
    const day = candidate.scenes.day,
      night = candidate.scenes.night;
    if (!resourceURL(day) || !resourceURL(night))
      throw new Error("Choose local day and night scene resources.");
    const resources = candidate.resources.map((file) => resource(file, limits));
    resourceCount += resources.length;
    if (resourceCount > limits.maxResources * limits.maxCells)
      throw new Error("The package lists too many cell resource references.");
    const urls = new Set(resources.map((file) => file.url));
    if (
      urls.size !== resources.length ||
      !urls.has(day) ||
      !urls.has(night) ||
      resources.some(
        (file) =>
          !byURL.has(file.url) || !sameResource(file, byURL.get(file.url)!),
      )
    )
      throw new Error(
        "Include both scene roots and matching inventory resources in every cell.",
      );
    seen.add(candidate.id);
    return {
      id: candidate.id,
      quality: candidate.quality as StreamCell["quality"],
      bounds: {
        min: [...(min as [number, number])],
        max: [...(max as [number, number])],
      },
      scenes: { day, night },
      resources,
    };
  });
  if (!seen.has(value.entryCell))
    throw new Error("Choose an entry cell that exists in this package.");
  return {
    schema: "aethyrforge.stream-package.v1",
    abi: 1,
    identityScope: "stream-descriptor-v1",
    sha256: value.sha256,
    entryCell: value.entryCell,
    cells,
    files,
    totals: {
      rawBytes: value.totals.rawBytes as number,
      gzipBytes: value.totals.gzipBytes as number,
      uniqueRawBytes: value.totals.uniqueRawBytes as number,
      uniqueGzipBytes: value.totals.uniqueGzipBytes as number,
    },
  };
}
async function sha256(bytes: Uint8Array<ArrayBuffer>) {
  return Array.from(
    new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)),
    (n) => n.toString(16).padStart(2, "0"),
  ).join("");
}
/** ASCII descriptor strings and integer numbers match the cooker's sorted compact JSON. */
export async function streamPackageIdentity(manifest: StreamPackageManifest) {
  function sorted(value: unknown): unknown {
    if (Array.isArray(value)) return value.map(sorted);
    if (isRecord(value))
      return Object.fromEntries(
        Object.keys(value)
          .sort()
          .map((key) => [key, sorted(value[key])]),
      );
    return value;
  }
  const { schema, abi, entryCell, cells, files, totals } = manifest;
  return sha256(
    new TextEncoder().encode(
      JSON.stringify(sorted({ schema, abi, entryCell, cells, files, totals })),
    ),
  );
}
class Stopped extends Error {
  constructor(readonly reason: "stale" | "canceled") {
    super(
      reason === "stale"
        ? "A newer cell load replaced this request."
        : "Cell loading was canceled.",
    );
  }
}
interface CachedResource {
  descriptor: StreamResource;
  bytes: Uint8Array<ArrayBuffer>;
}
interface PendingLoad {
  generation: number;
  controller: AbortController;
  keys: Set<string>;
  reason: "stale" | "canceled";
  networkRequests: number;
  failure?: unknown;
}
interface ActiveCell {
  cell: StreamCell;
  keys: Set<string>;
  prepared: CellStreamPrepared;
}

/** One desired candidate, one active cell, bounded verified byte cache, no frame-loop ownership. */
export class CellStream {
  readonly #limits: Limits;
  readonly #fetch: typeof fetch;
  readonly #now: () => number;
  readonly #cache = new Map<string, CachedResource>();
  readonly #pending = new Map<number, PendingLoad>();
  #manifest?: StreamPackageManifest;
  #inventoryVerified = false;
  #active?: ActiveCell;
  #generation = 0;
  #cacheBytes = 0;
  #inFlightBytes = 0;
  #disposed = false;
  #committing = false;
  #counters = {
    highWaterBytes: 0,
    loads: 0,
    coldLoads: 0,
    warmLoads: 0,
    networkRequests: 0,
    requestedBytes: 0,
    cacheHits: 0,
    hashMs: 0,
    decodeMs: 0,
    prepareMs: 0,
    failures: 0,
    cancellations: 0,
    cleanupErrors: 0,
  };
  constructor(options: CellStreamOptions = {}) {
    this.#limits = {
      ...defaults,
      ...Object.fromEntries(
        Object.keys(defaults).map((key) => [
          key,
          options[key as keyof Limits] ?? defaults[key as keyof Limits],
        ]),
      ),
    };
    if (
      Object.values(this.#limits).some((n) => !integer(n) || n < 1) ||
      this.#limits.concurrency > 16 ||
      this.#limits.maxCells > 128 ||
      this.#limits.maxResources > 16_384
    )
      throw new Error(
        "Choose positive bounded stream limits and at most sixteen concurrent requests.",
      );
    this.#fetch = options.fetch ?? fetch;
    this.#now = options.now ?? (() => performance.now());
  }
  configure(candidate: unknown): void {
    this.#requireOpen();
    const manifest = validateStreamPackage(candidate, this.#limits);
    this.#stopPending("stale");
    this.#generation++;
    this.#manifest = manifest;
    this.#inventoryVerified = false;
  }
  cell(id: string): StreamCell | undefined {
    const cell = this.#manifest?.cells.find((cell) => cell.id === id);
    return cell && structuredClone(cell);
  }
  snapshot(): CellStreamSnapshot {
    return {
      activeCell: this.#active?.cell.id ?? null,
      bounds: this.#active ? structuredClone(this.#active.cell.bounds) : null,
      generation: this.#generation,
      pendingLoads: this.#pending.size,
      cachedResources: this.#cache.size,
      heldBytes: this.#cacheBytes + this.#inFlightBytes,
      cacheBytes: this.#cacheBytes,
      inFlightBytes: this.#inFlightBytes,
      ...this.#counters,
      disposed: this.#disposed,
    };
  }
  /** Walk admission uses active collision bounds, never a merely requested cell. */
  canEnter(point: readonly number[]): boolean {
    if (
      this.#disposed ||
      !this.#active ||
      !Array.isArray(point) ||
      ![2, 3].includes(point.length) ||
      !point.every(Number.isFinite)
    )
      return false;
    const [x, z] = [point[0], point.length === 3 ? point[2] : point[1]];
    const { min, max } = this.#active.cell.bounds;
    return x >= min[0] && x <= max[0] && z >= min[1] && z <= max[1];
  }
  cancel(): void {
    this.#stopPending("canceled");
    this.#generation++;
  }
  /** Release unneeded cache bytes. An active or staging cell always keeps its inputs. */
  unloadInactive(): number {
    return this.#evict((key) => !this.#pinned(key));
  }
  /** An active cell cannot be unloaded until another complete cell replaces it. */
  unload(cellId?: string): number {
    if (cellId === undefined) return this.unloadInactive();
    if (cellId === this.#active?.cell.id) return 0;
    const cell = this.#manifest?.cells.find((cell) => cell.id === cellId);
    if (!cell) return 0;
    const keys = new Set(cell.resources.map(cacheKey));
    return this.#evict((key) => keys.has(key) && !this.#pinned(key));
  }
  dispose(): void {
    if (this.#disposed) return;
    this.#stopPending("canceled");
    this.#generation++;
    this.#disposed = true;
    if (this.#active) this.#release(this.#active.prepared);
    this.#active = undefined;
    this.#evict(() => true);
    this.#manifest = undefined;
  }
  async load(
    cellId: string,
    options: CellStreamLoadOptions,
  ): Promise<CellStreamResult> {
    this.#requireOpen();
    const manifest = this.#manifest;
    const cell = manifest?.cells.find((cell) => cell.id === cellId);
    this.#stopPending("stale");
    const generation = ++this.#generation;
    this.#counters.loads++;
    if (
      !manifest ||
      !cell ||
      !options ||
      typeof options.prepare !== "function"
    ) {
      this.#counters.failures++;
      return {
        ok: false,
        cellId,
        generation,
        reason: "failed",
        message:
          "Configure an available cell and its preparation callback first.",
      };
    }
    const pending: PendingLoad = {
      generation,
      controller: new AbortController(),
      keys: new Set(cell.resources.map(cacheKey)),
      reason: "canceled",
      networkRequests: 0,
    };
    this.#pending.set(generation, pending);
    const abort = () => {
      pending.reason = "canceled";
      pending.controller.abort();
    };
    options.signal?.addEventListener("abort", abort, { once: true });
    if (options.signal?.aborted) abort();
    let prepared: CellStreamPrepared | undefined;
    try {
      this.#current(pending);
      if (!this.#inventoryVerified) {
        const start = this.#now();
        const identity = await streamPackageIdentity(manifest);
        this.#counters.hashMs += Math.max(0, this.#now() - start);
        this.#current(pending);
        if (identity !== manifest.sha256)
          throw new Error(
            "The cell descriptor does not match its expected hash.",
          );
        this.#inventoryVerified = true;
      }
      const resources = new Map<string, Uint8Array<ArrayBuffer>>();
      let index = 0;
      const fetchNext = async () => {
        try {
          while (index < cell.resources.length) {
            this.#current(pending);
            const descriptor = cell.resources[index++];
            resources.set(
              descriptor.url,
              await this.#resource(descriptor, pending),
            );
          }
        } catch (error) {
          if (
            !(error instanceof Stopped) &&
            !pending.controller.signal.aborted
          ) {
            pending.failure = error;
            pending.controller.abort();
          }
          throw error;
        }
      };
      // Bounded concurrent network tasks, rather than one request per inventory entry.
      const jobs = Array.from(
        { length: Math.min(this.#limits.concurrency, cell.resources.length) },
        fetchNext,
      );
      const results = await Promise.allSettled(jobs);
      const failed = results.find(
        (result): result is PromiseRejectedResult =>
          result.status === "rejected",
      );
      if (failed) throw pending.failure ?? failed.reason;
      this.#current(pending);
      const start = this.#now();
      try {
        prepared = await options.prepare({
          cell: structuredClone(cell),
          resources,
          generation,
          signal: pending.controller.signal,
        });
      } finally {
        this.#counters.prepareMs += Math.max(0, this.#now() - start);
      }
      if (
        !prepared ||
        typeof prepared.commit !== "function" ||
        typeof prepared.dispose !== "function" ||
        (prepared.decodeMs !== undefined &&
          (typeof prepared.decodeMs !== "number" ||
            !Number.isFinite(prepared.decodeMs) ||
            prepared.decodeMs < 0))
      )
        throw new Error("Prepare a complete cell before activating it.");
      this.#counters.decodeMs += prepared.decodeMs ?? 0;
      this.#current(pending);
      this.#committing = true;
      try {
        prepared.commit();
      } finally {
        this.#committing = false;
      }
      const previous = this.#active;
      this.#active = { cell, keys: pending.keys, prepared };
      prepared = undefined;
      if (previous) this.#release(previous.prepared);
      const cache = pending.networkRequests === 0 ? "warm" : "cold";
      this.#counters[cache === "warm" ? "warmLoads" : "coldLoads"]++;
      return { ok: true, cellId, generation, cache };
    } catch (error) {
      if (prepared) this.#release(prepared);
      const stopped =
        pending.controller.signal.aborted ||
        pending.generation !== this.#generation ||
        this.#disposed;
      const reason = pending.failure
        ? "failed"
        : error instanceof Stopped
          ? error.reason
          : stopped
            ? pending.reason
            : "failed";
      this.#counters[reason === "failed" ? "failures" : "cancellations"]++;
      return {
        ok: false,
        cellId,
        generation,
        reason,
        message:
          reason === "failed"
            ? String(error instanceof Error ? error.message : error)
            : new Stopped(reason).message,
      };
    } finally {
      options.signal?.removeEventListener("abort", abort);
      this.#pending.delete(generation);
    }
  }
  #requireOpen() {
    if (this.#disposed)
      throw new Error("Open a new stream after closing this one.");
    if (this.#committing)
      throw new Error(
        "Finish activating this cell before changing its stream.",
      );
  }
  #stopPending(reason: "stale" | "canceled") {
    if (this.#committing)
      throw new Error(
        "Finish activating this cell before changing its stream.",
      );
    for (const pending of this.#pending.values()) {
      pending.reason = reason;
      pending.controller.abort();
    }
  }
  #current(pending: PendingLoad) {
    if (
      pending.controller.signal.aborted ||
      this.#disposed ||
      pending.generation !== this.#generation
    )
      throw new Stopped(pending.reason);
  }
  #pinned(key: string) {
    return (
      this.#active?.keys.has(key) ||
      [...this.#pending.values()].some((pending) => pending.keys.has(key))
    );
  }
  #evict(predicate: (key: string) => boolean) {
    let freed = 0;
    for (const [key, cached] of this.#cache)
      if (predicate(key)) {
        this.#cache.delete(key);
        freed += cached.bytes.byteLength;
      }
    this.#cacheBytes -= freed;
    return freed;
  }
  #release(prepared: CellStreamPrepared) {
    try {
      prepared.dispose();
    } catch {
      this.#counters.cleanupErrors++;
    }
  }
  #reserve(bytes: number) {
    if (
      this.#cacheBytes + this.#inFlightBytes + bytes >
      this.#limits.maxHeldBytes
    ) {
      for (const key of this.#cache.keys()) {
        if (!this.#pinned(key)) this.#evict((candidate) => candidate === key);
        if (
          this.#cacheBytes + this.#inFlightBytes + bytes <=
          this.#limits.maxHeldBytes
        )
          break;
      }
    }
    if (
      this.#cacheBytes + this.#inFlightBytes + bytes >
      this.#limits.maxHeldBytes
    )
      throw new Error(
        "This cell and the working cell exceed the stream memory limit.",
      );
    this.#inFlightBytes += bytes;
    this.#counters.highWaterBytes = Math.max(
      this.#counters.highWaterBytes,
      this.#cacheBytes + this.#inFlightBytes,
    );
    let held = true;
    return () => {
      if (held) {
        this.#inFlightBytes -= bytes;
        held = false;
      }
    };
  }
  async #resource(
    descriptor: StreamResource,
    pending: PendingLoad,
  ): Promise<Uint8Array<ArrayBuffer>> {
    const key = cacheKey(descriptor),
      cached = this.#cache.get(key);
    if (cached) {
      const start = this.#now();
      const identity = await sha256(cached.bytes);
      this.#counters.hashMs += Math.max(0, this.#now() - start);
      this.#current(pending);
      if (
        identity !== descriptor.sha256 ||
        cached.bytes.byteLength !== descriptor.bytes
      )
        throw new Error("A cached resource no longer matches this package.");
      this.#cache.delete(key);
      this.#cache.set(key, cached);
      this.#counters.cacheHits++;
      return cached.bytes;
    }
    const release = this.#reserve(descriptor.bytes);
    let reader: ReadableStreamDefaultReader<Uint8Array> | undefined;
    let response: Response | undefined;
    try {
      this.#counters.networkRequests++;
      pending.networkRequests++;
      response = await this.#fetch(descriptor.url, {
        signal: pending.controller.signal,
        cache: "no-store",
        redirect: "error",
      });
      this.#current(pending);
      if (!response.ok || response.redirected)
        throw new Error("A required cell resource could not open.");
      reader = response.body?.getReader();
      if (!reader)
        throw new Error("A required cell resource has no readable bytes.");
      const bytes = new Uint8Array(descriptor.bytes);
      let length = 0;
      for (;;) {
        const chunk = await reader.read();
        this.#current(pending);
        if (chunk.done) break;
        this.#counters.requestedBytes += chunk.value.byteLength;
        if (length + chunk.value.byteLength > descriptor.bytes)
          throw new Error("A cell resource exceeds its declared size.");
        bytes.set(chunk.value, length);
        length += chunk.value.byteLength;
      }
      if (length !== descriptor.bytes)
        throw new Error("A cell resource is incomplete.");
      const start = this.#now();
      const identity = await sha256(bytes);
      this.#counters.hashMs += Math.max(0, this.#now() - start);
      this.#current(pending);
      if (identity !== descriptor.sha256)
        throw new Error("A cell resource does not match its expected hash.");
      release();
      this.#cache.set(key, { descriptor, bytes });
      this.#cacheBytes += bytes.byteLength;
      return bytes;
    } finally {
      try {
        if (reader) await reader.cancel();
        else await response?.body?.cancel();
      } catch {
        /* Fetch cancellation can close the reader first. */
      }
      release();
    }
  }
}
