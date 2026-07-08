/**
 * App Store reviewer demo mode — hardcoded, public-safe example data.
 *
 * Owner requirement (2026-07-07): "a way for iOS store reviewers to login with
 * a demo account — long press on the login-with-GitHub button to log in with
 * the demo account, and they see hardcoded example data."
 *
 * Everything here is DELIBERATELY GENERIC. There are no real user accounts, no
 * real repositories, no real balances, and no real tokens. The demo session
 * token is an obviously-fake sentinel string; it is never accepted by any
 * server. Demo mode is fully self-contained/offline: every product data source
 * short-circuits to these fixtures when the active auth session is the demo
 * session, so a reviewer never sees a loading, error, or unauthorized state and
 * can freely navigate every screen.
 *
 * The gate is at the DATA-SOURCE layer (the sync scope-entities hook and the
 * mobile credits/repos/model-preference API clients), not inside each screen,
 * so screens render the same components against fixtures they would against
 * live data.
 */

import {
  CHAT_MESSAGE_ENTITY_TYPE,
  CHAT_THREAD_ENTITY_TYPE,
  decodeChatMessageEntity,
  decodeChatThreadEntity,
  type ChatMessageEntity,
  type ChatThreadEntity,
} from "@openagentsinc/khala-sync"

import type { KhalaStoredCredentials } from "../auth/khala-auth-store"

// NOTE: this module intentionally does NOT import the credits/repos/model-
// preference API modules (not even their types). Those modules import this one
// for the demo gate, so importing them back — even `import type` — creates a
// dependency cycle the architecture check rejects. Instead the demo data is
// typed with the local structural aliases below, and each API module's gate
// assigns it into that module's own Result type, which acts as a compile-time
// conformance check that the two shapes stay identical.

/** Structural mirror of `KhalaMobileCreditsTransaction` (see the note above). */
export type DemoCreditsTransaction = Readonly<{
  amountUsdCents: number
  description: string
  id: string
  kind: "grant" | "purchase" | "charge" | "other"
  occurredAt: string
}>

/** Structural mirror of `KhalaMobileRepository`. */
export type DemoRepository = Readonly<{
  defaultBranch: string
  description: string | null
  fullName: string
  htmlUrl: string
  id: string
  name: string
  owner: string
  private: boolean
  provider: "github"
}>

/** Structural mirror of `KhalaModelPreference`. */
export type DemoModelPreference = Readonly<{
  availableModelIds: ReadonlyArray<string>
  effectiveModelId: string | null
  fallback: "none" | "no_preference_set" | "preference_unavailable" | "default_unavailable"
  preferredModelId: string | null
  updatedAt: string | null
  usedPreference: boolean
}>

/** Obviously-fake sentinel that identifies the reviewer demo session. It is
 * never a real bearer token and is never sent to (or accepted by) any server —
 * the data-source gates below intercept every request that carries it. */
export const DEMO_REVIEWER_TOKEN = "demo-reviewer-session.example.v1"

/** Generic demo identity. Not a real user id. */
export const DEMO_REVIEWER_OWNER_USER_ID = "demo-reviewer"
export const DEMO_REVIEWER_GITHUB_LOGIN = "demo-user"

/** Synthetic in-app credentials established when a reviewer enters demo mode. */
export const DEMO_REVIEWER_CREDENTIALS: KhalaStoredCredentials = {
  githubLogin: DEMO_REVIEWER_GITHUB_LOGIN,
  ownerUserId: DEMO_REVIEWER_OWNER_USER_ID,
  token: DEMO_REVIEWER_TOKEN,
}

/** True when a token is the reviewer demo-session sentinel. Every data source
 * checks this to decide whether to serve fixtures instead of hitting a live
 * backend. */
export const isDemoToken = (token: string): boolean => token === DEMO_REVIEWER_TOKEN

// --- Chat threads + messages (Khala Sync scopes) ---------------------------

const DEMO_THREAD_WEB_APP = "demo-thread-example-web-app"
const DEMO_THREAD_API = "demo-thread-example-api"
const DEMO_THREAD_TESTS = "demo-thread-example-tests"

