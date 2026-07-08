// Khala Sync capture daemon on Cloud Run (#8554).
//
// The live-tail delivery pipe: this always-on service tails the
// `khala_sync_changelog` (LISTEN `khala_sync_changelog_append` wake + short
// poll fallback) over a DIRECT Cloud SQL SESSION connection and pushes
// ordered whole-version-group batches to the LiveHub `/append` route, which
// fans DeltaFrames out to subscribed WebSocket clients. Checkpoints advance
// only on a hub 2xx; delivery is at-least-once (the hub dedupes by version).
//
// ## Why a long-lived Cloud Run service (not launchd on a Mac)
//
// The capture daemon used to run under launchd on the owner's Mac over a
// direct DB IP. CFG-14 closed the instance's public ingress, freezing that
// path. This service replaces it: it connects through the Cloud SQL Auth
// Connector unix socket (`--add-cloudsql-instances`, mounted under
// `/cloudsql/<instance>`) and stays resident.
//
// ## Deploy shape (see scripts/deploy-cloudrun.sh)
//
//   min=max=1 instance   — a SINGLETON daemon. LISTEN/NOTIFY needs one
//                          persistent session; a second instance would just
//                          double-push (the hub dedupes by version) and churn
//                          extra LISTEN connections, so we pin exactly one.
//   --no-cpu-throttling   — the daemon's loop, LISTEN connection, and poll
//                          timer must run BETWEEN HTTP requests; Cloud Run
//                          would otherwise freeze CPU when no request is in
//                          flight and stall live delivery.
//   timeout 3600          — irrelevant to the daemon (it is not request-bound)
//                          but harmless.
//
// ## Env
//
//   PGHOST/PGUSER/PGPASSWORD/PGDATABASE — Cloud SQL Auth Connector socket
//     session connection (khala_capture; SELECT changelog/checkpoints +
//     UPDATE checkpoints). captureConfigFromEnv selects socket mode when
//     PGHOST is an absolute connector path.
//   KHALA_SYNC_HUB_APPEND_URL           — LiveHub `/append` URL.
//   KHALA_SYNC_HUB_TOKEN                — LiveHub shared service bearer.
//   PORT                                — Cloud Run health port (default 8080).
//
// Secrets are read from the environment (Secret Manager mounts) and NEVER
// logged.

import {
  captureConfigFromEnv,
  startCaptureDaemon,
} from "@openagentsinc/khala-sync-server/capture"

const PORT = Number(process.env["PORT"] ?? "8080")

const config = {
  ...captureConfigFromEnv(),
  log: (line: string) => console.log(line),
}

const startedAt = Date.now()
console.log(
  `khala-capture: starting daemon (poll fallback ${config.pollIntervalMs ?? 5000}ms, ` +
    `mode ${config.socket !== undefined ? "cloudsql-socket" : "direct-url"})`,
)

const daemon = startCaptureDaemon(config)

let listenerUp = false
void daemon.listenerReady.then(() => {
  listenerUp = true
})

// Minimal health server. Cloud Run needs the container to answer on $PORT for
// readiness; the capture work happens in the resident daemon, not per request.
const server = Bun.serve({
  port: PORT,
  fetch(request) {
    const url = new URL(request.url)
    if (url.pathname === "/health" || url.pathname === "/") {
      return Response.json({
        ok: true,
        service: "khala-capture",
        listener: listenerUp ? "listening" : "connecting",
        uptimeSeconds: Math.round((Date.now() - startedAt) / 1000),
      })
    }
    return new Response("not found", { status: 404 })
  },
})

console.log(`khala-capture: health server on :${server.port}`)

let stopping = false
const shutdown = (signal: string): void => {
  if (stopping) return
  stopping = true
  console.log(`khala-capture: stopping (${signal})`)
  server.stop(true)
  void daemon.stop().then(() => {
    console.log("khala-capture: daemon stopped")
    process.exit(0)
  })
}

process.on("SIGINT", () => shutdown("SIGINT"))
process.on("SIGTERM", () => shutdown("SIGTERM"))

// If the daemon loop ever exits on its own (it should not without stop()),
// surface it and let Cloud Run restart the instance.
void daemon.done.then(() => {
  if (!stopping) {
    console.error("khala-capture: daemon loop exited unexpectedly")
    process.exit(1)
  }
})
