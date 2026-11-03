# 119. Fuzzing

Every parser in this book reads bytes that someone else chose: a peer's
P2P message, a transaction in a block, a PSBT from another wallet, an
invoice pasted into a chat. The published vectors check the cases their
authors thought of. A fuzzer checks the rest by sending a parser inputs
that are almost right, many thousands of them, and asking two questions
of each answer: did the parser return at all, and was its answer right?

This chapter builds a small deterministic fuzzer for six parsers:
transactions (chapter 34), P2P message headers and version payloads
(chapter 65), PSBTs (chapter 94), BOLT 11 invoices (chapter 114) and
Base58Check (chapter 26). It seeds them from the pinned vectors, mutates
them, and compares every answer with an independent Python reference. It
reports what it found, and keeps each finding as a regression input.
The repository's existing fuzz tooling,
[tools/coax-nostr/fuzz/](../../tools/coax-nostr/fuzz/README.md), is a
libsecp256k1 oracle for the curve arithmetic of chapters 13 to 24. It
has no parser targets, so this chapter builds its own.

## What this chapter teaches

- What a fuzzer checks: no crash, no accepted input that is not what it
  claims to be, and the same answer as a reference.
- How to make fuzzing reproducible: inputs that are a pure function of a
  seed and an index, and a fixed budget.
- Why blind byte mutation stops at checksums, and how mutating the
  structure and then repairing the checksum gets past them.
- How to turn what a fuzzer finds into a regression test, and how to tell
  a bug in the parser from a bug in the reference or in the harness.

## Where the code is

| File | What it does |
| --- | --- |
| [fuzz_targets.coax](../../coax/tests/bitcoin/fuzz_targets.coax) | one exported entry for each parser: read the input, write the result back where the format has one encoding, return the decision |
| [fuzz.py](../../coax/tests/bitcoin/fuzz.py) | the fuzzer: builds the targets in checked and release builds, seeds, mutates, compares with the references, narrows a crash to one input |
| [fuzz_regressions.json](../../coax/tests/bitcoin/fuzz_regressions.json) | the inputs the fuzzer found, replayed first on every run |
| [make_fuzz_seeds.py](../../coax/tests/bitcoin/make_fuzz_seeds.py) | writes [fuzz_seeds.coax](../../coax/tests/bitcoin/fuzz_seeds.coax), the example's seeds |
| [fuzzing.coax](../examples/fuzzing.coax) | the example: a fixed budget of mutations run in Coax |

## What a fuzzer checks

A fuzzer needs an oracle: a way to decide whether an answer is wrong.
This chapter uses three.

1. **The parser returns.** Coax traps on an index or slice outside its
   bounds in every build, and on integer overflow in the checked build. A
   trap stops the process, so a parser that reads past the end of its
   input does not return a wrong answer quietly; it crashes, and the
   fuzzer sees the crash.
2. **An accepted input writes back to itself.** A transaction, a P2P
   header, a version payload and a Base58Check string each have exactly
   one encoding for each value. If the parser accepts some bytes and
   writing the result back gives other bytes, the parser accepted
   something malformed, such as a length with a redundant leading zero.
   `fuzz_targets.coax` returns 999 in that case. PSBTs and invoices are
   left out of this check: Core writes a PSBT's fields in its own order,
   and an invoice cannot be written back without the payee's key.
3. **The answer equals the reference's.** Each parser has a Python
   reference that shares no code with Coax:

   | Parser | Reference |
   | --- | --- |
   | Transactions | [tx_reference.py](../../coax/tests/bitcoin/tx_reference.py), Core's `UnserializeTransaction` and `DecodeTx` |
   | P2P headers and version payloads | [p2p.py](../../coax/tests/bitcoin/p2p.py), Core's header checks and `ProcessMessage`'s version reads |
   | PSBTs | [psbt_reference.py](../../coax/tests/bitcoin/psbt_reference.py), Core's `DecodeRawPSBT` |
   | BOLT 11 invoices | [bolt11.py](../../coax/tests/lightning/bolt11.py), an independent reader |
   | Base58Check | a reader in `fuzz.py`, written from Core's `base58.cpp` |

   The comparison is on the exact refusal code, not only accept or
   refuse, so a parser that refuses for the wrong reason is found too.
   Results that mean "more than the harness's arrays hold" are not
   compared, since the references have no such limit.

