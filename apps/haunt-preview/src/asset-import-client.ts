import { Effect } from "effect";
import type { SourceKind, SourceRecord } from "./asset-compiler.ts";
import type { OwnedAssetDescription as AssetDescription } from "./asset-import-protocol.ts";
import { AssetSourceStore, type StoredSource } from "./asset-source-store.ts";
import { AssetRuntimeAdapter, type AssetRuntime, type PreparedAsset } from "./asset-runtime-adapter.ts";
import { MAX_SOURCE_BYTES, MAX_SOURCE_COUNT, MAX_SOURCE_STORE_BYTES, copyImportedAsset, sameSource, sourceKey, validateImportedAsset } from "./asset-import-protocol.ts";
import type { AssetImportRequest, AssetImportResponse, ImportMetrics, SourceBytes, WorkerStats } from "./asset-import-protocol.ts";

export interface AssetWorkerPort {
  onmessage: ((event: MessageEvent<AssetImportResponse>) => void) | null;
  onerror: ((event: ErrorEvent) => void) | null;
  onmessageerror: ((event: MessageEvent) => void) | null;
  postMessage(message: AssetImportRequest, transfer: Transferable[]): void;
  terminate(): void;
}
type Options = { signal?: AbortSignal };
export type AssetImportMetrics = ImportMetrics & { uploads: number; uploadedBytes: number };
type Compiled = Extract<AssetImportResponse, { type: "compiled" }> & { metrics: AssetImportMetrics };
class Cancelled extends Error {}
const errorOf = (error: unknown) => error instanceof Error ? error : new Error("The asset could not be imported.");

/** Owns worker requests and transfers copies, leaving saved source bytes intact. */
export class AssetImportClient {
  private readonly port: AssetWorkerPort;
  private readonly timeoutMs: number;
  private readonly maxPending: number;
  private readonly pending = new Map<string, { expected: AssetImportResponse["type"]; done: (error?: Error, result?: AssetImportResponse) => void }>();
  private readonly known = new Set<string>();
  private next = 0;
  private closed = false;

  constructor(port: AssetWorkerPort, options: { timeoutMs?: number; maxPending?: number } = {}) {
    this.port = port;
    this.timeoutMs = options.timeoutMs ?? 30_000;
    this.maxPending = options.maxPending ?? 32;
    if (!Number.isFinite(this.timeoutMs) || this.timeoutMs < 1 || this.timeoutMs > 120_000 ||
      !Number.isInteger(this.maxPending) || this.maxPending < 1 || this.maxPending > 32) throw new Error("Invalid asset worker limits.");
    port.onmessage = event => {
      const response = event.data;
      if (!response || typeof response.requestId !== "string") return this.dispose(new Error("The asset worker returned an invalid response."));
      const pending = this.pending.get(response.requestId);
      if (!pending) return; // Cancelled requests may finish later.
      if (response.type === "error") pending.done(new Error(`${response.source ? `${response.source}: ` : ""}${response.message}`));
      else if (response.type === "cancelled") pending.done(new Cancelled("Import cancelled."));
      else if (response.type !== pending.expected) this.dispose(new Error("The asset worker returned an unexpected response."));
      else pending.done(undefined, response);
    };
    port.onerror = () => this.dispose(new Error("The asset worker stopped. Reopen the importer."));
    port.onmessageerror = () => this.dispose(new Error("The asset worker response could not be read."));
  }

