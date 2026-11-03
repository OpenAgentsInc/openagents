# 116. Test vectors and their provenance

Most chapters in this book end with the same claim: the code gives the
published answer on every published vector. That claim is only as good as
the vectors. If a file of expected results is edited, or a chapter reads a
copy that drifted from the upstream file, or a generator quietly changes
the expected values it writes, the checks still pass and prove nothing.

This chapter describes how the book pins every file it reads, so that each
expected value can be traced to one exact upstream revision and checked
byte for byte. It names the Bitcoin Core revision the book targets, adds a
check that reports any Core file pinned at another revision, and reports
what an audit of all 316 pins found and fixed.

## What this chapter teaches

- What a pin records: the file's bytes, an immutable source address, the
  license and the chapters it covers.
- How `check_vectors.py` checks the pins offline, and how `--fetch`
  compares them with the network.
- The difference between published vectors, which the book copies, and
  generated vectors, which a script writes from a pinned source when no
  one publishes the cases a chapter needs.
- Which Bitcoin Core revision the book targets, and why it is not a release.
- How `check_pins.py` checks that the pins agree with each other, with
  `SOURCES.md` and with the chapters that cite them.

## Where the code is

| File | What it does |
| --- | --- |
| [upstream/](../../coax/tests/vectors/upstream/) | the pinned files, byte for byte as published |
| [manifest.json](../../coax/tests/vectors/manifest.json) | one entry for each pinned file: path, URL, revision, license, coverage, date, byte count and SHA-256 |
| [SOURCES.md](../../coax/tests/vectors/SOURCES.md) | the same inventory for people, the reader rules for each source, the Core target and the pins by chapter |
| [check_vectors.py](../../coax/tests/vectors/check_vectors.py) | checks every file against its entry; `--fetch` re-reads the URLs |
| [test_integrity.py](../../coax/tests/vectors/test_integrity.py) | shows that `check_vectors.py` refuses each kind of damage |
| [pins.json](../../coax/tests/vectors/pins.json) | the Bitcoin Core revision the book targets, and the one exception |
| [check-pins.coax](../../coax/tests/vectors/check-pins.coax) | checks the Core target, the inventory, the citations, the chapter index and, with `--regenerate`, every generator |
| [test_pins.py](../../coax/tests/vectors/test_pins.py) | independently checks the Python reference and shows that `check_pins.py` refuses each kind of disagreement |
| [make_provenance_vectors.py](../../coax/tests/bitcoin/make_provenance_vectors.py) | copies one pinned file and its entry into the example's table |
| [provenance.coax](../examples/provenance.coax) | the example: hash the bytes, compare with the pin, then use the rows |

## What a pin records

Every file a chapter reads as expected data lives under
`coax/tests/vectors/upstream/` and has one entry in `manifest.json`. This
is the entry for BIP-340's test vectors, which chapter 23 and this
chapter's example read:

```text
{
  "path": "upstream/bip340/test-vectors.csv",
  "kind": "upstream",
  "url": "https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0340/test-vectors.csv",
  "revision": "git:5aadddb511a5dcfe4599531bd8ae5e5b96d0f793",
  "license": "BSD-2-Clause OR MIT OR CC0-1.0",
  "covers": ["bip340"],
  "retrieved": "2026-10-28",
  "sha256": "34c9d1d9c3a88d524bc80778540dc43f8306ec249a7485293063c376db851c2d",
  "bytes": 6892
}
```

The URL names a commit, not a branch. A branch such as `master` moves;
a commit hash does not, so anyone can fetch the same bytes later. The
`revision` field takes one of five forms, and `check_vectors.py` checks
each one:

| Revision | Count | What makes it immutable |
| --- | ---: | --- |
| `git:` and a 40-digit commit | 271 | the URL must contain the same commit |
| `RFC ` and a number | 7 | the URL must be that RFC's text at the RFC Editor; RFCs are never revised |
| `NIST ...` | 5 | a member of a NIST archive, with the archive's own SHA-256 |
| `static:` and a description | 32 | an author-hosted file with no version history; the SHA-256 is the pin |
| `adapter:` | 1 | the one derived file, described below |

Six of the `static:` files are assembled rather than downloaded: block
headers and spent outputs that a chapter read from mempool.space one
height or one block at a time. No single URL returns their bytes. Their
entries say `"assembled": true`, which `check_vectors.py` accepts only
with a `static:` revision, and `--fetch` skips them. The chapters that
read them check what can be checked without a network: each header
hashes to its block hash and names the one before.

