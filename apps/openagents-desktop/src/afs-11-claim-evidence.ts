/**
 * AFS-11 claim→rung→proof evidence ledger (GitHub issue #9089).
 *
 * AFS-11 asks for the complete packaged and release evidence of the Apple FM
 * router version-one system, and it demands that every product claim stays at
 * its honest rung. This module is the typed, mechanically checked record of
 * that promise. It maps each version-one product claim from the plan's
 * "Version-one cut line" section
 * (`docs/sol/2026-07-20-apple-fm-router-to-full-agent-system-plan.md`) to the
 * proof that supports it and to the honest evidence rung the claim actually
 * stands on.
 *
 * The invariant, enforced by `afs-11-claim-evidence.test.ts`, is that no claim
 * sits above its actual evidence:
 *
 *   - Every claim cites at least one proof.
 *   - Every cited proof file exists in the repository.
 *   - A claim on an achieved rung (unit-tested, integration-proven, or
 *     packaged-proven) cites at least one proof of that kind that actually ran
 *     and passed. Its rung is never higher than the strongest passing proof.
 *   - The `owner-signing-pending` rung is reserved for the release outcome that
 *     an autonomous agent must not assert. Its cited signing proof is a
 *     refused-or-not-run owner ceremony, never a passing result, and it must
 *     point at the owner action ledger.
 *
 * The runnable proofs already exist and pass; the signed and notarized
 * installed-application proof stays owner-reserved. This ledger records that
 * split honestly. It never claims a signed-release proof that did not run.
 *
 * This module is a pure typed record plus pure validators, in the same style as
 * the sibling `release-preflight.ts` and `mvp-proof.ts` proof modules. It has no
 * platform, provider, or store dependency and never touches secrets.
 */

/** The four honest rungs a version-one claim can stand on. */
export type EvidenceRung =
  | "unit-tested"
  | "integration-proven"
  | "packaged-proven"
  | "owner-signing-pending"

/** Increasing evidence strength. `owner-signing-pending` is the reserved top:
 *  it is the installed, signed application proof that an agent cannot run. */
export const rungOrder: Readonly<Record<EvidenceRung, number>> = {
  "unit-tested": 1,
  "integration-proven": 2,
  "packaged-proven": 3,
  "owner-signing-pending": 4,
}

/** What kind of evidence a proof provides. */
export type ProofKind = "unit" | "integration" | "packaged" | "owner-signing"

/** How a proof actually resolved. `refused-owner-gated` is an honest,
 *  expected outcome: the release lane fails closed when signing credentials
 *  are absent. `not-run` marks the owner ceremony that has not happened. */
export type ProofResult = "pass" | "refused-owner-gated" | "not-run"

const proofKindOrder: Readonly<Record<ProofKind, number>> = {
  unit: 1,
  integration: 2,
  packaged: 3,
  "owner-signing": 4,
}

export type ClaimProof = Readonly<{
  /** Repository-relative path to the proof file. The test asserts it exists. */
  ref: string
  kind: ProofKind
  /** True when this proof executes and produces a verdict in the normal sweep
   *  or was run by hand for this packet. */
  ran: boolean
  result: ProofResult
  detail: string
}>

export type ClaimRecord = Readonly<{
  id: string
  /** The version-one claim text, quoted from the cut line. */
  claim: string
  rung: EvidenceRung
  proofs: ReadonlyArray<ClaimProof>
  /** Set only on an `owner-signing-pending` claim. It points at the owner
   *  action ledger step that the owner must complete before the claim is met. */
  blockedOnOwner?: boolean
  ownerReservedStepRef?: string
}>

/**
 * The exact version-one capability identifiers, drawn verbatim from the plan's
 * "Version-one cut line" capability list. The ledger must cover every one, so a
 * later change cannot silently drop a claim from the release evidence.
 */
