import { chmodSync, mkdirSync } from "node:fs"
import path from "node:path"

import {
  ClientGroupId,
  ClientId,
  deviceLocalScope,
  LocalIdentityRef,
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
  createKhalaSyncPortableSessions,
  createPortableRequestCommandMutator,
  createKhalaSyncCodingComposerDrafts,
  createCodingCatalogPublishMutator,
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
  type KhalaSyncPortableSessions,
  type ConfirmedPortableSessionSnapshot,
  type KhalaSyncCodingComposerDrafts,
  type KhalaSyncRuntimeInteractions,
  type KhalaSyncRuntimeCommands,
  type KhalaSyncSessionOptions,
  type KhalaSyncTransport,
  type ScopeSyncState,
} from "@openagentsinc/khala-sync-client"
import type { PortableSessionCommand } from "@openagentsinc/portable-session-contract"
import { Effect } from "effect"
import { openDesktopSyncStore, type DesktopSyncStore } from "./desktop-sync-store.ts"
import {
  openDesktopCodingCatalog,
  type DesktopCodingCatalog,
} from "./desktop-coding-catalog.ts"

export const DesktopSyncSchemaVersion = SyncSchemaVersion.make(1)

export type DesktopSyncHostStatus = Readonly<{
  state: "local_ready" | "closed"
  syncPhase: ScopeSyncState["phase"] | "closed"
  lastDeltaAt: number | null
  schemaVersion: number
  identityState: "persisted"
  pendingMutationCount: number
  identityTier:"local_only"|"account_linked"
}>

export type DesktopSyncHost = Readonly<{
  status: () => DesktopSyncHostStatus
  conversation: () => KhalaSyncConversation | null
  timeline: () => KhalaSyncAgentTimeline | null
  agentGraph: () => KhalaSyncLiveAgentGraph | null
  portableSessions: () => KhalaSyncPortableSessions | null
  portableSnapshot: () => ConfirmedPortableSessionSnapshot | null
  requestPortableCommand: (command: PortableSessionCommand) => string | null
  runtime: () => KhalaSyncRuntimeCommands | null
  interactions: () => KhalaSyncRuntimeInteractions | null
  drafts: () => KhalaSyncCodingComposerDrafts | null
  codingCatalog: () => DesktopCodingCatalog | null
  publishCodingCatalog: () => boolean
  connectAuthenticated: (input: DesktopAuthenticatedSyncInput) => void
  disconnectAuthenticated: () => void
  unlinkAccount:()=>void
  close: () => void
}>

export type DesktopAuthenticatedSyncInput = Readonly<{
  verification:"server_verified"
  baseUrl: string
  ownerUserId: string
  authToken: () => string
  createTransport?: (config: HttpTransportConfig) => KhalaSyncTransport
  sessionOptions?: KhalaSyncSessionOptions
  now?: () => string
}>

const secureLocalFiles = (databasePath: string): void => {
  if (process.platform === "win32") return
  for (const candidate of [databasePath, `${databasePath}-wal`, `${databasePath}-shm`]) {
    try {
      chmodSync(candidate, 0o600)
    } catch {
      // WAL/SHM are created lazily. The owning directory remains mode 0700.
    }
  }
}

