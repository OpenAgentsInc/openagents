import { Effect, Schema as S } from 'effect'

import {
  type AgentForumIdentityStore,
  makeD1AgentRegistrationStore,
} from './agent-registration'
import type { ForumWriterActorInput } from './forum'
import type { IdentityDb } from './identity-db'

export const ARTANIS_REGISTERED_FORUM_SLUG = 'artanis'
export const LEGACY_ARTANIS_FORUM_ACTOR_REF = 'agent:agent_artanis'

export class ArtanisForumIdentityError extends S.TaggedErrorClass<ArtanisForumIdentityError>()(
  'ArtanisForumIdentityError',
  {
    reason: S.String,
  },
) {}

export type ArtanisForumIdentity = Readonly<{
  actor: ForumWriterActorInput
  actorRef: string
  slug: string | null
  userId: string
}>

export const resolveRegisteredArtanisForumIdentity = (
  store: AgentForumIdentityStore,
  nowIso: string,
): Effect.Effect<ArtanisForumIdentity, ArtanisForumIdentityError> =>
  Effect.tryPromise({
    catch: error =>
      new ArtanisForumIdentityError({
        reason:
          error instanceof Error
            ? error.message
            : 'Registered Artanis Forum identity lookup failed.',
      }),
    try: () =>
      store.findAgentForumIdentity(
        { slug: ARTANIS_REGISTERED_FORUM_SLUG },
        nowIso,
      ),
  }).pipe(
    Effect.flatMap(identity =>
      identity === undefined
        ? Effect.fail(
            new ArtanisForumIdentityError({
              reason:
                'Registered Artanis Forum identity is unavailable; reissue/recover the slug=artanis agent before Forum publication.',
            }),
          )
        : Effect.succeed({
            actor: {
              _tag: 'Agent' as const,
              session: identity.session,
            },
            actorRef: `agent:${identity.session.user.id}`,
            slug: identity.slug,
            userId: identity.session.user.id,
          }),
    ),
  )

// CFG-4 Domain 2 (#8519): the registered-identity lookup now needs BOTH
// stores — D1 for agent_profiles/agent_credentials and the Postgres
// identity handle for the authoritative `users`/`auth_identities` rows.
export const resolveRegisteredArtanisForumIdentityFromD1 = (
  db: D1Database,
  identityDb: IdentityDb,
  nowIso: string,
): Effect.Effect<ArtanisForumIdentity, ArtanisForumIdentityError> =>
  resolveRegisteredArtanisForumIdentity(
    makeD1AgentRegistrationStore(db, identityDb),
    nowIso,
  )

export const isArtanisForumPostActor = (
  actorRef: string,
  identity: ArtanisForumIdentity,
): boolean =>
  actorRef === identity.actorRef || actorRef === LEGACY_ARTANIS_FORUM_ACTOR_REF
