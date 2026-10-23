import { compileAsset } from "./asset-compiler.ts";
import type { SourceRecord } from "./asset-compiler.ts";
import { sourceBytesHash } from "./protocol.ts";
import { compileOwnedSource } from "./asset-owned-source-adapter.ts";
import {
  ASSET_IMPORT_VERSION, MAX_COMPILED_CACHE_BYTES, MAX_SOURCE_COUNT, MAX_SOURCE_STORE_BYTES,
  copyImportedAsset, descriptionBytes, descriptionTransfers, sameSource, sourceKey, validateImportedAsset,
} from "./asset-import-protocol.ts";
import type { AssetConverter, AssetImportRequest, AssetImportResponse, OwnedAssetDescription, SourceBytes, WorkerStats } from "./asset-import-protocol.ts";

type Cached = { description: OwnedAssetDescription; bytes: number };

export class AssetWorkerEngine {
  private readonly sources = new Map<string, Uint8Array>();
  private readonly cache = new Map<string, Cached>();
  private readonly jobs = new Map<string, { cancelled: boolean }>();
  private sourceBytes = 0;
  private cacheBytes = 0;
  private conversions = 0;
  private readonly convert: AssetConverter;
  private readonly cacheLimit: number;

  constructor(convert: AssetConverter, cacheLimit = MAX_COMPILED_CACHE_BYTES) {
    if (!Number.isSafeInteger(cacheLimit) || cacheLimit < 1 || cacheLimit > MAX_COMPILED_CACHE_BYTES) throw new Error("Invalid asset cache limit.");
    this.convert = convert;
    this.cacheLimit = cacheLimit;
  }

  stats(): WorkerStats {
    return { sourceBytes: this.sourceBytes, sourceCount: this.sources.size, cacheBytes: this.cacheBytes,
      cacheEntries: this.cache.size, conversions: this.conversions, active: this.jobs.size };
  }

  async handle(request: AssetImportRequest): Promise<AssetImportResponse> {
    try {
      if (!request || typeof request.requestId !== "string" || !request.requestId || request.requestId.length > 128) {
        throw new Error("The import request is invalid.");
      }
      switch (request.type) {
        case "put": return await this.put(request);
        case "compile": return await this.compile(request);
        case "cancel": {
          const job = this.jobs.get(request.target);
          if (job) job.cancelled = true;
          return { type: "cancelled", requestId: request.requestId };
        }
        case "stats": return { type: "stats", requestId: request.requestId, stats: this.stats() };
        default: throw new Error("The import operation is unavailable.");
      }
    } catch (error) {
      return { type: "error", requestId: request?.requestId ?? "", source: request?.type === "compile" ? request.source?.path ?? "" : "",
        message: error instanceof Error ? error.message : "The asset could not be imported." };
    }
  }

  private async put(request: Extract<AssetImportRequest, { type: "put" }>): Promise<AssetImportResponse> {
    const { record, bytes } = request.source;
    const key = sourceKey(record);
    if (!(bytes instanceof Uint8Array) || bytes.byteLength !== record.bytes) throw new Error(`${record.path}: source size does not match.`);
    const owned = bytes.slice();
    if (await sourceBytesHash(owned) !== record.hash) throw new Error(`${record.path}: source bytes changed.`);
    const evicted: string[] = [];
    if (!this.sources.has(key)) {
      while (this.sources.size >= MAX_SOURCE_COUNT * 4 || this.sourceBytes + owned.byteLength > MAX_SOURCE_STORE_BYTES) {
        const first = this.sources.keys().next().value;
        if (first === undefined) throw new Error("The asset source store is full.");
        this.sourceBytes -= this.sources.get(first)!.byteLength;
        this.sources.delete(first);
        evicted.push(first);
      }
      this.sources.set(key, owned);
      this.sourceBytes += owned.byteLength;
    }
    return { type: "stored", requestId: request.requestId, evicted };
  }

