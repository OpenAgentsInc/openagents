import type {
  PortableAttachment,
  PortableCapabilityLease,
  PortableCheckpoint,
  PortableCodingSession,
  PortableSessionCommand,
  PortableTargetDescriptor,
} from "./index.js"

export type PortableSessionSnapshot = {
  session: PortableCodingSession
  targets: ReadonlyArray<PortableTargetDescriptor>
  attachments: ReadonlyArray<PortableAttachment>
  checkpoints: ReadonlyArray<PortableCheckpoint>
  leases: ReadonlyArray<PortableCapabilityLease>
  pendingCommands: ReadonlyArray<PortableSessionCommand>
  topLevelCatalogSessionRefs: ReadonlyArray<string>
}

export type PortableSessionInvariantCode =
  | "session_identity_not_owner_minted"
  | "local_adoption_receipt_missing"
  | "duplicate_agent_ref"
  | "root_agent_missing"
  | "root_agent_has_parent"
  | "agent_parent_missing"
  | "agent_cycle"
  | "child_leaked_to_top_level_catalog"
  | "duplicate_attachment_generation"
  | "multiple_live_attachments"
  | "attachment_descendant_set_incomplete"
  | "agent_generation_ahead_of_attachment"
  | "attachment_target_missing"
  | "checkpoint_source_missing"
  | "checkpoint_generation_mismatch"
  | "checkpoint_secret_or_process_state_included"
  | "lease_attachment_missing"
  | "lease_scope_mismatch"
  | "active_attachment_uses_inactive_lease"
  | "command_source_is_stale"
  | "move_destination_missing"
  | "silent_target_change"

export type PortableSessionInvariantViolation = {
  readonly code: PortableSessionInvariantCode
  readonly subjectRef: string
  readonly detail: string
}

const liveAttachmentStates = new Set(["preparing", "active", "quiescing"])
const activeLeaseStates = new Set(["issued", "redeemed"])

function push(
  violations: PortableSessionInvariantViolation[],
  code: PortableSessionInvariantCode,
  subjectRef: string,
  detail: string,
): void {
  violations.push({ code, subjectRef, detail })
}

