# 95. Fuzzing

A relay reads bytes from anyone. A client reads what any relay sends. Every
reader in this book refuses bad input by design, and its checks show that
for the inputs someone thought of. Fuzzing runs a reader on inputs nobody
thought of: it changes valid inputs at random, many thousands of times, and
checks after each run that the reader kept its promises. This chapter
builds a small fuzzer in Coax, runs it on the book's parsers, and shows it
finding and shrinking two bugs planted in a toy reader.

Part XI, Interop and verification. Chapter 94 compares two implementations
on the same inputs; this chapter generates the hostile inputs, decides what
counts as a finding, and turns each finding into a short regression input.

## What this chapter teaches

- A seeded pseudo-random generator, so that every run reads the same inputs.
- Mutation: four small edits that turn a valid input into a nearly valid one.
- Invariants a reader must keep on any input: no read out of bounds, no
  endless loop, refusal or round-trip identity.
- Targets: JSON, filters, events, WebSocket frames, NIP-19 TLVs and NIP-44
  payloads, each run for a fixed budget with its seed recorded.
- Coverage guidance: keep the inputs that reach a new branch.
- Triage: sort findings by kind, then shrink each to a minimal input.

## Where the code is

| File | What it holds |
| --- | --- |
| [nostr/examples/fuzzing.coax](../examples/fuzzing.coax) | The generator, the mutator, one target per reader, the toy reader with two planted bugs, the guided fuzzer and the shrinker |
| [nostr/examples/checks/fuzzing.coax](../examples/checks/fuzzing.coax) | Checks against Python integers and hand-derived values |
| [coax/lib/nostr/json.coax](../../coax/lib/nostr/json.coax) | The JSON reader (chapter 17) |
| [coax/lib/nostr/filter_read.coax](../../coax/lib/nostr/filter_read.coax) | The filter reader (chapter 23) |
| [coax/lib/nostr/event.coax](../../coax/lib/nostr/event.coax) | The event reader (chapter 21) |
| [coax/lib/net/ws.coax](../../coax/lib/net/ws.coax) | The frame reader (chapter 26) |
| [coax/lib/nostr/nip19-core.coax](../../coax/lib/nostr/nip19-core.coax) | NIP-19 TLV encoding and decoding (chapter 16) |
| [coax/lib/nostr/nip44.coax](../../coax/lib/nostr/nip44.coax) | NIP-44 decryption (chapter 20) |
| [tools/coax-nostr/fuzz/](../../tools/coax-nostr/fuzz/README.md) | The secp256k1 differential harness, whose records this chapter cites |

The fuzzer is in the example, not in the library. No library code changed
for this chapter.

## A generator you can replay

A fuzzer that uses the system's random numbers finds a bug once and may
never find it again. This fuzzer draws every choice from xorshift64
(Marsaglia, 2003). The state is one 64-bit word that must not be 0. Each
step applies three shift-and-xor operations; `<<` discards the high bits,
as Coax defines it:

<!-- source: nostr/examples/fuzzing.coax prng -->
```jai
// xorshift64 (Marsaglia 2003, shifts 13, 7, 17). The state is never 0.
// The same seed gives the same inputs on every machine and in every build.
xorshift :: (rng: [] u64) -> u64 {
  x := rng[0];
  x ^= x << 13;
  x ^= x >> 7;
  x ^= x << 17;
  rng[0] = x;
  return x;
}
```

The seed fixes every input. A finding is reported with its seed and its
input number, and running the same seed again reaches the same input.

### Worked example: the first output from seed 1

Start with `x = 1`.

1. `x << 13` is `0x2000`. `x ^ 0x2000` is `0x2001`.
2. `0x2001 >> 7` is `0x40`. `0x2001 ^ 0x40` is `0x2041`.
3. `0x2041 << 17` is `0x40820000`. `0x2041 ^ 0x40820000` is
   `0x40822041`.

So the first output is `0x40822041`. The next two are
`0x100041060c011441` and `0x9b1e842f6e862629`; the example prints all
three, and the checks compare four outputs with Python's integers.

## Mutation

A random string is almost never valid JSON, so a reader refuses it at the
first byte and the rest of the reader never runs. A fuzzer starts instead
from valid seed inputs and changes a few bytes. `mutate_once` takes one
64-bit output `r` and uses its bits for one of four edits:

| `r % 4` | Edit | Position | Value |
| --- | --- | --- | --- |
| 0 | flip one bit | `(r >> 8) % n` | bit `(r >> 16) % 8` |
| 1 | write a dictionary byte | `(r >> 8) % n` | `DICT[(r >> 16) % 16]` |
| 2 | insert a byte | `(r >> 8) % (n + 1)` | `(r >> 32) & 0xff` |
| 3 | delete a byte | `(r >> 8) % n` | — |

The dictionary holds the bytes that the readers give meaning to: quotes,
backslashes, brackets, braces, commas, colons, `0`, `-`, `e`, `u`, and the
bytes `0x00`, `0x7f`, `0x80` and `0xff`. `mutate` copies a seed and applies
one to four edits; bits 62 and 63 of the first output say how many.

