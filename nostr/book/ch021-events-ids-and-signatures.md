# 21. Events, IDs and signatures

The event is the only object in Nostr. This chapter reads an event, recomputes
its ID from chapter 17's serialization, verifies its signature with Bitcoin chapter
23, and builds new signed events with Bitcoin chapter 24.

Part II, The event model. Result: The NIP-01 event: `id`, `pubkey`,
`created_at`, `kind`, `tags`, `content`, `sig`; recomputing the ID, verifying
the signature, and building signed events separately from reading them.

## What this chapter teaches

- The seven fields and their exact types: 32-byte lowercase hex, integer
  seconds, integer kind 0 to 65535, array of string arrays.
- Verification order: shape, ID recomputation, signature. A parsed event is
  not a verified one.
- Signing: build the serialization, hash, sign, and check the result before
  release.
- Read and sign are separate files, so read-only consumers never link a
  signer.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [event.coax](../../coax/lib/nostr/event.coax) | `nostr_event_read(text, scratch, overlap)` | Admits the JSON, decodes the seven fields, rebuilds the preimage in `scratch` and compares its SHA-256 with `id` (`#authority`) |
| | `nostr_event_verify_vartime(text, scratch, overlap)` | `nostr_event_read`, then the BIP-340 signature over the 32-byte ID; the only entry that sets `verified` |
| | `ne_parse`, `ne_serialize` | The field reader and the preimage writer that the two entries share |
| [event_sign.coax](../../coax/lib/nostr/event_sign.coax) | `nostr_event_sign_step(key, aux, created_at, kind, tags, content, work, scratch, out, overlap)` | Builds and signs one event in stages; `EVENT_MORE` until the event's length or `EVENT_REFUSED` (`#authority`) |
| [schnorr.coax](../../coax/lib/crypto/schnorr.coax) | `nostr_verify_vartime` | Bitcoin chapter 23's verifier over exactly 32 bytes of message |
| [schnorr_sign.coax](../../coax/lib/crypto/schnorr_sign.coax) | `schnorr_sign_step` | Bitcoin chapter 24's staged signer |

`event.coax` loads the JSON reader and admission policy of chapter 17, the
hex codec of Bitcoin chapter 1 and the verifier of Bitcoin chapter 23. It does not load the
signer. `event_sign.coax` loads `event.coax` and the signer. A relay or a
client that only reads events loads `event.coax` alone, so no code that
touches a secret key is compiled into it.

Nothing allocates. The caller passes the event text and a scratch buffer for
the preimage, and gets back a `NostrEvent` record by value. The record holds
the decoded `id`, `pubkey` and `sig` as byte arrays, `created_at` and `kind`
as integers, and byte ranges into the original text for `tags` and
`content`. The original text must stay alive and unchanged while those
ranges are used.

## The event

NIP-01 defines the event as one JSON object:

```jai
{
  "id": <32-bytes lowercase hex-encoded sha256 of the serialized event data>,
  "pubkey": <32-bytes lowercase hex-encoded public key of the event creator>,
  "created_at": <unix timestamp in seconds>,
  "kind": <integer between 0 and 65535>,
  "tags": [[<arbitrary string>...], ...],
  "content": <arbitrary string>,
  "sig": <64-bytes lowercase hex of the signature of the sha256 hash of the serialized event data>
}
```

`ne_parse` admits exactly this:

| Field | JSON type | Admitted values | Refusal |
| --- | --- | --- | --- |
| `id` | string | 64 lowercase hex digits, decoded to 32 bytes | `NE_HEX` |
| `pubkey` | string | 64 lowercase hex digits, decoded to 32 bytes | `NE_HEX` |
| `created_at` | number | an integer from 0 to 2^53 - 1 | `NE_JSON` above 2^53 - 1, `NE_SHAPE` below 0 |
| `kind` | number | an integer from 0 to 65535 | `NE_SHAPE` |
| `tags` | array | up to 2,000 arrays, each of up to 16 strings | `NE_SHAPE` |
| `content` | string | any string the JSON reader admits | `NE_JSON` |
| `sig` | string | 128 lowercase hex digits, decoded to 64 bytes | `NE_HEX` |

