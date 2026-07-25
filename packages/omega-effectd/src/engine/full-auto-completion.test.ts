import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { describe, expect, test } from "vite-plus/test"

import { admitFullAutoRunCompletion, resolveFullAutoRunVerificationSpec } from "./full-auto-completion.ts"
import type { FullAutoWorkspaceProbe } from "./full-auto-evidence.ts"
import {
  fullAutoRunTerminalReasonRef,
  openFullAutoRunRegistry,
  type FullAutoRunRegistry,
} from "./full-auto-run-registry.ts"
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

// OMEGA-MOB-31-03 (omega#47) / OMEGA-FA-10 (omega#43): the evidence chain the
// same admission stamps. The gate above decides WHETHER a run may finish; these
// prove that when it does, what the host publishes about the finished unit is
// something it measured.

const HEAD = "4f2b8c1d9e0a7b6c5d4e3f2a1b0c9d8e7f6a5b4c"
const BASE = "0123456789abcdef0123456789abcdef01234567"

const probe: FullAutoWorkspaceProbe = async ({ baselineRef }) => ({
  headRef: HEAD,
  generation: 7,
  diffShortstat: baselineRef === undefined ? "" : "2 files changed, 3 insertions(+)",
})

describe("OMEGA-MOB-31-03 evidence chain for a finished unit (omega#47)", () => {
  test("an admitted completion stamps a chain the host measured", async () => {
    await withRegistry(async (registry) => {
      const run = startAutonomyRun(registry, { doneCondition: "done.\nverify: pnpm test" })
      registry.recordWorkspaceBaseline(run.runRef, {
        headRef: BASE,
        generation: 6,
        recordedAtMs: now().getTime(),
      })
      const admission = await admitFullAutoRunCompletion({
        registry,
        run: registry.get(run.runRef)!,
        workspaceRef: "/ws",
        exec: passingExec,
        turnRef: "turn.full-auto.1",
        workspaceProbe: probe,
        now,
      })
      expect(admission.outcome).toBe("admitted")
      if (admission.outcome !== "admitted") return
      const evidence = admission.evidence!
      expect(evidence.hostExecuted).toBe(true)
      expect(evidence.allowed).toBe(true)
      expect(evidence.turnRef).toBe("turn.full-auto.1")
      expect(evidence.changeRef).toBe(`change.${HEAD}`)
      expect(evidence.projectGeneration).toBe("generation.project.00007")
      expect(evidence.diffSummary).toBe(`since ${BASE.slice(0, 7)}: 2 files changed, 3 insertions(+)`)
      expect(evidence.testCommand).toBe("pnpm test")
      expect(evidence.testOutcome).toBe("outcome.test.passed")
      // Durable on the run, not just returned to the caller.
      expect(registry.get(run.runRef)!.evidence).toEqual(evidence)
    })
  })

  test("a BLOCKED verification stamps nothing: an unfinished unit has no chain", async () => {
    await withRegistry(async (registry) => {
      const run = startAutonomyRun(registry, { doneCondition: "done.\nverify: pnpm test" })
      const admission = await admitFullAutoRunCompletion({
        registry,
        run,
        workspaceRef: "/ws",
        exec: failingExec,
        turnRef: "turn.full-auto.1",
        workspaceProbe: probe,
        now,
      })
      expect(admission.outcome).toBe("blocked")
      expect(registry.get(run.runRef)!.evidence).toBeUndefined()
    })
  })

  test("without a host turn ref or a workspace the host can read, nothing is stamped", async () => {
    for (const missing of ["turnRef", "probe", "workspace"] as const) {
      await withRegistry(async (registry) => {
        const run = startAutonomyRun(registry, { doneCondition: "done.\nverify: pnpm test" })
        const admission = await admitFullAutoRunCompletion({
          registry,
          run,
          workspaceRef: "/ws",
          exec: passingExec,
          ...(missing === "turnRef" ? {} : { turnRef: "turn.full-auto.1" }),
          ...(missing === "probe"
            ? {}
            : { workspaceProbe: missing === "workspace" ? async () => null : probe }),
          now,
        })
        expect(admission.outcome).toBe("admitted")
        if (admission.outcome !== "admitted") return
        // The run still finished -- honestly, with no chain -- rather than
        // finishing with an invented one.
        expect(admission.evidence).toBeNull()
        expect(registry.get(run.runRef)!.state).toBe("completed")
        expect(registry.get(run.runRef)!.evidence).toBeUndefined()
      })
    }
  })

  test("an `evidence_ref` check is host-consulted, not host-executed, and stamps nothing", async () => {
    await withRegistry(async (registry) => {
      const run = startAutonomyRun(registry, {
        doneCondition: "done.",
        verification: { kind: "evidence_ref", ref: "evidence.some.ref" },
      })
      const admission = await admitFullAutoRunCompletion({
        registry,
        run,
        workspaceRef: "/ws",
        evidencePresent: () => true,
        turnRef: "turn.full-auto.1",
        workspaceProbe: probe,
        now,
      })
      expect(admission.outcome).toBe("admitted")
      if (admission.outcome !== "admitted") return
      expect(admission.evidence).toBeNull()
    })
  })

  test("the chain is write-once: a second stamp never re-points a finished unit", async () => {
    await withRegistry(async (registry) => {
      const run = startAutonomyRun(registry, { doneCondition: "done.\nverify: pnpm test" })
      const admission = await admitFullAutoRunCompletion({
        registry,
        run,
        workspaceRef: "/ws",
        exec: passingExec,
        turnRef: "turn.full-auto.1",
        workspaceProbe: probe,
        now,
      })
      expect(admission.outcome).toBe("admitted")
      const stamped = registry.get(run.runRef)!.evidence!
      registry.recordEvidence(run.runRef, { ...stamped, turnRef: "turn.full-auto.99" })
      expect(registry.get(run.runRef)!.evidence).toEqual(stamped)
    })
  })

  test("the baseline is write-once: a run's starting tree does not move", () => {
    withRegistry((registry) => {
      const run = startAutonomyRun(registry, { doneCondition: "done." })
      const first = { headRef: BASE, generation: 6, recordedAtMs: now().getTime() }
      registry.recordWorkspaceBaseline(run.runRef, first)
      registry.recordWorkspaceBaseline(run.runRef, {
        headRef: HEAD,
        generation: 9,
        recordedAtMs: now().getTime() + 1,
      })
      expect(registry.get(run.runRef)!.workspaceBaseline).toEqual(first)
    })
  })

  test("a run finished before the chain existed is never backfilled with one", () => {
    withRegistry((registry) => {
      const run = startAutonomyRun(registry, { doneCondition: "done." })
      registry.transition(run.runRef, { to: "stopped", actor: "owner_ui", reason: "owner stopped" })
      const finished = registry.get(run.runRef)!
      expect(finished.state).toBe("stopped")
      // The facts a backfill would be tempted to reconstruct one from are all
      // present -- an objective, a terminal state, transitions -- and none of
      // them makes a chain appear.
      expect(finished.evidence).toBeUndefined()
      expect(fullAutoRunTerminalReasonRef(finished)).toBe("terminal.full_auto.stopped.owner_ui")
    })
  })
})
