#!/usr/bin/env bun

import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join, resolve } from "node:path"

type CommandResult = {
  exitCode: number
  stderr: string
  stdout: string
}

const repoRoot = resolve(import.meta.dir, "..")
const workspaceRoot = resolve(repoRoot, "../..")
const defaultBaseUrl = "https://openagents.com"

const compactTimestamp = (date: Date) =>
  date.toISOString().replace(/\D/g, "").slice(0, 14)

const requireEnv = (key: string) => {
  const value = Bun.env[key]?.trim()
  if (!value) throw new Error(`${key} is required`)
  return value
}

const redact = (text: string) =>
  text
    .replace(/Bearer\s+[A-Za-z0-9._~+/-]+/g, "Bearer <redacted>")
    .replace(/oa_agent_[A-Za-z0-9._~+/-]+/g, "oa_agent_<redacted>")
    .replace(/OPENAGENTS_AGENT_TOKEN=[^\s]+/g, "OPENAGENTS_AGENT_TOKEN=<redacted>")
    .replace(/OPENAGENTS_ADMIN_API_TOKEN=[^\s]+/g, "OPENAGENTS_ADMIN_API_TOKEN=<redacted>")

const run = async (
  args: string[],
  options: { cwd: string; env?: Record<string, string | undefined>; timeoutMs?: number },
): Promise<CommandResult> => {
  const proc = Bun.spawn(args, {
    cwd: options.cwd,
    env: { ...process.env, ...options.env },
    stderr: "pipe",
    stdout: "pipe",
  })
  const timeout = setTimeout(() => proc.kill(), options.timeoutMs ?? 60_000)
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]).finally(() => clearTimeout(timeout))

  return { exitCode, stderr: redact(stderr), stdout: redact(stdout) }
}

const runRequired = async (
  label: string,
  args: string[],
  options: { cwd: string; env?: Record<string, string | undefined>; timeoutMs?: number },
) => {
  const result = await run(args, options)
  if (result.exitCode !== 0) {
    throw new Error(`${label} failed (${result.exitCode}): ${result.stderr || result.stdout}`)
  }
  return result
}

const jsonFrom = <T>(result: CommandResult): T => JSON.parse(result.stdout) as T

const summarizeLease = (lease: unknown) => {
  if (lease === null || typeof lease !== "object") return null
  const record = lease as Record<string, unknown>
  return {
    assignmentRef: typeof record.assignmentRef === "string" ? record.assignmentRef : null,
    capabilityRefs: Array.isArray(record.capabilityRefs)
      ? record.capabilityRefs.filter((ref): ref is string => typeof ref === "string")
      : [],
    goal: typeof record.goal === "string" ? record.goal : null,
    leaseRef: typeof record.leaseRef === "string" ? record.leaseRef : null,
    paymentMode: typeof record.paymentMode === "string" ? record.paymentMode : null,
  }
}

const summarizeRunNoSpendFailure = (result: Record<string, any>) => {
  if (result.ok === true) return null
  const acceptance = result.acceptance as Record<string, unknown> | undefined
  return {
    acceptance: acceptance === undefined
      ? null
      : {
          accepted: acceptance.accepted === true,
          blockerRefs: Array.isArray(acceptance.blockerRefs)
            ? acceptance.blockerRefs.filter((ref): ref is string => typeof ref === "string")
            : [],
          denialRef: typeof acceptance.denialRef === "string" ? acceptance.denialRef : null,
          statusRef: typeof acceptance.statusRef === "string" ? acceptance.statusRef : null,
        },
    lease: summarizeLease(result.lease),
    leases: Array.isArray(result.leases) ? result.leases.map(summarizeLease) : [],
    reason: typeof result.reason === "string" ? result.reason : null,
  }
}

