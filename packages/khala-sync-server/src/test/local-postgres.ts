import { accessSync, constants } from "node:fs"
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

/** Directory containing initdb + pg_ctl, or null if none is available. */
export const findPgBinDir = (): string | null => {
  for (const dir of PG_BIN_DIR_CANDIDATES) {
    if (REQUIRED_BINARIES.every((bin) => isExecutable(path.join(dir, bin)))) {
      return dir
    }
  }
  // Fall back to PATH.
  if (REQUIRED_BINARIES.every((bin) => Bun.which(bin) !== null)) {
    const resolved = Bun.which("initdb")
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
  const result = Bun.spawnSync([...cmd], { stdout: "pipe", stderr: "pipe" })
  if (result.exitCode !== 0) {
    throw new Error(
      `${cmd[0]} failed (exit ${result.exitCode}):\n` +
        `${result.stdout.toString()}\n${result.stderr.toString()}`,
    )
  }
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
    await rm(dataDir, { recursive: true, force: true })
    throw error
  }

  let stopped = false
  const stop = async (): Promise<void> => {
    if (stopped) return
    stopped = true
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
      await rm(dataDir, { recursive: true, force: true })
    }
  }

  const urlFor = (database: string): string =>
    `postgres://${user}@${host}:${port}/${database}`

  return { url: urlFor("postgres"), host, port, user, dataDir, urlFor, stop }
}