export function auditPortableSessionSnapshot(
  snapshot: PortableSessionSnapshot,
): ReadonlyArray<PortableSessionInvariantViolation> {
  const violations: PortableSessionInvariantViolation[] = []
  const { session } = snapshot

  if (session.identityBasis !== "owner_minted") {
    push(violations, "session_identity_not_owner_minted", session.sessionRef,
      "session identity must be minted by owner authority")
  }
  if (session.adoptedFromLocalHistory && !session.adoptionReceiptRef) {
    push(violations, "local_adoption_receipt_missing", session.sessionRef,
      "adopted local history requires an explicit bounded adoption receipt")
  }

  const nodes = new Map<string, (typeof session.graph.nodes)[number]>()
  for (const node of session.graph.nodes) {
    if (nodes.has(node.agentRef)) {
      push(violations, "duplicate_agent_ref", node.agentRef,
        "canonical graph contains a duplicate agent ref")
    }
    nodes.set(node.agentRef, node)
  }
  const root = nodes.get(session.graph.rootAgentRef)
  if (!root) {
    push(violations, "root_agent_missing", session.graph.rootAgentRef,
      "canonical graph root does not exist")
  } else if (root.parentAgentRef) {
    push(violations, "root_agent_has_parent", root.agentRef,
      "canonical graph root cannot have a parent")
  }

  for (const node of nodes.values()) {
    if (node.parentAgentRef && !nodes.has(node.parentAgentRef)) {
      push(violations, "agent_parent_missing", node.agentRef,
        `parent ${node.parentAgentRef} is absent from canonical graph`)
    }
    const visited = new Set<string>([node.agentRef])
    let cursor = node
    while (cursor.parentAgentRef) {
      if (visited.has(cursor.parentAgentRef)) {
        push(violations, "agent_cycle", node.agentRef,
          "canonical parent edges contain a cycle")
        break
      }
      visited.add(cursor.parentAgentRef)
      const parent = nodes.get(cursor.parentAgentRef)
      if (!parent) break
      cursor = parent
    }
    if (node.parentAgentRef && snapshot.topLevelCatalogSessionRefs.includes(node.threadRef)) {
      push(violations, "child_leaked_to_top_level_catalog", node.agentRef,
        "child thread cannot be projected as a top-level session")
    }
  }

  const targetRefs = new Set(snapshot.targets.map((target) => target.targetRef))
  const attachments = new Map(snapshot.attachments.map((item) => [item.attachmentRef, item]))
  const generations = new Set<string>()
  const live = snapshot.attachments.filter((item) => liveAttachmentStates.has(item.state))
  if (live.length > 1) {
    for (const attachment of live) {
      push(violations, "multiple_live_attachments", attachment.attachmentRef,
        "at most one attachment generation may accept or prepare work")
    }
  }
  const expectedDescendants = new Set(session.graph.nodes.map((node) => node.agentRef))
  for (const attachment of snapshot.attachments) {
    const generationKey = `${attachment.sessionRef}:${attachment.generation}`
    if (generations.has(generationKey)) {
      push(violations, "duplicate_attachment_generation", attachment.attachmentRef,
        "one session cannot reuse an attachment generation")
    }
    generations.add(generationKey)
    if (!targetRefs.has(attachment.targetRef)) {
      push(violations, "attachment_target_missing", attachment.attachmentRef,
        "attachment target is absent from the authorized directory")
    }
    const descendants = new Set(attachment.descendantAgentRefs)
    if (attachment.state !== "reclaimed" &&
        [...expectedDescendants].some((agentRef) => !descendants.has(agentRef))) {
      push(violations, "attachment_descendant_set_incomplete", attachment.attachmentRef,
        "attachment fencing must cover root and every descendant")
    }
    for (const node of session.graph.nodes) {
      if (node.attachmentGeneration > attachment.generation &&
          liveAttachmentStates.has(attachment.state)) {
        push(violations, "agent_generation_ahead_of_attachment", node.agentRef,
          "live attachment cannot own an agent from a newer generation")
      }
    }
  }

  for (const checkpoint of snapshot.checkpoints) {
    const source = attachments.get(checkpoint.sourceAttachmentRef)
    if (!source) {
      push(violations, "checkpoint_source_missing", checkpoint.checkpointRef,
        "checkpoint source attachment is absent")
    } else if (source.generation !== checkpoint.sourceGeneration) {
      push(violations, "checkpoint_generation_mismatch", checkpoint.checkpointRef,
        "checkpoint must bind the exact source generation")
    }
    if (checkpoint.secretMaterial !== "excluded" || checkpoint.processState !== "excluded") {
      push(violations, "checkpoint_secret_or_process_state_included", checkpoint.checkpointRef,
        "credentials and live process state are never portable")
    }
  }

  for (const lease of snapshot.leases) {
    const attachment = attachments.get(lease.attachmentRef)
    if (!attachment) {
      push(violations, "lease_attachment_missing", lease.leaseRef,
        "lease attachment is absent")
      continue
    }
    if (lease.sessionRef !== attachment.sessionRef ||
        lease.targetRef !== attachment.targetRef ||
        lease.attachmentGeneration !== attachment.generation) {
      push(violations, "lease_scope_mismatch", lease.leaseRef,
        "lease must bind exact session, target, attachment, and generation")
    }
    if (attachment.state === "active" && !activeLeaseStates.has(lease.state)) {
      push(violations, "active_attachment_uses_inactive_lease", lease.leaseRef,
        "active attachment cannot rely on revoked, expired, or released capability")
    }
  }

  const active = live.find((item) => item.state === "active")
  for (const command of snapshot.pendingCommands) {
    const source = attachments.get(command.expectedAttachmentRef)
    if (!source || source.generation !== command.expectedGeneration ||
        (active && active.attachmentRef !== source.attachmentRef)) {
      push(violations, "command_source_is_stale", command.commandRef,
        "command expected attachment/generation is not authoritative")
    }
    if (["move", "attach", "failback"].includes(command.kind)) {
      if (!command.destinationTargetRef || !targetRefs.has(command.destinationTargetRef)) {
        push(violations, "move_destination_missing", command.commandRef,
          "movement requires an explicit authorized destination target")
      }
    }
    if (command.destinationTargetRef && active &&
        command.destinationTargetRef !== active.targetRef && command.kind === "resume") {
      push(violations, "silent_target_change", command.commandRef,
        "resume cannot silently change execution target; use move or failback")
    }
  }

  return violations
}

export function assertPortableSessionSnapshot(
  snapshot: PortableSessionSnapshot,
): void {
  const violations = auditPortableSessionSnapshot(snapshot)
  if (violations.length > 0) {
    throw new Error(violations.map((item) => `${item.code}:${item.subjectRef}`).join(","))
  }
}