<!-- source: nostr/examples/fuzzing.coax mutate -->
```jai
// Bytes that JSON, frames and TLVs give meaning to.
DICT :: [16] u8.[0x00, 0xff, '"', '\\', '[', ']', '{', '}', ',', ':', '0', '-', 'e', 'u', 0x7f, 0x80];
// One mutation of buf[0..<n] driven by r; returns the new length.
// Bits 0-1 choose the operation, bits 8 and up the position, bits 16-19
// the bit or the dictionary entry, bits 32-39 an inserted byte.
mutate_once :: (buf: [] u8, n: int, r: u64) -> int {
  op := cast(int) (r % 4);
  if op == 2 {
    if n >= buf.count { return n; }
    at := cast(int) ((r >> 8) % cast(u64) (n + 1));
    i := n;
    while i > at { buf[i] = buf[i - 1]; i -= 1; }
    buf[at] = cast,trunc(u8) (r >> 32);
    return n + 1;
  }
  if n == 0 { return n; }
  at := cast(int) ((r >> 8) % cast(u64) n);
  if op == 0 { buf[at] ^= cast(u8) 1 << cast(u8) ((r >> 16) % 8); return n; }
  if op == 1 { buf[at] = DICT[cast(int) ((r >> 16) % 16)]; return n; }
  for i: at..<n - 1 { buf[i] = buf[i + 1]; }
  return n - 1;
}
// Copy seed into buf and apply 1 to 4 mutations; bits 62-63 of the first
// output say how many. Returns the length.
mutate :: (seed: [] u8, buf: [] u8, rng: [] u64) -> int {
  for i: 0..<seed.count { buf[i] = seed[i]; }
  r := xorshift(rng);
  n := mutate_once(buf, seed.count, r);
  for k: 0..<cast(int) (r >> 62) { n = mutate_once(buf, n, xorshift(rng)); }
  return n;
}
```

### Worked example: one mutation of `[1]`

Mutate the 3-byte seed `[1]` with seed 1. The first output is
`r = 0x40822041`.

1. Bits 62 and 63 of `r` are 0, so there is one edit.
2. `r % 4` is `0x41 % 4 = 1`: write a dictionary byte.
3. The position is `0x408220 % 3`. Because `16 ≡ 1 (mod 3)`, a hex number
   leaves the same remainder as the sum of its digits:
   `4 + 0 + 8 + 2 + 2 + 0 = 16`, and `16 % 3 = 1`. The position is 1.
4. The dictionary index is `0x4082 % 16 = 2`, which is `"`.

The result is `["]`. The JSON reader refuses it, because the string never
closes. The example prints this line before any target runs.

## What counts as a finding

A refusal is not a finding. Most mutated inputs are refused, and that is the
reader working. A finding is an input that makes a reader break a promise.
Each target states its promises as invariants and checks them after every
input:

| Target | Seed input | Invariant |
| --- | --- | --- |
| JSON | the signed event of chapter 21, and a filter | every token's bytes lie inside the input; at most `n + 1` tokens; an accepted text stays accepted with a space after it |
| filters | a filter with ids, kinds, a `#d` tag, since and limit | an accepted filter written by `nf_write` reads back and writes the same bytes |
| events | the signed event of chapter 21 | a parsed event serializes to valid JSON; an event `nostr_event_read` accepts has its id equal to the hash of that serialization |
| frames | the fragmented stream of chapter 26 | an accepted frame ends inside the bytes given, and its header is inside the frame |
| NIP-19 | the TLV bytes of the NIP-19 `nprofile` example | TLV bytes the encoder accepts decode back to the same bytes |
| NIP-44 | the payload of encrypt_decrypt row 3 | a changed payload is refused or decrypts to the original plaintext |

Two kinds of finding need no invariant of their own. An index out of range
stops the debug build, which checks every slice bound, and it is reported
by AddressSanitizer in the C build. A loop that does not end is a hang; the
JSON invariant bounds the token count for that reason.

The JSON target:

<!-- source: nostr/examples/fuzzing.coax json -->
```jai
// JSON (chapter 17): every token's bytes lie inside the input, the reader
// stops within n + 1 tokens, and an accepted text stays accepted with a
// space after it.
json_ok :: (text: string) -> bool {
  j: [1] Json; json_start(j[..], text);
  tokens := 0; t := json_next(j[..]);
  while t != JSON_END && t != JSON_ERROR {
    if j[0].from < 0 || j[0].from > j[0].to || j[0].to > text.count { return false; }
    tokens += 1;
    if tokens > text.count + 1 { return false; }
    t = json_next(j[..]);
  }
  return true;
}
fuzz_json :: (seed_text: string, seed: u64, inputs: int) -> Run {
  rng := [1] u64.[seed]; s: [512] u8; buf: [520] u8; r: Run;
  m := bytes_of(seed_text, s[..]);
  for i: 0..<inputs {
    n := mutate(s[0..<m], buf[0..<512], rng[..]);
    text := cast(string) buf[0..<n];
    r.inputs += 1;
    if !json_ok(text) { r.findings += 1; continue; }
    if json_valid(text) {
      r.accepted += 1;
      buf[n] = ' ';
      if !json_valid(cast(string) buf[0..<n + 1]) { r.findings += 1; }
    }
  }
  return r;
}
```

The filter target reads, writes, reads again and writes again. The second
write must equal the first. The first write need not equal the input:
`nf_write` writes one spelling, and the input may use another.

