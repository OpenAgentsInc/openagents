// Khala Sync mutator registry for the openagents.com Worker (KS-3.1, #8291).
//
// Named, server-authoritative mutators (docs/khala-sync/SPEC.md §2.4): each
// runs inside ONE Postgres transaction via the push engine
// (`@openagentsinc/khala-sync-server`), performing permission check →
// validation → business writes → changelog appends, and returning an
// in-band `MutationResult`. Rejections are VALUES that ack the mutation;
// mutators must validate/permission-check BEFORE writing (the engine
// commits the transaction even for rejected results so the ledger ack can
// commit).
//
// v1 registry: ONE system-test mutator, `sync.debugEcho`, so the push route
// is end-to-end exercisable (client → route → Hyperdrive → transaction →
// changelog) before the KS-3.2 fleet mutators land. It writes a single
// `sync_debug_echo` entity into the CALLER'S OWN personal scope and nothing
// else — it is guarded to that scope, so it can never fan data into team,
// fleet, thread, or public scopes.

import { Schema as S } from 'effect'

import {
  EntityId,
  EntityType,
  MutationResult,
  MutatorName,
  personalScope,
  SyncScope,
} from '@openagentsinc/khala-sync'
import {
  defineMutator,
  fleetOperatorMutators,
  makeMutatorRegistry,
  type MutatorDefinition,
  type MutatorRegistry,
} from '@openagentsinc/khala-sync-server'

import { parseJsonUnknown } from './json-boundary'

export const SYNC_DEBUG_ECHO_MUTATOR_NAME = 'sync.debugEcho'
export const SYNC_DEBUG_ECHO_ENTITY_TYPE = 'sync_debug_echo'

/** In-band rejection code when the target scope is not the caller's own. */
export const SYNC_DEBUG_ECHO_SCOPE_REJECTION = 'unauthorized_scope'

const DebugEchoArgs = S.Struct({
  /** Must be the caller's own personal scope (`scope.user.<userId>`). */
  scope: SyncScope,
  entityId: S.String.check(S.isMinLength(1), S.isMaxLength(256)),
  echo: S.String.check(S.isMaxLength(4096)),
})
type DebugEchoArgs = typeof DebugEchoArgs.Type

export const decodeDebugEchoArgs = (argsJson: string): DebugEchoArgs =>
  S.decodeUnknownSync(DebugEchoArgs)(parseJsonUnknown(argsJson))

export const debugEchoMutator: MutatorDefinition =
  defineMutator<DebugEchoArgs>({
    decodeArgs: decodeDebugEchoArgs,
    execute: async (args, ctx) => {
      // Permission check BEFORE any write: debugEcho may only touch the
      // caller's own personal scope.
      const ownScope = personalScope(ctx.userId)
      if (args.scope !== ownScope) {
        return new MutationResult({
          errorCode: SYNC_DEBUG_ECHO_SCOPE_REJECTION,
          errorMessageSafe:
            'sync.debugEcho may only write to the caller’s own personal scope',
          mutationId: ctx.mutationId,
          status: 'rejected',
        })
      }

      await ctx.writer.appendChange({
        entityId: EntityId.make(args.entityId),
        entityType: EntityType.make(SYNC_DEBUG_ECHO_ENTITY_TYPE),
        mutationRef: ctx.mutationRef,
        op: 'upsert',
        postImage: {
          echo: args.echo,
          entityId: args.entityId,
          scope: args.scope,
        },
        scope: args.scope,
      })

      return new MutationResult({
        mutationId: ctx.mutationId,
        status: 'applied',
      })
    },
    name: MutatorName.make(SYNC_DEBUG_ECHO_MUTATOR_NAME),
  })

/**
 * The Worker's Khala Sync mutator registry: the `sync.debugEcho` system
 * test mutator plus the fleet cockpit operator mutators (KS-6.1 #8302 +
 * KS-3.2 #8292: `fleet.setDesiredSlots` / `fleet.pauseRun` /
 * `fleet.resumeRun` / `fleet.pauseWorker` / `fleet.resumeWorker` /
 * `fleet.acknowledgeInboxFlag` / `fleet.stopRun`, defined in
 * `@openagentsinc/khala-sync-server` and integration-tested there through
 * `executePush` against real Postgres — catalog in
 * docs/khala-sync/MUTATORS.md). The fleet mutators are owner-gated via
 * `khala_sync_scope_owners`; a foreign user gets an in-band
 * `unauthorized_scope` rejection, and `fleet.stopRun` additionally rejects
 * `confirmation_required` without `confirm: true`. HONEST V1: an applied
 * fleet mutation records a durable intent row and projects the updated
 * post-image — supervisor-side enforcement of intents (polling
 * GET /api/internal/khala-sync/fleet-intents) is a follow-up lane (see
 * docs/khala-sync/README.md).
 */
export const makeKhalaSyncWorkerMutatorRegistry = (): MutatorRegistry =>
  makeMutatorRegistry([debugEchoMutator, ...fleetOperatorMutators])
