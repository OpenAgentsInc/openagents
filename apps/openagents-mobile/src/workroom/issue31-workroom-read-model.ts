import { Schema } from "effect";
import {
  decodeIssue31HostAdjunct,
  Issue31CommandStateSchema,
} from "@openagentsinc/sarah/issue31-workroom";
import type {
  Issue31CommandState,
  Issue31HostAdjunct,
  Issue31HostProjection,
} from "@openagentsinc/sarah/issue31-workroom";

export const ISSUE31_WORKROOM_READ_MODEL_SCHEMA =
  "openagents.mobile.issue31_workroom_read_model.v1" as const;

export const ISSUE31_CAPABILITY_IDS = [
  "connection_and_identity",
  "owner_private_sarah",
  "memory",
  "read_state_and_reminders",
  "attention_and_receipts",
  "full_auto",
  "provider_accounts",
  "evidence_chain",
  "community_membership",
  "community_work",
  "experience",
] as const;

export type Issue31CapabilityId = (typeof ISSUE31_CAPABILITY_IDS)[number];
export type Issue31WorkroomRoom = "owner_private" | "community";
export type Issue31CapabilityRoom = Issue31WorkroomRoom | "shared";

export type Issue31SourceAuthority = "signed_nostr_record" | "omega_host_adjunct";
export type Issue31SourceStatus = "ready" | "unavailable" | "gap";
export type Issue31SourceFreshness = "live" | "recent" | "stale" | "unknown";
export type Issue31RoleState =
  | "owner"
  | "member"
  | "agent_operator"
  | "verifier"
  | "read_only"
  | "none";

const PublicRef = Schema.String.check(
  Schema.isMinLength(3),
  Schema.isMaxLength(256),
  Schema.isPattern(/^[a-z][a-z0-9_-]*(?:\.[A-Za-z0-9][A-Za-z0-9_-]*){1,}(?::[A-Za-z0-9._-]+)?$/),
);
const EventRef = Schema.String.check(Schema.isPattern(/^[0-9a-f]{64}$/));
const IsoTime = Schema.String.check(
  Schema.isMinLength(20),
  Schema.isMaxLength(32),
  Schema.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/),
);

export const Issue31ActionStateSchema = Issue31CommandStateSchema;
export type Issue31ActionState = Issue31CommandState;

export const Issue31SourceSnapshotSchema = Schema.Struct({
  capabilityId: Schema.Literals(ISSUE31_CAPABILITY_IDS),
  authority: Schema.Literals(["signed_nostr_record", "omega_host_adjunct"]),
  sourceRef: PublicRef,
  status: Schema.Literals(["ready", "unavailable", "gap"]),
  freshness: Schema.Literals(["live", "recent", "stale", "unknown"]),
  observedAt: Schema.NullOr(IsoTime),
  recordRefs: Schema.Array(Schema.Union([PublicRef, EventRef])).check(Schema.isMaxLength(64)),
  reasonRef: Schema.NullOr(PublicRef),
  role: Schema.Literals(["owner", "member", "agent_operator", "verifier", "read_only", "none"]),
  roleStatus: Schema.Literals(["active", "revoked", "unknown"]),
  actionState: Issue31ActionStateSchema,
});

export interface Issue31SourceSnapshot extends Schema.Schema.Type<
  typeof Issue31SourceSnapshotSchema
> {}

export interface Issue31CapabilityDescriptor {
  readonly id: Issue31CapabilityId;
  readonly room: Issue31CapabilityRoom;
  readonly label: string;
  readonly ownerCanSee: string;
  readonly permittedUserCanDo: string;
  readonly expectedAuthority: Issue31SourceAuthority;
  readonly sourceRef: string;
  readonly protocolRefs: ReadonlyArray<string>;
}

export interface Issue31CapabilityProjection extends Issue31CapabilityDescriptor {
  readonly source: Issue31SourceSnapshot;
  readonly hostObservation: Readonly<{
    hostRef: string;
    snapshotRef: string;
    generatedAtMs: number;
    projection: Issue31HostProjection;
  }> | null;
}

export interface Issue31WorkroomReadModel {
  readonly schema: typeof ISSUE31_WORKROOM_READ_MODEL_SCHEMA;
  readonly projectedAt: string;
  readonly rows: ReadonlyArray<Issue31CapabilityProjection>;
  readonly coverage: Readonly<{
    total: number;
    ready: number;
    unavailable: number;
    gaps: number;
    pending: number;
    refused: number;
    terminal: number;
  }>;
}

