/**
 * Effect Native ProductSpec workroom.
 *
 * The renderer owns drafts and projections only. Every ProductSpec identity,
 * plan transition, packet admission, evidence receipt, and verification
 * transition is returned by the typed preload bridge and decoded again here.
 * The selected coding session supplies the durable work-context identity.
 */
import {
  Badge,
  Button,
  ComponentValueBinding,
  IntentRef,
  Spacer,
  Stack,
  StaticPayload,
  Text,
  TextField,
  defineIntent,
  type View,
} from "@effect-native/core"
import { Effect, Schema, SubscriptionRef } from "@effect-native/core/effect"

import {
  decodeProductSpecPlanResult,
  decodeProductSpecEditConfirmationResult,
  decodeProductSpecEditProposalResult,
  decodeProductSpecProjectionResult,
  decodeProductSpecRunResult,
  type ProductSpecPlan,
  type ProductSpecEditProposal,
  type ProductSpecProjection,
  type ProductSpecRun,
  type ProductSpecWorkPacket,
} from "../product-spec-workroom-contract.ts"
import type { DesktopCodingCatalogProjection } from "../coding-catalog-contract.ts"

type ProductSpecResult<A> =
  | Readonly<{ ok: true; value: A; reconciled?: boolean }>
  | Readonly<{ ok: false; reason: string; message: string }>

const projectionResult = (value: unknown): ProductSpecResult<ProductSpecProjection> | null =>
  (decodeProductSpecProjectionResult(value) as ProductSpecResult<ProductSpecProjection> | null | undefined) ?? null
const planResult = (value: unknown): ProductSpecResult<ProductSpecPlan> | null =>
  (decodeProductSpecPlanResult(value) as ProductSpecResult<ProductSpecPlan> | null | undefined) ?? null
const runResult = (value: unknown): ProductSpecResult<ProductSpecRun> | null =>
  (decodeProductSpecRunResult(value) as ProductSpecResult<ProductSpecRun> | null | undefined) ?? null
const editProposalResult = (value: unknown): ProductSpecResult<ProductSpecEditProposal> | null =>
  (decodeProductSpecEditProposalResult(value) as ProductSpecResult<ProductSpecEditProposal> | null | undefined) ?? null

export type ProductSpecWorkspaceState = Readonly<{
  relativePath: string
  createTitle: string
  projection: ProductSpecProjection | null
  editDraft: string
  editProposal: ProductSpecEditProposal | null
  plan: ProductSpecPlan | null
  run: ProductSpecRun | null
  evidenceRef: string
  verifierRef: string
  verificationOutputRef: string
  blockedReason: string
  busy: string | null
  notice: string | null
  error: string | null
}>

export const emptyProductSpecWorkspaceState = (): ProductSpecWorkspaceState => ({
  relativePath: "docs/mvp/openagents-codex-mvp.product-spec.md",
  createTitle: "OpenAgents Codex MVP",
  projection: null,
  editDraft: "",
  editProposal: null,
  plan: null,
  run: null,
  evidenceRef: "",
  verifierRef: "",
  verificationOutputRef: "",
  blockedReason: "",
  busy: null,
  notice: null,
  error: null,
})

export type ProductSpecWorkspaceCapableState = Readonly<{
  codingCatalog: DesktopCodingCatalogProjection
  productSpec: ProductSpecWorkspaceState
}>

export type ProductSpecRendererBridge = Readonly<{
  open: (value: unknown) => Promise<unknown>
  create: (value: unknown) => Promise<unknown>
  proposeEdit: (value: unknown) => Promise<unknown>
  confirmEdit: (value: unknown) => Promise<unknown>
  proposePlan: (value: unknown) => Promise<unknown>
  acceptPlan: (value: unknown) => Promise<unknown>
  admitPacket: (value: unknown) => Promise<unknown>
  blockPacket: (value: unknown) => Promise<unknown>
  disposePacket: (value: unknown) => Promise<unknown>
  recordEvidence: (value: unknown) => Promise<unknown>
  verifyEvidence: (value: unknown) => Promise<unknown>
  setOwnerDisposition: (value: unknown) => Promise<unknown>
  run: (value: unknown) => Promise<unknown>
}>

const unavailable = async (): Promise<unknown> => ({
  ok: false,
  reason: "read_failed",
  message: "The ProductSpec workroom is unavailable in this desktop host.",
})

export const unavailableProductSpecRendererBridge: ProductSpecRendererBridge = {
  open: unavailable,
  create: unavailable,
  proposeEdit: unavailable,
  confirmEdit: unavailable,
  proposePlan: unavailable,
  acceptPlan: unavailable,
  admitPacket: unavailable,
  blockPacket: unavailable,
  disposePacket: unavailable,
  recordEvidence: unavailable,
  verifyEvidence: unavailable,
  setOwnerDisposition: unavailable,
  run: unavailable,
}

