# 56. Proof of work

A relay can ask for work before it stores an event. NIP-13 measures that
work as the number of leading zero bits of the event ID. To raise it, the
author changes a counter in a `nonce` tag and hashes again until the ID has
enough zeros. This chapter measures difficulty, reads the `nonce` tag, mines
a note with a test key and builds the relay's admission check with its
`pow:` refusals.

Part X, Every NIP. Result: NIP-13 difficulty as the number of leading zero
bits of the event ID, the `nonce` tag with its target, and relay admission by
`pow:` reason.

## What this chapter teaches

- Difficulty: count the leading zero bits of the 32-byte ID.
- The tag `["nonce", counter, target]`, and why the committed target matters
  as much as the work.
- A mining loop over the counter, and the expected number of tries,
  `2^d` for difficulty `d`.
- Relay policy: refuse with `pow: difficulty X is less than Y`, and refuse an
  event whose committed target is below the requirement even when its ID has
  more work.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [nip13.coax](../../coax/lib/nostr/nip13.coax) | `nip13_difficulty(id)` | Leading zero bits of a 32-byte ID |
| | `n13_nonce(e)` | The event's `nonce` tag: found, malformed, and its target |
| | `nip13_admit(text, required, scratch, overlap)` | A relay's admission: read the event, measure the recomputed ID, apply the policy |
| | `nip13_reason(a, out)` | The `OK` message's reason for a refusal |
| | `nip13_mine(pub, created_at, kind, tags, content, target, start, limit, out)` | Try counters until the ID reaches the target |
| [nip13_fixtures.coax](../../coax/tests/nostr/nip13_fixtures.coax) | | NIP-13's example note and notes with Python-computed IDs for the checks |

`nip13.coax` decides admission, so it is marked `#authority`. It builds on
the event reader of chapter 21 (`nostr_event_read`) and the tag walker of
chapter 22 (`nt_start`). It adds no JSON or hash code of its own.

<!-- source: coax/lib/nostr/nip13.coax record -->
```jai
N13Nonce :: struct {error:int; found:bool; target:int=-1;}
N13Admit :: struct {
  error:int=N13_EVENT; required:int;
  difficulty:int=-1;                 // leading zero bits of the recomputed ID
  has_nonce:bool; target:int=-1;     // the committed target, or -1
  id:[32]u8;
}
N13Mine :: struct {error:int=N13_MINE_INPUT; nonce:int; tries:int; difficulty:int; id:[32]u8; tags_n:int;}
```

`N13Admit.target` is `-1` when the event commits to no target.

## Difficulty

NIP-13 defines difficulty as the number of leading zero bits in the NIP-01
ID. Read the ID as 32 bytes from the first. Each zero byte adds 8. The first
nonzero byte adds the number of zero bits above its highest set bit, and
counting stops there. Bits after the first 1 don't count, even when they are
zero.

<!-- source: coax/lib/nostr/nip13.coax difficulty -->
```jai
// NIP-13 difficulty: whole zero bytes count 8 each; the first nonzero byte
// adds the zeros above its highest set bit, and counting stops there.
nip13_difficulty :: (id:[32]u8)->int {
  n:=0;
  for i:0..<32 {
    b:=cast(int)id[i];
    if b!=0 {
      while b<128 {n+=1;b=b*2;}
      return n;
    }
    n+=8;
  }
  return n;
}
```

### Worked example: three IDs by hand

Each hex digit is 4 bits. A `0` digit adds 4. The first nonzero digit adds
3 if it is `1`, 2 if it is `2` or `3`, 1 if it is `4` to `7`, and 0 if it is
`8` to `f`.

1. NIP-13's example note has the ID `000006d8...`. Five `0` digits give
   `5 × 4 = 20` bits. The next digit is `6`, which is `0110` in binary: one
   more zero bit. The difficulty is 21, as NIP-13 says.
2. NIP-13's prose example `002f...`: two `0` digits give 8, and `2` is
   `0010`, two more. The difficulty is 10.
3. NIP-13's prose example `000000000e9d...`: nine `0` digits give 36, and
   `e` is `1110`, no more. The difficulty is 36.

The 21 is checked twice: Python's `hashlib` gives
`000006d8c378af1779d2feebc7603a125d99eca0ccf1085959b307f64e5dd358` as the
SHA-256 of the note's 147-byte preimage, so the published ID matches its
fields, and the example counts 21 bits in it.