export const openDesktopSyncHost = (input: Readonly<{
  databasePath: string
  randomId: () => string
  openStore?: (path: string) => DesktopSyncStore
}>): DesktopSyncHost => {
  const directory = path.dirname(input.databasePath)
  mkdirSync(directory, { recursive: true, mode: 0o700 })
  if (process.platform !== "win32") chmodSync(directory, 0o700)

  const store = (input.openStore ?? openDesktopSyncStore)(input.databasePath)
  let closed = false
  let session: KhalaSyncSession | null = null
  let conversation: KhalaSyncConversation | null = null
  let timeline: KhalaSyncAgentTimeline | null = null
  let agentGraph: KhalaSyncLiveAgentGraph | null = null
  let portableSessions: KhalaSyncPortableSessions | null = null
  let runtime: KhalaSyncRuntimeCommands | null = null
  let authenticatedOwnerUserId: string | null = null
  let interactions: KhalaSyncRuntimeInteractions | null = null
  let drafts: KhalaSyncCodingComposerDrafts | null = null
  let codingCatalog: DesktopCodingCatalog | null = null
  let scope: SyncScope | null = null
  const codingCatalogPublishMutator = createCodingCatalogPublishMutator()
  const portableRequestCommandMutator = createPortableRequestCommandMutator()
  try {
    const persisted = Effect.runSync(store.identity())
    if (persisted === null) {
      Effect.runSync(store.setIdentity({
        clientGroupId: ClientGroupId.make(`openagents-desktop.${input.randomId()}`),
        clientId: ClientId.make(`desktop.${input.randomId()}`),
        schemaVersion: DesktopSyncSchemaVersion,
      }))
    }
    if(Effect.runSync(store.localIdentity())===null)Effect.runSync(store.setLocalIdentity({schemaVersion:1,identityRef:LocalIdentityRef.make(`local_${input.randomId()}`),createdAt:new Date().toISOString()}))
    const localIdentity = Effect.runSync(store.localIdentity())
    if (localIdentity === null) throw new Error("desktop local identity is unavailable")
    codingCatalog = openDesktopCodingCatalog({
      store,
      identityRef: localIdentity.identityRef,
      bindingFile: path.join(directory, "coding-bindings.json"),
      randomId: input.randomId,
    })
    drafts = createKhalaSyncCodingComposerDrafts({
      store,
      deviceScope: deviceLocalScope(localIdentity.identityRef),
      ownerRef: String(localIdentity.identityRef),
    })
    secureLocalFiles(input.databasePath)
  } catch (error) {
    Effect.runSync(store.close())
    throw error
  }

  const disconnectAuthenticated = (revoke = false): void => {
    if (session === null) return
    const closing = session
    session = null
    conversation = null
    timeline = null
    agentGraph = null
    portableSessions = null
    runtime = null
    interactions = null
    authenticatedOwnerUserId = null
    scope = null
    Effect.runSync(revoke ? closing.revoke() : closing.close())
  }

  const publishCodingCatalog = (): boolean => {
    if (session === null || codingCatalog === null || authenticatedOwnerUserId === null) return false
    const changeSet = codingCatalog.ownerScopedChangeSet(authenticatedOwnerUserId)
    if (
      changeSet.projects.length + changeSet.repositories.length +
      changeSet.worktrees.length + changeSet.sessions.length +
      Number(changeSet.navigation !== null) === 0
    ) return false
    Effect.runSync(session.mutate(codingCatalogPublishMutator, changeSet))
    return true
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
      schemaVersion: Number(DesktopSyncSchemaVersion),
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
    portableSessions: () =>
      session !== null && scope !== null && session.state(scope).phase === "live"
        ? portableSessions
        : null,
    portableSnapshot: () => {
      const portable = session !== null && scope !== null && session.state(scope).phase === "live"
        ? portableSessions
        : null
      if (portable === null) return null
      try {
        return Effect.runSync(portable.snapshot())
      } catch {
        return null
      }
    },
    requestPortableCommand: command => {
      const portable = session !== null && scope !== null && session.state(scope).phase === "live"
        ? portableSessions
        : null
      if (portable === null) return null
      try {
        return String(Effect.runSync(portable.request(command)))
      } catch {
        return null
      }
    },
    runtime: () =>
      session !== null && scope !== null && session.state(scope).phase === "live"
        ? runtime
        : null,
    interactions: () =>
      session !== null && scope !== null && session.state(scope).phase === "live"
        ? interactions
        : null,
    drafts: () => closed ? null : drafts,
    codingCatalog: () => closed ? null : codingCatalog,
    publishCodingCatalog,
    connectAuthenticated: connection => {
      if (closed) throw new Error("desktop Sync host is closed")
      const ownerUserId = connection.ownerUserId.trim()
      if (connection.verification!=="server_verified"||ownerUserId === "" || connection.authToken().trim() === "") {
        throw new Error("desktop authenticated Sync credential is incomplete")
      }
      disconnectAuthenticated()
      const identity = Effect.runSync(store.identity())
      const localIdentity=Effect.runSync(store.localIdentity())
      if (identity === null) throw new Error("desktop Sync identity is unavailable")
      if(localIdentity===null)throw new Error("desktop local identity is unavailable")
      Effect.runSync(store.setLocalAccountLink({schemaVersion:1,identityRef:localIdentity.identityRef,ownerUserId,linkedAt:new Date().toISOString(),linkReceiptRef:`link_${input.randomId()}`}))
      const mutators = createChatClientMutators({
        ownerUserId,
        ...(connection.now === undefined ? {} : { now: connection.now }),
      })
      const runtimeMutators = createRuntimeClientMutators()
      const interactionMutator = createRuntimeInteractionClientMutator()
      const overlay = Effect.runSync(createOverlay(store, [
        ...Object.values(mutators),
        ...Object.values(runtimeMutators),
        interactionMutator,
        codingCatalogPublishMutator,
        portableRequestCommandMutator,
      ]))
      const transportConfig = {
        baseUrl: connection.baseUrl,
        authToken: connection.authToken,
      }
      const transport = connection.createTransport?.(transportConfig) ??
        createHttpKhalaSyncTransport(transportConfig)
      scope = personalScope(ownerUserId)
      authenticatedOwnerUserId = ownerUserId
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
      portableSessions = createKhalaSyncPortableSessions({
        ownerRef: ownerUserId,
        ownerScope: scope,
        store,
        session,
        mutator: portableRequestCommandMutator,
      })
      runtime = createKhalaSyncRuntimeCommands({ mutators: runtimeMutators, session, store })
      interactions = createKhalaSyncRuntimeInteractions({
        store,
        session,
        mutator: interactionMutator,
      })
      Effect.runSync(session.subscribe(scope))
      publishCodingCatalog()
    },
    disconnectAuthenticated,
    unlinkAccount:()=>{try{disconnectAuthenticated(true)}finally{Effect.runSync(store.clearLocalAccountLink())}},
    close: () => {
      if (closed) return
      closed = true
      codingCatalog = null
      drafts = null
      disconnectAuthenticated()
      Effect.runSync(store.close())
    },
  }
}
