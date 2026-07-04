import { createCollection, type Collection } from '@tanstack/db'

import {
  CHAT_THREAD_ENTITY_TYPE,
  personalScope,
  type ChatThreadEntity,
} from '@openagentsinc/khala-sync'
import type {
  KhalaSyncOverlay,
  KhalaSyncSession,
} from '@openagentsinc/khala-sync-client'
import {
  chatCreateThreadClientMutator,
  chatRenameThreadClientMutator,
  chatThreadKhalaSyncCollectionOptions,
  chatThreadsForSidebar,
  type KhalaSyncCollectionUtils,
} from '@openagentsinc/khala-sync-db-collection'

export type WebChatThreadCollectionInput = Readonly<{
  ownerUserId: string
  overlay: KhalaSyncOverlay
  session: KhalaSyncSession
  now?: () => string
}>

export const WEB_CHAT_THREAD_COLLECTION_ENTITY_TYPE = CHAT_THREAD_ENTITY_TYPE

export const projectWebChatThreadSidebar = (
  threads: Iterable<ChatThreadEntity>,
  searchTerm?: string | null,
): Array<ChatThreadEntity> =>
  chatThreadsForSidebar(threads, {
    ...(searchTerm === undefined ? {} : { searchTerm }),
  })

export const createWebChatThreadCollection = (
  input: WebChatThreadCollectionInput,
): Collection<ChatThreadEntity, string, KhalaSyncCollectionUtils> => {
  const mutatorOptions = {
    ownerUserId: input.ownerUserId,
    ...(input.now === undefined ? {} : { now: input.now }),
  }

  return createCollection(
    chatThreadKhalaSyncCollectionOptions({
      createThreadMutator: chatCreateThreadClientMutator(mutatorOptions),
      ...(input.now === undefined ? {} : { optimisticNow: input.now }),
      ownerUserId: input.ownerUserId,
      overlay: input.overlay,
      renameThreadMutator: chatRenameThreadClientMutator(mutatorOptions),
      scope: personalScope(input.ownerUserId),
      session: input.session,
      startSync: true,
    }),
  )
}
