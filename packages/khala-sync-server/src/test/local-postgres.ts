import { spawnSync } from "node:child_process";
import { accessSync, constants, readFileSync } from "node:fs";
import { mkdir, readFile, rename, rm, stat, writeFile } from "node:fs/promises";
import * as net from "node:net";
import { tmpdir } from "node:os";
import * as path from "node:path";

/**
 * Throwaway local Postgres for integration tests (KS-0.3, reused by the
 * KS-2.x substrate lanes).
 *
 * `startLocalPostgres()` uses one temporary postmaster per machine-wide test
 * sweep. Each caller gets an isolated role and control database, so test
 * files can create their own databases without sharing data or credentials.
 * The final caller stops the postmaster and deletes its temporary data
 * directory.
 * A later caller reclaims state left by a killed test worker.
 *
 * Requires local Postgres server binaries (initdb/pg_ctl); on this repo's
 * dev Macs that is `brew install postgresql@16`. Tests should gate on
 * `hasLocalPostgres()` so machines without Postgres skip instead of fail.
 */

export interface LocalPostgres {
  /** Direct connection URL, e.g. postgres://postgres@127.0.0.1:54xxx/postgres */
  readonly url: string;
  readonly host: string;
  readonly port: number;
  readonly user: string;
  readonly dataDir: string;
  /** Connection URL for a specific database on this server. */
  readonly urlFor: (database: string) => string;
  /** Stop the server and delete the data directory. Safe to call once. */
  readonly stop: () => Promise<void>;
}

const PG_BIN_DIR_CANDIDATES = [
  "/opt/homebrew/opt/postgresql@16/bin",
  "/opt/homebrew/bin",
  "/usr/local/opt/postgresql@16/bin",
  "/usr/local/bin",
  "/usr/lib/postgresql/16/bin",
];

const REQUIRED_BINARIES = ["initdb", "pg_ctl", "psql"] as const;

const isExecutable = (file: string): boolean => {
  try {
    accessSync(file, constants.X_OK);
    return true;
  } catch {
    return false;
  }
};

/**
 * Resolve `binary` from PATH (runtime-agnostic `which`: this helper runs
 * under BOTH `pnpm exec vp test` in this package and vitest/Node in the
 * `openagents.com` Worker's stitch-seam suite, so no `Bun.*` APIs here).
 */
const whichOnPath = (binary: string): string | null => {
  for (const dir of (process.env.PATH ?? "").split(path.delimiter)) {
    if (dir === "") continue;
    const candidate = path.join(dir, binary);
    if (isExecutable(candidate)) return candidate;
  }
  return null;
};

/** Directory containing initdb + pg_ctl, or null if none is available. */
export const findPgBinDir = (): string | null => {
  for (const dir of PG_BIN_DIR_CANDIDATES) {
    if (REQUIRED_BINARIES.every((bin) => isExecutable(path.join(dir, bin)))) {
      return dir;
    }
  }
  // Fall back to PATH.
  if (REQUIRED_BINARIES.every((bin) => whichOnPath(bin) !== null)) {
    const resolved = whichOnPath("initdb");
    return resolved === null ? null : path.dirname(resolved);
  }
  return null;
};

export const hasLocalPostgres = (): boolean => findPgBinDir() !== null;

const freePort = (): Promise<number> =>
  new Promise((resolve, reject) => {
    const server = net.createServer();
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address() as net.AddressInfo;
      server.close(() => resolve(address.port));
    });
  });

const run = (cmd: ReadonlyArray<string>): void => {
  const [binary, ...args] = cmd;
  if (binary === undefined) throw new Error("empty command");
  const result = spawnSync(binary, args, { encoding: "utf8" });
  if (result.error !== undefined) throw result.error;
  if (result.status !== 0) {
    const output = `${result.stdout}\n${result.stderr}`;
    // Every postmaster holds one System V shared-memory segment, and macOS
    // caps the whole machine at `kern.sysv.shmmni` (32 by default). Under a
    // parallel sweep this suite's clusters compete for those 32 slots with
    // every segment leaked by a previously killed postmaster, so the same
    // commit starts or fails depending on machine history rather than code
    // (#9240). Name the real limit instead of surfacing a bare ENOSPC.
    if (/shared memory segment|No space left on device/i.test(output)) {
      throw new Error(
        `${binary} could not obtain a System V shared-memory segment.\n` +
          "This is host capacity, not a defect in the code under test: macOS " +
          "caps segments machine-wide at kern.sysv.shmmni (see `sysctl " +
          "kern.sysv.shmmni`), and leaked segments from killed postmasters " +
          "occupy slots permanently. Inspect with `ipcs -mba` (NATTCH 0 rows " +
          "are orphans) and reclaim with `ipcrm -m <id>`.\n" +
          output,
      );
    }
    throw new Error(`${binary} failed (exit ${String(result.status)}):\n${output}`);
  }
};