export const ProductSpecPathChanged = defineIntent("ProductSpecPathChanged", Schema.String)
export const ProductSpecTitleChanged = defineIntent("ProductSpecTitleChanged", Schema.String)
export const ProductSpecOpenRequested = defineIntent("ProductSpecOpenRequested", Schema.Null)
export const ProductSpecCreateRequested = defineIntent("ProductSpecCreateRequested", Schema.Null)
export const ProductSpecEditDraftChanged = defineIntent("ProductSpecEditDraftChanged", Schema.String)
export const ProductSpecEditProposed = defineIntent("ProductSpecEditProposed", Schema.Null)
export const ProductSpecEditConfirmed = defineIntent("ProductSpecEditConfirmed", Schema.Null)
export const ProductSpecPlanProposed = defineIntent("ProductSpecPlanProposed", Schema.Null)
export const ProductSpecPlanAccepted = defineIntent("ProductSpecPlanAccepted", Schema.Null)
export const ProductSpecEvidenceRefChanged = defineIntent("ProductSpecEvidenceRefChanged", Schema.String)
export const ProductSpecVerifierRefChanged = defineIntent("ProductSpecVerifierRefChanged", Schema.String)
export const ProductSpecVerificationOutputRefChanged = defineIntent("ProductSpecVerificationOutputRefChanged", Schema.String)
export const ProductSpecBlockedReasonChanged = defineIntent("ProductSpecBlockedReasonChanged", Schema.String)
export const ProductSpecPacketAdmitted = defineIntent("ProductSpecPacketAdmitted", Schema.String)
export const ProductSpecPacketBlocked = defineIntent("ProductSpecPacketBlocked", Schema.String)
export const ProductSpecPacketDispositionSelected = defineIntent("ProductSpecPacketDispositionSelected", Schema.Struct({
  packetRef: Schema.String,
  disposition: Schema.Literals(["failed", "cancelled", "superseded"]),
}))
export const ProductSpecEvidenceRecorded = defineIntent("ProductSpecEvidenceRecorded", Schema.String)
export const ProductSpecEvidenceVerified = defineIntent("ProductSpecEvidenceVerified", Schema.String)
export const ProductSpecOwnerDispositionSelected = defineIntent("ProductSpecOwnerDispositionSelected", Schema.Struct({
  packetRef: Schema.String,
  disposition: Schema.Literals(["accepted", "waived"]),
}))
export const ProductSpecRunRefreshed = defineIntent("ProductSpecRunRefreshed", Schema.Null)

export const productSpecWorkspaceIntents = [
  ProductSpecPathChanged,
  ProductSpecTitleChanged,
  ProductSpecOpenRequested,
  ProductSpecCreateRequested,
  ProductSpecEditDraftChanged,
  ProductSpecEditProposed,
  ProductSpecEditConfirmed,
  ProductSpecPlanProposed,
  ProductSpecPlanAccepted,
  ProductSpecEvidenceRefChanged,
  ProductSpecVerifierRefChanged,
  ProductSpecVerificationOutputRefChanged,
  ProductSpecBlockedReasonChanged,
  ProductSpecPacketAdmitted,
  ProductSpecPacketBlocked,
  ProductSpecPacketDispositionSelected,
  ProductSpecEvidenceRecorded,
  ProductSpecEvidenceVerified,
  ProductSpecOwnerDispositionSelected,
  ProductSpecRunRefreshed,
] as const

const selectedWorkContextRef = (catalog: DesktopCodingCatalogProjection): string | null => {
  const selected = catalog.selectedSessionRef === null
    ? undefined
    : catalog.sessions.find((session) => session.sessionRef === catalog.selectedSessionRef)
  return selected?.workContextRef ?? null
}

const safeRefPart = (value: string): string => {
  const sanitized = value.replace(/[^A-Za-z0-9._:-]/g, ".")
  return sanitized === "" || !/^[A-Za-z0-9]/.test(sanitized) ? `ref.${sanitized}` : sanitized
}

const operationMessage = (value: unknown, fallback: string): string =>
  typeof value === "object" && value !== null && "message" in value && typeof value.message === "string"
    ? value.message
    : fallback

const packetsFor = (projection: Extract<ProductSpecProjection, { state: "ready" }>) =>
  projection.criteria.map((criterion, index) => ({
    packetRef: `packet.${safeRefPart(criterion.id.toLowerCase())}`,
    title: criterion.body,
    criterionIds: [criterion.id],
    dependencyRefs: index === 0
      ? []
      : [`packet.${safeRefPart(projection.criteria[index - 1]!.id.toLowerCase())}`],
    allocation: index === 0 ? "root" as const : "child" as const,
  }))

/** Exact prompt envelope for the first Codex turn bound to an admitted packet. */
export const productSpecPacketPrompt = (
  run: ProductSpecRun,
  packet: ProductSpecWorkPacket,
): string => [
  "Use the built-in productspec-work skill to execute this admitted ProductSpec work packet.",
  "Treat the identities below as immutable host-confirmed authority. Do not substitute another spec, revision, packet, lease, or criterion.",
  `Spec: ${run.spec.specRef}`,
  `Spec revision: ${run.spec.revision}`,
  `Spec digest: ${run.spec.digest}`,
  `Run: ${run.runRef}`,
  `Plan: ${run.plan.planRef}`,
  `Packet: ${packet.packetRef}`,
  `Lease: ${packet.activeLease?.leaseRef ?? "missing"}`,
  `Acceptance criteria: ${packet.criterionIds.join(", ")}`,
  `Objective: ${packet.title}`,
  "Work only on this packet. Run relevant verification, then report exact evidence and any blocker without claiming independent verification.",
].join("\n")

