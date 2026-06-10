#!/usr/bin/env bun

import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises"
import { join, resolve } from "node:path"
import { tmpdir } from "node:os"

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

const publicGet = async (baseUrl: string, path: string) => {
  const response = await fetch(new URL(path, baseUrl))
  const text = await response.text()
  if (!response.ok) {
    throw new Error(`${path} failed (${response.status}): ${text}`)
  }
  return JSON.parse(text) as Record<string, unknown>
}

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
  const result = await runRequired("bun pm pack @openagents/nip90", ["bun", "pm", "pack"], {
    cwd: packageRoot,
    timeoutMs: 60_000,
  })
  const tarball = result.stdout
    .split("\n")
    .map(line => line.trim())
    .find(line => /^openagents-nip90-.*\.tgz$/.test(line))

  if (!tarball) {
    throw new Error(`failed to find packed @openagents/nip90 tarball in bun pm pack output: ${result.stdout}`)
  }

  return join(packageRoot, tarball)
}

const summarizeBootstrap = (bootstrap: Record<string, unknown>) => {
  const platform = bootstrap.platform && typeof bootstrap.platform === "object"
    ? bootstrap.platform as Record<string, unknown>
    : {}
  return {
    bin: bootstrap.bin,
    packageName: bootstrap.packageName,
    platform: {
      current: platform.current,
      supported: platform.supported,
      supportedTargets: platform.supportedTargets,
    },
  }
}

const summarizeStats = (stats: Record<string, unknown>) => ({
  asOfLabel: stats.asOfLabel,
  pylonSessionsOnlineNow: stats.pylonSessionsOnlineNow,
  pylonsAssignmentReadyNow: stats.pylonsAssignmentReadyNow,
  pylonsOnlineNow: stats.pylonsOnlineNow,
  pylonsRegisteredTotal: stats.pylonsRegisteredTotal,
  pylonsSeen24h: stats.pylonsSeen24h,
  pylonsWalletReadyNow: stats.pylonsWalletReadyNow,
  sellablePylonsOnlineNow: stats.sellablePylonsOnlineNow,
})

const summarizeFunnel = (funnel: Record<string, unknown>) => {
  const summary = funnel.funnel && typeof funnel.funnel === "object"
    ? funnel.funnel as Record<string, unknown>
    : {}
  return {
    byDarkCapacityReason: summary.byDarkCapacityReason,
    byStage: summary.byStage,
    darkCount: summary.darkCount,
    eligibleCount: summary.eligibleCount,
    registeredCount: summary.registeredCount,
    totalCount: summary.totalCount,
  }
}

