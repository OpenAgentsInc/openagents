# 93. Test vectors and their provenance

A test vector is only useful if you know where it came from. A digest that
matches a file you copied from an unknown place proves only that the copy is
consistent with itself. This chapter sets out how the stack pins every vector
file it checks against: the source URL, an immutable revision, the license,
the retrieval date, the byte count and the SHA-256 of the exact bytes. It
then checks those records in Coax.

Part VII, Interop and verification. Result: Every vector file pinned by source
URL, revision, license, retrieval date and SHA-256, with derived adapters
labelled and their input hashes recorded.

## What this chapter teaches

- The inventory: NIST SHA-256, RFC 4231/5869/7914/8439/4648/8785/8259,
  BIP-173, BIP-340, NIP-44, Wycheproof, JSONTestSuite and four NIP texts, and
  which chapter uses each set.
- What one provenance record holds, worked by hand for a 9-byte file.
- The rules a record must follow: an immutable revision, a well-formed
  digest, a path inside the corpus, and bytes that hash to the digest.
- Why upstream bytes are never reformatted, and how a derived file names its
  inputs.
- The offline integrity check and the explicit `--fetch` comparison.

## Where the code is

| File | What it does |
| --- | --- |
| [coax/tests/vectors/manifest.json](../../coax/tests/vectors/manifest.json) | One record per corpus file: 31 upstream and 1 derived |
| [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md) | The same inventory for people, with scope and reader rules |
| [coax/tests/vectors/check_vectors.py](../../coax/tests/vectors/check_vectors.py) | The offline audit of the files on disk; `--fetch` re-reads the pinned URLs |
| [coax/tests/vectors/test_integrity.py](../../coax/tests/vectors/test_integrity.py) | Ten tests that tamper with a copy of the corpus and expect a refusal |
| [coax/tests/vectors/make_catalog.py](../../coax/tests/vectors/make_catalog.py) | The one derived adapter, which writes `derived/catalog.json` |
| [nostr/examples/vector-provenance.coax](../examples/vector-provenance.coax) | The same record rules in Coax, run over 14 records and the bytes of 9 files |

The Python files are test tooling. Nothing ships them, and no build runs them.
The Coax example reads the rules, not the disk: Coax has no compile-time
file reads (spec section 10.1), and the book's examples are pure programs. It
carries the nine small JSONTestSuite files as hex, copied with `xxd -p`, and
checks them against their records.

## The inventory

All files sit under `coax/tests/vectors/upstream/`. All were retrieved on
2026-10-28. The full 64-digit digests are in
[SOURCES.md](../../coax/tests/vectors/SOURCES.md).

| Set | Files | Revision | Used by |
| --- | --- | --- | --- |
| NIST CAVS 11.0 SHA-256 | `nist/SHA256ShortMsg.rsp`, `nist/SHA256LongMsg.rsp`, `nist/Readme.txt` | archive `shabytetestvectors.zip`, SHA-256 `929ef80b...` | [4](../../bitcoin/book/ch004-sha-256.md) |
| RFC 4231 | `rfc/rfc4231.txt` | RFC 4231 | [5](../../bitcoin/book/ch008-hmac-sha256.md) |
| RFC 5869 | `rfc/rfc5869.txt` | RFC 5869 | [6](../../bitcoin/book/ch009-hkdf.md) |
| RFC 7914 | `rfc/rfc7914.txt` | RFC 7914 | [47](ch047-encrypted-key-backup.md) (cited for scrypt; its section 11 PBKDF2 cases are not yet run) |
| RFC 8439 | `rfc/rfc8439.txt` | RFC 8439 | [18](../../bitcoin/book/ch011-chacha20.md), [20](ch020-nip-44-encrypted-payloads.md) |
| RFC 4648 | `rfc/rfc4648.txt` | RFC 4648 | [1](../../bitcoin/book/ch001-bytes-hex-and-base64.md) |
| RFC 8785, RFC 8259 | `rfc/rfc8785.txt`, `rfc/rfc8259.txt` | RFC 8785, RFC 8259 | [17](ch017-json-you-can-sign.md) |
| BIP-340 | `bip340/test-vectors.csv`, `bip340/LICENSE` | `git:5aadddb5...` (bitcoin/bips) | [10](../../bitcoin/book/ch016-scalar-multiplication.md), [11](../../bitcoin/book/ch023-verify-a-schnorr-signature.md), [12](../../bitcoin/book/ch018-keys-and-randomness.md), [13](../../bitcoin/book/ch024-sign-with-schnorr.md), [21](ch021-events-ids-and-signatures.md) |
| BIP-173 | `bip173/bip-0173.mediawiki` | `git:5aadddb5...` (bitcoin/bips) | [15](../../bitcoin/book/ch027-bech32.md) |
| NIP-44 | `nip44/nip44.vectors.json`, `nip44/LICENSE` | `git:671a1f04...` (paulmillr/nip44) | [6](../../bitcoin/book/ch009-hkdf.md), [14](ch014-shared-secrets-with-ecdh.md), [20](ch020-nip-44-encrypted-payloads.md) |
| Wycheproof | `wycheproof/ecdh_secp256k1_test.json`, `wycheproof/LICENSE` | `git:d17cc89f...` (C2SP/wycheproof) | [14](ch014-shared-secrets-with-ecdh.md) |
| JSONTestSuite | nine files in `json/`, `json/LICENSE` | `git:1ef36fa0...` (nst/JSONTestSuite) | [17](ch017-json-you-can-sign.md) |
| NIP texts | `nips/01.md`, `nips/19.md`, `nips/44.md`, `nips/59.md` | `git:400d157c...` (nostr-protocol/nips) | [15](../../bitcoin/book/ch027-bech32.md), [16](ch016-nip-19-entities-and-nostr-uris.md), [20](ch020-nip-44-encrypted-payloads.md) |
| Source index (derived) | `derived/catalog.json` | `adapter:source-index-v1` | this chapter |