/** Example threads with realistic coding-assistant titles. */
export const demoChatThreads: ReadonlyArray<ChatThreadEntity> = [
  {
    createdAt: "2026-07-06T14:02:00Z",
    lastMessageAt: "2026-07-06T14:07:30Z",
    messageCount: 4,
    ownerUserId: DEMO_REVIEWER_OWNER_USER_ID,
    repoBinding: { defaultBranch: "main", name: "example-web-app", owner: "demo-user" },
    status: "active",
    threadId: DEMO_THREAD_WEB_APP,
    title: "Add a dark-mode toggle to the settings page",
    updatedAt: "2026-07-06T14:07:30Z",
  },
  {
    createdAt: "2026-07-05T09:30:00Z",
    lastMessageAt: "2026-07-05T09:41:12Z",
    messageCount: 2,
    ownerUserId: DEMO_REVIEWER_OWNER_USER_ID,
    repoBinding: { defaultBranch: "main", name: "example-api", owner: "demo-user" },
    status: "active",
    threadId: DEMO_THREAD_API,
    title: "Add pagination to the /orders endpoint",
    updatedAt: "2026-07-05T09:41:12Z",
  },
  {
    createdAt: "2026-07-04T18:15:00Z",
    lastMessageAt: "2026-07-04T18:20:44Z",
    messageCount: 2,
    ownerUserId: DEMO_REVIEWER_OWNER_USER_ID,
    repoBinding: null,
    status: "active",
    threadId: DEMO_THREAD_TESTS,
    title: "Write unit tests for the date formatter",
    updatedAt: "2026-07-04T18:20:44Z",
  },
].map(thread => decodeChatThreadEntity(thread))

const demoMessage = (input: {
  body: string
  createdAt: string
  isAgent: boolean
  messageId: string
  threadId: string
}): ChatMessageEntity =>
  decodeChatMessageEntity({
    authorUserId: input.isAgent ? "khala-agent" : DEMO_REVIEWER_OWNER_USER_ID,
    body: input.body,
    createdAt: input.createdAt,
    deletedAt: null,
    messageId: input.messageId,
    threadId: input.threadId,
    updatedAt: input.createdAt,
  })

/** Example conversations keyed by threadId so the chat screen looks real. */
export const demoChatMessagesByThread: Readonly<Record<string, ReadonlyArray<ChatMessageEntity>>> = {
  [DEMO_THREAD_WEB_APP]: [
    demoMessage({
      body: "Add a dark-mode toggle to the settings page and persist the choice.",
      createdAt: "2026-07-06T14:02:00Z",
      isAgent: false,
      messageId: "demo-msg-web-1",
      threadId: DEMO_THREAD_WEB_APP,
    }),
    demoMessage({
      body:
        "On it. Here is the plan:\n1. Add a `theme` value ('light' | 'dark') to the settings store.\n2. Add a Switch to SettingsScreen bound to that value.\n3. Read the stored theme on launch and apply it to the root provider.\nStarting with the store change now.",
      createdAt: "2026-07-06T14:03:10Z",
      isAgent: true,
      messageId: "demo-msg-web-2",
      threadId: DEMO_THREAD_WEB_APP,
    }),
    demoMessage({
      body: "Sounds good — make sure it defaults to the system setting the first time.",
      createdAt: "2026-07-06T14:05:40Z",
      isAgent: false,
      messageId: "demo-msg-web-3",
      threadId: DEMO_THREAD_WEB_APP,
    }),
    demoMessage({
      body:
        "Done. The toggle now defaults to the OS appearance on first launch and remembers the user's choice after that. I opened a pull request with the change and added a test that covers the persistence path.",
      createdAt: "2026-07-06T14:07:30Z",
      isAgent: true,
      messageId: "demo-msg-web-4",
      threadId: DEMO_THREAD_WEB_APP,
    }),
  ],
  [DEMO_THREAD_API]: [
    demoMessage({
      body: "Add cursor-based pagination to the GET /orders endpoint.",
      createdAt: "2026-07-05T09:30:00Z",
      isAgent: false,
      messageId: "demo-msg-api-1",
      threadId: DEMO_THREAD_API,
    }),
    demoMessage({
      body:
        "I added a `limit` and `cursor` query parameter and returned a `nextCursor` in the response. Existing callers that pass no parameters still get the first page, so the change is backward compatible.",
      createdAt: "2026-07-05T09:41:12Z",
      isAgent: true,
      messageId: "demo-msg-api-2",
      threadId: DEMO_THREAD_API,
    }),
  ],
  [DEMO_THREAD_TESTS]: [
    demoMessage({
      body: "Write unit tests for the date formatter, including timezone edge cases.",
      createdAt: "2026-07-04T18:15:00Z",
      isAgent: false,
      messageId: "demo-msg-tests-1",
      threadId: DEMO_THREAD_TESTS,
    }),
    demoMessage({
      body:
        "Added 8 test cases covering midnight boundaries, leap days, and a non-UTC timezone. All green locally.",
      createdAt: "2026-07-04T18:20:44Z",
      isAgent: true,
      messageId: "demo-msg-tests-2",
      threadId: DEMO_THREAD_TESTS,
    }),
  ],
}

