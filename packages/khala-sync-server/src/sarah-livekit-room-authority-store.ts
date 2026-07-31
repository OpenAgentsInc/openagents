import {
  decodeSarahLiveKitRoomAuthoritySnapshot,
  type SarahLiveKitRoomAuthoritySnapshot,
} from "@openagentsinc/audio-contract";
import { Schema } from "effect";

import type { SyncSql, SyncTransactionSql } from "./sql.js";

export class SarahLiveKitRoomAuthorityStoreError extends Schema.TaggedErrorClass<SarahLiveKitRoomAuthorityStoreError>()(
  "SarahLiveKitRoomAuthorityStoreError",
  {
    reason: Schema.Literals([
      "invalid",
      "not_found",
      "conflict",
      "stale_revision",
      "authority_mismatch",
      "unavailable",
    ]),
  },
) {}

type AuthorityRow = {
  presence_lease_ref: string;
  revision: string | number;
  snapshot_json: unknown;
};

type BindingRow = {
  session_ref: string;
  generation: string | number;
  community_ref: string | null;
  channel_ref: string | null;
  membership_revision: string | null;
  room_ref: string;
  room_epoch: string | number;
  sarah_participant_ref: string;
  dispatch_ref: string;
  sarah_presence_lease_ref: string;
  admission_digest: string;
  room_context_kind: "private" | "community";
  state: string;
};

const decodeSnapshot = (value: unknown): SarahLiveKitRoomAuthoritySnapshot => {
  try {
    const parsed = typeof value === "string" ? JSON.parse(value) : value;
    const snapshot = decodeSarahLiveKitRoomAuthoritySnapshot(parsed);
    const floorPresenceRef =
      snapshot.floor.state === "held"
        ? snapshot.floor.lease.presenceLeaseRef
        : snapshot.floor.presenceLeaseRef;
    if (
      floorPresenceRef !== snapshot.presence.leaseRef ||
      (snapshot.floor.state === "held" &&
        (snapshot.floor.lease.sessionRef !== snapshot.presence.sessionRef ||
          snapshot.floor.lease.generation !== snapshot.presence.generation ||
          snapshot.floor.lease.roomRef !== snapshot.presence.roomRef ||
          snapshot.floor.lease.roomEpoch !== snapshot.presence.roomEpoch ||
          snapshot.floor.lease.membershipRevision !== snapshot.presence.membershipRevision))
    ) {
      throw new Error("snapshot authority is internally inconsistent");
    }
    return snapshot;
  } catch {
    throw new SarahLiveKitRoomAuthorityStoreError({ reason: "invalid" });
  }
};

const decodeStoredSnapshot = (row: AuthorityRow): SarahLiveKitRoomAuthoritySnapshot => {
  try {
    const snapshot = decodeSnapshot(row.snapshot_json);
    if (
      snapshot.presence.leaseRef !== row.presence_lease_ref ||
      snapshot.revision !== Number(row.revision)
    ) {
      throw new Error("stored snapshot does not match its authority row");
    }
    return snapshot;
  } catch (error) {
    if (error instanceof SarahLiveKitRoomAuthorityStoreError && error.reason === "unavailable") {
      throw error;
    }
    throw new SarahLiveKitRoomAuthorityStoreError({ reason: "unavailable" });
  }
};

const bindingMatches = (
  binding: BindingRow,
  snapshot: SarahLiveKitRoomAuthoritySnapshot,
): boolean => {
  const presence = snapshot.presence;
  return (
    binding.room_context_kind === "community" &&
    binding.state === "active" &&
    binding.session_ref === presence.sessionRef &&
    Number(binding.generation) === presence.generation &&
    binding.community_ref === presence.communityRef &&
    binding.channel_ref === presence.channelRef &&
    binding.membership_revision === presence.membershipRevision &&
    binding.room_ref === presence.roomRef &&
    Number(binding.room_epoch) === presence.roomEpoch &&
    binding.sarah_participant_ref === presence.sarahParticipantRef &&
    binding.dispatch_ref === presence.dispatchRef &&
    binding.sarah_presence_lease_ref === presence.leaseRef &&
    binding.admission_digest === presence.admissionDigest
  );
};

const samePresence = (
  left: SarahLiveKitRoomAuthoritySnapshot,
  right: SarahLiveKitRoomAuthoritySnapshot,
): boolean => JSON.stringify(left.presence) === JSON.stringify(right.presence);