Some vectors live outside this corpus, in other pinned files:

- NIP-06 and NIP-49 vectors are in the NIP texts under
  [nostr/nips/official/](../nips/official/), pinned by
  [nostr/nips/upstream-lock.json](../nips/upstream-lock.json) and used by
  chapters [46](ch046-keys-from-a-seed-phrase.md) and
  [47](ch047-encrypted-key-backup.md).
- The Block NIP fixtures used by chapter
  [91](ch091-git-signing-projects-leases-and-read-state.md) are under
  [nostr/nips/block/](../nips/block/).
- RFC 6455's example key (chapter [26](ch026-websocket-frames.md)) is one
  value quoted from the RFC, not a pinned file.
- The seven BIP-350 Bech32m strings in chapter [15](../../bitcoin/book/ch027-bech32.md)'s
  checks are copied from BIP-350, which is not yet pinned in the corpus.
  That is a known gap: those strings have a citation but no recorded digest.

[PLAN.md](PLAN.md) names the vector file in each chapter's "Done means" line,
so a reader can go from a chapter to the exact bytes it is checked against.

## One record, worked by hand

Take the smallest JSON file, `upstream/json/n_object_trailing_comma.json`.
JSONTestSuite uses it to check that a parser refuses a trailing comma. Its
text is `{"id":0,}`, nine bytes:

| Offset | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Byte | `7b` | `22` | `69` | `64` | `22` | `3a` | `30` | `2c` | `7d` |
| Character | `{` | `"` | `i` | `d` | `"` | `:` | `0` | `,` | `}` |

There is no newline at the end. The test suite's files are byte-exact, and a
trailing newline would make it a different file with a different digest.
This is why the corpus never runs a formatter over `upstream/`.

### The record's fields

1. **Source URL.** The file is
   `test_parsing/n_object_trailing_comma.json` in `nst/JSONTestSuite`. The
   URL names a commit, not a branch:
   `https://raw.githubusercontent.com/nst/JSONTestSuite/1ef36fa01286573e846ac449e8683f8833c5b26a/test_parsing/n_object_trailing_comma.json`.
   A branch such as `master` can move. A commit cannot.
2. **Revision.** `git:` followed by the same 40-digit commit. The rule is
   that the commit, between slashes, appears in the URL.
3. **License.** MIT, the suite's license. The corpus keeps the suite's
   `LICENSE` file beside the vectors, with its own record.
4. **Retrieved.** `2026-10-28`, the day the bytes were downloaded. It is not
   a claim about when the file was published.
5. **Bytes.** 9.
6. **SHA-256.** The digest of the nine bytes.

### The digest

SHA-256 (Bitcoin chapter 4) pads a message to a multiple of 64 bytes: the message,
one `0x80` byte, zero bytes, and the message length in bits as 8 big-endian
bytes. Nine bytes is 72 bits, `0x48`. One block holds it:

- bytes 0–8: the message;
- byte 9: `0x80`;
- bytes 10–55: 46 zero bytes;
- bytes 56–63: `00 00 00 00 00 00 00 48`.

9 + 1 + 46 + 8 = 64. The 16 message words are `W0 = 0x7b226964`,
`W1 = 0x223a302c`, `W2 = 0x7d800000`, `W3` to `W14` zero, and `W15 = 0x48`.
Bitcoin chapter 4 runs the 64 rounds on such a block. The result is:

```text
60e04bc71fcc4c57abeef95a9604a96aa60f833881a2f028d27bd4f03e176603
```

### The record

Together, the manifest entry is:

```text
{
  "path": "upstream/json/n_object_trailing_comma.json",
  "kind": "upstream",
  "url": "https://raw.githubusercontent.com/nst/JSONTestSuite/1ef36fa01286573e846ac449e8683f8833c5b26a/test_parsing/n_object_trailing_comma.json",
  "revision": "git:1ef36fa01286573e846ac449e8683f8833c5b26a",
  "license": "MIT",
  "covers": ["json"],
  "retrieved": "2026-10-28",
  "sha256": "60e04bc71fcc4c57abeef95a9604a96aa60f833881a2f028d27bd4f03e176603",
  "bytes": 9
}
```

