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
  readonly claimCommunityRoomRendezvous: (
    snapshot: SarahLiveKitRoomAuthoritySnapshot,
    now: string,
  ) => Promise<
    Readonly<{
      claimed: boolean;
      presenceLeaseRef: string;
    }>
  >;
  readonly readActiveCommunityRoomRendezvous: (input: {
    readonly communityRef: string;
    readonly channelRef: string;
    readonly now: string;
  }) => Promise<
    | Readonly<{
        presenceLeaseRef: string;
        membershipRevision: string;
        roomRef: string;
        roomEpoch: number;
        sessionRef: string;
        generation: number;
      }>
    | undefined
  >;
  readonly retireCommunityRoomRendezvous: (input: {
    readonly presenceLeaseRef: string;
    readonly now: string;
  }) => Promise<boolean>;
  readonly listActiveCommunityRoomParticipants: (input: {
    readonly presenceLeaseRef: string;
    readonly now: string;
  }) => Promise<
    ReadonlyArray<
      Readonly<{
        ownerUserId: string;
        userRefDigest: string;
        memberPubkey: string;
        participantRef: string;
        membershipRevision: string;
        roomRef: string;
        roomEpoch: number;
      }>
    >
  >;
  readonly readCommunityRoomBinding: (input: {
    readonly presenceLeaseRef: string;
    readonly ownerUserId: string;
    readonly now: string;
  }) => Promise<
    | Readonly<{
        ownerUserId: string;
        sessionRef: string;
        generation: number;
        admissionDigest: string;
        communityRef: string;
        channelRef: string;
        membershipRevision: string;
        roomRef: string;
        roomEpoch: number;
        participantRef: string;
        sarahParticipantRef: string;
        dispatchRef: string;
        participantGrantDigest: string;
        joinExpiresAt: string;
        sessionExpiresAt: string;
      }>
    | undefined
  >;
  readonly create: (
    snapshot: SarahLiveKitRoomAuthoritySnapshot,
    now: string,
  ) => Promise<SarahLiveKitRoomAuthoritySnapshot>;
  readonly read: (
    presenceLeaseRef: string,
  ) => Promise<SarahLiveKitRoomAuthoritySnapshot | undefined>;
  readonly readParticipantBinding: (input: {
    readonly presenceLeaseRef: string;
    readonly ownerUserId?: string;
    readonly userRefDigest?: string;
    readonly now: string;
  }) => Promise<
    | Readonly<{
        ownerUserId: string;
        userRefDigest: string;
        memberPubkey: string;
        participantRef: string;
        communityRef: string;
        channelRef: string;
        membershipRevision: string;
        roomRef: string;
        roomEpoch: number;
      }>
    | undefined
  >;
  readonly bindParticipant: (input: {
    readonly presenceLeaseRef: string;
    readonly ownerUserId: string;
    readonly userRefDigest: string;
    readonly memberPubkey: string;
    readonly participantRef: string;
    readonly membershipRevision: string;
    readonly roomRef: string;
    readonly roomEpoch: number;
    readonly participantGrantDigest: string;
    readonly joinExpiresAt: string;
    readonly now: string;
  }) => Promise<void>;
  readonly removeParticipant: (input: {
    readonly presenceLeaseRef: string;
    readonly userRefDigest: string;
    readonly now: string;
  }) => Promise<boolean>;
  readonly compareAndSwap: (input: {
    readonly presenceLeaseRef: string;
    readonly expectedRevision: number;
    readonly snapshot: SarahLiveKitRoomAuthoritySnapshot;
    readonly now: string;
  }) => Promise<SarahLiveKitRoomAuthoritySnapshot>;
}

export class PostgresSarahLiveKitRoomAuthorityStore implements SarahLiveKitRoomAuthorityStore {
  constructor(private readonly sql: SyncSql) {}