const postJson = async (
  baseUrl: string,
  path: string,
  body: Record<string, unknown>,
  adminToken: string,
) => {
  const response = await fetch(new URL(path, baseUrl), {
    method: "POST",
    headers: {
      authorization: `Bearer ${adminToken}`,
      "content-type": "application/json",
      "Idempotency-Key": `pylon-packaged-runtime-task-smoke:${body.assignmentRef ?? compactTimestamp(new Date())}`,
    },
    body: JSON.stringify(body),
  })
  const text = await response.text()
  if (!response.ok) {
    throw new Error(`${path} failed (${response.status}): ${redact(text)}`)
  }
  return text.trim() ? JSON.parse(text) as Record<string, unknown> : {}
}

const runtimeCapabilityRefs = [
  "cap.gepa.retained.v1",
  "capability.public.packaged_binary",
  "capability.public.pylon_runtime_gate",
]

const pack = async () => {
  const result = await runRequired("bun pm pack", ["bun", "pm", "pack"], {
    cwd: repoRoot,
    timeoutMs: 60_000,
  })
  const tarball = result.stdout
    .split("\n")
    .map(line => line.trim())
    .find(line => /^openagentsinc-pylon-.*\.tgz$/.test(line))

  if (!tarball) {
    throw new Error(`failed to find packed tarball in bun pm pack output: ${result.stdout}`)
  }

  return join(repoRoot, tarball)
}

const packNip90 = async () => {
  const packageRoot = join(workspaceRoot, "packages/nip90")
  const result = await runRequired("bun pm pack @openagentsinc/nip90", ["bun", "pm", "pack"], {
    cwd: packageRoot,
    timeoutMs: 60_000,
  })
  const tarball = result.stdout
    .split("\n")
    .map(line => line.trim())
    .find(line => /^openagents-nip90-.*\.tgz$/.test(line))

  if (!tarball) {
    throw new Error(`failed to find packed @openagentsinc/nip90 tarball in bun pm pack output: ${result.stdout}`)
  }

  return join(packageRoot, tarball)
}

const packTassadarExecutor = async () => {
  const packageRoot = join(workspaceRoot, "packages/tassadar-executor")
  const result = await runRequired("bun pm pack @openagentsinc/tassadar-executor", ["bun", "pm", "pack"], {
    cwd: packageRoot,
    timeoutMs: 60_000,
  })
  const tarball = result.stdout
    .split("\n")
    .map(line => line.trim())
    .find(line => /^openagents-tassadar-executor-.*\.tgz$/.test(line))

  if (!tarball) {
    throw new Error(`failed to find packed @openagentsinc/tassadar-executor tarball in bun pm pack output: ${result.stdout}`)
  }

  return join(packageRoot, tarball)
}

const assignmentBody = (input: { assignmentRef: string; pylonRef: string }) => ({
  acceptanceCriteriaRefs: ["acceptance.public.pylon_runtime_gate.bounded_fixture_test_passes"],
  assignmentRef: input.assignmentRef,
  budget: {
    amountSats: 0,
    currency: "SAT",
    paymentMode: "unpaid_smoke",
  },
  campaignPaused: false,
  campaignPolicyRefs: ["policy.public.no_spend_smoke"],
  campaignRef: "campaign.public.pylon_runtime_gate_smoke",
  codingAssignment: {
    assignmentRef: input.assignmentRef,
    budget: {
      paymentMode: "unpaid_smoke",
    },
    objective: {
      objectiveRef: "objective.public.pylon_runtime_gate.fixture_repair",
    },
    publicSafe: true,
    requiredCapabilityRefs: ["cap.gepa.retained.v1"],
    runtimeGate: {
      agentKind: "codex_cli_or_fixture",
      fixtureRef: "fixture.public.pylon.codex_runtime.sum_repair.v1",
      schema: "openagents.pylon.runtime_gate.v0.3",
    },
    schema: "openagents.autopilot_coding_assignment.v1",
  },
  closeoutPathRefs: ["closeout.public.operator_review_required"],
  forumAutoPublishAllowed: false,
  idempotencyRefs: ["idempotency.public.pylon_runtime_gate"],
  jobKind: "validation",
  leaseSeconds: 600,
  noDuplicateAssignmentRefs: ["dedupe.public.pylon_runtime_gate_smoke"],
  noForumAutoPublishRefs: ["policy.public.no_forum_auto_publish"],
  operatorPauseRefs: ["pause.public.pylon_runtime_gate.not_paused"],
  paymentMode: "unpaid_smoke",
  pylonRef: input.pylonRef,
  requiredCapabilityRefs: runtimeCapabilityRefs,
  resultExpectationRefs: ["result.public.pylon_runtime_gate.fixture_repair_passed"],
  rollbackRefs: ["rollback.public.cancel_smoke_assignment"],
  selectionPolicyRefs: ["selection.public.explicit_pylon_ref"],
  spendCapRefs: ["spend_cap.public.no_spend"],
  taskRefs: ["task.public.pylon_runtime_gate.fixture_repair"],
})

