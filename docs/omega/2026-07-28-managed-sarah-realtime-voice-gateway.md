# Managed Sarah Realtime voice gateway

Date: 2026-07-28

Status: Implemented and off by default

Issues:
[OpenAgentsInc/openagents#9272](https://github.com/OpenAgentsInc/openagents/issues/9272)
and
[OpenAgentsInc/openagents#9273](https://github.com/OpenAgentsInc/openagents/issues/9273)

## Purpose

This service gives an approved Omega or OpenAgents mobile client a live Sarah
voice session.
OpenAgents owns the provider connection.
The client does not receive an OpenAI API key.

The service uses `gpt-realtime-2.1`.
The service sends a stable hash of the OpenAgents user ID as the OpenAI safety identifier.

## Service flow

1. The approved client sends an authenticated session request.
2. The API checks the user, device, feature configuration, and available credit.
3. The API holds a bounded credit amount.
4. The API returns a one-use gateway ticket.
5. The client opens the gateway WebSocket with the ticket.
6. The gateway consumes the ticket and opens the OpenAI WebSocket.
7. The gateway relays only approved audio and control frames.
8. The gateway records each provider usage event.
9. The gateway debits the recorded charge and releases the full hold.

The database permits one active Sarah voice session for each user.
The default maximum session time is 600 seconds.
The configured maximum cannot be more than 900 seconds.

## Session request

Use this path:

```text
POST /api/omega/sarah/voice/session
```

Use the normal OpenAgents bearer token.
Also send this header:

```text
x-openagents-omega-device-ref: <deviceRef>
```

The request uses `openagents.sarah.voice.v1`.
The `identity.ownerRef` value must be the authenticated user ID.
The header device value must equal `identity.deviceRef`.

Example request:

```json
{
  "schema": "openagents.sarah.voice.v1",
  "identity": {
    "ownerRef": "user_123",
    "deviceRef": "omega_install_123",
    "threadRef": "thread_123",
    "sessionRef": "voice_123",
    "generation": 1
  },
  "disclosureRef": "omega.voice.disclosure.v1",
  "clientProfile": "omega_editor"
}
```

The success response contains the gateway URL, one-use ticket, expiry values, held credit, model, and fixed audio formats.
It also contains the selected `clientProfile`.
The API returns `402` when available credit is too low.
The API returns `409` when the user has an active session.
The response always has `Cache-Control: no-store`.

## Automatic Nostr authentication

Omega and OpenAgents mobile can use their current local Nostr signer.
The client does not create a second identity for this flow.
This flow uses NIP-98.
It does not use NIP-42.

First, the client requests a challenge:

```text
POST /api/omega/sarah/voice/auth/challenge
```

This request is unsigned.
It does not contain a bearer token or a NIP-98 header.
The request has this form:

```json
{
  "schema": "openagents.sarah.voice.auth-challenge.v1",
  "deviceRef": "omega_install_123",
  "pubkey": "64 lowercase hexadecimal characters"
}
```

The server canonicalizes the public key to lowercase.
The server applies the current Omega account rules.
The challenge expires after 120 seconds.
The server limits challenge requests by client address and across the service.

The success status is `201`.
The response has `Cache-Control: no-store`.
The `expiresAtMs` value is Unix epoch time in milliseconds.
The response has this form:

```json
{
  "schema": "openagents.sarah.voice.auth-challenge.v1",
  "challenge": "a random base64url value",
  "expiresAtMs": 1785270000000,
  "ownerRef": "the canonical OpenAgents user ID"
}
```

The client must copy the returned `ownerRef`.
The client must not calculate or guess this value.
This rule supports an owner key that maps to a current account.

Next, the client adds this object to the normal voice session request:

```json
{
  "auth": {
    "method": "nostr_nip98",
    "challenge": "the challenge from the server"
  }
}
```

The client sends these headers:

```text
Authorization: Nostr <base64 encoded NIP-98 event>
x-openagents-omega-device-ref: <deviceRef>
Content-Type: application/json
```

The client serializes the complete request body one time.
It gives the same bytes to the signer and to the HTTP client.
The NIP-98 event has kind `27235` and empty content.
It has exactly one `u` tag, one `method` tag, and one `payload` tag.

The `u` value is the exact session URL.
The `method` value is `POST`.
The `payload` value is the SHA-256 hash of the exact request bytes.
The event time must be within 60 seconds of the server time.

The server verifies the event ID and signature.
It also verifies the URL, method, payload, time, public key, owner, device,
and challenge.
The server atomically marks each event ID and each challenge as consumed.
A replay returns `409`.

The success status is `201`.
The response has `Cache-Control: no-store`.
It includes this additional object:

```json
{
  "auth": {
    "method": "nostr_nip98",
    "accessToken": "oa_omega_<random value>",
    "expiresIn": 900
  }
}
```

The `expiresIn` value is in seconds.
The client stores this token in its current secure session store.
The current bearer session flow stays available.
It does not use a challenge.

## WebSocket connection

Open the `gatewayUrl` from the session response.
Send these headers:

```text
x-openagents-sarah-voice-session: <sessionRef>
x-openagents-sarah-voice-ticket: <ticket>
```

Do not put the ticket in a URL.
The ticket expires after 60 seconds or less.
The server consumes the ticket before it attempts the upgrade.
A failed upgrade cannot reuse the ticket.

The first client control frame must be `session_hello`.
Its disclosure reference must equal the session request value.
Control sequences and audio sequences each start at `0`.

Client audio uses the `OAA1` media envelope.
It must contain mono `pcm_s16le` audio at 24 kHz.
The server checks the identity, sequence, payload size, and SHA-256 digest.

The full client and server schemas are in:

```text
packages/audio-contract/src/sarah-realtime.ts
```

## Client profiles

The pre-release version 1 schema has two client profiles:

- `omega_editor`
- `mobile_voice_only`

An omitted profile selects `omega_editor`.
The server stores the selected profile with the session.
The server also returns the profile in the session response.
A client must stop if the response does not contain its requested profile.

The `mobile_voice_only` profile has no tools.
The provider receives `tools: []` and `tool_choice: "none"`.
The instructions prohibit editor, file, URL, shell, Git, payment, account, and
device actions.
The gateway closes the session if the provider sends a tool call.

This profile is a voice safety profile.
It is not an unmetered entitlement.
Mobile voice continues to use normal OpenAgents credit.

## Omega editor tools

The `omega_editor` profile gives the model this allowlist:

- `context_read`
- `open_path`
- `reveal_range`
- `replace_selection`
- `save_document`
- `start_agent_thread`

The server decodes each command with the shared Effect Schema.
The server rejects all other tool names.
The server rejects absolute paths, parent path traversal, and large line ranges.
Version 1 does not permit shell, Git, cloud, network, delete, rename, move, payment, or account commands.

`replace_selection` and `save_document` need user confirmation.
`start_agent_thread` also needs user confirmation.
It contains only a message and a `foreground` or `background` presentation.
The message must be from 1 through 16,384 UTF-8 bytes.
It cannot contain an agent field or a model field.
The server sends a proposal with an expiry and a SHA-256 digest.

The digest binds the user, device, session, generation, proposal, complete
command, and expiry.
Omega must return the exact proposal reference and digest.
Transcript text cannot confirm a command.

Omega sends a bounded `tool_outcome` after local execution.
The gateway sends only that bounded outcome to the model.

## Credit control

The session route increases `agent_balances.held_msat` in the reservation transaction.
The transaction fails if available credit is less than the configured hold.
A database constraint keeps `balance_msat` greater than or equal to `held_msat`.

The gateway records Realtime response usage and input transcription usage.
The charge uses exact provider token counts and the configured OpenAgents credit rate.
The rate is an OpenAgents credit rate.
It is not a provider price statement.

Each Realtime response ID is an idempotency key for response usage.
The transcription key contains the item ID and content index.
The stored session charge cannot be more than the held amount.
The gateway stops the session when the charge reaches the hold.

Settlement releases the full hold and debits the recorded charge in one transaction.
For a nonzero charge, the transaction writes a paid `pay_ins` adjustment and its balance leg.
Settlement is idempotent for one session.

## Cleanup

Normal close, provider close, and session expiry start settlement.
Usage writes run in order before settlement.
Socket close callbacks add settlement to the Cloud Run task drain.

The minute scheduler finds expired active sessions.
It settles them after a process stop or a lost socket close event.

## Configuration

The service is off unless this value is true:

```text
SARAH_REALTIME_VOICE_ENABLED
```

Set these positive whole-number values before you enable the service:

```text
SARAH_REALTIME_RESERVATION_MSAT
SARAH_REALTIME_CREDIT_MSAT_PER_MILLION_TOKENS
```

This optional value sets the session time:

```text
SARAH_REALTIME_MAX_SESSION_SECONDS
```

The permitted range is 60 through 900 seconds.
The default is 600 seconds.

The Cloud Run deployment already mounts `OPENAI_API_KEY` from Secret Manager.
Do not put that key in an environment file, response, log, trace, or client package.
Apply database migrations `0103_sarah_realtime_voice.sql` and
`0104_sarah_voice_client_profile.sql` before you enable the feature.

## Logs and private data

Sarah gateway logs can contain event names, counts, and bounded error messages.
They do not contain audio, transcripts, provider frames, tickets, API keys, editor text, or tool results.
Session rows contain account, device, thread, session, state, credit, time, close, and settlement data.
The ticket digest remains in the row until connection or settlement.
Usage rows contain token totals, a response reference, a charge, and a time.
The database does not store audio or transcript text.

## Verification

Run these commands:

```sh
pnpm --filter @openagentsinc/audio-contract test
pnpm --filter @openagentsinc/audio-contract typecheck
pnpm --filter @openagentsinc/khala-sync-server test -- sarah-realtime-voice-store.test.ts
pnpm --filter @openagentsinc/khala-sync-server typecheck
pnpm --dir apps/openagents.com/workers/api test -- src/sarah-realtime-voice-routes.test.ts src/cloudrun/sarah-realtime-bridge.test.ts
pnpm --dir apps/openagents.com/workers/api typecheck:cloudrun
pnpm --dir apps/openagents-mobile run test
pnpm --dir apps/openagents-mobile run typecheck
pnpm run check:ste
```
