# Mesh checks

Product logic runs in compiled Coax. `mesh_e2e.ts` is an independent Bun
protocol peer and public-pool reference, using pinned Noble/NIP-44 test
libraries only. It uses fresh throwaway identities, loopback by default, no
money, and memory-only private witnesses. It never serializes private stdin,
opening keys, conversation/signing keys, questions or result plaintexts into
evidence. Failure diagnostics avoid secret assertion values.

Install the pinned test dependencies and build with a qualified compiler:

```sh
bun install --cwd psionic/tests/pylon --frozen-lockfile
COAX=/path/to/qualified/coax psionic/platform/mesh-build.sh /tmp/mesh-build sanitized
```

For the full harness, also supply qualified native Coax relay, Pylon and toy
engine artifacts (their build instructions are in the existing Pylon demo):

```sh
MESH_BIN_DIR=/tmp/mesh-build \
RELAY_BIN=/path/to/coax-relay PYLON=/path/to/pylon SERVE=/path/to/psionic-serve \
bun psionic/tests/mesh21/mesh_e2e.ts
```

The native test starts two engines and two Pylons with temporary test-only
identity labels removed after startup. It does not touch existing Pylons.
The final pipeline uses the already observed public receipt and check; it
may wait up to one minute for that work's window to close. It never reissues
the model job just to compute its pool totals.

Select only changed scope with `MESH_MATCH` (substring), `MESH_ONLY` (exact
names separated by `|`) or `MESH_SKIP` (one exact name). Pool-only selectors
need only `psionic-mesh-pool`, the pinned test dependencies and the public toy
self-check fixture. For example:

```sh
MESH_BIN_DIR=/tmp/mesh-build \
MESH_ONLY='pool: malformed relay EVENT envelope refuses completion' \
bun psionic/tests/mesh21/mesh_e2e.ts
```

`OUT_DIR` controls public evidence; `MESH_SOURCE_ROOT` selects a different
source tree. For `pool: actual API receipt joins independently recomputed
totals` in isolation, `MESH_API_EVIDENCE_DIR` points at retained `results.json`
and public event records from a qualified native/API phase. This reuses the
signed records without rerunning the original model job.

Public publication is opt-in: `MESH_PUBLIC_COHORT` names an explicitly
approved, public cohort metadata file; then select `pool: actual public
cohort aggregate and independent verification`. Do not point this at an
arbitrary production account or private witness.

The public decoder/reference can also be imported with
`MESH_REFERENCE_ONLY=1`; that mode executes no cases or network calls.
Exports `publicBundle(bytes)` and `referencePool(bundle, explicitTrust)`
verify original signatures and recompute every free-profile input commitment
and total. Pass independently pinned provider/buyer/checker sets and window;
a bundle's own policy is not a source of trust.

Host-only gates are separate:

```sh
psionic/tests/mesh21/check_input.sh
psionic/tests/mesh21/check_public_input.sh
psionic/tests/mesh21/check_public_output.sh
```

The [qualification record](../../../docs/verification/2026-10-30-mesh-e2e.md)
keeps each actual artifact hash and scoped result, including the expected
v3 malformed-frame failure followed by the v4 fix. Do not rerun already
passing scopes because an unrelated main commit arrived.