const threadIdFromScope = (scope: string): string | null => {
  const prefix = "scope.thread."
  return scope.startsWith(prefix) ? scope.slice(prefix.length) : null
}

const isPersonalScope = (scope: string): boolean => scope.startsWith("scope.user.")

/**
 * Demo-mode replacement for a Khala Sync scope read. Returns the fixture
 * entities for a given entity type + scope, so the same
 * `useKhalaSyncScopeEntities` hook the thread list and thread view use renders
 * example data with no runtime, session, or network. Unknown scopes/entity
 * types return an empty list (a valid, non-error "ready" state).
 */
export const demoSyncScopeEntities = (entityType: string, scope: string): ReadonlyArray<unknown> => {
  if (entityType === CHAT_THREAD_ENTITY_TYPE && isPersonalScope(scope)) {
    return demoChatThreads
  }
  const threadId = threadIdFromScope(scope)
  if (threadId !== null) {
    if (entityType === CHAT_MESSAGE_ENTITY_TYPE) {
      return demoChatMessagesByThread[threadId] ?? []
    }
    if (entityType === CHAT_THREAD_ENTITY_TYPE) {
      const thread = demoChatThreads.find(candidate => candidate.threadId === threadId)
      return thread === undefined ? [] : [thread]
    }
    // Runtime events/turns: demo threads render as plain chat transcripts, so
    // there is no rich runtime transcript to show.
    return []
  }
  return []
}

// --- Credits ---------------------------------------------------------------

/** Example balance: $10.00. */
export const DEMO_CREDITS_BALANCE_USD_CENTS = 1_000

export const demoCreditsTransactions: ReadonlyArray<DemoCreditsTransaction> = [
  {
    amountUsdCents: 2_000,
    description: "Welcome credit",
    id: "demo-txn-1",
    kind: "grant",
    occurredAt: "2026-07-01T12:00:00Z",
  },
  {
    amountUsdCents: -640,
    description: "Coding session — example-web-app",
    id: "demo-txn-2",
    kind: "charge",
    occurredAt: "2026-07-06T14:08:00Z",
  },
  {
    amountUsdCents: -360,
    description: "Coding session — example-api",
    id: "demo-txn-3",
    kind: "charge",
    occurredAt: "2026-07-05T09:42:00Z",
  },
]

// --- Repositories ----------------------------------------------------------

export const demoRepositories: ReadonlyArray<DemoRepository> = [
  {
    defaultBranch: "main",
    description: "Example customer-facing web application.",
    fullName: "demo-user/example-web-app",
    htmlUrl: "https://example.com/demo-user/example-web-app",
    id: "demo-repo-1",
    name: "example-web-app",
    owner: "demo-user",
    private: false,
    provider: "github",
  },
  {
    defaultBranch: "main",
    description: "Example internal REST API service.",
    fullName: "demo-user/example-api",
    htmlUrl: "https://example.com/demo-user/example-api",
    id: "demo-repo-2",
    name: "example-api",
    owner: "demo-user",
    private: true,
    provider: "github",
  },
  {
    defaultBranch: "main",
    description: "Shared TypeScript utilities used across example projects.",
    fullName: "demo-user/example-shared-utils",
    htmlUrl: "https://example.com/demo-user/example-shared-utils",
    id: "demo-repo-3",
    name: "example-shared-utils",
    owner: "demo-user",
    private: true,
    provider: "github",
  },
]

// --- Model preference ------------------------------------------------------

/** Single Khala model (there are no variants). */
export const DEMO_MODEL_ID = "openagents/khala"

export const demoModelPreference = (preferredModelId: string = DEMO_MODEL_ID): DemoModelPreference => ({
  availableModelIds: [DEMO_MODEL_ID],
  effectiveModelId: DEMO_MODEL_ID,
  fallback: "none",
  preferredModelId,
  updatedAt: "2026-07-06T14:00:00Z",
  usedPreference: true,
})
