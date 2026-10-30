# Chapter21 public receipt and check-label profile

The authored authority unit is `psionic/lib/mesh/receipt.coax`. It implements
existing NIP-PYLON `openagents.pylon-receipt.v2` / kind 3201 and OpenAgents
NIP-32 kind 1985 labels; it introduces no wire version or payment flow.

The producer emits canonical JCS and exact content SHA-256 tags. The reader
requires a JCS body at most 4096 bytes, all fifteen documented fields, an empty
feature list, `cj-decision`, the exact provider's `pylon/decision` capability,
one accepted `jobs` unit and null payment. It binds the signer to buyer,
provider to both addressing tags, request and plaintext digests to 64 lowercase
hex characters, and safe integer clock/order fields. Shared slugs start with
an alphanumeric byte. Unknown fields, any `meta`, paid/attested receipts and
other outcomes/units remain outside this first producer/reader subset; they
are refused rather than relabeled free.

Labels have exactly the five documented tags. Their `x` is the checker's
own result-plaintext digest, not the label-content digest. Methods are inert
nonempty text of 1..512 bytes. The pure reader requires checker != provider;
the referenced receipt's buyer exclusion and explicitly pinned trust are
consumer responsibilities. The caller must verify the complete original
Nostr event ID and signature before either pure reader; admission here is
not trust and does not prove private execution.

Independent pinned body/tag/digest fixtures, per-field and per-tag negative
cases, exact integer/type/clock/capacity boundaries, canonical JSON and slug
rules passed 251/251 checks, including imported baseline checks. Entry-source
mutation killed 253/253 mutants, with zero survivors and no equivalence
waivers. New checks caught independently mismatched addressing tags and
ill-typed timestamps whose default values could otherwise appear ordered.

Commands (from the repository root):

```sh
/tmp/coax-root-only-tools/coax test psionic/lib/mesh/receipt.coax
/tmp/coax-root-only-tools/coax mutate psionic/lib/mesh/receipt.coax --root-only --jobs 8
```

This is the qualified scoped development driver from main 9322269a89,
SHA-256 `1cf5c9a31ef3bcf6d88a718c5ed7ff71f9dbc87ae04f57788f3ee59bfc2641e8`.
Native product compilation continues to use the separately qualified release
compiler. The entry SHA-256 is
`5091d7e24749c9ae9240cb99df5b33116f60fb29bfbaf687899a3674934a297e`.
The [manifest](data/oct30-mesh-receipt/manifest.json) pins every imported
source; [checks](data/oct30-mesh-receipt/checks.txt) and
[mutation](data/oct30-mesh-receipt/mutation.txt) record the actual gate.
Native fleet/checker/pool end-to-end integration is a separate qualification;
this record does not claim it has run or that Chapter21 is publicly deployed.