## The nonce tag and its target

To mine, the author adds a tag

```jai
["nonce", "776797", "20"]
```

The second value is the counter that the miner changes. The third is the
target difficulty the miner aimed for. Changing the counter changes the
preimage, so it changes the ID; nothing else in the event needs to change.

The target is the important part. A spammer who mines many events at a low
target sometimes gets lucky: one try in `2^8` reaches 8 bits whatever the
miner aimed for. Without a committed target, a relay that requires 8 bits
can't tell that lucky event from an honest one. With it, the relay sees
that the miner aimed for 4 and refuses. An honest miner loses nothing by
committing, so `nip13_admit` refuses an event that commits to no target.
NIP-13 allows this: clients "MAY reject a note matching a target difficulty
if it is missing a difficulty commitment".

`n13_nonce` accepts exactly one `nonce` tag. Its counter is 1 to 20 decimal
digits (any `u64` spelled in decimal). Its target, when present, is 0 to
256 in decimal without leading zeros. A fourth value, a second `nonce` tag,
an empty counter, a target of `257`, `020` or `eight`: each is a malformed
tag. The counter and target must be plain digits; a JSON escape such as
`1` for `1` is refused. The tag's name may be escaped.

<!-- source: coax/lib/nostr/nip13.coax nonce -->
```jai
// The event's nonce tag. Tags were checked by the event reader as arrays
// of strings; a forged record that breaks that refuses as N13_EVENT.
n13_nonce :: (e:NostrEvent)->N13Nonce {
  r:N13Nonce;j:[1]Json;if !nt_start(j[..],e) {return n13_bad(r,N13_EVENT);}
  tok:=json_next(j[..]);
  while tok==JSON_ARRAY {
    at:=0;nonce:=false;
    v:=json_next(j[..]);
    while v==JSON_STRING {
      // The name may be escaped; the counter and target are plain digits.
      raw:=json_text(j[..]);
      if at==0 {
        nonce=json_is(raw,"nonce");
        if nonce && r.found {return n13_bad(r,N13_NONCE);}
        if nonce {r.found=true;}
      } else if nonce {
        if at>2 {return n13_bad(r,N13_NONCE);}
        if at==1 && !n13_digits(raw,N13_MAX_COUNTER) {return n13_bad(r,N13_NONCE);}
        if at==2 {r.target=n13_target(raw);if r.target<0 {return n13_bad(r,N13_NONCE);}}
      }
      at+=1;v=json_next(j[..]);
    }
    if v!=JSON_ARRAY_END {return n13_bad(r,N13_EVENT);}
    if nonce && at<2 {return n13_bad(r,N13_NONCE);}
    tok=json_next(j[..]);
  }
  if tok!=JSON_ARRAY_END {return n13_bad(r,N13_EVENT);}
  return r;
}
```

## Mining

The miner appends `["nonce", "<n>", "<target>"]` to the event's other tags,
writes the NIP-01 preimage `[0,pubkey,created_at,kind,tags,content]`, hashes
it, and counts zero bits. It tries `n = start, start + 1, ...` until the ID
reaches the target or `limit` tries are spent.

<!-- source: coax/lib/nostr/nip13.coax mine -->
```jai
// Try nonces start, start + 1, ... until the ID has target leading zero
// bits or limit tries are spent. created_at stays fixed, so the run is
// deterministic; NIP-13 suggests a live miner also update it. The final
// tags (tags plus the nonce tag) are left in out[0..<tags_n].
nip13_mine :: (pub:[32]u8,created_at:int,kind:int,tags:string,content:string,
               target:int,start:int,limit:int,out:[]u8)->N13Mine {
  m:N13Mine;preimage:[N13_MAX_PREIMAGE]u8;
  if target<1 || target>N13_MAX_BITS || start<0 || limit<1 {return m;}
  if created_at<0 || kind<0 || kind>65535 {return m;}
  if !n13_tags_ok(tags) {return m;}
  while m.tries<limit {
    m.nonce=start+m.tries;m.tries+=1;
    m.tags_n=n13_tags(tags,m.nonce,target,out);
    if m.tags_n==NE_WRITE_FAILED {m.error=N13_MINE_OUTPUT;return m;}
    n:=n13_preimage(pub,created_at,kind,cast(string)out[0..<m.tags_n],content,preimage[..]);
    if n==NE_WRITE_FAILED {m.error=N13_MINE_OUTPUT;return m;}
    hashed:=sha256_hash_checked(preimage[0..<n],m.id[..],false);
    m.difficulty=nip13_difficulty(m.id);
    if m.difficulty>=target {m.error=N13_MINE_OK;return m;}
  }
  m.error=N13_MINE_LIMIT;return m;
}
```