<!-- source: nostr/examples/fuzzing.coax filter -->
```jai
// Filters (chapter 23): what filter_read accepts, nf_write writes, and the
// written text reads back and writes again to the same bytes.
FSpace :: struct { strs: [32] string; ints: [32] int; tags: [8] NostrTagFilter; bytes: [2048] u8; }
fread :: (text: string, f: [] NostrFilter, s: [] FSpace) -> int {
  sp: [1] FrSpace;
  sp[0] = FrSpace.{strs = s[0].strs[..], ints = s[0].ints[..], tags = s[0].tags[..], bytes = s[0].bytes[..]};
  return filter_read(text, f, sp[..]);
}
fuzz_filter :: (seed_text: string, seed: u64, inputs: int) -> Run {
  rng := [1] u64.[seed]; s: [512] u8; buf: [512] u8; r: Run;
  m := bytes_of(seed_text, s[..]);
  for i: 0..<inputs {
    n := mutate(s[0..<m], buf[..], rng[..]);
    r.inputs += 1;
    a: [1] FSpace; f: [1] NostrFilter;
    if fread(cast(string) buf[0..<n], f[..], a[..]) != FR_OK { continue; }
    r.accepted += 1;
    once: [1024] u8; twice: [1024] u8;
    w := nf_write(f[0], once[..], false);
    b: [1] FSpace; g: [1] NostrFilter;
    if w < 0 || fread(cast(string) once[0..<w], g[..], b[..]) != FR_OK { r.findings += 1; continue; }
    v := nf_write(g[0], twice[..], false);
    if v != w || cast(string) twice[0..<v] != cast(string) once[0..<w] { r.findings += 1; }
  }
  return r;
}
```

The event, frame, NIP-19 and NIP-44 targets follow the same shape:

<!-- source: nostr/examples/fuzzing.coax event -->
```jai
// Events (chapter 21): an event ne_parse accepts serializes to valid JSON,
// and nostr_event_read accepts only when the hash of that serialization is
// the event's id, written inside the output.
fuzz_event :: (seed_text: string, seed: u64, inputs: int) -> Run {
  rng := [1] u64.[seed]; s: [512] u8; buf: [512] u8; out: [1024] u8; r: Run;
  m := bytes_of(seed_text, s[..]);
  for i: 0..<inputs {
    n := mutate(s[0..<m], buf[..], rng[..]);
    text := cast(string) buf[0..<n];
    r.inputs += 1;
    e := ne_parse(text);
    if e.error != NE_OK { continue; }
    w := ne_serialize(e, out[..]);
    if w < 0 || !json_valid(cast(string) out[0..<w]) { r.findings += 1; continue; }
    read := nostr_event_read(text, out[..], false);
    if read.error != NE_OK { continue; }
    r.accepted += 1;
    if read.written != w || !ne_same_id(read) { r.findings += 1; }
  }
  return r;
}
```

<!-- source: nostr/examples/fuzzing.coax frames -->
```jai
// WebSocket frames (chapter 26): a frame ws_frame_read accepts never ends
// past the bytes it was given, and its header is inside the frame. An input
// counts as accepted when its first frame is.
fuzz_frames :: (seed_hex: [] u8, seed: u64, inputs: int) -> Run {
  rng := [1] u64.[seed]; buf: [64] u8; msg: [16] u8; r: Run;
  for i: 0..<inputs {
    n := mutate(seed_hex, buf[..], rng[..]);
    r.inputs += 1;
    reader: [1] WsReader; at := 0;
    while at < n {
      f := ws_frame_read(cast(string) buf[at..<n], msg.count);
      if f.status != WS_FRAME_OK { break; }
      if f.header > f.total || f.total > n - at { r.findings += 1; break; }
      if at == 0 { r.accepted += 1; }
      ev := ws_take(reader[..], f, cast(string) buf[at + f.header..<at + f.total], msg[..]);
      at += f.total;
      if ev >= WS_EV_CONTINUATION { break; }
    }
  }
  return r;
}
```

<!-- source: nostr/examples/fuzzing.coax nip19 -->
```jai
// NIP-19 TLVs (chapter 16): TLV bytes that nprofile encoding accepts decode
// back to the same bytes.
fuzz_nip19 :: (seed_bytes: [] u8, seed: u64, inputs: int) -> Run {
  rng := [1] u64.[seed]; buf: [160] u8; text: [400] u8; back: [160] u8; r: Run;
  for i: 0..<inputs {
    n := mutate(seed_bytes, buf[..], rng[..]);
    r.inputs += 1;
    e := nip19_encode_checked(N19_PROFILE, cast(string) buf[0..<n], text[..], false);
    if e.error != NC_OK { continue; }
    r.accepted += 1;
    d := nip19_decode_checked(cast(string) text[0..<e.written], back[..], false);
    if d.error != NC_OK || d.written != n || cast(string) back[0..<n] != cast(string) buf[0..<n] { r.findings += 1; }
  }
  return r;
}
```

<!-- source: nostr/examples/fuzzing.coax nip44 -->
```jai
// NIP-44 payloads (chapter 20): a changed payload either is refused or
// decrypts to the original plaintext. The MAC allows nothing else.
fuzz_nip44 :: (key: [] u8, payload: string, plain: string, seed: u64, inputs: int) -> Run {
  rng := [1] u64.[seed]; s: [200] u8; buf: [200] u8; work: [512] u8; out: [256] u8; r: Run;
  m := bytes_of(payload, s[..]);
  for i: 0..<inputs {
    n := mutate(s[0..<m], buf[..], rng[..]);
    r.inputs += 1;
    d := nip44_decrypt_checked(NIP44_COMPAT, key, cast(string) buf[0..<n], work[..], out[..], false);
    if d.error != NIP44_OK { continue; }
    r.accepted += 1;
    if cast(string) out[0..<d.written] != plain { r.findings += 1; }
  }
  return r;
}
```

