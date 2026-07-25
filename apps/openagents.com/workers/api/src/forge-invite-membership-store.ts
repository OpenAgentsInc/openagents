import { Schema as S } from 'effect'

import {
  ForgeActorBinding,
  ForgeBurnedKeyFact,
  ForgeInviteBinding,
  ForgeInvitePolicyError,
  type ForgeInvitePolicyStore,
  ForgeMembershipReconciliationState,
  verifyForgeOwnerAttestation,
} from './forge-invite-policy'

type TeamInviteRole = 'admin' | 'member' | 'viewer'

export const forgeRoleRefsForTeamInvite = (
  role: TeamInviteRole,
): ReadonlyArray<string> => [`forge:${role}`]

export type BindInvitedHumanInput = Readonly<{
  acceptedAt: string
  accountRef: string
  bindingEventCreatedAt: string
  bindingEventId: string
  bindingRef: string
  displayName: string
  expiresAt: string
  inviteBindingRef: string
  inviteDigest: string
  inviteRef: string
  invitedSubjectRef: string
  inviterBindingRef: string
  issuedAt: string
  nostrPubkey: string
  provenanceSourceRefs: ReadonlyArray<string>
  roleRefs: ReadonlyArray<string>
  teamRef: string
  tenantRef: string
}>

export type AttachInvitedAgentInput = Readonly<{
  accountRef: string
  bindingEventCreatedAt: string
  bindingEventId: string
  bindingRef: string
  displayName: string
  nostrPubkey: string
  ownerAuthTag: ReadonlyArray<string>
  ownerBindingRef: string
  sourceRefs: ReadonlyArray<string>
  tenantRef: string
  nowIso: string
}>

export type ForgeInviteMembershipStore = ForgeInvitePolicyStore &
  Readonly<{
    attachAgent: (input: AttachInvitedAgentInput) => Promise<ForgeActorBinding>
    bindHuman: (input: BindInvitedHumanInput) => Promise<ForgeActorBinding>
    isBurnedKey: (tenantRef: string, publicKey: string) => Promise<boolean>
    readActorBindingByAccount: (
      tenantRef: string,
      accountRef: string,
      actorKind: 'human' | 'agent',
    ) => Promise<ForgeActorBinding | undefined>
    readBurnedKeyFact: (
      tenantRef: string,
      publicKey: string,
    ) => Promise<ForgeBurnedKeyFact | undefined>
    readInviteBinding: (
      tenantRef: string,
      inviteRef: string,
    ) => Promise<ForgeInviteBinding | undefined>
    readReconciliationState: (
      tenantRef: string,
      bindingRef: string,
    ) => Promise<ForgeMembershipReconciliationState | undefined>
    reconcileMembership: (
      input: Readonly<{
        bindingRef: string
        nowIso: string
        observedPresent: boolean
        querySucceeded: boolean
        sourceMembershipGeneration: number
        sourceRefs: ReadonlyArray<string>
        teamRef: string
        tenantRef: string
      }>,
    ) => Promise<ForgeMembershipReconciliationState | undefined>
    tombstoneMember: (
      input: Readonly<{
        bindingRef: string
        burnReasonRef: string
        nowIso: string
        sourceRefs: ReadonlyArray<string>
        tenantRef: string
      }>,
    ) => Promise<ReadonlyArray<ForgeActorBinding>>
  }>

type ActorBindingRow = Readonly<{
  binding_ref: string
  tenant_ref: string
  account_ref: string
  actor_kind: 'human' | 'agent'
  display_name: string
  owner_binding_ref: string | null
  role_refs_json: string
  membership_state: 'active' | 'tombstoned'
  binding_generation: number
  created_at: string
  revoked_at: string | null
  nostr_pubkey: string | null
  nostr_binding_event_id: string | null
  nostr_binding_created_at: string | null
  nostr_binding_signature_valid: number
}>

type InviteBindingRow = Readonly<{
  invite_binding_ref: string
  tenant_ref: string
  team_ref: string
  invite_ref: string
  invite_digest: string
  invite_kind: 'team_workspace'
  inviter_binding_ref: string
  invited_subject_ref: string
  role_refs_json: string
  issued_at: string
  expires_at: string
  accepted_at: string | null
  accepted_binding_ref: string | null
  provenance_source_refs_json: string
}>

