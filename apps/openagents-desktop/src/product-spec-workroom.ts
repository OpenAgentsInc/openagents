import { createHash } from "node:crypto"
import {
  existsSync,
  mkdirSync,
  readFileSync,
  renameSync,
  chmodSync,
  writeFileSync,
} from "node:fs"
import { dirname, isAbsolute, relative, resolve, sep } from "node:path"

import {
  PRODUCT_SPEC_EXTENSION,
  starterProductSpec,
  validateExecutableProductSpec,
} from "@openagentsinc/product-spec"

import type {
  ProductSpecCreateRequest,
  ProductSpecEditConfirmation,
  ProductSpecEditConfirmRequest,
  ProductSpecEditProposal,
  ProductSpecEditProposalRequest,
  ProductSpecEvidenceRequest,
  ProductSpecIdentity,
  ProductSpecOpenRequest,
  ProductSpecOwnerDispositionRequest,
  ProductSpecOperationError,
  ProductSpecPacketAdmitRequest,
  ProductSpecPacketBlockRequest,
  ProductSpecPacketDispositionRequest,
  ProductSpecPlan,
  ProductSpecPlanAcceptRequest,
  ProductSpecPlanProposalRequest,
  ProductSpecProjection,
  ProductSpecRun,
  ProductSpecVerificationRequest,
  ProductSpecWorkPacket,
} from "./product-spec-workroom-contract.ts"

export type ProductSpecOperationResult<A> =
  | Readonly<{ ok: true; value: A; reconciled?: boolean }>
  | ProductSpecOperationError

export type ProductSpecWorkroom = Readonly<{
  open: (request: ProductSpecOpenRequest) => ProductSpecOperationResult<ProductSpecProjection>
  create: (request: ProductSpecCreateRequest) => ProductSpecOperationResult<ProductSpecProjection>
  proposeEdit: (request: ProductSpecEditProposalRequest) => ProductSpecOperationResult<ProductSpecEditProposal>
  confirmEdit: (request: ProductSpecEditConfirmRequest) => ProductSpecOperationResult<ProductSpecEditConfirmation>
  proposePlan: (request: ProductSpecPlanProposalRequest) => ProductSpecOperationResult<ProductSpecPlan>
  acceptPlan: (request: ProductSpecPlanAcceptRequest) => ProductSpecOperationResult<ProductSpecRun>
  admitPacket: (request: ProductSpecPacketAdmitRequest) => ProductSpecOperationResult<ProductSpecRun>
  blockPacket: (request: ProductSpecPacketBlockRequest) => ProductSpecOperationResult<ProductSpecRun>
  disposePacket: (request: ProductSpecPacketDispositionRequest) => ProductSpecOperationResult<ProductSpecRun>
  recordEvidence: (request: ProductSpecEvidenceRequest) => ProductSpecOperationResult<ProductSpecRun>
  verifyEvidence: (request: ProductSpecVerificationRequest) => ProductSpecOperationResult<ProductSpecRun>
  setOwnerDisposition: (request: ProductSpecOwnerDispositionRequest) => ProductSpecOperationResult<ProductSpecRun>
  run: (runRef: string) => ProductSpecOperationResult<ProductSpecRun>
}>

export type ProductSpecWorkroomOptions = Readonly<{
  workspaceRoot: string
  stateRoot: string
  now?: () => string
}>

const failure = (
  reason: ProductSpecOperationError["reason"],
  message: string,
): ProductSpecOperationError => ({ ok: false, reason, message })

const sha256 = (value: string | Buffer): string =>
  createHash("sha256").update(value).digest("hex")

const specDigest = (markdown: string): string => `sha256:${sha256(Buffer.from(markdown, "utf8"))}`

const identitiesEqual = (left: ProductSpecIdentity, right: ProductSpecIdentity): boolean =>
  left.specRef === right.specRef &&
  left.relativePath === right.relativePath &&
  left.revision === right.revision &&
  left.digest === right.digest

const unique = <A>(values: ReadonlyArray<A>): A[] => [...new Set(values)]