  private id(): string { return `asset-${++this.next}`; }
  private cancelRequest(target: string): void {
    try { this.port.postMessage({ type: "cancel", requestId: this.id(), target }, []); }
    catch (error) { this.dispose(errorOf(error)); }
  }
  private request(message: AssetImportRequest, expected: AssetImportResponse["type"], transfer: Transferable[] = [], signal?: AbortSignal): Promise<AssetImportResponse> {
    if (this.closed) return Promise.reject(new Error("The asset worker has closed."));
    if (signal?.aborted) return Promise.reject(new Cancelled("Import cancelled."));
    if (this.pending.size >= this.maxPending) return Promise.reject(new Error("Wait for the current imports to finish."));
    return new Promise((resolve, reject) => {
      const abort = () => { done(new Cancelled("Import cancelled.")); this.cancelRequest(message.requestId); };
      const timer = setTimeout(() => this.dispose(new Error("The asset import took too long. Reopen the importer.")), this.timeoutMs);
      const done = (error?: Error, result?: AssetImportResponse) => {
        if (!this.pending.delete(message.requestId)) return;
        clearTimeout(timer);
        signal?.removeEventListener("abort", abort);
        if (error) reject(error); else resolve(result!);
      };
      this.pending.set(message.requestId, { expected, done });
      signal?.addEventListener("abort", abort, { once: true });
      try { this.port.postMessage(message, transfer); }
      catch (error) { this.dispose(errorOf(error)); }
    });
  }

  async compile(root: SourceBytes, files: readonly SourceBytes[], revision: number, options: Options = {}): Promise<Compiled> {
    let uploads = 0, uploadedBytes = 0;
    const inputs = [root, ...files.filter(file => file.record.id !== root.record.id)];
    const keys = inputs.map(input => sourceKey(input.record));
    if (inputs.some(input => !(input.bytes instanceof Uint8Array) || input.bytes.byteLength !== input.record.bytes)) {
      throw new Error("The asset source size does not match.");
    }
    if (inputs.length > MAX_SOURCE_COUNT || inputs.reduce((total, input) => total + input.record.bytes, 0) > MAX_SOURCE_STORE_BYTES) {
      throw new Error("This import needs too many source bytes.");
    }
    // An upload can evict an earlier input. Refill missing pins before conversion.
    for (let attempt = 0; keys.some(key => !this.known.has(key)); attempt++) {
      if (attempt >= inputs.length * 2) throw new Error("The asset sources changed during upload. Try again.");
      const input = inputs.find((_, index) => !this.known.has(keys[index]))!;
      const key = sourceKey(input.record);
      const bytes = new Uint8Array(input.bytes);
      const response = await this.request({ type: "put", requestId: this.id(), source: { record: structuredClone(input.record), bytes } },
        "stored", [bytes.buffer], options.signal);
      if (response.type !== "stored" || !Array.isArray(response.evicted)) throw new Error("The asset worker did not store this source.");
      for (const evicted of response.evicted) this.known.delete(evicted);
      this.known.add(key);
      uploads++; uploadedBytes += input.record.bytes;
    }
    const response = await this.request({ type: "compile", requestId: this.id(), revision,
      source: structuredClone(root.record), files: inputs.slice(1).map(file => structuredClone(file.record)) }, "compiled", [], options.signal);
    if (response.type !== "compiled" || response.version !== 1 || response.revision !== revision) throw new Error("The imported asset revision does not match.");
    return { ...response, metrics: { ...response.metrics, uploads, uploadedBytes } };
  }

  async stats(): Promise<WorkerStats> {
    const response = await this.request({ type: "stats", requestId: this.id() }, "stats");
    if (response.type !== "stats") throw new Error("The asset worker statistics are unavailable.");
    return response.stats;
  }
  dispose(error = new Error("The asset worker has closed.")): void {
    if (this.closed) return;
    this.closed = true;
    for (const pending of [...this.pending.values()]) pending.done(error);
    this.known.clear();
    this.port.onmessage = this.port.onerror = this.port.onmessageerror = null;
    this.port.terminate();
  }
}

type ImportFailure =
  | { type: "failed"; id: string; message: string; source: string }
  | { type: "stale" | "cancelled"; id: string };
export type AssetImportOutcome = (
  | { type: "accepted"; id: string; description: AssetDescription; revision: number; metrics: AssetImportMetrics }
  | ImportFailure
) & { rebuilds?: AssetImportOutcome[] };
export type AssetSourceUpdate = { type: "stored"; id: string; revision: number; rebuilds: AssetImportOutcome[] } | ImportFailure;
type Job = { id: string; controller: AbortController; reason?: "stale" | "cancelled"; committing: boolean; detach(): void };

