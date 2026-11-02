# 100. Pylons and the mesh

Psionic's Pylons are the largest consumer of this book's stack. A Pylon is
one provider machine behind a Nostr key: it authenticates to a relay,
announces itself in a signed beacon, and answers decision jobs that buyers
seal to its key. This chapter follows one job through the Nostr side of a
Pylon, with test keys and a fixed clock, and shows which parts of the
exchange are checked and which are only claimed.

The registry records the native Pylon build and the mesh hooks as direct
consumers of this chapter's source. That adoption does not qualify every host
or approve production keys. The runtime still refuses `--key-file`; its
source audit, host key custody and owner approval are separate facts. See the
[completion route](COMPLETION.md#2-qualify-the-consumers-and-their-target-boundaries).

## What this chapter teaches

- How a Pylon answers a relay's NIP-42 challenge.
- What a buyer checks in a beacon, and what the beacon only claims.
- How a buyer seals a job with NIP-44 to the Pylon's key, and how the Pylon
  opens it, admits it and seals the answer back.
- Why a job sealed to another provider is never opened.
- How the mesh proves what one ciphertext held without giving out a
  conversation key.

## Where the code is

| File | Procedure | What it decides |
| --- | --- | --- |
| [nip42.coax](../../coax/lib/nostr/nip42.coax), [nip42-event.coax](../../coax/lib/nostr/nip42-event.coax) | `nip42_template`, `nip42_admit` | the AUTH event a client signs, and whether a relay admits it (chapter 38) |
| [nip44-key.coax](../../coax/lib/nostr/nip44-key.coax), [nip44.coax](../../coax/lib/nostr/nip44.coax) | `nip44_conversation_key_from_keys_checked`, `nip44_encrypt_checked`, `nip44_decrypt_checked` | the conversation key and NIP-44 v2 payloads (chapter 20) |
| [event_sign.coax](../../coax/lib/nostr/event_sign.coax), [event.coax](../../coax/lib/nostr/event.coax) | `nostr_event_sign_step`, `nostr_event_verify_vartime` | signing and verifying events (Bitcoin chapter 24 and chapter 21) |
| [beacon.coax](../../psionic/lib/pylon/beacon.coax), [host.coax](../../psionic/lib/pylon/host.coax) | `ph_beacon`, `ph_read_beacon`, `pb_fresh`, `pb_routable` | the kind 30200 beacon, written and read |
| [decision.coax](../../psionic/lib/pylon/decision.coax) | `pd_admit`, `pd_result`, `pd_status`, `pd_template` | whether a decrypted job is admitted, and the answer's body and template |
| [queue.coax](../../psionic/lib/pylon/queue.coax) | `pq_route` | what the running Pylon does with one event |
| [run.coax](../../psionic/lib/pylon/run.coax) | `py_event`, `py_seal`, `py_open_payload` | the Pylon program that wires the units above to a relay |
| [opening.coax](../../psionic/lib/mesh/opening.coax) | `mo_keys`, `mo_authenticate` | the mesh's nonce-specific opening keys |

`beacon.coax`, `decision.coax`, `queue.coax`, `nip42.coax`, `nip44.coax` and
`opening.coax` are `#authority` files. `run.coax` decides nothing itself.
[Psionic chapter 24](../../psionic/book/ch024-a-pylon-answers-decisions.md)
runs the program; [chapter 82](ch082-pylons-environments-and-deploys.md)
covers where Pylons run and how they are deployed.

## Four kinds of event

A Pylon's whole conversation with buyers uses four event kinds, plus the
NIP-42 AUTH event to its relay:

| Kind | Signed by | Content |
| --- | --- | --- |
| `22242` | Pylon | NIP-42 AUTH: the relay URL and the relay's challenge |
| `30200` | Pylon | the beacon: status, served model identity, hardware class, free slots |
| `25910` | buyer | a job, NIP-44 sealed to the Pylon |
| `26910`, `27010` | Pylon | the result or a status (refusal, busy, cancelled), NIP-44 sealed to the buyer |

Jobs and answers are in NIP-01's ephemeral range (20000 to 29999), so a
relay forwards them to the subscribers it has at that moment and stores
neither.

## The trace by hand

The example uses the published test keys 3, 7 and 5 (BIP-340 vector 0's
secret key is 3). The Pylon holds 3, the buyer 7, and another provider 5.
The clock is fixed at `CLOCK = 1791400000`, the time of `decision.coax`'s
fixtures.

### Keys

The x-only public keys are the x coordinates of `3G`, `7G` and `5G`:

| Key | x |
| --- | --- |
| Pylon, `3G` | `f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9` |
| buyer, `7G` | `5cbdf0646e5db4eaa398f365f2ea7a0e3d419b7e0330e39ce92bddedcac4f9bc` |
| other, `5G` | `2f8bde4d1a07209355b4a7250a5c5128e88b84bddc619ab7cba8d569b240efe4` |

`3G` and `5G` appear in Bitcoin chapter 16. The first two are `decision.coax`'s
`PD_WORKER` and `PD_CALLER`.

### The conversation key

NIP-44's conversation key is HKDF-extract with salt `"nip44-v2"` over the
x coordinate of the ECDH point. The buyer computes `7·(3G)` and the Pylon
computes `3·(7G)`. Both are `21G`:

```jai
x(21G) = 352bbf4a4cdd12564f93fa332ce333301d9ad40271f8107181340aef25be59d5
ck     = HMAC-SHA256(key = "nip44-v2", x(21G))
       = 87911e47109d035c08d1704433a66f396d3c4a28b9a26149f329f7e0dbcc4316
```

The provider `5G` shares a different point with the buyer, `7·5G = 35G`, so
its conversation key with the buyer is different (`2d954e97...3b0a7c24`).
A payload sealed under one key fails the other key's MAC check.

### The job's size

The job's plaintext is `decision.coax`'s `PD_CALL`, 269 bytes. NIP-44 pads
it before encryption:

1. 269 is more than 32, so the padding uses chunks.
2. The next power of two above 268 is 512. That is more than 256, so the
   chunk is 512 / 8 = 64.
3. The padded length is 64 · (⌊268 / 64⌋ + 1) = 64 · 5 = 320.
4. The decoded payload is 1 (version) + 32 (nonce) + 2 (length prefix) +
   320 + 32 (MAC) = 387 bytes.
5. Base64 makes that 4 · ⌈387 / 3⌉ = 516 characters.

The example's nonce is 32 bytes of `0x01`. The payload starts with the
bytes `02 01 01`, which base64 writes as `AgEB`, followed by `AQEB` for
each later group of three `0x01` bytes.

The answer is 560 bytes. The next power of two above 559 is 1024, so the
chunk is 128 and the padded length 128 · 5 = 640. The payload is
4 · ⌈707 / 3⌉ = 944 characters.

### The time window

`pd_admit` accepts a job at clock `now` only when all of these hold:

- `now - created_at ≤ 120` (`PD_PAST`) and `created_at - now ≤ 30`
  (`PD_AHEAD`);
- the expiration tag is after `now`;
- the deadline in the plaintext is after `now`.

The job has `created_at = 1791400000`, expiration `1791400060` and deadline
`1791400060`. It is admitted for `now` from `1791399970` to `1791400059`,
and at `CLOCK` it has 60 s, or 60000 ms, left. At `1791400060` it is
stale.

### The event id

The job's id is the SHA-256 of NIP-01's serialization (chapter 21):

```jai
[0,"5cbdf064...cac4f9bc",1791400000,25910,
 [["p","f9308a01...bce036f9"],["expiration","1791400060"]],"AgEBAQEB..."]
```

Python's `hashlib` gives
`8366669c1337ef543d110d5f79d762829f77137f6da54a50f3cfe5873cf9c248` for it.
The example prints the same id.

## Connect and authenticate

When a Pylon opens a connection, the relay sends a challenge. The Pylon's
`nip42_input` takes it, `nip42_template` writes a kind 22242 template with
the relay URL and the challenge, and `py_sign` signs it. The relay admits
the key only when the signature verifies, the kind is 22242, `created_at` is
within 600 s of its clock, and both tags name this relay and this
connection's challenge. Chapter 38 covers every refusal.

The Pylon sends nothing else until the relay accepts its AUTH: `pq_wire` in
`queue.coax` holds back the beacon, the subscription and the queued
answers. After every reconnect it authenticates again before it republishes
its beacon.

## The beacon: checked and claimed

The beacon (kind 30200) says that the Pylon is online, what model identity
it serves, its hardware class and how many slots are free. A buyer reads it
in three steps:

1. Verify the event's id and BIP-340 signature.
2. Read it with `ph_read_beacon`: kind 30200; one `d`, `t`, `x` and
   `expiration` tag; `x` equal to the SHA-256 of the content; the content's
   `provider` equal to the signer; each decision service's `capability`
   equal to `<signer>:pylon/decision`; `valid_until` equal to the
   expiration.
3. Check freshness with `pb_fresh` on the buyer's clock: `observed_at` at
   most 30 s ahead, and the clock at most `valid_until` and at most 300 s
   after `observed_at`.

Steps 1 and 2 bind every field to the signer's key: a beacon copied and
signed by another key fails at `provider`. Nothing binds the claims inside
it to the machine. The hardware class, the free slots and the model
identity are what the provider says. The model identity is checked again in
each answer: `pd_result` refuses an engine whose reply names another
identity (`identity_mismatch`). That check runs on the Pylon, so it shows
only that the Pylon and its engine agree. It does not prove which weights
ran. NIP-PYLON says so directly: "A beacon is a claim."

## Seal, open, admit, answer

The buyer seals the job's plaintext under the conversation key with a fresh
nonce and signs a kind 25910 event with one `p` tag naming the Pylon and an
`expiration` tag. The Pylon handles each delivered event in `py_event`'s
order. The example's `pylon_take` is the same order without the socket:

<!-- source: nostr/examples/pylon-mesh.coax take -->
```jai
// What the Pylon does with one EVENT, in py_event's order, at clock now:
// verify, open the content with its own key, admit, route. The fields it
// reads are kept in Taken for the answer.
Taken :: struct {
  verified: bool; kind: int; tried: bool; opened: u32; jb: PdJob; route: PqRoute;
  id: [64] u8; caller: [64] u8; plain: [4096] u8; plain_n: int; body: [4096] u8;
}
pylon_take :: (event: string, me: string, now: int, t: [] Taken) {
  scratch: [8192] u8;
  ev := nostr_event_verify_vartime(event, scratch[..], false);
  t[0].verified = ev.verified; t[0].kind = ev.kind; t[0].opened = NIP44_PAYLOAD; t[0].tried = false;
  id := ev.id; key := ev.public_key;
  caller := hex_of(key[..], t[0].caller[..]);
  idh := hex_of(id[..], t[0].id[..]);
  if ev.verified && ev.kind == PD_REQUEST {
    raw := ev.original[ev.content_from..<ev.content_to]; payload: [4096] u8;
    k := json_unescape(raw, payload[..]);
    ck := conv_key(PYLON, caller);
    r := open_payload(ck[..], cast(string) payload[0..<k], t[0].plain[..]);
    t[0].tried = true; t[0].opened = r.error; t[0].plain_n = r.written;
  }
  decrypted := t[0].opened == NIP44_OK;
  plaintext := ifx decrypted then cast(string) t[0].plain[0..<t[0].plain_n] else "";
  env := PdEnvelope.{kind = ev.kind, id = idh, pubkey = caller, created_at = ev.created_at,
                     tags = ev.original[ev.tags_from..<ev.tags_to], plaintext = plaintext};
  t[0].jb = pd_admit(env, me, now, t[0].body[..]);
  q: [1] PqQueue;
  t[0].route = pq_route(ev.verified, ev.kind, decrypted, cast(int) t[0].jb.code, cast(int) t[0].jb.type,
                        t[0].jb.answerable, false, pq_free(q[..]));
}
```

`pq_route` decides what happens next. A job that failed verification or
decryption is ignored: no answer goes back, because the Pylon cannot know
that the signer or the content is real. A job that decrypted but breaks a
rule gets a 27010 refusal when it is answerable (signer, request id and
attempt known); otherwise it too is ignored.

<!-- source: psionic/lib/pylon/queue.coax route -->
```jai
// What a received event gets. verified: its id and BIP-340 signature checked;
// decrypted: NIP-44 opened its content; code, type and answerable are
// pd_admit's; free is pq_free.
pq_route :: (verified:bool,kind:int,decrypted:bool,code:int,type:int,answerable:bool,draining:bool,free:int)->PqRoute #export_c {
    if !verified || kind!=PQ_REQUEST || !decrypted {return PqRoute.ignore;}
    if code!=PQ_NONE {return ifx answerable then PqRoute.refuse else PqRoute.ignore;}
    if type==PQ_CANCEL {return PqRoute.cancel;}
    if type!=PQ_CALL || !answerable {return PqRoute.ignore;}
    if draining || free<1 {return PqRoute.busy;}
    return PqRoute.queue;
}
```

A queued job's body goes to the engine. The engine's 200 body becomes the
26910 result through `pd_result`, which adds the Pylon's door, its key as
`provider` and the served identity. `py_seal` seals that result to the
buyer under the same conversation key with a new nonce, and `pd_template`
addresses it with one `p` tag naming the buyer and one `e` tag naming the
job event.

The buyer checks the answer the same way: the signature verifies, the
signer is the Pylon from the beacon, the `p` tag names the buyer, the `e`
tag names the job, and the payload opens under its conversation key.

## A request sealed to another provider

Only a holder of one of the two secret keys can compute the conversation
key. When a buyer seals a job to provider `5G`, the Pylon derives
`ck(3, 7G)` and the MAC check refuses the payload with `NIP44_MAC` before
any plaintext exists. That holds whatever the `p` tag says:

- With `p` naming `5G`, the payload fails the MAC check and `pd_admit` also
  says `not_addressed`.
- With `p` naming the Pylon, the payload still fails the MAC check. With no
  plaintext, `pd_admit` reads `malformed`, and the route is `ignore`.

In both cases `pq_route` returns `ignore`, so the Pylon sends no answer.
Only provider `5G`'s key opens the payload.

## The mesh's opening keys

A Psionic mesh sometimes needs a third party, such as a checker that replays
work, to confirm that one ciphertext held a given plaintext. Giving the
checker the conversation key would let it open every message between the
two parties. `opening.coax` gives out less: the 76 message keys that
NIP-44 derives for one nonce (`HKDF-expand(ck, nonce, 76)`: the ChaCha20
key, the ChaCha20 nonce and the HMAC key).

- `mo_keys(ck, payload)` returns those 76 bytes for the nonce in
  `payload`.
- `mo_authenticate(keys, payload, plaintext)` checks the MAC over the exact
  signed ciphertext, decrypts, checks the padding and compares the result
  with `plaintext` in constant time. It never returns the plaintext.

The keys open only the message whose nonce they came from. Against another
payload the MAC check fails. `opening.coax` never signs and never handles a
signing key.

## The example

The example is [nostr/examples/pylon-mesh.coax](../examples/pylon-mesh.coax).
It derives the three keys, runs the NIP-42 exchange, signs and reads the
beacon, seals the job, has the Pylon take it and answer it, opens the answer
as the buyer, checks it with the opening keys, and then shows the
refusals. No socket, relay or engine runs. The engine's answer is
`decision.coax`'s `PD_ANSWER`. BIP-340's aux and both NIP-44 nonces are
fixed so that every run prints the same bytes. `run.coax` draws all three
from the OS.

<!-- source: nostr/examples/pylon-mesh.coax example -->
```jai
#load "../../psionic/lib/pylon/host.coax";
#load "../../psionic/lib/pylon/queue.coax";
#load "../../psionic/lib/mesh/opening.coax";
#load "../../coax/lib/nostr/nip42-event.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#load "../../coax/lib/nostr/nip44-key.coax";
#import "Basic";

// The clock is fixed at the time of decision.coax's fixtures
// (2026-10-07T16:26:40Z). The engine is not run: its answer is
// decision.coax's PD_ANSWER, chapter 20's 200 body.
CLOCK :: 1791400000;
// Published test keys: the scalars 3, 7 and 5. Never sign with a real key
// in an example. The Pylon holds 3, the buyer 7, another provider 5.
PYLON :: "0000000000000000000000000000000000000000000000000000000000000003";
BUYER :: "0000000000000000000000000000000000000000000000000000000000000007";
OTHER :: "0000000000000000000000000000000000000000000000000000000000000005";
// BIP-340's aux and the two NIP-44 nonces are fixed so that the output is
// the same on every run. run.coax draws all three from the OS.
AUX :: "0000000000000000000000000000000000000000000000000000000000000000";
NONCE_JOB :: "0101010101010101010101010101010101010101010101010101010101010101";
NONCE_ANSWER :: "0202020202020202020202020202020202020202020202020202020202020202";
RELAY_URL :: "wss://relay.example.com/";
CHALLENGE :: "pylon-challenge-1";

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
// The x-only public key of a test secret, through the signer: an event
// signed here and read back.
public_of :: (secret: string, out: [] u8) -> string {
  s: [1024] u8; w: [1024] u8; e := nostr_event_read(sign(secret, 0, 1, "[]", "", s[..]), w[..], false);
  key := e.public_key;
  return hex_of(key[..], out);
}
// One event signed with a test key: event_sign.coax's stages, as py_sign
// runs them, with AUX in place of the OS's 32 bytes.
sign :: (secret: string, created_at: int, kind: int, tags: string, content: string, out: [] u8) -> string {
  key := bytes_of(secret); aux := bytes_of(AUX);
  work: [1] NostrSignWork; scratch: [16384] u8; k := EVENT_MORE;
  while k == EVENT_MORE {
    k = nostr_event_sign_step(key[..], aux[..], created_at, kind, tags, content, work[..], scratch[..], out, false);
  }
  #assert k > 0;
  return cast(string) out[0..<k];
}
// A conversation key from a secret and a peer's x-only key (nip44-key.coax).
conv_key :: (secret: string, peer: string) -> [32] u8 {
  key := bytes_of(secret); x := bytes_of(peer); ecdh: [1] EcdhScratch; ck: [32] u8;
  #assert nip44_conversation_key_from_keys_checked(key[..], x[..], ecdh[..], ck[..], false) == NIP44_OK;
  return ck;
}
// NIP-44 v2 of plaintext under ck with a fixed nonce, as py_seal does.
seal :: (ck: [] u8, nonce_hex: string, plaintext: string, out: [] u8) -> string {
  nonce := bytes_of(nonce_hex); work: [8192] u8; pt: [4096] u8;
  n := plaintext.count;
  r := nip44_encrypt_checked(NIP44_COMPAT, ck, nonce[..], bytes_in(plaintext, pt[..]),
         work[0..<nip44_encrypt_work_size(NIP44_COMPAT, n)], out[0..<nip44_payload_size(NIP44_COMPAT, n)], false);
  #assert r.error == NIP44_OK;
  return cast(string) out[0..<r.written];
}
// Opens a payload under ck, as py_open_payload does: the result.
open_payload :: (ck: [] u8, payload: string, out: [] u8) -> Nip44Result {
  work: [8192] u8; size := nip44_decrypt_work_size(NIP44_COMPAT, payload.count);
  if size < 0 || size > work.count { return Nip44Result.{error = NIP44_CAPACITY}; }
  return nip44_decrypt_checked(NIP44_COMPAT, ck, payload, work[0..<size], out, false);
}
// A string's bytes copied into out, as a [] u8 view.
bytes_in :: (s: string, out: [] u8) -> [] u8 {
  for i: 0..<s.count { out[i] = s[i]; }
  return out[0..<s.count];
}
nip44_name :: (e: u32) -> string {
  names := [13] string.["NIP44_OK", "NIP44_VERSION", "NIP44_PAYLOAD", "NIP44_BASE64", "NIP44_MAC", "NIP44_PADDING",
    "NIP44_LENGTH", "NIP44_CAPACITY", "NIP44_KEY", "NIP44_OUTPUT", "NIP44_ALIAS", "NIP44_PROFILE", "NIP44_INTERNAL"];
  return names[e];
}
ne_name :: (e: int) -> string {
  names := [9] string.["NE_OK", "NE_JSON", "NE_SHAPE", "NE_LIMIT", "NE_HEX", "NE_OUTPUT", "NE_ALIAS", "NE_ID", "NE_SIGNATURE"];
  return names[e];
}
n42_name :: (e: int) -> string {
  names := [13] string.["N42_OK", "N42_FRAME", "N42_URL", "N42_STALE", "N42_NO_CHALLENGE", "N42_EVENT",
    "N42_KIND", "N42_TIME", "N42_TAGS", "N42_RELAY", "N42_CHALLENGE", "N42_OUTPUT", "N42_MACHINE"];
  return names[e];
}

// book:begin take
// What the Pylon does with one EVENT, in py_event's order, at clock now:
// verify, open the content with its own key, admit, route. The fields it
// reads are kept in Taken for the answer.
Taken :: struct {
  verified: bool; kind: int; tried: bool; opened: u32; jb: PdJob; route: PqRoute;
  id: [64] u8; caller: [64] u8; plain: [4096] u8; plain_n: int; body: [4096] u8;
}
pylon_take :: (event: string, me: string, now: int, t: [] Taken) {
  scratch: [8192] u8;
  ev := nostr_event_verify_vartime(event, scratch[..], false);
  t[0].verified = ev.verified; t[0].kind = ev.kind; t[0].opened = NIP44_PAYLOAD; t[0].tried = false;
  id := ev.id; key := ev.public_key;
  caller := hex_of(key[..], t[0].caller[..]);
  idh := hex_of(id[..], t[0].id[..]);
  if ev.verified && ev.kind == PD_REQUEST {
    raw := ev.original[ev.content_from..<ev.content_to]; payload: [4096] u8;
    k := json_unescape(raw, payload[..]);
    ck := conv_key(PYLON, caller);
    r := open_payload(ck[..], cast(string) payload[0..<k], t[0].plain[..]);
    t[0].tried = true; t[0].opened = r.error; t[0].plain_n = r.written;
  }
  decrypted := t[0].opened == NIP44_OK;
  plaintext := ifx decrypted then cast(string) t[0].plain[0..<t[0].plain_n] else "";
  env := PdEnvelope.{kind = ev.kind, id = idh, pubkey = caller, created_at = ev.created_at,
                     tags = ev.original[ev.tags_from..<ev.tags_to], plaintext = plaintext};
  t[0].jb = pd_admit(env, me, now, t[0].body[..]);
  q: [1] PqQueue;
  t[0].route = pq_route(ev.verified, ev.kind, decrypted, cast(int) t[0].jb.code, cast(int) t[0].jb.type,
                        t[0].jb.answerable, false, pq_free(q[..]));
}
// book:end take
show_taken :: (label: string, t: [] Taken) {
  opened := ifx t[0].tried then nip44_name(t[0].opened) else "not tried";
  print("%: verified %, kind %, opened %, admit %, route %\n", label, t[0].verified, t[0].kind,
        opened, code_text(t[0].jb.code), t[0].route);
}
code_text :: (c: PdCode) -> string { return ifx c == PdCode.none then "none" else pd_code_name(c); }

main :: () {
  print("== 1. Keys\n");
  b1: [64] u8; b2: [64] u8; b3: [64] u8;
  pylon := public_of(PYLON, b1[..]); buyer := public_of(BUYER, b2[..]); other := public_of(OTHER, b3[..]);
  print("Pylon (3G)          %\nbuyer (7G)          %\nother provider (5G) %\n", pylon, buyer, other);
  print("Pylon is decision.coax's PD_WORKER: %; buyer is PD_CALLER: %\n", pylon == PD_WORKER, buyer == PD_CALLER);

  print("\n== 2. NIP-42: the Pylon authenticates to the relay\n");
  relay_n42: [1] Nip42; relay_out: [256] u8;
  #assert nip42_init(relay_n42[..], RELAY_URL) == N42_OK;
  f := nip42_issue(relay_n42[..], 1, CHALLENGE, relay_out[..]);
  challenge_frame := cast(string) relay_out[0..<f.written];
  print("relay sends %\n", challenge_frame);
  rl: [1] Relay; n42: [1] Nip42;
  g := rl_start(rl[..], 0); opened := rl_opened(rl[..], g, 0);
  #assert nip42_init(n42[..], RELAY_URL) == N42_OK;
  #assert nip42_input(rl[..], n42[..], g, challenge_frame) >= 0;
  tb: [512] u8; t := nip42_template(n42[..], g, CLOCK, tb[..]);
  print("Pylon's AUTH template: kind %, created_at %, tags %\n", t.kind, t.created_at, t.tags);
  ab: [1024] u8; auth := sign(PYLON, t.created_at, t.kind, t.tags, t.content, ab[..]);
  as: [4096] u8; admitted := nip42_admit(relay_n42[..], 1, CLOCK, auth, as[..], false);
  ak := admitted.public_key; akh: [64] u8;
  print("relay: nip42_admit -> %, key %\n", n42_name(admitted.error), hex_of(ak[..], akh[..]));
  wb: [1024] u8;
  wrong := sign(PYLON, CLOCK, 22242, "[[\"relay\",\"wss://relay.example.com/\"],[\"challenge\",\"old-challenge\"]]", "", wb[..]);
  print("relay: an answer to another challenge -> %\n", n42_name(nip42_admit(relay_n42[..], 1, CLOCK, wrong, as[..], false).error));

  print("\n== 3. The beacon: what is checked and what is only claimed\n");
  lanes: [1] string; services: [1] PbService; settlement: [1] string;
  beacon := pb_fixture(lanes[..], services[..], settlement[..]);
  bs: [4096] u8; bt: [8192] u8; bn := ph_beacon(beacon, PbAuth.{}, bs[..], bt[..]);
  #assert bn > 0;
  // Sign the template's four fields, as py_publish does after py_fields.
  bc: [4096] u8; tpl := py_fields_of(cast(string) bt[0..<bn], bc[..]);
  be: [8192] u8; signed_beacon := sign(PYLON, tpl.created_at, tpl.kind, tpl.tags, tpl.content, be[..]);
  print("beacon: kind %, % bytes signed, tags %\n", tpl.kind, signed_beacon.count, tpl.tags);
  read_beacon("buyer reads it", signed_beacon, CLOCK + 10);
  oe: [8192] u8; by_other := sign(OTHER, tpl.created_at, tpl.kind, tpl.tags, tpl.content, oe[..]);
  read_beacon("the same template signed by 5G", by_other, CLOCK + 10);
  read_beacon("the Pylon's beacon at observed_at + 301", signed_beacon, CLOCK + 301);

  print("\n== 4. The buyer seals a job to the Pylon\n");
  ck_buyer := conv_key(BUYER, pylon); ck_pylon := conv_key(PYLON, buyer);
  hb: [64] u8; print("conversation key (7, 3G) %\n", hex_of(ck_buyer[..], hb[..]));
  print("conversation key (3, 7G) is the same: %\n", cb_ct_equal(ck_buyer[..], ck_pylon[..]) == CB_MAX64);
  print("plaintext: % bytes, padded to %\n", PD_CALL.count, nip44_calc_padded_len(PD_CALL.count));
  pj: [4096] u8; payload := seal(ck_buyer[..], NONCE_JOB, PD_CALL, pj[..]);
  print("payload: % characters, starts %\n", payload.count, payload[0..<48]);
  jt: [8192] u8; jn := pt_template(jt[..], buyer, CLOCK, PD_REQUEST, PD_TAGS, payload);
  jtpl := py_fields_of(cast(string) jt[0..<jn], bc[..]);
  je: [8192] u8; request := sign(BUYER, jtpl.created_at, jtpl.kind, jtpl.tags, jtpl.content, je[..]);
  print("job event: kind %, tags %, id %\n", PD_REQUEST, PD_TAGS, request[7..<71]);

  print("\n== 5. The Pylon takes it\n");
  taken: [1] Taken;
  pylon_take(request, pylon, CLOCK, taken[..]);
  show_taken("job", taken[..]);
  jb := taken[0].jb;
  print("request %, attempt %, % ms to the deadline\n", jb.request, jb.attempt, pd_left_ms(jb, CLOCK));
  print("engine body: %\n", cast(string) taken[0].body[0..<jb.body]);

  print("\n== 6. The Pylon answers, sealed to the buyer\n");
  receipt := "{\"attempt_id\":\"r-1#1\",\"schema\":\"openagents.receipt.execution.v1\",\"transport\":\"relay\"}";
  rb: [4096] u8; result := pd_result(rb[..], jb, PD_ANSWER, "studio-gpu", pylon, "psionic-pylon-v1", PB_IDENTITY, receipt);
  print("pd_result: %, % bytes\n", code_text(result.code), result.written);
  result_text := cast(string) rb[0..<result.written];
  pa: [8192] u8; sealed := seal(ck_pylon[..], NONCE_ANSWER, result_text, pa[..]);
  at: [8192] u8; an := pd_template(at[..], pylon, CLOCK + 2, PD_RESULT, jb, sealed);
  atpl := py_fields_of(cast(string) at[0..<an], bc[..]);
  ae: [8192] u8; answer := sign(PYLON, atpl.created_at, atpl.kind, atpl.tags, atpl.content, ae[..]);
  print("answer event: kind %, tags %\n", atpl.kind, atpl.tags);
  // The buyer: verify, check the signer and the p and e tags, open.
  vs: [8192] u8; v := nostr_event_verify_vartime(answer, vs[..], false);
  vk := v.public_key; vkh: [64] u8;
  vtags := v.original[v.tags_from..<v.tags_to];
  print("buyer: verified %, signer is the Pylon %, p names the buyer %, e names the job %\n", v.verified,
        hex_of(vk[..], vkh[..]) == pylon, pt_tag(vtags, "p").v[1] == buyer, pt_tag(vtags, "e").v[1] == request[7..<71]);
  ob: [8192] u8; got := open_payload(ck_buyer[..], sealed, ob[..]);
  print("buyer opens it: %, the result is pd_result's bytes %\n", nip44_name(got.error), cast(string) ob[0..<got.written] == result_text);
  print("%\n", cast(string) ob[0..<got.written]);

  print("\n== 7. The mesh's opening capability\n");
  keys: [76] u8;
  #assert mo_keys(ck_pylon[..], sealed, keys[..]);
  rk: [4096] u8; result_bytes := bytes_in(result_text, rk[..]); cb: [4096] u8;
  kh: [152] u8; print("message keys for the answer (76 bytes): %...\n", hex_of(keys[0..<16], kh[..]));
  print("a checker with only these keys: the answer opens to the result %\n", mo_authenticate(keys[..], sealed, result_bytes));
  edited: [4096] u8; for i: 0..<result_text.count { edited[i] = result_text[i]; }
  edited[result_text.count - 3] = 'X';
  print("the same keys against an edited result: %\n", mo_authenticate(keys[..], sealed, edited[0..<result_text.count]));
  print("the same keys against the job's payload: %\n", mo_authenticate(keys[..], payload, bytes_in(PD_CALL, cb[..])));

  print("\n== 8. Refusals\n");
  // A job sealed to the other provider and addressed to it.
  ck_other := conv_key(BUYER, other);
  po: [4096] u8; to_other := seal(ck_other[..], NONCE_JOB, PD_CALL, po[..]);
  ot: [512] u8; other_tags := tags_for(other, ot[..]);
  oe2: [8192] u8; other_job := sign(BUYER, CLOCK, PD_REQUEST, other_tags, to_other, oe2[..]);
  pylon_take(other_job, pylon, CLOCK, taken[..]);
  show_taken("sealed to 5G, p names 5G", taken[..]);
  ck_5 := conv_key(OTHER, buyer); o5: [4096] u8;
  print("  the provider 5G opens it: %\n", nip44_name(open_payload(ck_5[..], to_other, o5[..]).error));
  // The same ciphertext, p naming the Pylon.
  me2: [8192] u8; misaddressed := sign(BUYER, CLOCK, PD_REQUEST, PD_TAGS, to_other, me2[..]);
  pylon_take(misaddressed, pylon, CLOCK, taken[..]);
  show_taken("sealed to 5G, p names the Pylon", taken[..]);
  // The job with one signature digit changed.
  fb: [8192] u8; forged := flip_signature(request, fb[..]);
  pylon_take(forged, pylon, CLOCK, taken[..]);
  fs: [8192] u8;
  print("signature digit changed: % from the verifier\n", ne_name(nostr_event_verify_vartime(forged, fs[..], false).error));
  show_taken("  so the Pylon", taken[..]);
  // The job at the edge of its expiration tag.
  pylon_take(request, pylon, CLOCK + 59, taken[..]);
  show_taken("at expiration - 1", taken[..]);
  pylon_take(request, pylon, CLOCK + 60, taken[..]);
  show_taken("at expiration", taken[..]);
  sb: [1024] u8; sn := pd_status(sb[..], taken[0].jb, PdStatus.error, taken[0].jb.code, -1);
  print("  27010 body: %\n", cast(string) sb[0..<sn]);
}

// The four signed fields of a template, as run.coax's py_fields reads them.
py_fields_of :: (template: string, content: [] u8) -> PyFields {
  r: PyFields; j: [1] Json; json_start(j[..], template);
  #assert json_next(j[..]) == JSON_OBJECT;
  while true {
    tok := json_next(j[..]); if tok == JSON_OBJECT_END { break; }
    key := json_text(j[..]); tok = json_next(j[..]);
    if json_is(key, "created_at") { r.created_at = json_int(json_text(j[..])).value; }
    else if json_is(key, "kind") { r.kind = json_int(json_text(j[..])).value; }
    else if json_is(key, "tags") { r.tags = pt_raw(j[..], tok); }
    else if json_is(key, "content") { n := json_unescape(json_text(j[..]), content); r.content = cast(string) content[0..<n]; }
    else { raw := pt_raw(j[..], tok); }
  }
  return r;
}
PyFields :: struct { created_at: int; kind: int; tags: string; content: string; }
// A beacon as a buyer reads it: verify, then ph_read_beacon with the
// verified signer, then freshness on the buyer's clock.
read_beacon :: (label: string, event: string, received: int) {
  s: [16384] u8; e := nostr_event_verify_vartime(event, s[..], false);
  key := e.public_key; kh: [64] u8; signer := hex_of(key[..], kh[..]);
  c: [8192] u8; k := json_unescape(e.original[e.content_from..<e.content_to], c[..]);
  r := ph_read_beacon(e.kind, signer, e.original[e.tags_from..<e.tags_to], cast(string) c[0..<k]);
  if r.code != PbCode.ok { print("%: verified %, read %\n", label, e.verified, r.code); return; }
  fresh := pb_fresh(r, received, PbHeld.{});
  print("%: verified %, read %, fresh %, routable %\n", label, e.verified, r.code, fresh, pb_routable(r, fresh));
  if fresh == PbFresh.fresh {
    print("  checked: signer = provider, capability %:pylon/decision, x = SHA-256 of the content\n", py_short_of(signer));
    print("  claimed: % % % GB, % of % slots free, identity %\n", r.family, r.tier, r.memory_gb, r.free, r.total, r.identity);
  }
}
py_short_of :: (s: string) -> string { return s[0..<12]; }
tags_for :: (to: string, out: [] u8) -> string {
  n := json_raw(out, 0, "[[\"p\",\""); n = json_raw(out, n, to); n = json_raw(out, n, "\"],[\"expiration\",\"1791400060\"]]");
  return cast(string) out[0..<n];
}
// A copy of the event with the first digit of its signature changed.
flip_signature :: (event: string, out: [] u8) -> string {
  for i: 0..<event.count { out[i] = event[i]; }
  at := pb_index(event, "\"sig\":\"") + 7;
  out[at] = ifx event[at] == '0' then cast(u8) '1' else cast(u8) '0';
  return cast(string) out[0..<event.count];
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/pylon-mesh.coax --run-steps 250000000
```

The output matches the values worked out by hand: the three keys, the
conversation key, the padded length 320, the 516-character payload starting
`AgEB`, the job id `8366669c...3cf9c248` and 60000 ms left:

```text
== 1. Keys
Pylon (3G)          f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9
buyer (7G)          5cbdf0646e5db4eaa398f365f2ea7a0e3d419b7e0330e39ce92bddedcac4f9bc
other provider (5G) 2f8bde4d1a07209355b4a7250a5c5128e88b84bddc619ab7cba8d569b240efe4
Pylon is decision.coax's PD_WORKER: true; buyer is PD_CALLER: true

== 2. NIP-42: the Pylon authenticates to the relay
relay sends ["AUTH","pylon-challenge-1"]
Pylon's AUTH template: kind 22242, created_at 1791400000, tags [["relay","wss://relay.example.com"],["challenge","pylon-challenge-1"]]
relay: nip42_admit -> N42_OK, key f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9
relay: an answer to another challenge -> N42_CHALLENGE

== 3. The beacon: what is checked and what is only claimed
beacon: kind 30200, 1221 bytes signed, tags [["d","studio-gpu"],["t","oa:pylon-beacon:v1"],["x","a200d2649d0fdabc548e70175b18b0f274674b21f587fae8bc4c5d1be9797664"],["expiration","1791400300"]]
buyer reads it: verified true, read ok, fresh fresh, routable true
  checked: signer = provider, capability f9308a019258:pylon/decision, x = SHA-256 of the content
  claimed: gpu medium 16 GB, 1 of 1 slots free, identity elephant-1@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
the same template signed by 5G: verified true, read provider
the Pylon's beacon at observed_at + 301: verified true, read ok, fresh stale, routable false

== 4. The buyer seals a job to the Pylon
conversation key (7, 3G) 87911e47109d035c08d1704433a66f396d3c4a28b9a26149f329f7e0dbcc4316
conversation key (3, 7G) is the same: true
plaintext: 269 bytes, padded to 320
payload: 516 characters, starts AgEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBt+8M
job event: kind 25910, tags [["p","f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"],["expiration","1791400060"]], id 8366669c1337ef543d110d5f79d762829f77137f6da54a50f3cfe5873cf9c248

== 5. The Pylon takes it
job: verified true, kind 25910, opened NIP44_OK, admit none, route queue
request r-1, attempt 1, 60000 ms to the deadline
engine body: {"model":"elephant-1","state":{"ticket":"I was billed twice"},"questions":{"refund":{"type":"noul","instructions":"Does the customer ask for money back?"}}}

== 6. The Pylon answers, sealed to the buyer
pd_result: none, 560 bytes
answer event: kind 26910, tags [["p","5cbdf0646e5db4eaa398f365f2ea7a0e3d419b7e0330e39ce92bddedcac4f9bc"],["e","8366669c1337ef543d110d5f79d762829f77137f6da54a50f3cfe5873cf9c248"]]
buyer: verified true, signer is the Pylon true, p names the buyer true, e names the job true
buyer opens it: NIP44_OK, the result is pd_result's bytes true
{"attempt":1,"outcome":"answered","receipt":{"attempt_id":"r-1#1","schema":"openagents.receipt.execution.v1","transport":"relay"},"request":"r-1","requires":[],"response":{"answers":{"refund":{"type":"noul","noul":0.93}},"latency_ms":412,"model":"elephant-1","service":{"door":"pylon:studio-gpu","identity":"elephant-1@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","provider":"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9","version":"psionic-pylon-v1"},"usage":null},"type":"result","v":"openagents.systemone.v1"}

== 7. The mesh's opening capability
message keys for the answer (76 bytes): 0305260fb3798f3d538cb9a103a7ab15...
a checker with only these keys: the answer opens to the result true
the same keys against an edited result: false
the same keys against the job's payload: false

== 8. Refusals
sealed to 5G, p names 5G: verified true, kind 25910, opened NIP44_MAC, admit not_addressed, route ignore
  the provider 5G opens it: NIP44_OK
sealed to 5G, p names the Pylon: verified true, kind 25910, opened NIP44_MAC, admit malformed, route ignore
signature digit changed: NE_SIGNATURE from the verifier
  so the Pylon: verified false, kind 25910, opened not tried, admit malformed, route ignore
at expiration - 1: verified true, kind 25910, opened NIP44_OK, admit none, route queue
at expiration: verified true, kind 25910, opened NIP44_OK, admit stale, route refuse
  27010 body: {"attempt":1,"code":"stale","message":"the request is outside its time window","request":"r-1","requires":[],"status":"error","type":"status","v":"openagents.systemone.v1"}
```

The refusals:

- The relay refuses an AUTH event that answers another challenge with
  `N42_CHALLENGE`.
- The beacon signed by `5G` reads `provider`: its content names `3G`.
- The Pylon's beacon at `observed_at + 301` is `stale`, so it is not
  routable.
- A job sealed to `5G` fails the Pylon's MAC check (`NIP44_MAC`) whether
  `p` names `5G` or the Pylon, and is ignored. Provider `5G` opens it.
- A job with one signature digit changed fails verification
  (`NE_SIGNATURE`). The Pylon does not try to open it and ignores it.
- At `1791400059` the job is queued. At `1791400060`, its expiration, it is
  stale and refused with the 27010 body shown.
- The opening keys refuse an edited result and the job's payload.

The book's checks run this program in debug and release builds and compare
the output with [nostr/fixtures/pylon-mesh.out](../fixtures/pylon-mesh.out).

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/pylon-mesh.coax](../examples/checks/pylon-mesh.coax),
which loads the example, so running the example doesn't run them. They
compare the example's values with values computed independently in Python
(integers, `hashlib`, `hmac`, and a ChaCha20 written from RFC 8439 for this
check), and test the two rules in this chapter's "Done means":

