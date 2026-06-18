// Pylon-native tip flow (issue #4712; sprint
// docs/pylon/2026-06-10-v03-sprint-agent-economy.md). The Pylon carries
// the local user's registered agent identity and uses the platform's
// reliable-tips ladder: tips never fail for a funded sender - the rung
// (direct_lightning | credited) is rendered honestly, balances are
// sweepable, and onboarding claims Spark tip-recipient readiness so the
// silent-untippable trap cannot happen to a Pylon user.

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
// claim Forum tip-recipient readiness with the node's native Spark address
// (#5345). The Spark address is derived/static and registration-free, so a
// Spark-wallet tipper can pay it Spark→Spark with no Lightning Address / LSP
// registration. The Spark Lightning Address is a best-effort optional add for
// external Lightning senders; its LSP registration may be network-blocked, and
// readiness must not depend on it. Idempotent - re-claiming refreshes the
// attachment.
export async function claimTipReadiness(
  options: TipsNetworkOptions,
  input: { pylonRef: string; sparkAddress?: string | null; lightningAddress?: string | null },
): Promise<Record<string, unknown>> {
  const slug = input.pylonRef.replace(/[^a-z0-9]+/gi, "-").toLowerCase().slice(0, 40)
  const sparkAddress =
    typeof input.sparkAddress === "string" && input.sparkAddress.trim() !== ""
      ? input.sparkAddress.trim()
      : undefined
  const lightningAddress =
    typeof input.lightningAddress === "string" && input.lightningAddress.trim() !== ""
      ? input.lightningAddress.trim()
      : undefined
  if (sparkAddress === undefined && lightningAddress === undefined) {
    throw new Error(
      "A native Spark address or a Spark Lightning Address is required for tip-recipient readiness",
    )
  }
  // The native Spark rail is self-custodial (the node's own Spark wallet), so
  // it claims `mdk_agent_wallet`. A Lightning-address-only claim still uses the
  // external-Lightning provider class.
  const providerClass = sparkAddress !== undefined ? "mdk_agent_wallet" : "external_lightning"
  const readinessRefs =
    sparkAddress !== undefined
      ? [
          "readiness.public.spark_address.offline_receive_ready",
          "readiness.public.spark_primary.agent_balance",
        ]
      : [
          "readiness.public.spark_lightning_address.receive_ready",
          "readiness.public.spark_primary.agent_balance",
        ]
  return agentRequest(options, {
    idempotencyKey: `pylon-tip-claim:${slug}`,
    body: {
      ...(sparkAddress === undefined ? {} : { sparkAddress }),
      ...(lightningAddress === undefined ? {} : { lightningAddress }),
      providerClass,
      readinessRefs,
      custodyPolicyRefs: ["policy.public.forum_tip_recipient.spark_self_custody"],
      receiveCapabilityRef: `receive_capability.public.${slug}.redacted`,
      walletRef: `wallet.public.spark.${slug}.redacted`,
    },
    method: "POST",
    path: "/api/forum/tip-recipient-wallets/claims",
  })
}