/** Publishes exact source pins before synchronously selecting prepared resources. */
export class AssetImportSession {
  private readonly store: AssetSourceStore;
  private readonly client: AssetImportClient;
  private readonly runtime: AssetRuntime;
  private readonly jobs = new Map<string, Job>();
  private closed = false;
  private commits = 0;

  constructor(store: AssetSourceStore, client: AssetImportClient, runtime: AssetRuntime) {
    this.store = store; this.client = client; this.runtime = runtime;
  }
  private begin(id: string, signal?: AbortSignal): Job {
    const previous = this.jobs.get(id);
    if (previous && !previous.committing) { previous.reason = "stale"; previous.controller.abort(); }
    const controller = new AbortController();
    const job: Job = { id, controller, committing: false, detach: () => signal?.removeEventListener("abort", abort) };
    const abort = () => { if (!job.committing) { job.reason = "cancelled"; controller.abort(); } };
    signal?.addEventListener("abort", abort, { once: true });
    if (signal?.aborted) abort();
    this.jobs.set(id, job);
    return job;
  }
  private check(job: Job): void {
    if (this.closed || job.controller.signal.aborted || this.jobs.get(job.id) !== job) throw new Cancelled("Import cancelled.");
  }
  private finish(job: Job): void {
    job.detach();
    if (this.jobs.get(job.id) === job) this.jobs.delete(job.id);
  }
  private failure(job: Job, source: string, error: unknown): ImportFailure {
    if (this.closed || job.controller.signal.aborted || error instanceof Cancelled || this.jobs.get(job.id) !== job) {
      return { type: job.reason ?? (this.closed ? "cancelled" : "stale"), id: job.id };
    }
    return { type: "failed", id: job.id, source, message: errorOf(error).message };
  }
  cancel(id: string): void {
    const job = this.jobs.get(id);
    if (job && !job.committing) { job.reason = "cancelled"; job.controller.abort(); }
  }

  private async convert(job: Job, root: SourceRecord, inputs: SourceRecord[], revision: number, restoring = false): Promise<AssetImportOutcome> {
    let prepared: PreparedAsset | undefined;
    try {
      this.check(job);
      const files = await Promise.all(inputs.map(async record => ({ record, bytes: await this.store.read(record) })));
      this.check(job);
      const result = await this.client.compile(files.find(file => sameSource(file.record, root))!, files, revision, { signal: job.controller.signal });
      validateImportedAsset(result.description, root, inputs);
      this.check(job);
      prepared = await this.runtime.prepare(result.description);
      this.check(job);
      if (restoring) {
        const saved = (await this.store.list()).find(source => source.record.id === job.id)?.accepted;
        if (!saved || saved.revision !== revision || saved.sources.length !== inputs.length ||
          !saved.sources.every(pin => inputs.some(input => sameSource(input, pin)))) return { type: "stale", id: job.id };
        this.check(job);
      } else {
        // Cancellation stops here: a successful manifest transaction must be followed by commit.
        job.committing = true; this.commits++;
        if (!await this.store.accept(job.id, revision, result.description.sources)) return { type: "stale", id: job.id };
      }
      this.runtime.commit(job.id, revision, prepared);
      prepared = undefined;
      return { type: "accepted", id: job.id, revision, description: copyImportedAsset(result.description), metrics: result.metrics };
    } catch (error) { return this.failure(job, root.path, error); }
    finally {
      prepared?.dispose();
      if (job.committing) { job.committing = false; this.commits--; }
      if (this.closed && this.commits === 0) this.runtime.dispose();
    }
  }