async function main() {
  const now = new Date()
  const baseUrl = Bun.env.OPENAGENTS_BASE_URL?.trim() || defaultBaseUrl
  const agentToken = requireEnv("OPENAGENTS_AGENT_TOKEN")
  const adminToken = Bun.env.OPENAGENTS_ADMIN_API_TOKEN?.trim()
  const pylonRef =
    Bun.env.PYLON_PACKAGED_RUNTIME_TASK_SMOKE_PYLON_REF?.trim() ||
    `pylon.codex.packaged_runtime_task_smoke.${compactTimestamp(now)}`
  const assignmentRef =
    Bun.env.PYLON_PACKAGED_RUNTIME_TASK_SMOKE_ASSIGNMENT_REF?.trim() ||
    `assignment.public.pylon_runtime_gate.${compactTimestamp(now)}`
  const tmpDir = await mkdtemp(join(tmpdir(), "pylon-packaged-runtime-task-smoke."))
  const projectDir = join(tmpDir, "install")
  const pylonHome = join(tmpDir, "pylon-home")
  let tarball: string | undefined
  let nip90Tarball: string | undefined
  let tassadarExecutorTarball: string | undefined
  await mkdir(projectDir, { recursive: true })

  try {
    tarball = await pack()
    nip90Tarball = await packNip90()
    tassadarExecutorTarball = await packTassadarExecutor()
    await writeFile(
      join(projectDir, "package.json"),
      `${JSON.stringify({
        dependencies: {
          "@openagentsinc/pylon": `file:${tarball}`,
        },
        name: "pylon-packaged-runtime-task-smoke",
        overrides: {
          "@openagentsinc/nip90": `file:${nip90Tarball}`,
          "@openagentsinc/tassadar-executor": `file:${tassadarExecutorTarball}`,
        },
        private: true,
        type: "module",
      }, null, 2)}\n`,
    )
    await runRequired(
      "fresh packaged install",
      ["bun", "--dns-result-order=ipv4first", "install"],
      { cwd: projectDir, timeoutMs: 90_000 },
    )

    const env = {
      OPENAGENTS_AGENT_TOKEN: agentToken,
      PYLON_HOME: pylonHome,
      PYLON_OPENAGENTS_BASE_URL: baseUrl,
    }
    const commonBootstrapArgs = [
      "--pylon-ref",
      pylonRef,
      "--display-name",
      "Pylon packaged runtime task smoke",
      ...runtimeCapabilityRefs.flatMap(ref => ["--capability-ref", ref]),
    ]
    const bootstrap = await runRequired(
      "packaged bootstrap",
      [
        "bunx",
        "pylon",
        "bootstrap",
        "--json",
        ...commonBootstrapArgs,
      ],
      { cwd: projectDir, env },
    )
    await runRequired(
      "packaged provider go-online",
      ["bunx", "pylon", "provider", "go-online"],
      { cwd: projectDir, env },
    )
    await runRequired(
      "packaged presence register",
      ["bunx", "pylon", "presence", "register", "--base-url", baseUrl, ...commonBootstrapArgs],
      { cwd: projectDir, env },
    )
    await runRequired(
      "packaged presence heartbeat",
      ["bunx", "pylon", "presence", "heartbeat", "--base-url", baseUrl, ...commonBootstrapArgs],
      { cwd: projectDir, env },
    )
    await runRequired(
      "packaged wallet readiness report",
      ["bunx", "pylon", "wallet", "report-readiness", "--base-url", baseUrl],
      { cwd: projectDir, env, timeoutMs: 20_000 },
    )
    const bootstrapJson = jsonFrom<Record<string, any>>(bootstrap)

    const assignmentCreated = adminToken
      ? await postJson(baseUrl, "/api/operator/pylons/assignments", assignmentBody({ assignmentRef, pylonRef }), adminToken)
      : null
    const runNoSpend = await runRequired(
      "packaged runtime task run-no-spend",
      ["bunx", "pylon", "assignment", "run-no-spend", "--base-url", baseUrl],
      { cwd: projectDir, env, timeoutMs: 120_000 },
    )
    const result = jsonFrom<Record<string, any>>(runNoSpend)
    const closeout = result.closeout as Record<string, unknown> | undefined
    const failure = summarizeRunNoSpendFailure(result)
    const blockerRefs = [
      ...(assignmentCreated === null && result.ok !== true
        ? ["blocker.pylon.packaged_runtime_task.admin_assignment_create_token_missing"]
        : []),
      ...(result.ok === true ? [] : ["blocker.pylon.packaged_runtime_task.not_accepted"]),
      ...(failure?.acceptance?.blockerRefs ?? []),
    ]
    const output = {
      assignmentCreated: assignmentCreated === null ? false : true,
      assignmentRef,
      baseUrl,
      blockerRefs,
      closeout: closeout === undefined
        ? null
        : {
            artifactRefs: closeout.artifactRefs,
            buildRefs: closeout.buildRefs,
            closeoutRefs: closeout.closeoutRefs,
            paymentMode: closeout.paymentMode,
            payoutClaimAllowed: closeout.payoutClaimAllowed,
            proofRefs: closeout.proofRefs,
            receiptRefs: closeout.receiptRefs,
            resultRefs: closeout.resultRefs,
            settlementState: closeout.settlementState,
            status: closeout.status,
            summaryRefs: closeout.summaryRefs,
            testRefs: closeout.testRefs,
          },
      evidenceRefs: [
        "route:/api/pylons/register",
        "route:/api/pylons/{pylonRef}/heartbeat",
        "route:/api/pylons/{pylonRef}/assignments",
        "route:/api/pylons/{pylonRef}/assignments/{assignmentRef}/accept",
        "route:/api/pylons/{pylonRef}/assignments/{assignmentRef}/progress",
        "route:/api/pylons/{pylonRef}/assignments/{assignmentRef}/artifacts",
        "route:/api/pylons/{pylonRef}/assignments/{assignmentRef}/closeout",
      ],
      failure,
      pylonRef,
      status: blockerRefs.length === 0 ? "passed" : "partial",
      stepRefs: [
        "smoke.pylon.packaged_install",
        "smoke.pylon.packaged_bootstrap",
        "smoke.pylon.packaged_provider_go_online",
        "smoke.pylon.packaged_presence_register",
        "smoke.pylon.packaged_presence_heartbeat",
        "smoke.pylon.packaged_wallet_readiness",
        ...(assignmentCreated === null ? ["skip.pylon.assignment_create.admin_token_missing"] : ["smoke.pylon.assignment_create"]),
        "smoke.pylon.packaged_runtime_task_run_no_spend",
      ],
      bootstrap: {
        bin: bootstrapJson.bin,
        packageName: bootstrapJson.packageName,
        platform: (bootstrapJson.platform as Record<string, unknown> | undefined)?.current,
        supported: (bootstrapJson.platform as Record<string, unknown> | undefined)?.supported,
      },
    }

    process.stdout.write(`${JSON.stringify(output, null, 2)}\n`)
    process.exitCode = output.status === "passed" ? 0 : 2
  } finally {
    await rm(tmpDir, { recursive: true, force: true })
    if (tarball) await rm(tarball, { force: true })
    if (nip90Tarball) await rm(nip90Tarball, { force: true })
    if (tassadarExecutorTarball) await rm(tassadarExecutorTarball, { force: true })
  }
}

main().catch(error => {
  process.stderr.write(`${redact(error instanceof Error ? error.message : String(error))}\n`)
  process.exitCode = 1
})