These targets mutate their seeds without coverage feedback. The readers are
library code, and Coax doesn't yet insert coverage counters into the code
it emits (see [Coverage in the emitted C](#coverage-in-the-emitted-c)).

## Coverage guidance

A blind fuzzer mutates only the seeds. A coverage-guided fuzzer, such as
libFuzzer or AFL, records which branches each input reaches, and adds an
input to its corpus when it reaches a branch that no earlier input reached.
Later inputs are mutations of corpus entries, so the fuzzer moves one step
at a time into code that the seeds never reach.

### A reader with two planted bugs

To show guidance and triage on a known answer, the example has a toy TLV
reader in the style of NIP-19. A record is a type byte, a length byte and
the value:

| Type | Meaning | Rule |
| --- | --- | --- |
| 0 | special | must come before type 2 |
| 1 | relay | any length |
| 2 | author | exactly 32 bytes |
| 255 | padding | any length, skipped |
| other | unknown | skipped |

With `fixed` false the reader has two bugs:

1. Type 2 reads 32 bytes because the type says so. It doesn't check the
   length byte or the bytes left. A short record reads past the end of
   the input.
2. Padding advances by its length but forgets its 2-byte header. A
   zero-length pad never advances.

The reader reads every byte through `peek`, which records an index out of
range instead of reading it, as AddressSanitizer records a bad access. The
loop gives up after `n + 1` records, which no valid input needs, and reports
a hang. Each branch marks one of ten slots in `hits`; those are the
coverage counters a compiler would insert.

<!-- source: nostr/examples/fuzzing.coax planted -->
```jai
// A toy TLV reader with two planted bugs. Each byte is read through peek,
// which records an out-of-bounds index the way AddressSanitizer reports
// one, and each branch marks its slot in hits for coverage.
TLV_OK :: 0; TLV_SHORT :: 1; TLV_ORDER :: 2; TLV_HANG :: 3;
Probe :: struct { bytes: []u8; oob: bool; hits: [10] bool; }
peek :: (p: [] Probe, i: int) -> u8 {
  if i < 0 || i >= p[0].bytes.count { p[0].oob = true; return 0; }
  return p[0].bytes[i];
}
// Records: type 0 (special, first), type 1 (relay), type 2 (author, 32
// bytes, after the special), type 255 (padding), others skipped. fixed
// false keeps the two bugs: type 2 trusts its type and reads 32 bytes, and
// padding forgets its 2-byte header, so a zero-length pad never advances.
// The loop gives up after n + 1 records: no valid input needs more.
tlv_read :: (p: [] Probe, fixed: bool) -> int {
  n := p[0].bytes.count; at := 0; special := false; records := 0; sum := 0;
  p[0].hits[0] = true;
  while at < n {
    records += 1;
    if records > n + 1 { p[0].hits[9] = true; return TLV_HANG; }
    if at + 2 > n { p[0].hits[1] = true; return TLV_SHORT; }
    t := peek(p, at); size := cast(int) peek(p, at + 1);
    if t == 2 {
      if !special { p[0].hits[2] = true; return TLV_ORDER; }
      p[0].hits[3] = true;
      if fixed && (size != 32 || at + 2 + size > n) { return TLV_SHORT; }
      for i: 0..<32 { sum += cast(int) peek(p, at + 2 + i); }
      at += 34;
      continue;
    }
    if at + 2 + size > n { p[0].hits[4] = true; return TLV_SHORT; }
    if t == 0 { p[0].hits[5] = true; special = true; }
    else if t == 1 { p[0].hits[6] = true; }
    else if t == 255 { p[0].hits[7] = true; at += size; if fixed { at += 2; } continue; }
    else { p[0].hits[8] = true; }
    for i: 0..<size { sum += cast(int) peek(p, at + 2 + i); }
    at += 2 + size;
  }
  return TLV_OK;
}
// What one input does: 0 nothing wrong, 1 an out-of-bounds read, 2 a hang.
FIND_NONE :: 0; FIND_OOB :: 1; FIND_HANG :: 2;
FIND :: [3] string.["none", "out-of-bounds read", "hang"];
probe :: (bytes: [] u8, fixed: bool, hits: [] bool) -> int {
  p: [1] Probe; p[0].bytes = bytes;
  status := tlv_read(p[..], fixed);
  for i: 0..<10 { hits[i] = p[0].hits[i]; }
  if p[0].oob { return FIND_OOB; }
  if status == TLV_HANG { return FIND_HANG; }
  return FIND_NONE;
}
```

### The guided loop

`guided` keeps a corpus of up to 64 inputs. Each round picks a corpus entry,
mutates it and runs the reader. An input that reaches a new slot, and has no
finding, joins the corpus. The first input of each kind of finding is kept.
With `keep` false the corpus never grows, which is blind fuzzing with the
same generator.

<!-- source: nostr/examples/fuzzing.coax guided -->
```jai
// Coverage-guided fuzzing of the TLV reader. The corpus starts with one
// seed; an input that reaches a branch no earlier input reached joins it.
// The first input of each finding kind is kept. Returns the input index
// at which both kinds were found, or -1.
CAP :: 48;
Found :: struct { at: [3] int; bytes: [3] [CAP] u8; count: [3] int; corpus: int; covered: int; }
guided :: (seed_bytes: [] u8, seed: u64, inputs: int, keep: bool, fixed: bool) -> Found {
  corpus: [64] [CAP] u8; lens: [64] int; seen: [10] bool; found: Found;
  for i: 0..<seed_bytes.count { corpus[0][i] = seed_bytes[i]; }
  lens[0] = seed_bytes.count; found.corpus = 1;
  found.at[1] = -1; found.at[2] = -1;
  rng := [1] u64.[seed]; buf: [CAP] u8; hits: [10] bool;
  for i: 0..<inputs {
    c := cast(int) (xorshift(rng[..]) % cast(u64) found.corpus);
    n := mutate(corpus[c][0..<lens[c]], buf[..], rng[..]);
    kind := probe(buf[0..<n], fixed, hits[..]);
    if kind != FIND_NONE && found.at[kind] < 0 {
      found.at[kind] = i; found.count[kind] = n;
      for k: 0..<n { found.bytes[kind][k] = buf[k]; }
    }
    fresh := false;
    for b: 0..<10 { if hits[b] && !seen[b] { seen[b] = true; fresh = true; } }
    if keep && fresh && kind == FIND_NONE && found.corpus < 64 {
      for k: 0..<n { corpus[found.corpus][k] = buf[k]; }
      lens[found.corpus] = n; found.corpus += 1;
    }
    if found.at[1] >= 0 && found.at[2] >= 0 { break; }
  }
  for b: 0..<10 { if seen[b] { found.covered += 1; } }
  return found;
}
```

From the seed record `00 01 61 01 03 77 73 73` (special `a`, relay `wss`)
and seed 7, the guided run grows a corpus of 7 inputs, reaches all 10
slots, and finds the hang at input 580 and the read out of bounds at input
2706. The blind run finds the read out of bounds sooner, at input 106, but
the hang only at input 7075. One seed is one sample; over seeds 1 to 20, the
guided runs needed 44,536 inputs in total to find both bugs, and the blind
runs 80,928.

## Triage and shrinking

A fuzzer finds the same bug many times, through inputs full of irrelevant
bytes. Triage sorts findings by kind (here: read out of bounds, or hang) and
keeps one input of each. Shrinking then removes everything the finding
doesn't need. `shrink` tries deleting each byte, then setting each nonzero
byte to 0, and keeps any change that gives the same kind of finding. It
repeats until a full pass changes nothing. The result is 1-minimal: no
single deletion or zeroing keeps the finding. libFuzzer's
`-minimize_crash=1` and AFL's `afl-tmin` do the same job with more edits.

<!-- source: nostr/examples/fuzzing.coax shrink -->
```jai
// Shrink an input while it still gives the same finding: try deleting
// each byte, then setting each byte to 0, until neither helps. The result
// is 1-minimal: no single deletion or zeroing keeps the finding.
shrink :: (bytes: [] u8, count: int, kind: int) -> int {
  n := count;
  hits: [10] bool; trial: [CAP] u8;
  changed := true;
  while changed {
    changed = false;
    i := 0;
    while i < n {
      for k: 0..<i { trial[k] = bytes[k]; }
      for k: i + 1..<n { trial[k - 1] = bytes[k]; }
      if probe(trial[0..<n - 1], false, hits[..]) == kind {
        for k: 0..<n - 1 { bytes[k] = trial[k]; }
        n -= 1; changed = true;
      } else { i += 1; }
    }
    for k: 0..<n {
      if bytes[k] == 0 { continue; }
      old := bytes[k]; bytes[k] = 0;
      if probe(bytes[0..<n], false, hits[..]) == kind { changed = true; } else { bytes[k] = old; }
    }
  }
  return n;
}
```

### Worked example: shrinking the read out of bounds

The guided run found `00 01 33 02 ca 01 03 75 73` (9 bytes). The reader
reads a special record of length 1 (`33`), then a type 2 record at offset
3, and reads 32 bytes from offset 5 in a 9-byte input. Shrinking:

1. Deleting byte 0, 1, 2 or 3 changes how the records line up, and the
   reader refuses the result as short or out of order without reading past
   the end. Those bytes stay.
2. Deleting byte 4 (`ca`) keeps the type 2 record at offset 3, so the read
   still runs past the end. Deleting at position 4 succeeds until the input
   is `00 01 33 02 73`. One more deletion leaves `00 01 33 02`, which has
   no room for a header after offset 3; that is refused as short.
3. Zeroing byte 1 makes the special record empty and puts `33` where a type
   byte goes; that is refused as short. Zeroing byte 2 (`33`) and byte 4
   (`73`) both keep the finding: `00 01 00 02 00`.
4. The next deletion pass can now delete byte 1: `00 00 02 00` is an empty
   special record followed by a type 2 header with nothing after it. The
   reader reads offsets 4 to 35 of a 4-byte input.
5. No deletion or zeroing of `00 00 02 00` keeps the finding, so the
   shrinker stops.

The hang shrinks the same way: `ff 00 ca 01 03 75 80 db` loses every byte
after the zero-length pad, and the result is `ff 00`.

Each shrunk input is a regression test. The checks file asserts that
`00 00 02 00` reads out of bounds and `ff 00` hangs in the buggy reader,
and that the fixed reader refuses the first as short and accepts the
second. With `fixed` true, the same guided run reaches 9 of the 10 slots
(the hang slot is never reached) and gives no finding in 20,000 inputs.

## The example

The example is [nostr/examples/fuzzing.coax](../examples/fuzzing.coax). It
prints the generator's first outputs and the worked mutation, runs every
target for a fixed budget with its seed, then fuzzes the toy reader, shrinks
both findings and runs the fixed reader.

<!-- source: nostr/examples/fuzzing.coax main -->
```jai
main :: () {
  // The generator's first outputs from seed 1, and one mutation of "[1]".
  rng := [1] u64.[1];
  print("xorshift64 from 1:");
  for i: 0..<3 { print(" "); print_hex64(xorshift(rng[..])); }
  print("\n");
  rng[0] = 1;
  seed: [3] u8; #assert bytes_of("[1]", seed[..]) == 3; buf: [8] u8;
  n := mutate(seed[..], buf[..], rng[..]);
  print("\"[1]\" mutated with seed 1: \"%\", JSON %\n", cast(string) buf[0..<n], json_valid(cast(string) buf[0..<n]));

  // Every reader for a fixed budget, each with its seed.
  report("json, event seed", 20261101, fuzz_json(EVENT, 20261101, 3000));
  report("json, filter seed", 20261102, fuzz_json(FILTER, 20261102, 3000));
  report("filter", 20261103, fuzz_filter(FILTER, 20261103, 3000));
  report("event", 20261104, fuzz_event(EVENT, 20261104, 1000));
  frames: [18] u8; #assert hex_decode_checked("010348656c89017000016c80016f880203e8", frames[..], false).error == 0;
  report("frames", 20261105, fuzz_frames(frames[..], 20261105, 3000));
  tlv: [160] u8; d := nip19_decode_checked(NPROFILE, tlv[..], false);
  report("nip19 nprofile TLV", 20261106, fuzz_nip19(tlv[0..<d.written], 20261106, 3000));
  key := key_of(SEALED_KEY);
  report("nip44 payload", 20261107, fuzz_nip44(key[..], SEALED_PAYLOAD, SEALED_PLAIN, 20261107, 1000));

  // The planted bugs: coverage-guided, then blind, from the same seed.
  tseed := [8] u8.[0, 1, 'a', 1, 3, 'w', 's', 's'];
  print("seed record: special \"a\", relay \"wss\": %\n", read_status(tseed[..], false));
  g := guided(tseed[..], 7, 20000, true, false);
  print("guided: corpus %, branches %, out-of-bounds read at input %, hang at input %\n", g.corpus, g.covered, g.at[1], g.at[2]);
  blind := guided(tseed[..], 7, 20000, false, false);
  print("blind: branches %, out-of-bounds read at input %, hang at input %\n", blind.covered, blind.at[1], blind.at[2]);
  for kind: 1..<3 {
    if g.at[kind] < 0 { continue; }
    show(FIND[kind], g.bytes[kind][0..<g.count[kind]]);
    m := shrink(g.bytes[kind][..], g.count[kind], kind);
    show("  shrunk", g.bytes[kind][0..<m]);
    print("  buggy reader: %, fixed reader: %\n", read_status(g.bytes[kind][0..<m], false), read_status(g.bytes[kind][0..<m], true));
  }
  // Inputs until both bugs are found, summed over seeds 1 to 20 (20000
  // counted when a run doesn't find both).
  sum_guided := 0; sum_blind := 0;
  for s: 1..<21 {
    a := guided(tseed[..], cast(u64) s, 20000, true, false);
    b := guided(tseed[..], cast(u64) s, 20000, false, false);
    sum_guided += both_at(a); sum_blind += both_at(b);
  }
  print("seeds 1 to 20, inputs to find both: guided %, blind %\n", sum_guided, sum_blind);
  fixed := guided(tseed[..], 7, 20000, true, true);
  print("fixed reader, 20000 inputs: branches %, out-of-bounds read at input %, hang at input %\n", fixed.covered, fixed.at[1], fixed.at[2]);
}
```

It prints:

```text
xorshift64 from 1: 0x40822041 0x100041060c011441 0x9b1e842f6e862629
"[1]" mutated with seed 1: "["]", JSON false
json, event seed: seed 20261101, 3000 inputs, 1006 accepted, 0 findings
json, filter seed: seed 20261102, 3000 inputs, 732 accepted, 0 findings
filter: seed 20261103, 3000 inputs, 98 accepted, 0 findings
event: seed 20261104, 1000 inputs, 13 accepted, 0 findings
frames: seed 20261105, 3000 inputs, 2341 accepted, 0 findings
nip19 nprofile TLV: seed 20261106, 3000 inputs, 695 accepted, 0 findings
nip44 payload: seed 20261107, 1000 inputs, 2 accepted, 0 findings
seed record: special "a", relay "wss": ok
guided: corpus 7, branches 10, out-of-bounds read at input 2706, hang at input 580
blind: branches 10, out-of-bounds read at input 106, hang at input 7075
out-of-bounds read: 00 01 33 02 ca 01 03 75 73
  shrunk: 00 00 02 00
  buggy reader: ok, fixed reader: short
hang: ff 00 ca 01 03 75 80 db
  shrunk: ff 00
  buggy reader: hang, fixed reader: ok
seeds 1 to 20, inputs to find both: guided 44536, blind 80928
fixed reader, 20000 inputs: branches 9, out-of-bounds read at input -1, hang at input -1
```

Read the output this way:

- Each target line is the record that the Done means line asks for: the
  seed, the budget, how many inputs the reader accepted, and 0 findings.
  The accepted count shows that the mutations reach the accepting code, not
  only the first refusal. For frames, it counts inputs whose first frame
  was accepted.
- The NIP-44 target accepts 2 of 1000 inputs. Both are edit sequences that left the
  payload's bytes unchanged (a Python model of the mutator reproduces
  both), and both decrypt to the original plaintext. Every other edit
  is refused by the MAC or the base64 reader.
- The buggy reader returns `ok` on `00 00 02 00`: the read out of bounds
  is silent, and only the probe, or AddressSanitizer, sees it. The fixed
  reader refuses the same input as `short`.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/fuzzing.coax](../examples/checks/fuzzing.coax). They