const runOutput = (cmd: ReadonlyArray<string>): string => {
  const [binary, ...args] = cmd;
  if (binary === undefined) throw new Error("empty command");
  const result = spawnSync(binary, args, { encoding: "utf8" });
  if (result.error !== undefined) throw result.error;
  if (result.status !== 0) {
    throw new Error(
      `${binary} failed (exit ${String(result.status)}):\n${result.stdout}\n${result.stderr}`,
    );
  }
  return result.stdout;
};

const commandSucceeded = (cmd: ReadonlyArray<string>): boolean => {
  const [binary, ...args] = cmd;
  if (binary === undefined) return false;
  const result = spawnSync(binary, args, { encoding: "utf8" });
  return result.error === undefined && result.status === 0;
};

/**
 * The postmaster records its System V shared-memory key and id on line 7 of
 * `postmaster.pid`. Reading it before shutdown lets `stop()` verify the
 * segment is actually gone rather than assume it, so a cluster this suite
 * started can never permanently consume one of the machine's few slots.
 */
const readShmemId = (dataDir: string): string | null => {
  try {
    const line = readFileSync(path.join(dataDir, "postmaster.pid"), "utf8").split("\n")[6];
    const id = line?.trim().split(/\s+/)[1];
    return id !== undefined && /^\d+$/.test(id) ? id : null;
  } catch {
    return null;
  }
};

/** Best-effort reclaim of a segment the postmaster failed to remove. */
const reclaimShmemSegment = (id: string | null): void => {
  if (id === null) return;
  // Exits non-zero when the segment is already gone, which is the good case.
  spawnSync("ipcrm", ["-m", id], { encoding: "utf8" });
};

interface SharedClient {
  readonly database: string;
  readonly pid: number;
  readonly role: string;
}

interface SharedPostgresState {
  readonly dataDir: string;
  readonly host: string;
  readonly port: number;
  readonly user: string;
  readonly clients: Readonly<Record<string, SharedClient>>;
}

const SHARED_ROOT = path.join(tmpdir(), "openagents-local-postgres-v1");
const SHARED_LOCK = `${SHARED_ROOT}.lock`;
const SHARED_LOCK_OWNER = path.join(SHARED_LOCK, "owner.json");
const SHARED_STATE = path.join(SHARED_ROOT, "state.json");
const SHARED_DATA_DIR = path.join(SHARED_ROOT, "data");
const SHARED_LOCK_TIMEOUT_MS = 120_000;
const SHARED_LOCK_STALE_MS = 120_000;
let localClientCounter = 0;
let localLockTail: Promise<void> = Promise.resolve();

const sleep = (ms: number): Promise<void> => new Promise((resolve) => setTimeout(resolve, ms));

const processIsLive = (pid: number): boolean => {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    return !(
      typeof error === "object" &&
      error !== null &&
      "code" in error &&
      error.code === "ESRCH"
    );
  }
};

const readSharedState = async (): Promise<SharedPostgresState | null> => {
  try {
    return JSON.parse(await readFile(SHARED_STATE, "utf8")) as SharedPostgresState;
  } catch {
    return null;
  }
};

const writeSharedState = async (state: SharedPostgresState): Promise<void> => {
  const temporary = `${SHARED_STATE}.${process.pid}.${Date.now()}.tmp`;
  await writeFile(temporary, `${JSON.stringify(state)}\n`, { mode: 0o600 });
  await rename(temporary, SHARED_STATE);
};

const lockIsStale = async (): Promise<boolean> => {
  try {
    const raw = await readFile(SHARED_LOCK_OWNER, "utf8");
    const owner = JSON.parse(raw) as { readonly pid?: unknown; readonly startedAt?: unknown };
    const pid = typeof owner.pid === "number" ? owner.pid : null;
    const startedAt = typeof owner.startedAt === "number" ? owner.startedAt : 0;
    return (pid === null || !processIsLive(pid)) && Date.now() - startedAt > SHARED_LOCK_STALE_MS;
  } catch {
    // A competing process creates the directory before it can write its
    // owner record. Treat that short handoff as live: deleting the directory
    // here would let two callers enter the critical section at once.
    try {
      const lockMetadata = await stat(SHARED_LOCK);
      return Date.now() - lockMetadata.mtimeMs > SHARED_LOCK_STALE_MS;
    } catch {
      // The lock disappeared while we inspected it. The caller will retry
      // mkdir and either acquire the replacement lock or observe its owner.
      return false;
    }
  }
};