  async claimCommunityRoomRendezvous(
    value: SarahLiveKitRoomAuthoritySnapshot,
    now: string,
  ): Promise<Readonly<{ claimed: boolean; presenceLeaseRef: string }>> {
    const snapshot = decodeSnapshot(value);
    if (
      !snapshot.presenceActive ||
      snapshot.presence.expiresAtMs <= Date.parse(now) ||
      !Number.isFinite(Date.parse(now))
    ) {
      throw new SarahLiveKitRoomAuthorityStoreError({ reason: "invalid" });
    }
    try {
      return await this.sql.begin(async (transaction) => {
        await transaction`
          UPDATE sarah_livekit_community_room_rendezvous
          SET state='retired',retired_at=${now},updated_at=${now}
          WHERE community_ref=${snapshot.presence.communityRef}
            AND channel_ref=${snapshot.presence.channelRef}
            AND state='active'
            AND expires_at<=${now}`;
        const inserted: Array<{ presence_lease_ref: string }> = await transaction`
          INSERT INTO sarah_livekit_community_room_rendezvous
            (presence_lease_ref,community_ref,channel_ref,membership_revision,
             room_ref,room_epoch,session_ref,generation,expires_at,state,
             created_at,updated_at)
          VALUES (${snapshot.presence.leaseRef},${snapshot.presence.communityRef},
            ${snapshot.presence.channelRef},${snapshot.presence.membershipRevision},
            ${snapshot.presence.roomRef},${snapshot.presence.roomEpoch},
            ${snapshot.presence.sessionRef},${snapshot.presence.generation},
            ${new Date(snapshot.presence.expiresAtMs).toISOString()},'active',${now},${now})
          ON CONFLICT (community_ref,channel_ref) WHERE state='active' DO NOTHING
          RETURNING presence_lease_ref`;
        if (inserted[0] !== undefined) {
          return {
            claimed: true,
            presenceLeaseRef: inserted[0].presence_lease_ref,
          };
        }
        const active: Array<{ presence_lease_ref: string }> = await transaction`
          SELECT presence_lease_ref
          FROM sarah_livekit_community_room_rendezvous
          WHERE community_ref=${snapshot.presence.communityRef}
            AND channel_ref=${snapshot.presence.channelRef}
            AND state='active'
            AND expires_at>${now}
          FOR UPDATE`;
        const winner = active[0];
        if (winner === undefined) {
          throw new SarahLiveKitRoomAuthorityStoreError({ reason: "conflict" });
        }
        return {
          claimed: winner.presence_lease_ref === snapshot.presence.leaseRef,
          presenceLeaseRef: winner.presence_lease_ref,
        };
      });
    } catch (error) {
      if (error instanceof SarahLiveKitRoomAuthorityStoreError) throw error;
      throw new SarahLiveKitRoomAuthorityStoreError({ reason: "unavailable" });
    }
  }