- `x(21G)`, the conversation key from both sides, and the buyer's key with
  `5G`;
- the job's whole payload and its event id;
- the answer's whole payload and its 76 opening keys;
- a job sealed to four other providers' keys is never opened by the Pylon,
  and each provider it was sealed to opens it;
- the time window from `1791399968` to `1791400061`, one second at a time,
  through `pd_admit` and `pq_route`, and its four edges through the whole
  signed and sealed path.

<!-- source: nostr/examples/checks/pylon-mesh.coax check -->
```jai
// Values computed with Python's integers, hashlib and hmac, and a ChaCha20
// written from RFC 8439 for this check: x(21G) by affine double-and-add, the
// conversation key as HMAC-SHA256("nip44-v2", x), the message keys as
// HKDF-expand(ck, nonce, 76), and the payload and NIP-01 event id built from
// those. They share no code with the Coax units.
PY_X21 :: "352bbf4a4cdd12564f93fa332ce333301d9ad40271f8107181340aef25be59d5";
PY_CK :: "87911e47109d035c08d1704433a66f396d3c4a28b9a26149f329f7e0dbcc4316";
PY_CK_OTHER :: "2d954e97ab55c8f2cad76128c302d3df98fc9957aca7a533bd133f093b0a7c24";
PY_JOB_PAYLOAD :: "AgEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBt+8MkZun2BPjGrnN1aQUXhOK4hl8PP6RcWV5Gkxr44ZZHc2DovjUcImkxz9MAqIZHzCpgbOEtLlIDsh5Lspv2zRX03G8cnlXKUkGBJJKza3aW512bbCYv5qTNJMiqYLjlUJANQPCa1BY/KhLDHq9mcEfsVD8Mz39hakvZUtErJiU0sAAFHj6csDOTgEuX4giYVKPAzWzsT983sCAubVFDn43w9gNfhk1wWXAV0dB14jfpGg2g+l9vxeXYoWvNm1tK09r5kn2AKI97WcIJ0xHdRqDr658gjX2xuxgUfDTvOlx1UN/wBzs2klCOOzskjFy3alm+CzktkY4T3EmAb/jeZsBbXFxPRyPy3h/gy6u3Tei12n+TibZF8nXMsjd5fs4FFxzR4lV9UtTwGHHsQnMiWGRqYu3QXWCCha+QKBJXL6Ufm78V7k7QTYdWLiDiSW/Hpbu/hqASuya8iCjVPWQHYjQ";
PY_JOB_ID :: "8366669c1337ef543d110d5f79d762829f77137f6da54a50f3cfe5873cf9c248";
PY_ANSWER_PAYLOAD :: "AgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgIC7yuihQpAOmhx7pU1k70HwBkai2dEie+F7kZ/FRP1L5XtHVzbXqdH1SKdZfcotaRuRbFT+Uu8bLdnzosgjJadcl9oq5rDitHhBYzaMVzC8DXSTM+tiUwQxW7afxINjINl6UxMN8q7oAAr7somNnBRJCX1/sfC91vg3xtXrEg45JCobP+VEwuLKtrkZ5thxhI6I2csTDIdionwv7E7xYZqNZUEOaxmHB9KnKUz3YE/d23Ugd2W35q2aDAfDA99Z1Isbb4CHV48kSFozpDyKXxaFX+D7eIGHLv6oVJXlxDY+mHHbVQUwn4EfI2fubjyMD40cUNqSjRIwk+GxCG0/xn4mlHZUoKesu4EABdlfZg+tfJkvUGSZZSphc2HU77B5rKvG0D546v6voJjRiYnmUSZLNcNjtIjoGAwja0f1lQUAM4aZ1bMioRldhLX4ixEBFkGRR99sLqqmPc1Bfb4DxwXzmtBNI0qNKkFl2wAXdtvq6w7//sIP/X/73Lz04SsjczEM6FA/O5SxssQu5A3sYrgNOMv7lYPROJvisJ1qPIfM2E2cyUG8FSqBx7q3u1iS2tAxpn+b6VNpguYEZrDm2FFuvNrwcXYjOPFave79SV44h2klPdRDKxctGE+it8/Q4x9jgmKvx4Cabmp3k+RcNkVm1q6szu61QM7RRngrmLgvKV6kCninlinnuOAdFPRCVtG5T0Kg9L9Uiztrn10Uqq/34kpEg77v6BcKmPPIMUj0RISEx1055ySIiDLbf0qZYHDWepJaqaRqaR7RdbcBHytPk6yi6kkjtAmCCEe3O4hS2yJ9MgpWzE87BdTRx0/sBQ+u7NRXUngB5VBS/laTx1eozzf++CYIp2PfcFf5FZ+hefIxfjs2VxVcfBUHd/96lB/mZo=";
PY_ANSWER_KEYS :: "0305260fb3798f3d538cb9a103a7ab1527068570a7478f8ce4d11d812a3f310082bcb7ce9852940973bf43d4bbeccaf22785797a8e371f6b1f46b2ae896b51e228cf3ac31b377b2d502965de";
RECEIPT :: "{\"attempt_id\":\"r-1#1\",\"schema\":\"openagents.receipt.execution.v1\",\"transport\":\"relay\"}";

hex_is :: (bytes: [] u8, want: string) -> bool { h: [256] u8; return hex_of(bytes, h[..]) == want; }

#test "both sides derive the conversation key Python derives from x(21G)" #steps 1000000000 {
  pylon_x := PD_WORKER; buyer_x := PD_CALLER;
  a := conv_key(BUYER, pylon_x); b := conv_key(PYLON, buyer_x);
  #assert hex_is(a[..], PY_CK) && hex_is(b[..], PY_CK);
  // The ECDH x alone, then HKDF-extract: the two steps nip44-key.coax joins.
  ecdh: [1] EcdhScratch; key := bytes_of(PYLON); peer := bytes_of(buyer_x);
  #assert ecdh_stage(key[..], peer[..], ecdh[..]) == EC_OK && hex_is(ecdh[0].staged[..], PY_X21);
  c := conv_key(BUYER, "2f8bde4d1a07209355b4a7250a5c5128e88b84bddc619ab7cba8d569b240efe4");
  #assert hex_is(c[..], PY_CK_OTHER);
}
#test "the job's payload and event id are Python's" #steps 1000000000 {
  ck := conv_key(BUYER, PD_WORKER);
  pj: [4096] u8; payload := seal(ck[..], NONCE_JOB, PD_CALL, pj[..]);
  #assert payload == PY_JOB_PAYLOAD;
  // 516 = 4 * ceil((1 + 32 + 2 + 320 + 32) / 3), worked in the chapter.
  #assert payload.count == 516 && nip44_calc_padded_len(PD_CALL.count) == 320;
  e: [8192] u8; request := sign(BUYER, CLOCK, PD_REQUEST, PD_TAGS, payload, e[..]);
  #assert request[7..<71] == PY_JOB_ID;
}
#test "the answer's payload and its opening keys are Python's" #steps 1000000000 {
  body: [4096] u8; jb := pd_admit(pd_env(PD_CALL), PD_WORKER, CLOCK, body[..]);
  rb: [4096] u8; r := pd_result(rb[..], jb, PD_ANSWER, "studio-gpu", PD_WORKER, "psionic-pylon-v1", PB_IDENTITY, RECEIPT);
  #assert r.code == PdCode.none && r.written == 560;
  ck := conv_key(PYLON, PD_CALLER);
  pa: [8192] u8; sealed := seal(ck[..], NONCE_ANSWER, cast(string) rb[0..<r.written], pa[..]);
  #assert sealed == PY_ANSWER_PAYLOAD;
  keys: [76] u8; #assert mo_keys(ck[..], sealed, keys[..]) && hex_is(keys[..], PY_ANSWER_KEYS);
}
// The Pylon opens a job only when it was sealed to the Pylon's key: sealed
// to any of these other providers, the MAC check refuses it and the route
// is ignore, whatever the p tag says.
OTHERS :: [4] string.[
  "0000000000000000000000000000000000000000000000000000000000000002",
  "0000000000000000000000000000000000000000000000000000000000000005",
  "0000000000000000000000000000000000000000000000000000000000000004",
  "b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef"];
#forall "a job sealed to another provider is never opened" i: 0..<4 #steps 2000000000 {
  xb: [64] u8; x := public_of(OTHERS[i], xb[..]);
  ck := conv_key(BUYER, x);
  po: [4096] u8; sealed := seal(ck[..], NONCE_JOB, PD_CALL, po[..]);
  e: [8192] u8; request := sign(BUYER, CLOCK, PD_REQUEST, PD_TAGS, sealed, e[..]);
  t: [1] Taken; pylon_take(request, PD_WORKER, CLOCK, t[..]);
  #assert t[0].verified && t[0].tried && t[0].opened == NIP44_MAC && t[0].route == PqRoute.ignore;
  // The provider it was sealed to opens it.
  mine := conv_key(OTHERS[i], PD_CALLER); out: [4096] u8;
  r := open_payload(mine[..], sealed, out[..]);
  #assert r.error == NIP44_OK && cast(string) out[0..<r.written] == PD_CALL;
}
// The freshness window of the hand calculation: created_at 1791400000,
// expiration 1791400060, PD_AHEAD 30. Queued from 1791399970 to 1791400059.
#forall "the job is queued exactly inside its window" dt: -32..<62 {
  body: [4096] u8; jb := pd_admit(pd_env(PD_CALL), PD_WORKER, CLOCK + dt, body[..]);
  q: [1] PqQueue;
  route := pq_route(true, PD_REQUEST, true, cast(int) jb.code, cast(int) jb.type, jb.answerable, false, pq_free(q[..]));
  inside := dt >= -30 && dt <= 59;
  #assert (route == PqRoute.queue) == inside;
  #assert inside || (jb.code == PdCode.stale && route == PqRoute.refuse);
}
// The same edges through the whole path: the signed, sealed job verified,
// opened and admitted.
#test "the signed job at the window's four edges" #steps 2000000000 {
  ck := conv_key(BUYER, PD_WORKER);
  pj: [4096] u8; payload := seal(ck[..], NONCE_JOB, PD_CALL, pj[..]);
  e: [8192] u8; request := sign(BUYER, CLOCK, PD_REQUEST, PD_TAGS, payload, e[..]);
  edges := [4] int.[-31, -30, 59, 60];
  for dt: edges {
    t: [1] Taken; pylon_take(request, PD_WORKER, CLOCK + dt, t[..]);
    inside := dt == -30 || dt == 59;
    #assert t[0].verified && t[0].opened == NIP44_OK && (t[0].route == PqRoute.queue) == inside;
    #assert inside || t[0].route == PqRoute.refuse;
  }
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/pylon-mesh.coax --run-steps 250000000
coax/bin/coax mutate nostr/examples/checks/pylon-mesh.coax --root-only --run-steps 250000000
```

