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
  sendReadinessPreflight: SendReadinessPreflight
  settlementRefs: string[]
  legacySparkMigration?: LegacySparkMigrationPreflight
}

export type WalletCommandResult = {
  exitCode: number
  stdout: string
  stderr: string
}

export type WalletCommandRunner = (args: string[]) => Promise<WalletCommandResult>
export type LegacySparkCommandRunner = (args: string[]) => Promise<WalletCommandResult>

export type LegacySparkMigrationState = "not-detected" | "blocked" | "ready" | "consent-required" | "migrated"
export type LegacySparkMigrationRecoveryMode =
  | "legacy-helper-credential"
  | "local-recovery"
  | "unavailable"

export type LegacySparkMigrationPreflight = {
  schema: "openagents.pylon.legacy_spark_migration.v0.3"
  state: LegacySparkMigrationState
  dryRun: boolean
  legacyBalanceDetected: boolean
  legacySpendableBalanceSats: number | null
  unclaimedDepositCount: number | null
  helperInitReady: boolean
  legacyCredentialReady: boolean
  identityMnemonicPresent: boolean
  mnemonicRecoveryAvailable: boolean
  mnemonicBackedRecoveryReady: boolean
  recoveryMode: LegacySparkMigrationRecoveryMode
  destinationInvoiceReady: boolean
  explicitConsentRequired: boolean
  migrationRecommended: boolean
  blockerRefs: string[]
  nextActionRefs: string[]
  publicReceiptRefs: string[]
  contentRedacted: true
}

export type LegacySparkMigrationOptions = {
  destinationInvoiceReady?: boolean
  dryRun?: boolean
  env?: NodeJS.ProcessEnv
  helperRunner?: LegacySparkCommandRunner
  identityMnemonicPath?: string
  mnemonicRecoveryRequested?: boolean
  now?: () => Date
  yes?: boolean
}

export type SendReadinessPreflight = {
  schema: "openagents.pylon.send_readiness_preflight.v0.3"
  balanceKnown: boolean
  blockerRefs: string[]
  mode: "original-wallet-home" | "mnemonic-only-restore" | "unknown"
  outboundCapacityKnown: boolean
  outboundCapacityPositive: boolean
  portConfigured: boolean
  portIsolationRef: "mdk.port.configured" | "mdk.port.default_possible_crosstalk"
  sendReady: boolean
}

export type WalletNetworkOptions = {
  agentToken?: string
  baseUrl: string
  fetch?: typeof fetch
  now?: () => Date
  pylonRef: string
}

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