  async readActiveCommunityRoomRendezvous(input: {
    readonly communityRef: string;
    readonly channelRef: string;
    readonly now: string;
  }): Promise<
    | Readonly<{
        presenceLeaseRef: string;
        membershipRevision: string;
        roomRef: string;
        roomEpoch: number;
        sessionRef: string;
        generation: number;
      }>
    | undefined
  > {
    if (
      input.communityRef.trim() === "" ||
      input.channelRef.trim() === "" ||
      !Number.isFinite(Date.parse(input.now))
    ) {
      throw new SarahLiveKitRoomAuthorityStoreError({ reason: "invalid" });
    }
    try {
      const rows: Array<{
        presence_lease_ref: string;
        membership_revision: string;
        room_ref: string;
        room_epoch: string | number;
        session_ref: string;
        generation: string | number;
      }> = await this.sql`
        SELECT rendezvous.presence_lease_ref,rendezvous.membership_revision,
          rendezvous.room_ref,rendezvous.room_epoch,rendezvous.session_ref,
          rendezvous.generation
        FROM sarah_livekit_community_room_rendezvous AS rendezvous
        INNER JOIN sarah_livekit_room_authorities AS authority
          ON authority.presence_lease_ref=rendezvous.presence_lease_ref
        INNER JOIN sarah_livekit_room_bindings AS binding
          ON binding.sarah_presence_lease_ref=rendezvous.presence_lease_ref
        INNER JOIN sarah_realtime_voice_sessions AS session
          ON session.session_ref=rendezvous.session_ref
        WHERE rendezvous.community_ref=${input.communityRef}
          AND rendezvous.channel_ref=${input.channelRef}
          AND rendezvous.state='active'
          AND rendezvous.expires_at>${input.now}
          AND binding.state='active'
          AND binding.worker_stop_reason IS NULL
          AND binding.worker_closed_at IS NULL
          AND session.state='connected'
          AND session.session_expires_at>${input.now}
          AND authority.community_ref=rendezvous.community_ref
          AND authority.channel_ref=rendezvous.channel_ref
          AND authority.membership_revision=rendezvous.membership_revision
          AND authority.room_ref=rendezvous.room_ref
          AND authority.room_epoch=rendezvous.room_epoch
        LIMIT 1`;
      const row = rows[0];
      if (row === undefined) return undefined;
      const roomEpoch = Number(row.room_epoch);
      const generation = Number(row.generation);
      if (
        !Number.isSafeInteger(roomEpoch) ||
        roomEpoch < 1 ||
        !Number.isSafeInteger(generation) ||
        generation < 1
      ) {
        throw new SarahLiveKitRoomAuthorityStoreError({ reason: "unavailable" });
      }
      return {
        presenceLeaseRef: row.presence_lease_ref,
        membershipRevision: row.membership_revision,
        roomRef: row.room_ref,
        roomEpoch,
        sessionRef: row.session_ref,
        generation,
      };
    } catch (error) {
      if (error instanceof SarahLiveKitRoomAuthorityStoreError) throw error;
      throw new SarahLiveKitRoomAuthorityStoreError({ reason: "unavailable" });
    }
  }

  async retireCommunityRoomRendezvous(input: {
    readonly presenceLeaseRef: string;
    readonly now: string;
  }): Promise<boolean> {
    if (input.presenceLeaseRef.trim() === "" || !Number.isFinite(Date.parse(input.now))) {
      throw new SarahLiveKitRoomAuthorityStoreError({ reason: "invalid" });
    }
    try {
      const rows: Array<{ presence_lease_ref: string }> = await this.sql`
        UPDATE sarah_livekit_community_room_rendezvous
        SET state='retired',retired_at=${input.now},updated_at=${input.now}
        WHERE presence_lease_ref=${input.presenceLeaseRef}
          AND state='active'
        RETURNING presence_lease_ref`;
      return rows[0]?.presence_lease_ref === input.presenceLeaseRef;
    } catch {
      throw new SarahLiveKitRoomAuthorityStoreError({ reason: "unavailable" });
    }
  }