NIP-13 recommends updating `created_at` while mining. This miner keeps it
fixed so that a run is deterministic: the same inputs give the same nonce
and ID every time. A live client can call it again with a new `created_at`
when a run hits its limit. Each try is about one SHA-256 of a 150-byte
preimage. The ID doesn't commit to the signature, so the miner works
without the private key: it needs only the public key. That is why NIP-13
can talk of delegated proof of work.

### Worked example: how many tries

A SHA-256 output behaves like 256 uniform random bits. An ID has at least
`d` leading zero bits when its first `d` bits are all 0, which happens with
probability `2^-d`. The number of tries until the first success has the
geometric distribution, with mean `2^d`.

| Target `d` | Expected tries `2^d` | Tries this example took |
| ---: | ---: | ---: |
| 8 | 256 | 70 |
| 10 | 1,024 | 8 |
| 16 | 65,536 | 1,804 |
| 20 | 1,048,576 | — |

Each extra bit doubles the expected work. The runs above are single
samples: at target 16 this note was lucky. The same luck is what the
committed target guards against.

With the test key, `created_at` 1761686400, kind 1, content `It's just me
mining my own business` and no other tags, counters 0 to 68 fall short and
counter 69 gives `00f042ea...`: two zero digits and then `f`, exactly 8 bits.
The example prints this, and the checks compare it with Python.

## Relay admission

`nip13_admit` takes the event's JSON text, not a parsed record. It reads
the event with `nostr_event_read`, which recomputes the ID from the fields
and refuses a mismatch, so the work it measures is real. It verifies no
signature. Run it before signature verification: one SHA-256 is cheaper
than a BIP-340 check, and a refused event needs no signature check.

The policy, with `required` from 1 to 256, in this order:

1. An event that doesn't read, or whose ID doesn't match its fields,
   refuses as `invalid:`.
2. A malformed `nonce` tag refuses.
3. An ID with fewer than `required` leading zero bits refuses.
4. An event with no committed target refuses.
5. A committed target below `required` refuses, even when the ID has more
   work.

With `required` 0 the relay asks for no work and admits every readable
event without reading its `nonce` tag. A requirement outside 0 to 256 is a
configuration error.

<!-- source: coax/lib/nostr/nip13.coax admit -->
```jai
// A relay's NIP-13 admission. required is 0 (no proof of work asked) to
// 256. The event's ID is recomputed from its fields before it is measured.
nip13_admit :: (text:string,required:int,scratch:[]u8,overlap:bool)->N13Admit {
  a:N13Admit;a.required=required;
  if required<0 || required>N13_MAX_BITS {a.error=N13_POLICY;return a;}
  e:=nostr_event_read(text,scratch,overlap);if e.error!=NE_OK {return a;}
  a.id=e.id;a.difficulty=nip13_difficulty(e.id);
  if required==0 {a.error=N13_OK;return a;}
  nonce:=n13_nonce(e);a.has_nonce=nonce.found;a.target=nonce.target;
  if nonce.error!=N13_OK {a.error=nonce.error;return a;}
  if a.difficulty<required {a.error=N13_DIFFICULTY;return a;}
  // Without a commitment a lucky low-target miner can't be told apart.
  if a.target<0 {a.error=N13_NO_TARGET;return a;}
  if a.target<required {a.error=N13_TARGET;return a;}
  a.error=N13_OK;return a;
}
```

The reason goes in the relay's `OK` message
([chapter 24](ch024-relay-messages.md)). NIP-01 defines the `pow:` prefix
and gives `pow: difficulty 26 is less than 30` as its example; the first
reason below has the same form.

