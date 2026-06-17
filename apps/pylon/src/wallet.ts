import { readFile, unlink, writeFile } from "node:fs/promises"
import { existsSync } from "node:fs"
import { createHash, randomUUID } from "node:crypto"
import { homedir } from "node:os"
import { join } from "node:path"
import type { PylonPaths } from "./state"
import { assertPublicProjectionSafe, ensureStateDirectories } from "./state"
import { toSatNumber } from "./sat-number"

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
  unifiedBalance: UnifiedWalletBalanceProjection
  legacySparkMigration?: LegacySparkMigrationPreflight
}

export type WalletCommandResult = {
  exitCode: number
  stdout: string
  stderr: string
}

// ---------------------------------------------------------------------------
// Spark backup receive (slice 1: testable, inert core).
//
// Spark is reintroduced as a backup rail when the primary MDK rail is
// offline/unavailable AND the backup is explicitly opt-in enabled. Receive and
// own-wallet reconcile paths are inert by default. General Spark spend is exposed
// only through an explicit consent command (`wallet send --rail spark
// --confirm-send`) and emits public-safe refs only.
// Public projections expose only redacted refs + blocker refs, never raw
// Spark address/invoice/preimage/mnemonic/key/path material.
// ---------------------------------------------------------------------------

export type SparkBackupReceiveState =
  | "disabled"
  | "credential-missing"
  | "helper-unavailable"
  | "address-ready"
  | "cached-address-ready"
  | "receive-selected-mdk-offline"
  | "payment-detected"
  | "claim-pending"
  | "claim-blocked"
  | "credited"
  | "sweep-to-mdk-recommended"
  | "swept-to-mdk"

export type SparkBackupReceiveProjection = {
  schema: "openagents.pylon.spark_backup_receive.v0.1"
  enabled: boolean
  state: SparkBackupReceiveState
  selectedBecauseRefs: string[]
  receiveTargetRef: string | null
  // Public-safe redacted ref for the static Lightning Address backup target
  // (`wallet.backup.lightning_address.<digest>`), present only when the
  // requested receive kind is `lightning-address` and the helper returned one.
  // The raw lightning address is local-only, never placed in any projection.
  lightningAddressRef: string | null
  rawTargetAvailableLocally: boolean
  credentialReady: boolean
  helperReady: boolean
  detectedBalanceSats: number | null
  unclaimedDepositCount: number | null
  // #5166: pending Lightning HTLCs (offline-received funds awaiting
  // `backup-claim`). Read-only — lets an operator see incoming funds before
  // claiming. Optional for back-compat with older projections.
  claimableHtlcCount?: number | null
  claimableHtlcSats?: number | null
  blockerRefs: string[]
  nextActionRefs: string[]
  publicReceiptRefs: string[]
  contentRedacted: true
}

/**
 * Narrow, injectable Spark helper command contract.
 *
 * The live Breez SDK Spark integration is NOT wired in this slice. A helper is
 * an injected dependency that responds to exactly these four commands. A stub
 * (`unavailableSparkBackupHelper`) reports `helper-unavailable` when none is
 * configured; tests inject a fake helper.
 */
export type SparkBackupCommand = "status" | "address" | "history" | "unclaimed-deposits" | "lightning-address" | "claim"

export type SparkBackupHelper = (command: SparkBackupCommand) => Promise<WalletCommandResult>

/**
 * Default stub helper. There is no live Breez SDK in slice 1, so every command
 * reports the helper is unavailable. This keeps default node behavior inert.
 */
export const unavailableSparkBackupHelper: SparkBackupHelper = async (command) => ({
  exitCode: 1,
  stdout: "",
  stderr: `spark backup helper unavailable: ${command}`,
})

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
  guidedRecovery: LegacySparkMigrationGuidedRecovery
  nextActionRefs: string[]
  publicReceiptRefs: string[]
  contentRedacted: true
}

export type LegacySparkMigrationGuidedRecovery = {
  schema: "openagents.pylon.legacy_spark_guided_recovery.v0.3"
  userFacingAnswer: string
  localRecoveryAvailable: boolean
  localRecoverySelected: boolean
  destinationState: "ready" | "not-ready"
  consentState: "required" | "accepted"
  nextStepSummary: string
  secretHandlingRefs: string[]
}