  async listActiveCommunityRoomParticipants(input: {
    readonly presenceLeaseRef: string;
    readonly now: string;
  }): Promise<
    ReadonlyArray<
      Readonly<{
        ownerUserId: string;
        userRefDigest: string;
        memberPubkey: string;
        participantRef: string;
        membershipRevision: string;
        roomRef: string;
        roomEpoch: number;
      }>
    >
  > {
    if (input.presenceLeaseRef.trim() === "" || !Number.isFinite(Date.parse(input.now))) {
      throw new SarahLiveKitRoomAuthorityStoreError({ reason: "invalid" });
    }
    try {
      const rows: Array<{
        owner_user_id: string;
        user_ref_digest: string;
        member_pubkey: string;
        participant_ref: string;
        membership_revision: string;
        room_ref: string;
        room_epoch: string | number;
      }> = await this.sql`
        SELECT owner_user_id,user_ref_digest,member_pubkey,participant_ref,
          membership_revision,room_ref,room_epoch
        FROM sarah_livekit_room_members
        WHERE presence_lease_ref=${input.presenceLeaseRef}
          AND state='active'
          AND join_expires_at>${input.now}
        ORDER BY user_ref_digest`;
      return rows.map((row) => {
        const roomEpoch = Number(row.room_epoch);
        if (!Number.isSafeInteger(roomEpoch) || roomEpoch < 1) {
          throw new SarahLiveKitRoomAuthorityStoreError({ reason: "unavailable" });
        }
        return {
          ownerUserId: row.owner_user_id,
          userRefDigest: row.user_ref_digest,
          memberPubkey: row.member_pubkey,
          participantRef: row.participant_ref,
          membershipRevision: row.membership_revision,
          roomRef: row.room_ref,
          roomEpoch,
        };
      });
    } catch (error) {
      if (error instanceof SarahLiveKitRoomAuthorityStoreError) throw error;
      throw new SarahLiveKitRoomAuthorityStoreError({ reason: "unavailable" });
    }
  }