const withFileLock = async <A>(operation: () => Promise<A>): Promise<A> => {
  await mkdir(SHARED_ROOT, { recursive: true, mode: 0o700 });
  const deadline = Date.now() + SHARED_LOCK_TIMEOUT_MS;
  for (;;) {
    try {
      await mkdir(SHARED_LOCK, { mode: 0o700 });
      await writeFile(
        SHARED_LOCK_OWNER,
        JSON.stringify({ pid: process.pid, startedAt: Date.now() }),
        {
          mode: 0o600,
        },
      );
      break;
    } catch (error) {
      if (
        !(typeof error === "object" && error !== null && "code" in error && error.code === "EEXIST")
      ) {
        throw error;
      }
      if (await lockIsStale()) {
        await rm(SHARED_LOCK, { recursive: true, force: true });
        continue;
      }
      if (Date.now() >= deadline) {
        throw new Error("timed out waiting for the shared local Postgres test lock");
      }
      await sleep(20);
    }
  }

  try {
    return await operation();
  } finally {
    await rm(SHARED_LOCK, { recursive: true, force: true });
  }
};

/**
 * `mkdir` coordinates separate Vitest worker processes. This process-local
 * tail also serializes callers in the same worker before they read or update
 * the shared state file.
 */
const withSharedLock = async <A>(operation: () => Promise<A>): Promise<A> => {
  let releaseLocalLock: (() => void) | undefined;
  const previous = localLockTail;
  localLockTail = new Promise<void>((resolve) => {
    releaseLocalLock = resolve;
  });
  await previous;
  try {
    return await withFileLock(operation);
  } finally {
    releaseLocalLock?.();
  }
};

const quoteIdentifier = (value: string): string => `"${value.replaceAll('"', '""')}"`;
const quoteLiteral = (value: string): string => `'${value.replaceAll("'", "''")}'`;

const postgresCommand = (
  binDir: string,
  state: Pick<SharedPostgresState, "host" | "port">,
  sql: string,
): ReadonlyArray<string> => [
  path.join(binDir, "psql"),
  "--no-psqlrc",
  "--quiet",
  "--tuples-only",
  "--no-align",
  "--host",
  state.host,
  "--port",
  String(state.port),
  "--username",
  "postgres",
  "--dbname",
  "postgres",
  "--command",
  sql,
];

const serverAcceptsConnections = async (
  binDir: string,
  state: SharedPostgresState,
): Promise<boolean> => {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if (commandSucceeded(postgresCommand(binDir, state, "SELECT 1"))) return true;
    await sleep(25);
  }
  return false;
};

const cleanClient = (binDir: string, state: SharedPostgresState, client: SharedClient): void => {
  run(
    postgresCommand(
      binDir,
      state,
      `SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE usename = ${quoteLiteral(client.role)} AND pid <> pg_backend_pid()`,
    ),
  );
  const databases = runOutput(
    postgresCommand(
      binDir,
      state,
      `SELECT datname FROM pg_database WHERE datdba = (SELECT oid FROM pg_roles WHERE rolname = ${quoteLiteral(client.role)})`,
    ),
  )
    .split("\n")
    .map((database) => database.trim())
    .filter((database) => database !== "");
  for (const database of databases) {
    run(
      postgresCommand(
        binDir,
        state,
        `DROP DATABASE IF EXISTS ${quoteIdentifier(database)} WITH (FORCE)`,
      ),
    );
  }
  run(postgresCommand(binDir, state, `DROP ROLE IF EXISTS ${quoteIdentifier(client.role)}`));
};

const removeStaleClients = (binDir: string, state: SharedPostgresState): SharedPostgresState => {
  const clients = { ...state.clients };
  for (const [clientId, client] of Object.entries(clients)) {
    if (processIsLive(client.pid)) continue;
    cleanClient(binDir, state, client);
    delete clients[clientId];
  }
  return { ...state, clients };
};

