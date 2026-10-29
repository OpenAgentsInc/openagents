# Unicode NFKC

`nfkc.coax` implements Unicode **18.0.0 NFKC**, following UAX #15 revision 58.
It uses the full pinned Unicode data, recursive compatibility/canonical
decompositions, canonical combining classes, full composition exclusions and
algorithmic Hangul. There is no host normalization API or runtime data download.
`nfkc-data.coax` is committed generated data, so normal builds remain Python-free.

```coax
#load "nfkc.coax";
scratch:[18432]u32;
out:[73728]u8;
result:=un_nfkc("Åﬃ",scratch[..],out[..],false);
// Check result.error before reading out[0..<result.written].
```

The input `string` is a counted byte view, including possible embedded NULs;
the normalizer validates strict UTF-8 itself. It accepts valid scalar values,
including noncharacters and codepoints unassigned in the pinned version.
Surrogates, overlong encodings, invalid continuation bytes, truncation and
values above U+10FFFF refuse. This is NFKC, without case folding, stripping or
stream-safe character insertion. A consumer owns any further domain policy.

| Argument/result | Initial bounded profile |
| --- | --- |
| Input | 0–1,024 bytes |
| Caller scratch | 0–18,432 `u32` codepoints; 73,728 bytes at full capacity |
| Caller destination | 0–73,728 bytes |
| Success | `UN_OK`, exact written byte count; unused destination unchanged |
| Refusal | `written=0`, destination unchanged; scratch may contain partial data |

Errors are `UN_UTF8=1`, `UN_LIMIT=2` for invalid view counts, `UN_SPACE=3` for
insufficient scratch/destination, and `UN_ALIAS=4` for a reported overlap.
The caller must retain stable valid input and writable storage, truthful view
extents, aligned scratch, exclusive access, and disjoint input/scratch/output
for the whole synchronous call. The overlap flag is the host's observation,
not pointer authentication. Only `un_nfkc` is exported; helpers assume its
validated internal values and the generated table invariants.

The workspace ceiling follows the largest recursive decomposition of eighteen
codepoints per input scalar, with at most four UTF-8 bytes per result scalar.
The profile is a finite storage/work bound, not a latency guarantee. Canonical
ordering uses insertion sort and can take quadratic work. Current Coax source
also copies the 1,002/5,982/961-entry lookup-key arrays into locals because
mutable views cannot borrow constants. The tested native `-O2` backend optimized
those copies away (largest reported individual frame: 96 bytes); `-O0` reported
a 24,192-byte frame. Those are compiler observations, not portable total-stack
bounds. The actual Wasm tests reserve a 1 MiB stack.

The measured 1,023-byte reverse combining-class case took about 2 ms in the
named native run and 2.4–2.9 ms in Node/V8 Wasm. These are single warm observations
on the development Mac, not worst-case, device or real-time budgets. Data-dependent
lookup/order work and secret cleanup remain separate security considerations.

For NIP-49, consumers must admit the **normalized** password length against
their KDF profile and own cleanup of input, scratch and output. The normalizer
does not integrate the scrypt wrapper, approve production secrets, or guarantee
erasure. A Unicode-version upgrade is explicit; older hosts can differ for
characters they considered unassigned, so callers must not silently substitute
an installed host version.

Explicit tooling and tests:

```sh
python3 tools/coax-unicode/tables.py --check
python3 coax/tests/unicode/test_tables.py
coax/bin/coax test coax/lib/text/nfkc.coax
coax/bin/coax mutate coax/lib/text/nfkc.coax
python3 coax/tests/unicode/conformance.py
```

The full original corpus and original payload/archive hashes are in
`coax/tests/unicode/upstream/unicode-18.0.0/`. The generator parses that data
without using Python's installed Unicode normalization. Evidence and review:
[`docs/verification/2026-10-29-unicode-nfkc.md`](../../../docs/verification/2026-10-29-unicode-nfkc.md).
