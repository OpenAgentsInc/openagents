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