const startSharedServer = async (binDir: string): Promise<SharedPostgresState> => {
  const host = "127.0.0.1";
  const user = "postgres";
  const port = await freePort();
  const logFile = path.join(SHARED_ROOT, "postgres.log");
  await mkdir(SHARED_ROOT, { recursive: true, mode: 0o700 });
  await rm(SHARED_DATA_DIR, { recursive: true, force: true });
  await rm(SHARED_STATE, { force: true });
  run([
    path.join(binDir, "initdb"),
    "--pgdata",
    SHARED_DATA_DIR,
    "--username",
    user,
    "--auth",
    "trust",
    "--no-sync",
    "--encoding",
    "UTF8",
  ]);
  run([
    path.join(binDir, "pg_ctl"),
    "--pgdata",
    SHARED_DATA_DIR,
    "--log",
    logFile,
    "--wait",
    "--timeout",
    "60",
    "--options",
    `-p ${port} -c listen_addresses=${host} -c unix_socket_directories='${SHARED_ROOT}' -c fsync=off -c max_connections=512`,
    "start",
  ]);
  return { dataDir: SHARED_DATA_DIR, host, port, user, clients: {} };
};

const stopSharedServer = async (binDir: string, state: SharedPostgresState): Promise<void> => {
  const shmemId = readShmemId(state.dataDir);
  try {
    run([
      path.join(binDir, "pg_ctl"),
      "--pgdata",
      state.dataDir,
      "--mode",
      "immediate",
      "--wait",
      "stop",
    ]);
  } finally {
    reclaimShmemSegment(shmemId);
    await rm(state.dataDir, { recursive: true, force: true });
    await rm(SHARED_STATE, { force: true });
  }
};

export const startLocalPostgres = async (): Promise<LocalPostgres> => {
  const binDir = findPgBinDir();
  if (binDir === null) {
    throw new Error(
      "no local Postgres binaries (initdb/pg_ctl) found — " +
        "install postgresql@16 (brew install postgresql@16) or gate the " +
        "test with hasLocalPostgres()",
    );
  }
  const clientId = `${process.pid}-${++localClientCounter}-${Math.random().toString(36).slice(2, 10)}`;
  const identifierSuffix = `${process.pid}_${localClientCounter}_${Math.random().toString(36).slice(2, 8)}`;
  const role = `oa_test_${identifierSuffix}`;
  const database = `oa_suite_${identifierSuffix}`;

  const state = await withSharedLock(async () => {
    let current = await readSharedState();
    if (current === null || !(await serverAcceptsConnections(binDir, current))) {
      if (current !== null) {
        reclaimShmemSegment(readShmemId(current.dataDir));
      }
      current = await startSharedServer(binDir);
    } else {
      current = removeStaleClients(binDir, current);
    }

    try {
      run(postgresCommand(binDir, current, `CREATE ROLE ${quoteIdentifier(role)} LOGIN CREATEDB`));
      run(
        postgresCommand(
          binDir,
          current,
          `CREATE DATABASE ${quoteIdentifier(database)} OWNER ${quoteIdentifier(role)}`,
        ),
      );
    } catch (error) {
      run(
        postgresCommand(
          binDir,
          current,
          `DROP DATABASE IF EXISTS ${quoteIdentifier(database)} WITH (FORCE)`,
        ),
      );
      run(postgresCommand(binDir, current, `DROP ROLE IF EXISTS ${quoteIdentifier(role)}`));
      if (Object.keys(current.clients).length === 0) await stopSharedServer(binDir, current);
      throw error;
    }

    const next: SharedPostgresState = {
      ...current,
      clients: {
        ...current.clients,
        [clientId]: { database, pid: process.pid, role },
      },
    };
    await writeSharedState(next);
    return next;
  });

  let stopped = false;
  const stop = async (): Promise<void> => {
    if (stopped) return;
    stopped = true;
    await withSharedLock(async () => {
      const current = await readSharedState();
      if (current === null || !(await serverAcceptsConnections(binDir, current))) return;
      const client = current.clients[clientId];
      const withoutStaleClients = removeStaleClients(binDir, current);
      if (client !== undefined) cleanClient(binDir, withoutStaleClients, client);
      const clients = { ...withoutStaleClients.clients };
      delete clients[clientId];
      const next = { ...withoutStaleClients, clients };
      if (Object.keys(next.clients).length === 0) {
        await stopSharedServer(binDir, next);
      } else {
        await writeSharedState(next);
      }
    });
  };

  const urlFor = (targetDatabase: string): string =>
    `postgres://${role}@${state.host}:${state.port}/${targetDatabase}`;

  return {
    url: urlFor(database),
    host: state.host,
    port: state.port,
    user: role,
    dataDir: state.dataDir,
    urlFor,
    stop,
  };
};