## Seeds and mutations

The corpus is the pinned vectors: 142 transactions from Core's
`tx_valid.json` and `tx_invalid.json`, 153 PSBTs from BIP-174, BIP-370,
BIP-371 and Core's `rpc_psbt.json`, BOLT 11's 31 example invoices, 59
Base58Check strings and Base58 values from Core's `key_io_valid.json` and
`base58_encode_decode.json`, and P2P messages written with Core's pinned
test framework, `messages.py`. Each seed file's SHA-256 is checked against
its pin before it is read (chapter 116).

Each input stacks one to eight mutations on a seed: flip a bit, set a
byte to a boundary value (`0x00`, `0x7f`, `0xfd`, `0xff` and others),
insert or delete bytes, cut the input short, repeat a run of its bytes,
splice in part of another seed, set a 4-byte field to 0, 1, `2^32 - 1` or
a value next to its own, or insert a CompactSize prefix of each width.

Blind mutations rarely get past a checksum. A P2P header carries the
first 4 bytes of the payload's SHA-256d, and an invoice ends in a Bech32
checksum, so almost every changed input is refused at the first check and
the code behind it is never reached. `fuzz.py` mutates these two formats
by their structure. For a P2P message it changes the bytes, then usually
writes the length and checksum to match the new payload. For an invoice
it decodes the Bech32 words, changes the words or the human-readable
part, and computes the checksum again, so the input reaches the tagged
fields and the signature.

Every input is a pure function of three values: the target, the seed and
the index. `random.Random('tx:119:4071')` makes input 4,071 of seed 119,
whatever ran before it. A failure is reported as those three values, and
anyone can make the same input again.

## Crashes

`fuzz.py` compiles `fuzz_targets.coax` to C twice, a checked build and a
release build, and loads each as a shared library. The inputs run in a
child process, a thousand at a time. If the child dies, the fuzzer runs
each half of that range again, and so on, until one index is left: the
crashing input. Each run first replays every input in
`fuzz_regressions.json` in both builds and checks it against the
reference.

```shell
python3 -I coax/tests/bitcoin/fuzz.py
python3 -I coax/tests/bitcoin/fuzz.py --budget 100000 --chunk 10000
```

## What the fuzzer found