`.gitattributes` in the vector folder turns off Git's line-ending
conversion for `upstream/`. The BIP-340 file ends each line with a
carriage return and a line feed, and the pin covers those bytes. A
checkout that converted them would change the hash, and the example
shows that the check then fails.

## Checking the pins

`check_vectors.py` reads every entry and refuses the corpus if any of these
fail: the path is inside the vector folder, is not a link and appears
once; the file's SHA-256 and byte count equal the entry; the URL is HTTPS
and the revision is immutable in one of the forms above; the source,
license and coverage are present; and every file under `upstream/` has an
entry. It needs no network and only Python's standard library.

```shell
python3 coax/tests/vectors/check_vectors.py
```

```text
{"assembled_files": 6, "bytes": 13821142, "derived_files": 1, "network": false, "upstream_files": 315}
```

`--fetch` downloads every URL again and compares the bytes with the pin.
It never rewrites a file: a changed upstream is a finding, and changing a
pin is a reviewed commit of its own.

```shell
python3 coax/tests/vectors/check_vectors.py --fetch
```

`test_integrity.py` damages a copy of the corpus in each way the checker
must refuse (a changed byte, a missing file, an unlisted file, a missing
license, a branch URL, a static entry with no description, a path that
escapes the folder, a duplicate, a link, and a derived file whose inputs
changed) and confirms that each one fails.

```shell
python3 coax/tests/vectors/test_integrity.py
```

The checks protect the bytes, not the manifest itself. Someone who
replaces a file and its entry together passes them. That change is a
visible edit to `manifest.json` in Git, and review is what catches it.

## Published and generated vectors

A published vector comes with its expected result from the people who
wrote the rule: BIP-340's CSV, BIP-341's wallet vectors, Bitcoin Core's
`script_tests.json`, Wycheproof's ECDSA cases. The book copies these files
whole and reads them in place, or a `make_*_vectors.py` script copies the
rows a chapter needs into a Coax table and changes nothing.

Some rules have no published vectors, so a script writes them. The book
uses three kinds of generator:

- **Transliterated from Core.** Core tests timelocks by building a chain
  in memory and publishes no vectors for them. Chapter 39's
  [make_locktime_vectors.py](../../coax/tests/bitcoin/make_locktime_vectors.py)
  transliterates `IsFinalTx` and the sequence-lock functions from the
  pinned `consensus/tx_verify.cpp` into Python and writes the vectors with
  its own verdicts.
- **Run from Core's own Python.** BIP-341's vectors cover only key-path
  spends without an annex. Chapter 43's
  [make_taproot_sighash_vectors.py](../../coax/tests/bitcoin/make_taproot_sighash_vectors.py)
  runs `TaprootSignatureMsg` from Core's pinned `test_framework/script.py`
  as written, first on BIP-341's own transaction, where it must give the
  seven published messages, then on 30 cases the BIP does not cover.
- **Made by an independent implementation.** No BIP publishes adaptor
  signature vectors, and BOLT 7 publishes none for `channel_announcement`
  or `channel_update`. Chapter 103's
  [adaptor.py](../../coax/tests/bitcoin/adaptor.py) builds them on the
  point and hash functions of BIP-340's pinned `reference.py`. Chapter 112
  makes its gossip messages with keys of its own and recomputes every line
  in Python.

A generated vector is weaker than a published one: it shows that two
implementations agree, not that either agrees with the rule's authors.
The book keeps three rules for them. The generator checks the SHA-256 of
every pinned file it reads before it reads it. It shares no code with
Coax. Where any published case exists, the generator must reproduce it
first, as chapter 43's does with BIP-341's seven messages.

The one derived file in the manifest, `derived/catalog.json`, is an index
of which cases each source contains. Its entry records the generator's
own SHA-256 and the SHA-256 of each input, so `check_vectors.py` refuses
it when either changes.

## The Bitcoin Core target

The book reads 142 files from Bitcoin Core: unit test data, functional
test framework code and the C++ sources that chapters transliterate. They
are pinned at one revision, recorded in
[pins.json](../../coax/tests/vectors/pins.json):

```text
ddea9196493d3b8fd841a36a6c53efd85dac7a05
```

