import { Database } from "bun:sqlite"
import { chmod, mkdir } from "node:fs/promises"
import { join } from "node:path"

import {
  createBootstrapSummary,
  parseBootstrapArgs,
  type BootstrapSummary,
} from "../bootstrap.js"
import { PylonFleetRunManager } from "./fleet-run-manager.js"
import {
  createPylonOrchestrationStore,
  type PylonOrchestrationStore,
} from "./store.js"

export const PYLON_FLEET_RUN_DATABASE_FILENAME = "orchestration.sqlite"

type PylonFleetRunBootstrap = Pick<BootstrapSummary, "paths">

export type OpenPylonFleetRunRuntimeInput = {
  /** Reuse a daemon/CLI bootstrap summary when one is already available. */
  readonly bootstrap?: PylonFleetRunBootstrap | undefined
  /** Canonical bootstrap input when constructing outside an existing daemon. */
  readonly env?: NodeJS.ProcessEnv | undefined
  readonly now?: (() => Date) | undefined
}

/**
 * Local Pylon-owned FleetRun composition.
 *
 * `databasePath` is deliberately a local diagnostic, not a public projection:
 * never serialize this runtime object into Sarah, Khala Sync, issue comments,
 * or receipts. Public run snapshots contain bounded refs/state only.
 */
export type PylonFleetRunRuntime = {
  readonly databasePath: string
  readonly manager: PylonFleetRunManager
  readonly store: PylonOrchestrationStore
  readonly close: () => Promise<void>
}

export const pylonFleetRunDatabasePath = (
  bootstrap: PylonFleetRunBootstrap,
): string => join(bootstrap.paths.home, PYLON_FLEET_RUN_DATABASE_FILENAME)

/**
 * Open the durable FleetRun authority at the canonical Pylon-home boundary.
 *
 * There is intentionally no `:memory:` option and no `CODEX_HOME` input. Tests
 * that need an ephemeral runtime pass a temporary explicit `PYLON_HOME`; the
 * same construction path and on-disk schema are exercised as production.
 */
export async function openPylonFleetRunRuntime(
  input: OpenPylonFleetRunRuntimeInput = {},
): Promise<PylonFleetRunRuntime> {
  const bootstrap = input.bootstrap ?? createBootstrapSummary(
    parseBootstrapArgs(["--json"]),
    input.env ?? process.env,
  )
  await mkdir(bootstrap.paths.home, { recursive: true, mode: 0o700 })

  const databasePath = pylonFleetRunDatabasePath(bootstrap)
  const database = new Database(databasePath, { create: true })
  try {
    // The store is process-shared by the standing Pylon. A bounded wait avoids
    // turning a short reader/writer overlap into a false startup failure.
    database.exec("PRAGMA busy_timeout = 5000")
    await chmod(databasePath, 0o600)
    const store = createPylonOrchestrationStore(database)
    const manager = new PylonFleetRunManager({
      store,
      ...(input.now === undefined ? {} : { now: input.now }),
    })
    let closed = false
    const close = async (): Promise<void> => {
      if (closed) return
      closed = true
      // Deterministic for a quiescent manager. Standing-node wiring must first
      // drain/reconcile in-flight dispatch work; manager.close stops loop
      // handles but does not overclaim an await-idle guarantee for detached
      // runner bookkeeping.
      try {
        await manager.close()
      } finally {
        database.close()
      }
    }
    return { close, databasePath, manager, store }
  } catch (error) {
    database.close()
    throw error
  }
}
