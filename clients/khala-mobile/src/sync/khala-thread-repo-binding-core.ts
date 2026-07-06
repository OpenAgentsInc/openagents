/**
 * MM-B2 (#8472): thread<->repo binding — the pure client-mutator logic that
 * sets/clears `ChatThreadEntity.repoBinding` (the typed contract landed in
 * `packages/khala-sync/src/chat.ts` for this issue). Mirrors the existing
 * `chatRenameThreadClientMutator` pattern in
 * `packages/khala-sync-db-collection` (read current entity via the overlay
 * view, decode, apply the change, re-encode, upsert into both the owner's
 * personal scope and the thread-local scope) — but lives here rather than in
 * that package, since this issue's scope is `clients/khala-mobile` plus the
 * one contract addition in `packages/khala-sync` (see AGENTS/issue scope
 * note), not `khala-sync-db-collection`.
 *
 * KNOWN GAP (honest, tracked): `CHAT_BIND_THREAD_REPO_MUTATOR_NAME` is not
 * yet a server-recognized mutator in `khala-sync-server` — pushing this
 * mutation through the sync session's push loop will currently be rejected
 * server-side as an unknown mutator (visible via the runtime's existing
 * rejection tracking). The LOCAL optimistic apply below still works fully
 * offline-first (the phone's own durable store and UI reflect the binding
 * immediately), which is real and useful on its own, but a repo binding does
 * not durably reach the cloud executor until a server-side counterpart
 * mutator lands. The exact contract a server implementer needs is posted as
 * a comment on issue #8472.
 */
import {
  canonicalJson,
  CHAT_THREAD_ENTITY_TYPE,
  decodeChatThreadEntity,
  decodeChatThreadRepoBinding,
  encodeChatThreadEntity,
  MutatorName,
  personalScope,
  threadScope,
  type ChatThreadEntity,
  type ChatThreadRepoBinding,
} from "@openagentsinc/khala-sync"
import type { ClientMutator, OverlayEffect, OverlayReadView } from "@openagentsinc/khala-sync-client"

export const CHAT_BIND_THREAD_REPO_MUTATOR_NAME = "chat.bindThreadRepo"

export type ChatBindThreadRepoArgs = Readonly<{
  threadId: string
  /** `null` explicitly clears the binding (repo-less thread); omitted keys
   * are never produced by the picker UI (it always sends a full binding or
   * an explicit clear). */
  repo: Readonly<{ defaultBranch: string; name: string; owner: string }> | null
}>

export type ChatBindThreadRepoOptions = Readonly<{
  now?: () => string
  ownerUserId: string
}>

const defaultNowIso = (): string => new Date().toISOString()

/** Reads the current thread entity from the overlay view (falling back to a
 * minimal placeholder if the thread has never been seen yet — matching
 * `chatRenameThreadClientMutator`'s own defensive fallback shape) and returns
 * the updated entity with `repoBinding` set/cleared. Exported standalone
 * (not just embedded in the mutator) so the picker UI can preview the
 * resulting entity before committing, and so this is directly unit-testable
 * without an overlay. */
export const applyThreadRepoBinding = (
  current: ChatThreadEntity,
  repo: ChatBindThreadRepoArgs["repo"],
): ChatThreadEntity =>
  decodeChatThreadEntity({
    ...current,
    repoBinding:
      repo === null
        ? null
        : {
            defaultBranch: repo.defaultBranch,
            name: repo.name,
            owner: repo.owner,
          },
  })

const placeholderThread = (
  threadId: string,
  options: ChatBindThreadRepoOptions,
): ChatThreadEntity => {
  const now = (options.now ?? defaultNowIso)()
  return decodeChatThreadEntity({
    createdAt: now,
    lastMessageAt: null,
    messageCount: 0,
    ownerUserId: options.ownerUserId,
    status: "active",
    threadId,
    title: "",
    updatedAt: now,
  })
}

/** Builds the two `upsert` effects (owner personal scope + thread-local
 * scope) a bound thread needs, matching `chatThreadOverlayEffects`'s shape in
 * `khala-sync-db-collection` exactly (same two scopes, same canonical-JSON
 * post-image contract) so a future server-side mutator or client refactor
 * can converge on one shape without a behavior change. */
export const chatThreadRepoBindingOverlayEffects = (
  entity: ChatThreadEntity,
): ReadonlyArray<Extract<OverlayEffect, { kind: "upsert" }>> => {
  const postImageJson = canonicalJson(encodeChatThreadEntity(entity))
  return [
    {
      entityId: entity.threadId,
      entityType: CHAT_THREAD_ENTITY_TYPE,
      kind: "upsert",
      postImageJson,
      scope: personalScope(entity.ownerUserId),
    },
    {
      entityId: entity.threadId,
      entityType: CHAT_THREAD_ENTITY_TYPE,
      kind: "upsert",
      postImageJson,
      scope: threadScope(entity.threadId),
    },
  ]
}

export const chatBindThreadRepoClientMutator = (
  options: ChatBindThreadRepoOptions,
): ClientMutator<ChatBindThreadRepoArgs> => ({
  apply: (args: ChatBindThreadRepoArgs, view: OverlayReadView) => {
    const scope = personalScope(options.ownerUserId)
    const currentJson = view.get(scope, CHAT_THREAD_ENTITY_TYPE, args.threadId)
    const current =
      currentJson === undefined
        ? placeholderThread(args.threadId, options)
        : decodeChatThreadEntity(JSON.parse(currentJson) as unknown)
    return chatThreadRepoBindingOverlayEffects(applyThreadRepoBinding(current, args.repo))
  },
  name: MutatorName.make(CHAT_BIND_THREAD_REPO_MUTATOR_NAME),
})

export const decodePickedRepoBinding = (
  input: Readonly<{ defaultBranch: string; name: string; owner: string }>,
): ChatThreadRepoBinding => decodeChatThreadRepoBinding(input)