### One byte changed

Change byte 6 from `0x30` (`0`) to `0x31` (`1`). The text becomes
`{"id":1,}`, still nine bytes, and its digest is

```text
cbebbf25b67aff657eafd9651125efd950be0618f4df8d1015aa51a4c81c0351
```

It is not the recorded digest. The byte count still matches, so only the
digest catches the change. The audit checks the digest first and refuses with
`digest mismatch: upstream/json/n_object_trailing_comma.json`.

## The record rules

`check_vectors.py` applies these rules to each record, in this order, and
stops at the first that fails:

| Rule | Refusal |
| --- | --- |
| The path starts with `upstream/` or `derived/` | `invalid corpus path` |
| `retrieved` has the form `YYYY-MM-DD` | `invalid retrieved` |
| `sha256` is 64 lowercase hex digits | `invalid digest` |
| The path has no empty, `.` or `..` segment and no symlink | `invalid path` |
| The file exists | `missing file` |
| The file's SHA-256 equals `sha256` | `digest mismatch` |
| The file's length equals `bytes` | `byte count mismatch` |
| The URL starts with `https://` | `source URL` |
| A `git:` revision is 40 lowercase hex digits and appears as `/<commit>/` in the URL | `unpinned revision` |
| An `RFC n` revision has the URL `https://www.rfc-editor.org/rfc/rfcn.txt` | `RFC revision` |
| Any other revision pins an archive member and the archive's SHA-256 | `unpinned revision`, `archive digest` |

Two more rules look at the corpus as a whole: every file under `upstream/`
and `derived/` must have a record (`unlisted corpus file`), and no path may
appear twice (`duplicate path`).

A derived file must say how it was made. `derived/catalog.json` names its
generator, `make_catalog.py`, with that script's SHA-256
(`280444035ce58c55...`), and the path and SHA-256 of each of its six inputs.
The audit checks that each input digest equals the input's own record, that
the generator's digest matches, and that running the generator over the
committed inputs reproduces the catalog byte for byte. The catalog indexes
rows and counts. It computes no cryptographic answers.

The digests show that the files have not changed since the manifest was
reviewed. They don't show that the manifest itself is honest. That trust
comes from the Git review of the commit that added each pin.

### Cross-pins

Two pins check each other. The NIP-44 text pins its own vector file by
digest, and the corpus pins both:

```shell
grep -n 269ed0f6 coax/tests/vectors/upstream/nips/44.md
```

```text
303:    269ed0f69e4c192512cc779e78c555090cebc7c785b609e338a62afc3ce25040  nip44.vectors.json
```

That is the digest recorded for `upstream/nip44/nip44.vectors.json`, which
was taken from a different repository at a different commit.

## The rules in Coax

The example holds 14 records copied from `manifest.json`: the nine
JSONTestSuite files, whose bytes it carries, and five large files whose
records it checks without their bytes. A `Pin` holds the fields the rules
read. The refusals use the same words as `check_vectors.py`:

<!-- source: nostr/examples/vector-provenance.coax records -->
```jai
// One record, the fields check_vectors.py reads.
Pin :: struct { path: string; url: string; revision: string; archive: string;
                retrieved: string; bytes: int; sha256: string; }
pin :: (i: int) -> Pin {
  return Pin.{path = PIN_PATH[i], url = PIN_URL[i], revision = PIN_REVISION[i],
              archive = PIN_ARCHIVE[i], retrieved = RETRIEVED,
              bytes = PIN_BYTES[i], sha256 = PIN_SHA256[i]};
}

// The refusals, worded as check_vectors.py words them.
OK :: 0;
REASON :: [11] string.["ok", "invalid corpus path", "invalid retrieved",
    "invalid digest", "invalid path", "digest mismatch", "byte count mismatch",
    "source URL", "unpinned revision", "RFC revision", "archive digest"];
```

The rules are split as the Python audit orders them. `check_record` looks at
the path, date and digest spelling. `check_bytes` hashes the bytes with
Bitcoin chapter 4's `sha256_hash_checked` and compares the digest, then the count.
`check_source` checks the URL and revision. The commit search starts at
position 7, the second slash of `https://`, because nothing before it can
begin `/<commit>/`:

<!-- source: nostr/examples/vector-provenance.coax rules -->
```jai
// The metadata rules, in check_vectors.py's order: path prefix, date,
// digest spelling, path segments. The URL and revision rules come after the
// bytes in that script, so they are a second step.
check_record :: (p: Pin) -> int {
  if !starts(p.path, "upstream/") && !starts(p.path, "derived/") { return 1; }
  if !date_shape(p.retrieved) { return 2; }
  if !lower_hex(p.sha256, 0, 64) { return 3; }
  if !segments_named(p.path) { return 4; }
  return OK;
}
check_source :: (p: Pin) -> int {
  if !starts(p.url, "https://") { return 7; }
  if starts(p.revision, "git:") {
    if !lower_hex(p.revision, 4, 40) { return 8; }
    // "/<commit>/" must sit in the URL: a branch name there can move.
    // Positions before 7 are inside "https:/".
    for at: 7..<p.url.count {
      if p.url[at] == '/' && match(p.url, at + 1, p.revision, 4) &&
         match(p.url, at + 41, "/", 0) { return OK; }
    }
    return 8;
  }
  if starts(p.revision, "RFC ") {
    // "RFC n" must name https://www.rfc-editor.org/rfc/rfcn.txt and no other.
    if !digits_from(p.revision, 4) { return 9; }
    n := p.revision.count - 4;
    if p.url.count != RFC_BASE.count + n + 4 { return 9; }
    if !starts(p.url, RFC_BASE) { return 9; }
    if !match(p.url, RFC_BASE.count, p.revision, 4) { return 9; }
    if !match(p.url, RFC_BASE.count + n, ".txt", 0) { return 9; }
    return OK;
  }
  // Neither a commit nor an RFC: the record must pin its archive.
  if p.archive.count == 0 { return 8; }
  if !lower_hex(p.archive, 0, 64) { return 10; }
  return OK;
}
// The bytes against the record: the digest first, then the count.
check_bytes :: (p: Pin, data: [] u8) -> int {
  digest: [32] u8; text: [64] u8;
  #assert sha256_hash_checked(data, digest[..], false) == SHA_OK;
  #assert hex_encode_checked(cast(string) digest[..], text[..], false).error == 0;
  if cast(string) text[..] != p.sha256 { return 5; }
  if data.count != p.bytes { return 6; }
  return OK;
}
// The whole audit of one record, with or without its bytes.
audit :: (p: Pin, data: [] u8, have_bytes: bool) -> int {
  r := check_record(p);
  if r != OK { return r; }
  if have_bytes { r = check_bytes(p, data); if r != OK { return r; } }
  return check_source(p);
}
```

`pad_block` builds the padded block worked by hand above. It refuses a
message of 56 bytes or more, which needs a second block:

<!-- source: nostr/examples/vector-provenance.coax pad -->
```jai
// The one padded SHA-256 block of a message: its bytes, 0x80, zeros, and
// the length in bits as 8 big-endian bytes. 56 bytes or more need two.
Block :: struct { ok: bool; bytes: [64] u8; }
pad_block :: (data: [] u8) -> Block {
  r: Block;
  if data.count >= 56 { return r; }
  for i: 0..<data.count { r.bytes[i] = data[i]; }
  r.bytes[data.count] = 0x80;
  bits := data.count * 8;
  for i: 0..<8 { r.bytes[63 - i] = cast(u8) ((bits >> (8 * i)) & 0xff); }
  r.ok = true;
  return r;
}
```

`main` prints the worked record, audits all 14 records, and then changes one
field at a time:

<!-- source: nostr/examples/vector-provenance.coax main -->
```jai
main :: () {
  // The worked record: n_object_trailing_comma.json.
  buf: [1000] u8;
  comma := file_bytes(2, buf[..]);
  print("file: %, % bytes, text %\n", PIN_PATH[2], comma.count, cast(string) comma);
  block := pad_block(comma);
  #assert block.ok;
  show("padded block", block.bytes[..]);
  digest: [32] u8;
  #assert sha256_hash_checked(comma, digest[..], false) == SHA_OK;
  show("sha256", digest[..]);

  // Every record's metadata, and the bytes of the nine small files.
  passed := 0;
  for i: 0..<PINS {
    data := buf[0..<0];
    if i < FILES { data = file_bytes(i, buf[..]); }
    r := audit(pin(i), data, i < FILES);
    say(pin(i), r);
    if r == OK { passed += 1; }
  }
  print("% of % records pass, % with their bytes\n", passed, PINS, FILES);

  // Refusals. One byte changed: {"id":0,} becomes {"id":1,}.
  changed := file_bytes(2, buf[..]);
  changed[6] = '1';
  say(pin(2), audit(pin(2), changed, true));
  // Right bytes, wrong count in the record.
  wrong := pin(2); wrong.bytes = 10;
  say(wrong, audit(wrong, file_bytes(2, buf[..]), true));
  // A branch name where the commit should be.
  moving := pin(9);
  moving.url = "https://raw.githubusercontent.com/bitcoin/bips/master/bip-0340/test-vectors.csv";
  say(moving, audit(moving, buf[0..<0], false));
  // An RFC record pointing at another RFC's URL.
  other := pin(11); other.url = "https://www.rfc-editor.org/rfc/rfc7539.txt";
  say(other, audit(other, buf[0..<0], false));
  // An archive member with no archive digest, and with a cut one.
  loose := pin(12); loose.archive = "";
  say(loose, audit(loose, buf[0..<0], false));
  loose.archive = "929ef80b7b3418aca026643f6f248815";
  say(loose, audit(loose, buf[0..<0], false));
  // A digest in capitals, a path that climbs out, a path outside the corpus.
  loud := pin(10); loud.sha256 = "269ED0F69E4C192512CC779E78C555090CEBC7C785B609E338A62AFC3CE25040";
  say(loud, audit(loud, buf[0..<0], false));
  climb := pin(10); climb.path = "upstream/../manifest.json";
  say(climb, audit(climb, buf[0..<0], false));
  outside := pin(10); outside.path = "nip44.vectors.json";
  say(outside, audit(outside, buf[0..<0], false));
  late := pin(10); late.retrieved = "28 Oct 2026";
  say(late, audit(late, buf[0..<0], false));
}
```

