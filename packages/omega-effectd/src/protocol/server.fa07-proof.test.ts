/**
 * FA-07 proof matrix — supervised omega-effectd journey (not fixture-only).
 *
 * Consolidates stronger automated gates from the Full Auto port audit §12.2
 * on the framed protocol. Packaged Omega owner dogfood and independent
 * assurance remain separate evidence rows in the FA-07 receipt.
 */

import { mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import { describe, expect, test } from "vite-plus/test"

import { createOmegaEffectdService } from "../service.ts"
import { resolveFullAutoRegistryPath } from "../paths.ts"
import { createOmegaEffectdFramedServer } from "./server.ts"
import { OMEGA_EFFECTD_PROTOCOL_SCHEMA } from "./framed.ts"

const withRoot = async (fn: (root: string) => Promise<void>): Promise<void> => {
  const root = mkdtempSync(path.join(tmpdir(), "oa-effectd-fa07-"))
  try {
    await fn(root)
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
}

const request = (id: string, generation: number, method: string, params?: unknown) =>
  JSON.stringify({
    schema: OMEGA_EFFECTD_PROTOCOL_SCHEMA,
    kind: "request",
    id,
    generation,
    method,
    ...(params === undefined ? {} : { params }),
  })

describe("FA-07 proof matrix (omega-effectd)", () => {
  test("incident eviction → typed stall; controls; redaction; native join; Sync stub; mobile", async () => {
    await withRoot(async root => {
      const service = createOmegaEffectdService({ paths: { dataRoot: root } })
      const server = createOmegaEffectdFramedServer(service, { dataRoot: root })
      await server.handleLine(request("1", 0, "initialize", { generation: 1 }))

      const started = await server.handleLine(
        request("2", 1, "start", {
          workspaceRef: "workspace.omega.supervised",
          title: "FA-07 proof",
          objective: "SECRET_FA07_OBJECTIVE_MUST_NOT_ENTER_RECEIPT_OR_ATTENTION",
          doneCondition: "SECRET_FA07_DONE",
          turnCap: 6,
          projectRef: "project.fa07",
          worktreeRef: "worktree.fa07",
          gitHead: "abc123",
        }),
      )
      expect(started?.ok).toBe(true)
      const run = (
        started?.result as {
          run: {
            runRef: string
            threadRef: string
            nativeEvidence: { projectRef: string; worktreeRef: string; gitHead: string | null }
          }
        }
      ).run
      expect(run.nativeEvidence.projectRef).toBe("project.fa07")
      expect(run.nativeEvidence.worktreeRef).toBe("worktree.fa07")
      expect(run.nativeEvidence.gitHead).toBe("abc123")

      const paused = await server.handleLine(request("3", 1, "pause", { runRef: run.runRef }))
      expect((paused?.result as { run: { state: string } }).run.state).toBe("paused")
      const resumed = await server.handleLine(request("4", 1, "resume", { runRef: run.runRef }))
      expect((resumed?.result as { run: { state: string } }).run.state).toBe("running")

      const mobilePause = await server.handleLine(
        request("5", 1, "apply_control_intent", {
          intentId: "intent.fa07.pause",
          runRef: run.runRef,
          action: "pause",
          actor: "mobile",
        }),
      )
      expect(
        (mobilePause?.result as { outcome: { status: string } }).outcome.status,
      ).toBe("applied")
      const mobileResume = await server.handleLine(
        request("5b", 1, "apply_control_intent", {
          intentId: "intent.fa07.resume",
          runRef: run.runRef,
          action: "resume",
          actor: "mobile",
        }),
      )
      expect(
        (mobileResume?.result as { outcome: { status: string } }).outcome.status,
      ).toBe("applied")
      expect(
        (
          (
            await server.handleLine(request("5c", 1, "get_run", { runRef: run.runRef }))
          )?.result as { run: { state: string } }
        ).run.state,
      ).toBe("running")

      const receipt = await server.handleLine(request("6", 1, "get_receipt", { runRef: run.runRef }))
      const receiptJson = JSON.stringify(receipt)
      expect(receiptJson).not.toContain("SECRET_FA07_OBJECTIVE")
      expect(receiptJson).not.toContain("SECRET_FA07_DONE")
      expect(
        typeof (receipt?.result as { receipt: { objectiveDigest: string } }).receipt
          .objectiveDigest,
      ).toBe("string")

      const sync = await server.handleLine(request("7", 1, "get_sync_status"))
      expect((sync?.result as { publishBlocksDispatch: boolean }).publishBlocksDispatch).toBe(
        false,
      )

      const publish = await server.handleLine(
        request("8", 1, "publish_projection", { runRef: run.runRef }),
      )
      expect((publish?.result as { ok: boolean }).ok).toBe(false)

      // 2026-07-17 eviction shape: drop host thread record → typed stall.
      writeFileSync(
        resolveFullAutoRegistryPath({ dataRoot: root }),
        JSON.stringify(
          { schema: "openagents.desktop.full_auto_registry.v1", records: [] },
          null,
          2,
        ),
      )
      const serviceB = createOmegaEffectdService({ paths: { dataRoot: root } })
      const serverB = createOmegaEffectdFramedServer(serviceB, { dataRoot: root })
      await serverB.handleLine(request("20", 0, "initialize", { generation: 2 }))
      const detail = await serverB.handleLine(
        request("21", 2, "get_run", { runRef: run.runRef }),
      )
      const stalled = (
        detail?.result as {
          run: { state: string; stallCause: string | null; recoveryAction: string }
        }
      ).run
      expect(stalled.state).toBe("stalled")
      expect(stalled.stallCause).toBe("host_thread_missing")
      expect(stalled.recoveryAction).toBe("stop_only")

      const attention = await serverB.handleLine(
        request("22", 2, "decide_attention", {
          runRef: run.runRef,
          permissionGranted: true,
        }),
      )
      const note = (
        attention?.result as { attention: { title: string; body: string } | null }
      ).attention
      expect(note).not.toBeNull()
      expect(JSON.stringify(note)).not.toContain("SECRET_FA07_OBJECTIVE")

      const boundary = await serverB.handleLine(
        request("23", 2, "assess_native_boundary", { runRef: run.runRef }),
      )
      expect((boundary?.result as { assessment: { ok: boolean } }).assessment.ok).toBe(true)

      const stopped = await serverB.handleLine(request("24", 2, "stop", { runRef: run.runRef }))
      expect((stopped?.result as { run: { state: string } }).run.state).toBe("stopped")
    })
  })

  test("ordinary list_runs never carries objective text", async () => {
    await withRoot(async root => {
      const service = createOmegaEffectdService({ paths: { dataRoot: root } })
      const server = createOmegaEffectdFramedServer(service, { dataRoot: root })
      await server.handleLine(request("1", 0, "initialize", { generation: 1 }))
      await server.handleLine(
        request("2", 1, "start", {
          workspaceRef: "workspace.omega.supervised",
          title: "Redaction list",
          objective: "MUST_NOT_APPEAR_IN_LIST_RUNS",
          doneCondition: "done",
          projectRef: "project.list",
          worktreeRef: "worktree.list",
        }),
      )
      const listed = await server.handleLine(request("3", 1, "list_runs"))
      expect(JSON.stringify(listed)).not.toContain("MUST_NOT_APPEAR_IN_LIST_RUNS")
    })
  })
})