  private async compile(request: Extract<AssetImportRequest, { type: "compile" }>): Promise<AssetImportResponse> {
    const began = performance.now();
    if (!Number.isSafeInteger(request.revision) || request.revision < 1 || !Array.isArray(request.files) || request.files.length > MAX_SOURCE_COUNT) {
      throw new Error("The asset revision is invalid.");
    }
    sourceKey(request.source);
    const records = new Map<string, SourceRecord>();
    for (const record of [request.source, ...request.files]) {
      sourceKey(record);
      const old = records.get(record.id);
      if (old && !sameSource(old, record)) throw new Error(`${record.path}: duplicate source ID.`);
      records.set(record.id, structuredClone(record));
    }
    const inputs = [...records.values()];
    if (inputs.length > MAX_SOURCE_COUNT || inputs.reduce((n, record) => n + record.bytes, 0) > MAX_SOURCE_STORE_BYTES) throw new Error("This import needs too many source bytes.");
    for (const [key, entry] of this.cache) {
      if (entry.description.id !== request.source.id || !entry.description.sources.some(record => sameSource(record, request.source)) ||
        !entry.description.sources.every(record => inputs.some(input => sameSource(input, record)))) continue;
      this.cache.delete(key);
      this.cache.set(key, entry);
      return { type: "compiled", version: ASSET_IMPORT_VERSION, requestId: request.requestId, revision: request.revision,
        description: structuredClone(entry.description), metrics: { compileMs: performance.now() - began,
          transferredBytes: entry.bytes, outputBytes: entry.bytes, cacheHit: true } };
    }
    if (this.jobs.size >= 2 || this.jobs.has(request.requestId)) throw new Error("Two imports are already running. Try again when one finishes.");
    const files: SourceBytes[] = inputs.map(record => {
      const bytes = this.sources.get(sourceKey(record));
      if (!bytes) throw new Error(`${record.path}: source bytes are missing. Reopen the asset and try again.`);
      return { record, bytes: bytes.slice() };
    });
    const root = files.find(file => file.record.id === request.source.id)!;
    const job = { cancelled: false };
    this.jobs.set(request.requestId, job);
    try {
      this.conversions++;
      const converted = await this.convert(root.record, root.bytes, files.filter(file => file !== root));
      if (job.cancelled) return { type: "cancelled", requestId: request.requestId };
      validateImportedAsset(converted, request.source, inputs);
      const description = copyImportedAsset(converted);
      const bytes = descriptionBytes(description);
      if (bytes <= this.cacheLimit) {
        const key = JSON.stringify([ASSET_IMPORT_VERSION, description.id, [...description.sources].sort((a, b) => a.id.localeCompare(b.id))]);
        const old = this.cache.get(key);
        if (old) this.cacheBytes -= old.bytes;
        this.cache.delete(key);
        this.cache.set(key, { description: structuredClone(description), bytes });
        this.cacheBytes += bytes;
        while (this.cacheBytes > this.cacheLimit || this.cache.size > MAX_SOURCE_COUNT) {
          const first = this.cache.keys().next().value!;
          this.cacheBytes -= this.cache.get(first)!.bytes;
          this.cache.delete(first);
        }
      }
      return { type: "compiled", version: ASSET_IMPORT_VERSION, requestId: request.requestId, revision: request.revision,
        description, metrics: { compileMs: performance.now() - began, transferredBytes: bytes, outputBytes: bytes, cacheHit: false } };
    } catch (error) {
      if (job.cancelled) return { type: "cancelled", requestId: request.requestId };
      throw error;
    } finally {
      this.jobs.delete(request.requestId);
    }
  }
}

const scope = globalThis as unknown as { document?: unknown; onmessage: ((event: MessageEvent<AssetImportRequest>) => void) | null;
  postMessage?: (value: AssetImportResponse, transfer: ArrayBuffer[]) => void };
if (typeof self !== "undefined" && typeof scope.document === "undefined" && scope.postMessage) {
  const convert: AssetConverter = compileAsset;
  const engine = new AssetWorkerEngine(async (source, bytes, files) =>
    await compileOwnedSource(source, bytes, files, convert) ?? await convert(source, bytes, files));
  scope.onmessage = event => {
    void engine.handle(event.data).then(response => scope.postMessage!(response,
      response.type === "compiled" ? descriptionTransfers(response.description) : []));
  };
}
