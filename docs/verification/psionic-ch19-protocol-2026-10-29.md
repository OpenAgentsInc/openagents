# Chapter 19: the Pylon's protocol logic — 2026-10-29

Psionic chapter 19 ("A Pylon answers decisions") needs signing, NIP-44,
a relay client and the protocol rules. This record covers the rules: the
parts that need no key, no cipher and no socket. They are in
`psionic/lib/pylon/`: four `#authority` units and one glue unit.

| File | What it decides |
| --- | --- |
| `template.coax` | The unsigned event template `{pubkey, created_at, kind, tags, content}` every Pylon record is written as, and the tag reader the checkers share. |
| `beacon.coax` | NIP-PYLON kind `30200` v1: the writer (JCS-ordered content, `d`/`t`/`x`/`expiration` tags, optional owner `auth` tag), the reader's checker (Validation and Freshness), served identities, hardware classes, the online/draining/offline lifecycle and the republish schedule. |
| `relays.coax` | NIP-65 kind `10002`: the writer and checker, and the read relays a buyer sends jobs to. |
| `host.coax` | Not `#authority`: it decides nothing. The one-call entries that hash with `coax/lib/crypto/sha256.coax` (`ph_beacon`, `ph_read_beacon`, `ph_digest`), and the checks that tie the units to the real SHA-256 and to chapter 16's `sv_decode`. Keeping SHA-256 and the server out of the authority units' load graph keeps their mutation runs to their own code. |
| `decision.coax` | NIP-DEC jobs: a decrypted `25910` to chapter 16's exact server body or a typed refusal; the server's `200` body to a `26910` result naming the pylon's served identity; `27010` status bodies; cancel mapped onto the server slot; the deadline as the server's decide budget; unsigned answer templates. |

## The boundary other agents call

Each is one call. Nothing here signs, encrypts or touches a socket.

- **Beacon out.** `ph_beacon(beacon:PbBeacon, auth:PbAuth, scratch:[]u8, out:[]u8)->int`
  writes the unsigned `30200` template (created_at = observed_at); it is
  `pb_content`, SHA-256, then `pb_write(beacon, auth, content, digest, out)`. The host
  passes `out[0..<n]` to the NIP-01 signer (`coax/lib/nostr/event_sign.coax`,
  openagents-d7), which adds `id` and `sig`. `PbAuth` is the owner's NIP-OA
  key and signature; empty omits the tag.
- **Relay list out.** `pr_write(out, pubkey, created_at, relays:[]PrRelay)->int`,
  the same way.
- **Beacon in (chapter 20).** After `nostr_event_verify_vartime`, call
  `ph_read_beacon(kind, signer_hex, tags_json, content)->PbRead` (or
  `pb_read(..., digest)` with a digest already in hand), then
  `pb_fresh(read, received_at, held)->PbFresh` and
  `pb_routable(read, fresh)`. An `auth` tag is reported (`auth`, `owner`,
  `auth_signature`) but not verified: NIP-OA verification is the caller's.
- **Relay list in.** `pr_read(kind, tags_json, content, reads:[]string)->PrList`.
- **Decision job in.** After verifying the `25910` and NIP-44-decrypting its
  content (`coax/lib/nostr/nip44.coax`, claude-nip44), fill
  `PdEnvelope{kind, id, pubkey, created_at, tags, plaintext}` and call
  `pd_admit(env, worker_hex, now, body:[]u8)->PdJob`. On `code==none` and a
  call, `body[0..<job.body]` is the server request; the host runs
  `pd_code(sv_code_name(sv_admit(engine, sv_decode(body, pools), pools)))` and gives the
  slot the smaller of `pd_left_ms(job, now)` (-2: no deadline, -1: passed) and its `limits.decide_ms` as the decide budget.
- **Decision answers out.** `pd_status(out, job, status, code, retry_ms)` for
  `27010`; `pd_result(out, job, server_200_body, slug, provider, version,
  identity, receipt)->PdOut` for `26910`. Encrypt the plaintext to the
  caller, then `pd_template(out, worker, now, kind, job, ciphertext)` gives
  the unsigned template (`p` = caller, `e` = request event) for the signer.
- **Cancel.** `pd_cancel(pending, dispatched, cancel)->PdCancel`: `drop`
  before dispatch, `stop` after (the host calls `sv_closed` on the slot,
  which gives `cancel_close`), then a `27010` error `cancelled`.

## Choices and spec disagreements

Written down, not resolved by guessing. The NIPs were not edited.

1. **NIP-DEC result prose vs the deployed wire.** NIP-DEC's "Result,
   status, and cancel" says a `26910` carries `outcome` (`completed`),
   `dispatched`, `response`, `receipt` and `code`. The implementation that
   existing buyers parse (`crates/nostr/src/decision.rs` and
   `crates/pylon/src/provider.rs`, read with `git show 0b401561a0^:<path>`)
   uses `outcome: "answered"` with `response`, a non-answered outcome with an
   `error` object, and sends every refusal as a `27010` status `error`. The
   coordinator chose the deployed wire; `decision.coax` builds it.