It is the merge of pull request 36554 into `master` on 2026-11-02. It is
not a release tag. On 2026-11-03 it was 741 commits past `d25470d9`, where
the v32 release branch left `master`, and the latest release was v32.1
(`f0de87f0`, 2026-10-26). So the target is a development revision that
contains everything in v32 and changes since. The book targets it because
most chapters were written against it, and moving 141 pinned files to
v32.1 would change data that every one of those chapters has checked. A
chapter that describes policy, such as standardness or the size of
`OP_RETURN` outputs, describes the policy at this revision, not at a
release.

One Core file is pinned elsewhere, and `pins.json` lists it with its
reason: `chainparams_v22.cpp`, at the v22.0 tag (`a0988140`). Chapter 51
reads Taproot's Speedy Trial deployment parameters from it. Later releases
bury Taproot at a fixed height and no longer list those parameters.

## Checking that the pins agree

`check_vectors.py` checks each pin on its own. The new
[check_pins.py](../../coax/tests/vectors/check_pins.py) checks that they
agree with each other and with the book:

1. Every Bitcoin Core file is pinned at the target revision, or
   `pins.json` lists it as an exception with a reason. An exception must
   name a pinned file that is not at the target.
2. Every manifest entry has exactly one row in `SOURCES.md`'s inventory,
   with the same URL and SHA-256, and every row has an entry.
3. Every path under `coax/tests/vectors/upstream/` that a chapter, example,
   library or test names is pinned.
4. `SOURCES.md` lists, for each chapter, the pinned files the chapter
   cites. `--write` rebuilds the list, and the check fails when it is
   stale.
5. With `--regenerate`, it copies `coax/tests/` to a scratch folder, runs
   every `make_*.py` generator there and compares each file with the
   committed one byte for byte.

```shell
coax/web/tool.sh coax/tests/vectors/check-pins.coax --regenerate
```

```text
Bitcoin Core target ddea9196 (master on 2026-11-02, the merge of bitcoin#36554; not a release tag): 142 pinned files, 1 at another revision
  upstream/bitcoin-core/chainparams_v22.cpp at a0988140: v22.0 (tag v22.0, a0988140): chapter 51 reads Taproot's Speedy Trial deployment parameters from it; later releases bury Taproot at a fixed height and no longer list them
  note: bitcoin/bips is pinned at 3 revisions: 5aadddb5, 927b6de9, ada34196
316 manifest entries, 389 citations checked
47 generators rebuilt their files in a scratch copy
check_pins.py: every pin agrees
```

The note is not a failure. The BIPs repository holds independent
documents, and each file is pinned at the commit it was fetched from;
moving one BIP never changes another's text. Only Core is held to one
revision, because its files describe one program.

[test_pins.py](../../coax/tests/vectors/test_pins.py) changes a copy of
the pins in each way the check must refuse: a Core file at another
revision, an exception with no reason, for a missing file or for a file at
the target, an inventory row removed, repeated or with another hash, a
citation of an unpinned file, and a stale or missing index.

```shell
python3 coax/tests/vectors/test_pins.py
```

## What the audit found

On 2026-11-03 the new checks ran over all 316 pins for the first time.
They found:

- **Four files with no inventory row.** NIST's `SHA512ShortMsg.rsp` and
  `SHA512LongMsg.rsp`, added for chapter 7, and mempool.space's
  `retarget-boundaries.txt` and `headers-0-2016.txt`. Each had a correct
  manifest entry, so `check_vectors.py` passed. The rows are added.
- **Six pins that `--fetch` could not reproduce.** These are the six
  assembled files. `--fetch` stopped at the first one, so a run could not
  confirm the files after it. They are now marked `"assembled": true` and
  skipped, and `--fetch` compared the other 309 upstream files with no
  difference.
- **No list of pins by chapter.** It is added to `SOURCES.md` and covers
  every chapter that cites a pinned file.
- **No Bitcoin Core file at an unlisted revision**, no citation of an
  unpinned file and no generator that writes different bytes.

## The example

The example takes one pinned file, BIP-340's `test-vectors.csv`, and
does what every chapter's checks rely on: it hashes the bytes in Coax,
compares the hash with the pin, and only then reads the rows and verifies
each signature with chapter 23's `schnorr_verify_vartime`. Then it edits
one byte, and strips the carriage returns, to show what the pin refuses.
[make_provenance_vectors.py](../../coax/tests/bitcoin/make_provenance_vectors.py)
copies the file's bytes and its manifest entry into
[provenance_vectors.coax](../../coax/tests/bitcoin/provenance_vectors.coax),
and refuses if the file does not match its entry.