## The example

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/vector-provenance.coax
```

The first three lines are the record worked by hand: 9 bytes, the padded
block ending `...0048`, and the digest `60e04bc7...`. All 14 records pass.
Then each refusal names its rule and its path:

```text
file: upstream/json/n_object_trailing_comma.json, 9 bytes, text {"id":0,}
padded block: 7b226964223a302c7d80000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000048
sha256: 60e04bc71fcc4c57abeef95a9604a96aa60f833881a2f028d27bd4f03e176603
ok: upstream/json/i_string_UTF-16LE_with_BOM.json
ok: upstream/json/i_structure_500_nested_arrays.json
ok: upstream/json/n_object_trailing_comma.json
ok: upstream/json/n_string_incomplete_escape.json
ok: upstream/json/n_string_invalid-utf-8-in-escape.json
ok: upstream/json/n_string_unescaped_ctrl_char.json
ok: upstream/json/y_number_int_with_exp.json
ok: upstream/json/y_string_escaped_control_character.json
ok: upstream/json/y_string_utf8.json
ok: upstream/bip340/test-vectors.csv
ok: upstream/nip44/nip44.vectors.json
ok: upstream/rfc/rfc8439.txt
ok: upstream/nist/SHA256ShortMsg.rsp
ok: upstream/wycheproof/ecdh_secp256k1_test.json
14 of 14 records pass, 9 with their bytes
vector integrity: digest mismatch: upstream/json/n_object_trailing_comma.json
vector integrity: byte count mismatch: upstream/json/n_object_trailing_comma.json
vector integrity: unpinned revision: upstream/bip340/test-vectors.csv
vector integrity: RFC revision: upstream/rfc/rfc8439.txt
vector integrity: unpinned revision: upstream/nist/SHA256ShortMsg.rsp
vector integrity: archive digest: upstream/nist/SHA256ShortMsg.rsp
vector integrity: invalid digest: upstream/nip44/nip44.vectors.json
vector integrity: invalid path: upstream/../manifest.json
vector integrity: invalid corpus path: nip44.vectors.json
vector integrity: invalid retrieved: upstream/nip44/nip44.vectors.json
```

The refusals:

- `{"id":1,}` has the right length and the wrong digest.
- The right bytes with `bytes = 10` in the record pass the digest and fail
  the count.
- A URL with `master` in place of the commit fails `unpinned revision`.
- An RFC 8439 record pointing at `rfc7539.txt` fails `RFC revision`.
- The NIST record with no archive digest fails `unpinned revision`; with a
  32-digit archive digest it fails `archive digest`.
- A digest in capitals, a path through `..`, a path outside `upstream/`, and
  a date written `28 Oct 2026` each fail their spelling rule.

`check_vectors.py` gives the same diagnostic for each of these edits made to
a copy of the real corpus, prefixed by `vector integrity: ` as the Coax
example prints it. This was run on 2026-11-01 with a scratch script that
copies the corpus to a temporary directory, makes each edit, and calls
`audit`.

The book's checks run this program in debug and release builds and compare
the output with
[nostr/fixtures/vector-provenance.out](../fixtures/vector-provenance.out).

## How the code is checked

### The offline audit

```shell
python3 coax/tests/vectors/check_vectors.py
cd coax/tests/vectors && python3 test_integrity.py
```

On 2026-11-01 the audit printed
`{"bytes": 1344183, "derived_files": 1, "network": false, "upstream_files": 31}`
and `test_integrity.py` ran its 10 tests in 0.12 seconds, all passing. The
tests copy the corpus to a temporary directory and expect a refusal for a
flipped byte, a missing file, each missing metadata field, a branch URL, a
path through `..`, a duplicate path, a symlink, a wrong derived input
digest, an unlisted file and an edited generator.

The audit makes no network request. A test patches `urlopen` to fail and
the audit still passes.

### The network comparison

```shell
python3 coax/tests/vectors/check_vectors.py --fetch
```

`--fetch` downloads every pinned URL, checks the NIST archive's digest
before reading its members, and compares each file's digest with its
record. It never rewrites a committed file. It was not run for this
chapter; the corpus has not changed since its pins were reviewed on
2026-10-28.

### Checks in the example

The checks are in
[nostr/examples/checks/vector-provenance.coax](../examples/checks/vector-provenance.coax),
which loads the example. They compare the example with values from other
tools:

- the digests and lengths from GNU coreutils 9.11 `sha256sum` and `wc -c`
  over the committed files, for the nine embedded files and the five larger
  records;
- the FIPS 180-2 digest of `abc`, through `check_bytes`;
- the padded block worked by hand, and the 55-byte and 56-byte edges;
- 108 one-bit changes across the nine files and nine one-byte truncations,
  each of which must fail its digest;
- 13 tampered fields, each with its expected refusal, and the edges of the
  revision rules.

<!-- source: nostr/examples/checks/vector-provenance.coax check -->
```jai
// GNU coreutils 9.11 `sha256sum` and `wc -c` over the committed files in
// coax/tests/vectors/upstream/, run 2026-11-01, in the example's PIN order.
// They share no code with the Coax SHA-256 or with check_vectors.py.
SHA256SUM :: [14] string.[
    "6a9c15ecc8fc3da72b0ba5e3539e07f2aad3c704b496fe3496579dc723ce49c5",
    "a6c6b45361ff77e7372a015a1f0289e9c09d1fe1ed59cf9773d599c55acf57cf",
    "60e04bc71fcc4c57abeef95a9604a96aa60f833881a2f028d27bd4f03e176603",
    "df3709e1b19bc2562a9bc0301265c237ca5a5b35bd9bc5c3a7f45c57092e0537",
    "1c1ef70149258ff7ee082817269431c45e34f75a2268c3dff56f35d3a4859f1e",
    "c85db33857913bc4c4d4f044c72cd06d50ffb5644b2e5093ef8b46ebfe340ed0",
    "6d014f6467daa9753d57f0d75067fe62c9f3b0116455a3222c26e58432541b0d",
    "1a9f37085203c6d712945c5fb87ad5e08fa38515e51ee4a637c7558bb1b82725",
    "adec79032c3509dce21c9b0b96445a0fe398533d6647d12abc639eef16e621ab",
    "34c9d1d9c3a88d524bc80778540dc43f8306ec249a7485293063c376db851c2d",
    "269ed0f69e4c192512cc779e78c555090cebc7c785b609e338a62afc3ce25040",
    "25bef70fbf7a07ff45c2fe4cb7c6ce954eac687413d8610603268b4e4415324c",
    "75e1cb83994638481808e225b9eb0c1ebd0c232d952ac42b61abce6363be283c",
    "049fa2be70e9db836a043aba6132b53cd97de4c0de5277bca6d65a2770075cae"];