  async readCommunityRoomBinding(input: {
    readonly presenceLeaseRef: string;
    readonly ownerUserId: string;
    readonly now: string;
  }): Promise<
    | Readonly<{
        ownerUserId: string;
        sessionRef: string;
        generation: number;
        admissionDigest: string;
        communityRef: string;
        channelRef: string;
        membershipRevision: string;
        roomRef: string;
        roomEpoch: number;
        participantRef: string;
        sarahParticipantRef: string;
        dispatchRef: string;
        participantGrantDigest: string;
        joinExpiresAt: string;
        sessionExpiresAt: string;
      }>
    | undefined
  > {
    if (!Number.isFinite(Date.parse(input.now))) {
      throw new SarahLiveKitRoomAuthorityStoreError({ reason: "invalid" });
    }
    try {
      const rows: Array<{
        owner_user_id: string;
        session_ref: string;
        generation: string | number;
        admission_digest: string;
        community_ref: string;
        channel_ref: string;
        membership_revision: string;
        room_ref: string;
        room_epoch: string | number;
        participant_ref: string;
        sarah_participant_ref: string;
        dispatch_ref: string;
        participant_grant_digest: string;
        join_expires_at: string;
        session_expires_at: string;
      }> = await this.sql`
        SELECT binding.owner_user_id,binding.session_ref,binding.generation,
          binding.admission_digest,binding.community_ref,binding.channel_ref,
          binding.membership_revision,binding.room_ref,binding.room_epoch,
          binding.participant_ref,binding.sarah_participant_ref,binding.dispatch_ref,
          binding.participant_grant_digest,binding.join_expires_at,
          session.session_expires_at
        FROM sarah_livekit_room_bindings AS binding
        INNER JOIN sarah_realtime_voice_sessions AS session
          ON session.session_ref=binding.session_ref
        WHERE binding.sarah_presence_lease_ref=${input.presenceLeaseRef}
          AND binding.owner_user_id=${input.ownerUserId}
          AND binding.room_context_kind='community'
          AND binding.state IN ('prepared', 'active')
          AND binding.worker_stop_reason IS NULL
          AND binding.worker_closed_at IS NULL
          AND session.state IN ('reserved', 'connected')
          AND session.session_expires_at>${input.now}
        LIMIT 1`;
      const row = rows[0];
      if (row === undefined) return undefined;
      const generation = Number(row.generation);
      const roomEpoch = Number(row.room_epoch);
      if (
        !Number.isSafeInteger(generation) ||
        generation < 1 ||
        !Number.isSafeInteger(roomEpoch) ||
        roomEpoch < 1
      ) {
        throw new SarahLiveKitRoomAuthorityStoreError({ reason: "unavailable" });
      }
      return {
        ownerUserId: row.owner_user_id,
        sessionRef: row.session_ref,
        generation,
        admissionDigest: row.admission_digest,
        communityRef: row.community_ref,
        channelRef: row.channel_ref,
        membershipRevision: row.membership_revision,
        roomRef: row.room_ref,
        roomEpoch,
        participantRef: row.participant_ref,
        sarahParticipantRef: row.sarah_participant_ref,
        dispatchRef: row.dispatch_ref,
        participantGrantDigest: row.participant_grant_digest,
        joinExpiresAt: row.join_expires_at,
        sessionExpiresAt: row.session_expires_at,
      };
    } catch (error) {
      if (error instanceof SarahLiveKitRoomAuthorityStoreError) throw error;
      throw new SarahLiveKitRoomAuthorityStoreError({ reason: "unavailable" });
    }
  }

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
    readonly ownerUserId?: string;
    readonly userRefDigest?: string;
    readonly now: string;
  }): Promise<
    | Readonly<{
        ownerUserId: string;
        userRefDigest: string;
        memberPubkey: string;
        participantRef: string;
        communityRef: string;
        channelRef: string;
        membershipRevision: string;
        roomRef: string;
        roomEpoch: number;
      }>
    | undefined
  > {
    if (
      !Number.isFinite(Date.parse(input.now)) ||
      (input.ownerUserId === undefined) === (input.userRefDigest === undefined)
    ) {
      throw new SarahLiveKitRoomAuthorityStoreError({ reason: "invalid" });
    }
    try {
      const rows: Array<{
        owner_user_id: string;
        user_ref_digest: string;
        member_pubkey: string;
        participant_ref: string;
        community_ref: string;
        channel_ref: string;
        membership_revision: string;
        room_ref: string;
        room_epoch: string | number;
      }> = await this.sql`
        SELECT member.owner_user_id,member.user_ref_digest,member.member_pubkey,
          member.participant_ref,binding.community_ref,binding.channel_ref,
          member.membership_revision,member.room_ref,member.room_epoch
        FROM sarah_livekit_room_authorities AS authority
        INNER JOIN sarah_livekit_room_bindings AS binding
          ON binding.sarah_presence_lease_ref=authority.presence_lease_ref
        INNER JOIN sarah_livekit_room_members AS member
          ON member.presence_lease_ref=authority.presence_lease_ref
        INNER JOIN sarah_realtime_voice_sessions AS session
          ON session.session_ref=binding.session_ref
        WHERE authority.presence_lease_ref=${input.presenceLeaseRef}
          AND (${input.ownerUserId ?? null}::text IS NULL
            OR member.owner_user_id=${input.ownerUserId ?? null})
          AND (${input.userRefDigest ?? null}::text IS NULL
            OR member.user_ref_digest=${input.userRefDigest ?? null})
          AND binding.room_context_kind='community'
          AND binding.state='active'
          AND member.state='active'
          AND member.join_expires_at>${input.now}
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
          AND member.membership_revision=authority.membership_revision
          AND member.room_ref=authority.room_ref
          AND member.room_epoch=authority.room_epoch
        LIMIT 1`;
      const row = rows[0];
      if (row === undefined) return undefined;
      const roomEpoch = Number(row.room_epoch);
      if (!Number.isSafeInteger(roomEpoch) || roomEpoch < 1) {
        throw new SarahLiveKitRoomAuthorityStoreError({ reason: "unavailable" });
      }
      return {
        ownerUserId: row.owner_user_id,
        userRefDigest: row.user_ref_digest,
        memberPubkey: row.member_pubkey,
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

  async bindParticipant(input: {
    readonly presenceLeaseRef: string;
    readonly ownerUserId: string;
    readonly userRefDigest: string;
    readonly memberPubkey: string;
    readonly participantRef: string;
    readonly membershipRevision: string;
    readonly roomRef: string;
    readonly roomEpoch: number;
    readonly participantGrantDigest: string;
    readonly joinExpiresAt: string;
    readonly now: string;
  }): Promise<void> {
    if (
      !/^[a-f0-9]{64}$/u.test(input.userRefDigest) ||
      !/^[a-f0-9]{64}$/u.test(input.memberPubkey) ||
      !/^[a-f0-9]{64}$/u.test(input.membershipRevision) ||
      !/^[a-f0-9]{64}$/u.test(input.participantGrantDigest) ||
      !Number.isSafeInteger(input.roomEpoch) ||
      input.roomEpoch < 1 ||
      Date.parse(input.joinExpiresAt) <= Date.parse(input.now)
    ) {
      throw new SarahLiveKitRoomAuthorityStoreError({ reason: "invalid" });
    }
    try {
      await this.sql.begin(async (transaction) => {
        const authorities: AuthorityRow[] = await transaction`
          SELECT presence_lease_ref,revision,snapshot_json
          FROM sarah_livekit_room_authorities
          WHERE presence_lease_ref=${input.presenceLeaseRef}
          FOR UPDATE`;
        const authority = authorities[0];
        if (authority === undefined) {
          throw new SarahLiveKitRoomAuthorityStoreError({ reason: "not_found" });
        }
        const snapshot = decodeStoredSnapshot(authority);
        if (
          !snapshot.presenceActive ||
          snapshot.presence.membershipRevision !== input.membershipRevision ||
          snapshot.presence.roomRef !== input.roomRef ||
          snapshot.presence.roomEpoch !== input.roomEpoch
        ) {
          throw new SarahLiveKitRoomAuthorityStoreError({ reason: "authority_mismatch" });
        }
        await transaction`
          INSERT INTO sarah_livekit_room_members
            (presence_lease_ref,owner_user_id,user_ref_digest,member_pubkey,
             participant_ref,membership_revision,room_ref,room_epoch,
             participant_grant_digest,join_expires_at,state,created_at,updated_at)
          VALUES (${input.presenceLeaseRef},${input.ownerUserId},${input.userRefDigest},
            ${input.memberPubkey},${input.participantRef},${input.membershipRevision},
            ${input.roomRef},${input.roomEpoch},${input.participantGrantDigest},
            ${input.joinExpiresAt},'active',${input.now},${input.now})
          ON CONFLICT (presence_lease_ref,owner_user_id) DO UPDATE SET
            user_ref_digest=EXCLUDED.user_ref_digest,
            member_pubkey=EXCLUDED.member_pubkey,
            participant_ref=EXCLUDED.participant_ref,
            membership_revision=EXCLUDED.membership_revision,
            room_ref=EXCLUDED.room_ref,
            room_epoch=EXCLUDED.room_epoch,
            participant_grant_digest=EXCLUDED.participant_grant_digest,
            join_expires_at=EXCLUDED.join_expires_at,
            state='active',
            removed_at=NULL,
            updated_at=EXCLUDED.updated_at`;
      });
    } catch (error) {
      if (error instanceof SarahLiveKitRoomAuthorityStoreError) throw error;
      throw new SarahLiveKitRoomAuthorityStoreError({ reason: "unavailable" });
    }
  }

  async removeParticipant(input: {
    readonly presenceLeaseRef: string;
    readonly userRefDigest: string;
    readonly now: string;
  }): Promise<boolean> {
    if (!/^[a-f0-9]{64}$/u.test(input.userRefDigest) || !Number.isFinite(Date.parse(input.now))) {
      throw new SarahLiveKitRoomAuthorityStoreError({ reason: "invalid" });
    }
    try {
      const rows: Array<{ user_ref_digest: string }> = await this.sql`
        UPDATE sarah_livekit_room_members
        SET state='removed',removed_at=${input.now},updated_at=${input.now}
        WHERE presence_lease_ref=${input.presenceLeaseRef}
          AND user_ref_digest=${input.userRefDigest}
          AND state='active'
        RETURNING user_ref_digest`;
      return rows[0]?.user_ref_digest === input.userRefDigest;
    } catch {
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
