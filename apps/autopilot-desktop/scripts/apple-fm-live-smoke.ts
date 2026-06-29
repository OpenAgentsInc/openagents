import { mkdtempSync } from "node:fs"
import { mkdir, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { Effect } from "effect"
import {
  fetchAppleFmReadiness,
  fetchNodeState,
  startAppleFmSession,
} from "../src/bun/pylon-control"
import { createBootstrapSummary, parseBootstrapArgs } from "../../pylon/src/bootstrap"
import { collectPylonAppleFmStatus } from "../../pylon/src/node/apple-fm-status"
import { startControlServer } from "../../pylon/src/node/control-server"
import { createControlSessionActions } from "../../pylon/src/node/control-sessions"
import { makePylonNodeRuntime } from "../../pylon/src/node/runtime"

type SmokeSummary = {
  readiness?: unknown
  row?: unknown
  saw?: Record<string, boolean>
  retained?: unknown
  redaction?: Record<string, boolean>
  disabled?: unknown
}

const bridgeBaseUrl =
  Bun.env.PROBE_APPLE_FM_BASE_URL ??
  Bun.env.OPENAGENTS_APPLE_FM_BASE_URL ??
  "http://127.0.0.1:11435"

const root = mkdtempSync(join(tmpdir(), "openagents-apple-fm-live-smoke-"))
const disabledBridge = Bun.serve({
  port: 0,
  fetch: request => {
    const url = new URL(request.url)
    if (url.pathname === "/health") {
      return Response.json({
        message: "Apple Intelligence is disabled in the smoke fixture.",
        model: "apple-foundation-model",
        platform: "macOS-arm64-test",
        ready: false,
        unavailableReason: "apple_intelligence_disabled",
        version: "fake-disabled-bridge",
      })
    }

    return Response.json({ error: "not found" }, { status: 404 })
  },
})

try {
  const pylonHome = join(root, "pylon-home")
  const proofsDir = join(root, "proofs")
  const worktree = join(root, "worktree")
  await mkdir(pylonHome, { recursive: true })
  await mkdir(worktree, { recursive: true })
  await writeFile(
    join(worktree, "README.md"),
    "# Live Smoke Fixture\n\nThis content must stay out of public evidence.\n",
    "utf8",
  )

  const summary = createBootstrapSummary(
    parseBootstrapArgs(["--json", "--pylon-ref", "pylon.live.apple-fm-smoke"]),
    { PYLON_HOME: pylonHome },
  )
  const token = "live-smoke-token-public-safe"

  const result = await Effect.runPromise(
    Effect.scoped(
      Effect.gen(function* () {
        const runtime = yield* makePylonNodeRuntime
        const env = { PROBE_APPLE_FM_BASE_URL: bridgeBaseUrl }
        const sessions = createControlSessionActions({ env, proofsDir, summary })
        const server = yield* startControlServer(runtime, {
          token,
          port: 0,
          actions: {
            walletSend: async () => ({ dispatched: false }),
            walletReceive: async () => ({ unavailable: true }),
            walletAdmitPayoutTarget: async () => ({ admitted: false }),
            appleFmStatus: () =>
              collectPylonAppleFmStatus({
                env,
                fetch,
                now: new Date("2026-06-15T00:00:00.000Z"),
                summary,
              }),
            sessions,
          },
        })

        const readiness = yield* Effect.promise(() =>
          fetchAppleFmReadiness({ baseUrl: server.url, token }),
        )
        const started = yield* Effect.promise(() =>
          startAppleFmSession({
            baseUrl: server.url,
            prompt:
              "Use read_file on README.md, then reply with the exact marker OPENAGENTS_LOCAL_TOOL_OK and a short local-only note.",
            timeoutSeconds: 300,
            token,
            worktreePath: worktree,
          }),
        )
        if (!started.ok) {
          return { readiness, row: null, retained: null }
        }

        let state = yield* Effect.promise(() =>
          fetchNodeState({ baseUrl: server.url, token }),
        )
        for (
          let attempt = 0;
          attempt < 120 &&
          state.sessions.find(row => row.sessionRef === started.sessionRef)?.state !==
            "completed";
          attempt += 1
        ) {
          yield* Effect.sleep("250 millis")
          state = yield* Effect.promise(() =>
            fetchNodeState({ baseUrl: server.url, token }),
          )
        }

        const rows = yield* Effect.promise(() => sessions.list())
        const row = rows.find(entry => entry.sessionRef === started.sessionRef) ?? null
        const retained = yield* Effect.promise(() => sessions.artifact(started.sessionRef))
        const sessionEvents = yield* Effect.promise(() => sessions.events(started.sessionRef))
        const eventText = JSON.stringify(sessionEvents.recentEvents)
        const serialized = JSON.stringify({ eventText, retained })
        const artifact = retained.artifact as { executor?: Record<string, unknown>; schema?: string; adapter?: string } | null
        const executor = artifact?.executor
        const energyEstimate = executor?.energyEstimate as Record<string, unknown> | undefined

        const disabledEnv = { PROBE_APPLE_FM_BASE_URL: String(disabledBridge.url) }
        const disabledSessions = createControlSessionActions({
          env: disabledEnv,
          proofsDir,
          summary,
        })
        const disabledServer = yield* startControlServer(runtime, {
          token,
          port: 0,
          actions: {
            walletSend: async () => ({ dispatched: false }),
            walletReceive: async () => ({ unavailable: true }),
            walletAdmitPayoutTarget: async () => ({ admitted: false }),
            appleFmStatus: () =>
              collectPylonAppleFmStatus({
                env: disabledEnv,
                fetch,
                now: new Date("2026-06-15T00:00:00.000Z"),
                summary,
              }),
            sessions: disabledSessions,
          },
        })
        const disabled = yield* Effect.promise(() =>
          fetchAppleFmReadiness({ baseUrl: disabledServer.url, token }),
        )

        return {
          disabled: {
            available: disabled.available,
            blockerRefs: disabled.blockerRefs,
            ok: disabled.ok,
            status: disabled.status,
            unavailableReason: disabled.unavailableReason,
          },
          readiness: {
            available: readiness.available,
            backendKind: readiness.backendKind,
            model: readiness.model,
            ok: readiness.ok,
            platform: readiness.platform,
            status: readiness.status,
            version: readiness.version,
          },
          redaction: {
            bearerLeaked: serialized.includes("Bearer "),
            callbackTokenLeaked:
              serialized.includes("session_token") || serialized.includes(token),
            callbackUrlLeaked: serialized.includes("tool-callback"),
            fixtureBodyLeaked: serialized.includes("This content must stay out"),
            promptLeaked: serialized.includes("Use read_file on README.md"),
            tempPathLeaked: serialized.includes(root),
          },
          retained: {
            adapter: artifact?.adapter,
            commandCount: executor?.commandCount,
            editedFileCount: executor?.editedFileCount,
            energyEstimate:
              energyEstimate === undefined
                ? undefined
                : {
                    assumptionRefs: energyEstimate.assumptionRefs,
                    caveatRefs: energyEstimate.caveatRefs,
                    energyKwh: energyEstimate.energyKwh,
                    evidenceState: energyEstimate.evidenceState,
                    methodRef: energyEstimate.methodRef,
                    modeledPowerKw: energyEstimate.modeledPowerKw,
                    wallClockHours: energyEstimate.wallClockHours,
                    wallClockSeconds: energyEstimate.wallClockSeconds,
                  },
            executionMode: executor?.executionMode,
            executionPathRef: executor?.executionPathRef,
            externalSessionRefPrefix:
              typeof executor?.externalSessionRef === "string"
                ? executor.externalSessionRef.slice(0, 35)
                : executor?.externalSessionRef,
            kind: retained.kind,
            networkAccessEnabled: executor?.networkAccessEnabled,
            outcome: executor?.outcome,
            resourceUsageReceiptRef: row?.resourceUsageReceiptRef ?? null,
            sandboxMode: executor?.sandboxMode,
            schema: artifact?.schema,
            totalTokens: executor?.totalTokens,
          },
          row:
            row === null
              ? null
              : {
                  adapter: row.adapter,
                  cloudRunner: row.cloudRunner,
                  lane: row.lane,
                  resourceUsageReceiptRef: row.resourceUsageReceiptRef,
                  state: row.state,
                },
          saw: {
            backendReady: eventText.includes("Apple FM local backend ready"),
            localMode: eventText.includes(
              "control session mode: local_bounded; adapter: apple_fm; sandbox: read-only; network: disabled",
            ),
            toolSuccess: eventText.includes("Apple FM tool read_file: success"),
          },
        } satisfies SmokeSummary
      }),
    ),
  )

  assertSmokeSummary(result)
  console.log(JSON.stringify(result, null, 2))
} finally {
  disabledBridge.stop(true)
  await rm(root, { recursive: true, force: true })
}

function assertSmokeSummary(summary: SmokeSummary) {
  const serialized = JSON.stringify(summary)
  const mustContain = [
    '"ok":true',
    '"available":true',
    '"status":"ready"',
    '"adapter":"apple_fm"',
    '"lane":"local"',
    '"state":"completed"',
    '"cloudRunner":null',
    '"resourceUsageReceiptRef":null',
    '"backendReady":true',
    '"toolSuccess":true',
    '"localMode":true',
    '"executionPathRef":"control_session.apple_fm_local"',
    '"evidenceState":"modeled"',
    '"methodRef":"method.apple_fm.power.modeled_default_kw_wall_clock"',
    '"modeledPowerKw":0.02',
    '"energyKwh":',
    '"caveat.apple_fm.power.modeled_not_measured"',
    '"caveat.apple_fm.power.not_ao_kwh_without_accepted_outcome"',
    '"executionMode":"local_bounded"',
    '"sandboxMode":"read-only"',
    '"networkAccessEnabled":false',
    '"outcome":"completed"',
    '"externalSessionRefPrefix":"session.pylon.apple_fm_bridge.',
    '"promptLeaked":false',
    '"fixtureBodyLeaked":false',
    '"callbackTokenLeaked":false',
    '"callbackUrlLeaked":false',
    '"bearerLeaked":false',
    '"tempPathLeaked":false',
    '"unavailableReason":"apple_intelligence_disabled"',
  ]
  const missing = mustContain.filter(fragment => !serialized.includes(fragment))
  if (missing.length > 0) {
    throw new Error(`Apple FM live smoke failed public-safe checks: ${missing.join(", ")}`)
  }
}