export const makeProductSpecWorkspaceHandlers = <S extends ProductSpecWorkspaceCapableState>(
  state: SubscriptionRef.SubscriptionRef<S>,
  bridge: ProductSpecRendererBridge = unavailableProductSpecRendererBridge,
  makeRef: () => string = () => globalThis.crypto.randomUUID(),
  onPacketAdmitted?: (run: ProductSpecRun, packet: ProductSpecWorkPacket) => Promise<void>,
) => {
  const setWorkspace = (mutate: (workspace: ProductSpecWorkspaceState) => ProductSpecWorkspaceState) =>
    SubscriptionRef.update(state, (current) => ({ ...current, productSpec: mutate(current.productSpec) }))

  const workContextOrError = (current: S): string | null => selectedWorkContextRef(current.codingCatalog)

  const runOperation = (
    packetRef: string,
    operation: (current: S, packet: ProductSpecWorkPacket) => Promise<unknown>,
    afterConfirmed?: (run: ProductSpecRun, packet: ProductSpecWorkPacket) => Promise<void>,
  ) =>
    Effect.gen(function* () {
      const current = yield* SubscriptionRef.get(state)
      const packet = current.productSpec.run?.plan.packets.find((candidate) => candidate.packetRef === packetRef)
      if (packet === undefined || current.productSpec.busy !== null) return
      yield* setWorkspace((workspace) => ({ ...workspace, busy: packetRef, error: null, notice: null }))
      const raw = yield* Effect.promise(() => operation(current, packet).catch(() => null))
      const result = runResult(raw)
      if (result !== null && result.ok) {
        const confirmedPacket = result.value.plan.packets.find((value) => value.packetRef === packetRef)
        yield* setWorkspace((workspace) => ({
          ...workspace,
          run: result.value,
          plan: result.value.plan,
          busy: null,
          notice: `Packet ${packetRef} is now ${result.value.plan.packets.find((value) => value.packetRef === packetRef)?.state ?? "updated"}.`,
          error: null,
        }))
        if (confirmedPacket !== undefined && afterConfirmed !== undefined) {
          yield* Effect.promise(() => afterConfirmed(result.value, confirmedPacket).catch(() => undefined))
        }
      } else {
        yield* setWorkspace((workspace) => ({ ...workspace, busy: null, error: operationMessage(result, "The packet transition could not be confirmed.") }))
      }
    })

  const openOrCreate = (kind: "open" | "create") => Effect.gen(function* () {
    const current = yield* SubscriptionRef.get(state)
    if (current.productSpec.busy !== null) return
    const workContextRef = workContextOrError(current)
    if (workContextRef === null) {
      yield* setWorkspace((workspace) => ({ ...workspace, error: "Select an admitted coding workspace before opening a ProductSpec." }))
      return
    }
    const relativePath = current.productSpec.relativePath.trim()
    const title = current.productSpec.createTitle.trim()
    yield* setWorkspace((workspace) => ({ ...workspace, busy: kind, error: null, notice: null }))
    const raw = yield* Effect.promise(() => (kind === "open"
      ? bridge.open({ workContextRef, relativePath })
      : bridge.create({ workContextRef, relativePath, title })).catch(() => null))
    const result = projectionResult(raw)
    if (result !== null && result.ok) {
      yield* setWorkspace((workspace) => ({
        ...workspace,
        projection: result.value,
        editDraft: result.value.sourceMarkdown,
        editProposal: null,
        plan: null,
        run: null,
        busy: null,
        notice: result.value.state === "ready" ? "ProductSpec is executable." : "ProductSpec validation completed with blockers.",
        error: null,
      }))
    } else {
      yield* setWorkspace((workspace) => ({ ...workspace, busy: null, error: operationMessage(result, "The ProductSpec could not be read.") }))
    }
  })

  return {
    ProductSpecPathChanged: (value: string) => setWorkspace((workspace) => ({ ...workspace, relativePath: value.slice(0, 512) })),
    ProductSpecTitleChanged: (value: string) => setWorkspace((workspace) => ({ ...workspace, createTitle: value.slice(0, 200) })),
    ProductSpecOpenRequested: () => openOrCreate("open"),
    ProductSpecCreateRequested: () => openOrCreate("create"),
    ProductSpecEditDraftChanged: (value: string) => setWorkspace(workspace => ({ ...workspace, editDraft: value.slice(0, 1_000_000), editProposal: null })),
    ProductSpecEditProposed: () => Effect.gen(function* () {
      const current = yield* SubscriptionRef.get(state)
      const projection = current.productSpec.projection
      const workContextRef = workContextOrError(current)
      if (projection?.state !== "ready" || workContextRef === null || current.productSpec.busy !== null || current.productSpec.editDraft === projection.sourceMarkdown) return
      yield* setWorkspace(workspace => ({ ...workspace, busy: "edit-propose", error: null, notice: null }))
      const raw = yield* Effect.promise(() => bridge.proposeEdit({
        workContextRef,
        expectedCurrent: projection.identity,
        proposedMarkdown: current.productSpec.editDraft,
      }).catch(() => null))
      const result = editProposalResult(raw)
      yield* setWorkspace(workspace => result !== null && result.ok
        ? { ...workspace, editProposal: result.value, busy: null, notice: "Review the exact diff and criterion reconciliation before confirming.", error: null }
        : { ...workspace, busy: null, error: operationMessage(result, "The ProductSpec edit could not be proposed.") })
    }),
    ProductSpecEditConfirmed: () => Effect.gen(function* () {
      const current = yield* SubscriptionRef.get(state)
      const projection = current.productSpec.projection
      const proposal = current.productSpec.editProposal
      if (projection?.state !== "ready" || proposal === null || current.productSpec.busy !== null) return
      yield* setWorkspace(workspace => ({ ...workspace, busy: "edit-confirm", error: null, notice: null }))
      const raw = yield* Effect.promise(() => bridge.confirmEdit({ proposalRef: proposal.proposalRef, expectedCurrent: projection.identity, criterionDisposition: "supersede_affected_packets" }).catch(() => null))
      const result = decodeProductSpecEditConfirmationResult(raw) as ProductSpecResult<{ projection: ProductSpecProjection }> | null
      yield* setWorkspace(workspace => result !== null && result.ok
        ? { ...workspace, projection: result.value.projection, editDraft: result.value.projection.sourceMarkdown, editProposal: null, plan: null, run: null, busy: null, notice: "ProductSpec revision confirmed. Create a new plan for the new immutable identity.", error: null }
        : { ...workspace, busy: null, error: operationMessage(result, "The ProductSpec edit confirmation failed.") })
    }),
    ProductSpecPlanProposed: () => Effect.gen(function* () {
      const current = yield* SubscriptionRef.get(state)
      const projection = current.productSpec.projection
      const workContextRef = workContextOrError(current)
      if (projection?.state !== "ready" || workContextRef === null || current.productSpec.busy !== null) return
      yield* setWorkspace((workspace) => ({ ...workspace, busy: "plan", error: null, notice: null }))
      const raw = yield* Effect.promise(() => bridge.proposePlan({
        workContextRef,
        spec: projection.identity,
        packets: packetsFor(projection),
        deferredCriterionIds: [],
      }).catch(() => null))
      const result = planResult(raw)
      yield* setWorkspace((workspace) => result !== null && result.ok
        ? { ...workspace, plan: result.value, run: null, busy: null, notice: "Plan proposed. Review it before accepting.", error: null }
        : { ...workspace, busy: null, error: operationMessage(result, "The plan proposal was rejected.") })
    }),
    ProductSpecPlanAccepted: () => Effect.gen(function* () {
      const current = yield* SubscriptionRef.get(state)
      const plan = current.productSpec.plan
      const projection = current.productSpec.projection
      if (plan === null || projection?.state !== "ready" || current.productSpec.busy !== null) return
      yield* setWorkspace((workspace) => ({ ...workspace, busy: "accept", error: null, notice: null }))
      const raw = yield* Effect.promise(() => bridge.acceptPlan({ planRef: plan.planRef, expectedSpec: projection.identity }).catch(() => null))
      const result = runResult(raw)
      yield* setWorkspace((workspace) => result !== null && result.ok
        ? { ...workspace, plan: result.value.plan, run: result.value, busy: null, notice: "Plan accepted. Packet admission is now available.", error: null }
        : { ...workspace, busy: null, error: operationMessage(result, "The plan could not be accepted.") })
    }),
    ProductSpecEvidenceRefChanged: (value: string) => setWorkspace((workspace) => ({ ...workspace, evidenceRef: value.slice(0, 256) })),
    ProductSpecVerifierRefChanged: (value: string) => setWorkspace((workspace) => ({ ...workspace, verifierRef: value.slice(0, 256) })),
    ProductSpecVerificationOutputRefChanged: (value: string) => setWorkspace((workspace) => ({ ...workspace, verificationOutputRef: value.slice(0, 256) })),
    ProductSpecBlockedReasonChanged: (value: string) => setWorkspace((workspace) => ({ ...workspace, blockedReason: value.slice(0, 2_000) })),
    ProductSpecPacketAdmitted: (packetRef: string) => runOperation(packetRef, async (current) => {
      const run = current.productSpec.run!
      return bridge.admitPacket({
        runRef: run.runRef,
        packetRef,
        leaseRef: `lease.desktop.${safeRefPart(makeRef())}`,
        executorRef: "executor.desktop.owner",
        executionMode: "owner-present",
        expectedSpec: run.spec,
      })
    }, onPacketAdmitted),
    ProductSpecPacketBlocked: (packetRef: string) => runOperation(packetRef, async (current, packet) => bridge.blockPacket({
      runRef: current.productSpec.run!.runRef,
      packetRef,
      leaseRef: packet.activeLease?.leaseRef ?? "lease.missing",
      reason: current.productSpec.blockedReason.trim(),
      expectedSpec: current.productSpec.run!.spec,
    })),
    ProductSpecPacketDispositionSelected: ({ packetRef, disposition }: { packetRef: string; disposition: "failed" | "cancelled" | "superseded" }) => runOperation(packetRef, async current => bridge.disposePacket({
      runRef: current.productSpec.run!.runRef,
      packetRef,
      disposition,
      reason: current.productSpec.blockedReason.trim(),
      expectedSpec: current.productSpec.run!.spec,
    })),
    ProductSpecEvidenceRecorded: (packetRef: string) => runOperation(packetRef, async (current, packet) => bridge.recordEvidence({
      runRef: current.productSpec.run!.runRef,
      packetRef,
      leaseRef: packet.activeLease?.leaseRef ?? "lease.missing",
      evidenceRef: current.productSpec.evidenceRef.trim(),
      evidenceKind: "receipt",
      expectedSpec: current.productSpec.run!.spec,
    })),
    ProductSpecEvidenceVerified: (packetRef: string) => runOperation(packetRef, async (current) => bridge.verifyEvidence({
      runRef: current.productSpec.run!.runRef,
      packetRef,
      verifierRef: current.productSpec.verifierRef.trim(),
      outputRef: current.productSpec.verificationOutputRef.trim(),
      evidenceReceiptRefs: current.productSpec.run!.plan.packets
        .find(candidate => candidate.packetRef === packetRef)?.evidenceReceipts
        .map(receipt => receipt.receiptRef) ?? [],
      expectedSpec: current.productSpec.run!.spec,
    })),
    ProductSpecOwnerDispositionSelected: ({ packetRef, disposition }: { packetRef: string; disposition: "accepted" | "waived" }) => runOperation(packetRef, async current => bridge.setOwnerDisposition({
      runRef: current.productSpec.run!.runRef,
      packetRef,
      disposition,
      ownerRef: "owner.desktop.local",
      ...(disposition === "waived" ? { reason: current.productSpec.blockedReason.trim() } : {}),
      expectedSpec: current.productSpec.run!.spec,
    })),
    ProductSpecRunRefreshed: () => Effect.gen(function* () {
      const current = yield* SubscriptionRef.get(state)
      const runRef = current.productSpec.run?.runRef
      if (runRef === undefined || current.productSpec.busy !== null) return
      yield* setWorkspace((workspace) => ({ ...workspace, busy: "refresh", error: null, notice: null }))
      const raw = yield* Effect.promise(() => bridge.run({ runRef }).catch(() => null))
      const result = runResult(raw)
      yield* setWorkspace((workspace) => result !== null && result.ok
        ? { ...workspace, run: result.value, plan: result.value.plan, busy: null, notice: "Run projection refreshed.", error: null }
        : { ...workspace, busy: null, error: operationMessage(result, "The run projection could not be refreshed.") })
    }),
  }
}