<!-- source: bitcoin/examples/provenance.coax example -->
```jai
#load "../../coax/lib/crypto/schnorr.coax";
#load "../../coax/lib/nostr/hex.coax";
#load "../../coax/tests/bitcoin/provenance_vectors.coax";
#import "Basic";

PV_ROOM :: 8192;
CR :: 13;
LF :: 10;
COMMA :: 44;

// The pinned file's bytes, decoded from the generated table. Every chunk is
// valid hex, so a decoding error stops the program.
load_pinned :: (out: [] u8) -> int {
  count := 0;
  for chunk: PV_HEX {
    r := hex_decode_checked(chunk, out[count..<out.count], false);
    #assert r.error == 0;
    count += r.written;
  }
  return count;
}
// SHA-256 of bytes as lowercase hex, written into text.
digest_hex :: (bytes: [] u8, text: [] u8) -> string {
  digest: [32] u8;
  #assert sha256_hash_checked(bytes, digest[..], false) == SHA_OK;
  r := hex_encode_checked(cast(string) digest[..], text, false);
  return cast(string) text[0..<r.written];
}
// The pin decides: the bytes are used only when their count and SHA-256
// equal the manifest entry.
matches_pin :: (bytes: [] u8) -> bool {
  text: [64] u8;
  return bytes.count == PV_BYTES && digest_hex(bytes, text[..]) == PV_SHA256;
}

// A line runs from start to end, without its CR LF; the next line starts at
// next.
Line :: struct { start: int; end: int; next: int; }
line_at :: (bytes: [] u8, start: int) -> Line {
  i := start;
  while i < bytes.count && bytes[i] != LF { i += 1; }
  end := i;
  if end > start && bytes[end - 1] == CR { end -= 1; }
  return Line.{start = start, end = end, next = ifx i < bytes.count then i + 1 else i};
}
// Field k of a line, as byte offsets, and as a slice. The file has no
// quoted fields.
Span :: struct { start: int; end: int; }
field_span :: (bytes: [] u8, line: Line, k: int) -> Span {
  start := line.start; seen := 0;
  for i: line.start..<line.end {
    if bytes[i] == COMMA {
      if seen == k { return Span.{start = start, end = i}; }
      seen += 1; start = i + 1;
    }
  }
  return ifx seen == k then Span.{start = start, end = line.end} else Span.{start = line.end, end = line.end};
}
field :: (bytes: [] u8, line: Line, k: int) -> [] u8 {
  s := field_span(bytes, line, k);
  return bytes[s.start..<s.end];
}
// Decode a hex field into out, refusing a wrong length.
decode_field :: (text: [] u8, out: [] u8, want: int) -> int {
  r := hex_decode_checked(cast(string) text, out, false);
  if r.error != 0 || (want >= 0 && r.written != want) { return -1; }
  return r.written;
}
// Verify one row: public key, message and signature from fields 2, 4 and 5.
// The result is 1 for valid, 0 for refused, -1 for a field that doesn't
// decode.
verify_row :: (bytes: [] u8, line: Line) -> int {
  key: [32] u8; message: [100] u8; sig: [64] u8;
  if decode_field(field(bytes, line, 2), key[..], 32) < 0 { return -1; }
  n := decode_field(field(bytes, line, 4), message[..], -1);
  if n < 0 { return -1; }
  if decode_field(field(bytes, line, 5), sig[..], 64) < 0 { return -1; }
  return ifx schnorr_verify_vartime(message[0..<n], key[..], sig[..]) then 1 else 0;
}
// The byte offset of field 5 (the signature) on data row 0.
row0_signature :: (bytes: [] u8) -> int {
  row0 := line_at(bytes, line_at(bytes, 0).next);
  return field_span(bytes, row0, 5).start;
}

main :: () {
  storage: [PV_ROOM] u8;
  file := storage[0..<load_pinned(storage[..])];
  text: [64] u8;
  print("pinned file: %\n", PV_PATH);
  print("revision: %\n", PV_REVISION);
  print("bytes: %, the pin says %\n", file.count, PV_BYTES);
  print("SHA-256: %\n", digest_hex(file, text[..]));
  print("the pin: %\n", PV_SHA256);
  print("matches the pin: %\n", matches_pin(file));
  if !matches_pin(file) { print("refusing the rows\n"); return; }

  // The rows, read only after the pin agrees.
  header := line_at(file, 0);
  print("columns: %\n", cast(string) file[header.start..<header.end]);
  agree := 0; rows := 0; line := line_at(file, header.next);
  while line.start < file.count {
    expected := cast(string) field(file, line, 6) == "TRUE";
    got := verify_row(file, line);
    print("row %: expected %, verifies %", cast(string) field(file, line, 0), expected, got == 1);
    comment := field(file, line, 7);
    if comment.count > 0 { print(": %", cast(string) comment); }
    print("\n");
    if got == ifx expected then 1 else 0 { agree += 1; }
    rows += 1;
    line = line_at(file, line.next);
  }
  print("% of % rows give their published result\n", agree, rows);

  // One edited byte: the first hex digit of row 0's signature.
  edited: [PV_ROOM] u8;
  for i: 0..<file.count { edited[i] = file[i]; }
  copy := edited[0..<file.count];
  at := row0_signature(copy);
  print("edit byte %, row 0's signature, % to %\n", at, cast(string) copy[at..<at + 1], "F");
  copy[at] = 'F';
  print("SHA-256: %\n", digest_hex(copy, text[..]));
  print("matches the pin: %\n", matches_pin(copy));
  row0 := line_at(copy, line_at(copy, 0).next);
  print("unpinned, row 0 would read: expected TRUE, verifies %\n", verify_row(copy, row0) == 1);

  // The same rows with LF line endings, as a checkout that converts them
  // would leave the file.
  unix: [PV_ROOM] u8; n := 0;
  for b: file { if b != CR { unix[n] = b; n += 1; } }
  print("without its % carriage returns: % bytes, matches the pin %\n", file.count - n, n, matches_pin(unix[0..<n]));
}
```