compare the fuzzer's parts with independent values:

- four xorshift64 outputs from seed 1, and the 1000th output from seed
  20261101, computed with Python's integers;
- one edit of each kind on `[1]`, against a Python model of `mutate_once`;
- the probe records reads at -1 and at `n`;
- the two shrunk inputs give their findings in the buggy reader and none
  in the fixed reader;
- for seeds 1 to 10, the guided fuzzer finds both bugs, each shrinks to the
  hand-derived length (4 and 2 bytes), and the result is 1-minimal;
- for seeds 1 to 5, the fixed reader gives no finding in 20,000 inputs.

<!-- source: nostr/examples/checks/fuzzing.coax check -->
```jai
// xorshift64 outputs computed with Python's integers, masking each left
// shift to 64 bits: the first four from seed 1, and the 1000th from the
// JSON target's seed.
#test "xorshift64 matches Python integers" {
  rng := [1] u64.[1];
  want := [4] u64.[0x40822041, 0x100041060c011441, 0x9b1e842f6e862629, 0xf554f503555d8025];
  for w: want { #assert xorshift(rng[..]) == w; }
  rng[0] = 20261101;
  last: u64;
  for i: 0..<1000 { last = xorshift(rng[..]); }
  #assert last == 0x2a317c08f968e919;
}
// Each operation on "[1]", with the results a Python model of mutate_once
// gives for the same r.
mutated :: (r: u64, want: string) -> bool {
  buf: [8] u8; buf[0] = '['; buf[1] = '1'; buf[2] = ']';
  n := mutate_once(buf[..], 3, r);
  return cast(string) buf[0..<n] == want;
}
#test "one mutation of each kind" {
  #assert mutated(0x40822041, "[\"]");    // op 1: position 1 gets DICT[2]
  #assert mutated(0x3, "1]");             // op 3: delete position 0
  #assert mutated(0x4100000002, "A[1]");  // op 2: insert 0x41 at 0
  #assert mutated(0x50000, "[1}");        // op 0: flip bit 5 of position 2
}
#test "an insert into a full buffer and a change of nothing keep the length" {
  full: [3] u8; full[0] = 'a'; full[1] = 'b'; full[2] = 'c';
  #assert mutate_once(full[..], 3, 0x2) == 3;
  empty: [4] u8;
  #assert mutate_once(empty[..], 0, 0x1) == 0 && mutate_once(empty[..], 0, 0x3) == 0;
}
// The probe reports reads at -1 and at n, and nothing for reads inside.
#test "peek records out-of-bounds reads" {
  b := [2] u8.[7, 9];
  p: [1] Probe; p[0].bytes = b[..];
  #assert peek(p[..], 1) == 9 && !p[0].oob;
  #assert peek(p[..], 2) == 0 && p[0].oob;
  p[0].oob = false;
  #assert peek(p[..], -1) == 0 && p[0].oob;
}
// The two shrunk inputs, worked out by hand in the chapter.
#test "the shrunk inputs give their findings" {
  hits: [10] bool;
  oob := [4] u8.[0, 0, 2, 0];
  #assert probe(oob[..], false, hits[..]) == FIND_OOB;
  #assert probe(oob[..], true, hits[..]) == FIND_NONE;
  hang := [2] u8.[255, 0];
  #assert probe(hang[..], false, hits[..]) == FIND_HANG;
  #assert probe(hang[..], true, hits[..]) == FIND_NONE;
}
// For seeds 1 to 10 the guided fuzzer finds both bugs, and each finding
// shrinks to the same input, which is 1-minimal: no single deletion or
// zeroing keeps the finding.
TSEED :: [8] u8.[0, 1, 'a', 1, 3, 'w', 's', 's'];
#forall "each seed finds and shrinks both bugs" s: 1..<11 #steps 1000000000 {
  seed := TSEED;
  f := guided(seed[..], cast(u64) s, 20000, true, false);
  want_len := [3] int.[0, 4, 2];
  for kind: 1..<3 {
    #assert f.at[kind] >= 0;
    m := shrink(f.bytes[kind][..], f.count[kind], kind);
    #assert m == want_len[kind];
    hits: [10] bool; trial: [CAP] u8;
    for i: 0..<m {
      for k: 0..<i { trial[k] = f.bytes[kind][k]; }
      for k: i + 1..<m { trial[k - 1] = f.bytes[kind][k]; }
      #assert probe(trial[0..<m - 1], false, hits[..]) != kind;
      if f.bytes[kind][i] != 0 {
        for k: 0..<m { trial[k] = f.bytes[kind][k]; }
        trial[i] = 0;
        #assert probe(trial[0..<m], false, hits[..]) != kind;
      }
    }
  }
}
// The fixed reader gives no finding for seeds 1 to 5.
#forall "the fixed reader has no finding" s: 1..<6 #steps 1000000000 {
  seed := TSEED;
  f := guided(seed[..], cast(u64) s, 20000, true, true);
  #assert f.at[1] < 0 && f.at[2] < 0;
}
// The JSON invariant holds on a complete text and on a cut one; the
// reader refuses trailing bytes.
#test "the JSON invariant on whole and cut texts" {
  #assert json_ok("[1,{\"a\":\"b\"}]");
  #assert json_ok("[1,");
  #assert json_valid("[1]") && !json_valid("[1] x");
}
```