const exactEditDiff = (previous: string, next: string): string => {
  const before = previous.split("\n")
  const after = next.split("\n")
  let prefix = 0
  while (prefix < before.length && prefix < after.length && before[prefix] === after[prefix]) prefix += 1
  let suffix = 0
  while (suffix < before.length - prefix && suffix < after.length - prefix &&
    before[before.length - 1 - suffix] === after[after.length - 1 - suffix]) suffix += 1
  const removed = before.slice(prefix, before.length - suffix)
  const added = after.slice(prefix, after.length - suffix)
  return [
    "--- accepted ProductSpec",
    "+++ proposed ProductSpec",
    `@@ -${prefix + 1},${removed.length} +${prefix + 1},${added.length} @@`,
    ...removed.map(line => `-${line}`),
    ...added.map(line => `+${line}`),
  ].join("\n")
}

type StoredEditProposal = Readonly<{
  projection: ProductSpecEditProposal
  proposedMarkdown: string
}>

const replacePacket = (
  run: ProductSpecRun,
  packet: ProductSpecWorkPacket,
  now: string,
): ProductSpecRun => ({
  ...run,
  updatedAt: now,
  plan: {
    ...run.plan,
    packets: run.plan.packets.map(candidate =>
      candidate.packetRef === packet.packetRef ? packet : candidate),
  },
})

const hasDependencyCycle = (
  packets: ReadonlyArray<Readonly<{ packetRef: string; dependencyRefs: ReadonlyArray<string> }>>,
): boolean => {
  const byRef = new Map(packets.map(packet => [packet.packetRef, packet]))
  const visiting = new Set<string>()
  const visited = new Set<string>()
  const visit = (packetRef: string): boolean => {
    if (visiting.has(packetRef)) return true
    if (visited.has(packetRef)) return false
    visiting.add(packetRef)
    const packet = byRef.get(packetRef)
    if (packet !== undefined && packet.dependencyRefs.some(visit)) return true
    visiting.delete(packetRef)
    visited.add(packetRef)
    return false
  }
  return packets.some(packet => visit(packet.packetRef))
}

