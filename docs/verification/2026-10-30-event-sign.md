# NIP-01 event builder: `coax/lib/nostr/event_sign.coax` (2026-10-30)

Board: #bab4 `nostr-event-sign`. Psionic chapter 19 prerequisite 2.

## What it is

`nostr_event_sign_step(key, aux, created_at, kind, tags, content, work,
scratch, out, overlap)` builds and signs one NIP-01 event in resumable stages:
1. the key;
2. the id (preimage and SHA-256, through `event.coax`'s serializer);
3. the BIP-340 signature, over several calls to `schnorr_sign_step`;
4. the event JSON.

It returns `EVENT_MORE` (0) until it finishes. A positive result is the
event's length in `out`, and `EVENT_REFUSED` (-1) is a refusal. The JSON is
published only after the shared reader (`nostr_event_read`) recomputes its id
from those bytes. So what we sign is exactly what `event.coax` verifies.

Inputs are checked before the first stage:
- created_at from 0 to 2^53 - 1;
- kind from 0 to 65535;
- tags must be a JSON array of arrays of strings.

Views must be stable and disjoint. A refusal of the views or inputs changes
nothing. Any later refusal clears the workspace, and the last stage's refusal
also clears `out`.

## Evidence

- **Checks:** 290 of 290 pass (`coax test coax/lib/nostr/event_sign.coax
  --run-steps 250000000`). That includes:
  - one event signed independently in pure Python
    (`coax/tests/nostr/event_sign_vectors.py`) with BIP-340 vector 1's key and
    aux, and escapes in its tags and content, matched byte for byte;
  - refusal of each bad input;
  - workspace clearing.
- **Mutation:** 5,194 mutants, 5,194 killed, 0 survived, none marked equivalent. This count includes the signer's own mutants,
  because `coax mutate` mutates every `#authority` file a unit loads. It ran
  as 100 shards on the Mac with the compiler built from `00fb372858`.
- **Native:** `coax/tests/nostr/schnorr_sign.py` drives both the signer and
  this file through the generated C. It runs checked and release builds,
  plain and ASan/UBSan, 44 requests each, and all four pass.
  - 4 events: unicode and control-character escapes, an empty event, the
    largest created_at and kind, and 5,000 bytes of content. Each is
    re-serialized, re-hashed and verified by an independent Python verifier.
  - 5 refused inputs: created_at -1 and 2^53, kind 65536, a non-string tag
    value, and malformed tags JSON.

## Not done

- **Wasm and iOS agreement:** not run.
- **Real keys:** test keys only until the signer's open gates close
  (`2026-10-30-schnorr-sign.md`: #89db, #4828, review), plus the owner's
  approval.
