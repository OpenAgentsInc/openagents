/**
 * CX-6 (#8550): durable per-thread Codex continuity pin. This mirrors the
 * repo-binding client mutator shape: optimistic local post-image updates in
 * the owner personal scope and thread-local scope, while the server mutator
 * persists the same public-safe refs beside the thread row.
 */
import {
  canonicalJson,
  CHAT_THREAD_ENTITY_TYPE,
  decodeChatThreadCodexContinuityPin,
  decodeChatThreadEntity,
  encodeChatThreadEntity,
  MutatorName,
  personalScope,
  threadScope,
  type ChatThreadCodexContinuityPin,
  type ChatThreadEntity,
} from "@openagentsinc/khala-sync"
import type { ClientMutator, OverlayEffect, OverlayReadView } from "@openagentsinc/khala-sync-client"

export const CHAT_PIN_CODEX_CONTINUITY_MUTATOR_NAME = "chat.pinCodexContinuity"

export type ChatPinCodexContinuityArgs = Readonly<{
  threadId: string
  codexContinuity: Readonly<{
    provider: "chatgpt_codex"
    providerAccountRef: string
    authGrantRef: string
    accountRefHash?: string
    pinnedAt: string
  }> | null
}>

export type ChatPinCodexContinuityOptions = Readonly<{
  now?: () => string
  ownerUserId: string
}>

const defaultNowIso = (): string => new Date().toISOString()

export const applyThreadCodexContinuityPin = (
  current: ChatThreadEntity,
  codexContinuity: ChatPinCodexContinuityArgs["codexContinuity"],
): ChatThreadEntity =>
  decodeChatThreadEntity({
    ...current,
    codexContinuity:
      codexContinuity === null
        ? null
        : {
            ...(codexContinuity.accountRefHash === undefined
              ? {}
              : { accountRefHash: codexContinuity.accountRefHash }),
            authGrantRef: codexContinuity.authGrantRef,
            pinnedAt: codexContinuity.pinnedAt,
            provider: codexContinuity.provider,
            providerAccountRef: codexContinuity.providerAccountRef,
          },
  })

const placeholderThread = (
  threadId: string,
  options: ChatPinCodexContinuityOptions,
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

export const chatThreadCodexContinuityOverlayEffects = (
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

export const chatPinCodexContinuityClientMutator = (
  options: ChatPinCodexContinuityOptions,
): ClientMutator<ChatPinCodexContinuityArgs> => ({
  apply: (args: ChatPinCodexContinuityArgs, view: OverlayReadView) => {
    const scope = personalScope(options.ownerUserId)
    const currentJson = view.get(scope, CHAT_THREAD_ENTITY_TYPE, args.threadId)
    const current =
      currentJson === undefined
        ? placeholderThread(args.threadId, options)
        : decodeChatThreadEntity(JSON.parse(currentJson) as unknown)
    return chatThreadCodexContinuityOverlayEffects(
      applyThreadCodexContinuityPin(current, args.codexContinuity),
    )
  },
  name: MutatorName.make(CHAT_PIN_CODEX_CONTINUITY_MUTATOR_NAME),
})

export const decodePickedCodexContinuityPin = (
  input: ChatPinCodexContinuityArgs["codexContinuity"],
): ChatThreadCodexContinuityPin | null =>
  input === null ? null : decodeChatThreadCodexContinuityPin(input)