Each field must appear exactly once. A missing field is `NE_SHAPE`. A
repeated one, even when the second spelling uses an escape such as
`"kind"`, is refused by chapter 17's admission policy as a duplicate
key, which the reader returns as `NE_JSON`. A field that isn't one of the
seven is allowed and skipped: NIP-01 says later NIPs may add fields, and
some relays add their own. It never enters the ID, so it can't change what
was signed.

Uppercase hex is refused even though the generic hex decoder of Bitcoin chapter 1
accepts it. NIP-01 says lowercase, and two spellings of one ID would give one
event two keys in a store. `ne_lower_hex` adds one test before the decoder:
every lowercase hex digit (`0` to `9`, `a` to `f`) has ASCII bit 5 set, and
`A` to `F` don't. The decoder rejects every other byte on its own.

`created_at` and `kind` must be integer tokens. `1.0`, `1e3` and `-0` are
refused by the admission policy, which also refuses any integer beyond
2^53 - 1 (chapter 17). The reader then refuses negative values, and a
`kind` above 65535.

### Verification order

`nostr_event_verify_vartime` runs three steps, each only when the previous
one passed:

1. **Shape.** Admit the JSON and decode the seven fields. Any refusal here
   is `NE_JSON`, `NE_SHAPE`, `NE_HEX` or `NE_LIMIT`.
2. **ID.** Write the preimage `[0,pubkey,created_at,kind,tags,content]`
   from the decoded fields, with chapter 17's writer, hash it with SHA-256,
   and compare the hash with the decoded `id`. A difference is `NE_ID`.
3. **Signature.** Verify `sig` as a BIP-340 signature by `pubkey` over the
   32 bytes of the ID. A failure is `NE_SIGNATURE`.

`nostr_event_read` stops after step 2 and never sets `verified`. A record
that parsed is not a verified record, and code that decides anything from
an event must call the verify entry. The order also means a relay refuses a malformed or mislabeled event
before it pays for the curve arithmetic.

The preimage is written from the decoded values, not copied from the input.
The input may have spaces between tokens, fields in any order, an unknown
field, or `h` where the writer would put `h`. All of these give the same
preimage, and so the same ID. The reader keeps the original bytes too: a
relay stores and forwards what the author sent (chapter 17).

A signature that verifies proves only that the holder of `pubkey`'s private
key signed these fields. It says nothing about whether the event is recent,
whether that author is allowed to post where it was sent, or whether a
later event replaces it. Those checks belong to later chapters.

## Worked example: build, sign and check one event

Chapter 17 worked out the preimage of a small event by hand:

| Field | Value |
| --- | --- |
| `pubkey` | `dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659`, BIP-340 vector 1's public key |
| `created_at` | 1761686400 |
| `kind` | 1 |
| `tags` | `[["t","nostr"]]` |
| `content` | the 8 bytes `68 69 0a 22 78 22 5c 01`: `h`, `i`, line feed, `"`, `x`, `"`, `\`, `0x01` |

The preimage is these 119 bytes:

```text
[0,"dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659",1761686400,1,[["t","nostr"]],"hi\n\"x\"\\\u0001"]
```

Its SHA-256, and so the event's ID, is:

```text
ef9fc293e97c60875a6af22b70820a645a9d7dabeaba2e221ba32d96dc50a1a3
```

### Sign

The signature is Bitcoin chapter 24's BIP-340 signature over the 32 bytes of the ID,
not over the preimage. Sign with BIP-340 vector 1's published secret key
`b7e15162…5190cfef` and its auxiliary bytes `00…01`. These are test values
from the BIP; a real key never appears in an example. A pure-Python BIP-340
signer written from the BIP gives this 64-byte signature, `x(R)` then `s`:

```text
434d7839f0efb03875c96af21ea07b0428860602ad04729cf57d78f336902ef3
1e3e4b743b49aec045db97a91b8c70012da8fa21679ee701b88572dc05079494
```

### Write the event

The event is the seven fields as one JSON object. `event_sign.coax` writes
them in NIP-01's order with no whitespace, using the same string writer as
the preimage. Count its bytes piece by piece:

| Piece | Bytes | Running total |
| --- | ---: | ---: |
| `{"id":` and the 64-digit ID in quotes | 6 + 66 | 72 |
| `,"pubkey":` and the 64-digit key in quotes | 10 + 66 | 148 |
| `,"created_at":1761686400` | 14 + 10 | 172 |
| `,"kind":1` | 8 + 1 | 181 |
| `,"tags":[["t","nostr"]]` | 8 + 15 | 204 |
| `,"content":"hi\n\"x\"\\\u0001"` | 11 + 19 | 234 |
| `,"sig":` and the 128-digit signature in quotes | 7 + 130 | 371 |
| `}` | 1 | 372 |

The signed event is 372 bytes.

### Check

Verification repeats the work from the other side:

1. Decode the seven fields. The `content` string's 19 bytes unescape to the
   original 8.
2. Rebuild the preimage from the decoded fields: the same 119 bytes as
   above. Hash them: `ef9fc293…dc50a1a3`, equal to the `id` field.
3. Verify the signature by `dff1d77f…502ba659` over those 32 bytes.

Now change one field at a time and follow the steps. Each change stops at the
first step that notices it:

| Change | Step 1 | Step 2 | Step 3 | Status |
| --- | --- | --- | --- | --- |
| `id` first digit `e` to `f` | passes | hash `ef9f…` is not `ff9f…` | not run | `NE_ID` |
| `content` `h` to `H` | passes | the preimage changes, so the hash changes | not run | `NE_ID` |
| `kind` 1 to 2 | passes | the preimage changes | not run | `NE_ID` |
| `sig` first digit `4` to `5` | passes | passes | `R` is a different point | `NE_SIGNATURE` |
| `pubkey` in uppercase | `D` is not lowercase hex | not run | not run | `NE_HEX` |
| `kind` removed | six fields | not run | not run | `NE_SHAPE` |
| `"kind":1` twice | duplicate key | not run | not run | `NE_JSON` |
| `created_at` -1 | negative | not run | not run | `NE_SHAPE` |
| `created_at` 2^53 | not a safe integer | not run | not run | `NE_JSON` |
| `created_at` 2^53 - 1 | passes | the preimage changes | not run | `NE_ID` |
| `kind` 65536 | above 65535 | not run | not run | `NE_SHAPE` |
| tag `["t",1]` | a tag value is not a string | not run | not run | `NE_SHAPE` |

Changing `content`, `kind` or `created_at` to another admitted value always
gives `NE_ID`, never `NE_SIGNATURE`: the ID check comes first, and any change to a signed
field changes the hash. To get past step 2, a forger has to replace the ID
too, and then the signature is over a different message and step 3 refuses.

## Signing in stages

`nostr_event_sign_step` takes the same inputs on every call and keeps its
progress in one `NostrSignWork`. One event takes eight calls:

1. Stage 0, the key: run the signer's key stage (call 1 of Bitcoin chapter 24's six)
   and keep the x-only public key.
2. Stage 1, the ID: write the preimage into `scratch` with the same pieces as
   `ne_serialize`, and hash it.
3. Stage 2, the signature: call the signer until it returns `SIGN_DONE`. The
   key stage already ran, so this takes the signer's five remaining calls:
   nonce, equation and the three fixed-work self-check stages.
4. Stage 3, the event: write the JSON into `out`, then run
   `nostr_event_read` on those bytes. The event is released only when the
   reader recomputes the same ID from them.

The last step means the signer can't publish an event that the reader
would refuse. If `event_sign.coax`'s writer and `event.coax`'s serializer
ever disagreed on one escape, or a caller changed `content` between calls,
the read check fails, `out` is cleared and the result is `EVENT_REFUSED`.
Inputs are checked before stage 0: `created_at` from 0 to 2^53 - 1, `kind`
from 0 to 65535, and `tags` one complete JSON value. Any refusal after that
clears the workspace, which holds the signer's secret intermediates.

The `aux` bytes must be 32 fresh random bytes from the host (Bitcoin chapter 18). The
example uses BIP-340's fixed test value so that its output can be compared
byte for byte.

## The example

The example is [nostr/examples/events.coax](../examples/events.coax). It
signs chapter 17's event with the test key and prints the public key and ID
after the first two calls, reads the result back and prints the preimage,
and verifies it. It then edits the signed event in thirteen ways that refuse
and two that are accepted. Last, it verifies an event captured from a real
relay, four altered copies of it, and the example event published in
NIP-70. The two captured events are at the end of the file, outside the
listing.

<!-- source: nostr/examples/events.coax example -->
```jai
#load "../../coax/lib/nostr/event_sign.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
print_hex :: (label: string, bytes: [] u8) {
  text: [128] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// Decode hex that the program itself supplies; it must be valid.
decode_hex :: (text: string, out: [] u8) {
  r := hex_decode_checked(text, out, false);
  #assert r.error == NC_OK && r.written == out.count;
}
STATUS :: [9] string.["ok", "json", "shape", "limit", "hex", "output", "alias", "id", "signature"];

// Sign one event: call nostr_event_sign_step until it stops returning
// EVENT_MORE. Returns the event's length in out, or EVENT_REFUSED.
sign_event :: (key: [] u8, aux: [] u8, created_at: int, kind: int, tags: string,
               content: string, out: [] u8) -> int {
  work: [1] NostrSignWork; scratch: [1024] u8;
  calls := 0; result := EVENT_MORE;
  while result == EVENT_MORE {
    result = nostr_event_sign_step(key, aux, created_at, kind, tags, content,
                                   work[..], scratch[..], out, false);
    calls += 1;
    if calls == 1 { print_hex("call 1, key: pubkey", work[0].pub[..]); }
    if calls == 2 { print_hex("call 2, id", work[0].id[..]); }
  }
  print("calls: %, result: %\n", calls, result);
  return result;
}

JA :: [9] string.["ok", "policy", "size", "json", "string", "duplicate key", "keys", "work", "number"];

// Print what reading and verifying gave: the status, the admission rule
// for a JSON refusal, and the ID when the event verified.
report :: (label: string, e: NostrEvent) {
  print("%: %", label, STATUS[e.error]);
  if e.error == NE_JSON { print(" (%)", JA[e.admission]); }
  if e.verified {
    text: [64] u8; id := e.id;
    r := hex_encode_checked(cast(string) id[..], text[..], false);
    print(", id %", cast(string) text[0..<r.written]);
  }
  print("\n");
}
// Verify text with old replaced by new, and report it.
check_edit :: (label: string, text: string, old: string, new: string) {
  edit: [1024] u8; scratch: [1024] u8;
  report(label, nostr_event_verify_vartime(replace(text, old, new, edit[..]), scratch[..], false));
}

// text with the first occurrence of old replaced by new, written into out.
replace :: (text: string, old: string, new: string, out: [] u8) -> string {
  at := -1;
  for i: 0..<text.count - old.count + 1 {
    if at < 0 && text[i..<i + old.count] == old { at = i; }
  }
  #assert at >= 0;
  n := json_raw(out, 0, text[0..<at]);
  n = json_raw(out, n, new);
  n = json_raw(out, n, text[at + old.count..<text.count]);
  #assert n > 0;
  return cast(string) out[0..<n];
}

// Chapter 17's worked event, signed with BIP-340 vector 1's published test
// key and aux. Never sign with a real key in an example.
KEY_HEX :: "b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef";
AUX_HEX :: "0000000000000000000000000000000000000000000000000000000000000001";
// Its content is h i LF " x " \ 0x01, the bytes chapter 17 escapes.
CONTENT_HEX :: "68690a2278225c01";

main :: () {
  print("sign chapter 17's event\n");
  key: [32] u8; aux: [32] u8;
  decode_hex(KEY_HEX, key[..]); decode_hex(AUX_HEX, aux[..]);
  content: [8] u8; decode_hex(CONTENT_HEX, content[..]);
  out: [1024] u8;
  n := sign_event(key[..], aux[..], 1761686400, 1, "[[\"t\",\"nostr\"]]", cast(string) content[..], out[..]);
  signed := cast(string) out[0..<n];
  print("event: %\n", signed);

  print("read it back\n");
  scratch: [1024] u8;
  r := nostr_event_read(signed, scratch[..], false);
  print("read: %, verified %\n", STATUS[r.error], r.verified);
  print("preimage (% bytes): %\n", r.written, cast(string) scratch[0..<r.written]);
  print_hex("computed id", r.computed_id[..]);
  v := nostr_event_verify_vartime(signed, scratch[..], false);
  report("verify", v);
  print("verified: %, kind %, created_at %, tags %\n", v.verified, v.kind, v.created_at, v.tags);
  print_hex("sig", v.signature[..]);

  print("refusals\n");
  check_edit("id first digit e -> f", signed, "\"id\":\"ef", "\"id\":\"ff");
  check_edit("content h -> H", signed, "\"content\":\"hi", "\"content\":\"Hi");
  check_edit("kind 1 -> 2", signed, "\"kind\":1,", "\"kind\":2,");
  check_edit("sig first digit 4 -> 5", signed, "\"sig\":\"43", "\"sig\":\"53");
  check_edit("sig last digit 4 -> 5", signed, "9494\"}", "9495\"}");
  check_edit("pubkey in uppercase hex", signed, "dff1d77f", "DFF1D77F");
  check_edit("kind missing", signed, ",\"kind\":1,", ",");
  check_edit("kind twice", signed, ",\"kind\":1,", ",\"kind\":1,\"kind\":1,");
  check_edit("created_at -1", signed, "1761686400", "-1");
  check_edit("created_at 2^53 - 1", signed, "1761686400", "9007199254740991");
  check_edit("created_at 2^53", signed, "1761686400", "9007199254740992");
  check_edit("kind 65536", signed, "\"kind\":1,", "\"kind\":65536,");
  check_edit("tag value 1", signed, "[[\"t\",\"nostr\"]]", "[[\"t\",1]]");
  print("accepted\n");
  check_edit("extra field relay", signed, "{\"id\"", "{\"relay\":\"wss://relay.example.com\",\"id\"");
  check_edit("spaces between tokens", signed, ",\"kind\":1,", " , \"kind\" : 1 , ");

  print("a captured event\n");
  c := nostr_event_verify_vartime(CAPTURED, scratch[..], false);
  report("relay.openagents.com, 2026-10-28", c);
  print("bytes %, kind %, created_at %, tags %, preimage bytes %\n",
        CAPTURED.count, c.kind, c.created_at, c.tags, c.written);
  check_edit("id first digit 8 -> 9", CAPTURED, "\"id\":\"8", "\"id\":\"9");
  check_edit("content \"\" -> \"x\"", CAPTURED, "\"content\":\"\"", "\"content\":\"x\"");
  check_edit("sig last bit flipped", CAPTURED, "d71eb\"", "d71ea\"");
  check_edit("kind 30617 -> 99999", CAPTURED, "30617", "99999");

  print("NIP-70's example event\n");
  p := nostr_event_verify_vartime(NIP70, scratch[..], false);
  report("NIP-70", p);
  print_hex("claimed id ", p.id[..]);
  print_hex("computed id", p.computed_id[..]);
}
```

To run it:

```shell
coax/bin/coax run nostr/examples/events.coax --run-steps 250000000
```

The signer's compile-time tables need more than the default step budget,
hence `--run-steps`. The program prints:

```text
sign chapter 17's event
call 1, key: pubkey: dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659
call 2, id: ef9fc293e97c60875a6af22b70820a645a9d7dabeaba2e221ba32d96dc50a1a3
calls: 8, result: 372
event: {"id":"ef9fc293e97c60875a6af22b70820a645a9d7dabeaba2e221ba32d96dc50a1a3","pubkey":"dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659","created_at":1761686400,"kind":1,"tags":[["t","nostr"]],"content":"hi\n\"x\"\\\u0001","sig":"434d7839f0efb03875c96af21ea07b0428860602ad04729cf57d78f336902ef31e3e4b743b49aec045db97a91b8c70012da8fa21679ee701b88572dc05079494"}
read it back
read: ok, verified false
preimage (119 bytes): [0,"dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659",1761686400,1,[["t","nostr"]],"hi\n\"x\"\\\u0001"]
computed id: ef9fc293e97c60875a6af22b70820a645a9d7dabeaba2e221ba32d96dc50a1a3
verify: ok, id ef9fc293e97c60875a6af22b70820a645a9d7dabeaba2e221ba32d96dc50a1a3
verified: true, kind 1, created_at 1761686400, tags 1
sig: 434d7839f0efb03875c96af21ea07b0428860602ad04729cf57d78f336902ef31e3e4b743b49aec045db97a91b8c70012da8fa21679ee701b88572dc05079494
refusals
id first digit e -> f: id
content h -> H: id
kind 1 -> 2: id
sig first digit 4 -> 5: signature
sig last digit 4 -> 5: signature
pubkey in uppercase hex: hex
kind missing: shape
kind twice: json (duplicate key)
created_at -1: shape
created_at 2^53 - 1: id
created_at 2^53: json (number)
kind 65536: shape
tag value 1: shape
accepted
extra field relay: ok, id ef9fc293e97c60875a6af22b70820a645a9d7dabeaba2e221ba32d96dc50a1a3
spaces between tokens: ok, id ef9fc293e97c60875a6af22b70820a645a9d7dabeaba2e221ba32d96dc50a1a3
a captured event
relay.openagents.com, 2026-10-28: ok, id 813d9fb02bd78ce24200ba986b62ef727f0c032f712cff82241bd465f4bc9d21
bytes 681, kind 30617, created_at 1793214821, tags 7, preimage bytes 428
id first digit 8 -> 9: id
content "" -> "x": id
sig last bit flipped: signature
kind 30617 -> 99999: shape
NIP-70's example event
NIP-70: id
claimed id : cb8feca582979d91fe90455867b34dbf4d65e4b86e86b3c68c368ca9f9eef6f2
computed id: f017727ad7c6b4c872639506b75ad7c8f85e0896f71610fb491625e1c0b8b2e6
```

What the output shows:

- The public key, the ID and the 372-byte event are the values worked out
  above, and the signature is the Python signer's, byte for byte.
- `nostr_event_read` reports `ok` and `verified false`. Only the verify
  entry sets `verified`.
- Every refusal has the status that the table above predicts.
- An unknown `relay` field and spaces between tokens are accepted with the
  same ID. Neither is part of the preimage.
- The captured event is the Coax repository announcement (kind 30617) that
  `wss://relay.openagents.com` sent on 2026-10-28. Its 681 bytes verify. A
  changed ID digit and one content byte added each stop at the ID, a flipped
  signature bit at the signature, and kind 99999 at the shape.
- NIP-70's example event doesn't verify. Its claimed ID is not the SHA-256
  of its fields: the event in the NIP was edited by hand. A published
  example is not a test vector until something has checked it.

The book's checks run this program in debug and release builds and compare
the output with [nostr/fixtures/events.out](../fixtures/events.out).

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/events.coax](../examples/checks/events.coax), which
loads the example. They compare the code with values that come from outside
it:

- the whole signed event, byte for byte, with the event written by a
  pure-Python BIP-340 signer and Python's `json.dumps`, which also verified
  the signature;
- the library's own Python-signed event from
  [event_sign_vectors.py](../../coax/tests/nostr/event_sign_vectors.py), with
  escapes in its tags and content and an empty tag, signed through all eight
  stages;
- the captured event's preimage length and ID with Python's, and its four
  altered copies with their statuses;
- the ID that Python computes for NIP-70's example, which differs from the
  one printed in the NIP.

<!-- source: nostr/examples/checks/events.coax check -->
```jai
// Chapter 17's event signed by a pure-Python BIP-340 signer (hashlib and
// integers, written from the BIP; it shares no code with Coax) with the same
// key and aux, then written by Python's json.dumps(..., separators=(",", ":"),
// ensure_ascii=False). The same script verifies the signature.
PY_EVENT :: "{\"id\":\"ef9fc293e97c60875a6af22b70820a645a9d7dabeaba2e221ba32d96dc50a1a3\",\"pubkey\":\"dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659\",\"created_at\":1761686400,\"kind\":1,\"tags\":[[\"t\",\"nostr\"]],\"content\":\"hi\\n\\\"x\\\"\\\\\\u0001\",\"sig\":\"434d7839f0efb03875c96af21ea07b0428860602ad04729cf57d78f336902ef31e3e4b743b49aec045db97a91b8c70012da8fa21679ee701b88572dc05079494\"}";
// Python's preimage length and SHA-256 for the captured event, and its
// SHA-256 of the NIP-70 example's preimage.
PY_CAPTURED_PREIMAGE :: 428;
PY_CAPTURED_ID :: "813d9fb02bd78ce24200ba986b62ef727f0c032f712cff82241bd465f4bc9d21";
PY_NIP70_ID :: "f017727ad7c6b4c872639506b75ad7c8f85e0896f71610fb491625e1c0b8b2e6";

// Sign to the end without printing; returns the length in out or -1.
sign_quietly :: (key: [] u8, aux: [] u8, created_at: int, kind: int, tags: string,
                 content: string, out: [] u8) -> int {
  work: [1] NostrSignWork; scratch: [1024] u8; result := EVENT_MORE;
  while result == EVENT_MORE {
    result = nostr_event_sign_step(key, aux, created_at, kind, tags, content,
                                   work[..], scratch[..], out, false);
  }
  return result;
}
id_is :: (id: [32] u8, text: string) -> bool {
  want: [32] u8; decode_hex(text, want[..]);
  for i: 0..<32 { if id[i] != want[i] { return false; } }
  return true;
}
verify :: (text: string) -> NostrEvent {
  scratch: [1024] u8;
  return nostr_event_verify_vartime(text, scratch[..], false);
}

#test "chapter 17's event signs to the Python signer's bytes" #steps 400000000 {
  key: [32] u8; aux: [32] u8; content: [8] u8; out: [1024] u8;
  decode_hex(KEY_HEX, key[..]); decode_hex(AUX_HEX, aux[..]); decode_hex(CONTENT_HEX, content[..]);
  n := sign_quietly(key[..], aux[..], 1761686400, 1, "[[\"t\",\"nostr\"]]", cast(string) content[..], out[..]);
  #assert n == PY_EVENT.count && cast(string) out[0..<n] == PY_EVENT;
  e := verify(PY_EVENT);
  #assert e.error == NE_OK && e.verified && e.written == 119;
  #assert id_is(e.id, "ef9fc293e97c60875a6af22b70820a645a9d7dabeaba2e221ba32d96dc50a1a3");
}
// The library's own Python-signed event (event_sign_vectors.py): escapes in
// tags and content, an empty tag, and a 589-byte result.
#test "the library's Python-signed event signs byte for byte" #steps 400000000 {
  key := ES_KEY; aux := ES_AUX; tags := ES_TAGS; content := ES_CONTENT; out: [1024] u8;
  n := sign_quietly(key[..], aux[..], ES_CREATED, ES_KIND, cast(string) tags[..], cast(string) content[..], out[..]);
  #assert n == ES_EVENT.count;
  for i: 0..<n { #assert out[i] == ES_EVENT[i]; }
}
#test "the captured event verifies with Python's ID" #steps 400000000 {
  e := verify(CAPTURED);
  #assert e.error == NE_OK && e.verified && e.written == PY_CAPTURED_PREIMAGE;
  #assert id_is(e.id, PY_CAPTURED_ID) && id_is(e.computed_id, PY_CAPTURED_ID);
  #assert e.kind == 30617 && e.created_at == 1793214821 && e.tags == 7;
}
#test "each altered copy of the captured event refuses with its own status" #steps 400000000 {
  edit: [1024] u8;
  #assert verify(replace(CAPTURED, "\"id\":\"8", "\"id\":\"9", edit[..])).error == NE_ID;
  #assert verify(replace(CAPTURED, "\"content\":\"\"", "\"content\":\"x\"", edit[..])).error == NE_ID;
  #assert verify(replace(CAPTURED, "d71eb\"", "d71ea\"", edit[..])).error == NE_SIGNATURE;
  #assert verify(replace(CAPTURED, "30617", "99999", edit[..])).error == NE_SHAPE;
}
#test "NIP-70's example event has Python's computed ID, not its claimed one" {
  scratch: [1024] u8;
  e := nostr_event_read(NIP70, scratch[..], false);
  #assert e.error == NE_ID && !e.verified && e.written == 0;
  #assert id_is(e.computed_id, PY_NIP70_ID) && !id_is(e.id, PY_NIP70_ID);
}
#test "id_is notices a change in the first or last byte" {
  id: [32] u8; zero := "0000000000000000000000000000000000000000000000000000000000000000";
  #assert id_is(id, zero);
  id[0] = 1; #assert !id_is(id, zero);
  id[0] = 0; id[31] = 1; #assert !id_is(id, zero);
}
#test "replace changes exactly the first occurrence" {
  out: [16] u8;
  #assert replace("abab", "ab", "x", out[..]) == "xab";
  #assert replace("abab", "ba", "", out[..]) == "ab";
  #assert replace("ab", "b", "bcd", out[..]) == "abcd";
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/events.coax --run-steps 250000000
coax/bin/coax mutate nostr/examples/checks/events.coax --root-only --run-steps 250000000
```

