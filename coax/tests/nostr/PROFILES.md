# Nostr support profiles and budget preflight

Implemented 2026-10-28 for queue task `nostr-support-profiles` (`#f960`).
The canonical data and policy are [profiles.coax](../../lib/nostr/profiles.coax).
The generated [JSON inventory](../../lib/nostr/profiles.json) is an inspectable
maintenance artifact; it is not a new Nostr/protobuf wire protocol.

Eight profiles name the intended real consumers and required/optional NIPs:
`core-read`, `core-write`, `phone-signer`, `openagents-host`, `block-identity`,
`block-channel`, `agent-private`, and optional `key-backup`. Per-target records
separate native C11, iOS arm64 and wasm32. **No complete profile is advertised.**
Existing native wire codecs and JSON shape, and the Wasm socket probe, are partial
components; none proves cryptography, domain admission or production approval.
The live project fold in `2ace224626` does not add event-verification evidence.

Upstream Git pins match `nips/upstream-lock.json`, including per-file SHA-256
checks. The OpenAgents-authored corpus has a local SHA-256 pin over sorted
repository-relative UTF-8 path, NUL, exact file bytes, NUL. Evidence records bind
source paths to commits and source hashes; they are references to bounded checks,
not independent security-review attestations. Runtime permission decisions must
still verify the original source evidence, never a transported support flag.

## Calling the Coax policy

The exported C ABI and Coax procedures provide:

- `np_capabilities(profile, target)` and `np_required(profile)` return independent
  capability masks. `np_can_advertise(profile, target, claim)` accepts only a
  nonempty subset of registry-evidenced capabilities. Passing the required mask
  asks whether the whole profile is supported; that currently returns false.
  Callers cannot create evidence by supplying a Boolean.
- `np_limit(consumer, field)` and `np_bounds(consumer, measured)` perform bounded
  preflight. A measurement view must contain exactly the 12 axes listed in the
  generated inventory. Unknown consumers, oversized values and nonzero values
  on unavailable axes refuse. Zero on an unavailable axis means not applicable.
- `np_configuration(consumer, proposed)` permits narrower, coherent budgets;
  it refuses expansion, enabling an unavailable axis, disabling a required axis,
  content/raw-string maxima above original JSON, or JSON above a frame budget.
- `np_safe_integer(value)` admits only `[-(2^53-1), 2^53-1]`. It intentionally
  narrows the existing generic `json_int`, which also accepts `+/-2^53`.

The host lends truthful, valid pointer/count views. These procedures do not
validate pointer ownership or parse a JSON document. Measurements must come from
bounded parsing; a caller-supplied length is not proof of the underlying bytes.
Preflight success never verifies a signature, decrypts, authorizes a host task,
reserves storage or asserts that history is complete. Product consumers are not
rewired by this registry slice; their follow-up tasks must use these gates.

## Distinct consumer limits

| Consumer | State | Original JSON | Content | Raw JSON string | Tags / values each | Depth |
| --- | --- | ---: | ---: | ---: | --- | ---: |
| `proto-reference` | Existing selected wire codecs | 524,288 B | 262,144 B | n/a | 2,048 / 64 | 64 protobuf edges |
| `json-reader` | Existing generic JSON shape parser | 1,048,576 B | n/a | 65,536 B | n/a | 16 JSON containers |
| `project-web` | Planned event-admission pipeline inside the existing socket | 262,144 B | 65,536 B | 65,536 B | 2,000 / 16 | 16 JSON containers |

The proto-reference protobuf and output ceilings are 1 MiB; its original JSON
is opaque source material, not input to a JSON validator in the wire codec.
The generic JSON reader's writer ceiling is 1 MiB. The project pipeline's frame
and output ceilings are 262,144 bytes, with 64 pending verifications and a store
bounded independently by 512 events and 1 MiB. These queue/store budgets are
proposed configuration, not an installed store. No parser truncates to fit.

Budgets constrain actual measurements independently; they do not promise that
every combination of maxima can be transported simultaneously. Relay framing,
escaping, field-specific proto options and total resident storage still need
their own actual-byte checks. A 262,144-byte content field valid in a protobuf
fixture does not imply it fits the project JSON/socket path. Existing schemas,
published snapshots and parser/socket implementations are unchanged.

## Scoped checks and regeneration

From the repository root:

```sh
coax/bin/coax test coax/lib/nostr/profiles.coax
coax/bin/coax mutate coax/lib/nostr/profiles.coax
python3 coax/tests/nostr/profiles.py
python3 coax/tests/nostr/profiles.py --validate coax/lib/nostr/profiles.json
python3 coax/tests/nostr/profiles.py --write
```

`profile_matrix.coax` renders the registry. The Python script is optional,
explicit maintenance/test tooling: it compiles that Coax renderer with the host
C compiler, compares exact generated output, validates proposed machine-readable
claims through the Coax C ABI, checks source pins and exercises the policy against
an independent fixture/model. No Python enters an ordinary compiler/app build.
`--validate` permits supported narrowed budgets and refuses forged capability,
source/evidence, required-mask and implementation-state claims. `--write` updates
only the generated inventory; source policy is always edited in Coax.

Evidence for this change: five Coax checks, including 800 capability cases and
36 per-budget boundary cases; 136 mutants killed, zero survivors; eight focused
native/tooling checks. Emitted checked C ran under ASan/UBSan, and release/wrapping
C under `-O2`. Boundary inputs include unknown IDs, wrong cardinality, every cap
and cap+1, `UINT64_MAX`, `INT64_MIN/MAX`, disabled axes, incoherent pipes and
unsupported full-profile/signature claims. Seeded machine-readable faults are
refused. Existing proto options, JSON constants and socket caps are cross-checked.

The native JSON integer bridge compiles an **exact runtime prefix** of the actual
JSON source (before `// ---- checks`). The full source SHA-256 is
`3fcb17e95fd242ef68eeb1a6f0db703df5a5d98629c4075722b00dd41033ce42`;
the copied prefix SHA-256 is
`28887ed82a25ff92dd7ee6965ee18417f62421725dd79df23c2664a723c8226b`.
Its full captured-transcript check exceeds the compiler's default compile-time
budget when loaded into this combined wrapper, so that unrelated heavy suite is
not rerun or claimed here. The prefix is not a replacement parser: the narrow
bridge proves that actual `json_int(+/-2^53)` succeeds while the new safe-integer
projection refuses it, with both adjacent safe values admitted.

Verification used the existing `coax0 0.13.0` compiler, binary SHA-256
`1871b8030ecfcea59c75d573d4d6a8faab4cb46a28e4cbd8d71b492b59ba402a`,
and Apple clang 21.0.0 on arm64. Compiler/bootstrap, iOS, crypto and network
admission suites were not rerun for this new pure registry. This evidence does
not claim a full-profile, production-secret or cryptographic-security gate.