export const makeProductSpecWorkroom = (
  options: ProductSpecWorkroomOptions,
): ProductSpecWorkroom => {
  const workspaceRoot = resolve(options.workspaceRoot)
  const stateRoot = resolve(options.stateRoot)
  const now = options.now ?? (() => new Date().toISOString())
  mkdirSync(resolve(stateRoot, "plans"), { recursive: true })
  mkdirSync(resolve(stateRoot, "runs"), { recursive: true })
  mkdirSync(resolve(stateRoot, "edits"), { recursive: true })
  mkdirSync(resolve(stateRoot, "snapshots"), { recursive: true })

  const targetPath = (relativePath: string): string | null => {
    if (isAbsolute(relativePath) || !relativePath.endsWith(PRODUCT_SPEC_EXTENSION)) return null
    const target = resolve(workspaceRoot, relativePath)
    const fromRoot = relative(workspaceRoot, target)
    if (fromRoot === "" || fromRoot === ".." || fromRoot.startsWith(`..${sep}`) || isAbsolute(fromRoot)) {
      return null
    }
    return target
  }

  const atomicJson = (path: string, value: unknown): void => {
    mkdirSync(dirname(path), { recursive: true })
    const temp = `${path}.${process.pid}.tmp`
    writeFileSync(temp, `${JSON.stringify(value, null, 2)}\n`, { encoding: "utf8", mode: 0o600 })
    renameSync(temp, path)
  }

  const planFile = (planRef: string): string => resolve(stateRoot, "plans", `${planRef}.json`)
  const runFile = (runRef: string): string => resolve(stateRoot, "runs", `${runRef}.json`)
  const editFile = (proposalRef: string): string => resolve(stateRoot, "edits", `${proposalRef}.json`)
  const snapshotFile = (identity: ProductSpecIdentity): string =>
    resolve(stateRoot, "snapshots", `${identity.digest.slice("sha256:".length)}.product-spec.md`)
  const loadJson = <A>(path: string): A | null => {
    try {
      return JSON.parse(readFileSync(path, "utf8")) as A
    } catch {
      return null
    }
  }

  const open = (
    request: ProductSpecOpenRequest,
  ): ProductSpecOperationResult<ProductSpecProjection> => {
    const path = targetPath(request.relativePath)
    if (path === null) return failure("invalid_request", "ProductSpec path must remain inside the granted workspace.")
    let markdown: string
    try {
      markdown = readFileSync(path, "utf8")
    } catch {
      return failure("not_found", "The ProductSpec could not be read from the granted workspace.")
    }
    const validation = validateExecutableProductSpec(markdown)
    if (!validation.executable) {
      return {
        ok: true,
        value: {
          state: "invalid",
          relativePath: request.relativePath,
          sourceMarkdown: markdown,
          standardValid: validation.document !== undefined,
          executable: false,
          errors: validation.errors,
          warnings: validation.warnings,
        },
      }
    }
    const digest = specDigest(markdown)
    const identity: ProductSpecIdentity = {
      specRef: `product.spec.${digest.slice("sha256:".length, "sha256:".length + 24)}`,
      relativePath: request.relativePath,
      revision: validation.document.frontmatter.spec_revision!,
      digest,
    }
    return {
      ok: true,
      value: {
        state: "ready",
        title: validation.document.frontmatter.title,
        sourceMarkdown: markdown,
        identity,
        executable: true,
        criteria: validation.criteria.map(criterion => ({
          ...criterion,
          criterionRef: `${request.relativePath}@${identity.revision}+${identity.digest}#${criterion.id}`,
        })),
        warnings: validation.warnings,
      },
    }
  }

  const create = (
    request: ProductSpecCreateRequest,
  ): ProductSpecOperationResult<ProductSpecProjection> => {
    const path = targetPath(request.relativePath)
    if (path === null) return failure("invalid_request", "ProductSpec path must remain inside the granted workspace.")
    if (existsSync(path)) return failure("write_failed", "A ProductSpec already exists at that path.")
    try {
      mkdirSync(dirname(path), { recursive: true })
      writeFileSync(path, starterProductSpec({
        title: request.title,
        author: request.author ?? "OpenAgents",
        now: now(),
      }), { encoding: "utf8", flag: "wx" })
    } catch {
      return failure("write_failed", "The ProductSpec draft could not be created.")
    }
    return open({ workContextRef: request.workContextRef, relativePath: request.relativePath })
  }

  const proposeEdit = (
    request: ProductSpecEditProposalRequest,
  ): ProductSpecOperationResult<ProductSpecEditProposal> => {
    const current = open({
      workContextRef: request.workContextRef,
      relativePath: request.expectedCurrent.relativePath,
    })
    if (!current.ok || current.value.state !== "ready" ||
        !identitiesEqual(current.value.identity, request.expectedCurrent)) {
      return failure("proposal_stale", "The ProductSpec changed before this edit proposal was created.")
    }
    const validation = validateExecutableProductSpec(request.proposedMarkdown)
    if (!validation.executable) {
      return failure("not_executable", "The proposed ProductSpec revision is not executable.")
    }
    const nextDigest = specDigest(request.proposedMarkdown)
    const next: ProductSpecIdentity = {
      specRef: `product.spec.${nextDigest.slice("sha256:".length, "sha256:".length + 24)}`,
      relativePath: request.expectedCurrent.relativePath,
      revision: validation.document.frontmatter.spec_revision!,
      digest: nextDigest,
    }
    if (next.revision <= request.expectedCurrent.revision) {
      return failure("revision_not_incremented", "An intent edit must increase spec_revision before confirmation.")
    }
    if (next.digest === request.expectedCurrent.digest) {
      return failure("revision_not_incremented", "An intent edit must change the ProductSpec bytes.")
    }
    const previousById = new Map(current.value.criteria.map(criterion => [criterion.id, criterion]))
    const nextById = new Map(validation.criteria.map(criterion => [criterion.id, criterion]))
    const retainedCriterionIds = [...previousById.keys()].filter(id => nextById.has(id))
    const reconciliation = {
      retainedCriterionIds,
      changedCriterionIds: retainedCriterionIds.filter(id =>
        previousById.get(id)!.body !== nextById.get(id)!.body),
      addedCriterionIds: [...nextById.keys()].filter(id => !previousById.has(id)),
      removedCriterionIds: [...previousById.keys()].filter(id => !nextById.has(id)),
    }
    const proposalHash = sha256(JSON.stringify({
      workContextRef: request.workContextRef,
      previous: request.expectedCurrent,
      next,
      reconciliation,
      diff: exactEditDiff(current.value.sourceMarkdown, request.proposedMarkdown),
    }))
    const proposal: ProductSpecEditProposal = {
      proposalRef: `product.edit.${proposalHash.slice(0, 24)}`,
      workContextRef: request.workContextRef,
      previous: request.expectedCurrent,
      next,
      reconciliation,
      diff: exactEditDiff(current.value.sourceMarkdown, request.proposedMarkdown),
      proposedAt: now(),
      state: "proposed",
    }
    const existing = loadJson<StoredEditProposal>(editFile(proposal.proposalRef))
    if (existing !== null) return { ok: true, value: existing.projection, reconciled: true }
    try {
      atomicJson(editFile(proposal.proposalRef), { projection: proposal, proposedMarkdown: request.proposedMarkdown })
    } catch {
      return failure("write_failed", "The ProductSpec edit proposal could not be persisted.")
    }
    return { ok: true, value: proposal }
  }

  const confirmEdit = (
    request: ProductSpecEditConfirmRequest,
  ): ProductSpecOperationResult<ProductSpecEditConfirmation> => {
    const stored = loadJson<StoredEditProposal>(editFile(request.proposalRef))
    if (stored === null) return failure("not_found", "The ProductSpec edit proposal does not exist.")
    const proposal = stored.projection
    if (!identitiesEqual(proposal.previous, request.expectedCurrent)) {
      return failure("proposal_stale", "The confirmation does not match the proposed prior revision.")
    }
    if (proposal.state === "confirmed") {
      const reopened = open({
        workContextRef: proposal.workContextRef,
        relativePath: proposal.next.relativePath,
      })
      if (reopened.ok && reopened.value.state === "ready" &&
          identitiesEqual(reopened.value.identity, proposal.next)) {
        return { ok: true, value: { proposal, projection: reopened.value, reconciled: true, criterionDisposition: request.criterionDisposition }, reconciled: true }
      }
      return failure("proposal_stale", "The confirmed ProductSpec no longer matches its retained revision.")
    }
    const current = open({
      workContextRef: proposal.workContextRef,
      relativePath: proposal.previous.relativePath,
    })
    if (!current.ok || current.value.state !== "ready" ||
        !identitiesEqual(current.value.identity, proposal.previous)) {
      return failure("proposal_stale", "The ProductSpec changed before edit confirmation.")
    }
    const path = targetPath(proposal.next.relativePath)
    if (path === null) return failure("invalid_request", "The ProductSpec path is outside the granted workspace.")
    const temp = `${path}.${process.pid}.confirmed.tmp`
    try {
      writeFileSync(temp, stored.proposedMarkdown, { encoding: "utf8", mode: 0o600, flag: "wx" })
      renameSync(temp, path)
      if (process.platform !== "win32") chmodSync(path, 0o600)
    } catch {
      return failure("write_failed", "The confirmed ProductSpec revision could not be written.")
    }
    const reopened = open({ workContextRef: proposal.workContextRef, relativePath: proposal.next.relativePath })
    if (!reopened.ok || reopened.value.state !== "ready" ||
        !identitiesEqual(reopened.value.identity, proposal.next)) {
      return failure("write_failed", "The confirmed ProductSpec revision did not read back exactly.")
    }
    const confirmed: ProductSpecEditProposal = { ...proposal, state: "confirmed", confirmedAt: now() }
    try {
      atomicJson(editFile(confirmed.proposalRef), { projection: confirmed, proposedMarkdown: stored.proposedMarkdown })
    } catch {
      return failure("write_failed", "The ProductSpec confirmation receipt could not be persisted.")
    }
    return { ok: true, value: { proposal: confirmed, projection: reopened.value, reconciled: false, criterionDisposition: request.criterionDisposition } }
  }

  const proposePlan = (
    request: ProductSpecPlanProposalRequest,
  ): ProductSpecOperationResult<ProductSpecPlan> => {
    const projection = open({
      workContextRef: request.workContextRef,
      relativePath: request.spec.relativePath,
    })
    if (!projection.ok) return projection
    if (projection.value.state !== "ready") {
      return failure("not_executable", "The ProductSpec must be executable before a plan can be proposed.")
    }
    if (!identitiesEqual(projection.value.identity, request.spec)) {
      return failure("revision_mismatch", "The ProductSpec revision or digest changed before plan proposal.")
    }
    if (request.packets.length < 2) {
      return failure("invalid_plan", "An accepted ProductSpec plan requires at least two work packets.")
    }
    const packetRefs = request.packets.map(packet => packet.packetRef)
    if (unique(packetRefs).length !== packetRefs.length) {
      return failure("invalid_plan", "Work packet refs must be unique.")
    }
    const criterionById = new Map(projection.value.criteria.map(criterion => [criterion.id, criterion]))
    const deferred = unique(request.deferredCriterionIds)
    if (deferred.length !== request.deferredCriterionIds.length || deferred.some(id => !criterionById.has(id))) {
      return failure("invalid_plan", "Deferred criterion IDs must be unique and belong to the pinned ProductSpec.")
    }
    const signatures = new Set<string>()
    for (const packet of request.packets) {
      if (packet.criterionIds.length === 0 ||
          unique(packet.criterionIds).length !== packet.criterionIds.length ||
          packet.criterionIds.some(id => !criterionById.has(id))) {
        return failure("invalid_plan", `Packet ${packet.packetRef} has invalid criterion refs.`)
      }
      if (unique(packet.dependencyRefs).length !== packet.dependencyRefs.length ||
          packet.dependencyRefs.includes(packet.packetRef) ||
          packet.dependencyRefs.some(ref => !packetRefs.includes(ref))) {
        return failure("invalid_plan", `Packet ${packet.packetRef} has invalid dependencies.`)
      }
      const signature = [...packet.criterionIds].sort().join("|")
      if (signatures.has(signature)) {
        return failure("invalid_plan", "Duplicate criterion-equivalent work packets refuse.")
      }
      signatures.add(signature)
    }
    if (hasDependencyCycle(request.packets)) {
      return failure("invalid_plan", "Cyclic work packet dependencies refuse.")
    }
    if (!request.packets.some(packet => packet.allocation === "child")) {
      return failure("invalid_plan", "At least one work packet must be allocatable to a child agent.")
    }
    const covered = new Set(request.packets.flatMap(packet => packet.criterionIds))
    if (deferred.some(id => covered.has(id)) ||
        projection.value.criteria.some(criterion => !covered.has(criterion.id) && !deferred.includes(criterion.id))) {
      return failure("invalid_plan", "Every criterion must be mapped or explicitly deferred exactly once.")
    }
    const proposedAt = now()
    const planHash = sha256(JSON.stringify({
      spec: request.spec,
      workContextRef: request.workContextRef,
      packets: request.packets,
      deferred,
    }))
    const plan: ProductSpecPlan = {
      planRef: `product.plan.${planHash.slice(0, 24)}`,
      spec: request.spec,
      workContextRef: request.workContextRef,
      state: "proposed",
      packets: request.packets.map(packet => ({
        ...packet,
        criterionIds: [...packet.criterionIds],
        criterionRefs: packet.criterionIds.map(id => criterionById.get(id)!.criterionRef),
        dependencyRefs: [...packet.dependencyRefs],
        state: "planned",
        evidenceRefs: [],
        evidenceReceipts: [],
        verifierRefs: [],
        verificationReceipts: [],
        ownerDisposition: null,
        activeLease: null,
      })),
      deferredCriterionIds: deferred,
      proposedAt,
    }
    try {
      atomicJson(planFile(plan.planRef), plan)
    } catch {
      return failure("write_failed", "The proposed ProductSpec plan could not be persisted.")
    }
    return { ok: true, value: plan }
  }

  const loadRun = (runRef: string): ProductSpecOperationResult<ProductSpecRun> => {
    const value = loadJson<ProductSpecRun>(runFile(runRef))
    return value === null
      ? failure("not_found", "The ProductSpec run does not exist.")
      : { ok: true, value: {
          ...value,
          plan: {
            ...value.plan,
            packets: value.plan.packets.map(packet => ({
              ...packet,
              // Forward-read legacy MVP run files without inventing proof.
              evidenceReceipts: packet.evidenceReceipts ?? [],
              verificationReceipts: packet.verificationReceipts ?? [],
              ownerDisposition: packet.ownerDisposition ?? null,
            })),
          },
        } }
  }

  const persistRun = (run: ProductSpecRun): ProductSpecOperationResult<ProductSpecRun> => {
    try {
      atomicJson(runFile(run.runRef), run)
      atomicJson(planFile(run.plan.planRef), run.plan)
      return { ok: true, value: run }
    } catch {
      return failure("write_failed", "The ProductSpec run transition could not be persisted.")
    }
  }

  const acceptPlan = (
    request: ProductSpecPlanAcceptRequest,
  ): ProductSpecOperationResult<ProductSpecRun> => {
    const plan = loadJson<ProductSpecPlan>(planFile(request.planRef))
    if (plan === null) return failure("not_found", "The proposed ProductSpec plan does not exist.")
    if (!identitiesEqual(plan.spec, request.expectedSpec)) {
      return failure("revision_mismatch", "The accepted ProductSpec identity does not match the proposed plan.")
    }
    const projection = open({ workContextRef: plan.workContextRef, relativePath: plan.spec.relativePath })
    if (!projection.ok || projection.value.state !== "ready" ||
        !identitiesEqual(projection.value.identity, request.expectedSpec)) {
      return failure("revision_mismatch", "The ProductSpec changed before plan acceptance.")
    }
    const runRef = `product.run.${sha256(`${plan.workContextRef}:${plan.planRef}`).slice(0, 24)}`
    const existing = loadJson<ProductSpecRun>(runFile(runRef))
    if (existing !== null) return { ok: true, value: existing, reconciled: true }
    if (plan.state !== "proposed") return failure("invalid_transition", "Only a proposed plan can be accepted.")
    const acceptedAt = now()
    const acceptedPlan: ProductSpecPlan = { ...plan, state: "accepted", acceptedAt }
    const run: ProductSpecRun = {
      runRef,
      spec: plan.spec,
      workContextRef: plan.workContextRef,
      plan: acceptedPlan,
      createdAt: acceptedAt,
      updatedAt: acceptedAt,
    }
    try {
      const specPath = targetPath(plan.spec.relativePath)
      if (specPath === null) return failure("invalid_request", "The accepted ProductSpec path is invalid.")
      const bytes = readFileSync(specPath)
      if (`sha256:${sha256(bytes)}` !== plan.spec.digest) {
        return failure("revision_mismatch", "The ProductSpec changed before its immutable run snapshot was retained.")
      }
      const destination = snapshotFile(plan.spec)
      if (!existsSync(destination)) writeFileSync(destination, bytes, { flag: "wx", mode: 0o600 })
    } catch {
      return failure("write_failed", "The immutable ProductSpec run snapshot could not be retained.")
    }
    return persistRun(run)
  }

  const ensureCurrentSpec = (run: ProductSpecRun): ProductSpecOperationResult<ProductSpecRun> => {
    const projection = open({ workContextRef: run.workContextRef, relativePath: run.spec.relativePath })
    if (!projection.ok || projection.value.state !== "ready" ||
        !identitiesEqual(projection.value.identity, run.spec)) {
      const mismatch: ProductSpecRun = {
        ...run,
        updatedAt: now(),
        plan: { ...run.plan, state: "revision_mismatch" },
      }
      const persisted = persistRun(mismatch)
      return persisted.ok
        ? failure("revision_mismatch", "The ProductSpec revision or digest changed; dispatch is stopped.")
        : persisted
    }
    return { ok: true, value: run }
  }

  const admitPacket = (
    request: ProductSpecPacketAdmitRequest,
  ): ProductSpecOperationResult<ProductSpecRun> => {
    const loaded = loadRun(request.runRef)
    if (!loaded.ok) return loaded
    if (!identitiesEqual(loaded.value.spec, request.expectedSpec)) {
      return failure("revision_mismatch", "Packet admission does not match the accepted ProductSpec.")
    }
    const current = ensureCurrentSpec(loaded.value)
    if (!current.ok) return current
    if (current.value.plan.state !== "accepted") {
      return failure("plan_not_accepted", "Work dispatch requires an accepted ProductSpec plan.")
    }
    const packet = current.value.plan.packets.find(candidate => candidate.packetRef === request.packetRef)
    if (packet === undefined) return failure("packet_not_found", "The work packet does not exist.")
    if (packet.state === "active") {
      return packet.activeLease?.leaseRef === request.leaseRef &&
        packet.activeLease.executorRef === request.executorRef
        ? { ok: true, value: current.value, reconciled: true }
        : failure("lease_conflict", "The work packet already has an active mutation lease.")
    }
    if (packet.state !== "planned" && packet.state !== "blocked") {
      return failure("invalid_transition", `A ${packet.state} packet cannot be admitted.`)
    }
    const dependencies = packet.dependencyRefs.map(ref =>
      current.value.plan.packets.find(candidate => candidate.packetRef === ref))
    if (dependencies.some(dependency => dependency?.state !== "verified")) {
      return failure("dependency_not_verified", "Every packet dependency must be verified before admission.")
    }
    const nextPacket: ProductSpecWorkPacket = {
      ...packet,
      state: "active",
      blockedReason: undefined,
      activeLease: {
        leaseRef: request.leaseRef,
        executorRef: request.executorRef,
        executionMode: request.executionMode,
        admittedAt: now(),
      },
    }
    return persistRun(replacePacket(current.value, nextPacket, now()))
  }

  const blockPacket = (
    request: ProductSpecPacketBlockRequest,
  ): ProductSpecOperationResult<ProductSpecRun> => {
    const loaded = loadRun(request.runRef)
    if (!loaded.ok) return loaded
    if (!identitiesEqual(loaded.value.spec, request.expectedSpec)) {
      return failure("revision_mismatch", "Packet blocking does not match the accepted ProductSpec.")
    }
    const current = ensureCurrentSpec(loaded.value)
    if (!current.ok) return current
    const packet = current.value.plan.packets.find(candidate => candidate.packetRef === request.packetRef)
    if (packet === undefined) return failure("packet_not_found", "The work packet does not exist.")
    if (packet.state !== "active" || packet.activeLease?.leaseRef !== request.leaseRef) {
      return failure("invalid_transition", "Only the active lease can block this packet.")
    }
    return persistRun(replacePacket(current.value, {
      ...packet,
      state: "blocked",
      blockedReason: request.reason,
      activeLease: null,
    }, now()))
  }

  const recordEvidence = (
    request: ProductSpecEvidenceRequest,
  ): ProductSpecOperationResult<ProductSpecRun> => {
    const loaded = loadRun(request.runRef)
    if (!loaded.ok) return loaded
    if (!identitiesEqual(loaded.value.spec, request.expectedSpec)) {
      return failure("revision_mismatch", "Packet evidence does not match the accepted ProductSpec.")
    }
    const current = ensureCurrentSpec(loaded.value)
    if (!current.ok) return current
    const packet = current.value.plan.packets.find(candidate => candidate.packetRef === request.packetRef)
    if (packet === undefined) return failure("packet_not_found", "The work packet does not exist.")
    const evidenceReceiptRef = `receipt.evidence.${sha256([
      current.value.runRef,
      packet.packetRef,
      request.leaseRef,
      request.evidenceRef,
      request.evidenceKind,
      current.value.spec.digest,
    ].join("\n")).slice(0, 32)}`
    if (packet.state === "evidence_present" && (packet.evidenceReceipts ?? []).some(receipt => receipt.receiptRef === evidenceReceiptRef)) {
      return { ok: true, value: current.value, reconciled: true }
    }
    if (packet.state !== "active" || packet.activeLease?.leaseRef !== request.leaseRef) {
      return failure("invalid_transition", "Only the active lease can record terminal packet evidence.")
    }
    return persistRun(replacePacket(current.value, {
      ...packet,
      state: "evidence_present",
      evidenceRefs: unique([...packet.evidenceRefs, request.evidenceRef]),
      evidenceReceipts: [...(packet.evidenceReceipts ?? []), {
        receiptRef: evidenceReceiptRef,
        evidenceRef: request.evidenceRef,
        kind: request.evidenceKind,
        producerRef: packet.activeLease.executorRef,
        spec: current.value.spec,
        criterionIds: packet.criterionIds,
        producedAt: now(),
      }],
      evidenceProducerRef: packet.activeLease.executorRef,
      activeLease: null,
    }, now()))
  }

  const disposePacket = (
    request: ProductSpecPacketDispositionRequest,
  ): ProductSpecOperationResult<ProductSpecRun> => {
    const loaded = loadRun(request.runRef)
    if (!loaded.ok) return loaded
    if (!identitiesEqual(loaded.value.spec, request.expectedSpec)) {
      return failure("revision_mismatch", "Packet disposition does not match the accepted ProductSpec.")
    }
    const current = ensureCurrentSpec(loaded.value)
    if (!current.ok) return current
    const packet = current.value.plan.packets.find(candidate => candidate.packetRef === request.packetRef)
    if (packet === undefined) return failure("packet_not_found", "The work packet does not exist.")
    if (packet.state === request.disposition && packet.blockedReason === request.reason) {
      return { ok: true, value: current.value, reconciled: true }
    }
    if (packet.state === "verified" || packet.state === "failed" || packet.state === "cancelled" || packet.state === "superseded") {
      return failure("invalid_transition", `A ${packet.state} packet cannot become ${request.disposition}.`)
    }
    return persistRun(replacePacket(current.value, {
      ...packet,
      state: request.disposition,
      blockedReason: request.reason,
      activeLease: null,
    }, now()))
  }

  const verifyEvidence = (
    request: ProductSpecVerificationRequest,
  ): ProductSpecOperationResult<ProductSpecRun> => {
    const loaded = loadRun(request.runRef)
    if (!loaded.ok) return loaded
    if (!identitiesEqual(loaded.value.spec, request.expectedSpec)) {
      return failure("revision_mismatch", "Packet verification does not match the accepted ProductSpec.")
    }
    const current = ensureCurrentSpec(loaded.value)
    if (!current.ok) return current
    const packet = current.value.plan.packets.find(candidate => candidate.packetRef === request.packetRef)
    if (packet === undefined) return failure("packet_not_found", "The work packet does not exist.")
    const resolvedEvidence = request.evidenceReceiptRefs.map(receiptRef =>
      (packet.evidenceReceipts ?? []).find(receipt => receipt.receiptRef === receiptRef))
    if (resolvedEvidence.some(receipt => receipt === undefined)) {
      return failure("evidence_required", "Verification must resolve every linked evidence receipt on this packet.")
    }
    if (resolvedEvidence.some(receipt => !identitiesEqual(receipt!.spec, current.value.spec))) {
      return failure("revision_mismatch", "Verification evidence belongs to a different ProductSpec revision.")
    }
    const verificationReceiptRef = `receipt.verification.${sha256([
      current.value.runRef,
      packet.packetRef,
      request.verifierRef,
      request.outputRef,
      ...request.evidenceReceiptRefs,
      current.value.spec.digest,
    ].join("\n")).slice(0, 32)}`
    if (packet.state === "verified" && (packet.verificationReceipts ?? []).some(receipt => receipt.receiptRef === verificationReceiptRef)) {
      return { ok: true, value: current.value, reconciled: true }
    }
    if (packet.state !== "evidence_present" || packet.evidenceRefs.length === 0) {
      return failure("evidence_required", "Evidence-present is required before independent verification.")
    }
    if (packet.evidenceProducerRef === undefined || packet.evidenceProducerRef === request.verifierRef) {
      return failure("verifier_required", "Independent verification requires a verifier distinct from the evidence producer.")
    }
    return persistRun(replacePacket(current.value, {
      ...packet,
      state: "verified",
      verifierRefs: unique([...packet.verifierRefs, request.verifierRef]),
      verificationReceipts: [...(packet.verificationReceipts ?? []), {
        receiptRef: verificationReceiptRef,
        evidenceReceiptRefs: request.evidenceReceiptRefs,
        outputRef: request.outputRef,
        verifierRef: request.verifierRef,
        spec: current.value.spec,
        criterionIds: packet.criterionIds,
        verdict: "passed",
        verifiedAt: now(),
      }],
    }, now()))
  }

  const setOwnerDisposition = (
    request: ProductSpecOwnerDispositionRequest,
  ): ProductSpecOperationResult<ProductSpecRun> => {
    const loaded = loadRun(request.runRef)
    if (!loaded.ok) return loaded
    if (!identitiesEqual(loaded.value.spec, request.expectedSpec)) {
      return failure("revision_mismatch", "Owner disposition does not match the accepted ProductSpec.")
    }
    const current = ensureCurrentSpec(loaded.value)
    if (!current.ok) return current
    const packet = current.value.plan.packets.find(candidate => candidate.packetRef === request.packetRef)
    if (packet === undefined) return failure("packet_not_found", "The work packet does not exist.")
    if (packet.ownerDisposition?.disposition === request.disposition &&
      packet.ownerDisposition.ownerRef === request.ownerRef &&
      packet.ownerDisposition.reason === request.reason) {
      return { ok: true, value: current.value, reconciled: true }
    }
    if (packet.state !== "verified" || packet.verificationReceipts.length === 0) {
      return failure("invalid_transition", "Owner acceptance or waiver requires an independently verified packet.")
    }
    if (request.disposition === "waived" && request.reason === undefined) {
      return failure("invalid_request", "An owner waiver requires a bounded reason.")
    }
    return persistRun(replacePacket(current.value, {
      ...packet,
      ownerDisposition: {
        disposition: request.disposition,
        ownerRef: request.ownerRef,
        reason: request.reason,
        decidedAt: now(),
      },
    }, now()))
  }

  return {
    open,
    create,
    proposeEdit,
    confirmEdit,
    proposePlan,
    acceptPlan,
    admitPacket,
    blockPacket,
    disposePacket,
    recordEvidence,
    verifyEvidence,
    setOwnerDisposition,
    run: loadRun,
  }
}