export const ISSUE31_CAPABILITY_DESCRIPTORS: ReadonlyArray<Issue31CapabilityDescriptor> = [
  {
    id: "connection_and_identity",
    room: "shared",
    label: "Connection and identity",
    ownerCanSee: "Device key, Omega key, owner binding, grant, relay, freshness, and revocation",
    permittedUserCanDo: "Pair, renew, or revoke a device without a cloud login gate",
    expectedAuthority: "signed_nostr_record",
    sourceRef: "source.issue31.nostr.host_pairing",
    protocolRefs: ["nostr.device_identity", "nostr.host_announcement", "nostr.pairing_grant"],
  },
  {
    id: "owner_private_sarah",
    room: "owner_private",
    label: "Owner-private Sarah",
    ownerCanSee: "Signed transcript, activity, state, and gaps",
    permittedUserCanDo: "Send, interrupt, retry a safe failure, or open its receipt",
    expectedAuthority: "signed_nostr_record",
    sourceRef: "source.issue31.sarah.nostr_turn",
    protocolRefs: ["nip17", "nip44", "nip59", "openagents.sarah.nostr_turn.v1"],
  },
  {
    id: "memory",
    room: "owner_private",
    label: "Memory",
    ownerCanSee: "Owner-decryptable engrams and local search results",
    permittedUserCanDo:
      "Inspect memory or remove local search data without deleting the signed record",
    expectedAuthority: "signed_nostr_record",
    sourceRef: "source.issue31.sarah.nostr_memory",
    protocolRefs: ["nip_ae", "openagents.sarah.nip_ae_companion.v1"],
  },
  {
    id: "read_state_and_reminders",
    room: "owner_private",
    label: "Read state and reminders",
    ownerCanSee: "Cross-device cursor and reminder state",
    permittedUserCanDo: "Mark read, create, change, dismiss, or expire reminders",
    expectedAuthority: "signed_nostr_record",
    sourceRef: "source.issue31.sarah.read_reminder",
    protocolRefs: ["nip_rs", "nip_er"],
  },
  {
    id: "attention_and_receipts",
    room: "owner_private",
    label: "Attention and receipts",
    ownerCanSee: "Exact message, run, decision, and result targets",
    permittedUserCanDo: "Open the exact target and inspect its authority state",
    expectedAuthority: "signed_nostr_record",
    sourceRef: "source.issue31.sarah.attention_receipts",
    protocolRefs: ["openagents.sarah.nostr_turn.v1", "nip_rs", "nip_er"],
  },
  {
    id: "full_auto",
    room: "shared",
    label: "Full Auto",
    ownerCanSee: "Objective, lane, exact time, state, live work, and terminal reason",
    permittedUserCanDo: "Pause, resume, stop, or ask Sarah about a run",
    expectedAuthority: "omega_host_adjunct",
    sourceRef: "source.issue31.omega.full_auto",
    protocolRefs: ["openagents.omega.issue31.host.v1"],
  },
  {
    id: "provider_accounts",
    room: "shared",
    label: "Provider accounts",
    ownerCanSee: "Provider, account label, readiness, quota, and lane mapping",
    permittedUserCanDo: "Request a safe connect handoff and follow host-owned progress",
    expectedAuthority: "omega_host_adjunct",
    sourceRef: "source.issue31.omega.provider_accounts",
    protocolRefs: ["openagents.omega.issue31.host.v1"],
  },
  {
    id: "evidence_chain",
    room: "shared",
    label: "Evidence chain",
    ownerCanSee: "Objective, turn, change, test, host verification, decision, and receipt",
    permittedUserCanDo: "Open each bounded hop and reject a broken or mismatched chain",
    expectedAuthority: "omega_host_adjunct",
    sourceRef: "source.issue31.omega.evidence_chain",
    protocolRefs: ["openagents.omega.issue31.host.v1"],
  },
  {
    id: "community_membership",
    room: "community",
    label: "Community membership",
    ownerCanSee: "Group, member, agent, attestation, persona, grant, and revocation",
    permittedUserCanDo: "Invite, join, attach an agent, or revoke when the signed role permits it",
    expectedAuthority: "signed_nostr_record",
    sourceRef: "source.issue31.sarah.community_membership",
    protocolRefs: ["nip29", "nip_oa", "nip_ap", "openagents.sarah.community_membership.v1"],
  },
  {
    id: "community_work",
    room: "community",
    label: "Community work",
    ownerCanSee:
      "Work unit, quote, accepted provider, result, verification, rejection, dispute, and appeal",
    permittedUserCanDo: "Take the role-scoped action for the current lifecycle state",
    expectedAuthority: "signed_nostr_record",
    sourceRef: "source.issue31.sarah.community_work",
    protocolRefs: ["nip29", "nip_lbr", "openagents.sarah.lbr_request_quote.v1"],
  },
  {
    id: "experience",
    room: "community",
    label: "Experience",
    ownerCanSee: "Accepted awards, recomputed total, scorer rank, and badges",
    permittedUserCanDo: "Inspect an award source and detect a rank mismatch",
    expectedAuthority: "signed_nostr_record",
    sourceRef: "source.issue31.sarah.experience",
    protocolRefs: ["nip32", "nip58", "nip85", "openagents.sarah.xp_rank.v1"],
  },
];

