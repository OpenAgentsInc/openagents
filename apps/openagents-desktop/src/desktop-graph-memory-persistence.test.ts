import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

import { openSqliteDatabase } from "@openagentsinc/sqlite-runtime";
import { afterEach, describe, expect, test } from "vite-plus/test";

import {
  DesktopGraphMemoryPersistenceError,
  openDesktopGraphMemoryPersistence,
} from "./desktop-graph-memory-persistence.js";
import type { SafeStorageLike } from "./desktop-session-vault.js";

const roots: Array<string> = [];
const temporaryRoot = (): string => {
  const root = mkdtempSync(path.join(tmpdir(), "openagents-graph-memory-"));
  roots.push(root);
  return root;
};

afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { force: true, recursive: true });
});

const safeStorage = (backend = "keychain_access"): SafeStorageLike => ({
  isEncryptionAvailable: () => true,
  getSelectedStorageBackend: () => backend,
  encryptString: (plaintext) => Buffer.from(`wrapped:${plaintext}`, "utf8"),
  decryptString: (encrypted) => {
    const value = encrypted.toString("utf8");
    if (!value.startsWith("wrapped:")) throw new Error("invalid wrapped value");
    return value.slice("wrapped:".length);
  },
});

const scopeA = { ownerScope: "owner.a", projectScope: "project.a" } as const;
const scopeB = { ownerScope: "owner.b", projectScope: "project.a" } as const;

describe("desktop graph-memory persistence", () => {
  test("disabled mode performs zero safe-storage, path, SQLite, and crypto work", () => {
    let touched = false;
    const unavailable: SafeStorageLike = {
      isEncryptionAvailable: () => {
        touched = true;
        throw new Error("must not run");
      },
      encryptString: () => {
        touched = true;
        throw new Error("must not run");
      },
      decryptString: () => {
        touched = true;
        throw new Error("must not run");
      },
    };
    const persistence = openDesktopGraphMemoryPersistence({
      enabled: false,
      databasePath: "/must/not/exist/graph-memory.sqlite",
      safeStorage: unavailable,
      openDatabase: () => {
        touched = true;
        throw new Error("must not run");
      },
    });

    expect(persistence.enabled).toBe(false);
    expect(persistence.load(scopeA)).toBeNull();
    persistence.save(scopeA, { revision: 1, payload: "not-written" });
    expect(persistence.compareAndSet(scopeA, null, { revision: 1, payload: "not-written" })).toBe(
      false,
    );
    expect(persistence.remove(scopeA)).toBe(false);
    persistence.close();
    expect(persistence.stats()).toEqual({ reads: 0, writes: 0 });
    expect(touched).toBe(false);
  });

  test("refuses unavailable and basic-text key custody before opening SQLite", () => {
    for (const storage of [
      { ...safeStorage(), isEncryptionAvailable: () => false },
      safeStorage("basic_text"),
    ]) {
      let opened = false;
      expect(() =>
        openDesktopGraphMemoryPersistence({
          enabled: true,
          databasePath: path.join(temporaryRoot(), "graph-memory.sqlite"),
          safeStorage: storage,
          openDatabase: () => {
            opened = true;
            throw new Error("must not open");
          },
        }),
      ).toThrowError(DesktopGraphMemoryPersistenceError);
      expect(opened).toBe(false);
    }
  });

  test("round-trips sealed scope state and keeps plaintext out of SQLite", () => {
    const databasePath = path.join(temporaryRoot(), "private", "graph-memory.sqlite");
    const sentinel = "OWNER_PRIVATE_GRAPH_SENTINEL_9217";
    const persistence = openDesktopGraphMemoryPersistence({
      enabled: true,
      databasePath,
      safeStorage: safeStorage(),
    });

    persistence.save(scopeA, { revision: 7, payload: JSON.stringify({ sentinel }) });
    expect(persistence.load(scopeA)).toEqual({
      revision: 7,
      payload: JSON.stringify({ sentinel }),
    });
    expect(persistence.load(scopeB)).toBeNull();
    expect(persistence.stats()).toEqual({ reads: 2, writes: 1 });
    expect(persistence.compareAndSet(scopeA, 6, { revision: 8, payload: "stale-write" })).toBe(
      false,
    );
    expect(
      persistence.compareAndSet(scopeA, 7, {
        revision: 8,
        payload: JSON.stringify({ sentinel, next: true }),
      }),
    ).toBe(true);
    expect(persistence.load(scopeA)?.revision).toBe(8);
    persistence.close();

    for (const candidate of [databasePath, `${databasePath}-wal`, `${databasePath}-shm`]) {
      if (existsSync(candidate)) {
        expect(readFileSync(candidate).includes(Buffer.from(sentinel, "utf8"))).toBe(false);
      }
    }

    const reopened = openDesktopGraphMemoryPersistence({
      enabled: true,
      databasePath,
      safeStorage: safeStorage(),
    });
    expect(reopened.load(scopeA)?.revision).toBe(8);
    expect(reopened.remove(scopeA)).toBe(true);
    expect(reopened.remove(scopeA)).toBe(false);
    expect(reopened.load(scopeA)).toBeNull();
    reopened.close();
  });

  test("fails closed when ciphertext is changed", () => {
    const databasePath = path.join(temporaryRoot(), "graph-memory.sqlite");
    const persistence = openDesktopGraphMemoryPersistence({
      enabled: true,
      databasePath,
      safeStorage: safeStorage(),
    });
    persistence.save(scopeA, { revision: 1, payload: "sealed" });
    persistence.close();

    const database = openSqliteDatabase(databasePath);
    database.run(
      "UPDATE graph_memory_scopes SET ciphertext = ? WHERE owner_scope = ? AND project_scope = ?",
      [Buffer.from("changed", "utf8"), scopeA.ownerScope, scopeA.projectScope],
    );
    database.close();

    const reopened = openDesktopGraphMemoryPersistence({
      enabled: true,
      databasePath,
      safeStorage: safeStorage(),
    });
    expect(() => reopened.load(scopeA)).toThrowError(DesktopGraphMemoryPersistenceError);
    reopened.close();
  });

  test("refuses a future schema version without mutating it", () => {
    const databasePath = path.join(temporaryRoot(), "graph-memory.sqlite");
    const persistence = openDesktopGraphMemoryPersistence({
      enabled: true,
      databasePath,
      safeStorage: safeStorage(),
    });
    persistence.close();

    const database = openSqliteDatabase(databasePath);
    database.run("UPDATE graph_memory_metadata SET wrapped_key = ? WHERE key_ref = ?", [
      "99",
      "graph-memory-schema-version",
    ]);
    database.close();

    expect(() =>
      openDesktopGraphMemoryPersistence({
        enabled: true,
        databasePath,
        safeStorage: safeStorage(),
      }),
    ).toThrowError(expect.objectContaining({ reason: "incompatible_version" }));
  });
});
