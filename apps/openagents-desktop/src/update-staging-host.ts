import { chmodSync, closeSync, existsSync, fsyncSync, mkdirSync, openSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs"
import { randomBytes } from "node:crypto"
import path from "node:path"

import {
  PRODUCTION_RELEASE_KEY_PIN,
  type PinnedReleaseKey,
  type UpdateChannel,
  type UpdateManifest,
  decodeLaunchHealthReceipt,
  isMonotonicUpgrade,
  verifyArtifactDigest,
  verifySignedUpdateManifest,
} from "./update-contract.ts"
import { assertCredentialFreeHttpsUrl, decodeUpdateManifest } from "./release-publish.ts"
import type { MacOSUpdateApplier } from "./macos-update-applier.ts"
import {
  type ApplicationArchitecture,
  type HostArchitecture,
  type HostPlatform,
  type ReleaseSetArtifact,
  decodeReleaseSetArtifact,
  V1_MIGRATION_END,
  selectReleaseArtifact,
  verifyReleaseSetArtifact,
  verifySignedReleaseSet,
} from "./release-set-contract.ts"
import { childRuntimeKinds, type ChildRuntimeDrainReceipt, type DesktopPlatformUpdateApplier } from "./update-platform-applier.ts"
import { initialUpdateState, migrationCategories, runUpdateEvents, type MigrationLedger } from "./update-rollback.ts"
import { decodeUpdateMigrationEvidence, migrationLedgerFromEvidence, type UpdateMigrationEvidence } from "./update-migration-evidence.ts"

const MAX_MANIFEST_BYTES = 32 * 1024
const MAX_SIGNATURE_BYTES = 8 * 1024
const MAX_RELEASE_BYTES = 16 * 1024
const MAX_RELEASE_SET_BYTES = 256 * 1024
const MAX_ARTIFACT_BYTES = 2 * 1024 * 1024 * 1024
const SAFE_FAILURE_REASONS = new Set(["feed_unavailable", "response_too_large"])

export type DesktopUpdateProjection = Readonly<{
  phase: "current" | "checking" | "available" | "downloading" | "staged" | "applying" | "restarting" | "rollback_available" | "rolling_back" | "rejected"
  channel: UpdateChannel
  installedVersion: string
  candidateVersion: string | null
  rollbackVersion: string | null
  reason: string | null
}>

/** Runtime B must boot to produce health; only an active/completed rollback exits startup. */
export const updateRecoveryRequiresStartupExit = (projection: DesktopUpdateProjection): boolean =>
  projection.phase === "rolling_back" || (projection.phase === "restarting" && projection.candidateVersion === null)

type UpdateDocument = Readonly<{
  version: 2
  channel: UpdateChannel
  installedVersion: string
  candidate: UpdateManifest | null
  releaseSetCandidate: ReleaseSetArtifact | null
  artifactUrl: string | null
  stagedArtifactName: string | null
  operation: "idle" | "staged" | "applying" | "awaiting_launch_receipt" | "awaiting_clean_shutdown" | "rolling_back" | "rollback_cleanup_pending" | "rollback_failed"
  previousVersion: string | null
  appliedAtMs: number | null
  launchTransactionRef: string | null
  rendererReadyAt: string | null
  providerReadyAt: string | null
  migrationEvidence: UpdateMigrationEvidence | null
  reason: string | null
}>

type ReleasePointer = Readonly<{
  channel: UpdateChannel
  version: string
  artifactName: string
  artifactUrl: string
}>

export type DesktopUpdateStagingHost = Readonly<{
  snapshot: () => DesktopUpdateProjection
  check: () => Promise<DesktopUpdateProjection>
  download: () => Promise<DesktopUpdateProjection>
  openInstaller: () => Promise<DesktopUpdateProjection>
  apply: () => Promise<DesktopUpdateProjection>
  rollback: () => Promise<DesktopUpdateProjection>
  reconcile: () => Promise<DesktopUpdateProjection>
  recordHealthyLaunch: (ready: Readonly<{ rendererReadyAt: string; providerReadyAt: string }>) => Promise<DesktopUpdateProjection>
  recordCleanShutdown: (drain: ChildRuntimeDrainReceipt) => boolean
}>

const decodeReleasePointer = (value: unknown): ReleasePointer | null => {
  if (typeof value !== "object" || value === null) return null
  const row = value as Record<string, unknown>
  if (
    (row.channel !== "stable" && row.channel !== "rc") ||
    typeof row.version !== "string" ||
    typeof row.artifactName !== "string" ||
    typeof row.artifactUrl !== "string"
  )
    return null
  try {
    assertCredentialFreeHttpsUrl(row.artifactUrl)
  } catch {
    return null
  }
  return row as ReleasePointer
}

const readDocument = (file: string, installedVersion: string, channel: UpdateChannel): UpdateDocument => {
  try {
    const raw = JSON.parse(readFileSync(file, "utf8")) as Record<string, unknown>
    const candidate = raw.candidate === null ? null : decodeUpdateManifest(raw.candidate)
    const version = raw.version === 1 ? 1 : raw.version === 2 ? 2 : null
    const releaseSetCandidate = version === 2 && raw.releaseSetCandidate !== null ? decodeReleaseSetArtifact(raw.releaseSetCandidate) : null
    if (
      version === null ||
      raw.channel !== channel ||
      typeof raw.installedVersion !== "string" ||
      (raw.candidate !== null && candidate === null) ||
      (version === 2 && raw.releaseSetCandidate !== null && releaseSetCandidate === null) ||
      (raw.artifactUrl !== null && typeof raw.artifactUrl !== "string") ||
      (raw.stagedArtifactName !== null && typeof raw.stagedArtifactName !== "string") ||
      (raw.stagedArtifactName !== null && raw.stagedArtifactName !== (candidate?.artifactName ?? releaseSetCandidate?.name)) ||
      (raw.reason !== null && typeof raw.reason !== "string")
    )
      throw new Error("invalid")
    if (typeof raw.artifactUrl === "string") assertCredentialFreeHttpsUrl(raw.artifactUrl)
    const operation =
      version === 2 && ["idle", "staged", "applying", "awaiting_launch_receipt", "awaiting_clean_shutdown", "rolling_back", "rollback_cleanup_pending", "rollback_failed"].includes(String(raw.operation))
        ? (raw.operation as UpdateDocument["operation"])
        : raw.stagedArtifactName === null
          ? "idle"
          : "staged"
    return {
      version: 2,
      channel,
      installedVersion: raw.installedVersion,
      candidate,
      releaseSetCandidate,
      artifactUrl: raw.artifactUrl as string | null,
      stagedArtifactName: raw.stagedArtifactName as string | null,
      operation,
      previousVersion: version === 2 && typeof raw.previousVersion === "string" ? raw.previousVersion : null,
      appliedAtMs: version === 2 && typeof raw.appliedAtMs === "number" && Number.isFinite(raw.appliedAtMs) ? raw.appliedAtMs : null,
      launchTransactionRef: version === 2 && typeof raw.launchTransactionRef === "string" && /^[0-9a-f]{32}$/.test(raw.launchTransactionRef) ? raw.launchTransactionRef : null,
      rendererReadyAt: version === 2 && typeof raw.rendererReadyAt === "string" ? raw.rendererReadyAt : null,
      providerReadyAt: version === 2 && typeof raw.providerReadyAt === "string" ? raw.providerReadyAt : null,
      migrationEvidence: version === 2 ? decodeUpdateMigrationEvidence(raw.migrationEvidence) : null,
      reason: raw.reason as string | null,
    }
  } catch {
    return {
      version: 2,
      channel,
      installedVersion,
      candidate: null,
      releaseSetCandidate: null,
      artifactUrl: null,
      stagedArtifactName: null,
      operation: "idle",
      previousVersion: null,
      appliedAtMs: null,
      launchTransactionRef: null,
      rendererReadyAt: null,
      providerReadyAt: null,
      migrationEvidence: null,
      reason: null,
    }
  }
}

const writeDocument = (file: string, value: UpdateDocument): void => {
  const parent = path.dirname(file)
  mkdirSync(parent, { recursive: true, mode: 0o700 })
  if (process.platform !== "win32") chmodSync(parent, 0o700)
  const temporary = `${file}.tmp`
  writeFileSync(temporary, JSON.stringify(value), { encoding: "utf8", mode: 0o600 })
  if (process.platform !== "win32") chmodSync(temporary, 0o600)
  const temporaryDescriptor = openSync(temporary, "r")
  try { fsyncSync(temporaryDescriptor) } finally { closeSync(temporaryDescriptor) }
  renameSync(temporary, file)
  if (process.platform !== "win32") {
    const parentDescriptor = openSync(parent, "r")
    try { fsyncSync(parentDescriptor) } finally { closeSync(parentDescriptor) }
  }
}

const boundedBytes = async (response: Response, maximum: number): Promise<Uint8Array> => {
  if (!response.ok) throw new Error("feed_unavailable")
  const declared = Number(response.headers.get("content-length"))
  if (Number.isFinite(declared) && declared > maximum) throw new Error("response_too_large")
  const bytes = new Uint8Array(await response.arrayBuffer())
  if (bytes.byteLength > maximum) throw new Error("response_too_large")
  return bytes
}

const publicFailureReason = (error: unknown, fallback: string): string =>
  error instanceof Error && SAFE_FAILURE_REASONS.has(error.message) ? error.message : fallback

export const openDesktopUpdateStagingHost = (
  input: Readonly<{
    root: string
    installedVersion: string
    channel: UpdateChannel
    fetch?: typeof globalThis.fetch
    pin?: PinnedReleaseKey
    openPath: (artifactPath: string) => Promise<string>
    applier?: (Pick<MacOSUpdateApplier, "rollbackAvailable" | "rollbackVersion" | "install" | "rollback"> & Partial<Pick<MacOSUpdateApplier, "rollbackCompletionStatus">>) | DesktopPlatformUpdateApplier
    platform?: HostPlatform
    hostArchitecture?: HostArchitecture
    applicationArchitecture?: ApplicationArchitecture
    hostVersion?: string
    now?: () => number
    drainChildren?: () => Promise<ChildRuntimeDrainReceipt>
    migrationEvidence?: () => UpdateMigrationEvidence | null
    restart?: () => void
    baseUrl?: string
  }>,
): DesktopUpdateStagingHost => {
  const fetchImpl = input.fetch ?? globalThis.fetch
  const pin = input.pin ?? PRODUCTION_RELEASE_KEY_PIN
  const baseUrl = input.baseUrl ?? `https://updates.openagents.com/desktop/openagents/${input.channel}`
  const platform = input.platform ?? (process.platform === "darwin" || process.platform === "win32" || process.platform === "linux" ? process.platform : null)
  const hostArchitecture = input.hostArchitecture ?? (process.arch === "arm64" || process.arch === "x64" ? process.arch : null)
  const applicationArchitecture = input.applicationArchitecture ?? (process.arch === "arm64" || process.arch === "x64" ? process.arch : null)
  const hostVersion = input.hostVersion ?? "0"
  const now = input.now ?? Date.now
  const documentFile = path.join(input.root, "state.json")
  const launchReceiptFile = path.join(input.root, "launch-receipt.json")
  const watchdogResultFile = path.join(input.root, "first-launch-watchdog.result")
  let document = readDocument(documentFile, input.installedVersion, input.channel)
  const interruptedName = document.candidate?.artifactName ?? document.releaseSetCandidate?.name
  if (interruptedName !== undefined) rmSync(path.join(input.root, `${interruptedName}.tmp`), { force: true })
  let transient: DesktopUpdateProjection["phase"] | null = null
  let mutationTail: Promise<void> = Promise.resolve()
  const serialized = <T>(work: () => Promise<T>): Promise<T> => {
    const run = mutationTail.then(work, work)
    mutationTail = run.then(() => undefined, () => undefined)
    return run
  }

  const stagedPath = (): string | null => (document.stagedArtifactName === null ? null : path.join(input.root, document.stagedArtifactName))
  const clearStaged = (): void => {
    const staged = stagedPath()
    if (staged !== null) rmSync(staged, { force: true })
  }
  const snapshot = (): DesktopUpdateProjection => ({
    phase:
      transient ??
      (document.reason !== null
        ? "rejected"
        : document.operation === "awaiting_launch_receipt" || document.operation === "awaiting_clean_shutdown"
          ? "restarting"
          : document.operation === "rolling_back"
            ? "rolling_back"
            : document.stagedArtifactName !== null && stagedPath() !== null && existsSync(stagedPath()!)
              ? "staged"
              : document.candidate !== null || document.releaseSetCandidate !== null
                ? "available"
                : input.applier?.rollbackAvailable() === true
                  ? "rollback_available"
                  : "current"),
    channel: input.channel,
    installedVersion: input.installedVersion,
    candidateVersion: document.candidate?.version ?? document.releaseSetCandidate?.version ?? null,
    rollbackVersion: input.applier?.rollbackVersion() ?? null,
    reason: document.reason,
  })
  const reject = (reason: string): DesktopUpdateProjection => {
    clearStaged()
    document = {
      ...document,
      candidate: null,
      releaseSetCandidate: null,
      artifactUrl: null,
      stagedArtifactName: null,
      operation: "idle",
      appliedAtMs: null,
      reason: reason.slice(0, 120),
    }
    writeDocument(documentFile, document)
    transient = null
    return snapshot()
  }
  const recoverAfterDrain = async (reason: string): Promise<DesktopUpdateProjection> => {
    if (input.applier?.rollbackAvailable() === true) {
      const rollback = await input.applier.rollback()
      if (!rollback.ok) {
        document = { ...document, operation: "rollback_failed", reason: "drain_recovery_rollback_failed" }
        writeDocument(documentFile, document)
        input.restart?.()
        transient = null
        return snapshot()
      }
    }
    const projection = reject(reason)
    input.restart?.()
    return projection
  }

  const checkInternal = async (): Promise<DesktopUpdateProjection> => {
    if (document.operation !== "idle") return snapshot()
    transient = "checking"
    try {
      if (platform === null || hostArchitecture === null || applicationArchitecture === null) {
        return reject("unsupported_host")
      }
      const [releaseSetResponse, releaseSetSignatureResponse] = await Promise.all([
        fetchImpl(`${baseUrl}/release-set.json`),
        fetchImpl(`${baseUrl}/release-set.sig.json`),
      ])
      if (releaseSetResponse.ok || releaseSetSignatureResponse.ok) {
        const releaseSetBytes = await boundedBytes(releaseSetResponse, MAX_RELEASE_SET_BYTES)
        const releaseSetSignatureBytes = await boundedBytes(releaseSetSignatureResponse, MAX_SIGNATURE_BYTES)
        let releaseSetSignature: unknown
        try {
          releaseSetSignature = JSON.parse(new TextDecoder().decode(releaseSetSignatureBytes))
        } catch {
          return reject("feed_schema_invalid")
        }
        const verifiedSet = verifySignedReleaseSet(releaseSetBytes, releaseSetSignature, pin, input.channel)
        if (!verifiedSet.ok) return reject(verifiedSet.reason)
        const selected = selectReleaseArtifact({
          releaseSet: verifiedSet.releaseSet,
          installedChannel: input.channel,
          installedVersion: input.installedVersion,
          platform,
          architecture: hostArchitecture,
          applicationArchitecture,
          hostVersion,
        })
        if (!selected.ok) return reject(selected.reason)
        clearStaged()
        document = {
          ...document,
          installedVersion: input.installedVersion,
          candidate: null,
          releaseSetCandidate: selected.artifact,
          artifactUrl: selected.artifact.url,
          stagedArtifactName: null,
          operation: "idle",
          previousVersion: null,
          appliedAtMs: null,
          reason: null,
        }
        writeDocument(documentFile, document)
        transient = null
        return snapshot()
      }
      // Bounded v1 compatibility is historical macOS arm64 only. It is never
      // used as a fallback for another target or architecture migration.
      if (platform !== "darwin" || hostArchitecture !== "arm64" || applicationArchitecture !== "arm64") {
        return reject("release_set_unavailable")
      }
      if (new Date(now()).toISOString() > V1_MIGRATION_END) return reject("v1_migration_expired")
      const [manifestResponse, signatureResponse, releaseResponse] = await Promise.all([
        fetchImpl(`${baseUrl}/manifest.json`),
        fetchImpl(`${baseUrl}/manifest.sig.json`),
        fetchImpl(`${baseUrl}/release.json`),
      ])
      const manifestBytes = await boundedBytes(manifestResponse, MAX_MANIFEST_BYTES)
      const signatureBytes = await boundedBytes(signatureResponse, MAX_SIGNATURE_BYTES)
      const releaseBytes = await boundedBytes(releaseResponse, MAX_RELEASE_BYTES)
      let signature: unknown
      let release: unknown
      try {
        signature = JSON.parse(new TextDecoder().decode(signatureBytes))
        release = JSON.parse(new TextDecoder().decode(releaseBytes))
      } catch {
        return reject("feed_schema_invalid")
      }
      const verified = verifySignedUpdateManifest(manifestBytes, signature, pin, input.channel)
      if (!verified.ok) return reject(verified.reason)
      const pointer = decodeReleasePointer(release)
      if (
        pointer === null ||
        pointer.channel !== verified.manifest.channel ||
        pointer.version !== verified.manifest.version ||
        pointer.artifactName !== verified.manifest.artifactName
      ) {
        return reject("release_pointer_mismatch")
      }
      if (!isMonotonicUpgrade(input.installedVersion, verified.manifest.version, input.channel).admissible) {
        clearStaged()
        document = {
          ...document,
          candidate: null,
          releaseSetCandidate: null,
          artifactUrl: null,
          stagedArtifactName: null,
          operation: "idle",
          reason: null,
        }
      } else {
        clearStaged()
        document = {
          ...document,
          installedVersion: input.installedVersion,
          candidate: verified.manifest,
          releaseSetCandidate: null,
          artifactUrl: pointer.artifactUrl,
          stagedArtifactName: null,
          operation: "idle",
          reason: null,
        }
      }
      writeDocument(documentFile, document)
      transient = null
      return snapshot()
    } catch (error) {
      return reject(publicFailureReason(error, "update_check_failed"))
    }
  }

  const downloadInternal = async (): Promise<DesktopUpdateProjection> => {
    const candidate = document.candidate ?? document.releaseSetCandidate
    if (candidate === null || document.artifactUrl === null || document.operation !== "idle") return snapshot()
    transient = "downloading"
    try {
      const byteLength = document.candidate?.artifactByteLength ?? document.releaseSetCandidate?.byteLength ?? 0
      if (byteLength > MAX_ARTIFACT_BYTES) return reject("artifact_too_large")
      const bytes = await boundedBytes(await fetchImpl(document.artifactUrl), MAX_ARTIFACT_BYTES)
      const verified =
        document.candidate !== null
          ? verifyArtifactDigest(document.candidate, bytes)
          : document.releaseSetCandidate !== null && verifyReleaseSetArtifact(document.releaseSetCandidate, bytes)
      if (!verified) return reject("artifact_rejected")
      mkdirSync(input.root, { recursive: true, mode: 0o700 })
      const name = document.candidate?.artifactName ?? document.releaseSetCandidate?.name
      if (name === undefined) return reject("artifact_rejected")
      const temporary = path.join(input.root, `${name}.tmp`)
      writeFileSync(temporary, bytes, { mode: 0o600 })
      const destination = path.join(input.root, name)
      renameSync(temporary, destination)
      document = { ...document, stagedArtifactName: name, operation: "staged", reason: null }
      writeDocument(documentFile, document)
      transient = null
      return snapshot()
    } catch (error) {
      return reject(publicFailureReason(error, "update_download_failed"))
    }
  }

  const openInstallerInternal = async (): Promise<DesktopUpdateProjection> => {
    const artifact = stagedPath()
    if (artifact === null || !existsSync(artifact)) return snapshot()
    const error = await input.openPath(artifact)
    if (error !== "") return reject("installer_open_failed")
    return snapshot()
  }

  const applyInternal = async (): Promise<DesktopUpdateProjection> => {
    const artifact = stagedPath()
    const candidateVersion = document.candidate?.version ?? document.releaseSetCandidate?.version ?? null
    if (artifact === null || candidateVersion === null || !existsSync(artifact)) return snapshot()
    if (input.applier === undefined) return reject("update_apply_unavailable")
    if (hostArchitecture === null || applicationArchitecture === null) return reject("unsupported_host")
    if (
      document.releaseSetCandidate !== null &&
      "target" in input.applier &&
      (input.applier.target !== document.releaseSetCandidate.target || input.applier.format !== document.releaseSetCandidate.format)
    )
      return reject("platform_applier_mismatch")
    transient = "applying"
    try {
      const evidence = input.migrationEvidence?.()
      if (evidence === undefined || evidence === null) return reject("migration_evidence_unavailable")
      const ledger: MigrationLedger = migrationLedgerFromEvidence(evidence)
      const manifestForReducer: UpdateManifest = document.candidate ?? {
        schema: "openagents.desktop.update_manifest.v1",
        app: "openagents-desktop",
        channel: input.channel,
        version: candidateVersion,
        artifactName: document.releaseSetCandidate?.name ?? "candidate.bin",
        artifactSha256: document.releaseSetCandidate?.sha256 ?? "0".repeat(64),
        artifactByteLength: document.releaseSetCandidate?.byteLength ?? 1,
        releasedAt: new Date(now()).toISOString(),
      }
      const admission = runUpdateEvents(initialUpdateState(input.installedVersion, input.channel), [
        { type: "check_started" },
        { type: "manifest_verified", manifest: manifestForReducer },
        { type: "artifact_verified" },
        ...migrationCategories.map((category) => ({ type: "migration_recorded", category, disposition: ledger[category] }) as const),
        { type: "staged" },
        { type: "apply_requested" },
      ])
      if (admission.state.phase !== "applying" || admission.refusals.length > 0) {
        return reject(admission.refusals[0] ?? "migration_ledger_incomplete")
      }
      if (input.drainChildren !== undefined) {
        const drain = await input.drainChildren()
        if (!drain.ok) return recoverAfterDrain("child_runtime_drain_timeout")
      }
      document = {
        ...document,
        operation: "applying",
        previousVersion: input.installedVersion,
        migrationEvidence: evidence,
        reason: null,
      }
      writeDocument(documentFile, document)
      const result = await input.applier.install(artifact, candidateVersion, hostArchitecture)
      if (!result.ok) return recoverAfterDrain(result.reason)
      clearStaged()
      rmSync(launchReceiptFile, { force: true })
      const appliedAtMs = now()
      const launchTransactionRef = randomBytes(16).toString("hex")
      document = {
        ...document,
        installedVersion: candidateVersion,
        artifactUrl: null,
        stagedArtifactName: null,
        operation: "awaiting_launch_receipt",
        previousVersion: result.previousVersion,
        appliedAtMs,
        launchTransactionRef,
        rendererReadyAt: null,
        providerReadyAt: null,
        reason: document.reason,
      }
      writeDocument(documentFile, document)
      const retainedSlot = "rollbackClaim" in input.applier && input.applier.rollbackClaim === "retained_slot"
      if (retainedSlot && input.applier.armFirstLaunchRollback === undefined) return await recoverAfterDrain("first_launch_watchdog_unavailable")
      if ("armFirstLaunchRollback" in input.applier && input.applier.armFirstLaunchRollback !== undefined && result.previousVersion !== null) {
        const armed = await input.applier.armFirstLaunchRollback({
          receiptPath: launchReceiptFile,
          expectedVersion: candidateVersion,
          transactionRef: launchTransactionRef,
          previousVersion: result.previousVersion,
          previousArchitecture: applicationArchitecture,
          deadlineMs: appliedAtMs + 10 * 60 * 1000,
        })
        if (!armed) {
          document = { ...document, operation: "rolling_back", reason: "watchdog_failed" }
          writeDocument(documentFile, document)
          const rolledBack = await input.applier.rollback()
          if (!rolledBack.ok) {
            document = { ...document, operation: "rollback_failed", reason: "watchdog_and_rollback_failed" }
            writeDocument(documentFile, document)
            input.restart?.()
            transient = null
            return snapshot()
          }
          input.restart?.()
          return reject("first_launch_watchdog_unavailable")
        }
      }
      transient = "restarting"
      input.restart?.()
      return snapshot()
    } catch {
      input.restart?.()
      return reject("update_apply_failed")
    }
  }

  const rollbackInternal = async (): Promise<DesktopUpdateProjection> => {
    if (input.applier?.rollbackAvailable() !== true) return snapshot()
    transient = "rolling_back"
    const rollbackReason = document.reason
    try {
      document = { ...document, operation: "rolling_back" }
      writeDocument(documentFile, document)
      const result = await input.applier.rollback()
      if (!result.ok) return reject(result.reason)
      clearStaged()
      document = {
        version: 2,
        channel: input.channel,
        installedVersion: result.installedVersion,
        candidate: null,
        releaseSetCandidate: null,
        artifactUrl: null,
        stagedArtifactName: null,
        operation: "idle",
        previousVersion: null,
        appliedAtMs: null,
        launchTransactionRef: null,
        rendererReadyAt: null,
        providerReadyAt: null,
        migrationEvidence: null,
        reason: rollbackReason,
      }
      writeDocument(documentFile, document)
      transient = "restarting"
      input.restart?.()
      return snapshot()
    } catch {
      return reject("rollback_failed")
    }
  }

  const reconcileInternal = async (): Promise<DesktopUpdateProjection> => {
    const completeAutomaticRollback = (): DesktopUpdateProjection => {
      if (document.operation !== "rollback_cleanup_pending") {
        document = {
          version: 2,
          channel: input.channel,
          installedVersion: input.installedVersion,
          candidate: null,
          releaseSetCandidate: null,
          artifactUrl: null,
          stagedArtifactName: null,
          operation: "rollback_cleanup_pending",
          previousVersion: null,
          appliedAtMs: null,
          launchTransactionRef: null,
          rendererReadyAt: null,
          providerReadyAt: null,
          migrationEvidence: null,
          reason: null,
        }
        // This durable common-state bridge MUST precede both destructive
        // cleanups; restart can resume idempotently from any following point.
        writeDocument(documentFile, document)
      }
      rmSync(path.join(input.root, "apply-transaction.json"), { force: true })
      rmSync(path.join(input.root, "rollback"), { recursive: true, force: true })
      document = { ...document, operation: "idle" }
      writeDocument(documentFile, document)
      return snapshot()
    }
    if (document.operation === "rollback_cleanup_pending") return completeAutomaticRollback()
    // Native terminal authority must be consumed before interrupted-operation
    // fallback: manual rollback can die after publishing rolled_back, when the
    // retained slot is already unavailable by design.
    if (input.applier?.rollbackCompletionStatus?.() === "rolled_back") return completeAutomaticRollback()
    if (document.operation === "applying" || document.operation === "rolling_back") {
      return input.applier?.rollbackAvailable() === true ? rollbackInternal() : reject("interrupted_apply_recovery_unavailable")
    }
    if (document.operation === "rollback_failed") return snapshot()
    if ((document.operation !== "awaiting_launch_receipt" && document.operation !== "awaiting_clean_shutdown") || document.appliedAtMs === null) {
      return snapshot()
    }
    // Power loss can occur after the native selector but before its watchdog
    // diagnostic. The durable native transaction is authoritative: a terminal
    // rollback completes directly; rollback_prepared with the old app running
    // safely replays the idempotent retained-slot rollback.
    if (document.previousVersion === input.installedVersion) {
      if (input.applier?.rollbackCompletionStatus?.() === "rolled_back") return completeAutomaticRollback()
      if (input.applier?.rollbackAvailable() === true) return rollbackInternal()
    }
    try {
      const watchdogResult = readFileSync(watchdogResultFile, "utf8").trim()
      if (watchdogResult === "rollback_failed") {
        document = { ...document, operation: "rollback_failed", reason: "watchdog_rollback_failed" }
        writeDocument(documentFile, document)
        return snapshot()
      }
      if (watchdogResult === "rolled_back" && document.previousVersion === input.installedVersion) {
        return completeAutomaticRollback()
      }
    } catch {
      /* watchdog has not completed */
    }
    let receipt = null
    try {
      receipt = decodeLaunchHealthReceipt(JSON.parse(readFileSync(launchReceiptFile, "utf8")))
    } catch {
      /* absent is typed below */
    }
    const expectedVersion = document.candidate?.version ?? document.releaseSetCandidate?.version ?? null
    if (expectedVersion === null || document.launchTransactionRef === null || document.migrationEvidence === null) return reject("update_state_invalid")
    if (receipt === null) return snapshot()
    if (receipt.version !== expectedVersion || receipt.transactionRef !== document.launchTransactionRef) return snapshot()
    document = {
      ...document,
      candidate: null,
      releaseSetCandidate: null,
      operation: "idle",
      appliedAtMs: null,
      launchTransactionRef: null,
      rendererReadyAt: null,
      providerReadyAt: null,
      migrationEvidence: null,
      reason: null,
    }
    writeDocument(documentFile, document)
    transient = null
    return snapshot()
  }

  const recordHealthyLaunchInternal = async (ready: Readonly<{ rendererReadyAt: string; providerReadyAt: string }>): Promise<DesktopUpdateProjection> => {
    if (document.operation !== "awaiting_launch_receipt") return snapshot()
    const expectedVersion = document.candidate?.version ?? document.releaseSetCandidate?.version
    if (expectedVersion === undefined || input.installedVersion !== expectedVersion || document.launchTransactionRef === null) return snapshot()
    if (!Number.isFinite(Date.parse(ready.rendererReadyAt)) || !Number.isFinite(Date.parse(ready.providerReadyAt))) return reject("launch_health_invalid")
    document = { ...document, operation: "awaiting_clean_shutdown", rendererReadyAt: ready.rendererReadyAt, providerReadyAt: ready.providerReadyAt }
    writeDocument(documentFile, document)
    transient = "restarting"
    return snapshot()
  }

  const recordCleanShutdown = (drain: ChildRuntimeDrainReceipt): boolean => {
    if (!drain.ok || drain.timedOut.length > 0 || !childRuntimeKinds.every(kind => drain.drained.includes(kind))) return false
    if (document.operation !== "awaiting_clean_shutdown" || document.launchTransactionRef === null || document.rendererReadyAt === null || document.providerReadyAt === null) return false
    const expectedVersion = document.candidate?.version ?? document.releaseSetCandidate?.version
    if (expectedVersion === undefined || expectedVersion !== input.installedVersion) return false
    try {
      mkdirSync(input.root, { recursive: true, mode: 0o700 })
      const temporary = `${launchReceiptFile}.tmp`
      writeFileSync(temporary, JSON.stringify({ schema: "openagents.desktop.launch_health.v1", app: "openagents-desktop", version: expectedVersion, transactionRef: document.launchTransactionRef, rendererReadyAt: document.rendererReadyAt, providerReadyAt: document.providerReadyAt, cleanShutdownAt: new Date(now()).toISOString() }), { encoding: "utf8", mode: 0o600 })
      if (process.platform !== "win32") chmodSync(temporary, 0o600)
      const temporaryDescriptor = openSync(temporary, "r")
      try { fsyncSync(temporaryDescriptor) } finally { closeSync(temporaryDescriptor) }
      renameSync(temporary, launchReceiptFile)
      const parentDescriptor = openSync(input.root, "r")
      try { fsyncSync(parentDescriptor) } finally { closeSync(parentDescriptor) }
      return true
    } catch {
      return false
    }
  }

  const check = (): Promise<DesktopUpdateProjection> => serialized(checkInternal)
  const download = (): Promise<DesktopUpdateProjection> => serialized(downloadInternal)
  const openInstaller = (): Promise<DesktopUpdateProjection> => serialized(openInstallerInternal)
  const apply = (): Promise<DesktopUpdateProjection> => serialized(applyInternal)
  const rollback = (): Promise<DesktopUpdateProjection> => serialized(rollbackInternal)
  const reconcile = (): Promise<DesktopUpdateProjection> => serialized(reconcileInternal)
  const recordHealthyLaunch = (ready: Readonly<{ rendererReadyAt: string; providerReadyAt: string }>): Promise<DesktopUpdateProjection> => serialized(() => recordHealthyLaunchInternal(ready))

  return {
    snapshot,
    check,
    download,
    openInstaller,
    apply,
    rollback,
    reconcile,
    recordHealthyLaunch,
    recordCleanShutdown,
  }
}