const stateTone = (state: ProductSpecWorkPacket["state"]): "neutral" | "info" | "warn" | "success" | "danger" =>
  state === "verified" ? "success"
    : state === "active" || state === "evidence_present" ? "info"
      : state === "blocked" ? "warn"
        : state === "failed" || state === "cancelled" ? "danger"
          : "neutral"

const packetView = (
  workspace: ProductSpecWorkspaceState,
  packet: ProductSpecWorkPacket,
  packets: ReadonlyArray<ProductSpecWorkPacket>,
): View => {
  const busy = workspace.busy !== null
  const evidenceReady = workspace.evidenceRef.trim() !== ""
  const verifierReady = workspace.verifierRef.trim() !== "" && workspace.verificationOutputRef.trim() !== ""
  const blockReady = workspace.blockedReason.trim() !== ""
  const dependenciesVerified = packet.dependencyRefs.every((dependencyRef) =>
    packets.find((candidate) => candidate.packetRef === dependencyRef)?.state === "verified")
  return Stack({ key: `product-spec-packet-${packet.packetRef}`, direction: "column", gap: "2", style: { width: "full", minWidth: 0, padding: "3", backgroundColor: "surfaceRaised", borderRadius: "md" } }, [
    Stack({ key: `product-spec-packet-head-${packet.packetRef}`, direction: "row", gap: "2", align: "center", style: { width: "full", minWidth: 0 } }, [
      Badge({ key: `product-spec-packet-state-${packet.packetRef}`, label: packet.state.replace("_", " "), tone: stateTone(packet.state) }),
      Text({ key: `product-spec-packet-title-${packet.packetRef}`, content: packet.title, variant: "label", color: "textPrimary" }),
      Spacer({ key: `product-spec-packet-fill-${packet.packetRef}`, flex: true }),
      ...(packet.state === "planned" ? [Button({ key: `product-spec-admit-${packet.packetRef}`, label: dependenciesVerified ? "Start packet" : "Waiting for dependencies", variant: "primary", disabled: busy || !dependenciesVerified, onPress: IntentRef("ProductSpecPacketAdmitted", StaticPayload(packet.packetRef)) })] : []),
      ...(packet.state === "active" ? [
        Button({ key: `product-spec-evidence-${packet.packetRef}`, label: "Record evidence", variant: "primary", disabled: busy || !evidenceReady, onPress: IntentRef("ProductSpecEvidenceRecorded", StaticPayload(packet.packetRef)) }),
        Button({ key: `product-spec-block-${packet.packetRef}`, label: "Block", variant: "secondary", disabled: busy || !blockReady, onPress: IntentRef("ProductSpecPacketBlocked", StaticPayload(packet.packetRef)) }),
      ] : []),
      ...(packet.state === "evidence_present" ? [Button({ key: `product-spec-verify-${packet.packetRef}`, label: "Verify evidence", variant: "primary", disabled: busy || !verifierReady, onPress: IntentRef("ProductSpecEvidenceVerified", StaticPayload(packet.packetRef)) })] : []),
      ...(packet.state === "verified" && packet.ownerDisposition === null ? [
        Button({ key: `product-spec-owner-accept-${packet.packetRef}`, label: "Owner accept", variant: "primary", disabled: busy, onPress: IntentRef("ProductSpecOwnerDispositionSelected", StaticPayload({ packetRef: packet.packetRef, disposition: "accepted" })) }),
        Button({ key: `product-spec-owner-waive-${packet.packetRef}`, label: "Owner waive", variant: "secondary", disabled: busy || !blockReady, onPress: IntentRef("ProductSpecOwnerDispositionSelected", StaticPayload({ packetRef: packet.packetRef, disposition: "waived" })) }),
      ] : []),
      ...(["planned", "active", "blocked", "evidence_present"].includes(packet.state) ? [
        Button({ key: `product-spec-cancel-${packet.packetRef}`, label: "Cancel", variant: "ghost", disabled: busy || !blockReady, onPress: IntentRef("ProductSpecPacketDispositionSelected", StaticPayload({ packetRef: packet.packetRef, disposition: "cancelled" })) }),
        Button({ key: `product-spec-supersede-${packet.packetRef}`, label: "Supersede", variant: "ghost", disabled: busy || !blockReady, onPress: IntentRef("ProductSpecPacketDispositionSelected", StaticPayload({ packetRef: packet.packetRef, disposition: "superseded" })) }),
      ] : []),
      ...(["active", "blocked", "evidence_present"].includes(packet.state) ? [Button({ key: `product-spec-fail-${packet.packetRef}`, label: "Mark failed", variant: "ghost", disabled: busy || !blockReady, onPress: IntentRef("ProductSpecPacketDispositionSelected", StaticPayload({ packetRef: packet.packetRef, disposition: "failed" })) })] : []),
    ]),
    Text({ key: `product-spec-packet-ref-${packet.packetRef}`, content: packet.packetRef, variant: "caption", color: "textMuted" }),
    Text({ key: `product-spec-packet-criteria-${packet.packetRef}`, content: `Criteria: ${packet.criterionIds.join(", ")} · Allocation: ${packet.allocation}`, variant: "caption", color: "textMuted" }),
    Text({ key: `product-spec-packet-deps-${packet.packetRef}`, content: `Depends on: ${packet.dependencyRefs.length === 0 ? "none" : packet.dependencyRefs.join(", ")}`, variant: "caption", color: "textMuted" }),
    ...(packet.activeLease === undefined || packet.activeLease === null ? [] : [Text({ key: `product-spec-packet-lease-${packet.packetRef}`, content: `Lease: ${packet.activeLease.leaseRef} · ${packet.activeLease.executorRef}`, variant: "caption", color: "textMuted" })]),
    ...(packet.evidenceRefs.length === 0 ? [] : [Text({ key: `product-spec-packet-evidence-refs-${packet.packetRef}`, content: `Evidence: ${packet.evidenceRefs.join(", ")}`, variant: "caption", color: "success" })]),
    ...(packet.evidenceReceipts.length === 0 ? [] : [Text({ key: `product-spec-packet-evidence-receipts-${packet.packetRef}`, content: `Evidence receipts: ${packet.evidenceReceipts.map(receipt => `${receipt.kind}:${receipt.receiptRef}`).join(", ")}`, variant: "caption", color: "success" })]),
    ...(packet.evidenceProducerRef === undefined ? [] : [Text({ key: `product-spec-packet-evidence-producer-${packet.packetRef}`, content: `Produced by: ${packet.evidenceProducerRef}`, variant: "caption", color: "textMuted" })]),
    ...(packet.verifierRefs.length === 0 ? [] : [Text({ key: `product-spec-packet-verifier-refs-${packet.packetRef}`, content: `Verified by: ${packet.verifierRefs.join(", ")}`, variant: "caption", color: "success" })]),
    ...(packet.verificationReceipts.length === 0 ? [] : [Text({ key: `product-spec-packet-verification-receipts-${packet.packetRef}`, content: `Verification receipts: ${packet.verificationReceipts.map(receipt => `${receipt.outputRef}:${receipt.receiptRef}`).join(", ")}`, variant: "caption", color: "success" })]),
    ...(packet.ownerDisposition === null ? [Text({ key: `product-spec-packet-owner-pending-${packet.packetRef}`, content: "Owner disposition: pending", variant: "caption", color: "textMuted" })] : [Text({ key: `product-spec-packet-owner-${packet.packetRef}`, content: `Owner ${packet.ownerDisposition.disposition} · ${packet.ownerDisposition.ownerRef}${packet.ownerDisposition.reason === undefined ? "" : ` · ${packet.ownerDisposition.reason}`}`, variant: "caption", color: packet.ownerDisposition.disposition === "accepted" ? "success" : "warning" })]),
    ...(packet.blockedReason === undefined ? [] : [Text({ key: `product-spec-packet-blocked-${packet.packetRef}`, content: packet.blockedReason, variant: "caption", color: "warning" })]),
  ])
}

