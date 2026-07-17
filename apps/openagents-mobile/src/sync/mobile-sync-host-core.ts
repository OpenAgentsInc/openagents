import {
  ClientGroupId,
  ClientId,
  LocalIdentityRef,
  deviceLocalScope,
  personalScope,
  SyncSchemaVersion,
  type SyncScope,
} from "@openagentsinc/khala-sync"
import {
  createHttpKhalaSyncTransport,
  createChatClientMutators,
  createKhalaSyncSession,
  createKhalaSyncConversation,
  createKhalaSyncAgentTimeline,
  createKhalaSyncLiveAgentGraph,
  createKhalaSyncCodingCatalog,
  createKhalaSyncPortableSessions,
  createKhalaSyncAttentionInbox,
  createPortableRequestCommandMutator,
  createKhalaSyncCodingComposerDrafts,
  createKhalaSyncRuntimeInteractions,
  createKhalaSyncRuntimeCommands,
  createRuntimeInteractionClientMutator,
  createRuntimeClientMutators,
  createOverlay,
  type HttpTransportConfig,
  type KhalaSyncSession,
  type KhalaSyncConversation,
  type KhalaSyncAgentTimeline,
  type KhalaSyncLiveAgentGraph,
  type KhalaSyncCodingCatalog,
  type KhalaSyncCodingComposerDrafts,
  type ConfirmedPortableSessionSnapshot,
  type ConfirmedRuntimeAttentionSnapshot,
  type KhalaSyncAttentionInbox,
  type KhalaSyncPortableSessions,
  type KhalaSyncRuntimeInteractions,
  type KhalaSyncRuntimeCommands,
  type KhalaSyncSessionOptions,
  type KhalaSyncTransport,
  type ScopeSyncState,
} from "@openagentsinc/khala-sync-client"
import type { KhalaSyncExpoSqliteStore } from "@openagentsinc/khala-sync-client/expo-sqlite-store"
import { Effect } from "effect"

import {
  openMobileCodingNavigation,
  type MobileCodingNavigation,
} from "../coding/mobile-coding-navigation"

export const MobileSyncSchemaVersion = SyncSchemaVersion.make(1)

export type MobileSyncHostStatus = Readonly<{
  state: "local_ready" | "closed"
  syncPhase: ScopeSyncState["phase"] | "closed"
  lastDeltaAt: number | null
  schemaVersion: number
  identityState: "persisted"
  pendingMutationCount: number
  identityTier:"local_only"|"account_linked"
}>

export type MobileSyncHost = Readonly<{
  status: () => MobileSyncHostStatus
  conversation: () => KhalaSyncConversation | null
  timeline: () => KhalaSyncAgentTimeline | null
  agentGraph: () => KhalaSyncLiveAgentGraph | null
  runtime: () => KhalaSyncRuntimeCommands | null
  interactions: () => KhalaSyncRuntimeInteractions | null
  drafts: () => KhalaSyncCodingComposerDrafts | null
  coding: () => MobileCodingNavigation
  portable: () => KhalaSyncPortableSessions | null
  attention: () => KhalaSyncAttentionInbox | null
  watchPortable: (listener: (snapshot: ConfirmedPortableSessionSnapshot) => void) => () => void
  watchAttention: (listener: (snapshot: ConfirmedRuntimeAttentionSnapshot) => void) => () => void
  connectAuthenticated: (input: MobileAuthenticatedSyncInput) => void
  disconnectAuthenticated: () => void
  unlinkAccount:()=>void
  close: () => void
}>

export type MobileAuthenticatedSyncInput = Readonly<{
  verification:"server_verified"
  baseUrl: string
  ownerUserId: string
  authToken: () => string
  createTransport?: (config: HttpTransportConfig) => KhalaSyncTransport
  sessionOptions?: KhalaSyncSessionOptions
  now?: () => string
}>

/**
 * Host lifecycle around the shared Expo SQLite adapter. This core accepts its
 * native opener so Bun can prove restart semantics without loading Expo.
 */