2. **Door.** Chapter 16's HTTP server names itself `service.door: "psionic"`
   with no provider. A Pylon's answer is `{door: "pylon:<slug>", identity,
   provider, version}` as NIP-PYLON says; `pd_result` rewrites it. The
   identity string is the server's own, `elephant-1@sha256:<64 hex>`, and the
   server's `model` must equal its name before `@` and its
   `service.identity` the beacon's identity, else `identity_mismatch`.
3. **JCS.** NIP-PYLON says every body is JCS JSON; its beacon example is not
   in JCS key order. The writer emits JCS order. A caller-supplied `meta`
   must itself be JCS. The reader accepts any order, since the `x` tag
   covers the exact bytes.
4. **`requires` on requests.** NIP-DEC lists `requires`; the deployed callers
   send none. The pylon accepts it absent or `[]`, refuses any feature with
   `unsupported_feature` and a non-array with `malformed`.
5. **Freshness window.** NIP-DEC names no window. The deployed pylon used 120 s
   back and 30 s ahead (`RequestWindow::new(120, 30)`); so does `pd_admit`.
   An `expiration` at or before now and a `deadline` at or before now are
   `stale`; a `deadline` that differs from `expiration` is `invalid_request`
   (NIP-CJ: they must agree).
6. **State depth.** NIP-DEC allows a `state` object 32 levels deep; the shared
   JSON reader stops at 16, so such a payload is `unsupported_input`, as the
   chapter 16 server already answers.
7. **Reader rules beyond the text.** The beacon reader also refuses
   `since > observed_at` and `valid_until < observed_at` (negative uptime or
   a beacon expired when made), requires `pools` only to be valid when
   present, and for a `cj-decision` service requires the identity form
   (`<name>@sha256:<hex>` or `<name>@gguf-tensors:sha256:<hex>`) and the
   capability `<signer>:pylon/decision`. Tag values with JSON escapes are
   refused. A draining pylon keeps republishing on the refresh schedule so
   it does not turn unknown. The relay list is bounded at 16 relays, `ws://`
   or `wss://` URLs of at most 256 bytes, no duplicates. `pb_valid_until`
   refuses (-1) a lifetime outside 1..300 s and `pb_next` (-2) a refresh
   outside 60..240 s rather than clamping them.
8. **Tier and memory.** The reader checks `tier` is a band name and
   `memory_gb` one of 8..512; it does not cross-check tier against memory,
   since a `gpu` tier is by video memory and `memory_gb` is not.

## Checks

Compiler: `coax0 0.24.0`, built in the worktree (`coax/build.sh`). Host
load average was 90–160 throughout, so every mutation run was sharded.

`coax test` (each count includes the checks of the units it loads):

| File | Result |
| --- | --- |
| `psionic/lib/pylon/template.coax` | 24 checks, 24 passed |
| `psionic/lib/pylon/relays.coax` | 28 checks, 28 passed |
| `psionic/lib/pylon/host.coax` (loads all four units, SHA-256 and the server) | 197 checks, 197 passed |

The checks use the NIP-PYLON relay-list example verbatim, a `30200` beacon
built to the NIP's example shape (an Elephant decision service on one GPU),
a `25910` with a NIP-DEC structured noul, and the chapter 16 server's own
`200` body shape. Every NIP-PYLON Validation refusal has a case, each beacon
field at its bounds (label 64 bytes, 16 services, 16 settlements, 8 pools,
64-byte slugs, 128-byte models, 0..64 slots), freshness at its exact edges,
the republish schedule, and for decisions every refusal code, the 120 s/30 s
window and the expiration and deadline edges.

`coax mutate` (zero survivors is the gate; no `// mutant: equivalent`
markers were used):

| Run | Mutants | Result |
| --- | --- | --- |
| `relays.coax` (reaches `template.coax`), unsharded `--jobs 3` | 1237 | 0 survived |
| `decision.coax` (reaches `beacon.coax`, `template.coax`), `--shard I/7 --jobs 2` | 2346 | 0 survived (7 shard records merged with `--merge`) |

Earlier rounds found real gaps, all fixed with checks or by deleting
redundant conditions: round 1 (2409 mutants) 125 survivors, round 2 (2357)
13. A first unsharded attempt with SHA-256 loaded into `beacon.coax` timed
out at 15 minutes because SHA-256's own mutants joined the run; that is why
the digest is an input and `host.coax` holds the hash.

## What waits on other work

- **Signing**: `event_sign.coax` and `schnorr_sign.coax` (openagents-d7) turn
  every template here into a signed event; until then nothing is published.
- **NIP-44**: `nip44.coax` (claude-nip44) decrypts `25910` content into
  `PdEnvelope.plaintext` and encrypts `pd_status`/`pd_result` plaintext.
- **Relay**: claude-relay's client publishes the beacon and relay list and
  subscribes to `25910` with a `p` filter (`coax/lib/nostr/filter.coax`).
- **NIP-OA**: the `auth` tag is carried and shape-checked; its signature is
  not verified here. The owner's signature is an owner action
  (`NEEDS_OWNER.md`).
- **Receipt**: `pd_result` takes the execution receipt as a caller-supplied
  JSON object. The deployed reader requires `openagents.receipt.execution.v1`
  with `transport: "relay"`, `request_digest` (the JCS digest of `{attempt,
  deadline, model, questions, request, state}`), `attempt_id` and a JCS
  self-digest. Building it needs `jcs.coax` and SHA-256 over the request and
  is not done yet.
- **Not built**: v2 group beacons, `pylon.deadline.v1`, receipts (`3201`) and
  pool aggregates (`30201`), idempotency storage for `(worker, principal,
  request, attempt)`, and the host loop that wires these calls together.