```shell
coax/bin/coax run bitcoin/examples/provenance.coax
```

```text
pinned file: upstream/bip340/test-vectors.csv
revision: git:5aadddb511a5dcfe4599531bd8ae5e5b96d0f793
bytes: 6892, the pin says 6892
SHA-256: 34c9d1d9c3a88d524bc80778540dc43f8306ec249a7485293063c376db851c2d
the pin: 34c9d1d9c3a88d524bc80778540dc43f8306ec249a7485293063c376db851c2d
matches the pin: true
columns: index,secret key,public key,aux_rand,message,signature,verification result,comment
row 0: expected true, verifies true
row 1: expected true, verifies true
row 2: expected true, verifies true
row 3: expected true, verifies true: test fails if msg is reduced modulo p or n
row 4: expected true, verifies true
row 5: expected false, verifies false: public key not on the curve
row 6: expected false, verifies false: has_even_y(R) is false
row 7: expected false, verifies false: negated message
row 8: expected false, verifies false: negated s value
row 9: expected false, verifies false: sG - eP is infinite. Test fails in single verification if has_even_y(inf) is defined as true and x(inf) as 0
row 10: expected false, verifies false: sG - eP is infinite. Test fails in single verification if has_even_y(inf) is defined as true and x(inf) as 1
row 11: expected false, verifies false: sig[0:32] is not an X coordinate on the curve
row 12: expected false, verifies false: sig[0:32] is equal to field size
row 13: expected false, verifies false: sig[32:64] is equal to curve order
row 14: expected false, verifies false: public key is not a valid X coordinate because it exceeds the field size
row 15: expected true, verifies true: message of size 0 (added 2022-12)
row 16: expected true, verifies true: message of size 1 (added 2022-12)
row 17: expected true, verifies true: message of size 17 (added 2022-12)
row 18: expected true, verifies true: message of size 100 (added 2022-12)
19 of 19 rows give their published result
edit byte 346, row 0's signature, E to F
SHA-256: ad476ec4075842fcd34cfb7708d37cd0d8c92d14401fbf837cf51192ccbe9465
matches the pin: false
unpinned, row 0 would read: expected TRUE, verifies false
without its 20 carriage returns: 6872 bytes, matches the pin false
```

The edited file shows why the hash comes first. Without it, the changed
row reads as a verifier that refuses a valid signature, and you would
look for the bug in the wrong place. With it, the file is refused before
any row is read. The same holds the other way: an edit that turned a
`FALSE` into `TRUE` alongside a matching change to a signature would make
a broken verifier look correct, and the pin refuses that file too.

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/provenance.coax](../examples/checks/provenance.coax),
which loads the example. A changed bit at twelve seeded positions, and a
missing last byte, must each refuse the pin; the parser must find the
columns it reads under their published names; and every row must give its
published result:

