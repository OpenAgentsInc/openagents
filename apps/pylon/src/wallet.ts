import { readFile, unlink, writeFile } from "node:fs/promises"
import { existsSync } from "node:fs"
import { createHash, randomUUID } from "node:crypto"
import { homedir } from "node:os"
import { join } from "node:path"
import type { PylonPaths } from "./state.js"
import { assertPublicProjectionSafe, ensureStateDirectories } from "./state.js"
import { toSatNumber } from "./sat-number.js"

export type WalletReadiness =
  | "daemon-offline"
  | "balance-unknown"
  | "receive-ready"
  | "send-ready"
  | "send-ready-blocked"
  | "payout-target-admitted"
  | "payable-pending-settlement"
  | "settlement-recorded"

export type PayoutTargetKind =
  | "bolt12_offer"
  | "bolt11_invoice"
  | "bip353_name"
  | "lnurl_pay"
  | "spark_address"

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

import {
  classifySparkHelperFailureReason,
  sanitizeSparkDebug,
  type SparkHelperUnavailableReason,
} from "./spark-backup-helper.js"

// #5194: dump a raw helper result to stderr (PYLON_SPARK_DEBUG=1 only) whenever
// a read failed to become ready. The point: even when the stderr is EMPTY the
// operator gets a `[spark-helper:gate]` line with `exitCode=<n> stderrLen=<n>`,
// so an empty-stderr failure is itself diagnosable in ONE shot. Path-sanitized
// (no $HOME/temp absolute paths) but the COMPLETE stderr text otherwise, plus
// stdout, since this is stderr-only and NEVER enters a projection.
function dumpSparkHelperGate(
  context: string,
  result: { exitCode: number; stdout: string; stderr: string },
  extra?: string,
) {
  if (process.env.PYLON_SPARK_DEBUG !== "1") return
  const stderr = result.stderr ?? ""
  const stdout = result.stdout ?? ""
  console.error(
    `[spark-helper:gate] ${context} exitCode=${result.exitCode} stderrLen=${stderr.length} stdoutLen=${stdout.length}${
      extra ? ` ${extra}` : ""
    }`,
  )
  if (stderr.length > 0) {
    console.error(`[spark-helper:gate] ${context} stderr=${sanitizeSparkDebug(stderr).slice(0, 800)}`)
  }
  if (stdout.length > 0) {
    console.error(`[spark-helper:gate] ${context} stdout=${sanitizeSparkDebug(stdout).slice(0, 800)}`)
  }
}

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
  // #5197: true when detectedBalanceSats came from a non-forced fallback read
  // (the authoritative ensureSynced read did not complete — e.g. a fresh
  // post-restart sync). The number is shown for visibility but must NOT be
  // treated as a confirmed-spendable balance; re-read once it clears.
  balanceRefreshing?: boolean
  unclaimedDepositCount: number | null
  // #5166: pending Lightning HTLCs (offline-received funds awaiting
  // `backup-claim`). Read-only — lets an operator see incoming funds before
  // claiming. Optional for back-compat with older projections.
  claimableHtlcCount?: number | null
  claimableHtlcSats?: number | null
  // #5194: a bounded, public-safe enum explaining WHY the helper is unavailable
  // when `helperReady` is false (db_init_failed | timeout | network_unreachable
  // | module_load_failed | no_result | unknown). NEVER the raw stderr — that is
  // private. Null when the helper is ready or the state is not a failure. This
  // is the only failure detail a daemon-routed read can surface to the operator.
  helperUnavailableReason?: SparkHelperUnavailableReason | null
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

export type LegacyMdkRecoveryState =
  | "not-detected"
  | "blocked"
  | "consent-required"
  | "ready"
  | "recovered"
  | "recovery-failed"

export type LegacyMdkRecoveryProjection = {
  schema: "openagents.pylon.legacy_mdk_recovery.v0.1"
  state: LegacyMdkRecoveryState
  dryRun: boolean
  legacyBalanceDetected: boolean
  legacyBalanceSats: number | null
  requestedAmountSats: number | null
  destinationRef: string | null
  explicitConsentRequired: boolean
  primaryRailReenabled: false
  blockerRefs: string[]
  nextActionRefs: string[]
  publicReceiptRefs: string[]
  failureRefs: string[]
  contentRedacted: true
}