<!-- source: coax/lib/nostr/nip13.coax reason -->
```jai
// The OK message's reason for a refusal: its length in out, 0 for an
// admitted event, or -1 when out is too short.
nip13_reason :: (a:N13Admit,out:[]u8)->int {
  if a.error==N13_OK {return 0;}
  if a.error==N13_DIFFICULTY {
    n:=json_raw(out,0,"pow: difficulty ");n=json_write_int(out,n,a.difficulty);
    n=json_raw(out,n," is less than ");return json_write_int(out,n,a.required);
  }
  if a.error==N13_TARGET {
    n:=json_raw(out,0,"pow: committed target ");n=json_write_int(out,n,a.target);
    n=json_raw(out,n," is less than ");return json_write_int(out,n,a.required);
  }
  if a.error==N13_NO_TARGET {n:=json_raw(out,0,"pow: no committed target, need ");return json_write_int(out,n,a.required);}
  if a.error==N13_NONCE {return json_raw(out,0,"pow: malformed nonce tag");}
  if a.error==N13_EVENT {return json_raw(out,0,"invalid: event id does not match its fields");}
  if a.error==N13_POLICY {return json_raw(out,0,"error: required difficulty is not 0 to 256");}
  return -1;
}
```

### Worked example: NIP-13's note at three requirements

NIP-13's note has difficulty 21 and commits to 20.

| Required | Difficulty check | Target check | Result |
| ---: | --- | --- | --- |
| 20 | 21 ≥ 20 | 20 ≥ 20 | admitted |
| 21 | 21 ≥ 21 | 20 < 21 | `pow: committed target 20 is less than 21` |
| 22 | 21 < 22 | not reached | `pow: difficulty 21 is less than 22` |

At 21 the ID has enough work, but the author aimed lower. This is the case
NIP-13 describes: "if you require 40 bits to reply to your thread and see a
committed target of 30, you can safely reject it even if the note has 40
bits difficulty".

## The example

The example is [nostr/examples/proof-of-work.coax](../examples/proof-of-work.coax).
It counts the zero bits of NIP-13's three IDs, mines the test key's note to
8 and 16 bits, shows the miner stopping at its limit, signs the mined notes
with chapter 21's signer and verifies one, then asks a relay that requires
8 bits about each note. It shows five refusals, NIP-13's note at three
requirements, a bad requirement and an edited note.

