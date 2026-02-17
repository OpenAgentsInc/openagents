# Guest chat

Everyone uses the homepage (`/`) to chat. Guests and authenticated users share the same UI and stream path.

The important rule is that conversation IDs must fit the schema (`agent_conversations.id` and `threads.id` are `varchar(36)`).

## Current behavior

- Homepage (`/`) renders `resources/js/pages/index.tsx` for everyone.
- Guests get a local conversation id immediately in the browser (`g-` + 32 hex chars, 34 chars total).
- The frontend then calls `GET /api/chat/guest-session?conversationId=<id>` to bind that id to the session and pre-create DB rows.
- Input focus is **not blocked** while guest-session preflight runs.
- If the preflight request is slow/fails, `POST /api/chat` can still establish the guest session on first send.

## Why this changed

A previous format used `guest-` + UUIDv7, which exceeds 36 chars and causes Postgres errors on insert:

- `SQLSTATE[22001]: value too long for type character varying(36)`

The current format is intentionally bounded and validated.

## ID format and validation

Guest conversation IDs are now validated as:

- regex: `^g-[a-f0-9]{32}$`
- total length: 34 chars

This is enforced in `GuestChatSessionService::isGuestConversationId()`.

## API surface

### GET `/api/chat/guest-session`

- no auth required
- optional query: `conversationId`
- binds/returns a valid session guest conversation id
- ensures rows exist in:
  - `agent_conversations`
  - `threads`

Response:

```json
{ "conversationId": "g-0123456789abcdef0123456789abcdef" }
```

### POST `/api/chat?conversationId=...`

- accepts both auth and guest
- guest rules:
  - `conversationId` must match guest format
  - if session id is not established yet, backend will establish it from this first request
  - if session already has a different guest id, request is rejected (401)

## Backend components

- `app/Services/GuestChatSessionService.php`
  - `ensureGuestConversationId()`
  - `isGuestConversationId()`
  - `ensureGuestConversationAndThread()`
  - `guestUser()`
- `app/Http/Controllers/GuestChatSessionController.php`
- `app/Http/Controllers/ChatApiController.php` (guest branch)

## Frontend component

- `resources/js/pages/index.tsx`
  - creates guest id immediately
  - preflights `GET /api/chat/guest-session` in background
  - uses same `useChat`/stream path as authenticated chat

## Tests

- `tests/Feature/GuestChatSessionApiTest.php`
- `tests/Feature/GuestOnboardingChatTest.php`
- `tests/Feature/DashboardTest.php`
- `tests/Feature/ChatStreamingTest.php` (guest stream establishment + mismatch rejection)