WC_C :: [14] int.[12, 1000, 9, 5, 7, 7, 6, 10, 11, 6892, 37630, 88847, 10299, 501501];
hex32 :: (text: string) -> [32] u8 {
  b: [32] u8;
  #assert hex_decode_checked(text, b[..], false).error == 0;
  return b;
}
// The nine embedded files hash to sha256sum's digests and pass the audit.
#forall "the embedded files match sha256sum" i: 0..<FILES {
  buf: [1000] u8; digest: [32] u8;
  data := file_bytes(i, buf[..]);
  #assert data.count == WC_C[i];
  #assert sha256_hash_checked(data, digest[..], false) == SHA_OK;
  want := hex32(SHA256SUM[i]);
  #assert cast(string) digest[..] == cast(string) want[..];
  #assert audit(pin(i), data, true) == OK;
}
// The five larger records carry sha256sum's digest and byte count.
#forall "the larger records match sha256sum" i: FILES..<PINS {
  #assert PIN_SHA256[i] == SHA256SUM[i] && PIN_BYTES[i] == WC_C[i];
  none: [1] u8;
  #assert audit(pin(i), none[0..<0], false) == OK;
}
// FIPS 180-2 appendix B.1: SHA-256("abc"). A record for it passes.
#test "a record for abc passes" {
  p := pin(2);
  p.sha256 = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
  p.bytes = 3;
  abc: [3] u8; abc[0] = 'a'; abc[1] = 'b'; abc[2] = 'c';
  #assert check_bytes(p, abc[..]) == OK;
  abc[2] = 'd';
  #assert check_bytes(p, abc[..]) == 5;
  abc[2] = 'c'; p.bytes = 4;
  #assert check_bytes(p, abc[..]) == 6;
}
// The padded block worked by hand: 9 bytes, 0x80, 46 zero bytes, then the
// bit length 72 = 0x48 in 8 big-endian bytes.
#test "the hand-padded block" {
  buf: [1000] u8; text: [128] u8;
  b := pad_block(file_bytes(2, buf[..]));
  #assert b.ok;
  #assert hex_encode_checked(cast(string) b.bytes[..], text[..], false).error == 0;
  hand := "7b226964223a302c7d80";
  #assert cast(string) text[0..<20] == hand;
  for i: 20..<112 { #assert text[i] == '0'; }
  #assert cast(string) text[112..<128] == "0000000000000048";
}
// The largest one-block message, 55 bytes = 440 bits = 0x01b8, and the
// smallest two-block one, 56 bytes, which pad_block refuses.
#test "one block holds at most 55 bytes" {
  buf: [56] u8;
  for i: 0..<56 { buf[i] = 'a'; }
  b := pad_block(buf[0..<55]);
  #assert b.ok && b.bytes[54] == 'a' && b.bytes[55] == 0x80;
  #assert b.bytes[61] == 0 && b.bytes[62] == 0x01 && b.bytes[63] == 0xb8;
  #assert !pad_block(buf[..]).ok;
}
// Every one-byte change of every embedded file fails its digest. Bit k of
// byte k (mod the length) flips; the 1,000-byte file is sampled at 0..11.
#forall "one changed byte fails the digest" i: 0..<FILES, k: 0..<12 {
  buf: [1000] u8;
  data := file_bytes(i, buf[..]);
  data[k % data.count] ^= cast(u8) (1 << (k % 8));
  #assert audit(pin(i), data, true) == 5;
}
// Dropping the last byte fails the digest first, as in check_vectors.py.
#forall "a short file fails the digest" i: 0..<FILES {
  buf: [1000] u8;
  data := file_bytes(i, buf[..]);
  #assert audit(pin(i), data[0..<data.count - 1], true) == 5;
}
// Each refusal, one changed field at a time, on the NIP-44 corpus record.
BAD :: 13;
BAD_FIELD :: [13] int.[0, 0, 0, 1, 1, 1, 2, 3, 3, 4, 4, 4, 5];
BAD_VALUE :: [13] string.[
    "nip44.vectors.json", "upstream/../manifest.json", "upstream//x.json",
    "2026-10-2", "2026/10/28", "2026-1a-28",
    "269ed0f69e4c192512cc779e78c555090cebc7c785b609e338a62afc3ce2504",
    "http://raw.githubusercontent.com/paulmillr/nip44/671a1f04bcfacaf125b0db68adc45bc9ce0e763b/javascript/test/nip44.vectors.json",
    "https://raw.githubusercontent.com/paulmillr/nip44/main/javascript/test/nip44.vectors.json",
    "git:671a1f04", "git:671A1F04BCFACAF125B0DB68ADC45BC9CE0E763B", "RFC 44",
    "upstream/./nip44.vectors.json"];
