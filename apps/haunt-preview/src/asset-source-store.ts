import type { SourceKind, SourceRecord } from "./asset-compiler.ts";
import {
  assetPath,
  sourceKey,
  MAX_SOURCE_BYTES,
  MAX_SOURCE_STORE_BYTES,
  MAX_SOURCE_COUNT,
} from "./asset-import-protocol.ts";
import { sourceBytesHash } from "./protocol.ts";

export interface AssetStoreBackend {
  read(key: string): Promise<Uint8Array | undefined>;
  /** Replace a value atomically; a rejected write keeps its previous contents. */
  write(key: string, bytes: Uint8Array): Promise<void>;
  remove(key: string): Promise<void>;
}

export type StoredSource = {
  record: SourceRecord;
  original: SourceRecord;
  revision: number;
  accepted?: { revision: number; sources: SourceRecord[] };
};

type Manifest = { version: 1; sources: StoredSource[] };
const MANIFEST = "manifest.json";
const copy = <T>(value: T): T => structuredClone(value);
const objectKey = (source: SourceRecord) => `object-${source.hash}`;
const equal = (a: SourceRecord, b: SourceRecord) =>
  a.id === b.id && a.kind === b.kind && a.path === b.path &&
  a.hash === b.hash && a.bytes === b.bytes;

function checkedRecord(record: SourceRecord): SourceRecord {
  if (!record || typeof record.id !== "string") throw new Error("The asset source is invalid.");
  sourceKey(record);
  return { id: record.id, kind: record.kind, path: record.path, bytes: record.bytes, hash: record.hash };
}

function records(sources: StoredSource[]): SourceRecord[] {
  return sources.flatMap((s) => [s.original, s.record, ...(s.accepted?.sources ?? [])]);
}

function validate(sources: StoredSource[]): void {
  if (!Array.isArray(sources) || sources.length > MAX_SOURCE_COUNT) {
    throw new Error("This project has too many asset sources.");
  }
  const ids = new Set<string>();
  const sizes = new Map<string, number>();
  for (const source of sources) {
    if (!source || !Number.isSafeInteger(source.revision) || source.revision < 1 ||
      !source.record || !source.original || source.record.id !== source.original.id ||
      ids.has(source.record.id)) throw new Error("The saved asset list is invalid.");
    ids.add(source.record.id);
    if (source.accepted !== undefined && (!source.accepted || !Number.isSafeInteger(source.accepted.revision) ||
      source.accepted.revision < 1 || source.accepted.revision > source.revision ||
      !Array.isArray(source.accepted.sources) || !source.accepted.sources.length ||
      source.accepted.sources.length > MAX_SOURCE_COUNT ||
      !source.accepted.sources.some((s) => s.id === source.record.id))) {
      throw new Error("The saved asset versions are invalid.");
    }
    const acceptedIds = new Set<string>();
    for (const record of source.accepted?.sources ?? []) {
      if (acceptedIds.has(record.id)) throw new Error("The saved asset versions are invalid.");
      acceptedIds.add(record.id);
    }
  }
  for (const record of records(sources)) {
    checkedRecord(record);
    if (!ids.has(record.id)) throw new Error("The saved asset versions are invalid.");
    if (sizes.has(record.hash) && sizes.get(record.hash) !== record.bytes) {
      throw new Error("The saved asset sizes do not match.");
    }
    sizes.set(record.hash, record.bytes);
  }
  if ([...sizes.values()].reduce((total, n) => total + n, 0) > MAX_SOURCE_STORE_BYTES) {
    throw new Error("This project's asset sources are full.");
  }
}

class OpfsBackend implements AssetStoreBackend {
  private readonly directory: FileSystemDirectoryHandle;
  constructor(directory: FileSystemDirectoryHandle) { this.directory = directory; }
  private key(key: string): string {
    if (key !== MANIFEST && !/^object-[a-f0-9]{64}$/.test(key)) {
      throw new Error("Choose an asset in this project.");
    }
    return key;
  }
  async read(key: string): Promise<Uint8Array | undefined> {
    try {
      const file = await (await this.directory.getFileHandle(this.key(key))).getFile();
      if (file.size > MAX_SOURCE_STORE_BYTES) throw new Error("The saved asset file is too large.");
      return new Uint8Array(await file.arrayBuffer());
    } catch (error) {
      if (error instanceof DOMException && error.name === "NotFoundError") return undefined;
      throw error;
    }
  }
  async write(key: string, bytes: Uint8Array): Promise<void> {
    const file = await this.directory.getFileHandle(this.key(key), { create: true });
    const stream = await file.createWritable();
    try {
      await stream.write(new Uint8Array(bytes).buffer);
      await stream.close();
    } catch (error) {
      await stream.abort().catch(() => {});
      throw error;
    }
  }
  async remove(key: string): Promise<void> {
    try { await this.directory.removeEntry(this.key(key)); }
    catch (error) {
      if (!(error instanceof DOMException && error.name === "NotFoundError")) throw error;
    }
  }
}

/** Original files and last-working source versions stay separate from later drafts. */
export class AssetSourceStore {
  private readonly backend: AssetStoreBackend;
  private sources: StoredSource[] = [];
  private initialization?: Promise<void>;
  private queue: Promise<void> = Promise.resolve();

  constructor(backend: AssetStoreBackend) { this.backend = backend; }

