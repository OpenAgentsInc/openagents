/**
 * The storage driver port: a string key-value surface with NO atomicity
 * assumptions. Atomicity and torn-write detection are supplied above this
 * port by the journal (`journal.ts`), so any backend that can store strings
 * is safe — including ones (like `localStorage`) that guarantee nothing
 * across a crash.
 *
 * Bindings:
 * - `memoryStringKv` — tests and SSR.
 * - `webStorageStringKv` — wraps a DOM `Storage` (localStorage). Synchronous
 *   underneath, wrapped in Effect for a uniform surface.
 * - IndexedDB — an app-shell concern (SWAP-0/SWAP-7 wiring): implement this
 *   same port over an object store there. Its per-operation transactions
 *   only strengthen the journal's guarantees; nothing in this package
 *   depends on them. This package deliberately ships no raw IndexedDB code
 *   because the repo test runtime has no IndexedDB and an untested binding
 *   on a funds path is worse than none.
 */
import { Effect } from "effect";

import { StorageDriverError } from "./errors.js";

export interface StringKv {
  readonly get: (key: string) => Effect.Effect<string | null, StorageDriverError>;
  readonly set: (key: string, value: string) => Effect.Effect<void, StorageDriverError>;
  readonly delete: (key: string) => Effect.Effect<void, StorageDriverError>;
  /** Every stored key beginning with `prefix`. */
  readonly keys: (prefix: string) => Effect.Effect<ReadonlyArray<string>, StorageDriverError>;
}

export const memoryStringKv = (
  initial?: Readonly<Record<string, string>>,
): StringKv & { readonly snapshot: () => Record<string, string> } => {
  const data = new Map<string, string>(Object.entries(initial ?? {}));
  return {
    get: (key) => Effect.sync(() => data.get(key) ?? null),
    set: (key, value) =>
      Effect.sync(() => {
        data.set(key, value);
      }),
    delete: (key) =>
      Effect.sync(() => {
        data.delete(key);
      }),
    keys: (prefix) => Effect.sync(() => [...data.keys()].filter((key) => key.startsWith(prefix))),
    snapshot: () => Object.fromEntries(data.entries()),
  };
};

/** Structural subset of the DOM `Storage` interface. */
export interface StorageLike {
  readonly length: number;
  key(index: number): string | null;
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

const attempt = <A>(operation: "get" | "set" | "delete" | "keys", key: string, run: () => A) =>
  Effect.try({
    try: run,
    catch: (error) =>
      new StorageDriverError({
        operation,
        key,
        detail: error instanceof Error ? error.message : String(error),
      }),
  });

export const webStorageStringKv = (storage: StorageLike): StringKv => {
  return {
    get: (key) => attempt("get", key, () => storage.getItem(key)),
    set: (key, value) => attempt("set", key, () => storage.setItem(key, value)),
    delete: (key) => attempt("delete", key, () => storage.removeItem(key)),
    keys: (prefix) =>
      attempt("keys", prefix, () => {
        const found: string[] = [];
        for (let index = 0; index < storage.length; index += 1) {
          const key = storage.key(index);
          if (key !== null && key.startsWith(prefix)) found.push(key);
        }
        return found;
      }),
  };
};
