# The OpenAgents Protocol: review and plan

Date: 2026-11-11. Owner request: review the pre-Nuke OpenAgents Protocol,
decide whether to bring it back, sync it with the OpenAgents book, and decide
whether every part of the codebase (chat first) should use its schemas.

Follow-up: the [compliance plan](2026-11-11-compliance-plan.md) checks this
review against the code, corrects it (the Everglade profiles pin a hand-packed
codec, and Coax does not generate JSON mappers yet), and orders the work for
Coder and Everglade.

## Summary

The protocol already came back. The spec
([openagents-protocol-spec.md](openagents-protocol-spec.md), 4,873 lines) and
the schema tree ([proto/](../../proto/README.md)) were recovered after the Nuke
from `853417f290`, and Coax can import proto3 schemas (`#import_proto`) with
bounded codecs. What didn't come back is its use:

- **The book never mentions it.** No chapter of the OpenAgents book links
  `proto/` or the spec. Chapter 46 says the protocol is "signed JSON over
  WebSockets" (Nostr), and chapter 48 describes "one API", without saying
  where either one's shapes are defined.
- **The product mostly bypasses it.** 41 Coax files import protos, almost all
  in the Nostr library, the compiler and its book. Chat
  (`coax/web/api/chat.c`, `apps/openagents-web/chat.coax`), WorldBuilder, the
  room servers and most of the API invent their own JSON. One API file uses
  a generated codec.
- **The spec still describes the old world** in places: Loro rooms, CTRL and
  SESS revisions, a 2,368-ID schema inventory from the deleted apps, and
  inventories labelled "Implemented" for code that no longer exists.

Recommendation: **keep it, make it the one place every wire shape is defined,
and wind it through the codebase one consumer at a time, starting with what we
are building now** (auth, sync, WorldBuilder edits) and then chat. No
big-bang cutover, which the spec's own section 36 forbids for good reason.

## How the book and the spec fit

They agree once each says what it covers:

- **The book says why and how it feels**: everything an agent does is a signed
  event anyone can read (ch. 46–47), and there is one API our own apps use
  (ch. 48).
- **The spec says what the bytes are**: each record is one protobuf message,
  carried three ways (spec section 8): as a signed Nostr event's JSON (made by
  a generated mapper, never by hand), as HTTP JSON, or as binary frames.

So chapter 46's "signed JSON over WebSockets" is one of the three carriages,
and its JSON comes from the same schema as the API's. Each book chapter that
publishes or sends something should name the proto package that defines it in
its "What we build" section, and the spec should link back to the chapter.

## What to change in the spec

1. **Separate what exists from history.** Move the pre-Nuke inventories
   (sections 2 and 32's tables, the 2,368 schema IDs, "Implemented" labels for
   deleted crates) into an appendix titled as history. The main text describes
   only `proto/` as it is and the plan.
2. **Fold in today's decisions:**
   - sync is server-authoritative rows, documents, logs and blobs, not Loro
     ([docs/sync/](../sync/README.md)); retire the Loro-era CTRL and SESS
     wording;
   - our own auth ([docs/auth/](../auth/README.md)): email and Nostr, no phone
     numbers, sessions, agent keys;
   - Bitcoin-only payment rails, already closed in `Amount` and `Rail`;
   - Google-first model calls behind the Decisions API.
3. **Answer section 39's open questions** (recommendations below), so they
   stop blocking.

## What to wind through the codebase, in order

| Step | Consumer | Package |
| --- | --- | --- |
| 1 | The HTTP envelope every API route answers with: errors, pages, idempotency | `openagents.api.v1` (spec section 10) |
| 2 | Auth: sessions, devices, agent keys, the audit log | `openagents.identity.v1` and `openagents.account.v1` (spec section 15), starting as `v1alpha1` |
| 3 | Sync frames and records | `openagents.sync.v1alpha1`, new ([docs/sync/](../sync/README.md)) |
| 4 | WorldBuilder's and Aethyrforge's edit operations | `openagents.verse` / `engine` world edits, sharing `aethyr/forge/edit.coax`'s operations |
| 5 | **Chat**: conversations, turns, runner events, the session record | `openagents.chat.v1` and `openagents.session.v1` (spec section 17); chat already writes `openagents.session.v1` JSON lines, which become its first generated mapper |
| 6 | The room servers' messages | `openagents.verse` session and presence; the Everglade wire profiles already use generated codecs and are the pattern |
| 7 | Lightning Addresses, payments and receipts | `openagents.payment.v1` (spec section 26), NIP-57 zaps through the Nostr mirrors |
| 8 | Everything else, as each surface is next touched | by the rule below |

Each step: write the schema in `proto/`, generate the Coax codec, move the
producer and every consumer onto it, keep the JSON the same bytes where
anything signed or stored depends on it (a golden test), and delete the
hand-written encoder.

## The rule going forward

Added to [docs/reuse.md](../reuse.md): a new message between apps, the API,
rooms or devices gets a schema in `proto/` first. A page or route never
invents a JSON shape. A check can enforce part of it: a new `coax/web/api/`
route must name the proto message it answers with.

## Section 39, answered

| Question | Recommendation |
| --- | --- |
| Buf Schema Registry | Not now. Publish with GitHub releases; revisit when an outside developer asks |
| `openagents.*` or `com.openagents.*` | Keep `openagents.*`; it is frozen in `engine.v1` |
| Sales records | Evidence, as the spec says; no sales package |
| Forum | Nostr-native (NIP-22); `forum.v1` is a view package |
| CTRL folding into HOST | Yes, when sessions move to the sync spec's logs; retire NIP-CTRL then |