<!-- source: nostr/examples/proof-of-work.coax example -->
```jai
#load "../../coax/lib/nostr/event_sign.coax";
#load "../../coax/lib/nostr/nip13.coax";
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

// BIP-340 vector 1's published test key and aux, as in chapter 21. Never
// mine or sign with a real key in an example.
KEY_HEX :: "b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef";
AUX_HEX :: "0000000000000000000000000000000000000000000000000000000000000001";
PUB_HEX :: "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659";
CREATED :: 1761686400;
CONTENT :: "It's just me mining my own business";

// Sign a kind-1 note with the test key; its length in out.
sign :: (created_at: int, tags: string, out: [] u8) -> int {
  key: [32] u8; aux: [32] u8; work: [1] NostrSignWork; scratch: [1024] u8;
  decode_hex(KEY_HEX, key[..]); decode_hex(AUX_HEX, aux[..]);
  result := EVENT_MORE;
  while result == EVENT_MORE {
    result = nostr_event_sign_step(key[..], aux[..], created_at, 1, tags, CONTENT,
                                   work[..], scratch[..], out, false);
  }
  #assert result > 0;
  return result;
}
// Mine from nonce 0 and report the run.
mine :: (target: int, tags: [] u8) -> N13Mine {
  pub: [32] u8; decode_hex(PUB_HEX, pub[..]);
  m := nip13_mine(pub, CREATED, 1, "[]", CONTENT, target, 0, 100000, tags);
  print("target %: nonce %, tries %, expected about %, difficulty %\n",
        target, m.nonce, m.tries, 1 << target, m.difficulty);
  print_hex("  id", m.id[..]);
  return m;
}
// What a relay requiring `required` bits answers: the OK frame it sends.
relay :: (label: string, text: string, required: int) {
  scratch: [1024] u8; reason: [128] u8;
  a := nip13_admit(text, required, scratch[..], false);
  n := nip13_reason(a, reason[..]);
  #assert n >= 0;
  print("%: difficulty %, target %, need % -> [\"OK\",\"%...\",%,\"%\"]\n",
        label, a.difficulty, a.target, required, ID_PREFIX(text),
        a.error == N13_OK, cast(string) reason[0..<n]);
}
// The first 8 hex digits of the event's id field, for the printed frame.
ID_PREFIX :: (text: string) -> string { return text[7..<15]; }

main :: () {
  print("difficulty by hand\n");
  id: [32] u8;
  decode_hex("000000000e9d97a1ab09fc381030b346cdd7a142ad57e6df0b46dc9bef6c7e2d", id[..]);
  print("NIP-13 prose 000000000e9d...: %\n", nip13_difficulty(id));
  decode_hex("002fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff", id[..]);
  print("NIP-13 prose 002f...: %\n", nip13_difficulty(id));
  decode_hex("000006d8c378af1779d2feebc7603a125d99eca0ccf1085959b307f64e5dd358", id[..]);
  print("NIP-13 example 000006d8...: %\n", nip13_difficulty(id));

  print("mine with the test key, created_at %\n", CREATED);
  tags8: [256] u8; tags16: [256] u8;
  m8 := mine(8, tags8[..]);
  m16 := mine(16, tags16[..]);
  #assert m8.error == N13_MINE_OK && m16.error == N13_MINE_OK;
  print("tags: %\n", cast(string) tags8[0..<m8.tags_n]);
  short: [256] u8; pub: [32] u8; decode_hex(PUB_HEX, pub[..]);
  m := nip13_mine(pub, CREATED, 1, "[]", CONTENT, 8, 0, 50, short[..]);
  print("target 8 with 50 tries: %, last nonce %\n",
        ifx m.error == N13_MINE_LIMIT then "limit" else "found", m.nonce);

  print("sign the mined notes\n");
  e8: [1024] u8; e16: [1024] u8;
  n8 := sign(CREATED, cast(string) tags8[0..<m8.tags_n], e8[..]);
  n16 := sign(CREATED, cast(string) tags16[0..<m16.tags_n], e16[..]);
  print("event: %\n", cast(string) e8[0..<n8]);
  scratch: [1024] u8;
  v := nostr_event_verify_vartime(cast(string) e8[0..<n8], scratch[..], false);
  print("signature verified: %\n", v.verified);

  print("other notes, signed the same way\n");
  e0: [1024] u8; lucky: [1024] u8; bare: [1024] u8; none: [1024] u8; bad: [1024] u8;
  n0 := sign(CREATED, "[[\"nonce\",\"0\",\"8\"]]", e0[..]);
  nl := sign(CREATED, "[[\"nonce\",\"26\",\"4\"]]", lucky[..]);
  nb := sign(CREATED, "[[\"nonce\",\"269\"]]", bare[..]);
  nn := sign(1761686513, "[]", none[..]);
  nx := sign(CREATED, "[[\"nonce\",\"72\",\"eight\"]]", bad[..]);

  print("a relay that requires 8 bits\n");
  relay("mined to 8", cast(string) e8[0..<n8], 8);
  relay("mined to 16", cast(string) e16[0..<n16], 8);
  relay("NIP-13 example", NIP13, 8);
  print("refusals\n");
  relay("nonce 0, unmined", cast(string) e0[0..<n0], 8);
  relay("target 4, lucky", cast(string) lucky[0..<nl], 8);
  relay("no target", cast(string) bare[0..<nb], 8);
  relay("no nonce tag", cast(string) none[0..<nn], 8);
  relay("target eight", cast(string) bad[0..<nx], 8);
  print("NIP-13's example at other requirements\n");
  relay("NIP-13 example", NIP13, 20);
  relay("NIP-13 example", NIP13, 21);
  relay("NIP-13 example", NIP13, 22);
  a := nip13_admit(NIP13, 300, scratch[..], false);
  reason: [128] u8; r := nip13_reason(a, reason[..]);
  print("require 300: %\n", cast(string) reason[0..<r]);
  edited: [1024] u8;
  for i: 0..<NIP13.count { edited[i] = NIP13[i]; }
  edited[NIP13.count - 167] = 'S';
  a = nip13_admit(cast(string) edited[0..<NIP13.count], 8, scratch[..], false);
  r = nip13_reason(a, reason[..]);
  print("content edited: %\n", cast(string) reason[0..<r]);
  s := nostr_event_verify_vartime(NIP13, scratch[..], false);
  print("NIP-13 example signature verified: %\n", s.verified);
}
```

To run it:

```shell
coax/bin/coax run nostr/examples/proof-of-work.coax --run-steps 250000000
```

Signing loads chapter 10's comb tables, so the program compiles with
`--run-steps 250000000`. The output, which the book's checks compare in
debug and release ([proof-of-work.out](../fixtures/proof-of-work.out)):

```text
difficulty by hand
NIP-13 prose 000000000e9d...: 36
NIP-13 prose 002f...: 10
NIP-13 example 000006d8...: 21
mine with the test key, created_at 1761686400
target 8: nonce 69, tries 70, expected about 256, difficulty 8
  id: 00f042eafd86699df69326bc25498d0ca79e1cc91cfe49b55afda1e116b74624
target 16: nonce 1803, tries 1804, expected about 65536, difficulty 16
  id: 0000b2d007cdba8c9c973727e78367d168b9a1197f029f99bf89d7616df0c46f
tags: [["nonce","69","8"]]
target 8 with 50 tries: limit, last nonce 49
sign the mined notes
event: {"id":"00f042eafd86699df69326bc25498d0ca79e1cc91cfe49b55afda1e116b74624","pubkey":"dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659","created_at":1761686400,"kind":1,"tags":[["nonce","69","8"]],"content":"It's just me mining my own business","sig":"747ccf20a9d61d7bd9acd3b2605e8c0a19d5b1d415f071e8c7615563fdf517766a6accf216f81ced06540f343fd15a06e86a71dc7dbb3f541da3c09943189c18"}
signature verified: true
other notes, signed the same way
a relay that requires 8 bits
mined to 8: difficulty 8, target 8, need 8 -> ["OK","00f042ea...",true,""]
mined to 16: difficulty 16, target 16, need 8 -> ["OK","0000b2d0...",true,""]
NIP-13 example: difficulty 21, target 20, need 8 -> ["OK","000006d8...",true,""]
refusals
nonce 0, unmined: difficulty 0, target 8, need 8 -> ["OK","931ed009...",false,"pow: difficulty 0 is less than 8"]
target 4, lucky: difficulty 8, target 4, need 8 -> ["OK","00de4fbb...",false,"pow: committed target 4 is less than 8"]
no target: difficulty 8, target -1, need 8 -> ["OK","00c67325...",false,"pow: no committed target, need 8"]
no nonce tag: difficulty 8, target -1, need 8 -> ["OK","00be4241...",false,"pow: no committed target, need 8"]
target eight: difficulty 8, target -1, need 8 -> ["OK","00e93335...",false,"pow: malformed nonce tag"]
NIP-13's example at other requirements
NIP-13 example: difficulty 21, target 20, need 20 -> ["OK","000006d8...",true,""]
NIP-13 example: difficulty 21, target 20, need 21 -> ["OK","000006d8...",false,"pow: committed target 20 is less than 21"]
NIP-13 example: difficulty 21, target 20, need 22 -> ["OK","000006d8...",false,"pow: difficulty 21 is less than 22"]
require 300: error: required difficulty is not 0 to 256
content edited: invalid: event id does not match its fields
NIP-13 example signature verified: true
```

Each refusal comes from a signed note whose ID is correct; only the work or
the tag is wrong:

- `nonce 0, unmined`: the ID `931ed009...` starts with `9`, `1001` in
  binary, so the difficulty is 0.
- `target 4, lucky`: counter 26 reached 8 bits while aiming for 4. The work
  is enough; the commitment is not.
- `no target`: `["nonce","269"]` reached 8 bits but committed to nothing.
- `no nonce tag`: `created_at` was stepped instead of a counter until the ID
  had 8 bits. The work is real, but there is no commitment.
- `target eight`: the target is not a number. The tag is malformed, whatever
  the ID's work.

`require 300` is the relay's own configuration error. `content edited`
changes the last letter of NIP-13's note: its ID no longer matches its
fields, so it refuses as `invalid:` before any work is measured.

## How the code is checked

### Checks in the library

`nip13.coax` holds its own `#test` checks: zero bits at every byte boundary
(256 for an all-zero ID, 255 for `00..01`), NIP-13's three IDs, the policy
on each fixture event in
[nip13_fixtures.coax](../../coax/tests/nostr/nip13_fixtures.coax) at the
requirements on both sides of its difficulty and target, 21 malformed
`nonce` tags, forged tag records, every reason string, and the miner's
inputs, limits and buffer bounds. The fixture events' IDs were computed by
Python's `hashlib`; their signatures are zero bytes, because admission never
verifies one.

