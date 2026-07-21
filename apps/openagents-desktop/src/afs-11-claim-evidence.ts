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
 * What this guard does and does not verify. This is the boundary an earlier
 * audit (orrery, #9089) found overstated, so it is written plainly here. The
 * guard is a structural, mechanical check. It does NOT re-run the cited proofs.
 * Working with `afs-11-claim-evidence.test.ts`, it proves:
 *
 *   - Coverage: every version-one capability from the cut line is present, and
 *     each claim cites at least one proof.
 *   - Source binding: each version-one capability claim text is present verbatim
 *     in the plan cut line, so the transcription cannot silently drift from its
 *     source (the test reads the plan file).
 *   - File existence: every cited proof file exists in the repository.
 *   - Proof shape and rung consistency: a runnable (unit or integration) proof
 *     must be a sweep-collected test file or a wired `check-` script, a ceremony
 *     (packaged or owner-signing) proof must be a script or runbook and never a
 *     sweep test, and a claim's rung is never above the strongest proof KIND
 *     that its ledger record marks as passing.
 *   - Reserved reservation: the `owner-signing-pending` outcome is marked
 *     blocked-on-owner, its cited signing proof is refused-or-not-run and never
 *     a passing result, and its reserved-step reference resolves to a real
 *     in-repository file.
 *   - Receipt pinning: the ceremony evidence record exists on disk, so any proof
 *     that records a manual ceremony run is anchored to a committed receipt.
 *
 * The pass or refuse VERDICTS of the cited proofs are produced elsewhere, not by
 * this guard. Runnable (unit and integration) proofs are `.test.ts`/`.test.tsx`
 * files or `check-` scripts that the normal `pnpm run check` gate executes, so a
 * red proof turns the completion gate red. Ceremony (packaged and owner-signing)
 * proofs are hand-run release steps that an automated guard cannot and must not
 * re-run, their recorded outcome lives in the committed, dated evidence record
 * `docs/apple-fm/2026-07-20-afs-11-release-evidence.md`. The `ran` and `result`
 * fields on each proof are that recorded state, not a verdict this module
 * computes.
 *
 * The signed and notarized installed-application acceptance journey stays
 * owner-reserved. This ledger records that split honestly. It never claims a
 * signed-release proof that did not run.
 *
 * This module is a pure typed record plus pure validators, in the same style as
 * the sibling `release-preflight.ts` and `mvp-proof.ts` proof modules. It has no
 * platform, provider, or store dependency and never touches secrets.
 */

/** The rungs a version-one claim can stand on. The first three are achieved
 *  evidence strengths. `owner-signing-pending` is not an evidence strength, it
 *  is a reserved status that means no achieved evidence yet. */
export type EvidenceRung =
  | "unit-tested"
  | "integration-proven"
  | "packaged-proven"
  | "owner-signing-pending"

/** The reserved status that stands for the owner ceremony an agent must not
 *  run. It is NOT the strongest rung, it means nothing is proven yet. */
export const reservedRung: EvidenceRung = "owner-signing-pending"

export const isReservedRung = (rung: EvidenceRung): boolean => rung === reservedRung

/**
 * Achieved evidence strength, increasing. An earlier audit (#9089) flagged that
 * ranking `owner-signing-pending` above `packaged-proven` made the least-proven
 * claim read as the most-proven to any numeric consumer. It is corrected here:
 * the reserved status carries strength 0 (no achieved evidence), never a value
 * above `packaged-proven`. A consumer that ranks claims by evidence strength
 * must read the reserved status as the weakest, awaiting the owner ceremony.
 */
export const rungOrder: Readonly<Record<EvidenceRung, number>> = {
  "owner-signing-pending": 0,
  "unit-tested": 1,
  "integration-proven": 2,
  "packaged-proven": 3,
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
 * The plan file whose "Version-one cut line" section is the source of the
 * version-one capability claims. The test reads it and asserts every capability
 * claim text is present, so the ledger cannot drift from its source.
 */
export const versionOneCutLineSourceRef =
  "docs/sol/2026-07-20-apple-fm-router-to-full-agent-system-plan.md"

/**
 * The committed, dated evidence record. It carries the recorded outcome of the
 * ceremony proofs (release preflight, isolated-app proof, and the 2026-07-20
 * signing and notarization receipt). Any proof whose `ran`/`result` records a
 * manual ceremony is anchored to this file, which the test asserts exists.
 */
export const afs11EvidenceDocRef = "docs/apple-fm/2026-07-20-afs-11-release-evidence.md"

/**
 * The in-repository reference for the reserved owner step. It points at section
 * 4 of the committed evidence record, which describes the one remaining
 * owner-reserved action (the interactive installed-application acceptance
 * journey) and, in turn, cites the workspace owner action ledger and the
 * release signing runbook. An earlier audit (#9089) found the previous value
 * pointed at a `NEEDS_OWNER.md` anchor that did not resolve, so this now names a
 * real in-repository file that the test resolves on disk.
 */
export const ownerSigningLedgerRef =
  "docs/apple-fm/2026-07-20-afs-11-release-evidence.md#4-the-owner-reserved-signed-release-proof"

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
          "Run for #9089 against the built dist artifact set. Green rows: clean_origin_main, version_monotonic, attribution_intact, app_identity_stable, artifact_set_complete, no_upstream_updater_remnants, no_legacy_ui_entrypoints, no_source_checkout_paths. On 2026-07-20 an owner-authorized run reran the preflight with the Developer ID and ASC notary credentials loaded, and all nine oracles were green, including signing_credentials_present.",
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
        kind: "packaged",
        ran: true,
        result: "pass",
        detail:
          "On 2026-07-20 an owner-authorized run loaded the Developer ID and ASC notary credentials, and the release preflight passed all nine oracles including signing_credentials_present. make:mac then signed the OpenAgents RC 0.1.0-rc.25 arm64 app and DMG under Developer ID Application: OpenAgents, Inc. (HQWSG26L43) with a hardened runtime, and Apple notarization was Accepted for both the app and the DMG, which were stapled. Independent Gatekeeper assessment is green: codesign --verify --deep --strict is valid and satisfies the Designated Requirement, spctl --assess --type execute reports the app accepted as Notarized Developer ID, spctl accepts the DMG as Notarized Developer ID, and stapler validate works on the app and the DMG. The signed binary boots through the packaged smoke, and the 70 version-one capability tests pass. Full receipt: docs/apple-fm/2026-07-20-afs-11-release-evidence.md and issue #9089.",
      },
      {
        ref: "apps/oa-updates/docs/release-signing-runbook.md",
        kind: "owner-signing",
        ran: false,
        result: "not-run",
        detail:
          "The signing and notarization ceremony is complete (see the packaged proof above). What remains owner-reserved is the interactive installed-application version-one acceptance journey: install from the signed DMG on a clean machine, then drive the live Apple FM on-device answer and route recommendation and the host-selected codex-local delegation producing real running, done, failed, refused, and cancelled cards through the installed application interface. That journey needs a human at the interface, because this repository has no user-interface click automation, plus the on-device Apple Intelligence model and a logged-in Codex session, so an agent cannot run it. See the workspace NEEDS_OWNER ledger for the exact owner steps.",
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

/** True when the ref is a sweep-collected test file. */
const isSweepTestRef = (ref: string): boolean => /\.test\.tsx?$/.test(ref)

/** True when the ref is a wired boundary check script, for example
 *  `scripts/check-afs-boundaries.ts`. Its basename starts with `check-`. */
const isCheckScriptRef = (ref: string): boolean => /(^|\/)check-[^/]*\.ts$/.test(ref)

/** A runnable (unit or integration) proof must be executed by the normal
 *  `pnpm run check` gate: either a sweep test file or a wired check script. */
const isRunnableProofRef = (ref: string): boolean => isSweepTestRef(ref) || isCheckScriptRef(ref)

/** The reserved-step reference must name a real in-repository file, not free
 *  prose. It must carry a path (a `/`) and a file extension (a `.`). */
const looksLikeRepositoryPath = (ref: string): boolean => {
  const filePart = ref.split("#")[0] ?? ""
  return filePart.includes("/") && /\.[a-z0-9]+$/i.test(filePart) && !/\s/.test(filePart)
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
    // Proof-shape rules apply to every claim. A runnable proof must be a
    // sweep-executed file, and a ceremony proof must never masquerade as one.
    for (const proof of record.proofs) {
      const runnableKind = proof.kind === "unit" || proof.kind === "integration"
      if (runnableKind && !isRunnableProofRef(proof.ref)) {
        violations.push({
          claimId: record.id,
          rule: "runnable-proof-shape",
          detail: `a ${proof.kind} proof must be a sweep test or check script, not ${proof.ref}`,
        })
      }
      const ceremonyKind = proof.kind === "packaged" || proof.kind === "owner-signing"
      if (ceremonyKind && isSweepTestRef(proof.ref)) {
        violations.push({
          claimId: record.id,
          rule: "ceremony-proof-shape",
          detail: `a ${proof.kind} proof is a hand-run ceremony, it must not be a sweep test (${proof.ref})`,
        })
      }
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
      } else if (!looksLikeRepositoryPath(record.ownerReservedStepRef)) {
        violations.push({
          claimId: record.id,
          rule: "owner-pending-ledger-ref-shape",
          detail: "the reserved-step reference must name a real in-repository file, not free prose",
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
