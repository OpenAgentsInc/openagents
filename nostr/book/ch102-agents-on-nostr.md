# 102. Agents on Nostr

An OpenAgents agent is a Nostr key that an owner vouches for. It logs in to
a relay, takes a job, does the work, answers and gets paid, and every step
is an event or a record that another key can check. This chapter runs that
whole path once, offline, with test keys and a fixed clock, by composing
units that earlier chapters built: the owner's attestation, relay
authentication, the labor market, the job transport, the claim, the run
journal and the Lightning proof. It also says plainly which of these parts
OpenAgents runs in production today and which exist only in this book.

The implemented label covers this offline teaching profile. It does not
complete the production agent path, and the registry records no direct
external consumer of the composed CJ/MKT/COORD/RUN exchange. Follow the
[completion route](COMPLETION.md#4-connect-the-offline-integrations-to-applications)
for schema, transport, wallet and staged-runner exits. The pending owner
review of the narrowed offline done criteria remains in `NEEDS_OWNER.md`.

## What this chapter teaches

- How an owner's NIP-OA attestation lets an agent key into a relay that
  knows only the owner (NIP-42 and NIP-AA).
- How a labor order (NIP-MKT under NIP-LAB) names the agent as the worker
  and fixes the price before any work starts.
- How an execution job (NIP-CJ) travels: sealed to the agent, claimed once
  (NIP-COORD), journaled (NIP-RUN), answered, and checked by the buyer.
- How payment is decided: the exact accepted price, and a Lightning
  preimage that hashes to the invoice's payment hash.
- Where each layer refuses: an unattested key, a stale claim, a result from
  the wrong worker, and a payment that does not match.
- What runs in production today, and what is only in this book.

## Where the code is

No new library code was written for this chapter. The example composes
these units:

| File | Procedure | What it decides | Chapter |
| --- | --- | --- | --- |
| [event_sign.coax](../../coax/lib/nostr/event_sign.coax), [event.coax](../../coax/lib/nostr/event.coax) | `nostr_event_sign_step`, `nostr_event_verify_vartime` | signing and verifying events | 13, 21 |
| [nipoa.coax](../../coax/lib/nostr/nipoa.coax) | `noa_preimage`, `noa_hash`, `noa_write_tag` | the owner's attestation and its tag | 87 |
| [nipaa.coax](../../coax/lib/nostr/nipaa.coax), [nip42.coax](../../coax/lib/nostr/nip42.coax) | `naa_auth`, `naa_event` | whether the relay admits the agent's key, and its events | 87, 38 |
| [mkt/order.coax](../../coax/lib/nostr/mkt/order.coax) | `mkt_rfq` ... `mkt_result` | the order, delivery, acceptance and payment | 80 |
| [nip44-key.coax](../../coax/lib/nostr/nip44-key.coax), [nip44.coax](../../coax/lib/nostr/nip44.coax) | `nip44_conversation_key_from_keys_checked`, `nip44_encrypt_checked`, `nip44_decrypt_checked` | the sealed job and answers | 20 |
| [cj/result.coax](../../coax/lib/nostr/cj/result.coax) | `cj_answer`, `cj_tags` | whether an answer answers the request | 80 |
| [coord/claim.coax](../../coax/lib/nostr/coord/claim.coax) | `coord_claim`, `coord_fence` | the one claim on the job, and the write fence | 76 |
| [run/journal.coax](../../coax/lib/nostr/run/journal.coax) | `run_digest`, `run_replay` | the run's hash-chained records | 78 |

Every file in the table that decides admission, policy or money is an
`#authority` file whose checks came first and whose mutation record has no
survivors; "How the code is checked" lists the records.

## What runs today and what is only in this book

OpenAgents' agents use Nostr keys in production today, but not this
chapter's whole path:

- **The hosted project agent.**
  [coax/web/agent/runner.c](../../coax/web/agent/runner.c), on Cloud Run,
  holds the runner's Nostr key and signs one NIP-98 event (kind `27235`)
  for each request to the OpenAgents API. It signs in C with
  libsecp256k1, and it talks to the API over HTTPS, not to a relay.
- **The API's check of those requests is Coax.**
  [coax/web/api/nip98.coax](../../coax/web/api/nip98.coax) is compiled into
  the API and checks every NIP-98 header (chapter 49).
- **The `openagents` command line** signs its NIP-98 requests with
  [nip98_sign.coax](../../coax/lib/nostr/nip98_sign.coax).
- **Pylons** run this book's stack against a relay:
  `scripts/join-pylon.sh` builds `psionic/lib/pylon/run.coax`, which
  authenticates with NIP-42, signs, and seals and opens NIP-44 jobs and
  answers against `wss://relay.openagents.com` (chapter 100). That relay is
  not yet the Coax relay (chapter 103).

Only in this book, today:

- NIP-AA admission of an attested agent: no relay that OpenAgents runs
  calls `naa_auth`.
- The execution family of NIP-CJ (`25920`, `26920`, `27020`), NIP-MKT
  orders, NIP-COORD claims and NIP-RUN journals: no program outside the
  book's examples and the units' own checks loads `cj/result.coax`,
  `mkt/order.coax`, `coord/claim.coax` or `run/journal.coax`.
- The hosted runner moving its signing to chapter 13's signer, publishing
  through a relay with chapter 29's client, and a rehearsed rollback to the
  C path. That migration needs a staging deployment, and it is not done.

## The path

One agent, one job, one payment. The keys are the published test scalars:

| Key | Role | x-only public key |
| --- | --- | --- |
| 1 | owner, a member of the relay | `79be667e...16f81798` |
| 2 | the agent: provider and worker | `c6047f94...5c709ee5` |
| 3 | the buyer | `f9308a01...bce036f9` |
| 4 | the reviewer the terms name | `e493dbf1...e8c4cd13` |
| 7 | a stranger nobody attests | `5cbdf064...cac4f9bc` |

The clock starts at `T0 = 1790000000`. Four Nostr events cross the relay;
the market steps, the claim and the run records are kept by the parties
and are shown here as the state they change:

| Time | Step | Layer | Signer |
| --- | --- | --- | --- |
| T0 | AUTH event, kind `22242`, with the owner's `auth` tag | NIP-42, NIP-AA | agent |
| T0 + 10 to 40 | request for quote, quote, order, confirmation | NIP-MKT | buyer, agent |
| T0 + 60 | execute request, kind `25920`, sealed to the agent | NIP-CJ | buyer |
| T0 + 61 | claim `cj:req-1#1`; RUN records 0 and 1 | NIP-COORD, NIP-RUN | agent |
| T0 + 62 | RUN record 2 (dispatched); accepted, kind `27020` | NIP-RUN, NIP-CJ | agent |
| T0 + 90 | RUN record 3 (resolved) | NIP-RUN | agent |
| T0 + 92 | the fence; result, kind `26920`, sealed to the buyer | NIP-COORD, NIP-CJ | agent |
| T0 + 95 to 140 | submission, review, acceptance, invoice, attempt, proof | NIP-MKT | agent, reviewer, buyer |
| T0 + 141 | RUN record 4 (settled) | NIP-RUN | agent |

## The trace by hand

### The attestation

The owner signs, with BIP-340, the SHA-256 of one line:

```text
nostr:agent-auth:c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5:created_at<1790086400
```

That is 17 bytes of prefix, 64 of hex, 1 colon and 21 of conditions: 103
bytes. Its SHA-256 is `8aef2f5a...533c3dad`. The conditions have one clause,
so the tag authorizes any kind created before `1790086400`, one day after
`T0`. The tag is `["auth", owner, conditions, signature]`.

### The login

The agent answers the relay's challenge `agent-challenge-1` with a kind
`22242` event whose tags are the relay, the challenge and the `auth` tag.
The relay runs NIP-AA's steps in order:

1. NIP-42: the event verifies, is kind `22242`, and names this relay and
   this connection's challenge. Its `created_at`, `T0`, is within 120
   seconds of the relay's clock, `T0 + 5`.
2. The agent's key is not a member, so the relay reads the `auth` tag:
   exactly one, four elements.
3. The owner's signature verifies over the line above, built with the
   *event's* pubkey, and `T0 < 1790086400` holds.
4. The owner is a member, so the agent's key gets virtual membership on
   this connection.

Step 3 is why a stranger cannot copy the tag. The line the owner signed
contains the agent's key. On an event signed by `7G`, the relay builds the
line with `7G`'s key, and the owner's signature does not verify over it.

### The order

The agent quotes 50,000 msat with itself as the worker and `4G` as the
reviewer. The terms forbid a reviewer who is the provider or the worker.
The invoice `lnbc500n1...` asks for 500 nanobitcoin. One nanobitcoin is 100
millisatoshis, so the invoice is for 50,000 msat, exactly the price. Its
payment hash is the SHA-256 of the 32 bytes `01 02 ... 20`:
`ae216c2e...b7a612c9`.

### The job

The execute body is 299 bytes. NIP-44 pads lengths above 256 to a multiple
of one eighth of the next power of two: the next power of two is 512, so
the chunk is 64 and 299 pads to 320. The payload is 1 version byte, a
32-byte nonce, a 2-byte length, 320 padded bytes and a 32-byte MAC: 387
bytes, which is 4 × 129 = 516 base64 characters. The conversation key is
the same from both sides because `3 × (2G) = 2 × (3G) = 6G`.

### The claim

The agent's claim store starts at revision 0, token 0. The claim on
`cj:req-1#1` (request `req-1`, attempt 1, NIP-CJ's idempotency key) expects
revision 0, so it is granted: token 1, revision 1. Any later claim that
expects revision 0 is stale. One that expects revision 1 is current but
finds the resource held, so it conflicts. Before the agent publishes its
result, the fence checks that the agent holds the latest token, 1, in
epoch 1, before the lease ends at `T0 + 3600`.

### The journal

Each RUN record's digest is the SHA-256 of the schema line
`openagents.run-record.v1` and a newline, followed by the record's JCS
bytes. Record 0, with its keys sorted, is 294 bytes:

```text
{"controller":"c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5","data":{},"generation":0,"previous":null,"run":"2222222222222222222222222222222222222222222222222222222222222222","seq":0,"subject":{"attempt":null,"iteration":null,"step":null},"time":1790000061,"type":"created"}
```

Its digest is `sha256:62525f04...ff8dfdf9`. Each later record names the one
before it in `previous`. The accepted answer names record 1, the admission.
The result names record 3, the resolution. Record 4, settled, carries the
paid amount.

### The event ids

Each id is the SHA-256 of `[0, pubkey, created_at, kind, tags, content]`
serialized as NIP-01 says (chapter 17). With BIP-340's aux at 32 zero bytes
and the NIP-44 nonces fixed at `01...01`, `02...02` and `03...03`, the four
events have these ids:

| Event | id |
| --- | --- |
| AUTH, `22242` | `ce9b500e...d32d294d` |
| execute request, `25920` | `b1e227cd...f04e8cd6` |
| accepted, `27020` | `c860164f...8d8e6d34` |
| result, `26920` | `c0ab882a...362febe8` |

An independent Python program computes the same four ids, the signatures
and the RUN digests; "How the code is checked" shows how.

## Refusals

Each layer refuses at its own point, and a refusal changes no state:

- **An unattested key.** `7G` answers the challenge with no `auth` tag: "no
  auth tag" (`NAA_NO_AUTH`), and the relay refuses its events as "not
  authenticated". With the agent's tag copied onto its own AUTH event, the
  tag does not verify (`NAA_CREDENTIAL`, "not the owner's signature").
- **A stale claim.** A second delivery of the same job, claimed at the
  revision it saw, 0, is `stale` (`CO_STALE`). At the current revision, 1,
  it is `conflict`. The token stays 1 and nothing is dispatched twice.
- **The wrong worker's result.** `7G` signs a `26920` with the agent's tags
  and the agent's sealed content. The event verifies, but `cj_answer`
  refuses it: the request named `2G` as the worker (`CJ_SIGNER`).
- **A payment mismatch.** An invoice for 60,000 msat against the accepted
  50,000 is refused (`MKT_AMOUNT`). A wallet report of "confirmed" with a
  preimage that does not hash to the payment hash is no proof
  (`MKT_PROOF`). One for 49,000 msat is refused (`MKT_AMOUNT`). Nothing is
  paid on those paths.

## The example

The example is
[nostr/examples/agents-on-nostr.coax](../examples/agents-on-nostr.coax). It
runs the path above in ten sections and then the refusals. No socket or
relay runs: the relay's side is `naa_auth` on a connection held in memory,
and events go from one party to the other as strings. The constants and
helpers come first:

<!-- source: nostr/examples/agents-on-nostr.coax example -->
```jai
// Chapter 102: one OpenAgents agent on the Nostr stack, end to end, offline.
// The owner (key 1) attests the agent (key 2); the agent logs in to a relay
// with NIP-42 and NIP-AA, quotes a labor order (MKT) to a buyer (key 3),
// takes the buyer's execution job (CJ), claims it (COORD), keeps a run
// journal (RUN), answers, and is paid against a Lightning preimage. Key 4
// reviews the delivery; key 7 is attested by nobody. Published test keys,
// zero BIP-340 aux and fixed NIP-44 nonces, so every byte is the same on
// every run: TEST ONLY. Never sign with a real key in an example.
#load "../../coax/lib/nostr/nipaa.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#load "../../coax/lib/nostr/nip44-key.coax";
#load "../../coax/lib/nostr/cj/result.coax";
#load "../../coax/lib/nostr/mkt/order.coax";
#load "../../coax/lib/nostr/coord/claim.coax";
#load "../../coax/lib/nostr/run/journal.coax";
#import "Basic";

T0 :: 1790000000;
OWNER_SK :: "0000000000000000000000000000000000000000000000000000000000000001";
AGENT_SK :: "0000000000000000000000000000000000000000000000000000000000000002";
BUYER_SK :: "0000000000000000000000000000000000000000000000000000000000000003";
STRANGER_SK :: "0000000000000000000000000000000000000000000000000000000000000007";
REVIEWER :: "e493dbf1c10d80f3581e4904930b1404cc6c13900ee0758474fa94abe8c4cd13";   // 4G
AUX :: "0000000000000000000000000000000000000000000000000000000000000000";
NONCE_REQUEST :: "0101010101010101010101010101010101010101010101010101010101010101";
NONCE_ACCEPTED :: "0202020202020202020202020202020202020202020202020202020202020202";
NONCE_RESULT :: "0303030303030303030303030303030303030303030303030303030303030303";
RELAY :: "wss://relay.example.com";
CHALLENGE :: "agent-challenge-1";
// The owner's conditions: any kind, created before T0 + 86400.
CONDS :: "created_at<1790086400";
RUN :: "2222222222222222222222222222222222222222222222222222222222222222";
CLAIM :: "cj:req-1#1";   // NIP-CJ's idempotency key: request req-1, attempt 1
TERMS :: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
// x402's 50,000 msat invoice; its payment hash is SHA-256 of 01 02 ... 20.
INVOICE :: "lnbc500n1p4wfve5pp54cskcth4y3ar0qkpxhh6y7drunxuvyy5yu8462l9333qfdaxztys";
HASH :: "ae216c2ef5247a3782c135efa279a3e4cdc61094270f5d2be58c6204b7a612c9";
PREIMAGE :: "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20";
WRONG_PREIMAGE :: "2001020304050607080910111213141516171819202122232425262728293031";
EXECUTE :: "{\"v\":\"openagents.execution.v1\",\"requires\":[\"openagents.labor-binding.v1\"],\"type\":\"execute\",\"request\":\"req-1\",\"attempt\":1,\"run\":\"2222222222222222222222222222222222222222222222222222222222222222\",\"target\":\"op:summarize\",\"input\":{\"text\":\"one paragraph\"},\"deadline\":1790003600,\"retain_until\":1790086400}";

bytes_of :: (text: string) -> [32] u8 {
  b: [32] u8;
  #assert hex_decode_checked(text, b[..], false).error == NC_OK;
  return b;
}
hex_of :: (bytes: [] u8, out: [] u8) -> string {
  r := hex_encode_checked(cast(string) bytes, out, false);
  #assert r.error == NC_OK;
  return cast(string) out[0..<r.written];
}
// One event signed with a test key by chapter 13's signer (event_sign.coax).
sign :: (secret: string, created_at: int, kind: int, tags: string, content: string, out: [] u8) -> string {
  key := bytes_of(secret); aux := bytes_of(AUX);
  work: [1] NostrSignWork; scratch: [16384] u8; k := EVENT_MORE;
  while k == EVENT_MORE {
    k = nostr_event_sign_step(key[..], aux[..], created_at, kind, tags, content, work[..], scratch[..], out, false);
  }
  #assert k > 0;
  return cast(string) out[0..<k];
}
// The x-only public key of a test secret: an event signed and read back.
public_of :: (secret: string, out: [] u8) -> string {
  s: [1024] u8; w: [1024] u8; e := nostr_event_read(sign(secret, 0, 1, "[]", "", s[..]), w[..], false);
  key := e.public_key;
  return hex_of(key[..], out);
}
// BIP-340 over a 32-byte hash: the owner's attestation signature.
sign_hash :: (secret: string, message: [] u8, out: [] u8) {
  key := bytes_of(secret); aux := bytes_of(AUX); work: [1] SignWork; status := SIGN_MORE;
  while status == SIGN_MORE { status = schnorr_sign_step(key[..], aux[..], message, work[..], out, false); }
  #assert status == SIGN_DONE;
}
conv_key :: (secret: string, peer: string) -> [32] u8 {
  key := bytes_of(secret); x := bytes_of(peer); ecdh: [1] EcdhScratch; ck: [32] u8;
  #assert nip44_conversation_key_from_keys_checked(key[..], x[..], ecdh[..], ck[..], false) == NIP44_OK;
  return ck;
}
bytes_in :: (s: string, out: [] u8) -> [] u8 {
  for i: 0..<s.count { out[i] = s[i]; }
  return out[0..<s.count];
}
seal :: (ck: [] u8, nonce_hex: string, plaintext: string, out: [] u8) -> string {
  nonce := bytes_of(nonce_hex); work: [8192] u8; pt: [4096] u8; n := plaintext.count;
  r := nip44_encrypt_checked(NIP44_COMPAT, ck, nonce[..], bytes_in(plaintext, pt[..]),
         work[0..<nip44_encrypt_work_size(NIP44_COMPAT, n)], out[0..<nip44_payload_size(NIP44_COMPAT, n)], false);
  #assert r.error == NIP44_OK;
  return cast(string) out[0..<r.written];
}
open_content :: (ck: [] u8, ev: NostrEvent, out: [] u8) -> string {
  raw := ev.original[ev.content_from..<ev.content_to]; payload: [4096] u8;
  k := json_unescape(raw, payload[..]);
  work: [8192] u8; size := nip44_decrypt_work_size(NIP44_COMPAT, k);
  r := nip44_decrypt_checked(NIP44_COMPAT, ck, cast(string) payload[0..<k], work[0..<size], out, false);
  if r.error != NIP44_OK { return ""; }
  return cast(string) out[0..<r.written];
}
verify :: (text: string, scratch: [] u8) -> NostrEvent { return nostr_event_verify_vartime(text, scratch, false); }
put :: (out: [] u8, at: int, text: string) -> int { n := at; for c: text { out[n] = c; n += 1; } return n; }
id_of :: (event: string) -> string { return event[7..<71]; }

NAA_WORDS :: [9] string.["member", "virtual member", "NIP-42 refuses", "stale", "no auth tag", "more than one auth tag",
                         "the tag doesn't verify", "the owner is not a member", "connection full"];
NOA_WORDS :: [11] string.["ok", "not a verified event", "no auth tag", "more than one auth tag", "not four elements",
                          "not lowercase hex", "the owner is the event's own key", "the conditions don't read",
                          "not the owner's signature", "outside the conditions", "unchecked"];
E_WORDS :: [4] string.["accepted", "restricted: not authenticated", "restricted: kind not in the credential", "restricted: virtual member"];
MKT_WORDS :: [21] string.["accepted","malformed","not_admitted","identity_mismatch","stale","conflict",
  "limit_exceeded","unsupported_profile","quote_expired","quote_consumed","confirmation_expired",
  "price_exceeded","content_unavailable","idempotency_conflict","not accepted: nothing to pay against",
  "amount: not the accepted price","no proof: the preimage does not hash to the payment hash",
  "already paid","pending: an earlier attempt is unresolved","late","not passed by the reviewer"];
CJ_WORDS :: [8] string.["answers the request","refused: the event does not verify","refused: kind",
  "refused: another family","refused: tags","refused: not the worker the request named",
  "refused: p is not the caller","refused: e is not the request"];
CO_WORDS :: [8] string.["ok", "malformed", "limit", "stale", "conflict", "not_admitted", "expired", "closed"];
RN_WORDS :: [12] string.["ok","incomplete: a gap","conflict: a fork","broken chain","digest mismatch",
  "not_admitted","stale","order","unfenced handoff","closed: after settled","malformed","limit_exceeded"];
```

The relay's side of the login. A fresh NIP-42 state issues the challenge,
the key signs its AUTH event, and `naa_auth` decides:

<!-- source: nostr/examples/agents-on-nostr.coax login -->
```jai
// The agent (or any key) answers the relay's challenge on a fresh
// connection; the relay runs NIP-AA under its own clock, now.
Login :: struct { a: NaaAuth; text: string; }
login :: (secret: string, extra: string, conn: [] NaaConnection, m: NaaMembers, created_at: int, now: int, out: [] u8) -> Login {
  relay: [1] Nip42; frame: [64] u8;
  #assert nip42_init(relay[..], RELAY) == N42_OK && nip42_issue(relay[..], 1, CHALLENGE, frame[..]).error == N42_OK;
  tb: [1024] u8; n := put(tb[..], 0, "[[\"relay\",\""); n = put(tb[..], n, RELAY);
  n = put(tb[..], n, "\"],[\"challenge\",\""); n = put(tb[..], n, CHALLENGE); n = put(tb[..], n, "\"]");
  n = put(tb[..], n, extra); n = put(tb[..], n, "]");
  text := sign(secret, created_at, 22242, cast(string) tb[0..<n], "", out);
  scratch: [4096] u8;
  return Login.{a = naa_auth(conn, m, relay[..], 1, now, text, scratch[..]), text = text};
}
```

The tags and the bodies the agent and the buyer exchange. The bodies are
short forms of NIP-CJ's execute, accepted and result payloads. No Coax
parser for those payloads exists yet, so nothing here checks their schema.

<!-- source: nostr/examples/agents-on-nostr.coax bodies -->
```jai
// The job's tags: one p naming the worker, and NIP-40's expiration
// mirroring the body's deadline.
request_tags :: (worker: string, out: [] u8) -> string {
  n := put(out, 0, "[[\"p\",\""); n = put(out, n, worker); n = put(out, n, "\"],[\"expiration\",\"1790003600\"]]");
  return cast(string) out[0..<n];
}
// Every answer: one p naming the caller (the buyer, 3G) and one e naming
// the exact request.
answer_tags_of :: (request_id: string, out: [] u8) -> string {
  n := put(out, 0, "[[\"p\",\""); n = put(out, n, BUYER_X); n = put(out, n, "\"],[\"e\",\"");
  n = put(out, n, request_id); n = put(out, n, "\"]]");
  return cast(string) out[0..<n];
}
// The 27020 accepted body: it names the RUN record that admitted the job.
accepted_of :: (admitted: string, out: [] u8) -> string {
  n := put(out, 0, "{\"v\":\"openagents.execution.v1\",\"requires\":[],\"type\":\"accepted\",\"request\":\"req-1\",\"attempt\":1,\"run\":\"");
  n = put(out, n, RUN); n = put(out, n, "\",\"record\":\""); n = put(out, n, admitted);
  n = put(out, n, "\",\"controller\":\""); n = put(out, n, AGENT_X); n = put(out, n, "\",\"retain_until\":1790086400}");
  return cast(string) out[0..<n];
}
// The 26920 result body: it names the RUN record that resolved the step.
result_of :: (resolved: string, out: [] u8) -> string {
  n := put(out, 0, "{\"v\":\"openagents.execution.v1\",\"requires\":[],\"type\":\"result\",\"request\":\"req-1\",\"attempt\":1,\"run\":\"");
  n = put(out, n, RUN); n = put(out, n, "\",\"status\":\"succeeded\",\"output\":{\"summary\":\"one sentence\"},\"record\":\"");
  n = put(out, n, resolved); n = put(out, n, "\"}");
  return cast(string) out[0..<n];
}
```

One run record, hashed and chained:

<!-- source: nostr/examples/agents-on-nostr.coax journal -->
```jai
// One RUN record, chained onto the one before it: the agent is the only
// controller, generation 0.
record :: (rs: [] RunRecord, store: [] u8, seq: int, type: int, step: string, time: int, data: string) {
  r: RunRecord; r.run = RUN; r.seq = seq; r.controller = AGENT_X; r.signer = AGENT_X; r.type = type;
  r.iteration = -1; r.step = step; r.attempt = ifx step.count == 0 then 0 else 1; r.time = time; r.data = data;
  r.previous = ifx seq == 0 then "" else rs[seq - 1].digest;
  work: [4096] u8; d := store[seq * 71..<seq * 71 + 71];
  #assert run_digest(r, work[..], d) == RN_OK;
  r.digest = cast(string) d; rs[seq] = r;
}
```

And the path itself:

<!-- source: nostr/examples/agents-on-nostr.coax main -->
```jai
main :: () {
  print("== 1. Keys\n");
  h1: [64] u8; h2: [64] u8; h3: [64] u8; h7: [64] u8;
  owner := public_of(OWNER_SK, h1[..]); agent := public_of(AGENT_SK, h2[..]);
  buyer := public_of(BUYER_SK, h3[..]); stranger := public_of(STRANGER_SK, h7[..]);
  print("owner 1G    %\nagent 2G    %\nbuyer 3G    %\nreviewer 4G %\nstranger 7G %\n", owner, agent, buyer, REVIEWER, stranger);
  #assert agent == AGENT_X && buyer == BUYER_X;

  print("\n== 2. NIP-OA: the owner attests the agent\n");
  ax := bytes_of(agent); ox := bytes_of(owner);
  pre: [NOA_TEXT_MAX] u8; pn := noa_preimage(ax, CONDS, pre[..]);
  print("preimage (% bytes): %\n", pn, cast(string) pre[0..<pn]);
  hash: [32] u8; #assert noa_hash(ax, CONDS, hash[..]);
  hh: [64] u8; print("sha256: %\n", hex_of(hash[..], hh[..]));
  sig: [64] u8; sign_hash(OWNER_SK, hash[..], sig[..]);
  sh: [128] u8; print("owner's signature, aux 0: %\n", hex_of(sig[..], sh[..]));
  tag: [512] u8; tn := noa_write_tag(ox, CONDS, sig, tag[..], 1); tag[0] = ',';
  attested := cast(string) tag[0..<tn];

  print("\n== 3. NIP-42 and NIP-AA: the agent logs in, the owner a member\n");
  m: [1] NaaMembers; #assert naa_add(m[..], ox);
  conn: [1] NaaConnection; ab: [2048] u8;
  l := login(AGENT_SK, attested, conn[..], m[0], T0, T0 + 5, ab[..]);
  print("AUTH (22242) at T0, % bytes, id %\n", l.text.count, id_of(l.text));
  print("tags %\n", l.text[text_find(l.text, "\"tags\":") + 7..<text_find(l.text, ",\"content\"")]);
  print("relay at T0 + 5: %\n", NAA_WORDS[l.a.code]);
  print("the agent's 26920 and 27020 events: %, %\n", E_WORDS[naa_event(conn[0], ax, 26920, true)], E_WORDS[naa_event(conn[0], ax, 27020, true)]);

  print("\n== 4. MKT: the agent quotes, the buyer orders\n");
  o := MktOffering.{provider = agent, valid_until = T0 + 86400, profiles = MKT_LIGHTNING, networks = MKT_BITCOIN};
  mk := [1] MktMarket.[MktMarket.{market = "m-1", buyer = buyer, provider = agent}]; q: [2] MktTerms;
  t: MktTerms; t.quote_id = "A"; t.digest = TERMS; t.worker = agent; t.reviewer = REVIEWER;
  t.price_msat = 50000; t.fee_limit_msat = 1000; t.profile = MKT_LIGHTNING; t.network = MKT_BITCOIN;
  t.quote_expires_at = T0 + 600; t.order_confirm_by = T0 + 900; t.delivery_due_at = T0 + 3600;
  t.review_due_at = T0 + 7200; t.payment_due_at = T0 + 86400; t.retain_until = T0 + 30 * 86400;
  print("T0 + 10 buyer asks, limit 60000 msat: %\n", MKT_WORDS[mkt_rfq(mk[..], o, buyer, 60000, T0 + 300, T0 + 40 * 86400, T0 + 10)]);
  print("T0 + 20 agent quotes 50000 msat, worker 2G, reviewer 4G: %\n", MKT_WORDS[mkt_quote(mk[..], q[..], t, agent, T0 + 20)]);
  print("T0 + 30 buyer orders quote A: %\n", MKT_WORDS[mkt_order(mk[..], q[..], buyer, "A", TERMS, "order-1", T0 + 30)]);
  print("T0 + 40 agent confirms: %\n", MKT_WORDS[mkt_ack(mk[..], q[..], agent, true, T0 + 40)]);

  print("\n== 5. CJ: the buyer seals an execution job to the agent\n");
  ck_buyer := conv_key(BUYER_SK, agent); ck_agent := conv_key(AGENT_SK, buyer);
  ch: [64] u8; print("conversation key (3, 2G) %; (2, 3G) is the same: %\n", hex_of(ck_buyer[..], ch[..]), cj_same(ck_buyer, ck_agent));
  print("execute body: % bytes, padded to %\n", EXECUTE.count, nip44_calc_padded_len(EXECUTE.count));
  pr: [4096] u8; payload := seal(ck_buyer[..], NONCE_REQUEST, EXECUTE, pr[..]);
  rt: [256] u8; rb: [8192] u8; request := sign(BUYER_SK, T0 + 60, 25920, request_tags(agent, rt[..]), payload, rb[..]);
  print("25920 at T0 + 60, % characters of content, id %\n", payload.count, id_of(request));
  rs1: [8192] u8; req := verify(request, rs1[..]);
  print("the agent: verified %, one p naming it %\n", req.verified, cj_tags(req).np == 1 && cj_same(cj_tags(req).p, ax));
  ob: [4096] u8; opened := open_content(ck_agent[..], req, ob[..]);
  print("opened, the execute body: %\n", opened == EXECUTE);

  print("\n== 6. COORD: the agent claims req-1 attempt 1 before any effect\n");
  st := [1] CoordState.[CoordState.{owner = agent, epoch = 1, revision = 0, max_lease = 3600}];
  claims: [4] CoordClaim; holds: [4] CoordHold;
  want := [1] CoordWant.[CoordWant.{resource = CLAIM, write = true}];
  cq := CoordRequest.{proposal = id_of(request), worker = agent, expected_revision = 0, requested_until = T0 + 3600};
  print("T0 + 61 claim %, expected revision 0: %, token %, revision %\n", CLAIM, CO_WORDS[coord_claim(st[..], claims[..], holds[..], cq, want[..], T0 + 61)], st[0].token, st[0].revision);

  print("\n== 7. RUN: the journal, and the accepted answer\n");
  rs: [5] RunRecord; store: [355] u8;
  record(rs[..], store[..], 0, RN_CREATED, "", T0 + 61, "{}");
  record(rs[..], store[..], 1, RN_ADMITTED, "", T0 + 61, "{}");
  record(rs[..], store[..], 2, RN_DISPATCHED, "execute", T0 + 62, "{}");
  for i: 0..<3 { print("seq % %: %\n", i, RN_NAMES[rs[i].type], rs[i].digest); }
  acc: [1024] u8; accepted_body := accepted_of(rs[1].digest, acc[..]);
  at: [256] u8; answer_tags := answer_tags_of(id_of(request), at[..]);
  pa: [4096] u8; eb: [8192] u8;
  accepted := sign(AGENT_SK, T0 + 62, 27020, answer_tags, seal(ck_agent[..], NONCE_ACCEPTED, accepted_body, pa[..]), eb[..]);
  print("27020 at T0 + 62, id %\n", id_of(accepted));

  print("\n== 8. The work ends; the agent answers\n");
  record(rs[..], store[..], 3, RN_RESOLVED, "execute", T0 + 90, "{\"outcome\":\"succeeded\"}");
  print("seq 3 resolved: %\n", rs[3].digest);
  res: [1024] u8; result_body := result_of(rs[3].digest, res[..]);
  print("T0 + 92 the fence before publishing, token 1: %\n", CO_WORDS[coord_fence(st[..], claims[..], holds[..], agent, CLAIM, 1, 1, T0 + 92)]);
  ps: [4096] u8; xb: [8192] u8; result_payload := seal(ck_agent[..], NONCE_RESULT, result_body, ps[..]);
  result := sign(AGENT_SK, T0 + 92, 26920, answer_tags, result_payload, xb[..]);
  print("26920 at T0 + 92, id %\n", id_of(result));
  vs: [8192] u8; v1 := verify(accepted, vs[..]); print("buyer: 27020 %\n", CJ_WORDS[cj_answer(req, v1)]);
  v2 := verify(result, vs[..]); print("buyer: 26920 %\n", CJ_WORDS[cj_answer(req, v2)]);
  bo: [4096] u8; print("buyer opens it: %\n", open_content(ck_buyer[..], v2, bo[..]));

  print("\n== 9. MKT: delivery, review, acceptance, payment\n");
  print("T0 + 95 agent submits: %\n", MKT_WORDS[mkt_submit(mk[..], q[..], agent, T0 + 95)]);
  print("T0 + 100 reviewer 4G records passed: %\n", MKT_WORDS[mkt_verify(mk[..], q[..], REVIEWER, MKT_PASSED)]);
  print("T0 + 110 buyer accepts: %; amount due % msat\n", MKT_WORDS[mkt_accept(mk[..], q[..], buyer, T0 + 100, T0 + 110)], mk[0].amount_due);
  print("T0 + 120 agent's invoice lnbc500n1...: %\n", MKT_WORDS[mkt_instruct(mk[..], q[..], agent, "ob-1", 50000, INVOICE, HASH, T0 + 5000, T0 + 120)]);
  print("T0 + 130 buyer records attempt 1: %\n", MKT_WORDS[mkt_attempt(mk[..], q[..], buyer, 1, 1000, T0 + 130)]);
  pf := MktProof.{rail = MKT_RAIL_LIGHTNING, state = MKT_PAY_CONFIRMED, amount_msat = 50000, preimage = PREIMAGE};
  print("T0 + 140 wallet: confirmed, preimage 0102...20: %; paid % msat\n", MKT_WORDS[mkt_result(mk[..], buyer, pf)], mk[0].paid_msat);
  record(rs[..], store[..], 4, RN_SETTLED, "", T0 + 141, "{\"paid_msat\":50000}");
  t5: [5] RunSubject; w5: [4096] u8; head := run_replay(rs[..], agent, t5[..], w5[..]);
  print("seq 4 settled: %\nreplay: %, head seq %, settled %\n", rs[4].digest, RN_WORDS[head.error], head.seq, head.settled);

  print("\n== 10. Refusals\n");
  r1: [1] NaaConnection; sb: [2048] u8;
  l = login(STRANGER_SK, "", r1[..], m[0], T0, T0 + 5, sb[..]);
  print("7G logs in with no auth tag: %; its 26920: %\n", NAA_WORDS[l.a.code], E_WORDS[naa_event(r1[0], bytes_of(stranger), 26920, true)]);
  l = login(STRANGER_SK, attested, r1[..], m[0], T0, T0 + 5, sb[..]);
  print("7G copies the agent's auth tag: %, %\n", NAA_WORDS[l.a.code], NOA_WORDS[l.a.detail]);
  cq2 := cq; cq2.proposal = "a second delivery of the job";
  print("a second claim of %, expected revision 0: %\n", CLAIM, CO_WORDS[coord_claim(st[..], claims[..], holds[..], cq2, want[..], T0 + 63)]);
  cq2.expected_revision = 1;
  print("the same at the current revision 1: %; token %, revision %\n", CO_WORDS[coord_claim(st[..], claims[..], holds[..], cq2, want[..], T0 + 63)], st[0].token, st[0].revision);
  ib: [8192] u8; impostor := sign(STRANGER_SK, T0 + 92, 26920, answer_tags, result_payload, ib[..]);
  v3 := verify(impostor, vs[..]); print("26920 from 7G with the agent's tags and content: %\n", CJ_WORDS[cj_answer(req, v3)]);
  m2 := [1] MktMarket.[mk[0]]; m2[0].obligation = ""; m2[0].payment = MKT_NONE; m2[0].attempts = 0; m2[0].paid_msat = 0;
  print("the agent asks 60000 msat: %\n", MKT_WORDS[mkt_instruct(m2[..], q[..], agent, "ob-1", 60000, INVOICE, HASH, T0 + 5000, T0 + 120)]);
  #assert mkt_instruct(m2[..], q[..], agent, "ob-1", 50000, INVOICE, HASH, T0 + 5000, T0 + 120) == MKT_OK;
  #assert mkt_attempt(m2[..], q[..], buyer, 1, 1000, T0 + 130) == MKT_OK;
  bad := pf; bad.preimage = WRONG_PREIMAGE;
  print("confirmed with another preimage: %\n", MKT_WORDS[mkt_result(m2[..], buyer, bad)]);
  bad = pf; bad.amount_msat = 49000;
  print("confirmed for 49000 msat: %\n", MKT_WORDS[mkt_result(m2[..], buyer, bad)]);
  print("paid on that copy: % msat\n", m2[0].paid_msat);
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/agents-on-nostr.coax --run-steps 250000000
```

The output shows the values worked out by hand: the 103-byte preimage and
its hash, the four event ids, the 299 bytes padded to 320 and 516
characters, token 1 and revision 1, the five RUN digests, and the 50,000
msat paid:

```text
== 1. Keys
owner 1G    79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798
agent 2G    c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5
buyer 3G    f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9
reviewer 4G e493dbf1c10d80f3581e4904930b1404cc6c13900ee0758474fa94abe8c4cd13
stranger 7G 5cbdf0646e5db4eaa398f365f2ea7a0e3d419b7e0330e39ce92bddedcac4f9bc

== 2. NIP-OA: the owner attests the agent
preimage (103 bytes): nostr:agent-auth:c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5:created_at<1790086400
sha256: 8aef2f5a49c479e813dbba1b9f08abc865eda3d1c6c04f00ddb4de75533c3dad
owner's signature, aux 0: 1d9a9a58b4a5059b73c3e3afd5360176338f4343e3a01b4a77d9d9e1deacac5318db1f78c734620c7e59d7c8ba431e22bbb46ff924f52b2c8020958423649079

== 3. NIP-42 and NIP-AA: the agent logs in, the owner a member
AUTH (22242) at T0, 646 bytes, id ce9b500ee1da0e6af2562292f1a55c616d028f5025c8a2bafa5a2a04d32d294d
tags [["relay","wss://relay.example.com"],["challenge","agent-challenge-1"],["auth","79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798","created_at<1790086400","1d9a9a58b4a5059b73c3e3afd5360176338f4343e3a01b4a77d9d9e1deacac5318db1f78c734620c7e59d7c8ba431e22bbb46ff924f52b2c8020958423649079"]]
relay at T0 + 5: virtual member
the agent's 26920 and 27020 events: accepted, accepted

== 4. MKT: the agent quotes, the buyer orders
T0 + 10 buyer asks, limit 60000 msat: accepted
T0 + 20 agent quotes 50000 msat, worker 2G, reviewer 4G: accepted
T0 + 30 buyer orders quote A: accepted
T0 + 40 agent confirms: accepted

== 5. CJ: the buyer seals an execution job to the agent
conversation key (3, 2G) dd8a0fe7f326cfcdd3c2cdce6da9a142a5655ad68bfa46a3a8aa1ddaa958d2c1; (2, 3G) is the same: true
execute body: 299 bytes, padded to 320
25920 at T0 + 60, 516 characters of content, id b1e227cdeedce259e39cb684d1fc61bbf22a9d49ad77a372f6f3f62af04e8cd6
the agent: verified true, one p naming it true
opened, the execute body: true

== 6. COORD: the agent claims req-1 attempt 1 before any effect
T0 + 61 claim cj:req-1#1, expected revision 0: ok, token 1, revision 1

== 7. RUN: the journal, and the accepted answer
seq 0 created: sha256:62525f0425bae66575b2bee9628d7726d652ba05b43a4d479216ba59ff8dfdf9
seq 1 admitted: sha256:12412cd290a943cb10c878413417de80b46e7d4f06bbf1ad20783829e019b914
seq 2 dispatched: sha256:0cdde7fc8007dd49d1d1a44a4999dded6412a5a2099e067f0775f84ca2d84c3e
27020 at T0 + 62, id c860164fac739a555345d415056870eae428fc0a78bbba1088bdbf678d8e6d34

== 8. The work ends; the agent answers
seq 3 resolved: sha256:39512c3be4c291cb227896c5a52b96c4b4a5c6b1a09e003798eb4c3fc33e288b
T0 + 92 the fence before publishing, token 1: ok
26920 at T0 + 92, id c0ab882a59256db05d0f70deae8f3d31555b906b8b548a124cd5891f362febe8
buyer: 27020 answers the request
buyer: 26920 answers the request
buyer opens it: {"v":"openagents.execution.v1","requires":[],"type":"result","request":"req-1","attempt":1,"run":"2222222222222222222222222222222222222222222222222222222222222222","status":"succeeded","output":{"summary":"one sentence"},"record":"sha256:39512c3be4c291cb227896c5a52b96c4b4a5c6b1a09e003798eb4c3fc33e288b"}

== 9. MKT: delivery, review, acceptance, payment
T0 + 95 agent submits: accepted
T0 + 100 reviewer 4G records passed: accepted
T0 + 110 buyer accepts: accepted; amount due 50000 msat
T0 + 120 agent's invoice lnbc500n1...: accepted
T0 + 130 buyer records attempt 1: accepted
T0 + 140 wallet: confirmed, preimage 0102...20: accepted; paid 50000 msat
seq 4 settled: sha256:d0eb6cf4574c573a359da166b74ef9f25a93006433dcb3e989a55b6cfe7bc633
replay: ok, head seq 4, settled true

== 10. Refusals
7G logs in with no auth tag: no auth tag; its 26920: restricted: not authenticated
7G copies the agent's auth tag: the tag doesn't verify, not the owner's signature
a second claim of cj:req-1#1, expected revision 0: stale
the same at the current revision 1: conflict; token 1, revision 1
26920 from 7G with the agent's tags and content: refused: not the worker the request named
the agent asks 60000 msat: amount: not the accepted price
confirmed with another preimage: no proof: the preimage does not hash to the payment hash
confirmed for 49000 msat: amount: not the accepted price
paid on that copy: 0 msat
```

The book's checks run this program in debug and release builds and compare
the output with
[nostr/fixtures/agents-on-nostr.out](../fixtures/agents-on-nostr.out).

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/agents-on-nostr.coax](../examples/checks/agents-on-nostr.coax),
which loads the example, so running the example doesn't run them. The
values they compare with come from
[coax/tests/nostr/agents_on_nostr_vectors.py](../../coax/tests/nostr/agents_on_nostr_vectors.py),
which writes
[agents_on_nostr_vectors.coax](../../coax/tests/nostr/agents_on_nostr_vectors.coax).
It computes the sequence with Python integers and `hashlib`, and with
BIP-340, NIP-44 v2 and NIP-01 written from the specifications in
`nip59_reference.py`. It shares no code with Coax.

- The five public keys, the attestation hash and the owner's signature.
- The AUTH event's id. The relay admits the agent and refuses the
  stranger, the copied tag, an AUTH event 121 seconds old, and one created
  at `1790086400`, outside the owner's clause.
- The conversation key from both sides, the 516-character payload, and the
  execute request's id and signature.
- All five RUN digests. The journal replays to settled, and refuses a
  record signed by the stranger.
- The accepted and result events' ids and signatures, and the impostor's
  id. The buyer admits both answers and refuses the impostor.
- For every expected revision from 0 to 3, a second claim of the job is
  never granted, and the fence admits the agent and not the stranger.
- The payment hash is Python's SHA-256 of the preimage. Every one of the
  64 preimages with one hex digit changed proves nothing, and nothing is
  paid.

```shell
coax/bin/coax test nostr/examples/checks/agents-on-nostr.coax --run-steps 250000000
python3 coax/tests/nostr/agents_on_nostr_vectors.py | diff - coax/tests/nostr/agents_on_nostr_vectors.coax
```

On 2026-11-01, `coax test` ran 601 checks, the loaded units' own checks included, and all passed. The checks spent about 190 seconds of compute; the two that sign and verify the most events take 25 and 36 seconds. The Python program's output equals the committed constants. The example's output was the same in debug and release. Root-only mutation of the checks file was started, but it had not finished at the 15-minute limit for one command, so it was stopped and has no result.

### The units' records

No library file changed for this chapter, so the units' mutation runs were
not repeated. Their records, all with no survivors and none marked
equivalent:

- `nipoa.coax` 210 of 210 and `nipaa.coax` 155 of 155, on 2026-11-01
  (chapter 87). `nip42.coax`: chapter 38 and
  [2026-10-29-nostr-nip42.md](../../docs/verification/2026-10-29-nostr-nip42.md).
- `cj/result.coax` 105 of 105 and `mkt/order.coax` 391 of 391, on
  2026-11-01 (chapter 80).
- `coord/claim.coax` 152 of 152, on 2026-11-01 (chapter 76).
- `run/journal.coax` 335 of 335 (chapter 78).
- Signing:
  [2026-10-30-event-sign.md](../../docs/verification/2026-10-30-event-sign.md).
  NIP-44:
  [2026-10-29-nip44-v2.md](../../docs/verification/2026-10-29-nip44-v2.md)
  and
  [2026-10-30-nip44-conversation-key.md](../../docs/verification/2026-10-30-nip44-conversation-key.md).

## What is not built yet

- The hosted runner on this stack: chapter 13's signer behind the
  signer-boundary gate in place of libsecp256k1, publishing through chapter
  29's client, the same identities and history in staging, and a rehearsed
  rollback to the C path.
- A relay that runs NIP-AA in production (chapter 103 plans the Coax
  relay).
- Schema checks of NIP-CJ's execute, accepted and result payloads, and
  NIP-LAB's labor binding between the order and the job.
- NIP-MKT's signed private envelopes (kind `3188`) around each market
  step, and NIP-RUN records published as events. Here the market and the
  journal are the parties' own state.
- Payment through a wallet. The example checks a preimage it is given;
  paying a BOLT11 invoice is the Spark wallet's job (chapter 101).

## Summary

- An owner vouches for an agent with one BIP-340 signature over the
  agent's key and the conditions. A relay that knows the owner admits the
  agent, and a copied tag fails on any other key.
- The market fixes the worker, the reviewer and the price before the work
  starts.
- The job is sealed to the worker, claimed once, journaled, and answered
  with a result that only the named worker can sign.
- The buyer pays exactly the accepted price, and only a preimage that
  hashes to the invoice's payment hash proves the payment.
- In production today, agents sign NIP-98 requests and Pylons speak this
  stack to a relay. NIP-AA admission, execution jobs, orders, claims and
  journals exist only in this book.

## Dependencies

- [79. Agents, skills and automation](ch079-agents-skills-and-automation.md)
- [80. Jobs, labor and markets](ch080-jobs-labor-and-markets.md)
- [13. Sign with Schnorr](ch013-sign-with-schnorr.md)
- [87. Owner attestation and agent
  authentication](ch087-owner-attestation-and-agent-authentication.md)
- [78. Runs, sessions, terminals and
  trajectories](ch078-runs-sessions-terminals-and-trajectories.md)
- [76. Work coordination and control](ch076-work-coordination-and-control.md)

## Sources

- [OpenAgents NIP-AGENT](../nips/openagents/NIP-AGENT.md)
- [OpenAgents NIP-CJ](../nips/openagents/NIP-CJ.md)
- [OpenAgents NIP-MKT](../nips/openagents/NIP-MKT.md) and
  [NIP-LAB](../nips/openagents/NIP-LAB.md)
- [OpenAgents NIP-COORD](../nips/openagents/NIP-COORD.md)
- [OpenAgents NIP-RUN](../nips/openagents/NIP-RUN.md)
- [NIP-OA](../nips/block/NIP-OA.md) and [NIP-AA](../nips/block/NIP-AA.md)
- [NIP-42](../nips/official/42.md), [NIP-44](../nips/official/44.md),
  [NIP-98](../nips/official/98.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