export const versionOneCapabilityIds = [
  "C-TURN-GRAPH",
  "C-FM-ANSWER",
  "C-FM-ROUTE",
  "C-DELEGATE-CODEX",
  "C-CARD-STATE",
  "C-MSG-CHAIN",
  "C-EDITOR-CONTEXT",
  "C-IDE08-PROPOSALS",
  "C-IDE-ADAPTERS",
  "C-LOCAL-STORAGE",
  "C-NO-CLOUD",
] as const

/** The reserved release-outcome claim. It is the AFS-11 outcome itself. */
export const releaseOutcomeClaimId = "C-SIGNED-RELEASE"

/**
 * The path, relative to the workspace root, of the owner action ledger step for
 * the signed and notarized installed-application proof. The workspace
 * `NEEDS_OWNER.md` file records the exact owner steps; this ledger only points
 * at it so the reserved rung always names a real owner action.
 */
export const ownerSigningLedgerRef = "NEEDS_OWNER.md#afs-11-signed-notarized-installed-app-proof"

export const afs11ClaimLedger: ReadonlyArray<ClaimRecord> = [
  {
    id: "C-TURN-GRAPH",
    claim: "One canonical local Desktop turn graph.",
    rung: "integration-proven",
    proofs: [
      {
        ref: "apps/openagents-desktop/src/afs-baseline-regression.test.ts",
        kind: "integration",
        ran: true,
        result: "pass",
        detail:
          "Freezes the AFS-00 baseline: the local chat answer path and the explicit provider path both reach a terminal journal disposition through the real shared engine.",
      },
      {
        ref: "apps/openagents-desktop/src/turn/desktop-turn-main.test.ts",
        kind: "integration",
        ran: true,
        result: "pass",
        detail: "Drives the canonical Desktop turn from intent to terminal state through the host wiring.",
      },
      {
        ref: "apps/openagents-desktop/src/turn/desktop-turn-journal.test.ts",
        kind: "integration",
        ran: true,
        result: "pass",
        detail: "Proves the local turn journal records the ordered turn graph.",
      },
    ],
  },
  {
    id: "C-FM-ANSWER",
    claim: "Apple FM local answers.",
    rung: "integration-proven",
    proofs: [
      {
        ref: "apps/openagents-desktop/src/apple-fm-host.test.ts",
        kind: "integration",
        ran: true,
        result: "pass",
        detail: "Exercises the Apple FM host lifecycle and a local completed answer through the launcher and probe.",
      },
      {
        ref: "apps/openagents-desktop/src/apple-fm-contract.test.ts",
        kind: "unit",
        ran: true,
        result: "pass",
        detail: "Validates the Apple FM contract decode and bounds.",
      },
    ],
  },
  {
    id: "C-FM-ROUTE",
    claim: "Apple FM route recommendations.",
    rung: "integration-proven",
    proofs: [
      {
        ref: "apps/openagents-desktop/src/turn/apple-fm-prompt.test.ts",
        kind: "integration",
        ran: true,
        result: "pass",
        detail: "Proves the bounded local route recommendation prompt and decode. An invalid recommendation has no route effect.",
      },
      {
        ref: "apps/openagents-desktop/src/turn/desktop-turn-main.test.ts",
        kind: "integration",
        ran: true,
        result: "pass",
        detail: "Proves the host records a RouteDecision and never adds a lane only because the model named it.",
      },
    ],
  },
  {
    id: "C-DELEGATE-CODEX",
    claim: "Host-selected delegation to one ready codex-local lane.",
    rung: "integration-proven",
    proofs: [
      {
        ref: "apps/openagents-desktop/src/turn/desktop-delegation.test.ts",
        kind: "integration",
        ran: true,
        result: "pass",
        detail: "Proves host-selected delegation to the ready codex-local lane.",
      },
      {
        ref: "apps/openagents-desktop/src/turn/desktop-codex-provider.test.ts",
        kind: "integration",
        ran: true,
        result: "pass",
        detail: "Proves the codex-local provider adapter behavior.",
      },
    ],
  },
  {
    id: "C-CARD-STATE",
    claim: "A local running, done, failed, refused, or cancelled card.",
    rung: "integration-proven",
    proofs: [
      {
        ref: "apps/openagents-desktop/src/renderer/runtime-cards.test.ts",
        kind: "unit",
        ran: true,
        result: "pass",
        detail: "Proves the running, done, and failed card presentation states.",
      },
      {
        ref: "apps/openagents-desktop/src/turn/desktop-turn-main.test.ts",
        kind: "integration",
        ran: true,
        result: "pass",
        detail: "Proves the done and refused terminal card states through the host.",
      },
      {
        ref: "apps/openagents-desktop/src/turn/desktop-provider-lane.test.ts",
        kind: "integration",
        ran: true,
        result: "pass",
        detail: "Proves the refused and recovered card states in the provider lane.",
      },
    ],
  },
  {
    id: "C-MSG-CHAIN",
    claim: "A local right-pane message chain.",
    rung: "integration-proven",
    proofs: [
      {
        ref: "apps/openagents-desktop/src/renderer/react-timeline.test.tsx",
        kind: "unit",
        ran: true,
        result: "pass",
        detail: "Proves the right-pane message chain rendering.",
      },
      {
        ref: "apps/openagents-desktop/src/turn/desktop-delegation.test.ts",
        kind: "integration",
        ran: true,
        result: "pass",
        detail: "Proves a delegated card opens its message chain in the right pane (AFS-04 follow-up).",
      },
    ],
  },
  {
    id: "C-EDITOR-CONTEXT",
    claim: "Editor context and answer candidates while the file stays visible.",
    rung: "integration-proven",
    proofs: [
      {
        ref: "apps/openagents-desktop/src/turn/editor-context-binding.test.ts",
        kind: "integration",
        ran: true,
        result: "pass",
        detail: "Proves editor context binding while the file stays visible.",
      },
      {
        ref: "apps/openagents-desktop/src/turn/editor-context-join.test.ts",
        kind: "integration",
        ran: true,
        result: "pass",
        detail: "Proves the editor context join into answer candidates.",
      },
    ],
  },
  {
    id: "C-IDE08-PROPOSALS",
    claim: "IDE-08 proposals for all requested file changes.",
    rung: "integration-proven",
    proofs: [
      {
        ref: "apps/openagents-desktop/src/ide/agent-code-contract.test.ts",
        kind: "unit",
        ran: true,
        result: "pass",
        detail: "Validates the IDE-08 agent-code proposal contract.",
      },
      {
        ref: "apps/openagents-desktop/src/ide/agent-code-host.test.ts",
        kind: "integration",
        ran: true,
        result: "pass",
        detail: "Proves the IDE-08 host produces proposals for requested file changes.",
      },
      {
        ref: "apps/openagents-desktop/scripts/ide-agent-code-packaged-journey.ts",
        kind: "packaged",
        ran: false,
        result: "not-run",
        detail:
          "A packaged journey exists. It runs against a built application from package:mac and belongs to the owner-gated packaged and signed proof.",
      },
    ],
  },
  {
    id: "C-IDE-ADAPTERS",
    claim: "IDE-10, IDE-11, and IDE-12 adapters for actions and evidence.",
    rung: "integration-proven",
    proofs: [
      {
        ref: "apps/openagents-desktop/src/ide/run-host.test.ts",
        kind: "integration",
        ran: true,
        result: "pass",
        detail: "Proves the IDE-10 run adapter actions and evidence.",
      },
      {
        ref: "apps/openagents-desktop/src/ide/dap-host.test.ts",
        kind: "integration",
        ran: true,
        result: "pass",
        detail: "Proves the IDE-11 debug adapter host over DAP.",
      },
      {
        ref: "apps/openagents-desktop/src/ide/source-control-host.test.ts",
        kind: "integration",
        ran: true,
        result: "pass",
        detail: "Proves the IDE-12 source control adapter host.",
      },
      {
        ref: "apps/openagents-desktop/src/ide/source-control-git-adapter.test.ts",
        kind: "integration",
        ran: true,
        result: "pass",
        detail: "Proves the IDE-12 Git adapter behavior.",
      },
    ],
  },
  {
    id: "C-LOCAL-STORAGE",
    claim: "Private local turn, card, message-chain, and recovery storage.",
    rung: "integration-proven",
    proofs: [
      {
        ref: "apps/openagents-desktop/src/turn/desktop-turn-journal.test.ts",
        kind: "integration",
        ran: true,
        result: "pass",
        detail: "Proves the private local turn and card journal storage.",
      },
      {
        ref: "apps/openagents-desktop/tests/desktop-session-recovery.test.ts",
        kind: "integration",
        ran: true,
        result: "pass",
        detail: "Proves recovery from private local session storage.",
      },
      {
        ref: "apps/openagents-desktop/tests/local-turn-restart.e2e.test.ts",
        kind: "integration",
        ran: true,
        result: "pass",
        detail: "Proves the local turn and message-chain state survives an application restart.",
      },
    ],
  },
  {
    id: "C-NO-CLOUD",
    claim: "No D1, R2, Worker, Cloudflare, or other OpenAgents cloud dependency.",
    rung: "integration-proven",
    proofs: [
      {
        ref: "scripts/check-afs-boundaries.ts",
        kind: "integration",
        ran: true,
        result: "pass",
        detail:
          "The AFS package-boundary check refuses a cloud client, provider SDK, SQL driver, or app import in the AFS root packages.",
      },
      {
        ref: "apps/openagents-desktop/tests/startup-contract.test.ts",
        kind: "integration",
        ran: true,
        result: "pass",
        detail: "Proves the local startup contract without a cloud dependency.",
      },
      {
        ref: "apps/openagents-desktop/scripts/release-preflight.ts",
        kind: "packaged",
        ran: true,
        result: "pass",
        detail:
          "The release preflight no_source_checkout_paths oracle passed against the built artifact set, so the staged application carries no absolute source-checkout runtime dependency.",
      },
    ],
  },
  {
    id: "C-PACKAGE-STAGING",
    claim:
      "The version-one application stages, packages, and passes the release contract up to the owner signing ceremony.",
    rung: "packaged-proven",
    proofs: [
      {
        ref: "apps/openagents-desktop/scripts/release-preflight.ts",
        kind: "packaged",
        ran: true,
        result: "pass",
        detail:
          "Run for #9089 against the built dist artifact set. Green rows: clean_origin_main, version_monotonic, attribution_intact, app_identity_stable, artifact_set_complete, no_upstream_updater_remnants, no_legacy_ui_entrypoints, no_source_checkout_paths. The only red row is signing_credentials_present, which is the owner-reserved ceremony below.",
      },
      {
        ref: "apps/openagents-desktop/src/isolated-app-proof.ts",
        kind: "packaged",
        ran: true,
        result: "pass",
        detail:
          "The double-gated isolated-app-proof mode exercises local coding surfaces with Chromium's mock keychain and no real signing. Its scoping logic proof ran green in isolation (10 of 10).",
      },
      {
        ref: "apps/openagents-desktop/src/isolated-app-proof.test.ts",
        kind: "integration",
        ran: true,
        result: "pass",
        detail: "Ran in isolation for #9089: 10 of 10 tests passed. It proves the isolated proof profile never reads the operator's real history.",
      },
    ],
  },
  {
    id: releaseOutcomeClaimId,
    claim:
      "The complete local-first version-one system is proven from an installed, signed, and notarized application.",
    rung: "owner-signing-pending",
    blockedOnOwner: true,
    ownerReservedStepRef: ownerSigningLedgerRef,
    proofs: [
      {
        ref: "apps/openagents-desktop/scripts/release-preflight.ts",
        kind: "owner-signing",
        ran: true,
        result: "refused-owner-gated",
        detail:
          "The signing_credentials_present oracle REFUSED because the owner-held Developer ID identity and ASC_API notary credentials are absent. There is no unsigned release fallback. This is the honest, expected fail-closed state.",
      },
      {
        ref: "apps/oa-updates/docs/release-signing-runbook.md",
        kind: "owner-signing",
        ran: false,
        result: "not-run",
        detail:
          "The signed and notarized installed-application proof is an owner ceremony. It requires the owner-held Developer ID and notary credentials and cannot be run autonomously. See the workspace NEEDS_OWNER ledger for the exact owner steps.",
      },
    ],
  },
]

