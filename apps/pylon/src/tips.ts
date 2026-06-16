// Pylon-native tip flow (issue #4712; sprint
// docs/pylon/2026-06-10-v03-sprint-agent-economy.md). The Pylon carries
// the local user's registered agent identity and uses the platform's
// reliable-tips ladder: tips never fail for a funded sender - the rung
// (direct_bolt12 | credited) is rendered honestly, balances are
// sweepable, and onboarding claims tip-recipient readiness so the
// silent-untippable trap cannot happen to a Pylon user.

import type { WalletCommandRunner } from "./wallet"
import { defaultWalletCommandRunner } from "./wallet"

export type TipsNetworkOptions = {
  baseUrl: string
  agentToken?: string
  fetch?: typeof fetch
  now?: () => Date
}

function requireAgentToken(options: TipsNetworkOptions): string {
  const token = options.agentToken ?? process.env.OPENAGENTS_AGENT_TOKEN
  if (!token) {
    throw new Error("OPENAGENTS_AGENT_TOKEN or --agent-token is required")
  }
  return token
}

async function agentRequest(
  options: TipsNetworkOptions,
  input: { path: string; method: "GET" | "POST"; body?: Record<string, unknown>; idempotencyKey?: string },
): Promise<Record<string, unknown>> {
  const token = requireAgentToken(options)
  const response = await (options.fetch ?? fetch)(new URL(input.path, options.baseUrl), {
    ...(input.body === undefined ? {} : { body: JSON.stringify(input.body) }),
    headers: {
      Authorization: `Bearer ${token}`,
      "Content-Type": "application/json",
      ...(input.idempotencyKey === undefined ? {} : { "Idempotency-Key": input.idempotencyKey }),
    },
    method: input.method,
  })
  const text = await response.text()
  const payload = text.trim() ? JSON.parse(text) as Record<string, unknown> : {}
  if (!response.ok) {
    const reason = typeof payload.reason === "string" ? payload.reason : typeof payload.error === "string" ? payload.error : String(response.status)
    throw new Error(`pylon tips request failed (${response.status}): ${reason}`)
  }
  return payload
}

export async function tipPost(
  options: TipsNetworkOptions,
  input: { postId: string; amountSat: number },
): Promise<Record<string, unknown>> {
  if (!Number.isInteger(input.amountSat) || input.amountSat <= 0) {
    throw new Error("tip amount must be a positive integer number of sats")
  }
  const now = options.now?.() ?? new Date()
  const idempotencyKey = `pylon-tip:${input.postId}:${input.amountSat}:${now.toISOString().slice(0, 16)}`
  return agentRequest(options, {
    body: { amountSat: input.amountSat },
    idempotencyKey,
    method: "POST",
    path: `/api/forum/posts/${encodeURIComponent(input.postId)}/tips/ladder`,
  })
}

export async function readBalance(options: TipsNetworkOptions): Promise<Record<string, unknown>> {
  return agentRequest(options, { method: "GET", path: "/api/agents/me/balance" })
}

export async function sweepStatus(options: TipsNetworkOptions): Promise<Record<string, unknown>> {
  const payload = await readBalance(options)
  const recent = Array.isArray(payload.recentActivity) ? payload.recentActivity as Array<Record<string, unknown>> : []
  return {
    balance: payload.balance,
    sweeps: recent.filter((row) => row.payInType === "sweep"),
  }
}

export async function setTipPreferences(
  options: TipsNetworkOptions,
  prefs: {
    sweepEnabled?: boolean
    sweepThresholdSat?: number
    sendCreditsBelowSat?: number
    receiveCreditsBelowSat?: number
  },
): Promise<Record<string, unknown>> {
  return agentRequest(options, {
    body: prefs,
    method: "POST",
    path: "/api/agents/me/balance/preferences",
  })
}

// Onboarding auto-claim (the Kenobi/Comunero lesson, made structural):
// generate a BOLT 12 offer from the local MDK wallet and claim Forum
// tip-recipient readiness with it. Idempotent - re-claiming refreshes
// the offer attachment.
export async function claimTipReadiness(
  options: TipsNetworkOptions,
  input: { pylonRef: string; lightningAddress?: string | null },
  runner: WalletCommandRunner = defaultWalletCommandRunner,
): Promise<Record<string, unknown>> {
  const offerResult = await runner(["receive-bolt12"])
  let offer: string | undefined
  try {
    const parsed = JSON.parse(offerResult.stdout.trim()) as { offer?: string }
    offer = parsed.offer?.trim()
  } catch {
    offer = undefined
  }
  if (!offer || !offer.startsWith("lno1")) {
    throw new Error("could not generate a BOLT 12 offer from the local wallet (is the daemon configured?)")
  }

  const slug = input.pylonRef.replace(/[^a-z0-9]+/gi, "-").toLowerCase().slice(0, 40)
  // The static Spark-hosted Lightning Address (#5078) is published alongside the
  // BOLT 12 offer when available, so the treasury can pay this recipient even
  // while their node is offline (LNURL -> BOLT11, LSP-held, claimed on sync).
  const lightningAddress =
    typeof input.lightningAddress === "string" && input.lightningAddress.trim() !== ""
      ? input.lightningAddress.trim()
      : undefined
  return agentRequest(options, {
    idempotencyKey: `pylon-tip-claim:${slug}`,
    body: {
      bolt12Offer: offer,
      ...(lightningAddress ? { lightningAddress } : {}),
      readinessRefs: [
        "readiness.public.mdk_agent.daemon_running",
        "readiness.public.mdk_agent.setup_present",
        "readiness.public.mdk_agent.receive_ready",
      ],
      receiveCapabilityRef: `receive_capability.public.${slug}.redacted`,
      walletRef: `wallet.public.${slug}.redacted`,
    },
    method: "POST",
    path: "/api/forum/tip-recipient-wallets/claims",
  })
}