`coax test nostr/examples/checks/fuzzing.coax` passed all 315 checks on
2026-11-01 (the file loads the libraries' own checks too).
`coax mutate nostr/examples/checks/fuzzing.coax --root-only` killed 11 of
11 mutants.

### AddressSanitizer and UndefinedBehaviorSanitizer

The debug build checks every slice bound. The example was also built as C
under AddressSanitizer and UndefinedBehaviorSanitizer on 2026-11-01
(x86_64, GCC 15.2, `coax0 0.46.0`):

```shell
coax/bin/coax c nostr/examples/fuzzing.coax > /tmp/fuzz.c
gcc -std=c11 -O1 -g -fsanitize=address,undefined -fno-omit-frame-pointer \
  -fno-sanitize-recover=all /tmp/fuzz.c -o /tmp/fuzz-asan -lm
/tmp/fuzz-asan
```

It exited 0 with no sanitizer report in 0.7 seconds, and its output was
byte for byte the fixture. The debug and release builds give the same
output.

### Earlier fuzz records

These records exist in the repository. None was rerun for this chapter.

- Chapter 26's example reads 20,000 fuzzed frame streams on every run of
  the book's checks, with 0 reads past the buffer.
- [schnorr-hour-2026-10-28.json](../../coax/tests/nostr/data/schnorr-hour-2026-10-28.json):
  a one-hour run of BIP-340 verification on 2026-10-29 from 01:48 to 02:48
  UTC, seed 202610290340, 1,743,107 generated cases (102,536 of them Nostr
  events), 0 failures. It covers verification correctness, not signing or
  constant time.
- [oracle-smoke-2026-10-28.json](../../tools/coax-nostr/fuzz/evidence/oracle-smoke-2026-10-28.json):
  the secp256k1 differential harness, seed 20261028, 411 generated cases,
  438 comparisons, 0 failures. It checks the oracles and the comparison
  plumbing, not Coax code.
- [seeded-carry-2026-10-28.json](../../tools/coax-nostr/fuzz/evidence/seeded-carry-2026-10-28.json)
  and
  [seeded-default-hash-2026-10-28.json](../../tools/coax-nostr/fuzz/evidence/seeded-default-hash-2026-10-28.json):
  the same harness with a planted lost carry and a planted wrong ECDH hash.
  Each run records exactly one mismatch, so the harness catches the bugs
  planted in it. Chapter 94 explains the comparison itself.

No libFuzzer or AFL run of the Nostr parsers is recorded. Apple's clang has
no libFuzzer, so the plan in
[docs/lightning/testing.md](../../docs/lightning/testing.md) runs the
seeded driver on the Mac and upstream clang's
`-fsanitize=fuzzer,address,undefined` on a Linux host.

### Coverage in the emitted C

The parser targets here are blind. The verification brainstorm
([section 7.6](../../docs/verification/2026-10-27-formal-verification-brainstorm.md))
proposes `coax fuzz`: the emitter inserts a counter on each branch, a
driver written in Coax keeps a corpus of inputs that reach new branches,
and each crash becomes a fixture. The toy reader's `hits` slots are that
design written by hand. Until the emitter does it, a guided run of a
library parser needs a clang build of the emitted C with
`-fsanitize=fuzzer`.

## Summary

- xorshift64 makes every input reproducible from a seed and an input number.
- Four edits (flip a bit, write a dictionary byte, insert, delete) turn
  valid seeds into nearly valid inputs that reach deep into a reader.
- A finding breaks an invariant: an index out of range, a hang, or a
  broken round trip. A refusal is not a finding.
- Six readers ran for fixed budgets with recorded seeds and gave no
  finding, under the debug build and under ASan and UBSan.
- Coverage guidance found both planted bugs with fewer inputs than blind
  mutation, and shrinking reduced them to `00 00 02 00` and `ff 00`,
  which the checks keep as regression tests.

## Dependencies

- [17. JSON you can sign](ch017-json-you-can-sign.md)
- [23. Filters](ch023-filters.md)
- [26. WebSocket frames](ch026-websocket-frames.md)
- [93. Test vectors and their
  provenance](ch093-test-vectors-and-their-provenance.md)

## Sources

- George Marsaglia, "Xorshift RNGs", *Journal of Statistical Software*
  8(14), 2003.
- [libFuzzer](https://llvm.org/docs/LibFuzzer.html) and
  [AFL++](https://aflplus.plus/docs/), for corpus, coverage and
  minimization.
- [NIP-19](../nips/official/19.md) for the `nprofile` seed and
  [NIP-44](../nips/official/44.md) for the payload seed; the vectors are
  pinned in [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
