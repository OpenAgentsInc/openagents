# Crypto byte helpers

Queue task `crypto-bytes` (`#993f`), 2026-10-28.

[bytes.coax](../../lib/crypto/bytes.coax) owns bounded endian load/store,
full-scan equality, masked selection and copy policy. It is `#authority`,
with checks written before implementation and zero surviving mutants.
[bytes-memory.coax](../../lib/crypto/bytes-memory.coax) contains only two runtime
address-observation wrappers. It follows the proto runtime's `memory.coax`
boundary: observed overlap is metadata, and the pure core decides whether an
operation is allowed. No pointer arithmetic or allocation enters the pure core.

The exported API is available from Coax or its generated C ABI:

| Procedure | Contract |
| --- | --- |
| `cb_span(total, at, n)` | Nonnegative span, at most 1 MiB of borrowed storage; subtraction checks avoid integer overflow. |
| `cb_load_be32/le32/be64/le64(data, at)` | Returns `{value, ok}`; failure returns zero and does not read outside the admitted span. |
| `cb_store_be32/le32/be64/le64(out, at, value)` | Typed 32/64-bit input; invalid spans refuse before writing; adjacent bytes remain unchanged. |
| `cb_mask(bit)` | Low bit selects zero or `UINT64_MAX`; `(bit & 1) * UINT64_MAX` is defined even in checked mode. |
| `cb_ct_equal(a, b)` | Equal admitted bytes return `UINT64_MAX`, unequal or invalid lengths return zero; empty views compare equal. It scans the complete public length. |
| `cb_ct_select32/64(mask, a, b)` | Each mask bit chooses the corresponding bit of `a` or `b`; zero/all-ones selects the whole word. |
| `cb_copy(source, out)` | Copies the full source into sufficient output capacity; refuses overlapping borrowed spans, including exact alias. |
| `cb_ct_select_bytes(mask, a, b, out)` | Equal input lengths and sufficient output capacity; either overlapping input/output span refuses before any mutation. |

`cb_copy` and `cb_ct_select_bytes` come from the runtime wrapper file. Their
unexported `_checked` helpers accept already observed overlap facts for pure
checks; application callers must use the wrappers rather than invent those
facts. Overlap is conservatively measured over whole borrowed views, including
unused output capacity. Disjoint subviews of one allocation work. Empty views
perform no accesses. These are borrowed views: the native host must lend valid
backing storage and truthful counts; this is not a pointer validator or an
ownership/lifetime system. Keep aliases stable for the duration of each call.

The 1 MiB span bound is a local operation budget, not a Nostr event limit. Larger
streams use bounded chunks. No Nostr knowledge or profile import exists in the
crypto library. Endian code assembles bytes explicitly and never reinterprets
storage. A 32-bit store's private common helper refuses an oversized `u64`
carrier before mutation; the public ABI accepts `u32` directly.

Equality accumulates byte XOR differences with OR. The accumulator is at most
255, so `(diff + 255) >> 8` produces the nonzero bit without unsigned underflow
or a content comparison. A register value barrier obscures masks used by the
selection code. It neither wipes storage nor proves timing behavior. Secure
zeroization is a separate `crypto-secure-zero` task; this slice implements no
`zeroize` shortcut. Compiler wrapping intrinsics are not required.

## Evidence and scope

```sh
coax/bin/coax test coax/lib/crypto/bytes.coax
coax/bin/coax mutate coax/lib/crypto/bytes.coax
python3 coax/tests/nostr/bytes.py
```

- **10 Coax checks** include 1,859 span combinations, 60 endian capacity cases,
  16 checked-safe mask cases, 1,024 byte-value/position comparisons and 48
  selection/copy capacity and alias combinations.
- **137 mutants killed; zero survivors.** Initial surviving cases led to exact
  `UINT32_MAX`, one-byte alias and equality-shape cases. Redundant length checks
  were simplified using equal input counts; no equivalent-mutation waiver was
  added.
- **Seven focused native tests** run checked and wrapping emitted C at `-O2`.
  Python's independent `int.to_bytes` and bit operations check fixed/seeded
  random 32/64-bit values, masks and output sentinels. Boundary cases include
  negative/zero views, `INT64_MIN/MAX`, wrong lengths, insufficient capacity,
  partial/exact alias and disjoint subviews. Either input overlapping selection
  output refuses without mutation.
- A standalone **ASan/UBSan** native harness exercises capacity, malformed view
  counts and alias boundaries. A separately altered emitted selection procedure
  returns the wrong input; the independent selection oracle detects it.
- **Native disassembly** of `cb_mask`, `cb_ct_select32` and `cb_ct_select64`
  contains no conditional branch in checked or wrapping `-O2` builds on the
  tested Mac. This gate covers these word procedures only. It does not prove
  the complete equality/selection pipeline, other architectures, Wasm JITs or
  secret-data leakage resistance. Public span/length/alias branches are allowed.

Verification used Apple clang 21.0.0, arm64, and the existing `coax0 0.13.0`
compiler binary SHA-256
`1871b8030ecfcea59c75d573d4d6a8faab4cb46a28e4cbd8d71b492b59ba402a`.
No compiler bootstrap, app, iOS, remote-commit or unrelated crypto suites were
rerun. Cross-target vector, leakage-probe and independent security-review gates
remain in the Nostr plan. A helper check/mutation score does not authorize
production signing or advertise a complete Nostr support profile.
