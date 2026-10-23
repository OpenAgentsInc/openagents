import { LAB } from "./lab.ts";
export const PROJECT = LAB.id;
export const ASSET = LAB.material;
export const MAX_SOURCE_BYTES = 64 * 1024;

export type ShaderEdit = {
  type: "shader";
  project: typeof PROJECT;
  preview: string;
  token: string;
  request: string;
  revision: number;
  asset: typeof ASSET;
  hash: string;
  source: string;
  savedAt: number;
};

export function validEdit(
  value: unknown,
  preview: string,
  token: string,
): value is ShaderEdit {
  if (!value || typeof value !== "object") return false;
  const e = value as ShaderEdit;
  return (
    e.type === "shader" &&
    e.project === PROJECT &&
    e.preview === preview &&
    e.token === token &&
    e.asset === ASSET &&
    typeof e.request === "string" &&
    e.request.length <= 128 &&
    Number.isSafeInteger(e.revision) &&
    e.revision > 0 &&
    typeof e.hash === "string" &&
    /^[a-f0-9]{64}$/.test(e.hash) &&
    typeof e.source === "string" &&
    new TextEncoder().encode(e.source).length <= MAX_SOURCE_BYTES &&
    Number.isFinite(e.savedAt)
  );
}

export async function sourceHash(source: string): Promise<string> {
  return sourceBytesHash(new TextEncoder().encode(source));
}

/** Hashes the exact bytes, independent of decoding or array offsets. */
export async function sourceBytesHash(bytes: Uint8Array): Promise<string> {
  const owned = new Uint8Array(bytes.byteLength);
  owned.set(bytes);
  const digest = await crypto.subtle.digest("SHA-256", owned);
  return [...new Uint8Array(digest)]
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

type Version<T> = { revision: number; hash: string; resource: T };

/** The running material is independent of drafts and asynchronous compilers. */
export class ShaderVersions<T> {
  draft = 0;
  active: Version<T> | undefined;
  pending: Version<T> | undefined;
  disposed = false;
  private readonly release: (resource: T) => void;

  constructor(release: (resource: T) => void) {
    this.release = release;
  }

  begin(revision: number): boolean {
    if (this.disposed || revision <= this.draft) return false;
    this.draft = revision;
    if (this.pending) this.release(this.pending.resource);
    this.pending = undefined;
    return true;
  }

  stage(revision: number, hash: string, resource: T): boolean {
    if (this.disposed || revision !== this.draft) {
      this.release(resource);
      return false;
    }
    if (this.pending) this.release(this.pending.resource);
    this.pending = { revision, hash, resource };
    return true;
  }

  commit(): Version<T> | undefined {
    if (!this.pending || this.pending.revision !== this.draft || this.disposed)
      return;
    if (this.active) this.release(this.active.resource);
    this.active = this.pending;
    this.pending = undefined;
    return this.active;
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    if (this.pending) this.release(this.pending.resource);
    if (this.active) this.release(this.active.resource);
    this.pending = this.active = undefined;
  }
}
