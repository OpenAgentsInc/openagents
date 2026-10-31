# Relay frames into the relay client: `coax/lib/nostr/relay_read.coax` (2026-10-30)

Psionic chapter 24 prerequisite 8: wiring `message.coax` into the relay
client. This replaces the test-only frame adapter in
`coax/tests/net/relay_e2e_frames.coax`.

## What it is

`rl_read(text, scratch, overlap) -> RlIn` turns one relay text frame into
`relay.coax`'s input:
- `message.coax` reads the frame.
- For an `EVENT`, `nostr_event_read` recomputes the id from the original
  bytes and `nostr_fast_verify_vartime` checks the signature, as
  `store_admit.coax` does. Only then is `verified` set, along with the
  event's id and created_at.
- `OK` carries the id and the verdict.
- `"auth-required:"` (NIP-42) on an `OK` or `CLOSED` message sets
  `auth_required`.
- Subscription ids are compared as their raw JSON string bytes.

Kind 0 means `message.coax` refused the frame, or it was an `AUTH` challenge.
The host routes `AUTH` to `nip42_input` and gives everything else to
`rl_input`, which counts it as `RL_BAD`. A verified event grants no author,
subscription or freshness authority: `relay.coax` decides admission.

## Evidence

- **Checks:** 267 of 267 pass, including everything it loads, on main at
  `c7f0d7a222`. With one captured relay.openagents.com event (kind 30617) and
  a copy with one signature bit changed
  (`coax/tests/nostr/relay_read_fixtures.coax`), the checks cover:
  - delivery and duplicate detection through `rl_input`;
  - the forged copy counted as `RL_FORGED`;
  - scratch too small or aliased verifying nothing;
  - `OK`, `EOSE`, `CLOSED` and `NOTICE`, with and without `auth-required`;
  - refused frames and `AUTH` as kind 0.
- **Mutation:** 5,371 mutants, 5,371 killed, 0 survived, none marked
  equivalent. It ran as 100 shards with `--jobs 1`, merged, on a snapshot of
  `c7f0d7a222`; all 32 loaded units are byte-identical to main when this
  landed. The count includes `message.coax`'s and `relay.coax`'s mutants,
  because `coax mutate` mutates every `#authority` unit a file loads.

## Not done

- **relay_e2e:** `coax/tests/net/relay_e2e.coax` still uses its scripted
  frame adapter. Moving it onto `rl_read` would need fixture frames whose
  subscription ids match `relay.coax`'s (`s0.1`) and real signed events in
  every scripted frame.
- **The Pylon** (`psionic/lib/pylon/`) switches to `rl_read` itself.
