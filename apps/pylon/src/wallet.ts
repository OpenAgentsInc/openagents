import { readFile, writeFile } from "node:fs/promises"
import { existsSync } from "node:fs"
import { createHash, randomUUID } from "node:crypto"
import type { PylonPaths } from "./state"
import { assertPublicProjectionSafe, ensureStateDirectories } from "./state"

export type WalletReadiness =
  | "daemon-offline"
  | "balance-unknown"
  | "receive-ready"
  | "send-ready"
  | "send-ready-blocked"
  | "payout-target-admitted"
  | "payable-pending-settlement"
  | "settlement-recorded"

export type PayoutTargetKind = "bolt12_offer" | "bolt11_invoice" | "bip353_name" | "lnurl_pay"

export type WalletStatusProjection = {
  schema: "openagents.pylon.wallet_status.v0.3"
  configured: boolean
  daemonOnline: boolean
  balanceSats: number | null
  receiveReady: boolean
  sendReady: boolean
  readiness: WalletReadiness
  blockerRefs: string[]
  payoutTargetRefs: string[]
  settlementRefs: string[]
}

export type WalletCommandResult = {
  exitCode: number
  stdout: string
  stderr: string
}

export type WalletCommandRunner = (args: string[]) => Promise<WalletCommandResult>

export type LedgerEvent = {
  eventId: string
  kind: string
  ref: string
  createdAt: string
  data: Record<string, unknown>
}

export const defaultWalletCommandRunner: WalletCommandRunner = async (args) => {
  const proc = Bun.spawn(["npx", "--yes", "@moneydevkit/agent-wallet@latest", ...args], {
    stdout: "pipe",
    stderr: "pipe",
  })
  const timeout = new Promise<never>((_, reject) =>
    setTimeout(() => {
      proc.kill()
      reject(new Error("MDK agent-wallet command timed out"))
    }, 3000),
  )
  const [stdout, stderr, exitCode] = await Promise.race([
    Promise.all([
      new Response(proc.stdout).text(),
      new Response(proc.stderr).text(),
      proc.exited,
    ]),
    timeout,
  ])
  return { exitCode, stdout, stderr }
}

function stableRef(prefix: string, value: string) {
  return `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`
}

function parseJson(stdout: string) {
  if (!stdout.trim()) return null
  return JSON.parse(stdout) as Record<string, unknown>
}

export async function classifyMdkWallet(runner: WalletCommandRunner = defaultWalletCommandRunner) {
  const result = await runner(["balance"])
  if (result.exitCode !== 0) {
    return {
      schema: "openagents.pylon.wallet_status.v0.3",
      configured: false,
      daemonOnline: false,
      balanceSats: null,
      receiveReady: false,
      sendReady: false,
      readiness: "daemon-offline",
      blockerRefs: ["blocker.wallet.daemon_offline"],
      payoutTargetRefs: [],
      settlementRefs: [],
    } satisfies WalletStatusProjection
  }

  const data = parseJson(result.stdout)
  const balance =
    typeof data?.balance_sats === "number"
      ? data.balance_sats
      : typeof data?.balance === "number"
        ? data.balance
        : typeof data?.confirmed === "number"
          ? data.confirmed
          : null
  if (balance === null) {
    return {
      schema: "openagents.pylon.wallet_status.v0.3",
      configured: true,
      daemonOnline: true,
      balanceSats: null,
      receiveReady: false,
      sendReady: false,
      readiness: "balance-unknown",
      blockerRefs: ["blocker.wallet.balance_unknown"],
      payoutTargetRefs: [],
      settlementRefs: [],
    } satisfies WalletStatusProjection
  }

  const sendReady = data?.send_ready === true && data?.restored_mnemonic_only !== true && data?.outbound_capacity_sats !== 0
  return {
    schema: "openagents.pylon.wallet_status.v0.3",
    configured: true,
    daemonOnline: true,
    balanceSats: balance,
    receiveReady: true,
    sendReady,
    readiness: sendReady ? "send-ready" : "send-ready-blocked",
    blockerRefs: sendReady ? [] : ["blocker.wallet.send_readiness_unproven"],
    payoutTargetRefs: [],
    settlementRefs: [],
  } satisfies WalletStatusProjection
}

export function admitPayoutTarget(input: { kind: PayoutTargetKind; ref: string }) {
  const allowedPrefix: Record<PayoutTargetKind, string> = {
    bolt12_offer: "payout.bolt12",
    bolt11_invoice: "payout.bolt11",
    bip353_name: "payout.bip353",
    lnurl_pay: "payout.lnurl",
  }
  if (!input.ref.startsWith(`${allowedPrefix[input.kind]}.`)) {
    throw new Error(`${input.kind} payout target must be a public-safe ${allowedPrefix[input.kind]}.* ref`)
  }
  const projection = {
    kind: input.kind,
    payoutTargetRef: input.ref,
    readiness: "payout-target-admitted",
  }
  assertPublicProjectionSafe(projection)
  return projection
}

export async function receiveWithMdk(amountSats: number, runner: WalletCommandRunner = defaultWalletCommandRunner) {
  const result = await runner(["receive", String(amountSats)])
  if (result.exitCode !== 0) {
    return { ok: false, receiptRef: stableRef("wallet.receive_failure", result.stderr || result.stdout) }
  }
  const data = parseJson(result.stdout)
  if (typeof data?.invoice !== "string") {
    return { ok: false, receiptRef: "wallet.receive_failure.missing_invoice" }
  }
  return { ok: true, receiptRef: stableRef("wallet.receive", data.invoice) }
}

export async function sendWithMdk(
  destinationRef: string,
  amountSats: number | undefined,
  runner: WalletCommandRunner = defaultWalletCommandRunner,
) {
  assertPublicProjectionSafe({ destinationRef })
  const args = amountSats === undefined ? ["send", destinationRef] : ["send", destinationRef, String(amountSats)]
  const result = await runner(args)
  if (result.exitCode !== 0) {
    return { ok: false, receiptRef: stableRef("wallet.send_failure", result.stderr || result.stdout) }
  }
  const data = parseJson(result.stdout)
  const paymentRef = stableRef("wallet.payment", JSON.stringify(data ?? result.stdout))
  return { ok: true, receiptRef: paymentRef }
}

export async function appendLedgerEvent(paths: PylonPaths, event: Omit<LedgerEvent, "eventId" | "createdAt"> & { eventId?: string }) {
  await ensureStateDirectories(paths)
  const eventId = event.eventId ?? stableRef("ledger", `${event.kind}:${event.ref}`)
  const existing = existsSync(paths.ledger) ? await readFile(paths.ledger, "utf8") : ""
  if (existing.split("\n").some((line) => line.includes(`"eventId":"${eventId}"`) || line.includes(`"eventId": "${eventId}"`))) {
    return eventId
  }
  const record: LedgerEvent = {
    eventId,
    kind: event.kind,
    ref: event.ref,
    createdAt: new Date().toISOString(),
    data: event.data,
  }
  assertPublicProjectionSafe(record)
  await writeFile(paths.ledger, `${existing}${JSON.stringify(record)}\n`)
  return eventId
}