On 2026-11-01, `coax test` ran 337 checks, including the loaded files' own
checks, in about 75 seconds. Signing chapter 17's event through all eight
stages uses 125 million interpreter steps, the library's event 49 million,
and each verification of the captured event about 77 million. Root-only
mutation of the checks file kills 14 of 14 mutants. The first run had one
survivor, a loop in `id_is` that could start at byte 1; a new check covers
the first and last bytes. The example's own procedures and the library were
not mutated for this chapter.

### The library's own checks

[event.coax](../../coax/lib/nostr/event.coax) loads its checks from
`coax/tests/nostr/event_fast_checks.coax` and `event_checks.coax`: field
names with escapes, the lowercase-hex rule for every byte value, tag limits
at 2,000 tags and 16 values, 41 malformed events and their statuses, unknown
fields kept out of the ID, the empty event, the largest `created_at` and
`kind`, and view and budget refusals. The
[2026-10-28 record](../../docs/verification/coax-nostr-event-2026-10-28.md)
describes the read path and its profile.

[event_sign.coax](../../coax/lib/nostr/event_sign.coax) checks each stage on
its own and the refusals of each input. The
[2026-10-30 record](../../docs/verification/2026-10-30-event-sign.md) reports
290 of 290 checks and 5,564 of 5,564 mutants killed, none marked equivalent.
In the same record, `schnorr_sign.py --round-trips 10000` signed 10,000
random events, each under a fresh key, in the release build, covering
Unicode, control characters, empty tags and content, and edge kinds and
times. All 10,000 agree with the independent Python NIP-01 serializer and
BIP-340 verifier in
[oracle.py](../../tools/coax-nostr/fuzz/oracle.py). The same harness runs
checked and release builds, plain and under ASan and UBSan, and the release
build as Wasm.

