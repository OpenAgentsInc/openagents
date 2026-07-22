import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { describe, expect, test } from "vite-plus/test"

import { admitFullAutoRunCompletion, resolveFullAutoRunVerificationSpec } from "./full-auto-completion.ts"
import { openFullAutoRunRegistry, type FullAutoRunRegistry } from "./full-auto-run-registry.ts"
import type { FullAutoVerificationExec } from "./full-auto-verification.ts"

// Oracle for behavior contract
// openagents_desktop.full_auto_host_verified_completion.v1 (HANDS-2 #9173).

const now = () => new Date("2026-07-22T00:00:00.000Z")

const withRegistry = <A>(fn: (registry: FullAutoRunRegistry) => A): A => {
  const dir = mkdtempSync(path.join(tmpdir(), "fa-completion-"))
  try {
    return fn(openFullAutoRunRegistry(path.join(dir, "runs.json"), now))
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
}

const startAutonomyRun = (
  registry: FullAutoRunRegistry,
  options: Readonly<{ doneCondition: string; verification?: unknown; workspaceRef?: string }>,
) => {
  const started = registry.startNew({
    title: "autonomy run",
    objective: "do the bounded work",
    doneCondition: options.doneCondition,
    objectiveSource: "user",
    workspaceRef: options.workspaceRef ?? "/tmp/ws",
    actor: "control_api",
    reason: "test",
  })
  if (!started.ok) throw new Error("run failed to start")
  registry.setAutonomy(started.run.runRef, {
    enabled: true,
    ...(options.verification === undefined ? {} : { verification: options.verification as never }),
  })
  return registry.get(started.run.runRef)!
}

const passingExec: FullAutoVerificationExec = async () => ({ exitCode: 0, stdout: "ok" })
const failingExec: FullAutoVerificationExec = async () => ({ exitCode: 2, stderr: "boom" })

describe("HANDS-2 host verification-gated completion admission (#9173)", () => {
  test("resolveFullAutoRunVerificationSpec: explicit spec wins over the done-condition marker", () => {
    withRegistry((registry) => {
      const run = startAutonomyRun(registry, {
        doneCondition: "merged and green on main.\nverify: pnpm test",
        verification: { kind: "command", command: "echo explicit" },
      })
      const spec = resolveFullAutoRunVerificationSpec(run, "/ws")
      expect(spec).toEqual({ kind: "command", command: "echo explicit" })
    })
  })

  test("resolveFullAutoRunVerificationSpec: derives a command from the done-condition verify: marker", () => {
    withRegistry((registry) => {
      const run = startAutonomyRun(registry, { doneCondition: "merged and green on main.\nverify: pnpm run check" })
      const spec = resolveFullAutoRunVerificationSpec(run, "/ws")
      expect(spec).toEqual({ kind: "command", command: "pnpm run check", cwd: "/ws" })
    })
  })

  test("a PASSED verification admits completion, records the verdict, and transitions to completed", async () => {
    await withRegistry(async (registry) => {
      const run = startAutonomyRun(registry, { doneCondition: "done.\nverify: pnpm test" })
      const admission = await admitFullAutoRunCompletion({ registry, run, workspaceRef: "/ws", exec: passingExec, now })
      expect(admission.outcome).toBe("admitted")
      if (admission.outcome !== "admitted") return
      expect(admission.run.state).toBe("completed")
      expect(admission.result.status).toBe("passed")
      const stored = registry.get(run.runRef)!
      expect(stored.state).toBe("completed")
      // The host verdict is stored separately on the autonomy block.
      expect(stored.autonomy?.lastVerification?.status).toBe("passed")
      // Attributed to the host control layer, never owner_ui.
      expect(stored.transitions.at(-1)?.actor).toBe("control_api")
    })
  })

  test("a FAILED verification keeps the run active with a typed block reason and records the verdict", async () => {
    await withRegistry(async (registry) => {
      const run = startAutonomyRun(registry, { doneCondition: "done.\nverify: pnpm test" })
      const admission = await admitFullAutoRunCompletion({ registry, run, workspaceRef: "/ws", exec: failingExec, now })
      expect(admission.outcome).toBe("blocked")
      if (admission.outcome !== "blocked") return
      expect(admission.blockReason).toBe("host_verification_failed:exit_2")
      expect(admission.run.state).toBe("running")
      expect(registry.get(run.runRef)!.autonomy?.lastVerification?.status).toBe("failed")
    })
  })

  test("an ABSENT verification (no runnable check) never auto-admits completion", async () => {
    await withRegistry(async (registry) => {
      const run = startAutonomyRun(registry, { doneCondition: "just merge it, no structured marker here" })
      const admission = await admitFullAutoRunCompletion({ registry, run, workspaceRef: "/ws", exec: passingExec, now })
      expect(admission.outcome).toBe("blocked")
      if (admission.outcome !== "blocked") return
      expect(admission.blockReason).toBe("host_verification_absent")
      expect(registry.get(run.runRef)!.state).toBe("running")
    })
  })

  test("a command spec with no executor is an ERROR verdict, not a silent pass", async () => {
    await withRegistry(async (registry) => {
      const run = startAutonomyRun(registry, { doneCondition: "done.\nverify: pnpm test" })
      const admission = await admitFullAutoRunCompletion({ registry, run, workspaceRef: "/ws", now })
      expect(admission.outcome).toBe("blocked")
      if (admission.outcome !== "blocked") return
      expect(admission.blockReason).toBe("host_verification_error")
      expect(registry.get(run.runRef)!.state).toBe("running")
    })
  })

  test("a non-autonomy run is skipped so default Full Auto behavior is unchanged", async () => {
    await withRegistry(async (registry) => {
      const started = registry.startNew({
        title: "plain run",
        objective: "do work",
        doneCondition: "done.\nverify: pnpm test",
        objectiveSource: "user",
        workspaceRef: "/ws",
        actor: "control_api",
        reason: "test",
      })
      if (!started.ok) throw new Error("start failed")
      const admission = await admitFullAutoRunCompletion({ registry, run: started.run, exec: passingExec, now })
      expect(admission.outcome).toBe("skipped")
      if (admission.outcome !== "skipped") return
      expect(admission.reason).toBe("autonomy_disabled")
      expect(registry.get(started.run.runRef)!.state).toBe("running")
    })
  })

  test("an already-terminal run is skipped (idempotent replay never re-completes)", async () => {
    await withRegistry(async (registry) => {
      const run = startAutonomyRun(registry, { doneCondition: "done.\nverify: pnpm test" })
      const first = await admitFullAutoRunCompletion({ registry, run, workspaceRef: "/ws", exec: passingExec, now })
      expect(first.outcome).toBe("admitted")
      const second = await admitFullAutoRunCompletion({
        registry,
        run: registry.get(run.runRef)!,
        workspaceRef: "/ws",
        exec: passingExec,
        now,
      })
      expect(second.outcome).toBe("skipped")
      if (second.outcome !== "skipped") return
      expect(second.reason).toBe("already_terminal")
    })
  })
})