async function main() {
  const now = new Date()
  const baseUrl = Bun.env.OPENAGENTS_BASE_URL?.trim() || defaultBaseUrl
  const agentToken = requireEnv("OPENAGENTS_AGENT_TOKEN")
  const pylonRef =
    Bun.env.PYLON_PACKAGED_NETWORK_SMOKE_PYLON_REF?.trim() ||
    `pylon.codex.packaged_network_smoke.${compactTimestamp(now)}`
  const payoutTargetRef =
    Bun.env.PYLON_PACKAGED_NETWORK_SMOKE_PAYOUT_TARGET_REF?.trim() ||
    `payout.bolt12.packaged_network_smoke_${compactTimestamp(now)}`
  const tmpDir = await mkdtemp(join(tmpdir(), "pylon-packaged-network-smoke."))
  const projectDir = join(tmpDir, "install")
  const pylonHome = join(tmpDir, "pylon-home")
  let tarball: string | undefined
  let nip90Tarball: string | undefined
  await mkdir(projectDir, { recursive: true })

  try {
    tarball = await pack()
    nip90Tarball = await packNip90()
    await writeFile(
      join(projectDir, "package.json"),
      `${JSON.stringify({
        dependencies: {
          "@openagentsinc/pylon": `file:${tarball}`,
        },
        name: "pylon-packaged-network-smoke",
        overrides: {
          "@openagents/nip90": `file:${nip90Tarball}`,
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
      "Pylon packaged network smoke",
      "--capability-ref",
      "capability.public.pylon_cli",
      "--capability-ref",
      "capability.pylon.assignment_ready",
      "--capability-ref",
      "capability.public.packaged_binary",
      "--capability-ref",
      "capability.public.background_loop",
    ]

    const bootstrap = await runRequired(
      "packaged bootstrap",
      ["bunx", "pylon", "bootstrap", "--json", ...commonBootstrapArgs],
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
    const walletReport = await runRequired(
      "packaged wallet readiness report",
      ["bunx", "pylon", "wallet", "report-readiness", "--base-url", baseUrl],
      { cwd: projectDir, env, timeoutMs: 20_000 },
    )
    const payoutAdmission = await runRequired(
      "packaged payout-target admission request",
      [
        "bunx",
        "pylon",
        "wallet",
        "request-payout-target-admission",
        "--base-url",
        baseUrl,
        "--kind",
        "bolt12_offer",
        "--ref",
        payoutTargetRef,
      ],
      { cwd: projectDir, env },
    )

    const stats = await publicGet(baseUrl, "/api/public/pylon-stats")
    const funnel = await publicGet(baseUrl, "/api/public/pylon-capacity-funnel")
    const walletStatus = jsonFrom<{ status: { receiveReady: boolean; readiness: string } }>(walletReport).status
    const statsSummary = summarizeStats(stats)
    const funnelSummary = summarizeFunnel(funnel)
    const onlineNow = typeof statsSummary.pylonsOnlineNow === "number" && statsSummary.pylonsOnlineNow > 0
    const walletReadyNow =
      typeof statsSummary.pylonsWalletReadyNow === "number" && statsSummary.pylonsWalletReadyNow > 0
    const nonDarkNow =
      typeof funnelSummary.darkCount === "number" &&
      typeof funnelSummary.totalCount === "number" &&
      funnelSummary.totalCount > funnelSummary.darkCount
    const blockerRefs = [
      ...(walletStatus.receiveReady ? [] : ["blocker.pylon.packaged_network.wallet_not_receive_ready"]),
      ...(onlineNow ? [] : ["blocker.pylon.packaged_network.public_stats_not_online"]),
      ...(walletReadyNow ? [] : ["blocker.pylon.packaged_network.public_stats_not_wallet_ready"]),
      ...(nonDarkNow ? [] : ["blocker.pylon.packaged_network.capacity_funnel_still_dark"]),
    ]
    const result = {
      baseUrl,
      blockerRefs,
      evidenceRefs: [
        "route:/api/pylons/register",
        "route:/api/pylons/{pylonRef}/heartbeat",
        "route:/api/pylons/{pylonRef}/wallet-readiness",
        "route:/api/pylons/{pylonRef}/payout-target-admission",
        "route:/api/public/pylon-stats",
        "route:/api/public/pylon-capacity-funnel",
      ],
      funnel: funnelSummary,
      pylonRef,
      stats: statsSummary,
      status: blockerRefs.length === 0 ? "passed" : "partial",
      stepRefs: [
        "smoke.pylon.packaged_install",
        "smoke.pylon.packaged_bootstrap",
        "smoke.pylon.packaged_presence_register",
        "smoke.pylon.packaged_presence_heartbeat",
        "smoke.pylon.packaged_wallet_readiness",
        "smoke.pylon.packaged_payout_target_admission",
        "smoke.pylon.public_stats_read",
        "smoke.pylon.capacity_funnel_read",
      ],
      walletReadiness: {
        readiness: walletStatus.readiness,
        receiveReady: walletStatus.receiveReady,
      },
      bootstrap: summarizeBootstrap(jsonFrom<Record<string, unknown>>(bootstrap)),
      payoutAdmission: jsonFrom<Record<string, unknown>>(payoutAdmission),
    }

    process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
    process.exitCode = result.status === "passed" ? 0 : 2
  } finally {
    await rm(tmpDir, { recursive: true, force: true })
    if (tarball) await rm(tarball, { force: true })
    if (nip90Tarball) await rm(nip90Tarball, { force: true })
  }
}

main().catch(error => {
  process.stderr.write(`${redact(error instanceof Error ? error.message : String(error))}\n`)
  process.exitCode = 1
})