The [2026-10-29 NIP-01 page record](../../docs/verification/2026-10-29-nip01-page.md)
runs the captured event and four altered copies through the same reader and
verifier compiled to Wasm, on openagents.com/nip/01.

These runs were not repeated for this chapter. libsecp256k1 has not been run
on events signed in Coax; the independent verifier so far is the Python one.

## Summary

- An event has seven fields: `id`, `pubkey` and `sig` as lowercase hex of
  32, 32 and 64 bytes, `created_at` and `kind` as integers, `tags` as an
  array of arrays of strings, and `content` as a string. Unknown fields are
  allowed and never signed.
- Verification checks the shape, then recomputes the ID from the decoded
  fields, then verifies the BIP-340 signature over the ID. Each refusal has
  its own status, and only the full verify entry sets `verified`.
- A change to any signed field gives `NE_ID`, because the ID check comes
  before the signature.
- Signing writes the preimage, hashes it, signs the 32-byte ID, writes the
  event, and releases it only after the reader recomputes the same ID.
- `event.coax` reads and verifies; `event_sign.coax` signs. A reader never
  links the signer.

## Dependencies

- [Bitcoin chapter 23. Verify a Schnorr signature](../../bitcoin/book/ch023-verify-a-schnorr-signature.md)
- [Bitcoin chapter 24. Sign with Schnorr](../../bitcoin/book/ch024-sign-with-schnorr.md)
- [17. JSON you can sign](ch017-json-you-can-sign.md)
- [Coax chapter 18, The protocol boundary](../../coax/book/ch18-protocol.md)

## Sources

- [NIP-01](../nips/official/01.md), the event format, the serialization
  and the signature rule.
- [NIP-70](../nips/official/70.md), whose example event the example checks.
- [BIP-340](https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki)
  and its pinned
  [test-vectors.csv](../../coax/tests/vectors/upstream/bip340/test-vectors.csv),
  the source of the test key.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
