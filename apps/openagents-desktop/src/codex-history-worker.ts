/** Runs all local rollout filesystem work outside Electron's main process. */
import { parentPort, workerData } from "node:worker_threads"

import { findRecentCodexThread, readRecentCodexHistory } from "./codex-history.ts"

type Request =
  | Readonly<{ kind: "list"; sessionsRoot: string; limit?: number }>
  | Readonly<{ kind: "detail"; sessionsRoot: string; id: string; messageLimit?: number }>

const request = workerData as Request
const result = request.kind === "list"
  ? readRecentCodexHistory({ sessionsRoot: request.sessionsRoot, includeMessages: false, limit: request.limit })
  : findRecentCodexThread({ sessionsRoot: request.sessionsRoot, id: request.id, messageLimit: request.messageLimit })

parentPort?.postMessage({ ok: true, result })