BAD_CODE :: [13] int.[1, 4, 4, 2, 2, 2, 3, 7, 8, 8, 8, 9, 4];
#forall "each tampered field gives its refusal" i: 0..<BAD {
  p := pin(10);
  f := BAD_FIELD[i];
  if f == 0 || f == 5 { p.path = BAD_VALUE[i]; }
  if f == 1 { p.retrieved = BAD_VALUE[i]; }
  if f == 2 { p.sha256 = BAD_VALUE[i]; }
  if f == 3 { p.url = BAD_VALUE[i]; }
  if f == 4 { p.revision = BAD_VALUE[i]; }
  none: [1] u8;
  #assert audit(p, none[0..<0], false) == BAD_CODE[i];
}
// The RFC rule: right number and URL pass; wrong digits, suffix or prefix fail.
#test "the RFC revision names its URL" {
  p := pin(11);
  #assert check_source(p) == OK;
  p.revision = "RFC 8439x"; #assert check_source(p) == 9;
  p.revision = "RFC "; #assert check_source(p) == 9;
  p.revision = "RFC 8438"; #assert check_source(p) == 9;
  p.revision = "RFC 8439"; p.url = "https://www.rfc-editor.org/rfc/rfc8439.html"; #assert check_source(p) == 9;
  p.url = "https://www.rfc-editor.org/rfc/rfc8439.txt.gz"; #assert check_source(p) == 9;
  p.url = "https://www.ietf.org/rfc/rfc8439.txt"; #assert check_source(p) == 9;
  p.url = "https://www.rfc-editor.org/rfc/rfc8439.xtx"; #assert check_source(p) == 9;
  p.url = "https://www.rfc-editor.net/rfc/rfc8439.txt"; #assert check_source(p) == 9;
  p.url = "https://www.rfc-editor.org/rfc/rfc9439.txt"; #assert check_source(p) == 9;
  p.url = "https://www.rfc-editor.org/rfc/rfc7.txt"; p.revision = "RFC 7"; #assert check_source(p) == OK;
  p.revision = "RFC x"; #assert check_source(p) == 9;
}
// The commit must sit between slashes in the URL, at any position.
#test "the commit is a whole URL segment" {
  p := pin(9);
  p.url = "https://example.org/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/x.csv";
  #assert check_source(p) == OK;
  p.url = "https://example.org/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793x/x.csv";
  #assert check_source(p) == 8;
  p.url = "https://example.org/x5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/x.csv";
  #assert check_source(p) == 8;
  p.url = "https://e/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/";
  #assert check_source(p) == OK;
  p.url = "https://e/5aadddb511a5dcfe4599531bd8ae5e5b96d0f79/";
  #assert check_source(p) == 8;
  p.url = "https://e/5aadddb511a5dcfe4599531bd8ae5e5b96d0f79x/";
  #assert check_source(p) == 8;
  // The commit as the host, right after "https://", as Python's "in" finds it.
  p.url = "https://5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/x";
  #assert check_source(p) == OK;
}
// With bytes, a right digest still goes on to the source rules.
#test "matching bytes do not skip the revision" {
  buf: [1000] u8;
  p := pin(2);
  p.url = "https://raw.githubusercontent.com/nst/JSONTestSuite/master/test_parsing/n_object_trailing_comma.json";
  #assert audit(p, file_bytes(2, buf[..]), true) == 8;
}
// An archive record needs a 64-digit lowercase archive digest.
#test "the archive digest is required and spelled" {
  p := pin(12);
  #assert check_source(p) == OK;
  p.archive = ""; #assert check_source(p) == 8;
  p.archive = "929ef80b"; #assert check_source(p) == 10;
  p.archive = "929EF80B7B3418ACA026643F6F248815913B60E01741A44BBA9E118067F4C9B8"; #assert check_source(p) == 10;
}
// The helpers at their edges.
#test "text helpers" {
  #assert starts("upstream/x", "upstream/") && !starts("upstrea", "upstream/");
  #assert match("xabc", 1, "abc", 0) && !match("xabc", 2, "abc", 0) && !match("abc", -1, "a", 0);
  #assert match("abc", 1, "bc", 0) && !match("abc", 2, "cd", 0) && !match("xbc", 0, "abc", 0);
  #assert match("bc", 0, "abc", 1) && !match("bc", 0, "abc", 0);
  #assert digit('0') && digit('9') && !digit('/') && !digit(':');
  #assert lower_hex_digit('a') && lower_hex_digit('f') && !lower_hex_digit('g') && !lower_hex_digit('`');
  #assert lower_hex("x09af", 1, 4) && !lower_hex("x09ag", 1, 4) && !lower_hex("x09a:", 1, 4);
  #assert digits_from("RFC 0", 4) && !digits_from("RFC ", 4) && !digits_from("RFC x1", 4);
  #assert date_shape("2026-10-28") && !date_shape("2026-10-28 ") && !date_shape("2026-10:28");
  #assert date_shape("2099-09-29") && !date_shape("x026-10-28") && !date_shape("2026-10-2:");
  #assert segments_named("a/b") && !segments_named("a/") && !segments_named("/a");
  #assert !segments_named("a/../b") && !segments_named("..") && segments_named("a/.b") && segments_named("a/..b");
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/vector-provenance.coax
coax/bin/coax mutate nostr/examples/checks/vector-provenance.coax --root-only
```

`coax test` runs 55 checks, including Bitcoin chapter 4's and Bitcoin chapter 1's, in about
a second. Root-only mutation kills 3 of 3 mutants in the checks file. The
example's own procedures were mutated by running the same command on a
temporary file that joined the example and its checks: 222 mutants, 222
killed, on 2026-11-01. Only `say` and `show`, which print, are not reached
by a check.

## Summary

- Each vector file has one record: source URL, immutable revision, license,
  retrieval date, byte count and SHA-256.
- A Git revision must appear as `/<commit>/` in the URL; an RFC must use its
  numbered RFC Editor URL; an archive member must pin the archive's digest.
- Upstream bytes are kept exactly. A derived file names its generator and
  every input by digest, and the audit reproduces it.
- The Coax example applies the same rules and gives the same diagnostics as
  `check_vectors.py`. One changed byte in a vector file fails with
  `digest mismatch` and the file's path.

## Dependencies

- [4. SHA-256](../../bitcoin/book/ch004-sha-256.md)
- [Bitcoin chapter 23. Verify a Schnorr signature](../../bitcoin/book/ch023-verify-a-schnorr-signature.md)
- [20. NIP-44 encrypted payloads](ch020-nip-44-encrypted-payloads.md)

## Sources

- The inventory, licenses and reader rules:
  [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md) and
  [manifest.json](../../coax/tests/vectors/manifest.json).
- [FIPS 180-4](https://csrc.nist.gov/pubs/fips/180-4/upd1/final), the
  SHA-256 standard, and the
  [NIST CAVP SHA test vectors](https://csrc.nist.gov/projects/cryptographic-algorithm-validation-program/secure-hashing).
- [JSONTestSuite](https://github.com/nst/JSONTestSuite) at commit
  `1ef36fa01286573e846ac449e8683f8833c5b26a`.
- [NIP-44](../nips/official/44.md), which pins its vector file's digest.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