```shell
coax/bin/coax test coax/lib/nostr/nip13.coax
coax/bin/coax mutate coax/lib/nostr/nip13.coax --root-only
```

On 2026-11-01, `coax test` ran 205 checks, including the loaded files' own,
in about 10 seconds. Root-only mutation killed 266 of 266 mutants. The
first run left 17 survivors; each became a new check (a target of `0`, an
empty tag before the `nonce` tag, targets `00` and `01`, the bounds 256,
0 and 65535 on the miner) or a simpler expression (the counter and target
are read from the raw token, so no unescape length is compared).

### Checks in the example

The example's checks compare the code with values computed outside Coax.

<!-- source: nostr/examples/checks/proof-of-work.coax check -->
```jai
// IDs that Python's hashlib computed from
// json.dumps([0, pubkey, created_at, 1, tags, content], separators=(",", ":"),
// ensure_ascii=False) for the test key's notes, and the first nonce from 0
// that reaches each target. They share no code with Coax.
PY_MINED :: [9] string.[
  "8",  "69",   "00f042eafd86699df69326bc25498d0ca79e1cc91cfe49b55afda1e116b74624",
  "10", "7",    "00308b8dc0d6231b2fab46b86f29dd2f4e29ea8276a7fe7569c5beda2fcb06bc",
  "16", "1803", "0000b2d007cdba8c9c973727e78367d168b9a1197f029f99bf89d7616df0c46f"];
PY_TAGGED :: [12] string.[
  "[[\"nonce\",\"0\",\"8\"]]",       "931ed0097191c96897fb23bbe557a6cfe117506773a59e5eef85360ab3f2d380", "1761686400", "0",
  "[[\"nonce\",\"26\",\"4\"]]",      "00de4fbbf1b46be04a87fda01290d3ab7c0ab7d92752332377b6c8db53795c70", "1761686400", "8",
  "[]",                              "00be4241ccc40052dd2f80d48ea1db0a6f13a0c60a04736efd28129832defdce", "1761686513", "8"];
// NIP-13's example note: Python's ID from its fields, which is the
// published one.
PY_NIP13_ID :: "000006d8c378af1779d2feebc7603a125d99eca0ccf1085959b307f64e5dd358";

number :: (s: string) -> int { v := 0; for c: s { v = v * 10 + cast(int) (c - '0'); } return v; }
same_id :: (id: [32] u8, text: string) -> bool {
  want: [32] u8; decode_hex(text, want[..]);
  for i: 0..<32 { if id[i] != want[i] { return false; } }
  return true;
}

#forall "the miner stops at Python's first nonce with Python's ID" i: 0..<3 #steps 400000000 {
  pub: [32] u8; decode_hex(PUB_HEX, pub[..]); tags: [256] u8;
  m := nip13_mine(pub, CREATED, 1, "[]", CONTENT, number(PY_MINED[3 * i]), 0, 5000, tags[..]);
  #assert m.error == N13_MINE_OK && m.nonce == number(PY_MINED[3 * i + 1]);
  #assert m.tries == m.nonce + 1 && same_id(m.id, PY_MINED[3 * i + 2]);
}
#forall "the refused notes' IDs are Python's" i: 0..<3 {
  pub: [32] u8; decode_hex(PUB_HEX, pub[..]); pre: [1024] u8; id: [32] u8;
  n := n13_preimage(pub, number(PY_TAGGED[4 * i + 2]), 1, PY_TAGGED[4 * i], CONTENT, pre[..]);
  #assert n > 0 && sha256_hash_checked(pre[0..<n], id[..], false) == SHA_OK;
  #assert same_id(id, PY_TAGGED[4 * i + 1]) && nip13_difficulty(id) == number(PY_TAGGED[4 * i + 3]);
}
#test "NIP-13's example: Python's ID, 21 bits, a valid signature" #steps 400000000 {
  scratch: [1024] u8;
  e := nostr_event_verify_vartime(NIP13, scratch[..], false);
  #assert e.error == NE_OK && e.verified && same_id(e.computed_id, PY_NIP13_ID);
  #assert nip13_difficulty(e.id) == 21;
}

// NIP-13's JavaScript countLeadingZeroes, transcribed: 4 per zero hex
// digit, then clz32(nibble) - 28 for the first nonzero one.
nibble_zeros :: (hex: string) -> int {
  count := 0;
  for c: hex {
    nibble := 0;
    while "0123456789abcdef"[nibble] != c { nibble += 1; }
    if nibble == 0 { count += 4; continue; }
    clz := 32; x := nibble; while x > 0 { clz -= 1; x = x / 2; }
    return count + clz - 28;
  }
  return count;
}
IDS :: [9] string.[
  "f000000000000000000000000000000000000000000000000000000000000000",
  "000000000e9d97a1ab09fc381030b346cdd7a142ad57e6df0b46dc9bef6c7e2d",
  "002fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
  "000006d8c378af1779d2feebc7603a125d99eca0ccf1085959b307f64e5dd358",
  "931ed0097191c96897fb23bbe557a6cfe117506773a59e5eef85360ab3f2d380",
  "7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
  "0000000000000000000000000000000000000000000000000000000000000000",
  "0000000000000000000000000000000000000000000000000000000000000001",
  "0000b2d007cdba8c9c973727e78367d168b9a1197f029f99bf89d7616df0c46f"];
IDS_BITS :: [9] int.[0, 36, 10, 21, 0, 1, 256, 255, 16];
#forall "byte counting agrees with NIP-13's nibble counting and the hand values" i: 0..<9 {
  id: [32] u8; decode_hex(IDS[i], id[..]);
  #assert nip13_difficulty(id) == nibble_zeros(IDS[i]) && nibble_zeros(IDS[i]) == IDS_BITS[i];
}
#test "same_id notices a change in the first or last byte" {
  id: [32] u8; zero := "0000000000000000000000000000000000000000000000000000000000000000";
  #assert same_id(id, zero);
  id[0] = 1; #assert !same_id(id, zero);
  id[0] = 0; id[31] = 1; #assert !same_id(id, zero);
}
```