On 2026-11-03 `fuzz.py --budget 100000` sent 100,000 inputs to each of
the seven targets (`tx` reads with witnesses allowed; `tx_decode` is
`decoderawtransaction`'s two readings) in each build, 1,400,000 in all,
on an Apple arm64 laptop in about 7 minutes. Every one returned, every
accepted input wrote back, and every answer equaled the reference's. The
checked and release builds gave the same counts of each result:

| Parser | Seeds | Accepted | Refused | Crashes | Disagreements |
| --- | ---: | ---: | ---: | ---: | ---: |
| `tx` | 142 | 6,133 | 93,867 | 0 | 0 |
| `tx_decode` | 142 | 6,191 | 93,809 | 0 | 0 |
| `p2p_frame` | 10 | 26,040 | 73,960 | 0 | 0 |
| `p2p_version` | 14 | 53,933 | 46,067 | 0 | 0 |
| `psbt` | 153 | 1,677 | 98,323 | 0 | 0 |
| `invoice` | 31 | 10,472 | 89,528 | 0 | 0 |
| `base58check` | 59 | 905 | 99,095 | 0 | 0 |

The invoice target is the slowest by far, about 155 seconds a build,
because the reference recovers a public key from every signature in
Python. Mutation testing: not run (owner speed order).

The runs before that one found four problems. None was a crash, and none
was a bug in a Coax parser.

- **The P2P reference accepted a length cut short.** A version payload
  whose user agent length is `0xff` followed by fewer than 8 bytes was
  refused by Coax and accepted by `p2p.py`. Core's `ReadCompactSize`
  throws when the stream ends; the test framework's `deser_compact_size`,
  which `p2p.py` used, reads whatever bytes remain and returns 0. The
  reference was wrong. `p2p.py` now reads the length with
  `tx_reference.py`'s transliteration of `ReadCompactSize`, which also
  refuses non-canonical and oversized lengths, and applies
  `MAX_SUBVERSION_LENGTH`, 256 bytes, as Core's `LIMITED_STRING` does.
  Chapter 65's cross-check still passes, since none of its cases reached
  these paths.
- **The relay flag has 255 encodings of true.** The harness reported
  version payloads that Coax accepted but did not write back to the same
  bytes. Each ended in a relay flag such as `0xff`. Core reads the flag
  as a `bool`, so any nonzero byte is true, and writes it back as `0x01`.
  The parser was right and the harness's oracle was too strict: the
  version payload does not have one encoding. The harness now compares
  that byte as a `bool`.
- **The harness crashed, not the parser.** The first 100,000-input run
  reported 33 crashes in the invoice and Base58Check targets. Each was a
  Python error in `fuzz.py`'s own text mutator: it swapped the case of a
  non-ASCII byte, and `chr(0xff).swapcase()` is not one byte. The worker
  process died, and the fuzzer counted that as a crash. The mutator now
  swaps ASCII letters only, and a worker that exits with an error, rather
  than a signal, now stops the run as a harness failure instead of being
  reported as a crash in Coax.
- **The seeds were not fixed.** Early runs gave different results in
  the checked and release builds for the same indexes. The test
  framework's `msg_version` draws a random nonce when it is built, so the
  version seeds changed between runs. The seeds now set the nonce.

The inputs behind the first two are kept in
[fuzz_regressions.json](../../coax/tests/bitcoin/fuzz_regressions.json),
with three more that pin down the fix: a non-canonical user agent length,
and user agents of 257 and 256 bytes.

## The example

The example runs the same targets inside one Coax program, with a fixed
budget of 1,000 inputs for each parser, mutated from the seeds in
[fuzz_seeds.coax](../../coax/tests/bitcoin/fuzz_seeds.coax) by a seeded
xorshift64 generator. Its mutations are blind byte changes, so it shows
what blind fuzzing reaches and where it stops.

<!-- source: bitcoin/examples/fuzzing.coax example -->
```jai
#load "../../coax/tests/bitcoin/fuzz_targets.coax";
#load "../../coax/tests/bitcoin/fuzz_seeds.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

FZ_BUDGET :: 1000;   // inputs per parser: the run's fixed budget
FZ_ROOM :: 4096;
FZ_INTERESTING :: [8] u8.[0x00, 0x01, 0x7f, 0x80, 0xfc, 0xfd, 0xfe, 0xff];

// xorshift64: a small, seeded generator, so every run makes the same
// inputs. s[0] is the state; it must not be zero.
fz_next :: (s: [] u64) -> u64 {
  x := s[0];
  x ^= x << 13; x ^= x >> 7; x ^= x << 17;
  s[0] = x;
  return x;
}
fz_below :: (s: [] u64, n: int) -> int { return cast(int) (fz_next(s) % cast(u64) n); }

// One to four mutations of seed, written into out. Returns the length.
// Each one flips a bit, sets a byte to a boundary value, inserts a random
// byte, deletes up to 8 bytes, cuts the input short, or repeats up to 16
// of its bytes.
fz_mutate :: (s: [] u64, seed: [] u8, out: [] u8) -> int {
  n := seed.count;
  for i: 0..<n { out[i] = seed[i]; }
  for k: 0..<1 + fz_below(s, 4) {
    op := fz_below(s, 6); at := fz_below(s, n + 1);
    if op == 0 && n > 0 {
      if at == n { at -= 1; }
      out[at] ^= cast(u8) (1 << fz_below(s, 8));
    } else if op == 1 && n > 0 {
      if at == n { at -= 1; }
      out[at] = FZ_INTERESTING[fz_below(s, 8)];
    } else if op == 2 && n < out.count {
      i := n; while i > at { out[i] = out[i - 1]; i -= 1; }
      out[at] = cast(u8) fz_below(s, 256); n += 1;
    } else if op == 3 {
      d := 1 + fz_below(s, 8);
      if d > n - at { d = n - at; }
      for i: at..<n - d { out[i] = out[i + d]; }
      n -= d;
    } else if op == 4 {
      n = at;
    } else if op == 5 {
      d := 1 + fz_below(s, 16);
      if d > n - at { d = n - at; }
      if n + d > out.count { d = out.count - n; }
      i := n + d - 1; while i >= at + d { out[i] = out[i - d]; i -= 1; }
      n += d;
    }
  }
  return n;
}

// The parser numbered k in FZ_TARGETS.
fz_run :: (k: int, b: [] u8) -> int {
  if k == 0 { return fuzz_tx(b); }
  if k == 1 { return fuzz_p2p_frame(b); }
  if k == 2 { return fuzz_p2p_version(b); }
  if k == 3 { return fuzz_psbt(b); }
  if k == 4 { return fuzz_invoice(b); }
  return fuzz_base58check(b);
}

main :: () {
  state := [1] u64.[119];
  seed: [FZ_ROOM] u8; input: [FZ_ROOM] u8;
  print("% inputs per parser, xorshift64 from 119\n", FZ_BUDGET);
  for k: 0..<FZ_TARGETS.count {
    // this parser's seeds, as indexes into FZ_SEEDS
    first := -1; count := 0;
    for i: 0..<FZ_SEEDS.count { if FZ_SEEDS[i].target == k { if first < 0 { first = i; } count += 1; } }
    results: [1000] int;
    for i: 0..<FZ_BUDGET {
      row := FZ_SEEDS[first + fz_below(state[..], count)];
      r := hex_decode_checked(row.hex, seed[..], false);
      #assert r.error == 0;
      n := fz_mutate(state[..], seed[0..<r.written], input[..]);
      results[fz_run(k, input[0..<n])] += 1;
    }
    accepted := results[0] + results[100];
    for p: 101..<105 { accepted += results[p]; }
    print("%: % seeds, % accepted, % refused, % accepted but not written back\n",
          FZ_TARGETS[k], count, accepted, FZ_BUDGET - accepted - results[FUZZ_ROUNDTRIP], results[FUZZ_ROUNDTRIP]);
    print("  results:");
    for code: 0..<1000 { if results[code] > 0 { print(" %:%", code, results[code]); } }
    print("\n");
  }
}
```

```shell
coax/bin/coax run bitcoin/examples/fuzzing.coax
```

```text
1000 inputs per parser, xorshift64 from 119
tx: 6 seeds, 113 accepted, 887 refused, 0 accepted but not written back
  results: 0:113 1:526 2:74 3:25 4:2 5:28 6:232
p2p_frame: 4 seeds, 25 accepted, 975 refused, 0 accepted but not written back
  results: 0:25 1:462 2:135 3:113 4:239 5:26
p2p_version: 4 seeds, 449 accepted, 551 refused, 0 accepted but not written back
  results: 6:551 100:5 101:43 102:6 103:5 104:390
psbt: 6 seeds, 76 accepted, 924 refused, 0 accepted but not written back
  results: 0:76 2:79 3:403 4:10 5:53 6:1 8:7 11:78 12:81 14:14 15:8 16:3 17:4 19:2 20:10 21:2 22:5 23:11 32:2 34:40 35:1 36:1 37:1 38:20 43:2 49:5 50:9 54:5 55:1 57:6 58:6 61:16 62:3 64:6 66:9 67:8 74:3 75:6 76:3
invoice: 4 seeds, 0 accepted, 1000 refused, 0 accepted but not written back
  results: 1:1000
base58check: 6 seeds, 2 accepted, 998 refused, 0 accepted but not written back
  results: 0:2 1:564 2:386 3:48
```

The result codes are each parser's own: for transactions, 1 is "end of
data", 2 a non-canonical CompactSize, 3 a size above `MAX_SIZE`, 4 an
unknown flag, 5 a superfluous witness record and 6 bytes after the
locktime; for P2P headers, 1 is truncated, 2 a wrong magic, 3 oversize, 4
a wrong checksum and 5 an invalid command; for a version payload, 6 is a
bad payload and `100 + n` an accepted payload with `n` optional groups.
PSBT codes are the indexes of Core's refusal messages in
[psbt.coax](../../coax/lib/bitcoin/psbt.coax).

Three things stand out. Every invoice is refused with code 1, a bad
Bech32 checksum: blind mutation never reaches the field reader, which is
why `fuzz.py` mutates invoices by their words. Most changed P2P headers
fail the checksum (code 4) or end early (code 1). The few accepted
invoices and Base58Check strings are inputs whose mutations all fell at
the end of the input and changed nothing.

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/fuzzing.coax](../examples/checks/fuzzing.coax),
which loads the example. Every seed must be accepted; 800 seeded
mutations must stay inside their buffer, change most inputs and never
give an accepted input that does not write back; and the version payloads
the fuzzer kept must give their recorded results:

<!-- source: bitcoin/examples/checks/fuzzing.coax check -->
```jai
// Every seed is accepted, and writes back to itself where its format has
// one encoding.
#forall "every seed is accepted" i: 0..<30 #steps 2000000000 {
  b: [FZ_ROOM] u8;
  row := FZ_SEEDS[i];
  r := hex_decode_checked(row.hex, b[..], false);
  got := fz_run(row.target, b[0..<r.written]);
  #assert got == 0 || (got > 100 && got <= 104);
}
// The mutator stays inside its buffer and changes something most of the
// time, and no mutated input is accepted without writing back.
#forall "mutations stay in bounds and never break a write-back" start: 1..<5 #steps 2000000000 {
  state := [1] u64.[cast(u64) start * 2654435761];
  b: [FZ_ROOM] u8; input: [FZ_ROOM] u8; changed := 0;
  for i: 0..<200 {
    row := FZ_SEEDS[fz_below(state[..], FZ_SEEDS.count)];
    r := hex_decode_checked(row.hex, b[..], false);
    n := fz_mutate(state[..], b[0..<r.written], input[..]);
    #assert n >= 0 && n <= FZ_ROOM;
    if n != r.written || !fuzz_same(input[0..<n], b[0..<n]) { changed += 1; }
    #assert fz_run(row.target, input[0..<n]) != FUZZ_ROUNDTRIP;
  }
  #assert changed > 150;
}
// The version payloads fuzz.py kept (fuzz_regressions.json): a relay flag
// of 0xff, a user agent length cut short, a non-canonical length, and user
// agents one past and exactly at MAX_SUBVERSION_LENGTH.
#test "the version payloads fuzz.py kept give their results" #steps 2000000000 {
  full: [FZ_ROOM] u8; n := 0;
  for row: FZ_SEEDS {
    if row.target == 2 {
      r := hex_decode_checked(row.hex, full[..], false);
      if r.written > n { n = r.written; }
    }
  }
  #assert fuzz_p2p_version(full[0..<n]) == 104;
  full[n - 1] = 0xff;
  #assert fuzz_p2p_version(full[0..<n]) == 104;
  b: [FZ_ROOM] u8;
  for i: 0..<80 { b[i] = full[i]; }
  b[80] = 0xff; b[81] = 0; b[82] = 0;
  #assert fuzz_p2p_version(b[0..<83]) == P2P_BAD_PAYLOAD;
  for i: 81..<87 { b[i] = 0; }
  #assert fuzz_p2p_version(b[0..<87]) == P2P_BAD_PAYLOAD;
  b[80] = 0xfd; b[81] = 5; b[82] = 0;
  for i: 83..<88 { b[i] = 'a'; }
  #assert fuzz_p2p_version(b[0..<88]) == P2P_BAD_PAYLOAD;
  b[81] = 1; b[82] = 1;
  for i: 83..<83 + 257 { b[i] = 'x'; }
  #assert fuzz_p2p_version(b[0..<83 + 257]) == P2P_BAD_PAYLOAD;
  b[81] = 0;
  #assert fuzz_p2p_version(b[0..<83 + 256]) == 102;
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/fuzzing.coax
```