export type LegacySparkMigrationOptions = {
  destinationInvoiceReady?: boolean
  dryRun?: boolean
  // When true, an owner-authorized embedded Breez/Spark service key is available
  // even if no env credential is set. This counts as a valid credential so the
  // legacy migration never dead-ends on `breez_api_key_missing` (#5085). The key
  // is a service key with no spend authority and is NEVER passed through here.
  embeddedCredentialAvailable?: boolean
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

export type UnifiedWalletBalanceProjection = {
  schema: "openagents.pylon.unified_wallet_balance.v0.1" | "openagents.pylon.unified_wallet_balance.v0.2"
  primaryRail?: "mdk" | "spark"
  primaryBalanceSats?: number | null
  primarySpendableSats?: number | null
  mdkSpendableSats: number | null
  sparkBackupCreditedSats: number | null
  sparkBackupClaimableSats: number | null
  sparkBackupPendingSweepSats: number | null
  totalVisibleSats: number | null
  sourceRefs: string[]
  caveatRefs: string[]
  nextActionRefs: string[]
  contentRedacted: true
}

export type WalletNetworkOptions = {
  agentToken?: string
  baseUrl: string
  fetch?: typeof fetch
  now?: () => Date
  pylonRef: string
}

export type MdkDaemonPidfileReclaimResult = {
  pid: number | null
  pidfilePath: string
  reason: "absent" | "dead_pid" | "invalid_json" | "invalid_pid" | "live_pid" | "unlink_failed"
  reclaimed: boolean
}

export type LedgerEvent = {
  eventId: string
  kind: string
  ref: string
  createdAt: string
  data: Record<string, unknown>
}

export const defaultWalletCommandRunner: WalletCommandRunner = async (args) => {
  await reclaimStaleMdkDaemonPidfile()
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

export async function reclaimStaleMdkDaemonPidfile(options: {
  homeDir?: string
  isProcessLive?: (pid: number) => boolean
} = {}): Promise<MdkDaemonPidfileReclaimResult> {
  const pidfilePath = join(options.homeDir ?? process.env.HOME ?? homedir(), ".mdk-wallet", "daemon.pid")

  if (!existsSync(pidfilePath)) {
    return { pid: null, pidfilePath, reason: "absent", reclaimed: false }
  }

  let pid: number | null = null
  let reason: MdkDaemonPidfileReclaimResult["reason"] = "invalid_json"

  try {
    const parsed = JSON.parse(await readFile(pidfilePath, "utf8")) as { pid?: unknown }
    if (typeof parsed.pid === "number" && Number.isInteger(parsed.pid) && parsed.pid > 0) {
      pid = parsed.pid
      const isLive = options.isProcessLive ?? ((value: number) => {
        try {
          process.kill(value, 0)
          return true
        } catch (error) {
          const code = (error as NodeJS.ErrnoException).code
          return code === "EPERM"
        }
      })

      if (isLive(pid)) {
        return { pid, pidfilePath, reason: "live_pid", reclaimed: false }
      }

      reason = "dead_pid"
    } else {
      reason = "invalid_pid"
    }
  } catch {
    reason = "invalid_json"
  }

  try {
    await unlink(pidfilePath)
    return { pid, pidfilePath, reason, reclaimed: true }
  } catch {
    return { pid, pidfilePath, reason: "unlink_failed", reclaimed: false }
  }
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

const sleep = (ms: number) =>
  new Promise<void>((resolve) => setTimeout(resolve, ms))

function envNumber(env: NodeJS.ProcessEnv, key: string) {
  const raw = env[key]
  if (raw === undefined || raw.trim() === "") return null
  const parsed = Number(raw)
  return Number.isFinite(parsed) ? parsed : null
}

function hasLegacySparkCredential(env: NodeJS.ProcessEnv, embeddedCredentialAvailable = false) {
  // The embedded owner-authorized default key counts as a valid credential
  // (#5085): with it present, legacy migration must NOT raise
  // `breez_api_key_missing` even when no env key is set.
  if (embeddedCredentialAvailable) return true
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

function buildLegacySparkGuidedRecovery(input: {
  consentGiven: boolean
  destinationInvoiceReady: boolean
  identityMnemonicPresent: boolean
  legacyBalanceDetected: boolean
  mnemonicBackedRecoveryReady: boolean
  mnemonicRecoveryAvailable: boolean
  missingBreezCredential: boolean
  state: LegacySparkMigrationState
}): LegacySparkMigrationGuidedRecovery {
  const localRecoveryAvailable =
    input.legacyBalanceDetected &&
    (input.identityMnemonicPresent || input.mnemonicRecoveryAvailable || input.missingBreezCredential)
  const localRecoverySelected = input.mnemonicBackedRecoveryReady
  const userFacingAnswer = input.missingBreezCredential
    ? "No manual Breez credential is expected for normal recovery. Use the local recovery flow with the 12-word recovery phrase on this machine, then prepare the new wallet destination and consent before funds move."
    : "Use the local migration preflight first. Pylon will show destination readiness and require explicit consent before funds move."
  const nextStepSummary =
    input.state === "migrated"
      ? "Migration completed with public-safe receipt refs only."
      : !input.legacyBalanceDetected
        ? "No spendable old Spark balance was detected."
        : !input.destinationInvoiceReady
          ? "Prepare the new wallet destination, then rerun the migration preflight."
          : !localRecoverySelected && input.missingBreezCredential
            ? "Rerun with local recovery selected so the old helper does not block on a missing Breez credential."
            : !input.consentGiven
              ? "Review the local recovery plan, then rerun with explicit consent to execute."
              : "Execute only after the destination is ready and the local recovery plan has been reviewed."

  return {
    schema: "openagents.pylon.legacy_spark_guided_recovery.v0.3",
    userFacingAnswer,
    localRecoveryAvailable,
    localRecoverySelected,
    destinationState: input.destinationInvoiceReady ? "ready" : "not-ready",
    consentState: input.consentGiven ? "accepted" : "required",
    nextStepSummary,
    secretHandlingRefs: [
      "policy.wallet.legacy_spark.local_only_recovery_phrase",
      "policy.wallet.legacy_spark.no_support_channel_secrets",
      "policy.wallet.legacy_spark.public_receipts_only",
    ],
  }
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

const nonNegativeSats = (value: number | null | undefined): number | null =>
  typeof value === "number" && Number.isFinite(value) && value >= 0
    ? Math.floor(value)
    : null

const sumKnownSats = (values: ReadonlyArray<number | null>): number | null => {
  const known = values.filter((value): value is number => value !== null)
  return known.length === 0
    ? null
    : known.reduce((total, value) => total + value, 0)
}

export function unifiedWalletBalanceFromStatus(
  status: Pick<WalletStatusProjection, "balanceSats" | "sendReady">,
  sparkBackup?: Pick<
    SparkBackupReceiveProjection,
    "claimableHtlcSats" | "detectedBalanceSats" | "nextActionRefs" | "state"
  >,
): UnifiedWalletBalanceProjection {
  const mdkSpendableSats = nonNegativeSats(status.balanceSats)
  const sparkBackupCreditedSats = nonNegativeSats(
    sparkBackup?.detectedBalanceSats,
  )
  const sparkBackupClaimableSats = nonNegativeSats(
    sparkBackup?.claimableHtlcSats,
  )
  const sparkBackupPendingSweepSats =
    sparkBackupCreditedSats !== null && sparkBackupCreditedSats > 0
      ? sparkBackupCreditedSats
      : 0
  const hasSparkBalance =
    (sparkBackupCreditedSats ?? 0) > 0 ||
    (sparkBackupClaimableSats ?? 0) > 0 ||
    sparkBackup?.state === "sweep-to-mdk-recommended"

  return {
    schema: "openagents.pylon.unified_wallet_balance.v0.1",
    primaryRail: "mdk",
    primaryBalanceSats: mdkSpendableSats,
    primarySpendableSats: status.sendReady ? mdkSpendableSats : null,
    mdkSpendableSats,
    sparkBackupCreditedSats,
    sparkBackupClaimableSats,
    sparkBackupPendingSweepSats,
    totalVisibleSats: sumKnownSats([
      mdkSpendableSats,
      sparkBackupCreditedSats,
      sparkBackupClaimableSats,
    ]),
    sourceRefs: [
      "source.wallet.mdk_agent_wallet.balance",
      ...(sparkBackup === undefined
        ? []
        : ["source.wallet.spark_backup.status"]),
    ],
    caveatRefs: [
      "caveat.wallet.total_visible_is_not_single_spendable_balance",
      "caveat.wallet.spark_backup_is_not_mdk_spendable_until_sweep_receipt",
      ...(status.sendReady
        ? []
        : ["caveat.wallet.mdk_send_readiness_tracked_separately"]),
    ],
    nextActionRefs: hasSparkBalance
      ? [
          "action.wallet.spark_backup.review_consent_sweep",
          ...(sparkBackup?.nextActionRefs ?? []),
        ]
      : [],
    contentRedacted: true,
  }
}

export function withUnifiedWalletBalance(
  status: WalletStatusProjection,
  sparkBackup?: SparkBackupReceiveProjection,
): WalletStatusProjection {
  return {
    ...status,
    unifiedBalance: unifiedWalletBalanceFromStatus(status, sparkBackup),
  }
}

export function sparkPrimaryWalletBalanceFromStatus(
  status: Pick<WalletStatusProjection, "balanceSats" | "sendReady">,
  sparkBackup?: Pick<
    SparkBackupReceiveProjection,
    "claimableHtlcSats" | "detectedBalanceSats" | "nextActionRefs" | "state"
  >,
): UnifiedWalletBalanceProjection {
  const sparkCreditedSats = nonNegativeSats(sparkBackup?.detectedBalanceSats)
  const sparkClaimableSats = nonNegativeSats(sparkBackup?.claimableHtlcSats)
  const hasClaimable = (sparkClaimableSats ?? 0) > 0
  return {
    schema: "openagents.pylon.unified_wallet_balance.v0.2",
    primaryRail: "spark",
    primaryBalanceSats: sparkCreditedSats,
    primarySpendableSats: sparkCreditedSats,
    // MDK is now a treasury/checkout rail, not part of the agent-facing
    // balance. Keep the local MDK amount out of the public status shape.
    mdkSpendableSats: null,
    sparkBackupCreditedSats: sparkCreditedSats,
    sparkBackupClaimableSats: sparkClaimableSats,
    sparkBackupPendingSweepSats: 0,
    totalVisibleSats: sparkCreditedSats,
    sourceRefs: [
      "source.wallet.spark_primary.balance",
      ...(sparkBackup === undefined
        ? []
        : ["source.wallet.spark_backup.status"]),
      ...(status.balanceSats === null
        ? []
        : ["source.wallet.mdk_agent_wallet.excluded_from_agent_primary_balance"]),
    ],
    caveatRefs: [
      "caveat.wallet.mdk_excluded_from_agent_primary_balance",
      ...(hasClaimable
        ? ["caveat.wallet.spark_claimable_htlcs_require_backup_claim"]
        : []),
    ],
    nextActionRefs: [
      ...(hasClaimable ? ["action.wallet.spark_backup.run_backup_claim"] : []),
      ...(sparkBackup?.nextActionRefs ?? []),
    ],
    contentRedacted: true,
  }
}

export function withSparkPrimaryWalletBalance(
  status: WalletStatusProjection,
  sparkBackup?: SparkBackupReceiveProjection,
): WalletStatusProjection {
  const sparkBalanceSats = nonNegativeSats(sparkBackup?.detectedBalanceSats)
  const sparkClaimableSats = nonNegativeSats(sparkBackup?.claimableHtlcSats)
  const enabled = sparkBackup?.enabled === true
  const credentialReady = sparkBackup?.credentialReady === true
  const helperReady = sparkBackup?.helperReady === true
  const receiveTargetReady =
    sparkBackup !== undefined &&
    (
      sparkBackup.receiveTargetRef !== null ||
      sparkBackup.lightningAddressRef !== null ||
      sparkBackup.state === "address-ready" ||
      sparkBackup.state === "cached-address-ready" ||
      sparkBackup.state === "credited"
    )
  const configured = enabled && credentialReady
  const daemonOnline = configured && helperReady
  const receiveReady = daemonOnline && receiveTargetReady
  const sendReady = receiveReady && sparkBalanceSats !== null && sparkBalanceSats > 0
  const blockerRefs = [
    ...(!enabled ? ["blocker.wallet.spark_primary.disabled"] : []),
    ...(enabled && !credentialReady ? ["blocker.wallet.spark_primary.credential_missing"] : []),
    ...(configured && !helperReady ? ["blocker.wallet.spark_primary.helper_unavailable"] : []),
    ...(receiveReady && sparkBalanceSats === null ? ["blocker.wallet.spark_primary.balance_unknown"] : []),
    ...(receiveReady && sparkBalanceSats === 0 ? ["blocker.wallet.spark_primary.balance_empty"] : []),
    ...((sparkClaimableSats ?? 0) > 0 ? ["blocker.wallet.spark_primary.claim_pending"] : []),
    ...(sparkBackup?.blockerRefs ?? []),
  ]
  const readiness: WalletReadiness =
    !configured || !daemonOnline
      ? "daemon-offline"
      : sparkBalanceSats === null
        ? "balance-unknown"
        : sendReady
          ? "send-ready"
          : "receive-ready"

  return {
    ...status,
    configured,
    daemonOnline,
    balanceSats: sparkBalanceSats,
    receiveReady,
    sendReady,
    readiness,
    blockerRefs: [...new Set(blockerRefs)],
    unifiedBalance: sparkPrimaryWalletBalanceFromStatus(status, sparkBackup),
  }
}

export function mdkScopedAgentWalletStatus(): WalletStatusProjection {
  return {
    schema: "openagents.pylon.wallet_status.v0.3",
    configured: false,
    daemonOnline: false,
    balanceSats: null,
    receiveReady: false,
    sendReady: false,
    readiness: "daemon-offline",
    blockerRefs: ["blocker.wallet.spark_primary.status_unread"],
    payoutTargetRefs: [],
    sendReadinessPreflight: {
      schema: "openagents.pylon.send_readiness_preflight.v0.3",
      balanceKnown: false,
      blockerRefs: ["blocker.wallet.mdk_scoped_to_checkouts_treasury"],
      mode: "unknown",
      outboundCapacityKnown: false,
      outboundCapacityPositive: false,
      portConfigured: false,
      portIsolationRef: "mdk.port.default_possible_crosstalk",
      sendReady: false,
    },
    settlementRefs: [],
    unifiedBalance: sparkPrimaryWalletBalanceFromStatus({
      balanceSats: null,
      sendReady: false,
    }),
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
      unifiedBalance: unifiedWalletBalanceFromStatus({
        balanceSats: null,
        sendReady: false,
      }),
    } satisfies WalletStatusProjection
  }

  const data = parseJson(result.stdout)
  // #5166: amounts may arrive as number | bigint | decimal string; normalize.
  const balance =
    toSatNumber(data?.balance_sats) ??
    toSatNumber(data?.balance) ??
    toSatNumber(data?.confirmed)
  if (balance === null) {
    const sendReadinessPreflight = buildSendReadinessPreflight({
      balanceKnown: false,
      env,
      outboundCapacitySats: toSatNumber(data?.outbound_capacity_sats),
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
      unifiedBalance: unifiedWalletBalanceFromStatus({
        balanceSats: null,
        sendReady: false,
      }),
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
    unifiedBalance: unifiedWalletBalanceFromStatus({
      balanceSats: balance,
      sendReady,
    }),
  } satisfies WalletStatusProjection
}

export async function preflightLegacySparkMigration(
  options: LegacySparkMigrationOptions = {},
): Promise<LegacySparkMigrationPreflight> {
  const env = options.env ?? process.env
  const dryRun = options.dryRun !== false
  const hintedBalance = envNumber(env, "PYLON_LEGACY_SPARK_BALANCE_SATS")
  const hintedDeposits = envNumber(env, "PYLON_LEGACY_SPARK_UNCLAIMED_DEPOSIT_COUNT")
  const legacyHelperCredentialReady = hasLegacySparkCredential(
    env,
    options.embeddedCredentialAvailable === true,
  )
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
    toSatNumber(helperData?.balance_sats) ?? toSatNumber(helperData?.spendable_balance_sats)
  const helperDeposits = toSatNumber(helperData?.unclaimed_deposit_count)
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
    guidedRecovery: buildLegacySparkGuidedRecovery({
      consentGiven,
      destinationInvoiceReady,
      identityMnemonicPresent,
      legacyBalanceDetected,
      mnemonicBackedRecoveryReady,
      mnemonicRecoveryAvailable,
      missingBreezCredential,
      state,
    }),
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
  input: {
    status: WalletStatusProjection
    // #5166: optional, secret-free Spark selftest encoded into readinessRefs so
    // the platform can collect — fleet-wide — which gate makes the
    // offline-receive helper unavailable on each node. Booleans + an enum
    // source only; never a path, seed, address, or error string.
    sparkSelftest?: {
      isCompiledBinary: boolean
      enabled: boolean
      identitySource: string
      seedPresent: boolean
      moduleLoaded: boolean
    }
  },
  options: WalletNetworkOptions,
) {
  const primaryRail = input.status.unifiedBalance.primaryRail ?? "mdk"
  const readinessRef = `readiness.public.pylon.${input.status.readiness.replace(/_/g, "-")}`
  const selftestRefs = input.sparkSelftest
    ? [
        `selftest.spark.compiled_binary.${input.sparkSelftest.isCompiledBinary}`,
        `selftest.spark.enabled.${input.sparkSelftest.enabled}`,
        `selftest.spark.module_loaded.${input.sparkSelftest.moduleLoaded}`,
        `selftest.spark.seed_present.${input.sparkSelftest.seedPresent}`,
        `selftest.spark.identity_source.${input.sparkSelftest.identitySource}`,
      ]
    : []
  const body = {
    balanceRefs: input.status.balanceSats === null
      ? ["balance.public.not_reported"]
      : primaryRail === "spark"
        ? ["balance.public.spark.reported_redacted"]
        : ["balance.public.reported_redacted"],
    liquidityRefs: input.status.sendReady
      ? [
          primaryRail === "spark"
            ? "liquidity.public.spark_send_ready_redacted"
            : "liquidity.public.send_ready_redacted",
        ]
      : [
          primaryRail === "spark"
            ? "liquidity.public.spark_send_readiness_unproven"
            : "liquidity.public.send_readiness_unproven",
        ],
    readinessRefs: [readinessRef, ...input.status.blockerRefs, ...selftestRefs],
    status: input.status.receiveReady ? "ready" : "blocked",
    walletReady: input.status.receiveReady,
    walletRef: input.status.configured
      ? stableRef(`wallet.public.${primaryRail}`, options.pylonRef)
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

async function createMdkReceiveTarget(
  amountSats: number,
  runner: WalletCommandRunner = defaultWalletCommandRunner,
): Promise<
  | {
      ok: true
      target: string
      targetRef: string
    }
  | {
      ok: false
      failureRef: string
    }
> {
  const result = await runner(["receive", String(amountSats)])
  if (result.exitCode !== 0) {
    return { ok: false, failureRef: stableRef("wallet.mdk_receive_request_failure", result.stderr || result.stdout) }
  }
  const data = parseJson(result.stdout)
  if (typeof data?.invoice !== "string") {
    return { ok: false, failureRef: "wallet.mdk_receive_request_failure.missing_target" }
  }
  return {
    ok: true,
    target: data.invoice,
    targetRef: stableRef("wallet.mdk_receive_target", data.invoice),
  }
}

export async function receiveWithMdk(amountSats: number, runner: WalletCommandRunner = defaultWalletCommandRunner) {
  const target = await createMdkReceiveTarget(amountSats, runner)
  if (!target.ok) {
    return { ok: false, receiptRef: target.failureRef }
  }
  return { ok: true, receiptRef: target.targetRef }
}

/**
 * Classify an MDK receive failure as either an offline/unavailable class
 * (eligible for the Spark backup rail) or a validation/user-error class
 * (NOT eligible — stay on MDK).
 *
 * Offline class: daemon offline, connection refused/unreachable, init/start
 * timeouts, daemon-not-ready. Validation/user class: bad amount, malformed
 * arguments, policy rejections, anything that is not a transport/availability
 * problem. When uncertain, default to NO fallback (validation class) so we
 * never silently switch rails on a user error.
 */
export function classifyMdkReceiveFailure(result: WalletCommandResult): {
  class: "offline" | "validation"
  ref: string
} {
  const text = `${result.stderr}\n${result.stdout}`.toLowerCase()
  const offline =
    /daemon[\s_-]*(unavailable|offline|not[\s_-]*ready|unreachable)/.test(text) ||
    /\b(offline|unreachable)\b/.test(text) ||
    /connection\s+(refused|reset|failed|error)/.test(text) ||
    /econnrefused|enotfound|etimedout|ehostunreach|enetunreach/.test(text) ||
    /(init|initiali[sz]e|initiali[sz]ation|start|startup|connect|connection)\b.*\b(timed?\s*out|timeout)/.test(text) ||
    /\b(timed?\s*out|timeout)\b.*\b(init|initiali[sz]|start|startup|connect|connection|daemon)/.test(text) ||
    /could\s+not\s+connect|unable\s+to\s+connect|no\s+such\s+host|service\s+unavailable/.test(text)
  return {
    class: offline ? "offline" : "validation",
    ref: stableRef(offline ? "wallet.receive_failure.offline" : "wallet.receive_failure.validation", result.stderr || result.stdout || "unknown"),
  }
}

/**
 * Public-projection guard specialized for Spark backup. Runs the shared
 * `assertPublicProjectionSafe` (which now rejects raw Spark address/invoice
 * material via the state.ts forbidden patterns) and additionally rejects raw
 * Spark-shaped strings anywhere in the projection. Redacted refs such as
 * `wallet.backup.spark_address.<digest>` and `blocker.wallet.spark_backup.*`
 * remain allowed.
 */
function assertSparkBackupProjectionSafe<T>(projection: T): T {
  assertPublicProjectionSafe(projection)
  return projection
}

export type SparkBackupReceiveKind = "spark-address" | "lightning-address"

export type SparkBackupReceiveOptions = {
  enabled?: boolean
  env?: NodeJS.ProcessEnv
  helper?: SparkBackupHelper
  cachedAddress?: string | null
  showLocalTarget?: boolean
  // Which receive target to resolve. Defaults to `spark-address`. With
  // `lightning-address` we resolve the wallet's static Lightning Address
  // (LNURL-pay), so an online MDK treasury can pay an offline recipient.
  kind?: SparkBackupReceiveKind
  // The embedded owner-authorized default Breez key counts as a valid
  // credential (#5078), matching the helper resolver and the legacy-migration
  // path, so the receive backup works out-of-box once opt-in is enabled — no
  // manual env key required. Inert-by-default stays enforced by the
  // PYLON_SPARK_BACKUP_ENABLED flag, not by key presence.
  embeddedCredentialAvailable?: boolean
}

function hasSparkBackupCredential(env: NodeJS.ProcessEnv, embeddedCredentialAvailable = false) {
  // The embedded owner-authorized default key counts as a valid credential
  // (#5085) so a consented legacy-Spark sweep is not blocked on a missing env
  // key. Inert-by-default is still enforced by the opt-in/consent gates above.
  if (embeddedCredentialAvailable) return true
  return [
    env.PYLON_SPARK_BACKUP_CREDENTIAL_READY,
    env.OPENAGENTS_SPARK_API_KEY,
    env.BREEZ_API_KEY,
  ].some((value) => value !== undefined && value.trim() !== "")
}

function isSparkBackupEnabled(options: SparkBackupReceiveOptions, env: NodeJS.ProcessEnv) {
  if (options.enabled !== undefined) return options.enabled
  const raw = env.PYLON_SPARK_BACKUP_ENABLED
  return raw === "1" || raw === "true"
}

/**
 * Build a Spark backup receive projection. INERT by default:
 * - opt-in disabled -> `disabled` (no helper calls).
 * - missing credential -> `credential-missing` blocker.
 * - helper offline with a cached address -> `cached-address-ready` (no fresh
 *   sync claim).
 * - helper offline, no cache -> `helper-unavailable` blocker.
 * - helper ready -> `address-ready` with a redacted receive-target ref and
 *   `rawTargetAvailableLocally: true`.
 *
 * The raw Spark target is NEVER placed in the returned projection. It is only
 * surfaced separately (see `prepareSparkBackupReceive`) when `--show-local-target`
 * is explicitly set, for local-only output.
 */
export async function classifySparkBackupReceive(
  options: SparkBackupReceiveOptions = {},
): Promise<SparkBackupReceiveProjection> {
  const env = options.env ?? process.env
  const enabled = isSparkBackupEnabled(options, env)
  const helper = options.helper ?? unavailableSparkBackupHelper
  const cachedAddress = options.cachedAddress ?? null

  const kind: SparkBackupReceiveKind = options.kind ?? "spark-address"

  const base: SparkBackupReceiveProjection = {
    schema: "openagents.pylon.spark_backup_receive.v0.1",
    enabled,
    state: "disabled",
    selectedBecauseRefs: [],
    receiveTargetRef: null,
    lightningAddressRef: null,
    rawTargetAvailableLocally: false,
    credentialReady: false,
    helperReady: false,
    detectedBalanceSats: null,
    unclaimedDepositCount: null,
    blockerRefs: [],
    nextActionRefs: [],
    publicReceiptRefs: [],
    contentRedacted: true,
  }

  if (!enabled) {
    return assertSparkBackupProjectionSafe({
      ...base,
      state: "disabled",
      nextActionRefs: ["action.wallet.spark_backup.enable_opt_in"],
    })
  }

  const credentialReady = hasSparkBackupCredential(
    env,
    options.embeddedCredentialAvailable === true,
  )
  if (!credentialReady) {
    return assertSparkBackupProjectionSafe({
      ...base,
      state: "credential-missing",
      credentialReady: false,
      blockerRefs: ["blocker.wallet.spark_backup.credential_missing"],
      nextActionRefs: ["action.wallet.spark_backup.configure_local_credential"],
    })
  }

  let helperResult: WalletCommandResult
  try {
    helperResult = await helper(kind === "lightning-address" ? "lightning-address" : "address")
  } catch (error) {
    helperResult = { exitCode: 1, stdout: "", stderr: error instanceof Error ? error.message : String(error) }
  }
  const helperReady = helperResult.exitCode === 0
  const helperData = helperReady ? parseMaybeJson(helperResult.stdout) : null
  const rawTarget =
    kind === "lightning-address"
      ? typeof helperData?.lightning_address === "string"
        ? helperData.lightning_address
        : null
      : typeof helperData?.spark_address === "string"
        ? helperData.spark_address
        : typeof helperData?.address === "string"
          ? helperData.address
          : typeof helperData?.spark_invoice === "string"
            ? helperData.spark_invoice
            : null

  if (helperReady && rawTarget) {
    if (kind === "lightning-address") {
      return assertSparkBackupProjectionSafe({
        ...base,
        state: "address-ready",
        credentialReady: true,
        helperReady: true,
        lightningAddressRef: stableRef("wallet.backup.lightning_address", rawTarget),
        rawTargetAvailableLocally: true,
        nextActionRefs: ["action.wallet.spark_backup.backup_status"],
      })
    }
    return assertSparkBackupProjectionSafe({
      ...base,
      state: "address-ready",
      credentialReady: true,
      helperReady: true,
      receiveTargetRef: stableRef("wallet.backup.spark_address", rawTarget),
      rawTargetAvailableLocally: true,
      nextActionRefs: ["action.wallet.spark_backup.backup_status"],
    })
  }

  if (kind !== "lightning-address" && cachedAddress) {
    return assertSparkBackupProjectionSafe({
      ...base,
      state: "cached-address-ready",
      credentialReady: true,
      helperReady: false,
      receiveTargetRef: stableRef("wallet.backup.spark_address", cachedAddress),
      rawTargetAvailableLocally: true,
      blockerRefs: ["blocker.wallet.spark_backup.sync_unavailable"],
      nextActionRefs: ["action.wallet.spark_backup.reconnect_helper_to_sync"],
    })
  }

  return assertSparkBackupProjectionSafe({
    ...base,
    state: "helper-unavailable",
    credentialReady: true,
    helperReady: false,
    blockerRefs: ["blocker.wallet.spark_backup.helper_unavailable"],
    nextActionRefs: ["action.wallet.spark_backup.install_or_start_helper"],
  })
}

export type SparkBackupReceiveResult = {
  ok: boolean
  rail: "spark_backup"
  state: SparkBackupReceiveState
  receiptRef: string | null
  rawTargetAvailableLocally: boolean
  // Raw local target is only present when `--show-local-target` is explicitly
  // set, for local terminal/TUI output. It is NEVER placed in any projection
  // or network post.
  localTarget?: string
  projection: SparkBackupReceiveProjection
  blockerRefs: string[]
}

/**
 * Prepare a Spark backup receive target. Returns a redacted receipt ref and,
 * only when `showLocalTarget` is explicitly set, the raw local target for
 * local-only display. Without `showLocalTarget`, raw target output is withheld.
 */
export async function prepareSparkBackupReceive(
  options: SparkBackupReceiveOptions = {},
): Promise<SparkBackupReceiveResult> {
  const helper = options.helper ?? unavailableSparkBackupHelper
  const cachedAddress = options.cachedAddress ?? null
  const kind: SparkBackupReceiveKind = options.kind ?? "spark-address"
  const projection = await classifySparkBackupReceive(options)

  const ready = projection.state === "address-ready" || projection.state === "cached-address-ready"
  // The redacted receipt ref derives from whichever target the requested kind
  // resolved (lightning address ref for `lightning-address`, spark address ref
  // otherwise).
  const targetRef = kind === "lightning-address" ? projection.lightningAddressRef : projection.receiveTargetRef
  const receiptRef = targetRef
    ? `wallet.backup_receive.${targetRef.split(".").pop()}`
    : null

  // Resolve the raw local target only when explicitly requested. For the
  // cached path we already hold it; for the live path we re-read from the
  // helper (kept out of the projection).
  let localTarget: string | undefined
  if (ready && options.showLocalTarget === true) {
    if (kind !== "lightning-address" && projection.state === "cached-address-ready" && cachedAddress) {
      localTarget = cachedAddress
    } else if (projection.state === "address-ready") {
      try {
        if (kind === "lightning-address") {
          const result = await helper("lightning-address")
          const data = result.exitCode === 0 ? parseMaybeJson(result.stdout) : null
          localTarget = typeof data?.lightning_address === "string" ? data.lightning_address : undefined
        } else {
          const result = await helper("address")
          const data = result.exitCode === 0 ? parseMaybeJson(result.stdout) : null
          localTarget =
            typeof data?.spark_address === "string"
              ? data.spark_address
              : typeof data?.address === "string"
                ? data.address
                : typeof data?.spark_invoice === "string"
                  ? data.spark_invoice
                  : undefined
        }
      } catch {
        localTarget = undefined
      }
    }
  }

  return {
    ok: ready,
    rail: "spark_backup",
    state: projection.state,
    receiptRef,
    rawTargetAvailableLocally: projection.rawTargetAvailableLocally,
    ...(localTarget !== undefined ? { localTarget } : {}),
    projection,
    blockerRefs: projection.blockerRefs,
  }
}

export type WalletReceiveFallbackResult =
  | { ok: true; rail: "mdk"; receiptRef: string }
  | {
      ok: boolean
      rail: "spark_backup"
      receiptRef: string | null
      mdkFailureRef: string
      rawTargetAvailableLocally: boolean
      sparkBackup: SparkBackupReceiveResult
    }
  | { ok: false; rail: "mdk"; receiptRef: string; mdkFailureClass: "offline" | "validation" }

/**
 * `wallet receive` fallback chooser: MDK first, Spark backup second.
 *
 * Spark backup is consulted ONLY when:
 *   1. the MDK failure is in the offline/unavailable class, AND
 *   2. the Spark backup is opt-in enabled.
 *
 * On an MDK validation/user error, the rail does NOT switch (stays on MDK).
 * Default behavior is unchanged/inert: with the opt-in off and the stub helper,
 * a non-offline failure returns the MDK failure ref and an offline failure with
 * the backup disabled also stays on MDK.
 */
export async function receiveWithFallback(
  amountSats: number,
  options: {
    runner?: WalletCommandRunner
    sparkBackup?: SparkBackupReceiveOptions
  } = {},
): Promise<WalletReceiveFallbackResult> {
  const runner = options.runner ?? defaultWalletCommandRunner
  const result = await runner(["receive", String(amountSats)])
  if (result.exitCode === 0) {
    const data = parseJson(result.stdout)
    if (typeof data?.invoice === "string") {
      return { ok: true, rail: "mdk", receiptRef: stableRef("wallet.receive", data.invoice) }
    }
    return { ok: false, rail: "mdk", receiptRef: "wallet.receive_failure.missing_invoice", mdkFailureClass: "validation" }
  }

  const failure = classifyMdkReceiveFailure(result)
  const env = options.sparkBackup?.env ?? process.env
  const enabled = isSparkBackupEnabled(options.sparkBackup ?? {}, env)

  // Validation/user error, or backup disabled -> do NOT switch rails.
  if (failure.class !== "offline" || !enabled) {
    return { ok: false, rail: "mdk", receiptRef: failure.ref, mdkFailureClass: failure.class }
  }

  const sparkBackup = await prepareSparkBackupReceive(options.sparkBackup ?? {})
  // Annotate why the backup rail was selected, without leaking raw material.
  sparkBackup.projection.selectedBecauseRefs = [failure.ref]
  if (sparkBackup.ok && sparkBackup.projection.state === "address-ready") {
    sparkBackup.projection.state = "receive-selected-mdk-offline"
  }

  return {
    ok: sparkBackup.ok,
    rail: "spark_backup",
    receiptRef: sparkBackup.receiptRef,
    mdkFailureRef: failure.ref,
    rawTargetAvailableLocally: sparkBackup.rawTargetAvailableLocally,
    sparkBackup,
  }
}

/**
 * Reconciliation helper for `backup-status`: given a detected Spark balance,
 * recommend the next local reconcile action but NEVER mark settlement. Pending
 * HTLCs must be claimed before they can become credited Spark backup balance;
 * credited funds move only through the consented `migrate-spark` sweep.
 */
export function recommendSparkSweep(input: {
  claimableHtlcCount?: number | null
  claimableHtlcSats?: number | null
  detectedBalanceSats: number | null
  unclaimedDepositCount?: number | null
}): {
  state: SparkBackupReceiveState
  recommendsMigrateSpark: boolean
  settlementMarked: false
  nextActionRefs: string[]
} {
  const credited = input.detectedBalanceSats !== null && input.detectedBalanceSats > 0
  const claimable =
    (input.claimableHtlcCount !== null && input.claimableHtlcCount !== undefined && input.claimableHtlcCount > 0) ||
    (input.claimableHtlcSats !== null && input.claimableHtlcSats !== undefined && input.claimableHtlcSats > 0)
  return {
    state: claimable ? "claim-pending" : credited ? "sweep-to-mdk-recommended" : "credited",
    recommendsMigrateSpark: credited,
    settlementMarked: false,
    nextActionRefs: [
      ...(claimable ? ["action.wallet.spark_backup.run_backup_claim"] : []),
      ...(credited ? ["action.wallet.spark_backup.run_migrate_spark_with_consent"] : []),
    ],
  }
}

// ---------------------------------------------------------------------------
// Spark backup receive RECONCILE / sweep (slice 3 of #5078).
//
// `migrate-spark --confirm-sweep` is the consented reconcile half of receive.
// It moves the node's OWN received Spark backup funds into the node's OWN MDK
// wallet. It is NOT a payout/send to third parties, NOT accepted-work
// settlement, and adds NO public payout-target authority. PayoutTargetKind and
// admitPayoutTarget are unchanged.
//
// Safety:
// - Requires EXPLICIT consent (`confirmSweep`); without it the reconcile is a
//   dry-run that refuses to move funds (state `consent-required`).
// - Inert by default: gated behind the same opt-in + credential + helper as the
//   receive path. Missing pieces resolve to the slice-1 blockers.
// - Emits ONLY public-safe redacted refs (digests, amounts, blocker refs).
//   Never raw Spark address/invoice/preimage/mnemonic/key/path material.
// ---------------------------------------------------------------------------

export type SparkBackupReconcileState =
  | "disabled"
  | "credential-missing"
  | "helper-unavailable"
  | "nothing-to-sweep"
  | "consent-required"
  | "sweep-pending-mdk-credit"
  | "swept-to-mdk"
  | "sweep-failed"

export type SparkBackupMdkCreditState =
  | "not-requested"
  | "receive-target-created"
  | "transfer-sent"
  | "verified"
  | "pending"
  | "failed"

export type SparkBackupReconcileProjection = {
  schema: "openagents.pylon.spark_backup_reconcile.v0.1"
  enabled: boolean
  state: SparkBackupReconcileState
  confirmSweep: boolean
  consentRequired: boolean
  detectedBalanceSats: number | null
  unclaimedDepositCount: number | null
  claimableHtlcCount: number | null
  claimableHtlcSats: number | null
  claimedDepositCount: number | null
  destinationReady: boolean
  sweptAmountSats: number | null
  transferFeeSats: number | null
  mdkCreditState: SparkBackupMdkCreditState
  mdkReceiveTargetRef: string | null
  sparkTransferRef: string | null
  mdkBalanceBeforeSats: number | null
  mdkBalanceAfterSats: number | null
  mdkCreditedSats: number | null
  blockerRefs: string[]
  failureRefs: string[]
  nextActionRefs: string[]
  publicReceiptRefs: string[]
  contentRedacted: true
}

export type SparkBackupSweepTransferResult =
  | {
      ok: true
      transferRef: string
      amountSats: number | null
      feeSats: number | null
    }
  | {
      ok: false
      failureRef: string
    }

export type SparkBackupSweepTransfer = (input: {
  amountSats: number
  destination: string
  idempotencyKey: string
}) => Promise<SparkBackupSweepTransferResult>

export const unavailableSparkBackupSweepTransfer: SparkBackupSweepTransfer = async () => ({
  ok: false,
  failureRef: "wallet.spark_backup_transfer.unavailable",
})

export type SparkBackupSweepOptions = SparkBackupReceiveOptions & {
  // Explicit consent gate. Without `confirmSweep: true` the reconcile is a
  // dry-run and refuses to move funds (audit failure mode 6 + legacy-Spark
  // migration consent model).
  confirmSweep?: boolean
  // When true, the embedded owner-authorized default Breez/Spark key counts as
  // a valid credential for this consented reconcile (#5085), so a legacy
  // migration sweep is not blocked on a missing env key.
  embeddedCredentialAvailable?: boolean
  // Whether the node's MDK destination is ready to receive the swept funds.
  // The live claim+send path requires this; the mock-backed path threads it
  // through so tests can assert the destination-readiness blocker.
  destinationReady?: boolean
  // The node's own MDK wallet runner. Used only to create a local receive
  // target and verify the balance increase after the Spark transfer.
  mdkRunner?: WalletCommandRunner
  // Private, sweep-only transfer seam. It pays the node's own MDK receive
  // target from the node's own Spark backup wallet and returns public refs only.
  transfer?: SparkBackupSweepTransfer
  now?: () => Date
  verificationAttempts?: number
  verificationDelayMs?: number
}

export type SparkBackupSendState =
  | "disabled"
  | "credential-missing"
  | "invalid-request"
  | "consent-required"
  | "send-failed"
  | "sent"

export type SparkBackupSendProjection = {
  schema: "openagents.pylon.spark_backup_send.v0.1"
  enabled: boolean
  state: SparkBackupSendState
  confirmSend: boolean
  consentRequired: boolean
  amountSats: number | null
  feeSats: number | null
  destinationRef: string | null
  sparkPaymentRef: string | null
  transferRef: string | null
  method: "payment_request" | "lnurl_pay" | null
  status: string | null
  blockerRefs: string[]
  failureRefs: string[]
  nextActionRefs: string[]
  publicReceiptRefs: string[]
  contentRedacted: true
}

export type SparkBackupSendTransferResult =
  | {
      ok: true
      transferRef: string
      sparkPaymentRef: string
      amountSats: number | null
      feeSats: number | null
      method: "payment_request" | "lnurl_pay"
      status: string | null
    }
  | {
      ok: false
      failureRef: string
    }

export type SparkBackupSendTransfer = (input: {
  amountSats: number
  destination: string
  idempotencyKey: string
}) => Promise<SparkBackupSendTransferResult>

export const unavailableSparkBackupSendTransfer: SparkBackupSendTransfer = async () => ({
  ok: false,
  failureRef: "wallet.spark_backup_send.unavailable",
})

export type SparkBackupSendOptions = SparkBackupReceiveOptions & {
  amountSats?: number
  confirmSend?: boolean
  destination?: string
  embeddedCredentialAvailable?: boolean
  now?: () => Date
  transfer?: SparkBackupSendTransfer
}

function safeSparkBackupReconcile(
  projection: SparkBackupReconcileProjection,
): SparkBackupReconcileProjection {
  assertSparkBackupProjectionSafe(projection)
  return projection
}

/**
 * Probe the Spark backup helper for the node's OWN received balance and
 * unclaimed deposits via the receive-only `status` + `unclaimed-deposits`
 * commands. Returns only numeric counts/amounts (never raw material).
 */
export async function detectSparkBackupBalance(helper: SparkBackupHelper): Promise<{
  helperReady: boolean
  detectedBalanceSats: number | null
  unclaimedDepositCount: number | null
  claimableHtlcCount: number | null
  claimableHtlcSats: number | null
}> {
  let statusResult: WalletCommandResult
  try {
    statusResult = await helper("status")
  } catch (error) {
    statusResult = { exitCode: 1, stdout: "", stderr: error instanceof Error ? error.message : String(error) }
  }
  if (statusResult.exitCode !== 0) {
    return {
      helperReady: false,
      detectedBalanceSats: null,
      unclaimedDepositCount: null,
      claimableHtlcCount: null,
      claimableHtlcSats: null,
    }
  }
  const statusData = parseMaybeJson(statusResult.stdout)
  const detectedBalanceSats =
    toSatNumber(statusData?.balance_sats) ?? toSatNumber(statusData?.spendable_balance_sats)
  const claimableHtlcCount = toSatNumber(statusData?.claimable_htlc_count)
  const claimableHtlcSats = toSatNumber(statusData?.claimable_htlc_sats)
  let unclaimedDepositCount = toSatNumber(statusData?.unclaimed_deposit_count)

  if (unclaimedDepositCount === null) {
    let depositsResult: WalletCommandResult
    try {
      depositsResult = await helper("unclaimed-deposits")
    } catch (error) {
      depositsResult = { exitCode: 1, stdout: "", stderr: error instanceof Error ? error.message : String(error) }
    }
    if (depositsResult.exitCode === 0) {
      const depositsData = parseMaybeJson(depositsResult.stdout)
      unclaimedDepositCount = toSatNumber(depositsData?.unclaimed_deposit_count)
    }
  }

  return { helperReady: true, detectedBalanceSats, unclaimedDepositCount, claimableHtlcCount, claimableHtlcSats }
}

/**
 * `migrate-spark --confirm-sweep`: consented reconcile of the node's OWN
 * received Spark backup balance into its OWN MDK wallet.
 *
 * RECEIVE-SIDE RECONCILE, NOT PAYOUT. This never sends to a third party and
 * never registers a public payout target.
 *
 * Behavior:
 * - opt-in disabled / missing credential / helper unavailable -> slice-1
 *   blockers, no fund movement.
 * - nothing detected -> `nothing-to-sweep`, no fund movement.
 * - detected balance but NO explicit consent -> `consent-required` (refuses).
 * - detected credited balance + explicit consent + destination ready -> create
 *   a local MDK receive target, transfer from Spark, then record a reconcile
 *   receipt only after MDK balance verification.
 *
 * The actual transfer is performed by the injected sweep adapter in live mode;
 * mock-backed tests inject balance/transfer seams and assert public redaction.
 */
export async function sweepSparkBackupToMdk(
  options: SparkBackupSweepOptions = {},
): Promise<SparkBackupReconcileProjection> {
  const env = options.env ?? process.env
  const enabled = isSparkBackupEnabled(options, env)
  const helper = options.helper ?? unavailableSparkBackupHelper
  const confirmSweep = options.confirmSweep === true
  const mdkRunner = options.mdkRunner ?? defaultWalletCommandRunner
  const transfer = options.transfer ?? unavailableSparkBackupSweepTransfer

  const base: SparkBackupReconcileProjection = {
    schema: "openagents.pylon.spark_backup_reconcile.v0.1",
    enabled,
    state: "disabled",
    confirmSweep,
    consentRequired: true,
    detectedBalanceSats: null,
    unclaimedDepositCount: null,
    claimableHtlcCount: null,
    claimableHtlcSats: null,
    claimedDepositCount: null,
    destinationReady: options.destinationReady === true,
    sweptAmountSats: null,
    transferFeeSats: null,
    mdkCreditState: "not-requested",
    mdkReceiveTargetRef: null,
    sparkTransferRef: null,
    mdkBalanceBeforeSats: null,
    mdkBalanceAfterSats: null,
    mdkCreditedSats: null,
    blockerRefs: [],
    failureRefs: [],
    nextActionRefs: [],
    publicReceiptRefs: [],
    contentRedacted: true,
  }

  if (!enabled) {
    return safeSparkBackupReconcile({
      ...base,
      state: "disabled",
      nextActionRefs: ["action.wallet.spark_backup.enable_opt_in"],
    })
  }

  if (!hasSparkBackupCredential(env, options.embeddedCredentialAvailable === true)) {
    return safeSparkBackupReconcile({
      ...base,
      state: "credential-missing",
      blockerRefs: ["blocker.wallet.spark_backup.credential_missing"],
      nextActionRefs: ["action.wallet.spark_backup.configure_local_credential"],
    })
  }

  const detected = await detectSparkBackupBalance(helper)
  if (!detected.helperReady) {
    return safeSparkBackupReconcile({
      ...base,
      state: "helper-unavailable",
      blockerRefs: ["blocker.wallet.spark_backup.helper_unavailable"],
      nextActionRefs: ["action.wallet.spark_backup.install_or_start_helper"],
    })
  }

  const detectedBalanceSats = detected.detectedBalanceSats
  const unclaimedDepositCount = detected.unclaimedDepositCount
  const claimableHtlcCount = detected.claimableHtlcCount
  const claimableHtlcSats = detected.claimableHtlcSats
  const hasBalance = detectedBalanceSats !== null && detectedBalanceSats > 0
  const hasDeposits = unclaimedDepositCount !== null && unclaimedDepositCount > 0
  const hasClaimableHtlcs = claimableHtlcCount !== null && claimableHtlcCount > 0

  if (!hasBalance && !hasDeposits && !hasClaimableHtlcs) {
    return safeSparkBackupReconcile({
      ...base,
      state: "nothing-to-sweep",
      detectedBalanceSats,
      unclaimedDepositCount,
      claimableHtlcCount,
      claimableHtlcSats,
    })
  }

  if (!hasBalance) {
    return safeSparkBackupReconcile({
      ...base,
      state: "sweep-failed",
      detectedBalanceSats,
      unclaimedDepositCount,
      claimableHtlcCount,
      claimableHtlcSats,
      blockerRefs: hasClaimableHtlcs
        ? ["blocker.wallet.spark_backup.claim_required_before_sweep"]
        : ["blocker.wallet.spark_backup.no_credited_balance_detected"],
      nextActionRefs: hasClaimableHtlcs
        ? ["action.wallet.spark_backup.run_backup_claim_before_sweep"]
        : ["action.wallet.spark_backup.backup_status"],
    })
  }

  // Detected funds. Consent is mandatory before ANY movement (audit failure
  // mode 6 + the legacy-Spark migration consent model).
  if (!confirmSweep) {
    return safeSparkBackupReconcile({
      ...base,
      state: "consent-required",
      consentRequired: true,
      detectedBalanceSats,
      unclaimedDepositCount,
      claimableHtlcCount,
      claimableHtlcSats,
      blockerRefs: ["blocker.wallet.spark_backup.sweep_consent_required"],
      nextActionRefs: ["action.wallet.spark_backup.rerun_with_confirm_sweep"],
    })
  }

  // Destination readiness is load-bearing: never sweep into an MDK wallet that
  // cannot receive. Keep the funds where they are and record a public-safe
  // blocker rather than reporting a phantom settlement (audit failure mode 7).
  if (options.destinationReady !== true) {
    return safeSparkBackupReconcile({
      ...base,
      state: "sweep-failed",
      consentRequired: false,
      detectedBalanceSats,
      unclaimedDepositCount,
      claimableHtlcCount,
      claimableHtlcSats,
      destinationReady: false,
      blockerRefs: ["blocker.wallet.spark_backup.mdk_destination_not_ready"],
      nextActionRefs: ["action.wallet.spark_backup.prepare_mdk_destination"],
    })
  }

  const mdkBefore = await classifyMdkWallet(mdkRunner, env)
  const mdkBalanceBeforeSats = mdkBefore.balanceSats
  if (mdkBalanceBeforeSats === null) {
    return safeSparkBackupReconcile({
      ...base,
      state: "sweep-failed",
      consentRequired: false,
      detectedBalanceSats,
      unclaimedDepositCount,
      claimableHtlcCount,
      claimableHtlcSats,
      destinationReady: true,
      blockerRefs: ["blocker.wallet.spark_backup.mdk_balance_unverified_before_sweep"],
      nextActionRefs: ["action.wallet.mdk.restore_balance_status"],
    })
  }

  const receiveTarget = await createMdkReceiveTarget(detectedBalanceSats, mdkRunner)
  if (!receiveTarget.ok) {
    return safeSparkBackupReconcile({
      ...base,
      state: "sweep-failed",
      consentRequired: false,
      detectedBalanceSats,
      unclaimedDepositCount,
      claimableHtlcCount,
      claimableHtlcSats,
      destinationReady: true,
      mdkBalanceBeforeSats,
      mdkCreditState: "failed",
      blockerRefs: ["blocker.wallet.spark_backup.mdk_receive_target_failed"],
      failureRefs: [receiveTarget.failureRef],
      nextActionRefs: ["action.wallet.spark_backup.retry_after_mdk_receive_ready"],
    })
  }

  const now = options.now?.() ?? new Date()
  const idempotencyKey = `pylon:spark-backup-sweep:${stableRef(
    "sweep",
    JSON.stringify({
      amount: detectedBalanceSats,
      target: receiveTarget.targetRef,
      at: now.toISOString().slice(0, 10),
    }),
  ).split(".").pop()}`
  const transferResult = await transfer({
    amountSats: detectedBalanceSats,
    destination: receiveTarget.target,
    idempotencyKey,
  })
  if (!transferResult.ok) {
    return safeSparkBackupReconcile({
      ...base,
      state: "sweep-failed",
      consentRequired: false,
      detectedBalanceSats,
      unclaimedDepositCount,
      claimableHtlcCount,
      claimableHtlcSats,
      destinationReady: true,
      mdkBalanceBeforeSats,
      mdkReceiveTargetRef: receiveTarget.targetRef,
      mdkCreditState: "failed",
      blockerRefs: [
        transferResult.failureRef === "wallet.spark_backup_transfer.unavailable"
          ? "blocker.wallet.spark_backup.transfer_unavailable"
          : "blocker.wallet.spark_backup.transfer_failed",
      ],
      failureRefs: [transferResult.failureRef],
      nextActionRefs: ["action.wallet.spark_backup.retry_after_transfer_ready"],
    })
  }

  const attempts = Math.max(1, Math.floor(options.verificationAttempts ?? 3))
  const delayMs = Math.max(0, Math.floor(options.verificationDelayMs ?? 1_000))
  let mdkBalanceAfterSats: number | null = null
  for (let attempt = 0; attempt < attempts; attempt += 1) {
    const mdkAfter = await classifyMdkWallet(mdkRunner, env)
    mdkBalanceAfterSats = mdkAfter.balanceSats
    if (
      mdkBalanceAfterSats !== null &&
      mdkBalanceAfterSats >= mdkBalanceBeforeSats + detectedBalanceSats
    ) {
      break
    }
    if (attempt < attempts - 1 && delayMs > 0) {
      await sleep(delayMs)
    }
  }

  const mdkCreditedSats =
    mdkBalanceAfterSats === null
      ? null
      : Math.max(0, mdkBalanceAfterSats - mdkBalanceBeforeSats)
  const verified = mdkCreditedSats !== null && mdkCreditedSats >= detectedBalanceSats
  const claimedDepositCount = 0
  const sweptAmountSats = detectedBalanceSats
  const receiptDigest = stableRef(
    "reconcile",
    JSON.stringify({
      swept: sweptAmountSats,
      transferRef: transferResult.transferRef,
      targetRef: receiveTarget.targetRef,
      before: mdkBalanceBeforeSats,
      after: mdkBalanceAfterSats,
      at: now.toISOString(),
    }),
  )
    .split(".")
    .pop()
  const transferReceiptDigest = stableRef(
    "transfer",
    JSON.stringify({
      swept: sweptAmountSats,
      transferRef: transferResult.transferRef,
      targetRef: receiveTarget.targetRef,
      at: now.toISOString(),
    }),
  )
    .split(".")
    .pop()

  return safeSparkBackupReconcile({
    ...base,
    state: verified ? "swept-to-mdk" : "sweep-pending-mdk-credit",
    consentRequired: false,
    detectedBalanceSats,
    unclaimedDepositCount,
    claimableHtlcCount,
    claimableHtlcSats,
    claimedDepositCount,
    destinationReady: true,
    sweptAmountSats,
    transferFeeSats: transferResult.feeSats,
    mdkCreditState: verified ? "verified" : "pending",
    mdkReceiveTargetRef: receiveTarget.targetRef,
    sparkTransferRef: transferResult.transferRef,
    mdkBalanceBeforeSats,
    mdkBalanceAfterSats,
    mdkCreditedSats,
    nextActionRefs: verified
      ? ["action.wallet.spark_backup.confirm_mdk_balance_after_sweep"]
      : ["action.wallet.spark_backup.rerun_status_until_mdk_credit_visible"],
    publicReceiptRefs: verified
      ? [`receipt.pylon.spark_backup_reconcile.${receiptDigest}`]
      : [`receipt.pylon.spark_backup_transfer.${transferReceiptDigest}`],
  })
}

export async function sendWithSparkBackup(
  options: SparkBackupSendOptions = {},
): Promise<SparkBackupSendProjection> {
  const env = options.env ?? process.env
  const enabled = isSparkBackupEnabled(options, env)
  const confirmSend = options.confirmSend === true
  const destination = typeof options.destination === "string" ? options.destination.trim() : ""
  const amountSats =
    typeof options.amountSats === "number" && Number.isFinite(options.amountSats)
      ? Math.floor(options.amountSats)
      : null
  const destinationRef = destination.length > 0 ? stableRef("wallet.spark_backup_send.destination", destination) : null
  const base: SparkBackupSendProjection = {
    schema: "openagents.pylon.spark_backup_send.v0.1",
    enabled,
    state: "disabled",
    confirmSend,
    consentRequired: true,
    amountSats,
    feeSats: null,
    destinationRef,
    sparkPaymentRef: null,
    transferRef: null,
    method: null,
    status: null,
    blockerRefs: [],
    failureRefs: [],
    nextActionRefs: [],
    publicReceiptRefs: [],
    contentRedacted: true,
  }

  const safe = (projection: SparkBackupSendProjection) => {
    assertSparkBackupProjectionSafe(projection)
    return projection
  }

  if (!enabled) {
    return safe({
      ...base,
      state: "disabled",
      nextActionRefs: ["action.wallet.spark_backup.enable_opt_in"],
    })
  }

  if (!hasSparkBackupCredential(env, options.embeddedCredentialAvailable === true)) {
    return safe({
      ...base,
      state: "credential-missing",
      blockerRefs: ["blocker.wallet.spark_backup.credential_missing"],
      nextActionRefs: ["action.wallet.spark_backup.configure_local_credential"],
    })
  }

  if (destination.length === 0 || amountSats === null || amountSats <= 0) {
    return safe({
      ...base,
      state: "invalid-request",
      blockerRefs: [
        ...(destination.length === 0 ? ["blocker.wallet.spark_backup.send_destination_required"] : []),
        ...(amountSats === null || amountSats <= 0 ? ["blocker.wallet.spark_backup.send_amount_required"] : []),
      ],
      nextActionRefs: ["action.wallet.spark_backup.rerun_send_with_destination_and_amount"],
    })
  }

  if (!confirmSend) {
    return safe({
      ...base,
      state: "consent-required",
      blockerRefs: ["blocker.wallet.spark_backup.send_consent_required"],
      nextActionRefs: ["action.wallet.spark_backup.rerun_send_with_confirm_send"],
    })
  }

  const now = options.now?.() ?? new Date()
  const transfer = options.transfer ?? unavailableSparkBackupSendTransfer
  const idempotencyKey = `pylon:spark-backup-send:${stableRef(
    "send",
    JSON.stringify({
      amount: amountSats,
      destination: destinationRef,
      at: now.toISOString().slice(0, 10),
    }),
  ).split(".").pop()}`
  const result = await transfer({ amountSats, destination, idempotencyKey })
  if (!result.ok) {
    return safe({
      ...base,
      state: "send-failed",
      consentRequired: false,
      blockerRefs: [
        result.failureRef === "wallet.spark_backup_send.unavailable"
          ? "blocker.wallet.spark_backup.send_transfer_unavailable"
          : "blocker.wallet.spark_backup.send_failed",
      ],
      failureRefs: [result.failureRef],
      nextActionRefs: ["action.wallet.spark_backup.retry_send_after_fixing_transfer"],
    })
  }

  const receiptDigest = stableRef(
    "spark-backup-send",
    JSON.stringify({
      amount: result.amountSats,
      fee: result.feeSats,
      destinationRef,
      transferRef: result.transferRef,
      paymentRef: result.sparkPaymentRef,
      method: result.method,
      status: result.status,
      at: now.toISOString(),
    }),
  )
    .split(".")
    .pop()

  return safe({
    ...base,
    state: "sent",
    consentRequired: false,
    amountSats: result.amountSats,
    feeSats: result.feeSats,
    sparkPaymentRef: result.sparkPaymentRef,
    transferRef: result.transferRef,
    method: result.method,
    status: result.status,
    nextActionRefs: ["action.wallet.spark_backup.check_backup_status"],
    publicReceiptRefs: [`receipt.pylon.spark_backup_send.${receiptDigest}`],
  })
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

// ---------------------------------------------------------------------------
// Spark backup local private state (slice 2).
//
// The raw Spark receive target is cached ONLY in Pylon-home private state, mode
// 0600, never in public state and never in any projection. The cache lets
// `cached-address-ready` work when the helper is offline (audit failure mode 2).
// ---------------------------------------------------------------------------

export function sparkBackupTargetPath(paths: PylonPaths) {
  return `${paths.home}/wallet/spark-backup/receive-target.json`
}

type SparkBackupTargetFile = {
  schema: "openagents.pylon.spark_backup_target.local.v0.1"
  rawTarget: string
  updatedAt: string
}

export async function readCachedSparkTarget(paths: PylonPaths): Promise<string | null> {
  const path = sparkBackupTargetPath(paths)
  if (!existsSync(path)) return null
  try {
    const parsed = JSON.parse(await readFile(path, "utf8")) as Partial<SparkBackupTargetFile>
    return typeof parsed.rawTarget === "string" && parsed.rawTarget.trim() !== "" ? parsed.rawTarget : null
  } catch {
    return null
  }
}

export async function writeCachedSparkTarget(paths: PylonPaths, rawTarget: string): Promise<void> {
  if (!rawTarget || rawTarget.trim() === "") return
  const path = sparkBackupTargetPath(paths)
  const { mkdir } = await import("node:fs/promises")
  const dir = path.slice(0, path.lastIndexOf("/"))
  await mkdir(dir, { recursive: true })
  const file: SparkBackupTargetFile = {
    schema: "openagents.pylon.spark_backup_target.local.v0.1",
    rawTarget,
    updatedAt: new Date().toISOString(),
  }
  // Mode 0600: this file holds raw wallet-operable receive material.
  await writeFile(path, `${JSON.stringify(file, null, 2)}\n`, { mode: 0o600 })
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