const decodeSourceSnapshot = Schema.decodeUnknownSync(Issue31SourceSnapshotSchema);

export const decodeIssue31SourceSnapshot = (value: unknown): Issue31SourceSnapshot => {
  const source = decodeSourceSnapshot(value, { onExcessProperty: "error" });
  if (source.observedAt !== null && !Number.isFinite(Date.parse(source.observedAt))) {
    throw new Error(`Issue 31 source ${source.capabilityId} has an invalid observation time.`);
  }
  const descriptor = ISSUE31_CAPABILITY_DESCRIPTORS.find((row) => row.id === source.capabilityId);
  if (
    descriptor === undefined ||
    descriptor.expectedAuthority !== source.authority ||
    descriptor.sourceRef !== source.sourceRef
  ) {
    throw new Error(`Issue 31 source identity does not match ${source.capabilityId}.`);
  }
  if (source.status === "ready" && source.recordRefs.length === 0) {
    throw new Error(`Issue 31 source ${source.capabilityId} needs an authority record.`);
  }
  if (
    (source.status === "unavailable" || source.status === "gap") &&
    source.reasonRef === null
  ) {
    throw new Error(`Issue 31 source ${source.capabilityId} needs a reason reference.`);
  }
  if (source.status === "ready" && (source.freshness === "unknown" || source.observedAt === null)) {
    throw new Error(`Issue 31 source ${source.capabilityId} needs a fresh observation.`);
  }
  if (source.roleStatus === "active" && source.role === "none") {
    throw new Error(`Issue 31 source ${source.capabilityId} has an invalid active role.`);
  }
  if (
    source.authority === "signed_nostr_record" &&
    source.recordRefs.some((ref) => !/^[0-9a-f]{64}$/.test(ref))
  ) {
    throw new Error(`Issue 31 Nostr source ${source.capabilityId} needs signed event references.`);
  }
  return source;
};

const unavailableSource = (descriptor: Issue31CapabilityDescriptor): Issue31SourceSnapshot => ({
  capabilityId: descriptor.id,
  authority: descriptor.expectedAuthority,
  sourceRef: descriptor.sourceRef,
  status: "unavailable",
  freshness: "unknown",
  observedAt: null,
  recordRefs: [],
  reasonRef: `reason.issue31.source_not_connected:${descriptor.id}`,
  role: "none",
  roleStatus: "unknown",
  actionState: { kind: "idle" },
});

const hostCapabilityIds: Readonly<
  Record<Issue31HostProjection["capability"], Issue31CapabilityId>
> = {
  connection_identity: "connection_and_identity",
  full_auto_runs: "full_auto",
  provider_accounts: "provider_accounts",
  evidence_chain: "evidence_chain",
};

const hostRole = (projection: Issue31HostProjection): Issue31RoleState => {
  if (projection.role.status !== "active") return "none";
  if (projection.role.kind === "owner") return "owner";
  if (projection.role.kind === "member") return "member";
  if (projection.role.kind === "verifier") return "verifier";
  return "read_only";
};

const hostStatus = (projection: Issue31HostProjection): Issue31SourceStatus => {
  if (projection.gap === "unavailable") return "unavailable";
  if (projection.gap === "missing" || projection.gap === "partial") return "gap";
  return "ready";
};