  async importAsset(id: string, kind: SourceKind, path: string, bytes: Uint8Array, options: Options = {}): Promise<AssetImportOutcome> {
    if (this.closed) return { type: "cancelled", id };
    const job = this.begin(id, options.signal);
    let outcome: AssetImportOutcome;
    let dependents: StoredSource[] = [];
    try {
      if (!(bytes instanceof Uint8Array) || bytes.byteLength < 1 || bytes.byteLength > MAX_SOURCE_BYTES) {
        throw new Error("Choose an asset file up to 8 MiB.");
      }
      // Capture before the first await, including when the store is busy.
      const owned = new Uint8Array(bytes);
      this.check(job);
      const before = await this.store.list();
      const previous = before.find(source => source.record.id === id);
      this.check(job);
      const saved = await this.store.put(id, kind, path, owned);
      this.check(job);
      if (!previous || !sameSource(previous.record, saved.record)) {
        dependents = before.filter(source => source.record.id !== id && source.accepted?.sources.some(pin => pin.id === id));
      }
      const inputs = (await this.store.list()).map(source => source.record);
      outcome = await this.convert(job, saved.record, inputs, saved.revision);
    } catch (error) { outcome = this.failure(job, path, error); }
    finally { this.finish(job); }
    const rebuilds = outcome.type !== "cancelled" && outcome.type !== "stale" ? await this.rebuild(dependents, options) : [];
    return { ...outcome, ...(rebuilds.length ? { rebuilds } : {}) };
  }

  private async rebuild(dependents: StoredSource[], options: Options): Promise<AssetImportOutcome[]> {
    const rebuilds: AssetImportOutcome[] = [];
    for (const dependent of dependents) {
      if (this.closed) break;
      const next = this.begin(dependent.record.id, options.signal);
      try {
        const sources = await this.store.list();
        const source = sources.find(item => item.record.id === next.id);
        if (source) rebuilds.push(await this.convert(next, source.record, sources.map(item => item.record), source.revision));
      } catch (error) { rebuilds.push(this.failure(next, dependent.record.path, error));
      } finally { this.finish(next); }
    }
    return rebuilds;
  }

  /** Edits a saved dependency, such as a palette or binary buffer, without importing it as a model. */
  async updateSource(id: string, kind: SourceKind, path: string, bytes: Uint8Array, options: Options = {}): Promise<AssetSourceUpdate> {
    if (this.closed) return { type: "cancelled", id };
    const job = this.begin(id, options.signal);
    let saved: StoredSource, dependents: StoredSource[];
    try {
      if (!(bytes instanceof Uint8Array) || bytes.byteLength < 1 || bytes.byteLength > MAX_SOURCE_BYTES) {
        throw new Error("Choose an asset file up to 8 MiB.");
      }
      const owned = new Uint8Array(bytes);
      this.check(job);
      const before = await this.store.list();
      this.check(job);
      saved = await this.store.put(id, kind, path, owned);
      this.check(job);
      dependents = before.filter(source => source.accepted?.sources.some(pin => pin.id === id));
    } catch (error) { return this.failure(job, path, error); }
    finally { this.finish(job); }
    return { type: "stored", id, revision: saved.revision, rebuilds: await this.rebuild(dependents, options) };
  }

  async restore(): Promise<AssetImportOutcome[]> {
    if (this.closed) return [];
    const outcomes: AssetImportOutcome[] = [];
    for (const source of await this.store.list()) {
      if (!source.accepted) continue;
      const pins = source.accepted.sources, root = pins.find(pin => pin.id === source.record.id)!;
      const job = this.begin(root.id);
      try { outcomes.push(await this.convert(job, root, pins, source.accepted.revision, true)); }
      finally { this.finish(job); }
    }
    return outcomes;
  }
  dispose(): void {
    if (this.closed) return;
    this.closed = true;
    for (const job of this.jobs.values()) if (!job.committing) { job.reason = "cancelled"; job.controller.abort(); }
    this.client.dispose();
    if (this.commits === 0) this.runtime.dispose();
  }
}

/** Use within Effect.scoped so the worker and GPU resources always close together. */
export function acquireAssetImportSession(projectId: string, device: GPUDevice) {
  return Effect.acquireRelease(Effect.tryPromise({
    try: async () => {
      const store = await AssetSourceStore.open(projectId);
      const client = new AssetImportClient(new Worker(new URL("./asset-import-worker.ts", import.meta.url), { type: "module" }));
      return new AssetImportSession(store, client, new AssetRuntimeAdapter(device));
    }, catch: errorOf,
  }), session => Effect.sync(() => session.dispose()));
}