- Python's `hashlib` gives the first counter from 0 that reaches 8, 10 and
  16 bits, and each ID. The miner stops at the same counter with the same
  ID.
- Python's IDs for the refused notes match `n13_preimage` and SHA-256, and
  their difficulties match the hand counts.
- NIP-13's published note has Python's ID, 21 bits, and a signature that
  chapter 11's verifier accepts.
- NIP-13's JavaScript `countLeadingZeroes`, transcribed, counts nibbles
  instead of bytes; it agrees with `nip13_difficulty` and with the hand
  values on nine IDs.

```shell
coax/bin/coax test nostr/examples/checks/proof-of-work.coax --run-steps 250000000
coax/bin/coax mutate nostr/examples/checks/proof-of-work.coax --root-only --run-steps 250000000
```

On 2026-11-01, `coax test` ran 358 checks in about 50 seconds, and root-only
mutation killed 40 of 40 mutants in the checks file. Mutation of the
example's own `main` was not run; its output is pinned by the fixture.

### What is not built

- Updating `created_at` during mining, and a time limit for a miner on a
  phone. The miner takes a `limit` instead.
- Delegated proof of work: a service that mines for a client. The miner
  already needs only the public key; the service and its payment are not
  built.
- A per-kind or per-key difficulty. `coax/relay/relay.coax` takes one
  difficulty for every EVENT (`--pow BITS`, default 0, off), checks it with
  `nip13_admit` before the signature, and advertises it as NIP-11's
  `limitation.min_pow_difficulty`
  ([chapter 37](ch037-nip-11-relay-information.md)).

## Summary

- Difficulty is the number of leading zero bits of the event ID; NIP-13's
  example has 21.
- The `nonce` tag carries a counter that the miner changes and the target
  the miner committed to.
- Reaching `d` bits takes `2^d` tries on average; each bit doubles the work.
- `nip13_admit` refuses too little work, a malformed `nonce` tag, a missing
  commitment, and a committed target below the requirement even when the
  ID has more work, each with a `pow:` reason.

## Dependencies

- [21. Events, IDs and signatures](ch021-events-ids-and-signatures.md)

## Sources

- [NIP-13](../nips/official/13.md), pinned, with its example note and
  reference counters.
- [NIP-01](../nips/official/01.md) for the ID preimage and the `pow:`
  prefix of `OK` messages.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