const hostReasonRef = (projection: Issue31HostProjection): string | null => {
  if (projection.gap === "unavailable") {
    return `reason.issue31.host_unavailable:${projection.capability}`;
  }
  if (projection.gap === "missing" || projection.gap === "partial") {
    return `reason.issue31.host_gap:${projection.capability}`;
  }
  return null;
};

export const issue31SourceSnapshotsFromHostAdjunct = (
  adjunct: Issue31HostAdjunct,
): ReadonlyArray<Issue31SourceSnapshot> =>
  adjunct.projections
    .filter((projection) => projection.capability !== "connection_identity")
    .map((projection) => {
      const capabilityId = hostCapabilityIds[projection.capability];
      const descriptor = ISSUE31_CAPABILITY_DESCRIPTORS.find((row) => row.id === capabilityId);
      if (descriptor === undefined) {
        throw new Error(`Issue 31 host capability ${projection.capability} is not mapped.`);
      }
      return decodeIssue31SourceSnapshot({
        capabilityId,
        authority: "omega_host_adjunct",
        sourceRef: descriptor.sourceRef,
        status: hostStatus(projection),
        freshness: projection.freshness === "current" ? "live" : projection.freshness,
        observedAt: new Date(projection.source.observedAtMs).toISOString(),
        recordRefs: projection.recordRefs,
        reasonRef: hostReasonRef(projection),
        role: hostRole(projection),
        roleStatus: projection.role.status,
        actionState: projection.commandState,
      });
    });

export const projectIssue31WorkroomReadModel = (
  input: Readonly<{
    projectedAt: string;
    sources?: ReadonlyArray<unknown>;
    hostAdjunct?: unknown;
  }>,
): Issue31WorkroomReadModel => {
  const projectedAt = Schema.decodeUnknownSync(IsoTime)(input.projectedAt);
  if (!Number.isFinite(Date.parse(projectedAt))) {
    throw new Error("Issue 31 workroom projection time is invalid.");
  }
  const hostAdjunct =
    input.hostAdjunct === undefined ? undefined : decodeIssue31HostAdjunct(input.hostAdjunct);
  const decoded = [
    ...(input.sources ?? []).map(decodeIssue31SourceSnapshot),
    ...(hostAdjunct === undefined ? [] : issue31SourceSnapshotsFromHostAdjunct(hostAdjunct)),
  ];
  const byCapability = new Map<Issue31CapabilityId, Issue31SourceSnapshot>();
  for (const source of decoded) {
    if (byCapability.has(source.capabilityId)) {
      throw new Error(`Issue 31 source ${source.capabilityId} appears more than once.`);
    }
    byCapability.set(source.capabilityId, source);
  }
  const rows = ISSUE31_CAPABILITY_DESCRIPTORS.map((descriptor) => {
    const hostProjection = hostAdjunct?.projections.find(
      (projection) => hostCapabilityIds[projection.capability] === descriptor.id,
    );
    return {
      ...descriptor,
      source: byCapability.get(descriptor.id) ?? unavailableSource(descriptor),
      hostObservation:
        hostAdjunct === undefined || hostProjection === undefined
          ? null
          : {
              hostRef: hostAdjunct.hostRef,
              snapshotRef: hostAdjunct.snapshotRef,
              generatedAtMs: hostAdjunct.generatedAtMs,
              projection: hostProjection,
            },
    };
  });
  const count = (status: Issue31SourceStatus): number =>
    rows.filter((row) => row.source.status === status).length;
  return {
    schema: ISSUE31_WORKROOM_READ_MODEL_SCHEMA,
    projectedAt,
    rows,
    coverage: {
      total: rows.length,
      ready: count("ready"),
      unavailable: count("unavailable"),
      gaps: count("gap"),
      pending: rows.filter((row) => row.source.actionState.kind === "pending").length,
      refused: rows.filter((row) => row.source.actionState.kind === "refused").length,
      terminal: rows.filter((row) => row.source.actionState.kind === "terminal").length,
    },
  };
};

export const emptyIssue31WorkroomReadModel = (
  projectedAt: string = "1970-01-01T00:00:00.000Z",
): Issue31WorkroomReadModel => projectIssue31WorkroomReadModel({ projectedAt });

export const issue31RowsForRoom = (
  model: Issue31WorkroomReadModel,
  room: Issue31WorkroomRoom,
): ReadonlyArray<Issue31CapabilityProjection> =>
  model.rows.filter((row) => row.room === "shared" || row.room === room);
