/**
 * @openagentsinc/khala-sync-client — client engine for Khala Sync: local
 * store, transport, bootstrap/catch-up/live state machine, optimistic
 * mutators, and rebase.
 *
 * Spec: docs/khala-sync/SPEC.md §6. Implementation lands per the KS-5
 * workstream issues.
 *
 * Two hard client invariants (SPEC §7):
 * - Optimistic effects live ONLY in the in-memory overlay; the durable
 *   local store holds server-confirmed state exclusively.
 * - Apply is idempotent by (scope, version, entityType, entityId); the
 *   durable cursor, not the connection, is the source of truth.
 */

// Native clients already depend on this package for device-local coding
// drafts. Re-export the exact canonical composer boundary they need so app
// packages do not create a parallel draft model or a second dependency edge.
export {
  fetchFleetRunClientProjection,
  type FleetRunProjectionFetchResult,
} from "./fleet-run-client-projection.js"

// FA-RUN-05 (#8981): FullAutoRun mobile projection fetch (mobile, #8982) and
// publish (Desktop) ergonomics -- mirrors the FleetRunClientProjection
// fetch helper above.
export {
  fetchFullAutoRunClientProjection,
  publishFullAutoRunClientProjection,
  FULL_AUTO_RUNS_PATH,
  type FullAutoRunProjectionFetchResult,
  type FullAutoRunProjectionPublishResult,
} from "./full-auto-run-client-projection.js"

// MOB-FA-02 (#8994): typed durable Pause/Resume/Stop control-intent
// dispatch/list/outcome-report ergonomics -- the sibling mutation vocabulary
// for the projection route above.
export {
  dispatchFullAutoRunControlIntent,
  fetchFullAutoRunControlIntents,
  reportFullAutoRunControlIntentOutcome,
  FULL_AUTO_RUN_CONTROL_INTENTS_PATH,
  type FullAutoRunControlIntentDispatchResult,
  type FullAutoRunControlIntentFetch,
  type FullAutoRunControlIntentListResult,
  type FullAutoRunControlIntentOutcomeResult,
} from "./full-auto-run-control-intent.js"

export {
  admitFleetAttentionCommand,
  admitFleetRunCommand,
  fleetRunActions,
  projectFleetCockpitCard,
  type FleetAttentionAction,
  type FleetAttentionCommand,
  type FleetAuthority,
  type FleetCockpitCard,
  type FleetCockpitSource,
  type FleetRunAction,
  type FleetRunCommand,
} from "./fleet-cockpit.js"

export {
  applyComposerTransaction,
  composerAttachmentId,
  composerBlockId,
  decodeCodingComposerDraftSnapshot,
  DEFAULT_NATIVE_LOCAL_ATTACHMENT_UPLOAD_POLICY,
  emptyComposerSelection,
  emptyComposerState,
  parseComposerMarkdown,
  readyComposerAttachmentTransaction,
  retryComposerAttachmentTransaction,
  serializeComposerMarkdown,
  stageComposerAttachmentFiles,
  type CodingComposerDraftSnapshot,
  type CodingComposerTargetSelection,
  type ComposerAttachmentRefBlock,
  type ComposerDoc,
  type ComposerFileLike,
  type ComposerState,
} from "@openagentsinc/composer-state"

export {
  CHAT_MESSAGE_IMAGE_BYTES_LIMIT,
  CHAT_MESSAGE_IMAGE_COUNT_LIMIT,
  type ChatMessageImageAttachment,
} from "@openagentsinc/khala-sync"

// ---------------------------------------------------------------------------
// Local store (KS-5.1 + KS-5.4): contracts in store.ts; ALL SQL semantics
// in the driver-agnostic store-core.ts. Runtime-specific stores live behind
// explicit subpaths: `./sqlite-store` for Bun, `./expo-sqlite-store` for
// React Native hosts, and `./web` for the SQLite-WASM worker adapter. Keep this
// root entry free of runtime-only
// modules so Metro/React Native can import session, overlay, and transport
// code without trying to resolve Bun's `bun:sqlite`.
// ---------------------------------------------------------------------------

export {
  type ClientIdentity,
  type ConfirmedEntity,
  KhalaSyncClientStoreError,
  type KhalaSyncClientStoreErrorReason,
  type KhalaSyncLocalStore,
} from "./store.js"
export {
  createKhalaSyncStoreCore,
  KHALA_SYNC_LOCAL_STORE_SCHEMA_VERSION,
  KHALA_SYNC_STORE_SCHEMA,
  type KhalaSyncStoreCore,
  localStoreFromCore,
  type SqlDriver,
  type SqlValue,
  toKhalaSyncStoreError,
} from "./store-core.js"
// ---------------------------------------------------------------------------
// Optimistic mutators + rebase (KS-5.2): contracts + engine in overlay.ts.
// Optimistic effects live ONLY in the in-memory overlay (SPEC §7
// invariant 2); the durable store holds server-confirmed state exclusively.
// ---------------------------------------------------------------------------