export interface SarahLiveKitRoomAuthorityStore {
  readonly create: (
    snapshot: SarahLiveKitRoomAuthoritySnapshot,
    now: string,
  ) => Promise<SarahLiveKitRoomAuthoritySnapshot>;
  readonly read: (
    presenceLeaseRef: string,
  ) => Promise<SarahLiveKitRoomAuthoritySnapshot | undefined>;
  readonly readParticipantBinding: (input: {
    readonly presenceLeaseRef: string;
    readonly ownerUserId: string;
    readonly now: string;
  }) => Promise<
    | Readonly<{
        ownerUserId: string;
        participantRef: string;
        communityRef: string;
        channelRef: string;
        membershipRevision: string;
        roomRef: string;
        roomEpoch: number;
      }>
    | undefined
  >;
  readonly compareAndSwap: (input: {
    readonly presenceLeaseRef: string;
    readonly expectedRevision: number;
    readonly snapshot: SarahLiveKitRoomAuthoritySnapshot;
    readonly now: string;
  }) => Promise<SarahLiveKitRoomAuthoritySnapshot>;
}

export class PostgresSarahLiveKitRoomAuthorityStore implements SarahLiveKitRoomAuthorityStore {
  constructor(private readonly sql: SyncSql) {}

  async create(
    value: SarahLiveKitRoomAuthoritySnapshot,
    now: string,
  ): Promise<SarahLiveKitRoomAuthoritySnapshot> {
    const snapshot = decodeSnapshot(value);
    if (snapshot.revision !== 1 || !snapshot.presenceActive || !Number.isFinite(Date.parse(now))) {
      throw new SarahLiveKitRoomAuthorityStoreError({ reason: "invalid" });
    }
    return this.sql.begin(async (transaction) => {
      const bindings: BindingRow[] = await transaction`
        SELECT session_ref,generation,community_ref,channel_ref,membership_revision,
          room_ref,room_epoch,sarah_participant_ref,dispatch_ref,sarah_presence_lease_ref,
          admission_digest,room_context_kind,state
        FROM sarah_livekit_room_bindings
        WHERE sarah_presence_lease_ref = ${snapshot.presence.leaseRef}
        FOR UPDATE`;
      const binding = bindings[0];
      if (binding === undefined) {
        throw new SarahLiveKitRoomAuthorityStoreError({ reason: "not_found" });
      }
      if (!bindingMatches(binding, snapshot)) {
        throw new SarahLiveKitRoomAuthorityStoreError({ reason: "authority_mismatch" });
      }
      const existing = await this.readWith(transaction, snapshot.presence.leaseRef);
      if (existing !== undefined) {
        throw new SarahLiveKitRoomAuthorityStoreError({ reason: "conflict" });
      }
      const snapshotJson = JSON.stringify(snapshot);
      const rows: AuthorityRow[] = await transaction`
        INSERT INTO sarah_livekit_room_authorities
          (presence_lease_ref,session_ref,generation,community_ref,channel_ref,
           membership_revision,room_ref,room_epoch,revision,snapshot_json,created_at,updated_at)
        VALUES (${snapshot.presence.leaseRef},${snapshot.presence.sessionRef},
          ${snapshot.presence.generation},${snapshot.presence.communityRef},
          ${snapshot.presence.channelRef},${snapshot.presence.membershipRevision},
          ${snapshot.presence.roomRef},${snapshot.presence.roomEpoch},${snapshot.revision},
          ${snapshotJson}::jsonb,${now},${now})
        RETURNING presence_lease_ref,revision,snapshot_json`;
      const row = rows[0];
      if (row === undefined) {
        throw new SarahLiveKitRoomAuthorityStoreError({ reason: "unavailable" });
      }
      return decodeStoredSnapshot(row);
    });
  }

  async read(presenceLeaseRef: string): Promise<SarahLiveKitRoomAuthoritySnapshot | undefined> {
    return this.readWith(this.sql, presenceLeaseRef);
  }

  async readParticipantBinding(input: {
    readonly presenceLeaseRef: string;
    readonly ownerUserId: string;
    readonly now: string;
  }): Promise<
    | Readonly<{
        ownerUserId: string;
        participantRef: string;
        communityRef: string;
        channelRef: string;
        membershipRevision: string;
        roomRef: string;
        roomEpoch: number;
      }>
    | undefined
  > {
    if (!Number.isFinite(Date.parse(input.now))) {
      throw new SarahLiveKitRoomAuthorityStoreError({ reason: "invalid" });
    }
    try {
      const rows: Array<{
        owner_user_id: string;
        participant_ref: string;
        community_ref: string;
        channel_ref: string;
        membership_revision: string;
        room_ref: string;
        room_epoch: string | number;
      }> = await this.sql`
        SELECT binding.owner_user_id,binding.participant_ref,binding.community_ref,
          binding.channel_ref,binding.membership_revision,binding.room_ref,binding.room_epoch
        FROM sarah_livekit_room_authorities AS authority
        INNER JOIN sarah_livekit_room_bindings AS binding
          ON binding.sarah_presence_lease_ref=authority.presence_lease_ref
        INNER JOIN sarah_realtime_voice_sessions AS session
          ON session.session_ref=binding.session_ref
        WHERE authority.presence_lease_ref=${input.presenceLeaseRef}
          AND binding.owner_user_id=${input.ownerUserId}
          AND binding.room_context_kind='community'
          AND binding.state='active'
          AND binding.owner_joined_at IS NOT NULL
          AND binding.sarah_joined_at IS NOT NULL
          AND binding.worker_stop_reason IS NULL
          AND binding.worker_closed_at IS NULL
          AND session.state='connected'
          AND session.session_expires_at>${input.now}
          AND binding.community_ref=authority.community_ref
          AND binding.channel_ref=authority.channel_ref
          AND binding.membership_revision=authority.membership_revision
          AND binding.room_ref=authority.room_ref
          AND binding.room_epoch=authority.room_epoch
        LIMIT 1`;
      const row = rows[0];
      if (row === undefined) return undefined;
      const roomEpoch = Number(row.room_epoch);
      if (!Number.isSafeInteger(roomEpoch) || roomEpoch < 1) {
        throw new SarahLiveKitRoomAuthorityStoreError({ reason: "unavailable" });
      }
      return {
        ownerUserId: row.owner_user_id,
        participantRef: row.participant_ref,
        communityRef: row.community_ref,
        channelRef: row.channel_ref,
        membershipRevision: row.membership_revision,
        roomRef: row.room_ref,
        roomEpoch,
      };
    } catch (error) {
      if (error instanceof SarahLiveKitRoomAuthorityStoreError) throw error;
      throw new SarahLiveKitRoomAuthorityStoreError({ reason: "unavailable" });
    }
  }