`coax test` runs 512 checks, including the loaded files' own checks, in
about 50 seconds.

### The units' records

Each `#authority` unit has its own mutation record. These were not rerun
for this chapter, because the files did not change:

- `queue.coax`: 173 mutants, 173 killed, in
  [psionic-ch19-2026-10-29.md](../../docs/verification/psionic-ch19-2026-10-29.md).
- `beacon.coax`, `decision.coax`, `relays.coax`, `template.coax`: in
  [psionic-ch19-protocol-2026-10-29.md](../../docs/verification/psionic-ch19-protocol-2026-10-29.md).
- `opening.coax`: 61 of 61 selected mutants killed, in
  [psionic-mesh-private-foundation-2026-10-30.md](../../docs/verification/psionic-mesh-private-foundation-2026-10-30.md).
- NIP-42 and NIP-44: chapters 38 and 20, and
  [2026-10-29-nostr-nip42.md](../../docs/verification/2026-10-29-nostr-nip42.md),
  [2026-10-29-nip44-v2.md](../../docs/verification/2026-10-29-nip44-v2.md)
  and
  [2026-10-30-nip44-conversation-key.md](../../docs/verification/2026-10-30-nip44-conversation-key.md).

`coax test psionic/lib/pylon/run.coax` stops at the compiler's compile-time
step limit before it runs a check. That is why the example loads the units
`run.coax` wires together and not `run.coax` itself.