  static async open(projectId: string): Promise<AssetSourceStore> {
    if (typeof projectId !== "string" || !/^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/.test(projectId)) {
      throw new Error("Choose a valid project for these assets.");
    }
    if (typeof navigator === "undefined" || !navigator.storage?.getDirectory) {
      throw new Error("This browser cannot save asset files here.");
    }
    const root = await navigator.storage.getDirectory();
    const projects = await root.getDirectoryHandle("openagents-asset-sources-v1", { create: true });
    const directory = await projects.getDirectoryHandle(projectId, { create: true });
    const store = new AssetSourceStore(new OpfsBackend(directory));
    await store.initialize();
    return store;
  }

  initialize(): Promise<void> {
    if (!this.initialization) {
      this.initialization = (async () => {
        const bytes = await this.backend.read(MANIFEST);
        if (!bytes) return;
        if (bytes.length > MAX_SOURCE_STORE_BYTES) throw new Error("The saved asset list is too large.");
        const manifest: Manifest = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes));
        if (manifest?.version !== 1) throw new Error("This asset list has an unsupported version.");
        validate(manifest.sources);
        this.sources = manifest.sources.map((s) => ({
          record: checkedRecord(s.record), original: checkedRecord(s.original), revision: s.revision,
          ...(s.accepted ? { accepted: { revision: s.accepted.revision, sources: s.accepted.sources.map(checkedRecord) } } : {}),
        }));
      })().catch((error) => { this.initialization = undefined; throw error; });
    }
    return this.initialization;
  }

  private serialized<T>(action: () => Promise<T>): Promise<T> {
    const result = this.queue.then(async () => { await this.initialize(); return action(); });
    this.queue = result.then(() => {}, () => {});
    return result;
  }

  private async publish(sources: StoredSource[]): Promise<void> {
    validate(sources);
    const bytes = new TextEncoder().encode(JSON.stringify({ version: 1, sources }));
    if (bytes.length > MAX_SOURCE_STORE_BYTES) throw new Error("The saved asset list is too large.");
    const old = new Set(records(this.sources).map(objectKey));
    await this.backend.write(MANIFEST, bytes);
    this.sources = sources;
    const kept = new Set(records(sources).map(objectKey));
    // Cleanup cannot turn a completed save into a reported failure.
    for (const key of old) if (!kept.has(key)) await this.backend.remove(key).catch(() => {});
  }

  async put(id: string, kind: SourceKind, path: string, bytes: Uint8Array): Promise<StoredSource> {
    if (!(bytes instanceof Uint8Array) || !bytes.length || bytes.length > MAX_SOURCE_BYTES) {
      throw new Error("The asset source is empty or too large.");
    }
    const owned = new Uint8Array(bytes);
    const localPath = assetPath(path);
    return this.serialized(async () => {
      const record: SourceRecord = { id, kind, path: localPath, bytes: owned.length, hash: await sourceBytesHash(owned) };
      checkedRecord(record);
      const previous = this.sources.find((s) => s.record.id === id);
      if (previous && equal(previous.record, record)) return copy(previous);
      const revision = previous ? previous.revision + 1 : 1;
      if (!Number.isSafeInteger(revision)) throw new Error("This asset has too many versions.");
      const next: StoredSource = previous
        ? { ...copy(previous), record, revision }
        : { record, original: copy(record), revision };
      const sources = this.sources.filter((s) => s.record.id !== id).concat(next);
      validate(sources);
      const key = objectKey(record);
      const existing = await this.backend.read(key);
      if (existing && (existing.length !== record.bytes || await sourceBytesHash(existing) !== record.hash)) {
        throw new Error("The saved asset file has changed.");
      }
      try {
        if (!existing) await this.backend.write(key, owned);
        await this.publish(sources);
      } catch (error) {
        if (!existing) await this.backend.remove(key).catch(() => {});
        throw error;
      }
      return copy(next);
    });
  }

  async read(record: SourceRecord): Promise<Uint8Array> {
    const wanted = checkedRecord(record);
    return this.serialized(async () => {
      if (!records(this.sources).some((r) => equal(r, wanted))) throw new Error("This asset version is not saved in this project.");
      const bytes = await this.backend.read(objectKey(wanted));
      if (!bytes || bytes.length !== wanted.bytes || await sourceBytesHash(bytes) !== wanted.hash) {
        throw new Error("The saved asset file is missing or has changed.");
      }
      return new Uint8Array(bytes);
    });
  }

  list(): Promise<StoredSource[]> {
    return this.serialized(async () => copy(this.sources));
  }

  async accept(id: string, expectedRevision: number, sources: SourceRecord[]): Promise<boolean> {
    if (!Array.isArray(sources) || !sources.length || sources.length > MAX_SOURCE_COUNT) throw new Error("Choose the sources used by this asset.");
    const pins = sources.map(checkedRecord);
    if (new Set(pins.map((s) => s.id)).size !== pins.length || !pins.some((s) => s.id === id)) {
      throw new Error("Choose the sources used by this asset.");
    }
    return this.serialized(async () => {
      const current = this.sources.find((s) => s.record.id === id);
      if (!current || current.revision !== expectedRevision || pins.some((pin) =>
        !this.sources.some((s) => equal(s.record, pin)))) return false;
      // Check storage before identifying these exact bytes as the last working version.
      for (const pin of pins) {
        const bytes = await this.backend.read(objectKey(pin));
        if (!bytes || bytes.length !== pin.bytes || await sourceBytesHash(bytes) !== pin.hash) {
          throw new Error("The saved asset file is missing or has changed.");
        }
      }
      const next = copy(this.sources);
      next.find((s) => s.record.id === id)!.accepted = { revision: expectedRevision, sources: pins };
      await this.publish(next);
      return true;
    });
  }
}