export const productSpecWorkspaceView = (
  workspace: ProductSpecWorkspaceState,
  workContextRef: string | null,
): View => {
  const projection = workspace.projection
  const plan = workspace.run?.plan ?? workspace.plan
  const busy = workspace.busy !== null
  return Stack({ key: "product-spec-workspace", direction: "column", gap: "3", style: { width: "full", minWidth: 0, minHeight: 0, paddingTop: "2" } }, [
    Stack({ key: "product-spec-heading", direction: "row", gap: "2", align: "center", style: { width: "full" } }, [
      Text({ key: "product-spec-title", content: "ProductSpec workroom", variant: "title", color: "textPrimary" }),
      Badge({ key: "product-spec-effect-native", label: "Effect Native", tone: "info" }),
      Spacer({ key: "product-spec-heading-fill", flex: true }),
      ...(workspace.run === null ? [] : [Button({ key: "product-spec-refresh", label: "Refresh run", variant: "secondary", disabled: busy, onPress: IntentRef("ProductSpecRunRefreshed") })]),
    ]),
    Text({ key: "product-spec-purpose", content: "Define executable acceptance criteria, accept an explicit packet plan, then record and independently verify evidence. Host authority—not this view—admits every transition.", variant: "body", color: "textMuted" }),
    Text({ key: "product-spec-context", content: workContextRef === null ? "No admitted coding workspace selected" : `Work context: ${workContextRef}`, variant: "caption", color: workContextRef === null ? "warning" : "textMuted" }),
    Stack({ key: "product-spec-open-controls", direction: "row", gap: "2", align: "end", style: { width: "full", minWidth: 0 } }, [
      TextField({ key: "product-spec-path", value: workspace.relativePath, placeholder: "docs/example.product-spec.md", disabled: busy, a11y: { label: "ProductSpec workspace-relative path" }, onChange: IntentRef("ProductSpecPathChanged", ComponentValueBinding()), style: { flex: 2, minWidth: 0 } }),
      TextField({ key: "product-spec-create-title", value: workspace.createTitle, placeholder: "New ProductSpec title", disabled: busy, a11y: { label: "New ProductSpec title" }, onChange: IntentRef("ProductSpecTitleChanged", ComponentValueBinding()), style: { flex: 1, minWidth: 0 } }),
      Button({ key: "product-spec-open", label: "Open + validate", variant: "primary", disabled: busy || workContextRef === null || workspace.relativePath.trim() === "", onPress: IntentRef("ProductSpecOpenRequested") }),
      Button({ key: "product-spec-create", label: "Create", variant: "secondary", disabled: busy || workContextRef === null || workspace.relativePath.trim() === "" || workspace.createTitle.trim() === "", onPress: IntentRef("ProductSpecCreateRequested") }),
    ]),
    ...(workspace.notice === null ? [] : [Text({ key: "product-spec-notice", content: workspace.notice, variant: "caption", color: "success" })]),
    ...(workspace.error === null ? [] : [Text({ key: "product-spec-error", content: workspace.error, variant: "body", color: "danger" })]),
    ...(projection === null ? [Text({ key: "product-spec-empty", content: "Open an existing .product-spec.md file or create a starter in the selected worktree.", variant: "body", color: "textMuted" })]
      : projection.state === "invalid" ? [Stack({ key: "product-spec-invalid", direction: "column", gap: "1", style: { width: "full" } }, [
        Badge({ key: "product-spec-invalid-badge", label: "Not executable", tone: "danger" }),
        Text({ key: "product-spec-invalid-path", content: projection.relativePath, variant: "caption", color: "textMuted" }),
        TextField({ key: "product-spec-invalid-source", value: projection.sourceMarkdown, multiline: true, disabled: true, a11y: { label: "Invalid ProductSpec source" }, style: { width: "full", minHeight: 240 } }),
        ...projection.errors.map((issue, index) => Text({ key: `product-spec-error-${index}`, content: `${issue.path === undefined ? "" : `${issue.path} · `}${issue.code}: ${issue.message}`, variant: "body", color: "danger" })),
        ...projection.warnings.map((issue, index) => Text({ key: `product-spec-warning-${index}`, content: `${issue.path === undefined ? "" : `${issue.path} · `}${issue.code}: ${issue.message}`, variant: "caption", color: "warning" })),
      ])]
        : [Stack({ key: "product-spec-ready", direction: "column", gap: "2", style: { width: "full", minWidth: 0 } }, [
          Stack({ key: "product-spec-identity", direction: "row", gap: "2", align: "center", style: { width: "full", minWidth: 0 } }, [
            Badge({ key: "product-spec-ready-badge", label: "Executable", tone: "success" }),
            Text({ key: "product-spec-ready-title", content: projection.title, variant: "heading", color: "textPrimary" }),
            Text({ key: "product-spec-revision", content: `Revision ${projection.identity.revision}`, variant: "caption", color: "textMuted" }),
          ]),
          Text({ key: "product-spec-spec-ref", content: `Spec: ${projection.identity.specRef}`, variant: "caption", color: "textMuted" }),
          Text({ key: "product-spec-digest", content: projection.identity.digest, variant: "caption", color: "textMuted" }),
          TextField({ key: "product-spec-edit-draft", value: workspace.editDraft, multiline: true, disabled: busy || workspace.run !== null, a11y: { label: "ProductSpec revision draft" }, onChange: IntentRef("ProductSpecEditDraftChanged", ComponentValueBinding()), style: { width: "full", minHeight: 240 } }),
          ...(workspace.run !== null ? [Text({ key: "product-spec-edit-run-lock", content: "Finish or reconcile the accepted run before revising this ProductSpec.", variant: "caption", color: "warning" })] : []),
          ...(workspace.editProposal !== null ? [Stack({ key: "product-spec-edit-review", direction: "column", gap: "2", style: { width: "full", minWidth: 0, padding: "3", backgroundColor: "surfaceRaised", borderRadius: "md" } }, [
            Text({ key: "product-spec-edit-review-title", content: `Revision ${workspace.editProposal.previous.revision} → ${workspace.editProposal.next.revision}`, variant: "heading", color: "textPrimary" }),
            Text({ key: "product-spec-edit-reconciliation", content: `Retained: ${workspace.editProposal.reconciliation.retainedCriterionIds.join(", ") || "none"} · Changed: ${workspace.editProposal.reconciliation.changedCriterionIds.join(", ") || "none"} · Added: ${workspace.editProposal.reconciliation.addedCriterionIds.join(", ") || "none"} · Removed: ${workspace.editProposal.reconciliation.removedCriterionIds.join(", ") || "none"}`, variant: "body", color: "warning" }),
            Text({ key: "product-spec-edit-diff", content: workspace.editProposal.diff, variant: "caption", color: "textMuted" }),
            Text({ key: "product-spec-edit-disposition", content: "Confirmation explicitly supersedes every affected prior-revision packet. Evidence remains pinned to its original criterion and does not cross revisions.", variant: "caption", color: "warning" }),
            Button({ key: "product-spec-edit-confirm", label: "Confirm and supersede affected packets", variant: "primary", disabled: busy, onPress: IntentRef("ProductSpecEditConfirmed") }),
          ])] : workspace.run === null && workspace.editDraft !== projection.sourceMarkdown ? [Button({ key: "product-spec-edit-propose", label: "Review revision diff", variant: "secondary", disabled: busy, onPress: IntentRef("ProductSpecEditProposed") })] : []),
          Text({ key: "product-spec-criteria-title", content: `${projection.criteria.length} acceptance criteria`, variant: "label", color: "textPrimary" }),
          ...projection.criteria.map((criterion) => Stack({ key: `product-spec-criterion-${criterion.id}`, direction: "row", gap: "2", align: "start", style: { width: "full", minWidth: 0 } }, [
            Badge({ key: `product-spec-criterion-badge-${criterion.id}`, label: criterion.id, tone: "neutral" }),
            Text({ key: `product-spec-criterion-body-${criterion.id}`, content: criterion.body, variant: "body", color: "textPrimary" }),
          ])),
          ...(plan === null ? [Button({ key: "product-spec-propose-plan", label: "Propose criterion plan", variant: "primary", disabled: busy || projection.criteria.length === 0, onPress: IntentRef("ProductSpecPlanProposed") })] : []),
        ])]),
    ...(plan === null ? [] : [Stack({ key: "product-spec-plan", direction: "column", gap: "2", style: { width: "full", minWidth: 0 } }, [
      Stack({ key: "product-spec-plan-heading", direction: "row", gap: "2", align: "center", style: { width: "full" } }, [
        Text({ key: "product-spec-plan-title", content: "Work plan", variant: "heading", color: "textPrimary" }),
        Badge({ key: "product-spec-plan-state", label: plan.state, tone: plan.state === "accepted" ? "success" : "warn" }),
        Text({ key: "product-spec-plan-ref", content: plan.planRef, variant: "caption", color: "textMuted" }),
        Spacer({ key: "product-spec-plan-fill", flex: true }),
        ...(plan.state === "proposed" ? [Button({ key: "product-spec-accept-plan", label: "Accept plan", variant: "primary", disabled: busy, onPress: IntentRef("ProductSpecPlanAccepted") })] : []),
      ]),
      Text({ key: "product-spec-plan-copy", content: "One packet maps each criterion. Dependencies are sequential by default so the workroom prevents overlapping or duplicate execution until prior evidence is verified.", variant: "caption", color: "textMuted" }),
      ...(workspace.run === null ? [] : [Stack({ key: "product-spec-receipt-inputs", direction: "row", gap: "2", style: { width: "full", minWidth: 0 } }, [
        TextField({ key: "product-spec-evidence-ref", value: workspace.evidenceRef, placeholder: "evidence.receipt.ref", a11y: { label: "Evidence receipt reference" }, onChange: IntentRef("ProductSpecEvidenceRefChanged", ComponentValueBinding()), style: { flex: 1, minWidth: 0 } }),
        TextField({ key: "product-spec-verifier-ref", value: workspace.verifierRef, placeholder: "verifier.receipt.ref", a11y: { label: "Verifier receipt reference" }, onChange: IntentRef("ProductSpecVerifierRefChanged", ComponentValueBinding()), style: { flex: 1, minWidth: 0 } }),
        TextField({ key: "product-spec-verification-output-ref", value: workspace.verificationOutputRef, placeholder: "verification.output.ref", a11y: { label: "Verification output reference" }, onChange: IntentRef("ProductSpecVerificationOutputRefChanged", ComponentValueBinding()), style: { flex: 1, minWidth: 0 } }),
        TextField({ key: "product-spec-block-reason", value: workspace.blockedReason, placeholder: "Block reason", a11y: { label: "Packet block reason" }, onChange: IntentRef("ProductSpecBlockedReasonChanged", ComponentValueBinding()), style: { flex: 1, minWidth: 0 } }),
      ])]),
      ...plan.packets.map((packet) => packetView(workspace, packet, plan.packets)),
    ])]),
  ])
}