### Through the Coax relay

The example runs without a relay. Psionic chapter 24's end-to-end harness,
`psionic/tests/pylon/pylon_e2e.py`, runs the real program: it builds the
Coax relay and the Pylon with ASan and UBSan, starts the relay on
`127.0.0.1`, an engine and the Pylon, which checks its engine against its
reference set first (Psionic
[chapter 218](../../psionic/book/ch218-a-pylon-checks-itself-before-serving.md)),
and a buyer written with independent code (`@noble/curves` and
`nostr-effect`'s NIP-44). The Pylon authenticates with NIP-42, publishes its
beacon, and answers sealed jobs. The relay is restarted midway.
Its recorded runs, over `ws://` and `wss://`, are in
[psionic-ch19-2026-10-29.md](../../docs/verification/psionic-ch19-2026-10-29.md)
and Psionic chapter 24's "Run it" section. The run with the self-check, on
2026-10-30, is in
[psionic-ch159-2026-10-30.md](../../docs/verification/psionic-ch159-2026-10-30.md):
40 checks passed, ASan and UBSan were silent in the relay and the Pylon, and
the self-check passed before the Pylon published its relay list and beacon.
The harness needs the buyer's
`node_modules`, which are not installed in this checkout, so it was not
rerun for this chapter on 2026-11-01.

## Summary

- A Pylon answers its relay's NIP-42 challenge on every connection before
  it publishes anything.
- A buyer verifies a beacon's signature, its binding to the signer and its
  freshness. The hardware class, the slots and the model identity are
  claims.
- A job is NIP-44 sealed to the Pylon's key. The Pylon verifies, opens,
  admits and routes it, and seals its answer back under the same
  conversation key with a new nonce.
- A job sealed to another provider fails the MAC check and is ignored.
- The mesh's opening keys prove what one ciphertext held without giving out
  the conversation key.

## Dependencies

- [82. Pylons, environments and
  deploys](ch082-pylons-environments-and-deploys.md)
- [20. NIP-44 encrypted payloads](ch020-nip-44-encrypted-payloads.md)
- [38. NIP-42 authentication](ch038-nip-42-authentication.md)
- [Psionic chapter 24, A Pylon answers
  decisions](../../psionic/book/ch024-a-pylon-answers-decisions.md)

## Sources

- [OpenAgents NIP-PYLON](../nips/openagents/NIP-PYLON.md)
- [NIP-44](../nips/official/44.md)
- [NIP-42](../nips/official/42.md)
- [RFC 8439](https://www.rfc-editor.org/rfc/rfc8439), ChaCha20 and
  Poly1305, for the check's ChaCha20.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
