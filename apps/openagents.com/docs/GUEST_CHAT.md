# Guest chat

Everyone uses the **homepage** (`/`) to chat. No redirect: guests and authenticated users both stay on the homepage and get the same chat UI. Guests never call `POST /api/chats` (no “fetch convos”); they get a session-based conversation id instead.

## Design

- **Homepage** (`/`): Renders the index Inertia page for everyone. No server-side redirect.
- **Guests**: Get a conversation id via `GET /api/chat/guest-session`. That id is stored in the session and reused for the lifetime of the session. No list or create of “their” conversations.
- **Authenticated users**: Create one conversation on load via `POST /api/chats` and use that id for the stream.
- **Stream** (`POST /api/chat`): Accepts both authenticated users and guests. For guests, the request must include the same `conversationId` that is in the session (and match the `guest-*` pattern).

## Session and guest identity

- **Session key**: `chat.guest.conversation_id` holds the current guest conversation id (e.g. `guest-01933abc-...`).
- **Id format**: `guest-` + UUIDv7. Validated with regex `/^guest-[a-z0-9-]+$/i`.
- **Stability**: One id per session. Same guest returning to the homepage or visiting `/chat` gets the same id until the session is lost.

## Shared guest user (backend)

All guest sessions are backed by a single **system guest user** in the `users` table so that existing persistence (threads, runs, messages) keeps using `user_id` and no schema change is required.

- **Email**: `guest@openagents.internal` (constant in `GuestChatSessionService`).
- **Creation**: `User::firstOrCreate(['email' => ...], ['name' => 'Guest', 'workos_id' => 'guest-system', 'avatar' => ''])`. Handle is set by `User::booted()`.
- **Usage**: When the stream runs for a guest, the controller resolves this user and passes it to `RunOrchestrator::streamAutopilotRun()` like any other user.

## API

### GET `/api/chat/guest-session` (no auth)

- **Purpose**: Return the session guest conversation id and ensure the conversation and thread exist for that id (so the stream can run).
- **Response**: `{ "conversationId": "guest-01933abc-..." }`.
- **Side effects**:
  - Sets or reuses `chat.guest.conversation_id` in the session.
  - Ensures `agent_conversations` has a row `(id = conversationId, user_id = guest user id, title = 'Chat')`.
  - Ensures `threads` has a row for the same id and guest user (so `RunOrchestrator::resolveThreadContext` finds it).
- **Route**: `routes/web.php`, name `api.chat.guest-session`.

### POST `/api/chat` (guest or auth)

- **Query**: `conversationId` (required).
- **Auth**:
  - If the request has an authenticated user: require a conversation in `agent_conversations` for that user and `conversationId` (unchanged behavior).
  - If there is no user: require `conversationId` to equal `session('chat.guest.conversation_id')` and to match the guest id pattern. Then resolve the guest user, call `ensureGuestConversationAndThread(conversationId)`, and run the stream with the guest user. Otherwise return 401.
- **Middleware**: Only `ValidateWorkOSSession`; not behind `auth`, so guests can call it with a valid session and matching guest id.

## Service: `GuestChatSessionService`

Location: `app/Services/GuestChatSessionService.php`.

| Method | Purpose |
|--------|--------|
| `ensureGuestConversationId(Request $request, ?string $requestedConversationId = null): string` | Returns existing session guest id, or accepts a valid `guest-*` from the request, or creates a new `guest-{uuid7}` and stores it in the session. |
| `isGuestConversationId(?string $value): bool` | True if the value is a non-empty string matching `guest-*` (UUID-style). |
| `guestUser(): User` | Returns the shared guest user (`firstOrCreate` by `guest@openagents.internal`). |
| `ensureGuestConversationAndThread(string $conversationId): void` | Ensures `agent_conversations` and `threads` each have a row for `(conversationId, guest user id)` so the stream and orchestrator can run. Idempotent. |

Used by:

- `GuestChatSessionController` (GET guest-session).
- `ChatApiController::stream` (guest branch).
- `ChatPageController::show` (when rendering the chat page for an unauthenticated user: `ensureGuestConversationId` only; no DB ensure there, but the stream will ensure when the guest first sends).

## Frontend: index page

File: `resources/js/pages/index.tsx`.

- **Auth detection**: `usePage<IndexPageProps>().props.auth?.user`. Absent or null ⇒ guest.
- **Guests**:
  1. On mount (once): `fetch('/api/chat/guest-session', { credentials: 'include' })`.
  2. Parse JSON, take `conversationId`, set local state.
  3. Use that id with `useChat` and `DefaultChatTransport` as `api`: `/api/chat?conversationId=...`.
- **Authenticated**:
  1. On mount (once): `POST /api/chats` with `{ title: 'Chat' }`.
  2. On 401/419 set “auth required” and show sign-in; on success set `conversationId` from `data.data.id`.
  3. Same `useChat` + transport with that id.
- **Loading**: “Loading…” until `conversationId` is set; then the same message list and input for both.

No redirect to `/chat`; no POST to `/api/chats` for guests.

## Chat page (`/chat`)

`ChatPageController::show` still serves `/chat/{conversationId?}`. For unauthenticated visitors it uses `GuestChatSessionService::ensureGuestConversationId($request, $conversationId)` so the same session guest id is used whether the guest lands on `/` or `/chat`. No separate guest id logic lives in the controller; the service is the single place for session guest id and DB ensure.

## Database

- **Guest user**: One row in `users` (email `guest@openagents.internal`). Created on first need.
- **Guest conversations**: Rows in `agent_conversations` and `threads` with `user_id` = guest user id and `id` = session guest conversation id. Created when:
  - The frontend calls GET `/api/chat/guest-session`, or
  - The stream is invoked for a guest and runs `ensureGuestConversationAndThread`.
- Runs, messages, and run_events use the same tables with that `user_id`; no nullable or special-case schema for guests.

## Tests

- `tests/Feature/GuestOnboardingChatTest.php`: Session guest id stability and guest onboarding props on `/chat`.
- `tests/Feature/ChatStreamingTest.php`: Stream behavior for authenticated users (conversationId required, 422/404 cases).

Guest stream (sending a message as guest from the homepage) is covered by the same stream path; add dedicated guest-stream tests if you want explicit coverage.