type BurnedKeyRow = Readonly<{
  burned_key_fact_ref: string
  tenant_ref: string
  key_kind: 'human' | 'agent'
  public_key: string
  binding_ref: string
  burn_reason_ref: string
  burned_at: string
  burn_sequence: number
  source_refs_json: string
}>

type ReconciliationRow = Readonly<{
  reconciliation_ref: string
  tenant_ref: string
  team_ref: string
  binding_ref: string
  source_membership_generation: number
  reconciliation_generation: number
  observed_present: number
  absence_first_observed_at: string | null
  absence_confirmed_at: string | null
  hysteresis_deadline: string | null
  state: 'present' | 'absence_pending' | 'absence_confirmed'
  reconciled_at: string
  source_refs_json: string
}>

const stringArray = (value: string): ReadonlyArray<string> =>
  S.decodeUnknownSync(S.Array(S.String))(JSON.parse(value))

const actorFromRow = (row: ActorBindingRow): ForgeActorBinding =>
  ForgeActorBinding.make({
    accountRef: row.account_ref,
    actorKind: row.actor_kind,
    bindingGeneration: row.binding_generation,
    bindingRef: row.binding_ref,
    createdAt: row.created_at,
    displayName: row.display_name,
    membershipState: row.membership_state,
    nostrBindingCreatedAt: row.nostr_binding_created_at,
    nostrBindingEventId: row.nostr_binding_event_id,
    nostrBindingSignatureValid: row.nostr_binding_signature_valid === 1,
    nostrPubkey: row.nostr_pubkey,
    ownerBindingRef: row.owner_binding_ref,
    revokedAt: row.revoked_at,
    roleRefs: [...stringArray(row.role_refs_json)],
    tenantRef: row.tenant_ref,
  })

const inviteFromRow = (row: InviteBindingRow): ForgeInviteBinding =>
  ForgeInviteBinding.make({
    acceptedAt: row.accepted_at,
    acceptedBindingRef: row.accepted_binding_ref,
    expiresAt: row.expires_at,
    inviteBindingRef: row.invite_binding_ref,
    inviteDigest: row.invite_digest,
    inviteKind: row.invite_kind,
    inviteRef: row.invite_ref,
    invitedSubjectRef: row.invited_subject_ref,
    inviterBindingRef: row.inviter_binding_ref,
    issuedAt: row.issued_at,
    provenanceSourceRefs: [...stringArray(row.provenance_source_refs_json)],
    roleRefs: [...stringArray(row.role_refs_json)],
    teamRef: row.team_ref,
    tenantRef: row.tenant_ref,
  })

const burnedFromRow = (row: BurnedKeyRow): ForgeBurnedKeyFact =>
  ForgeBurnedKeyFact.make({
    bindingRef: row.binding_ref,
    burnReasonRef: row.burn_reason_ref,
    burnedAt: row.burned_at,
    burnedKeyFactRef: row.burned_key_fact_ref,
    burnSequence: row.burn_sequence,
    keyKind: row.key_kind,
    publicKey: row.public_key,
    sourceRefs: [...stringArray(row.source_refs_json)],
    tenantRef: row.tenant_ref,
  })

const reconciliationFromRow = (
  row: ReconciliationRow,
): ForgeMembershipReconciliationState =>
  ForgeMembershipReconciliationState.make({
    absenceConfirmedAt: row.absence_confirmed_at,
    absenceFirstObservedAt: row.absence_first_observed_at,
    bindingRef: row.binding_ref,
    hysteresisDeadline: row.hysteresis_deadline,
    observedPresent: row.observed_present === 1,
    reconciledAt: row.reconciled_at,
    reconciliationGeneration: row.reconciliation_generation,
    reconciliationRef: row.reconciliation_ref,
    sourceMembershipGeneration: row.source_membership_generation,
    sourceRefs: [...stringArray(row.source_refs_json)],
    state: row.state,
    teamRef: row.team_ref,
    tenantRef: row.tenant_ref,
  })

const actorColumns = `
  binding_ref, tenant_ref, account_ref, actor_kind, display_name,
  owner_binding_ref, role_refs_json, membership_state, binding_generation,
  created_at, revoked_at, nostr_pubkey, nostr_binding_event_id,
  nostr_binding_created_at, nostr_binding_signature_valid
`