export type LegacyMdkRecoveryOptions = {
  amountSats?: number
  destination?: string
  dryRun?: boolean
  env?: NodeJS.ProcessEnv
  now?: () => Date
  runner?: WalletCommandRunner
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

const DEFAULT_MDK_WALLET_COMMAND_TIMEOUT_MS = 30_000

export function agentWalletCommandTimeoutMs(env: NodeJS.ProcessEnv = process.env) {
  const parsed = Number(env.MDK_WALLET_COMMAND_TIMEOUT_MS)
  return Number.isInteger(parsed) && parsed > 0
    ? parsed
    : DEFAULT_MDK_WALLET_COMMAND_TIMEOUT_MS
}

export function agentWalletArgs(args: string[], env: NodeJS.ProcessEnv = process.env) {
  const port = env.MDK_WALLET_PORT?.trim()
  if (!port || args.includes("--port")) return args
  return [...args, "--port", port]
}

export const defaultWalletCommandRunner: WalletCommandRunner = async (args) => {
  await reclaimStaleMdkDaemonPidfile()
  const proc = Bun.spawn(["npx", "--yes", "@moneydevkit/agent-wallet@latest", ...agentWalletArgs(args)], {
    stdout: "pipe",
    stderr: "pipe",
  })
  const timeoutMs = agentWalletCommandTimeoutMs()
  const timeout = new Promise<never>((_, reject) =>
    setTimeout(() => {
      proc.kill()
      reject(new Error("MDK agent-wallet command timed out"))
    }, timeoutMs),
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
  action: "wallet-readiness" | "payout-target-admission" | "spark-payout-target",
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
  // #5306: when the helper is unavailable AND we know WHY (a bounded public-safe
  // reason from the read — db_init_failed | timeout | network_unreachable |
  // module_load_failed | no_result | unknown), surface a reason-qualified blocker
  // (`helper_unavailable.<reason>`) ALONGSIDE the bare ref so a degraded
  // `wallet status` gives an actionable detail instead of only the opaque
  // `helper_unavailable`. The default healthy path never hits this (helperReady).
  const helperUnavailableReason =
    typeof sparkBackup?.helperUnavailableReason === "string" ? sparkBackup.helperUnavailableReason : null
  const blockerRefs = [
    ...(!enabled ? ["blocker.wallet.spark_primary.disabled"] : []),
    ...(enabled && !credentialReady ? ["blocker.wallet.spark_primary.credential_missing"] : []),
    ...(configured && !helperReady
      ? [
          "blocker.wallet.spark_primary.helper_unavailable",
          ...(helperUnavailableReason
            ? [`blocker.wallet.spark_primary.helper_unavailable.${helperUnavailableReason}`]
            : []),
        ]
      : []),
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

/**
 * Surface whether this node has a registered payout target (Gap #2 from the
 * v1.0 self-serve shakeout). A contributor can claim + run verified work, but
 * if no payout target is registered the verified pair settles to NOTHING at the
 * downstream destination resolver — silently. This makes that state VISIBLE in
 * `wallet status` BEFORE earning by:
 *   - populating `payoutTargetRefs` with the registered digest ref(s), and
 *   - appending `blocker.wallet.payout_target_unregistered` when none exist.
 *
 * Applied to EVERY branch (default/unconfigured AND the live send-ready branch)
 * because a wallet can be send-ready yet have NO payout target registered —
 * exactly the Orrery case. Projection-safe: only redacted `payout.<kind>.<digest>`
 * refs are accepted (the same admission guard the register path uses); a raw
 * Spark address / invoice / offer is rejected before it can reach a projection.
 */
export function withPayoutTargetReadiness(
  status: WalletStatusProjection,
  registeredPayoutTargetRefs: ReadonlyArray<string | null | undefined> = [],
): WalletStatusProjection {
  const payoutTargetRefs = [
    ...new Set(
      registeredPayoutTargetRefs
        .map((ref) => (typeof ref === "string" ? ref.trim() : ""))
        .filter((ref) => ref !== ""),
    ),
  ]
  const hasPayoutTarget = payoutTargetRefs.length > 0
  const blockerRefs = hasPayoutTarget
    ? status.blockerRefs
    : [...new Set([...status.blockerRefs, "blocker.wallet.payout_target_unregistered"])]
  const next: WalletStatusProjection = {
    ...status,
    payoutTargetRefs,
    blockerRefs,
  }
  // Guard the projection so a raw payout target (e.g. a `spark1…` address) can
  // never ride the `payoutTargetRefs` field into a projection or log. Only the
  // redacted `payout.<kind>.<digest>` digest refs are admissible.
  assertPublicProjectionSafe(next)
  return next
}

// Projection-safe balance subset shared by the mobile `walletStatus` action and
// the dedicated `pylon balance --json` command (#5402 launch shakeout). Both
// read the SAME local primary-wallet projection (`classifyPrimaryAgentWallet`)
// so a contributor running `balance` sees the same readable, send-ready balance
// `wallet status` shows -- not the empty `{}` the old network earnings endpoint
// returned. This deliberately exposes only redacted/projection-safe fields:
// balance + readiness flags + blocker refs + the unified-balance projection.
// It NEVER carries seeds, raw Spark addresses, invoices, offers, or any other
// payment material -- the same redaction posture `wallet status` already uses.
export type WalletBalanceProjection = {
  schema: "openagents.pylon.wallet_balance.v0.1"
  configured: boolean
  daemonOnline: boolean
  balanceSats: number | null
  receiveReady: boolean
  sendReady: boolean
  readiness: WalletReadiness
  blockerRefs: string[]
  unifiedBalance: UnifiedWalletBalanceProjection
}

export function projectWalletBalance(status: WalletStatusProjection): WalletBalanceProjection {
  return {
    schema: "openagents.pylon.wallet_balance.v0.1",
    configured: status.configured,
    daemonOnline: status.daemonOnline,
    balanceSats: status.balanceSats,
    receiveReady: status.receiveReady,
    sendReady: status.sendReady,
    readiness: status.readiness,
    blockerRefs: status.blockerRefs,
    unifiedBalance: status.unifiedBalance,
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
    action: "wallet-readiness" | "payout-target-admission" | "spark-payout-target"
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

function safeLegacyMdkRecovery(
  projection: LegacyMdkRecoveryProjection,
): LegacyMdkRecoveryProjection {
  assertPublicProjectionSafe(projection)
  return projection
}

function legacyMdkRecoveryBalanceFrom(data: Record<string, unknown> | null): number | null {
  return (
    toSatNumber(data?.balance_sats) ??
    toSatNumber(data?.spendable_balance_sats) ??
    toSatNumber(data?.channel_balance_sats) ??
    toSatNumber(data?.confirmed_balance_sats)
  )
}

export async function recoverLegacyMdkBalance(
  options: LegacyMdkRecoveryOptions = {},
): Promise<LegacyMdkRecoveryProjection> {
  const env = options.env ?? process.env
  const dryRun = options.dryRun !== false
  const runner = options.runner ?? defaultWalletCommandRunner
  const hintedBalance = envNumber(env, "PYLON_LEGACY_MDK_BALANCE_SATS")
  let balanceResult: WalletCommandResult | null = null
  try {
    balanceResult = await runner(["balance"])
  } catch (error) {
    balanceResult = {
      exitCode: 1,
      stdout: "",
      stderr: error instanceof Error ? error.message : String(error),
    }
  }
  const balanceData = balanceResult.exitCode === 0 ? parseMaybeJson(balanceResult.stdout) : null
  const legacyBalanceSats = legacyMdkRecoveryBalanceFrom(balanceData) ?? hintedBalance
  const legacyBalanceDetected = legacyBalanceSats !== null && legacyBalanceSats > 0
  const destination = typeof options.destination === "string" ? options.destination.trim() : ""
  const destinationRef =
    destination.length > 0
      ? stableRef("wallet.legacy_mdk_recovery.destination", destination)
      : null
  const amountProvided = options.amountSats !== undefined
  const requestedAmountSats =
    typeof options.amountSats === "number" && Number.isFinite(options.amountSats) && options.amountSats > 0
      ? Math.floor(options.amountSats)
      : null
  const amountValid = !amountProvided || requestedAmountSats !== null
  const amountWithinBalance =
    requestedAmountSats === null ||
    legacyBalanceSats === null ||
    requestedAmountSats <= legacyBalanceSats
  const consentGiven = options.yes === true
  const balanceReadReady = balanceResult.exitCode === 0 || hintedBalance !== null
  const blockerRefs = [
    ...(balanceReadReady ? [] : ["blocker.wallet.legacy_mdk.balance_read_failed"]),
    ...(legacyBalanceDetected ? [] : ["blocker.wallet.legacy_mdk.no_residual_balance_detected"]),
    ...(destinationRef === null ? ["blocker.wallet.legacy_mdk.destination_required"] : []),
    ...(amountValid ? [] : ["blocker.wallet.legacy_mdk.amount_invalid"]),
    ...(amountWithinBalance ? [] : ["blocker.wallet.legacy_mdk.amount_exceeds_detected_balance"]),
  ]
  const ready = blockerRefs.length === 0
  const base: LegacyMdkRecoveryProjection = {
    schema: "openagents.pylon.legacy_mdk_recovery.v0.1",
    state: !legacyBalanceDetected ? "not-detected" : !ready ? "blocked" : !consentGiven ? "consent-required" : dryRun ? "ready" : "recovered",
    dryRun,
    legacyBalanceDetected,
    legacyBalanceSats,
    requestedAmountSats,
    destinationRef,
    explicitConsentRequired: !consentGiven,
    primaryRailReenabled: false,
    blockerRefs,
    nextActionRefs: ready
      ? ["action.wallet.legacy_mdk.review_and_confirm_local_recovery"]
      : [
          ...(legacyBalanceDetected ? [] : ["action.wallet.legacy_mdk.rerun_when_residual_balance_exists"]),
          ...(destinationRef === null ? ["action.wallet.legacy_mdk.prepare_local_destination"] : []),
          ...(amountValid ? [] : ["action.wallet.legacy_mdk.provide_positive_recovery_amount"]),
          ...(amountWithinBalance ? [] : ["action.wallet.legacy_mdk.lower_recovery_amount"]),
        ],
    publicReceiptRefs: [],
    failureRefs: [],
    contentRedacted: true,
  }

  if (base.state !== "recovered") return safeLegacyMdkRecovery(base)

  const sendArgs = requestedAmountSats === null
    ? ["send", destination]
    : ["send", destination, String(requestedAmountSats)]
  let sendResult: WalletCommandResult
  try {
    sendResult = await runner(sendArgs)
  } catch (error) {
    sendResult = {
      exitCode: 1,
      stdout: "",
      stderr: error instanceof Error ? error.message : String(error),
    }
  }
  if (sendResult.exitCode !== 0) {
    return safeLegacyMdkRecovery({
      ...base,
      state: "recovery-failed",
      failureRefs: [
        stableRef(
          "wallet.legacy_mdk_recovery.failure",
          `${sendResult.exitCode}:${sendResult.stderr || sendResult.stdout}`,
        ),
      ],
      nextActionRefs: ["action.wallet.legacy_mdk.inspect_local_mdk_recovery_failure"],
    })
  }

  const receiptDigest = stableRef(
    "legacy-mdk-recovery",
    JSON.stringify({
      amount: requestedAmountSats,
      balance: legacyBalanceSats,
      destinationRef,
      result: parseMaybeJson(sendResult.stdout) ?? sendResult.stdout,
      at: options.now?.().toISOString() ?? "recovery_time_redacted",
    }),
  )
    .split(".")
    .pop()

  return safeLegacyMdkRecovery({
    ...base,
    state: "recovered",
    explicitConsentRequired: false,
    nextActionRefs: ["action.wallet.legacy_mdk.verify_destination_credit_locally"],
    publicReceiptRefs: [`receipt.pylon.legacy_mdk_recovery.${receiptDigest}`],
  })
}

export function admitPayoutTarget(input: { kind: PayoutTargetKind; ref: string }) {
  const allowedPrefix: Record<PayoutTargetKind, string> = {
    bolt12_offer: "payout.bolt12",
    bolt11_invoice: "payout.bolt11",
    bip353_name: "payout.bip353",
    lnurl_pay: "payout.lnurl",
    // Spark address payout targets project ONLY the redacted digest ref
    // `payout.spark.<digest>`. The raw `spark1…` is payment material and never
    // enters a public projection, an admission body, or this ref (#5252).
    spark_address: "payout.spark",
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

/**
 * Derive the public-safe redacted payout-target ref for a raw Spark address.
 * The raw `spark1…` never appears in the returned ref or in any projection —
 * only its `payout.spark.<digest>` digest does. The digest is stable per raw
 * address, so re-registering the same address yields the same ref (idempotent).
 */
export function sparkPayoutTargetRef(rawSparkAddress: string): string {
  const trimmed = rawSparkAddress.trim()
  if (trimmed === "") {
    throw new Error("spark payout target requires a non-empty raw Spark address")
  }
  return stableRef("payout.spark", trimmed)
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

export type SparkPayoutTargetRegisterResult = {
  ok: boolean
  payoutTargetRef: string
  rawAddressPostedPrivately: true
  response: Record<string, unknown>
}

/**
 * Register this node's OWN raw Spark address as its registerable payout target
 * (#5252). The raw `spark1…` is PAYMENT MATERIAL: it rides ONLY the
 * authenticated POST body to the private operator endpoint
 * `/api/pylons/:ref/spark-payout-target`. Everything this function returns and
 * any projection/log it produces carries ONLY the redacted
 * `payout.spark.<digest>` ref. The server stores the raw address privately
 * (keyed to the agent's pylonRef) and emits the public `payout_target_admission`
 * event carrying only the digest.
 *
 * Auth is the agent's own bearer token, so a node can only set its own target.
 * The server upsert is idempotent (re-registering the same address is a no-op
 * update, not a duplicate).
 */
export async function registerSparkPayoutTarget(
  input: { rawSparkAddress: string },
  options: WalletNetworkOptions,
): Promise<SparkPayoutTargetRegisterResult> {
  const rawSparkAddress = input.rawSparkAddress.trim()
  if (rawSparkAddress === "") {
    throw new Error("spark payout target registration requires a non-empty raw Spark address")
  }
  const payoutTargetRef = sparkPayoutTargetRef(rawSparkAddress)
  // Admit the digest ref first so an unsafe/raw ref can never reach the wire.
  admitPayoutTarget({ kind: "spark_address", ref: payoutTargetRef })

  // The body carries the raw address ONLY under `rawSparkAddress`, plus the
  // redacted digest ref. The public-safe guard runs over the redacted view
  // (without the raw field) so it never leaks into a projection or log.
  const redactedView = {
    payoutTargetRef,
    policyRefs: ["policy.private.pylon.spark_payout_target_raw_stored_operator_only"],
    status: "registered",
  }
  assertPublicProjectionSafe(redactedView)

  const response = await postPylonEvent(options, {
    action: "spark-payout-target",
    body: {
      ...redactedView,
      // PRIVATE: never projected, never logged, never persisted to a public
      // event. Stored only in the operator/private store keyed to pylonRef.
      rawSparkAddress,
    },
    path: `/api/pylons/${encodeURIComponent(options.pylonRef)}/spark-payout-target`,
  })

  return {
    ok: true,
    payoutTargetRef,
    rawAddressPostedPrivately: true,
    response,
  }
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
 * The default TCP port the MDK agent-wallet daemon binds when `MDK_WALLET_PORT`
 * is unset. Collides with other common local Bitcoin tools (e.g. the Orange
 * wallet webhook also listens on `:3456`), which is the first-run conflict
 * reported in #5505. Kept as a named constant so the conflict guidance and any
 * future free-port probe share one source of truth.
 */
export const DEFAULT_MDK_WALLET_PORT = 3456

/**
 * Detect whether an MDK agent-wallet command failed because its daemon port was
 * already in use (a bind/EADDRINUSE-class failure), and — if so — produce a
 * clear, actionable, public-safe message instead of an opaque crash dump.
 *
 * Pylon shells out to `@moneydevkit/agent-wallet` (see `defaultWalletCommandRunner`),
 * so the daemon — not Pylon — owns the actual `listen()`. When that bind
 * collides on the default `:3456` (the #5505 report: an Orange wallet webhook
 * already held the port), the operator only saw an opaque failure and had to
 * discover `MDK_WALLET_PORT` themselves. This mirrors the proven control-port
 * conflict guidance in `formatNodeStartupError` (index.ts) so the wallet path
 * surfaces the same kind of actionable remediation.
 *
 * Pure + deterministic over the command result text; emits no secrets (only the
 * env var name, the numeric port, and a redacted stable ref), so the projection
 * is public-safe by construction.
 */
export function describeMdkPortConflict(
  result: WalletCommandResult,
  env: NodeJS.ProcessEnv = process.env,
): {
  isPortConflict: boolean
  port: number
  portConfigured: boolean
  ref: string
  message: string | null
} {
  const text = `${result.stderr}\n${result.stdout}`.toLowerCase()
  const isPortConflict =
    /eaddrinuse/.test(text) ||
    /address (already )?in use/.test(text) ||
    /port\s+\d*\s*(is\s+)?(already\s+)?in use/.test(text) ||
    /(bind|listen)(ing)?\b.*\b(failed|error)/.test(text) ||
    /failed to (bind|listen)/.test(text)

  const configuredRaw = env.MDK_WALLET_PORT
  const portConfigured = typeof configuredRaw === "string" && configuredRaw.trim() !== ""
  const parsedConfigured = portConfigured ? Number(configuredRaw!.trim()) : NaN
  const port =
    portConfigured && Number.isInteger(parsedConfigured) && parsedConfigured > 0
      ? parsedConfigured
      : DEFAULT_MDK_WALLET_PORT

  const ref = stableRef("wallet.mdk_port_conflict", result.stderr || result.stdout || "unknown")

  if (!isPortConflict) {
    return { isPortConflict: false, port, portConfigured, ref, message: null }
  }

  const message = [
    `MDK wallet daemon could not start: port ${port} is already in use.`,
    portConfigured
      ? `Another process is bound to 127.0.0.1:${port} (your configured MDK_WALLET_PORT).`
      : `Another local service (e.g. an Orange wallet webhook) is already bound to 127.0.0.1:${port}, the MDK default.`,
    `Set a free port and rerun, e.g.:`,
    `  MDK_WALLET_PORT=3458 pylon`,
    `Or stop the process already holding ${port} before starting the wallet daemon.`,
  ].join("\n")

  return { isPortConflict: true, port, portConfigured, ref, message }
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

/**
 * #5304: the Spark backup wallet is provisioned and ENABLED BY DEFAULT so a
 * fresh node is natively payable out of the box with zero manual commands. This
 * is the single canonical resolver for "is the Spark backup ON for this node?".
 *
 * Default: ON. An operator can still explicitly turn it OFF with either:
 *   - `PYLON_SPARK_BACKUP_DISABLED=1` (or `true`), or
 *   - `PYLON_SPARK_BACKUP_ENABLED=0` (or `false`).
 *
 * Any other value (unset, `1`, `true`, garbage) leaves it ON. The legacy
 * `PYLON_SPARK_BACKUP_ENABLED=1`/`true` opt-in is still honored (it just no
 * longer GATES the feature — it is now a no-op that confirms the default).
 *
 * NOTE: enabling the FEATURE is independent of whether a credential + seed are
 * present; those are checked downstream (`hasSparkBackupCredential`,
 * `resolveSparkBackupHelper`) so the projection still degrades to
 * `credential-missing` / `helper-unavailable` rather than crashing.
 */
export function isSparkBackupDefaultEnabled(env: NodeJS.ProcessEnv = process.env): boolean {
  const disabledFlag = env.PYLON_SPARK_BACKUP_DISABLED
  if (disabledFlag === "1" || disabledFlag === "true") return false
  const enabledFlag = env.PYLON_SPARK_BACKUP_ENABLED
  if (enabledFlag === "0" || enabledFlag === "false") return false
  return true
}

function isSparkBackupEnabled(options: SparkBackupReceiveOptions, env: NodeJS.ProcessEnv) {
  if (options.enabled !== undefined) return options.enabled
  // #5304: default-ON unless an explicit OFF override is set.
  return isSparkBackupDefaultEnabled(env)
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
  // #5194: distinguish "no in-process helper was wired" (resolver returned null,
  // so we fall back to the inert stub and NEVER attempt an SDK build) from "a
  // real SDK helper ran and failed". When the read is enabled but no helper was
  // injected, the operator was previously dead-ended on `helper-unavailable` /
  // `unknown` with empty stderr and no build attempt — exactly the silent
  // repro. Surface it explicitly under PYLON_SPARK_DEBUG so ONE run reveals it.
  const helperWired = options.helper !== undefined
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
  if (!helperReady) {
    // #5194: dump the raw result so an empty-stderr `unknown` is no longer a
    // dead-end — the gate line carries exitCode + stderrLen, and the explicit
    // `helperWired=false` note tells the operator the in-process SDK build was
    // never even attempted (resolver returned null → inert stub) versus a real
    // helper that ran and failed.
    dumpSparkHelperGate(
      `classify:${kind}`,
      helperResult,
      `helperWired=${helperWired}${helperWired ? "" : " (inert stub — no in-process SDK build attempted)"}`,
    )
  }
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

  // #5194: classify WHY the helper read failed (e.g. a corrupt local wallet DB
  // that fails storage init UPSTREAM of getInfo, a blocked network, or a module
  // that did not load) into a bounded, public-safe enum so the operator finally
  // sees a reason instead of a bare `helper-unavailable`. `helperReady && rawTarget`
  // was false above; a non-zero exit carries the (private) reason in stderr.
  const helperUnavailableReason = helperReady
    ? "no_result"
    : classifySparkHelperFailureReason(helperResult.stderr)
  return assertSparkBackupProjectionSafe({
    ...base,
    state: "helper-unavailable",
    credentialReady: true,
    helperReady: false,
    helperUnavailableReason,
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
  // #5196: the wait window elapsed without a definitive settle/fail — the send
  // may have gone through (slow/large Lightning send). The outcome is UNKNOWN, so
  // the node must NOT retry until it confirms failure; it verifies the balance /
  // re-reads status instead. Distinct from send-failed precisely to block a
  // double-spend retry.
  | "send-pending"
  | "sent"

export type SparkBackupSendProjection = {
  schema: "openagents.pylon.spark_backup_send.v0.1"
  enabled: boolean
  state: SparkBackupSendState
  confirmSend: boolean
  consentRequired: boolean
  amountSats: number | null
  feeSats: number | null
  // #5250: true when `feeSats` came from the PREPARED payment method because the
  // settled send result reported no/zero fee. Surfaces fee provenance so the
  // receipt reconciles (`amountSats + feeSats` == real balance delta) rather
  // than claiming `feeSats: 0` on a send that actually paid a Lightning/LSP fee.
  feeFromPrepared: boolean
  destinationRef: string | null
  // #5257: for a Lightning-Address send, the destination's bare DOMAIN (the part
  // after `@`, e.g. `bitnob.io`). PUBLIC-SAFE: the domain is the LNURL-pay
  // endpoint, not payment material — the full `name@domain` stays redacted to
  // `destinationRef`. Null for non-LA sends (bolt11/bolt12 and native Spark).
  // Surfaced so an operator can build a picture of which domains quote
  // extortionate Lightning fees and drive the per-domain fee policy.
  destinationDomain: string | null
  sparkPaymentRef: string | null
  transferRef: string | null
  // #5225: `spark_native` = a Spark→Spark send that routed natively (no Lightning
  // routing fee), distinct from a BOLT11 `payment_request` or an LNURL `lnurl_pay`.
  method: "payment_request" | "lnurl_pay" | "spark_native" | null
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
      // #5250: true when `feeSats` was taken from the PREPARED payment method
      // (`prepareResponse.paymentMethod`) because the settled send result
      // reported no/zero fee. Public-safe provenance so the receipt reconciles
      // (`amountSats + feeSats` matches the real balance delta) instead of
      // claiming `feeSats: 0` on a send that actually cost a Lightning/LSP fee.
      feeFromPrepared?: boolean
      // #5257: bare destination DOMAIN for a Lightning-Address send (public-safe
      // attribution; the full `name@domain` is never returned). Null/absent for
      // non-LA sends (bolt11/bolt12, native Spark).
      destinationDomain?: string | null
      // #5225: `spark_native` for a native Spark→Spark send (no Lightning routing fee).
      method: "payment_request" | "lnurl_pay" | "spark_native"
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
  // #5254: explicit operator override that RAISES the pre-send fee-guard
  // ceiling so a knowingly-expensive send can proceed. Undefined keeps the
  // default bound (the env PYLON_SPARK_MAX_FEE_SATS override is also consulted).
  maxFeeSats?: number
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
  // #5254: explicit operator override (from `--max-fee <sats>`) threaded to the
  // transfer so the pre-send fee guard can be raised for a knowingly-expensive
  // send. Default (undefined) keeps the guard at its computed bound.
  maxFeeSats?: number
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
  // #5194: when helperReady is false, a bounded public-safe reason the status
  // read failed (e.g. db_init_failed). Null when the helper is ready.
  helperUnavailableReason: SparkHelperUnavailableReason | null
  // #5197: true when the balance came from a non-forced fallback read (the
  // authoritative ensureSynced read failed/timed out, e.g. a fresh post-restart
  // sync). The number is shown but is NOT a confirmed-spendable balance.
  balanceRefreshing: boolean
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
    // #5194: same gate dump on the status path so a daemon-routed (or cold)
    // backup-status surfaces exitCode + stderrLen + the complete sanitized
    // stderr even when it is empty.
    dumpSparkHelperGate("detect:status", statusResult)
    return {
      helperReady: false,
      detectedBalanceSats: null,
      balanceRefreshing: false,
      unclaimedDepositCount: null,
      claimableHtlcCount: null,
      claimableHtlcSats: null,
      // #5194: surface WHY the status read failed (upstream of getInfo on a
      // corrupt-DB host) so a daemon-routed backup-status is no longer silent.
      helperUnavailableReason: classifySparkHelperFailureReason(statusResult.stderr),
    }
  }
  const statusData = parseMaybeJson(statusResult.stdout)
  const detectedBalanceSats =
    toSatNumber(statusData?.balance_sats) ?? toSatNumber(statusData?.spendable_balance_sats)
  // Absent (older helper) is treated as synced; only an explicit `false` flags it.
  const balanceRefreshing = statusData?.balance_synced === false
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

  return {
    helperReady: true,
    detectedBalanceSats,
    balanceRefreshing,
    unclaimedDepositCount,
    claimableHtlcCount,
    claimableHtlcSats,
    helperUnavailableReason: null,
  }
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
    feeFromPrepared: false,
    destinationRef,
    destinationDomain: null,
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
  const result = await transfer({
    amountSats,
    destination,
    idempotencyKey,
    // #5254: thread the operator fee-ceiling override (from `--max-fee`) so a
    // knowingly-expensive send can clear the pre-send fee guard.
    ...(options.maxFeeSats === undefined ? {} : { maxFeeSats: options.maxFeeSats }),
  })
  if (!result.ok) {
    // #5196: a timed-out send is INDETERMINATE — it may have settled. Mark it
    // pending and do NOT offer a retry action; the node must verify the balance /
    // re-read status before any retry, or it risks sending twice.
    const pending = result.failureRef.startsWith("wallet.spark_backup_send_indeterminate")
    if (pending) {
      return safe({
        ...base,
        state: "send-pending",
        consentRequired: false,
        blockerRefs: ["blocker.wallet.spark_backup.send_outcome_pending"],
        failureRefs: [result.failureRef],
        nextActionRefs: ["action.wallet.spark_backup.verify_balance_before_retry"],
      })
    }
    // #5257: a per-destination-domain policy refusal (operator deny list or a
    // per-domain fee bound) surfaces its OWN distinct, public-safe blocker plus a
    // raise-the-override next action. The failureRef carries the bare domain
    // (public-safe; never the full `name@domain`) as
    // `wallet.spark_backup_send.destination_fee_policy:<domain>`, so we key on it
    // directly and echo the domain into the projection for attribution. This is
    // pre-dispatch (zero sats move), composing cleanly with the #5254 magnitude
    // guard which is also pre-dispatch.
    const destinationFeePolicy =
      result.failureRef.startsWith("wallet.spark_backup_send") &&
      result.failureRef.includes("destination_fee_policy")
    if (destinationFeePolicy) {
      const policyDomain = result.failureRef.split("destination_fee_policy:")[1] ?? null
      return safe({
        ...base,
        state: "send-failed",
        consentRequired: false,
        ...(policyDomain && policyDomain.length > 0 ? { destinationDomain: policyDomain } : {}),
        blockerRefs: ["blocker.wallet.spark_backup.destination_fee_policy"],
        failureRefs: [result.failureRef],
        nextActionRefs: ["action.wallet.spark_backup.allow_destination_domain_or_adjust"],
      })
    }
    // #5254: a pre-send fee-guard rejection surfaces a DISTINCT, operator-legible
    // blocker + a raise-the-ceiling next action, rather than the generic
    // "fix the transfer and retry" guidance. The failureRef is public-safe
    // (integers only) and carries `fee_too_high`, so we key on it directly.
    const feeTooHigh =
      result.failureRef.startsWith("wallet.spark_backup_send") &&
      result.failureRef.includes("fee_too_high")
    if (feeTooHigh) {
      return safe({
        ...base,
        state: "send-failed",
        consentRequired: false,
        blockerRefs: ["blocker.wallet.spark_backup.send_fee_too_high"],
        failureRefs: [result.failureRef],
        nextActionRefs: ["action.wallet.spark_backup.raise_max_fee_or_adjust_amount"],
      })
    }
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
      feeFromPrepared: result.feeFromPrepared === true,
      destinationRef,
      // #5257: bare destination domain is public-safe attribution material.
      destinationDomain: result.destinationDomain ?? null,
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
    feeFromPrepared: result.feeFromPrepared === true,
    destinationDomain: result.destinationDomain ?? null,
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