export const openMobileSyncHostCore = (input: Readonly<{
  databaseName: string
  randomId: () => string
  openStore: (databaseName: string) => KhalaSyncExpoSqliteStore
}>): MobileSyncHost => {
  const store = input.openStore(input.databaseName)
  let closed = false
  let session: KhalaSyncSession | null = null
  let conversation: KhalaSyncConversation | null = null
  let timeline: KhalaSyncAgentTimeline | null = null
  let agentGraph: KhalaSyncLiveAgentGraph | null = null
  let runtime: KhalaSyncRuntimeCommands | null = null
  let interactions: KhalaSyncRuntimeInteractions | null = null
  let drafts: KhalaSyncCodingComposerDrafts | null = null
  let codingCatalog: KhalaSyncCodingCatalog | null = null
  let portable: KhalaSyncPortableSessions | null = null
  let attention: KhalaSyncAttentionInbox | null = null
  let scope: SyncScope | null = null
  try {
    const persisted = Effect.runSync(store.identity())
    if (persisted === null) {
      Effect.runSync(store.setIdentity({
        clientGroupId: ClientGroupId.make(`openagents-mobile.${input.randomId()}`),
        clientId: ClientId.make(`mobile.${input.randomId()}`),
        schemaVersion: MobileSyncSchemaVersion,
      }))
    }
    if(Effect.runSync(store.localIdentity())===null)Effect.runSync(store.setLocalIdentity({schemaVersion:1,identityRef:LocalIdentityRef.make(`local_${input.randomId()}`),createdAt:new Date().toISOString()}))
  } catch (error) {
    try {
      Effect.runSync(store.close())
    } catch {
      // Preserve the identity/bootstrap failure as the actionable error.
    }
    throw error
  }

  const localIdentity = Effect.runSync(store.localIdentity())
  if (localIdentity === null) {
    Effect.runSync(store.close())
    throw new Error("mobile local identity is unavailable")
  }
  const coding = openMobileCodingNavigation({
    store,
    deviceScope: deviceLocalScope(localIdentity.identityRef),
    catalog: () => codingCatalog,
    ownerScope: () => scope,
  })
  drafts = createKhalaSyncCodingComposerDrafts({
    store,
    deviceScope: deviceLocalScope(localIdentity.identityRef),
    ownerRef: String(localIdentity.identityRef),
  })

  const disconnectAuthenticated = (revoke = false): void => {
    if (session === null) return
    const closing = session
    session = null
    conversation = null
    timeline = null
    agentGraph = null
    runtime = null
    interactions = null
    codingCatalog = null
    portable = null
    attention = null
    scope = null
    Effect.runSync(revoke ? closing.revoke() : closing.close())
  }

  return {
    status: () => ({
      state: closed ? "closed" : "local_ready",
      syncPhase: closed
        ? "closed"
        : session === null || scope === null
          ? "idle"
          : session.state(scope).phase,
      lastDeltaAt: closed || session === null || scope === null
        ? null
        : session.lastDeltaAt(scope),
      schemaVersion: Number(MobileSyncSchemaVersion),
      identityState: "persisted",
      pendingMutationCount: closed ? 0 : Effect.runSync(store.pendingMutations()).length,
      identityTier:closed?"local_only":Effect.runSync(store.localAccountLink())===null?"local_only":"account_linked",
    }),
    conversation: () =>
      session !== null && scope !== null && session.state(scope).phase === "live"
        ? conversation
        : null,
    timeline: () =>
      session !== null && scope !== null && session.state(scope).phase === "live"
        ? timeline
        : null,
    agentGraph: () =>
      session !== null && scope !== null && session.state(scope).phase === "live"
        ? agentGraph
        : null,
    runtime: () =>
      session !== null && scope !== null && session.state(scope).phase === "live"
        ? runtime
        : null,
    interactions: () =>
      session !== null && scope !== null && session.state(scope).phase === "live"
        ? interactions
        : null,
    drafts: () => closed ? null : drafts,
    coding: () => coding,
    portable: () =>
      session !== null && scope !== null && session.state(scope).phase === "live"
        ? portable
        : null,
    attention: () =>
      session !== null && scope !== null && session.state(scope).phase === "live"
        ? attention
        : null,
    watchPortable: listener => {
      if (session === null || scope === null || portable === null || session.state(scope).phase !== "live") {
        return () => undefined
      }
      const watchedSession = session
      const watchedScope = scope
      const watchedPortable = portable
      return watchedSession.subscribeChanges(changedScope => {
        if (String(changedScope) !== String(watchedScope) ||
            watchedSession.state(watchedScope).phase !== "live") return
        void Effect.runPromise(watchedPortable.snapshot()).then(listener, () => undefined)
      })
    },
    watchAttention: listener => {
      if (session === null || scope === null || attention === null || session.state(scope).phase !== "live") {
        return () => undefined
      }
      const watchedSession = session
      const watchedScope = scope
      const watchedAttention = attention
      return watchedSession.subscribeChanges(changedScope => {
        if (String(changedScope) !== String(watchedScope) ||
            watchedSession.state(watchedScope).phase !== "live") return
        void Effect.runPromise(watchedAttention.snapshot()).then(listener, () => undefined)
      })
    },
    connectAuthenticated: connection => {
      if (closed) throw new Error("mobile Sync host is closed")
      const ownerUserId = connection.ownerUserId.trim()
      if (connection.verification!=="server_verified"||ownerUserId === "" || connection.authToken().trim() === "") {
        throw new Error("mobile authenticated Sync credential is incomplete")
      }
      disconnectAuthenticated()
      const identity = Effect.runSync(store.identity())
      const localIdentity=Effect.runSync(store.localIdentity())
      if (identity === null) throw new Error("mobile Sync identity is unavailable")
      if(localIdentity===null)throw new Error("mobile local identity is unavailable")
      Effect.runSync(store.setLocalAccountLink({schemaVersion:1,identityRef:localIdentity.identityRef,ownerUserId,linkedAt:new Date().toISOString(),linkReceiptRef:`link_${input.randomId()}`}))
      const mutators = createChatClientMutators({
        ownerUserId,
        ...(connection.now === undefined ? {} : { now: connection.now }),
      })
      const runtimeMutators = createRuntimeClientMutators()
      const interactionMutator = createRuntimeInteractionClientMutator()
      const portableCommandMutator = createPortableRequestCommandMutator()
      const overlay = Effect.runSync(createOverlay(store, [
        ...Object.values(mutators),
        ...Object.values(runtimeMutators),
        interactionMutator,
        portableCommandMutator,
      ]))
      const transportConfig = {
        baseUrl: connection.baseUrl,
        authToken: connection.authToken,
      }
      const transport = connection.createTransport?.(transportConfig) ??
        createHttpKhalaSyncTransport(transportConfig)
      scope = personalScope(ownerUserId)
      session = createKhalaSyncSession({
        baseUrl: connection.baseUrl,
        clientGroupId: identity.clientGroupId,
        clientId: identity.clientId,
        schemaVersion: identity.schemaVersion,
        authToken: connection.authToken,
      }, store, overlay, transport, connection.sessionOptions)
      conversation = createKhalaSyncConversation({
        ownerUserId,
        store,
        session,
        mutators,
      })
      timeline = createKhalaSyncAgentTimeline({ store, session })
      agentGraph = createKhalaSyncLiveAgentGraph({ store, session })
      runtime = createKhalaSyncRuntimeCommands({ mutators: runtimeMutators, session, store })
      interactions = createKhalaSyncRuntimeInteractions({
        store,
        session,
        mutator: interactionMutator,
      })
      codingCatalog = createKhalaSyncCodingCatalog({ store, session, ownerScope: scope })
      portable = createKhalaSyncPortableSessions({
        ownerRef: ownerUserId,
        ownerScope: scope,
        store,
        session,
        mutator: portableCommandMutator,
      })
      attention = createKhalaSyncAttentionInbox({
        ownerRef: ownerUserId,
        ownerScope: scope,
        store,
        session,
      })
      Effect.runSync(session.subscribe(scope))
    },
    disconnectAuthenticated,
    unlinkAccount:()=>{try{disconnectAuthenticated(true)}finally{Effect.runSync(store.clearLocalAccountLink())}},
    close: () => {
      if (closed) return
      closed = true
      drafts = null
      void coding.clearActive()
      disconnectAuthenticated()
      Effect.runSync(store.close())
    },
  }
}
