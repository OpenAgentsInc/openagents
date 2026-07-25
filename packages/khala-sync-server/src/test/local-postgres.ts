import { spawnSync } from "node:child_process"
import { accessSync, constants, readFileSync } from "node:fs"
import { mkdtemp, rm } from "node:fs/promises"
import * as net from "node:net"
import { tmpdir } from "node:os"
import * as path from "node:path"

/**
 * Throwaway local Postgres for integration tests (KS-0.3, reused by the
 * KS-2.x substrate lanes).
 *
 * `startLocalPostgres()` initdb's a fresh data directory under the OS temp
 * dir, starts a server on a random free 127.0.0.1 port with `pg_ctl`, and
 * returns a direct connection URL plus a `stop()` that shuts the server
 * down and deletes the data directory. No system-wide Postgres state is
 * touched.
 *
 * Requires local Postgres server binaries (initdb/pg_ctl); on this repo's
 * dev Macs that is `brew install postgresql@16`. Tests should gate on
 * `hasLocalPostgres()` so machines without Postgres skip instead of fail.
 */

export interface LocalPostgres {
  /** Direct connection URL, e.g. postgres://postgres@127.0.0.1:54xxx/postgres */
  readonly url: string
  readonly host: string
  readonly port: number
  readonly user: string
  readonly dataDir: string
  /** Connection URL for a specific database on this server. */
  readonly urlFor: (database: string) => string
  /** Stop the server and delete the data directory. Safe to call once. */
  readonly stop: () => Promise<void>
}

const PG_BIN_DIR_CANDIDATES = [
  "/opt/homebrew/opt/postgresql@16/bin",
  "/opt/homebrew/bin",
  "/usr/local/opt/postgresql@16/bin",
  "/usr/local/bin",
  "/usr/lib/postgresql/16/bin",
]

const REQUIRED_BINARIES = ["initdb", "pg_ctl"] as const

const isExecutable = (file: string): boolean => {
  try {
    accessSync(file, constants.X_OK)
    return true
  } catch {
    return false
  }
}

/**
 * Resolve `binary` from PATH (runtime-agnostic `which`: this helper runs
 * under BOTH `pnpm exec vp test` in this package and vitest/Node in the
 * `openagents.com` Worker's stitch-seam suite, so no `Bun.*` APIs here).
 */
const whichOnPath = (binary: string): string | null => {
  for (const dir of (process.env.PATH ?? "").split(path.delimiter)) {
    if (dir === "") continue
    const candidate = path.join(dir, binary)
    if (isExecutable(candidate)) return candidate
  }
  return null
}

/** Directory containing initdb + pg_ctl, or null if none is available. */
export const findPgBinDir = (): string | null => {
  for (const dir of PG_BIN_DIR_CANDIDATES) {
    if (REQUIRED_BINARIES.every((bin) => isExecutable(path.join(dir, bin)))) {
      return dir
    }
  }
  // Fall back to PATH.
  if (REQUIRED_BINARIES.every((bin) => whichOnPath(bin) !== null)) {
    const resolved = whichOnPath("initdb")
    return resolved === null ? null : path.dirname(resolved)
  }
  return null
}

export const hasLocalPostgres = (): boolean => findPgBinDir() !== null

const freePort = (): Promise<number> =>
  new Promise((resolve, reject) => {
    const server = net.createServer()
    server.once("error", reject)
    server.listen(0, "127.0.0.1", () => {
      const address = server.address() as net.AddressInfo
      server.close(() => resolve(address.port))
    })
  })

const run = (cmd: ReadonlyArray<string>): void => {
  const [binary, ...args] = cmd
  if (binary === undefined) throw new Error("empty command")
  const result = spawnSync(binary, args, { encoding: "utf8" })
  if (result.error !== undefined) throw result.error
  if (result.status !== 0) {
    const output = `${result.stdout}\n${result.stderr}`
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
      )
    }
    throw new Error(`${binary} failed (exit ${String(result.status)}):\n${output}`)
  }
}

/**
 * The postmaster records its System V shared-memory key and id on line 7 of
 * `postmaster.pid`. Reading it before shutdown lets `stop()` verify the
 * segment is actually gone rather than assume it, so a cluster this suite
 * started can never permanently consume one of the machine's few slots.
 */
const readShmemId = (dataDir: string): string | null => {
  try {
    const line = readFileSync(path.join(dataDir, "postmaster.pid"), "utf8").split("\n")[6]
    const id = line?.trim().split(/\s+/)[1]
    return id !== undefined && /^\d+$/.test(id) ? id : null
  } catch {
    return null
  }
}

/** Best-effort reclaim of a segment the postmaster failed to remove. */
const reclaimShmemSegment = (id: string | null): void => {
  if (id === null) return
  // Exits non-zero when the segment is already gone, which is the good case.
  spawnSync("ipcrm", ["-m", id], { encoding: "utf8" })
}

export const startLocalPostgres = async (): Promise<LocalPostgres> => {
  const binDir = findPgBinDir()
  if (binDir === null) {
    throw new Error(
      "no local Postgres binaries (initdb/pg_ctl) found — " +
        "install postgresql@16 (brew install postgresql@16) or gate the " +
        "test with hasLocalPostgres()",
    )
  }
  const user = "postgres"
  const host = "127.0.0.1"
  const dataDir = await mkdtemp(path.join(tmpdir(), "khala-sync-pg-"))
  const logFile = path.join(dataDir, "postgres.log")
  const port = await freePort()

  try {
    run([
      path.join(binDir, "initdb"),
      "--pgdata",
      dataDir,
      "--username",
      user,
      "--auth",
      "trust",
      "--no-sync",
      "--encoding",
      "UTF8",
    ])
    run([
      path.join(binDir, "pg_ctl"),
      "--pgdata",
      dataDir,
      "--log",
      logFile,
      "--wait",
      "--timeout",
      "60",
      "--options",
      `-p ${port} -c listen_addresses=${host} -c unix_socket_directories='${dataDir}' -c fsync=off`,
      "start",
    ])
  } catch (error) {
    reclaimShmemSegment(readShmemId(dataDir))
    await rm(dataDir, { recursive: true, force: true })
    throw error
  }

  let stopped = false
  const stop = async (): Promise<void> => {
    if (stopped) return
    stopped = true
    const shmemId = readShmemId(dataDir)
    try {
      run([
        path.join(binDir, "pg_ctl"),
        "--pgdata",
        dataDir,
        "--mode",
        "immediate",
        "--wait",
        "stop",
      ])
    } finally {
      reclaimShmemSegment(shmemId)
      await rm(dataDir, { recursive: true, force: true })
    }
  }

  const urlFor = (database: string): string =>
    `postgres://${user}@${host}:${port}/${database}`

  return { url: urlFor("postgres"), host, port, user, dataDir, urlFor, stop }
}
