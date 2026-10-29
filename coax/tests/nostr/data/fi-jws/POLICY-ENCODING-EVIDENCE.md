# Candidate FI configuration encoding

`fi-policy-encoding.coax` emits canonical configuration bytes, not an
`AssertionPolicyId` and not an authorization. It validates a selected community
and issuer with existing Coax policy, then serializes a domain/version string,
Host, audience, sorted issuer allowlist, issuer/JWKS URI, transport, class,
time/refresh settings, marker and sorted owner/client sets, and client/non-
collision flags. Strings are length-delimited, integer values use fixed-width
big-endian bytes, and sets sort bytewise. The caller must keep borrowed input
configuration valid and immutable during the call. Rotation keys/generation
are deliberately not part of these configuration bytes. The encoder validates
one selected pair in isolation; it does not bind the surrounding registry or
the effect of invalid/duplicate other communities on global selection.

It does not cover compiled verifier behavior, actual adapter/TLS behavior,
source/artifact/compiler identity, deployment trust, or provider binding.
The encoding is not installed into a build or provider and no consumer may use
it as the full ID. Those missing facts must be determined from trusted local
build/runtime state and enforced before any FI authorization. In particular,
no transported DTO flag or caller-supplied hash is evidence of compiled
behavior; the future build must derive and enforce its own fingerprint.

The independent Python oracle has 22 exact-output cases, one per selected
field change, set reordering, invalid settings and a maximum-size config. All
pass checked/release native, wasm32-wasip1 and native ASan/UBSan. The first
maximum case exposed an initial 40000-byte output capacity bug: a valid case
needs 41143 bytes. The owned capacity was increased to 65536 and that case now
passes; the Wasm test provisions a 1 MiB stack. Coax test currently passes 44
check groups. The first mutation run killed 583/596; 13 survivors were not
waived. Additional boundary checks and a simpler comparator were added; the
full default-budget mutation rerun killed 604/604, zero survivors and no
waivers.

SHA-256:

- `coax/lib/nostr/block/fi-policy-encoding.coax`: `bbf6759c9f899d8ef299b427178389c643f9cd6b8513156dd765fffeccfa4eac`
- `coax/tests/nostr/fi_policy_encoding.py`: `2470a813170b36eb45b7db07b38a16bad4fbe6234e7c36cd69ff20f6b1c979e4`
- `coax/tests/nostr/fi_policy_encoding_fixture.c`: `883a202a9a233c6c7410e5f712309ef2c70b314f62aafbbbd59598027a15d51e`
- `coax/tests/nostr/fi_policy_encoding_wasm.cjs`: `88af1af64a0a0f143d700063ba395aa94fd78bdc080533e07573f80c2ff1b1da`

Independent read-only source review (`fixedbase_rescue`) of the source hash
above found no concrete collision or omitted field within selected-configuration
scope. It checked domain/version, length and count framing, bytewise set order,
nonnegative fixed-width integers, explicit flags, validation and zero-result
overflow. Caller views must stay valid/stable and callers must inspect `ok`.
The 65536-byte owned result needs target stack budgeting. The review does not
approve a full policy ID, provider binding or production authority.

The same 44 Coax checks and 22 checked/release native, Wasm and ASan/UBSan
cases also passed with verified current-main coax0 0.21.0 SHA-256
`73d2aeb0e1e57c355d87242946f89e1462f8b765f57087f4307d471a0277e1d4`.
Mutation evidence uses the FI worktree coax0 0.18.2 binary SHA-256
`56f87661b9f1f105598132bf5c45f13e07fea46f4f23249921a22111d58f3ee4`.