`coax test` runs 546 checks, including those of the files it loads, on
2026-11-03, and all pass. Mutation testing: not run (owner speed order).

### Python

`fuzz.py --example` makes the example's 6,000 inputs in Python, with the
same generator and mutations draw for draw, decides each one with the
references, and compares the transcript with the fixture. With `--run`
it also runs the example in native debug and release builds.

```shell
python3 -I coax/tests/bitcoin/fuzz.py --example --run
```

On 2026-11-03 all 13 lines agreed, and the example's output in native
debug and release builds equaled them. Wasm was not run.

## What this chapter leaves out

- Coverage feedback. libFuzzer and AFL keep an input when it reaches new
  code, and grow the corpus from there. `fuzz.py` has no coverage
  measurement: its corpus is the pinned vectors, and it relies on the
  number of inputs and on structure-aware mutations.
- Bitcoin Core's own fuzz targets and its `qa-assets` corpus. They are
  not pinned here, so they are not used as seeds.
- Sanitizers on the generated C. Coax's own bounds and overflow checks
  stand in for AddressSanitizer and UndefinedBehaviorSanitizer here.
- Scripts, descriptors, addresses, blocks and the other P2P messages.
  Each could get a target in `fuzz_targets.coax` and a reference in
  `fuzz.py` the same way.