export {
  type ClientMutator,
  createOverlay,
  type KhalaSyncOverlay,
  KhalaSyncOverlayError,
  type KhalaSyncOverlayErrorReason,
  type OverlayEffect,
  type OverlayEntity,
  type OverlayError,
  type OverlayReadView,
  type OverlayView,
} from "./overlay.js"

export {
  CHAT_APPEND_MESSAGE_MUTATOR_NAME,
  CHAT_CREATE_THREAD_MUTATOR_NAME,
  CHAT_RENAME_THREAD_MUTATOR_NAME,
  type ChatAppendMessageArgs,
  type ChatClientMutatorOptions,
  type ChatClientMutators,
  type ChatCreateThreadArgs,
  type ChatRenameThreadArgs,
  chatAppendMessageClientMutator,
  chatCreateThreadClientMutator,
  chatMessagesForTranscript,
  chatRenameThreadClientMutator,
  chatThreadsForSidebar,
  compareChatMessagesForTranscript,
  compareChatThreadsForSidebar,
  createChatClientMutators,
} from "./chat.js"

export {
  ConfirmedChatMessageSchema,
  ConfirmedChatThreadSchema,
  type ConfirmedChatMessage,
  type ConfirmedChatThread,
  createKhalaSyncConversation,
  type KhalaSyncConversation,
  type KhalaSyncConversationChange,
  type KhalaSyncConversationStatus,
  KhalaSyncConversationStatusSchema,
} from "./conversation.js"

export {
  type ConfirmedRuntimeAttentionSnapshot,
  createKhalaSyncAttentionInbox,
  type KhalaSyncAttentionInbox,
  MAX_CONFIRMED_RUNTIME_ATTENTION,
  type RuntimeAttentionProjectionIssue,
} from "./attention.js"

export {
  KhalaConversationLiveEnvelopeSchema,
  type KhalaConversationLiveEnvelope,
  type KhalaConversationLiveMetrics,
  type KhalaConversationLiveOptions,
  type KhalaConversationLiveSnapshot,
  KhalaConversationLiveSnapshotSchema,
  type KhalaConversationLiveSubscription,
  type KhalaConversationLiveUpdate,
  KhalaConversationLiveUpdateSchema,
  openKhalaConversationLive,
} from "./live-conversation.js"

export {
  ConfirmedAgentRunSchema,
  ConfirmedAgentTimelineEventSchema,
  ConfirmedAgentTimelineItemSchema,
  type ConfirmedAgentRun,
  type ConfirmedAgentTimelineEvent,
  type ConfirmedAgentTimelineItem,
  type ConfirmedAgentTimelineSnapshot,
  createKhalaSyncAgentTimeline,
  type KhalaSyncAgentTimeline,
  type KhalaSyncAgentTimelineStatus,
  MAX_CONFIRMED_AGENT_TIMELINE_EVENTS,
} from "./agent-timeline.js"

export {
  ConfirmedLiveAgentGraphsSchema,
  type ConfirmedLiveAgentGraphSnapshot,
  createKhalaSyncLiveAgentGraph,
  type KhalaSyncLiveAgentGraph,
  type KhalaSyncLiveAgentGraphStatus,
  MAX_CONFIRMED_LIVE_AGENT_GRAPHS,
  MAX_CONFIRMED_LIVE_AGENT_GRAPH_EDGES,
  MAX_CONFIRMED_LIVE_AGENT_GRAPH_NODES,
} from "./live-agent-graph.js"

export {
  newestLiveAgentGraph,
  projectLiveAgentGraphPresentation,
  resolveLiveAgentGraphSelection,
  type LiveAgentGraphAuthority,
  type LiveAgentGraphPresentation,
  type LiveAgentGraphPresentationRow,
  type LiveAgentGraphTokenAttribution,
  type LiveAgentGraphTokenTruth,
  type LiveAgentGraphTokenUsage,
  type LiveAgentGraphTone,
} from "./live-agent-graph-presentation.js"