<!-- source: bitcoin/examples/checks/provenance.coax check -->
```jai
// Any one changed bit, anywhere in the file, and any missing last byte,
// refuses the pin.
#forall "one changed bit or a missing byte refuses the pin" seed: 0..<12 #steps 2000000000 {
  storage: [PV_ROOM] u8;
  file := storage[0..<load_pinned(storage[..])];
  #assert matches_pin(file);
  at := (seed * 2654435761) % file.count;
  file[at] ^= cast(u8) (1 << (seed % 8));
  #assert !matches_pin(file);
  file[at] ^= cast(u8) (1 << (seed % 8));
  #assert matches_pin(file) && !matches_pin(file[0..<file.count - 1]);
}
// The parser reads the columns the rows are verified by, and every row
// gives its published result.
#test "the columns are where the parser reads them and every row agrees" #steps 2000000000 {
  storage: [PV_ROOM] u8;
  file := storage[0..<load_pinned(storage[..])];
  header := line_at(file, 0);
  #assert cast(string) field(file, header, 2) == "public key";
  #assert cast(string) field(file, header, 4) == "message";
  #assert cast(string) field(file, header, 5) == "signature";
  #assert cast(string) field(file, header, 6) == "verification result";
  rows := 0; line := line_at(file, header.next);
  while line.start < file.count {
    expected := cast(string) field(file, line, 6) == "TRUE";
    #assert verify_row(file, line) == ifx expected then 1 else 0;
    rows += 1; line = line_at(file, line.next);
  }
  #assert rows == 19;
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/provenance.coax
```

`coax test` runs 140 checks, including those of the files it loads, in
about 21 seconds on 2026-11-03. Mutation testing: not run (owner speed
order).

### Python

[provenance.py](../../coax/tests/bitcoin/provenance.py) computes every
line of the example without the generated table: it reads the pinned file,
hashes it with `hashlib`, parses it with Python's `csv` module and
verifies each row with BIP-340's own `reference.py`, which is pinned
beside the CSV and checked against its hash before it runs. It also
confirms that the generated table holds the file's exact bytes.

```shell
python3 -I coax/tests/bitcoin/provenance.py --coax coax/bin/coax
```

On 2026-11-03 all 32 lines agreed, and the example's output in native
debug and release builds equaled them. Wasm was not run.

## What this chapter leaves out

- Signed provenance. The pins prove that the bytes are the ones recorded,
  and Git review proves who recorded them. Nothing here checks a
  signature from the upstream authors.
- Moving the Core target to a release. The book stays on `ddea9196` until
  a chapter needs a later change, or until a release contains everything
  the chapters read; moving it is one reviewed commit that repins every
  Core file and reruns every chapter.
- Comparing the book's results with Core and libsecp256k1 on generated
  inputs. That is chapter 117.

## Summary

- Every file the book reads as expected data is pinned in
  `manifest.json` by an immutable source address, its byte count and its
  SHA-256, and `check_vectors.py` checks every byte offline.
- Published vectors are copied unchanged. Generated vectors come from a
  pinned source, by a script that shares no code with Coax and reproduces
  any published case first.
- The book targets Bitcoin Core `ddea9196`, a `master` revision of
  2026-11-02 past v32, not a release. One file, `chainparams_v22.cpp`, is
  a listed exception.
- `check_pins.py` checks the Core target, the inventory, the citations,
  the chapter index and every generator. Its first run found four missing
  inventory rows and six pins `--fetch` could not reproduce, and both are
  fixed.
- A file is used only after its hash matches the pin, so an edited
  vector is refused before it can make a wrong result look right.

## Dependencies

None. Every chapter that reads a pinned file depends on this one.

## Sources

- [SOURCES.md](../../coax/tests/vectors/SOURCES.md) and
  [manifest.json](../../coax/tests/vectors/manifest.json): the inventory,
  the reader rules for each source and the pins by chapter.
- BIP-340, "Schnorr Signatures for secp256k1", with its test vectors at
  [coax/tests/vectors/upstream/bip340/test-vectors.csv](../../coax/tests/vectors/upstream/bip340/test-vectors.csv)
  and its reference implementation at
  [reference.py](../../coax/tests/vectors/upstream/bip340/reference.py).
- Bitcoin Core at `ddea9196493d3b8fd841a36a6c53efd85dac7a05`, pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/chainparams_v22.cpp),
  and the v32.1 release notes for the relation to the latest release.

The production provenance checker is Coax (`check-pins.coax` and `pin-policy.coax`).
`check_pins.py` remains an independent Python reference for parity and the
explicit `--regenerate` oracle run; it does not supply the production pin,
inventory, citation or chapter-index decisions.