## Summary

- A fuzzer is only as good as its oracle. This one has three: the parser
  returns, an accepted input writes back to itself where the format has
  one encoding, and the answer equals an independent reference's, code
  for code.
- Inputs are a pure function of target, seed and index, so every finding
  is reproducible from three values and every run of the example is the
  same.
- Blind mutation stops at checksums; mutating the structure and repairing
  the checksum reaches the code behind them.
- The runs found no crash and no Coax parser bug. They found a reference
  that accepted a length cut short, an oracle that was stricter than
  Core, and seeds that were not fixed. Each is fixed, and the inputs are
  kept as regression tests.

## Dependencies

- [26. Base58Check](ch026-base58check.md)
- [34. Transactions and their serialization](ch034-transactions-and-their-serialization.md)
- [65. P2P messages and checksums](ch065-p2p-messages-and-checksums.md)
- [94. PSBT](ch094-psbt.md)
- [114. Invoices](ch114-invoices.md)
- [116. Test vectors and their provenance](ch116-test-vectors-and-their-provenance.md)

## Sources

- Bitcoin Core at `ddea9196`, pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/tx_valid.json):
  `tx_valid.json`, `tx_invalid.json`, `key_io_valid.json`,
  `base58_encode_decode.json`, `rpc_psbt.json`, `serialize.h`
  (`ReadCompactSize`, `LIMITED_STRING`) and the functional test
  framework's `messages.py`.
- BIP-174, BIP-370 and BIP-371, with their test vectors, pinned in
  [coax/tests/vectors/upstream/bip174/](../../coax/tests/vectors/upstream/bip174/bip-0174.mediawiki).
- BOLT 11 and its `invoice-test.json`, pinned in
  [coax/tests/vectors/upstream/bolts/](../../coax/tests/vectors/upstream/bolts/11-payment-encoding.md).
- G. Marsaglia, "Xorshift RNGs", *Journal of Statistical Software* 8(14),
  2003.