export {
  CODING_PUBLISH_CATALOG_MUTATOR_NAME,
  createCodingCatalogPublishMutator,
  createKhalaSyncCodingCatalog,
  type CodingCatalogPublishChangeSet,
  type ConfirmedCodingCatalogSnapshot,
  type KhalaSyncCodingCatalog,
  type KhalaSyncCodingCatalogStatus,
  MAX_CONFIRMED_CODING_NAVIGATIONS,
  MAX_CONFIRMED_CODING_PROJECTS,
  MAX_CONFIRMED_CODING_REPOSITORIES,
  MAX_CONFIRMED_CODING_SESSIONS,
  MAX_CONFIRMED_CODING_WORKTREES,
} from "./coding-session.js"

export {
  createKhalaSyncPortableSessions,
  createPortableRequestCommandMutator,
  MAX_CONFIRMED_PORTABLE_ATTACHMENTS,
  MAX_CONFIRMED_PORTABLE_COMMANDS,
  MAX_CONFIRMED_PORTABLE_SESSIONS,
  PORTABLE_REQUEST_COMMAND_MUTATOR_NAME,
  type ConfirmedPortableSessionSnapshot,
  type KhalaSyncPortableSessions,
  type PortableProjectionIssue,
} from "./portable-session.js"

export {
  buildAppendUserMessageIntent,
  buildCloseTurnIntent,
  buildContinueTurnIntent,
  buildInterruptTurnIntent,
  buildRetryTurnIntent,
  buildStartTurnIntent,
  chatMessageBodyRef,
  createRuntimeClientMutators,
  createKhalaSyncRuntimeCommands,
  RUNTIME_APPEND_USER_MESSAGE_MUTATOR_NAME,
  RUNTIME_CLOSE_TURN_MUTATOR_NAME,
  RUNTIME_CONTINUE_TURN_MUTATOR_NAME,
  RUNTIME_INTERRUPT_TURN_MUTATOR_NAME,
  RUNTIME_RETRY_TURN_MUTATOR_NAME,
  RUNTIME_START_TURN_MUTATOR_NAME,
  type RuntimeClientMutators,
  type KhalaSyncRuntimeCommands,
  type RuntimeCommandContext,
  type RuntimeCommandOutcome,
  type RuntimeCommandOutcomeStatus,
  type RuntimeCommandSurface,
  type RuntimeCommandTarget,
} from "./runtime.js"

export {
  buildRuntimeInteractionDecisionCommand,
  confirmedRuntimeInteractions,
  createKhalaSyncRuntimeInteractions,
  createRuntimeInteractionClientMutator,
  RUNTIME_DECIDE_INTERACTION_MUTATOR_NAME,
  type ConfirmedRuntimeInteraction,
  type KhalaSyncRuntimeInteractions,
  type RuntimeInteractionDecisionCommand,
} from "./runtime-interactions.js"

export {
  CODING_COMPOSER_DRAFT_ENTITY_TYPE,
  createKhalaSyncCodingComposerDrafts,
  MAX_DEVICE_CODING_COMPOSER_DRAFT_BYTES,
  MAX_DEVICE_CODING_COMPOSER_DRAFTS,
  type CodingComposerDraftSaveOutcome,
  type KhalaSyncCodingComposerDrafts,
} from "./coding-composer-drafts.js"

// ---------------------------------------------------------------------------
// Transport (KS-5.3): injectable seam in transport.ts; HTTP+WebSocket
// implementation against the SPEC §3 routes, khala-sync codecs at every
// boundary, bearer auth from the session config's authToken().
// ---------------------------------------------------------------------------

export {
  createHttpKhalaSyncTransport,
  type HttpTransportConfig,
  type HttpTransportDeps,
  isAccessDeniedSignal,
  isRefetchSignal,
  isRetryableTransportError,
  KHALA_SYNC_BOOTSTRAP_PATH,
  KHALA_SYNC_CONNECT_PATH,
  KHALA_SYNC_LOG_PATH,
  KHALA_SYNC_PUSH_PATH,
  type KhalaSyncTransport,
  KhalaSyncTransportError,
  type KhalaSyncTransportErrorReason,
  type LiveSocket,
  type LiveSocketHandlers,
  type WebSocketLike,
} from "./transport.js"

// ---------------------------------------------------------------------------
// Sync session (KS-5.3): per-scope state machine in session.ts
// idle → bootstrapping → catching_up → live (+ must_refetch from any state);
// reconnect resumes from the DURABLE cursor; push loop drains the pending
// queue with in-band rejection handling.
// ---------------------------------------------------------------------------

export {
  computeBackoffMs,
  createKhalaSyncSession,
  type ConnectFailureSignal,
  type KhalaSyncSession,
  type KhalaSyncSessionConfig,
  type KhalaSyncSessionOptions,
  type ScopeSyncState,
} from "./session.js"