  async compareAndSwap(input: {
    readonly presenceLeaseRef: string;
    readonly expectedRevision: number;
    readonly snapshot: SarahLiveKitRoomAuthoritySnapshot;
    readonly now: string;
  }): Promise<SarahLiveKitRoomAuthoritySnapshot> {
    const snapshot = decodeSnapshot(input.snapshot);
    if (
      snapshot.presence.leaseRef !== input.presenceLeaseRef ||
      !Number.isSafeInteger(input.expectedRevision) ||
      input.expectedRevision < 1 ||
      snapshot.revision !== input.expectedRevision + 1 ||
      !Number.isFinite(Date.parse(input.now))
    ) {
      throw new SarahLiveKitRoomAuthorityStoreError({ reason: "invalid" });
    }
    return this.sql.begin(async (transaction) => {
      const bindings: BindingRow[] = await transaction`
        SELECT session_ref,generation,community_ref,channel_ref,membership_revision,
          room_ref,room_epoch,sarah_participant_ref,dispatch_ref,sarah_presence_lease_ref,
          admission_digest,room_context_kind,state
        FROM sarah_livekit_room_bindings
        WHERE sarah_presence_lease_ref = ${input.presenceLeaseRef}
        FOR UPDATE`;
      const binding = bindings[0];
      if (binding === undefined) {
        throw new SarahLiveKitRoomAuthorityStoreError({ reason: "not_found" });
      }
      if (!bindingMatches(binding, snapshot)) {
        throw new SarahLiveKitRoomAuthorityStoreError({ reason: "authority_mismatch" });
      }
      const rows: AuthorityRow[] = await transaction`
        SELECT presence_lease_ref,revision,snapshot_json
        FROM sarah_livekit_room_authorities
        WHERE presence_lease_ref = ${input.presenceLeaseRef}
        FOR UPDATE`;
      const row = rows[0];
      if (row === undefined) {
        throw new SarahLiveKitRoomAuthorityStoreError({ reason: "not_found" });
      }
      const current = decodeStoredSnapshot(row);
      if (current.revision !== input.expectedRevision) {
        throw new SarahLiveKitRoomAuthorityStoreError({ reason: "stale_revision" });
      }
      if (!samePresence(current, snapshot)) {
        throw new SarahLiveKitRoomAuthorityStoreError({ reason: "authority_mismatch" });
      }
      const snapshotJson = JSON.stringify(snapshot);
      const updated: AuthorityRow[] = await transaction`
        UPDATE sarah_livekit_room_authorities
        SET revision=${snapshot.revision},snapshot_json=${snapshotJson}::jsonb,updated_at=${input.now}
        WHERE presence_lease_ref=${input.presenceLeaseRef}
          AND revision=${input.expectedRevision}
        RETURNING presence_lease_ref,revision,snapshot_json`;
      const updatedRow = updated[0];
      if (updatedRow === undefined) {
        throw new SarahLiveKitRoomAuthorityStoreError({ reason: "stale_revision" });
      }
      return decodeStoredSnapshot(updatedRow);
    });
  }

  private async readWith(
    sql: SyncTransactionSql,
    presenceLeaseRef: string,
  ): Promise<SarahLiveKitRoomAuthoritySnapshot | undefined> {
    const rows: AuthorityRow[] = await sql`
      SELECT presence_lease_ref,revision,snapshot_json
      FROM sarah_livekit_room_authorities
      WHERE presence_lease_ref=${presenceLeaseRef}
      LIMIT 1`;
    return rows[0] === undefined ? undefined : decodeStoredSnapshot(rows[0]);
  }
}