const readActorBy = async (
  db: D1Database,
  clause: string,
  values: ReadonlyArray<string>,
): Promise<ForgeActorBinding | undefined> => {
  const row = await db
    .prepare(
      `SELECT ${actorColumns} FROM forge_actor_bindings WHERE ${clause} LIMIT 1`,
    )
    .bind(...values)
    .first<ActorBindingRow>()
  return row === null ? undefined : actorFromRow(row)
}

const conflict = (reason: string): ForgeInvitePolicyError =>
  new ForgeInvitePolicyError({ code: 'binding_conflict', reason })

const burned = (): ForgeInvitePolicyError =>
  new ForgeInvitePolicyError({
    code: 'key_burned',
    reason: 'A burned Forge key cannot be admitted again.',
  })

const json = (values: ReadonlyArray<string>): string =>
  JSON.stringify([...new Set(values)])

export const makeD1ForgeInviteMembershipStore = (
  db: D1Database,
): ForgeInviteMembershipStore => {
  const readActorBindingByRef = (
    tenantRef: string,
    bindingRef: string,
  ): Promise<ForgeActorBinding | undefined> =>
    readActorBy(db, 'tenant_ref = ? AND binding_ref = ?', [
      tenantRef,
      bindingRef,
    ])

  const readActorBindingByNostrPubkey = (
    tenantRef: string,
    nostrPubkey: string,
  ): Promise<ForgeActorBinding | undefined> =>
    readActorBy(db, 'tenant_ref = ? AND nostr_pubkey = ?', [
      tenantRef,
      nostrPubkey,
    ])

  const readActorBindingByAccount = (
    tenantRef: string,
    accountRef: string,
    actorKind: 'human' | 'agent',
  ): Promise<ForgeActorBinding | undefined> =>
    readActorBy(db, 'tenant_ref = ? AND account_ref = ? AND actor_kind = ?', [
      tenantRef,
      accountRef,
      actorKind,
    ])

  const readBurnedKeyFact = async (
    tenantRef: string,
    publicKey: string,
  ): Promise<ForgeBurnedKeyFact | undefined> => {
    const row = await db
      .prepare(
        `SELECT burned_key_fact_ref, tenant_ref, key_kind, public_key,
                binding_ref, burn_reason_ref, burned_at, burn_sequence,
                source_refs_json
           FROM forge_burned_key_facts
          WHERE tenant_ref = ? AND public_key = ?
          ORDER BY burn_sequence DESC
          LIMIT 1`,
      )
      .bind(tenantRef, publicKey)
      .first<BurnedKeyRow>()
    return row === null ? undefined : burnedFromRow(row)
  }

  const insertActor = async (
    input: Readonly<{
      accountRef: string
      actorKind: 'human' | 'agent'
      bindingRef: string
      createdAt: string
      displayName: string
      ownerBindingRef: string | null
      roleRefs: ReadonlyArray<string>
      tenantRef: string
    }>,
  ): Promise<ForgeActorBinding> => {
    const byAccount = await readActorBindingByAccount(
      input.tenantRef,
      input.accountRef,
      input.actorKind,
    )
    if (byAccount !== undefined) {
      if (
        byAccount.bindingRef === input.bindingRef &&
        byAccount.accountRef === input.accountRef &&
        byAccount.membershipState === 'active'
      ) {
        return byAccount
      }
      throw conflict('The Forge account is already bound.')
    }

    await db
      .prepare(
        `INSERT INTO forge_actor_bindings
          (${actorColumns})
         VALUES (?, ?, ?, ?, ?, ?, ?, 'active', 1, ?, NULL, NULL, NULL, NULL, 0)`,
      )
      .bind(
        input.bindingRef,
        input.tenantRef,
        input.accountRef,
        input.actorKind,
        input.displayName,
        input.ownerBindingRef,
        json(input.roleRefs),
        input.createdAt,
      )
      .run()
    const stored = await readActorBindingByRef(
      input.tenantRef,
      input.bindingRef,
    )
    if (stored === undefined) {
      throw conflict('The Forge actor binding write did not persist.')
    }
    return stored
  }

  const assertNostrIdentityAvailable = async (
    tenantRef: string,
    bindingRef: string,
    nostrPubkey: string,
  ): Promise<void> => {
    if ((await readBurnedKeyFact(tenantRef, nostrPubkey)) !== undefined) {
      throw burned()
    }
    const byKey = await readActorBindingByNostrPubkey(tenantRef, nostrPubkey)
    if (byKey !== undefined && byKey.bindingRef !== bindingRef) {
      throw conflict('The Nostr key is already bound.')
    }
  }

  const projectNostrIdentity = async (
    input: Readonly<{
      bindingEventCreatedAt: string
      bindingEventId: string
      bindingRef: string
      nostrPubkey: string
      tenantRef: string
    }>,
  ): Promise<ForgeActorBinding> => {
    await assertNostrIdentityAvailable(
      input.tenantRef,
      input.bindingRef,
      input.nostrPubkey,
    )

    await db
      .prepare(
        `UPDATE forge_actor_bindings
            SET nostr_pubkey = ?,
                nostr_binding_event_id = ?,
                nostr_binding_created_at = ?,
                nostr_binding_signature_valid = 1
          WHERE tenant_ref = ?
            AND binding_ref = ?
            AND (
              nostr_pubkey IS NULL
              OR (
                nostr_pubkey = ?
                AND nostr_binding_event_id = ?
              )
            )`,
      )
      .bind(
        input.nostrPubkey,
        input.bindingEventId,
        input.bindingEventCreatedAt,
        input.tenantRef,
        input.bindingRef,
        input.nostrPubkey,
        input.bindingEventId,
      )
      .run()
    const projected = await readActorBindingByRef(
      input.tenantRef,
      input.bindingRef,
    )
    if (
      projected?.nostrPubkey !== input.nostrPubkey ||
      projected.nostrBindingEventId !== input.bindingEventId ||
      !projected.nostrBindingSignatureValid
    ) {
      throw conflict('The Nostr identity projection conflicts with this actor.')
    }
    return projected
  }

  const tombstoneMember: ForgeInviteMembershipStore['tombstoneMember'] =
    async input => {
      const target = await readActorBindingByRef(
        input.tenantRef,
        input.bindingRef,
      )
      if (target === undefined) {
        return []
      }
      const children = await db
        .prepare(
          `SELECT ${actorColumns}
             FROM forge_actor_bindings
            WHERE tenant_ref = ?
              AND owner_binding_ref = ?
              AND membership_state = 'active'`,
        )
        .bind(input.tenantRef, input.bindingRef)
        .all<ActorBindingRow>()
      const affected = [target, ...children.results.map(actorFromRow)].filter(
        binding => binding.membershipState === 'active',
      )

      for (const binding of affected) {
        if (binding.nostrPubkey !== null) {
          await db
            .prepare(
              `INSERT INTO forge_burned_key_facts
                (burned_key_fact_ref, tenant_ref, key_kind, public_key,
                 binding_ref, burn_reason_ref, burned_at, burn_sequence,
                 source_refs_json)
               SELECT ?, ?, ?, ?, ?, ?, ?,
                      COALESCE(MAX(burn_sequence), 0) + 1, ?
                 FROM forge_burned_key_facts
                WHERE tenant_ref = ?
               ON CONFLICT(burned_key_fact_ref) DO NOTHING`,
            )
            .bind(
              `forge_burned_key.${input.tenantRef}.${binding.nostrPubkey}`,
              input.tenantRef,
              binding.actorKind,
              binding.nostrPubkey,
              binding.bindingRef,
              input.burnReasonRef,
              input.nowIso,
              json(input.sourceRefs),
              input.tenantRef,
            )
            .run()
        }
        await db
          .prepare(
            `UPDATE forge_actor_bindings
                SET membership_state = 'tombstoned',
                    revoked_at = ?,
                    binding_generation = binding_generation + 1
              WHERE tenant_ref = ?
                AND binding_ref = ?
                AND membership_state = 'active'`,
          )
          .bind(input.nowIso, input.tenantRef, binding.bindingRef)
          .run()
      }

      const resolved: Array<ForgeActorBinding> = []
      for (const binding of affected) {
        const current = await readActorBindingByRef(
          input.tenantRef,
          binding.bindingRef,
        )
        if (current !== undefined) {
          resolved.push(current)
        }
      }
      return resolved
    }

  const readReconciliationState = async (
    tenantRef: string,
    bindingRef: string,
  ): Promise<ForgeMembershipReconciliationState | undefined> => {
    const row = await db
      .prepare(
        `SELECT reconciliation_ref, tenant_ref, team_ref, binding_ref,
                source_membership_generation, reconciliation_generation,
                observed_present, absence_first_observed_at,
                absence_confirmed_at, hysteresis_deadline, state,
                reconciled_at, source_refs_json
           FROM forge_membership_reconciliation_state
          WHERE tenant_ref = ? AND binding_ref = ?
          LIMIT 1`,
      )
      .bind(tenantRef, bindingRef)
      .first<ReconciliationRow>()
    return row === null ? undefined : reconciliationFromRow(row)
  }

  return {
    readActorBindingByRef,
    readActorBindingByNostrPubkey,
    readActorBindingByAccount,

    isBurnedKey: async (tenantRef, publicKey) =>
      (await readBurnedKeyFact(tenantRef, publicKey)) !== undefined,

    readBurnedKeyFact,

    readInviteBinding: async (tenantRef, inviteRef) => {
      const row = await db
        .prepare(
          `SELECT invite_binding_ref, tenant_ref, team_ref, invite_ref,
                  invite_digest, invite_kind, inviter_binding_ref,
                  invited_subject_ref, role_refs_json, issued_at, expires_at,
                  accepted_at, accepted_binding_ref,
                  provenance_source_refs_json
             FROM forge_invite_bindings
            WHERE tenant_ref = ? AND invite_ref = ?
            LIMIT 1`,
        )
        .bind(tenantRef, inviteRef)
        .first<InviteBindingRow>()
      return row === null ? undefined : inviteFromRow(row)
    },

    bindHuman: async input => {
      await assertNostrIdentityAvailable(
        input.tenantRef,
        input.bindingRef,
        input.nostrPubkey,
      )
      const nativeActor = await insertActor({
        accountRef: input.accountRef,
        actorKind: 'human',
        bindingRef: input.bindingRef,
        createdAt: input.acceptedAt,
        displayName: input.displayName,
        ownerBindingRef: null,
        roleRefs: input.roleRefs,
        tenantRef: input.tenantRef,
      })
      const actor = await projectNostrIdentity({
        bindingEventCreatedAt: input.bindingEventCreatedAt,
        bindingEventId: input.bindingEventId,
        bindingRef: nativeActor.bindingRef,
        nostrPubkey: input.nostrPubkey,
        tenantRef: input.tenantRef,
      })
      await db
        .prepare(
          `INSERT INTO forge_invite_bindings
            (invite_binding_ref, tenant_ref, team_ref, invite_ref,
             invite_digest, invite_kind, inviter_binding_ref,
             invited_subject_ref, role_refs_json, issued_at, expires_at,
             accepted_at, accepted_binding_ref, provenance_source_refs_json)
           VALUES (?, ?, ?, ?, ?, 'team_workspace', ?, ?, ?, ?, ?, ?, ?, ?)
           ON CONFLICT(tenant_ref, invite_ref) DO NOTHING`,
        )
        .bind(
          input.inviteBindingRef,
          input.tenantRef,
          input.teamRef,
          input.inviteRef,
          input.inviteDigest,
          input.inviterBindingRef,
          input.invitedSubjectRef,
          json(input.roleRefs),
          input.issuedAt,
          input.expiresAt,
          input.acceptedAt,
          actor.bindingRef,
          json(input.provenanceSourceRefs),
        )
        .run()
      return actor
    },

    attachAgent: async input => {
      const owner = await readActorBindingByRef(
        input.tenantRef,
        input.ownerBindingRef,
      )
      if (
        owner === undefined ||
        owner.actorKind !== 'human' ||
        owner.membershipState !== 'active' ||
        owner.nostrPubkey === null
      ) {
        throw new ForgeInvitePolicyError({
          code: 'owner_membership_required',
          reason: 'An active invited human owner is required.',
        })
      }
      verifyForgeOwnerAttestation({
        agentPubkey: input.nostrPubkey,
        ownerAuthTag: input.ownerAuthTag,
        ownerPubkey: owner.nostrPubkey,
      })
      await assertNostrIdentityAvailable(
        input.tenantRef,
        input.bindingRef,
        input.nostrPubkey,
      )
      const nativeActor = await insertActor({
        accountRef: input.accountRef,
        actorKind: 'agent',
        bindingRef: input.bindingRef,
        createdAt: input.nowIso,
        displayName: input.displayName,
        ownerBindingRef: owner.bindingRef,
        roleRefs: owner.roleRefs,
        tenantRef: input.tenantRef,
      })
      return projectNostrIdentity({
        bindingEventCreatedAt: input.bindingEventCreatedAt,
        bindingEventId: input.bindingEventId,
        bindingRef: nativeActor.bindingRef,
        nostrPubkey: input.nostrPubkey,
        tenantRef: input.tenantRef,
      })
    },

    tombstoneMember,

    consumeNip98Replay: async consumption => {
      const result = await db
        .prepare(
          `INSERT INTO forge_nip98_replay_consumptions
            (consumption_ref, tenant_ref, request_digest, event_id,
             actor_pubkey, http_method, canonical_path, body_digest,
             event_created_at, consumed_at, expires_at, authority_generation,
             result)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
           ON CONFLICT DO NOTHING`,
        )
        .bind(
          consumption.consumptionRef,
          consumption.tenantRef,
          consumption.requestDigest,
          consumption.eventId,
          consumption.actorPubkey,
          consumption.httpMethod,
          consumption.canonicalPath,
          consumption.bodyDigest,
          consumption.eventCreatedAt,
          consumption.consumedAt,
          consumption.expiresAt,
          consumption.authorityGeneration,
          consumption.result,
        )
        .run()
      return (result.meta?.changes ?? 0) === 1
    },

    readReconciliationState,

    reconcileMembership: async input => {
      const current = await readReconciliationState(
        input.tenantRef,
        input.bindingRef,
      )
      if (!input.querySucceeded) {
        return current
      }
      if (
        current !== undefined &&
        input.sourceMembershipGeneration <= current.sourceMembershipGeneration
      ) {
        return current
      }

      const nextState = input.observedPresent
        ? 'present'
        : current?.state === 'absence_pending'
          ? 'absence_confirmed'
          : 'absence_pending'
      const firstAbsentAt = input.observedPresent
        ? null
        : (current?.absenceFirstObservedAt ?? input.nowIso)
      const confirmedAt =
        nextState === 'absence_confirmed' ? input.nowIso : null
      const deadline = input.observedPresent ? null : firstAbsentAt
      const reconciliationRef =
        current?.reconciliationRef ??
        `forge_membership_reconciliation.${input.tenantRef}.${input.bindingRef}`

      await db
        .prepare(
          `INSERT INTO forge_membership_reconciliation_state
            (reconciliation_ref, tenant_ref, team_ref, binding_ref,
             source_membership_generation, reconciliation_generation,
             observed_present, absence_first_observed_at,
             absence_confirmed_at, hysteresis_deadline, state,
             reconciled_at, source_refs_json)
           VALUES (?, ?, ?, ?, ?, 1, ?, ?, ?, ?, ?, ?, ?)
           ON CONFLICT(tenant_ref, binding_ref) DO UPDATE SET
             team_ref = excluded.team_ref,
             source_membership_generation =
               excluded.source_membership_generation,
             reconciliation_generation =
               forge_membership_reconciliation_state.reconciliation_generation + 1,
             observed_present = excluded.observed_present,
             absence_first_observed_at = excluded.absence_first_observed_at,
             absence_confirmed_at = excluded.absence_confirmed_at,
             hysteresis_deadline = excluded.hysteresis_deadline,
             state = excluded.state,
             reconciled_at = excluded.reconciled_at,
             source_refs_json = excluded.source_refs_json`,
        )
        .bind(
          reconciliationRef,
          input.tenantRef,
          input.teamRef,
          input.bindingRef,
          input.sourceMembershipGeneration,
          input.observedPresent ? 1 : 0,
          firstAbsentAt,
          confirmedAt,
          deadline,
          nextState,
          input.nowIso,
          json(input.sourceRefs),
        )
        .run()

      if (nextState === 'absence_confirmed') {
        await tombstoneMember({
          bindingRef: input.bindingRef,
          burnReasonRef: 'forge.membership.reconciliation_absence_confirmed',
          nowIso: input.nowIso,
          sourceRefs: input.sourceRefs,
          tenantRef: input.tenantRef,
        })
      }
      return readReconciliationState(input.tenantRef, input.bindingRef)
    },
  }
}
