# NIP-01 relay messages: `coax/lib/nostr/message.coax` (2026-10-30)

Board: #6015 `nostr-nip01-messages`. Psionic chapter 24 prerequisite 4.

## What it is

`nostr_message_read(text)` reads the five relay-to-client frames:
- `EVENT`: the subscription and the original span of the event object;
- `OK`: the id, the verdict and the machine prefix;
- `EOSE`: the subscription, with up to 4 trailing metadata values from the
  deployed relay;
- `CLOSED`: the subscription and a message with a prefix;
- `NOTICE`.

A label it doesn't know (`AUTH`, for one) refuses as `NM_TYPE`, and the host
routes it to `nip42_input`. Prefixes are the eight NIP-01 words
(`NM_PREFIXES`) or another single word, and an `OK false` or `CLOSED` without
one refuses.

Frames are bounded at `NM_MAX_FRAME` (270,336 bytes). Subscription ids must
be 1 to 64 characters of valid UTF-8 (`nm_sub_ok`).

The client frames `EVENT`, `REQ` (1 to 16 filters through `filter.coax`) and
`CLOSE` are written by `nostr_message_write_event`/`_req`/`_close`.

A parsed `EVENT` is not a verified event.
`nostr_message_event_verify_vartime` verifies the event it carries with the
shared verifier, and even that grants no author, subscription or freshness
authority.

## Evidence

- **Checks:** 214 of 214 pass, including the suites `message.coax` loads.
  They include the first frames of the captured relay.openagents.com
  transcript, every prefix and the frame bounds.
- **Native:** `coax run coax/tests/nostr/message_native.coax` reads all 243
  frames of the transcript. All 237 events verify, the 5 EOSEs read, and a
  changed signature byte is refused.
- **Mutation:** 4,162 mutants, 4,162 killed, 0 survived, none marked
  equivalent. It ran as 60 shards with `--jobs 1`, merged. Earlier runs left
  survivors. Unobservable checks were rewritten: `n<0` became `n<1`,
  `n!=64` and `== -1`; `nm_prefix` checks word characters before the colon;
  the size bound uses `event.coax`'s shared `ne_io`; and a decode branch that
  `json_valid`'s string bound makes unreachable was removed. The rest got
  sharper tests.
- **Build used:** the same snapshot as `2026-10-30-nostr-tags-metadata.md`.
  Its shared crypto predates `7ca28e856e` (#4828, `mul_wrap` in `bytes.coax`
  and `secp_scalar.coax`). Every other loaded unit is byte-identical to main.

## Not done

- **Wasm and iOS runs.**
- **Consumers:** the project page still reads frames in
  `apps/openagents-web/project-fold.coax`. `relay_read.coax` (relay.coax's
  input) and the Coax relay's `message_server.coax` constants are the next
  consumers.