// ---------------------------------------------------------------------------
// Pure validators
// ---------------------------------------------------------------------------

export type LedgerViolation = Readonly<{
  claimId: string
  rule: string
  detail: string
}>

/** The strongest rung a claim earns from proofs that actually ran and passed. */
export const strongestPassingRung = (record: ClaimRecord): number => {
  const passing = record.proofs.filter((proof) => proof.ran && proof.result === "pass")
  return passing.reduce((max, proof) => Math.max(max, proofKindOrder[proof.kind]), 0)
}

/**
 * Returns every way the ledger fails the "no claim above its evidence" rule.
 * An empty array means the ledger is honest. File existence is checked
 * separately by the test, which has filesystem access.
 */
export const ledgerViolations = (ledger: ReadonlyArray<ClaimRecord>): ReadonlyArray<LedgerViolation> => {
  const violations: LedgerViolation[] = []
  for (const record of ledger) {
    if (record.proofs.length === 0) {
      violations.push({ claimId: record.id, rule: "has-proof", detail: "claim cites no proof" })
      continue
    }
    if (record.rung === "owner-signing-pending") {
      if (record.blockedOnOwner !== true) {
        violations.push({
          claimId: record.id,
          rule: "owner-pending-blocked",
          detail: "an owner-signing-pending claim must be marked blockedOnOwner",
        })
      }
      if (record.ownerReservedStepRef === undefined || record.ownerReservedStepRef.trim() === "") {
        violations.push({
          claimId: record.id,
          rule: "owner-pending-ledger-ref",
          detail: "an owner-signing-pending claim must point at the owner action ledger",
        })
      }
      const asserted = record.proofs.some(
        (proof) => proof.kind === "owner-signing" && proof.ran && proof.result === "pass",
      )
      if (asserted) {
        violations.push({
          claimId: record.id,
          rule: "no-autonomous-signed-proof",
          detail: "an owner-signing-pending claim must not assert a passing signing proof",
        })
      }
      continue
    }
    // Achieved rungs: the claim must not stand above the strongest passing proof.
    const earned = strongestPassingRung(record)
    if (earned === 0) {
      violations.push({
        claimId: record.id,
        rule: "has-passing-proof",
        detail: "claim on an achieved rung cites no proof that ran and passed",
      })
      continue
    }
    if (rungOrder[record.rung] > earned) {
      violations.push({
        claimId: record.id,
        rule: "no-claim-above-evidence",
        detail: `rung ${record.rung} (${rungOrder[record.rung]}) exceeds the strongest passing proof (${earned})`,
      })
    }
  }
  return violations
}

/** Every repository-relative proof file the ledger cites, de-duplicated. */
export const citedProofRefs = (ledger: ReadonlyArray<ClaimRecord>): ReadonlyArray<string> => {
  const refs = new Set<string>()
  for (const record of ledger) {
    for (const proof of record.proofs) refs.add(proof.ref)
  }
  return [...refs]
}

/** The version-one capability ids the ledger is missing, if any. */
export const missingCapabilityIds = (ledger: ReadonlyArray<ClaimRecord>): ReadonlyArray<string> => {
  const present = new Set(ledger.map((record) => record.id))
  return versionOneCapabilityIds.filter((id) => !present.has(id))
}