export const defaultLegacySparkCommandRunner: LegacySparkCommandRunner = async (args) => {
  const helper = process.env.PYLON_LEGACY_SPARK_HELPER ?? "spark-wallet-cli"
  const proc = Bun.spawn([helper, ...args], {
    stdout: "pipe",
    stderr: "pipe",
  })
  const timeout = new Promise<never>((_, reject) =>
    setTimeout(() => {
      proc.kill()
      reject(new Error("legacy Spark helper command timed out"))
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

const compactTimestamp = (now: Date) =>
  now.toISOString().replace(/\D/g, "").slice(0, 14)

const makeIdempotencyKey = (
  pylonRef: string,
  action: "wallet-readiness" | "payout-target-admission",
  now: Date,
) => `pylon-wallet:${pylonRef}:${action}:${compactTimestamp(now)}`

function parseJson(stdout: string) {
  if (!stdout.trim()) return null
  return JSON.parse(stdout) as Record<string, unknown>
}

function parseMaybeJson(stdout: string) {
  try {
    return parseJson(stdout)
  } catch {
    return null
  }
}

function envNumber(env: NodeJS.ProcessEnv, key: string) {
  const raw = env[key]
  if (raw === undefined || raw.trim() === "") return null
  const parsed = Number(raw)
  return Number.isFinite(parsed) ? parsed : null
}

function hasLegacySparkCredential(env: NodeJS.ProcessEnv) {
  return [
    env.PYLON_LEGACY_SPARK_CREDENTIAL_READY,
    env.OPENAGENTS_SPARK_API_KEY,
    env.BREEZ_API_KEY,
  ].some((value) => value !== undefined && value.trim() !== "")
}

function isMissingBreezCredential(result: WalletCommandResult) {
  return /missing\s+breez\s+api\s+key|breez\s+api\s+key.*missing|spark\s+api\s+key.*missing/i.test(
    `${result.stderr}\n${result.stdout}`,
  )
}

function safeLegacySparkMigration(
  projection: LegacySparkMigrationPreflight,
): LegacySparkMigrationPreflight {
  assertPublicProjectionSafe(projection)
  return projection
}

function buildSendReadinessPreflight(input: {
  balanceKnown: boolean
  env: NodeJS.ProcessEnv
  outboundCapacitySats: number | null
  restoredMnemonicOnly: boolean
  sendReadyClaimed: boolean
}): SendReadinessPreflight {
  const portConfigured = typeof input.env.MDK_WALLET_PORT === "string" && input.env.MDK_WALLET_PORT.trim() !== ""
  const outboundCapacityKnown = input.outboundCapacitySats !== null
  const outboundCapacityPositive = input.outboundCapacitySats !== null && input.outboundCapacitySats > 0
  const mode = input.restoredMnemonicOnly
    ? "mnemonic-only-restore"
    : portConfigured && outboundCapacityKnown
      ? "original-wallet-home"
      : "unknown"
  const blockerRefs = [
    ...(portConfigured ? [] : ["blocker.wallet.mdk_port_unset"]),
    ...(input.balanceKnown ? [] : ["blocker.wallet.balance_unknown"]),
    ...(outboundCapacityKnown ? [] : ["blocker.wallet.outbound_capacity_unknown"]),
    ...(outboundCapacityPositive ? [] : ["blocker.wallet.outbound_capacity_zero"]),
    ...(input.restoredMnemonicOnly ? ["blocker.wallet.mnemonic_only_restore_not_send_ready"] : []),
  ]
  const sendReady =
    input.sendReadyClaimed &&
    portConfigured &&
    input.balanceKnown &&
    outboundCapacityPositive &&
    !input.restoredMnemonicOnly

  return {
    schema: "openagents.pylon.send_readiness_preflight.v0.3",
    balanceKnown: input.balanceKnown,
    blockerRefs,
    mode,
    outboundCapacityKnown,
    outboundCapacityPositive,
    portConfigured,
    portIsolationRef: portConfigured
      ? "mdk.port.configured"
      : "mdk.port.default_possible_crosstalk",
    sendReady,
  }
}

async function postPylonEvent(
  options: WalletNetworkOptions,
  input: {
    action: "wallet-readiness" | "payout-target-admission"
    body: Record<string, unknown>
    path: string
  },
) {
  const agentToken = options.agentToken ?? process.env.OPENAGENTS_AGENT_TOKEN
  if (!agentToken) {
    throw new Error("OPENAGENTS_AGENT_TOKEN or --agent-token is required")
  }

  const now = options.now?.() ?? new Date()
  const response = await (options.fetch ?? fetch)(new URL(input.path, options.baseUrl), {
    body: JSON.stringify(input.body),
    headers: {
      Authorization: `Bearer ${agentToken}`,
      "Content-Type": "application/json",
      "Idempotency-Key": makeIdempotencyKey(options.pylonRef, input.action, now),
    },
    method: "POST",
  })
  const text = await response.text()
  const payload = text.trim() ? JSON.parse(text) as Record<string, unknown> : {}

  if (!response.ok) {
    throw new Error(`Pylon ${input.action} request failed (${response.status}): ${text}`)
  }

  return payload
}

export async function classifyMdkWallet(
  runner: WalletCommandRunner = defaultWalletCommandRunner,
  env: NodeJS.ProcessEnv = process.env,
) {
  const result = await runner(["balance"])
  if (result.exitCode !== 0) {
    const sendReadinessPreflight = buildSendReadinessPreflight({
      balanceKnown: false,
      env,
      outboundCapacitySats: null,
      restoredMnemonicOnly: false,
      sendReadyClaimed: false,
    })
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
      sendReadinessPreflight,
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
    const sendReadinessPreflight = buildSendReadinessPreflight({
      balanceKnown: false,
      env,
      outboundCapacitySats: typeof data?.outbound_capacity_sats === "number" ? data.outbound_capacity_sats : null,
      restoredMnemonicOnly: data?.restored_mnemonic_only === true,
      sendReadyClaimed: data?.send_ready === true,
    })
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
      sendReadinessPreflight,
      settlementRefs: [],
    } satisfies WalletStatusProjection
  }

  const sendReadinessPreflight = buildSendReadinessPreflight({
    balanceKnown: true,
    env,
    outboundCapacitySats: typeof data?.outbound_capacity_sats === "number" ? data.outbound_capacity_sats : null,
    restoredMnemonicOnly: data?.restored_mnemonic_only === true,
    sendReadyClaimed: data?.send_ready === true,
  })
  const sendReady = sendReadinessPreflight.sendReady
  const blockerRefs = sendReady
    ? []
    : [
        "blocker.wallet.send_readiness_unproven",
        ...sendReadinessPreflight.blockerRefs,
      ]
  return {
    schema: "openagents.pylon.wallet_status.v0.3",
    configured: true,
    daemonOnline: true,
    balanceSats: balance,
    receiveReady: true,
    sendReady,
    readiness: sendReady ? "send-ready" : "send-ready-blocked",
    blockerRefs: [...new Set(blockerRefs)],
    payoutTargetRefs: [],
    sendReadinessPreflight,
    settlementRefs: [],
  } satisfies WalletStatusProjection
}

export async function preflightLegacySparkMigration(
  options: LegacySparkMigrationOptions = {},
): Promise<LegacySparkMigrationPreflight> {
  const env = options.env ?? process.env
  const dryRun = options.dryRun !== false
  const hintedBalance = envNumber(env, "PYLON_LEGACY_SPARK_BALANCE_SATS")
  const hintedDeposits = envNumber(env, "PYLON_LEGACY_SPARK_UNCLAIMED_DEPOSIT_COUNT")
  const legacyHelperCredentialReady = hasLegacySparkCredential(env)
  const identityMnemonicPresent = options.identityMnemonicPath === undefined
    ? env.PYLON_LEGACY_SPARK_IDENTITY_PRESENT === "1"
    : existsSync(options.identityMnemonicPath)
  const mnemonicRecoveryAvailable = options.mnemonicRecoveryRequested === true
  const helper = options.helperRunner ?? defaultLegacySparkCommandRunner

  let helperResult: WalletCommandResult | null = null
  try {
    helperResult = await helper(["status"])
  } catch (error) {
    helperResult = {
      exitCode: 1,
      stdout: "",
      stderr: error instanceof Error ? error.message : String(error),
    }
  }

  const helperData = helperResult.exitCode === 0 ? parseMaybeJson(helperResult.stdout) : null
  const helperBalance =
    typeof helperData?.balance_sats === "number"
      ? helperData.balance_sats
      : typeof helperData?.spendable_balance_sats === "number"
        ? helperData.spendable_balance_sats
        : null
  const helperDeposits =
    typeof helperData?.unclaimed_deposit_count === "number"
      ? helperData.unclaimed_deposit_count
      : null
  const legacySpendableBalanceSats = helperBalance ?? hintedBalance
  const unclaimedDepositCount = helperDeposits ?? hintedDeposits
  const legacyBalanceDetected = legacySpendableBalanceSats !== null && legacySpendableBalanceSats > 0
  const missingBreezCredential = isMissingBreezCredential(helperResult)
  const mnemonicBackedRecoveryReady =
    mnemonicRecoveryAvailable && legacyBalanceDetected && (identityMnemonicPresent || missingBreezCredential)
  const legacyCredentialReady = legacyHelperCredentialReady || mnemonicBackedRecoveryReady
  const helperInitReady = helperResult.exitCode === 0 || mnemonicBackedRecoveryReady
  const recoveryMode: LegacySparkMigrationRecoveryMode =
    legacyHelperCredentialReady
      ? "legacy-helper-credential"
      : mnemonicBackedRecoveryReady
        ? "local-recovery"
        : "unavailable"
  const destinationInvoiceReady = options.destinationInvoiceReady === true

  const blockerRefs = [
    ...(identityMnemonicPresent || mnemonicRecoveryAvailable
      ? []
      : ["blocker.wallet.legacy_spark.identity_or_private_mnemonic_recovery_required"]),
    ...((missingBreezCredential || !legacyCredentialReady) && !mnemonicBackedRecoveryReady
      ? ["blocker.wallet.legacy_spark.breez_api_key_missing"]
      : []),
    ...(helperInitReady ? [] : ["blocker.wallet.legacy_spark.helper_init_failed"]),
    ...(legacyBalanceDetected ? [] : ["blocker.wallet.legacy_spark.no_spendable_balance_detected"]),
    ...(destinationInvoiceReady ? [] : ["blocker.wallet.legacy_spark.destination_invoice_not_ready"]),
  ]
  const ready = blockerRefs.length === 0
  const consentGiven = options.yes === true
  const state: LegacySparkMigrationState =
    !legacyBalanceDetected
      ? "not-detected"
      : !ready
        ? "blocked"
        : !consentGiven
          ? "consent-required"
          : dryRun
            ? "ready"
            : "migrated"

  return safeLegacySparkMigration({
    schema: "openagents.pylon.legacy_spark_migration.v0.3",
    state,
    dryRun,
    legacyBalanceDetected,
    legacySpendableBalanceSats,
    unclaimedDepositCount,
    helperInitReady,
    legacyCredentialReady,
    identityMnemonicPresent,
    mnemonicRecoveryAvailable,
    mnemonicBackedRecoveryReady,
    recoveryMode,
    destinationInvoiceReady,
    explicitConsentRequired: !consentGiven,
    migrationRecommended: ready && !consentGiven,
    blockerRefs,
    nextActionRefs: ready
      ? [
          ...(recoveryMode === "local-recovery"
            ? ["action.wallet.legacy_spark.review_private_local_recovery_plan"]
            : []),
          "action.wallet.legacy_spark.review_and_confirm_migrate_spark_yes",
        ]
      : [
          ...(!legacyCredentialReady || missingBreezCredential
            ? [
                identityMnemonicPresent || mnemonicRecoveryAvailable
                  ? "action.wallet.legacy_spark.rerun_with_mnemonic_recovery_local_only"
                  : "action.wallet.legacy_spark.configure_supported_local_spark_credential",
              ]
            : []),
          ...(!identityMnemonicPresent && !mnemonicRecoveryAvailable
            ? ["action.wallet.legacy_spark.use_original_identity_path_or_private_mnemonic_recovery"]
            : []),
          ...(!destinationInvoiceReady
            ? ["action.wallet.legacy_spark.prepare_mdk_destination_invoice"]
            : []),
        ],
    publicReceiptRefs: state === "migrated"
      ? [`receipt.pylon.legacy_spark_migration.${stableRef("migration", JSON.stringify({
          balance: legacySpendableBalanceSats,
          deposits: unclaimedDepositCount,
          at: options.now?.().toISOString() ?? "dry_run_time_redacted",
        })).split(".").pop()}`]
      : [],
    contentRedacted: true,
  })
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

export async function reportWalletReadiness(
  input: { status: WalletStatusProjection },
  options: WalletNetworkOptions,
) {
  const readinessRef = `readiness.public.pylon.${input.status.readiness.replace(/_/g, "-")}`
  const body = {
    balanceRefs: input.status.balanceSats === null
      ? ["balance.public.not_reported"]
      : ["balance.public.reported_redacted"],
    liquidityRefs: input.status.sendReady
      ? ["liquidity.public.send_ready_redacted"]
      : ["liquidity.public.send_readiness_unproven"],
    readinessRefs: [readinessRef, ...input.status.blockerRefs],
    status: input.status.receiveReady ? "ready" : "blocked",
    walletReady: input.status.receiveReady,
    walletRef: input.status.configured
      ? stableRef("wallet.public.mdk", options.pylonRef)
      : undefined,
  }
  assertPublicProjectionSafe(body)

  return postPylonEvent(options, {
    action: "wallet-readiness",
    body,
    path: `/api/pylons/${encodeURIComponent(options.pylonRef)}/wallet-readiness`,
  })
}

export async function requestPayoutTargetAdmission(
  input: { kind: PayoutTargetKind; ref: string },
  options: WalletNetworkOptions,
) {
  const projection = admitPayoutTarget(input)
  const body = {
    admissionRefs: ["admission.public.pylon.payout_target.requested"],
    payoutTargetRef: projection.payoutTargetRef,
    policyRefs: ["policy.public.pylon.redacted_payout_target_only"],
    status: "requested",
  }
  assertPublicProjectionSafe(body)

  return postPylonEvent(options, {
    action: "payout-target-admission",
    body,
    path: `/api/pylons/${encodeURIComponent(options.pylonRef)}/payout-target-admission`,
  })
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
