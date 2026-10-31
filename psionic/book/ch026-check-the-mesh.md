# 26. Check the mesh

[Chapter 25](ch025-the-api-fans-out.md) returns a checked remote answer. This
chapter adds three separate observations: the buyer reports accepted work, a
separately configured checker samples and replays it, and a reader recomputes
public totals. These are claims under explicit trust, not proof that a machine
executed a model or that demand came from independent customers.

The first native profile is **free decisions with throwaway identities**. It
preserves NIP-PYLON's published receipt.v2, NIP-32 check labels and pool.v2.
It does not authorize payment, credits or production keys. The numerical
comparison scope is the deterministic toy CPU service; real backends need
an explicitly qualified comparison profile before they inherit a pass.

## Report only accepted remote work

Receipts are opt-in. The ordinary fleet command still returns answers without
publishing them. Add a checker executable and an explicitly pinned, different
reference provider to enable the mesh path:

```shell
psionic/platform/mesh-build.sh /tmp/psionic-mesh sanitized
identity=psionic-toy@sha256:b6680707750842c489bad8a5524bb1b1393ff04f6917b72cf49c2cf87d633573
original=${ORIGINAL_PROVIDER:?set the original Pylon public key}
reference=${REFERENCE_PROVIDER:?set a different trusted Pylon public key}
/tmp/psionic-mesh/psionic-fleet --identity "$identity" --port 8799 \
  --provider "$original" \
  --mesh-checker /tmp/psionic-mesh/psionic-mesh-check \
  --checker-provider "$reference" --sample-256 256 \
  --maximum-distance-ppb 0 --backend-profile psionic-toy-cpu-v1
```

Both Pylons must already serve the exact identity as in chapter 24. The
reference key comes from the operator's configuration. A signed beacon does
not establish a machine's owner, physical identity or trustworthiness.
`--provider` pins the original route; omitting it retains normal fleet choice.
The checker still refuses to call replay on the same provider independent.

A normal `POST /v1/systemone` returns before receipt/checker processing. Only
an acknowledged, admitted, on-time remote result can produce a receipt.
Fallback, refusal, cancellation and timeout produce no credited remote job.
The receipt has `outcome:"accepted"`, one `jobs` unit and null payment.
Publishing a receipt can fail independently after a successful HTTP reply;
the runtime reports unconfirmed publication rather than claiming success.

The buyer signs kind `3201`. Its exact public fields and `p/a/t/x` bindings
are checked by [`mesh/receipt.coax`](../lib/mesh/receipt.coax). The request is
the original signed job event ID. `request_digest` and `result_digest` hash
the **exact complete decrypted NIP-DEC envelope bytes**, including their
whitespace and metadata. They are not hashes of newly rendered JSON.
The receipt content itself is JCS and its `x` hashes those exact JCS bytes.
Even a valid digest is a commitment, not evidence of execution. Low-entropy
plaintext can sometimes be guessed against a public digest.

## Sample after the answer

Sampling draws a fresh OS entropy byte only after the answer passes. A
threshold in `[0,256]` selects `byte < threshold`: zero disables replay,
256 selects every result, and the default 64 selects roughly a quarter.
These choices do not depend on a publicly predictable request ID.

For a selected result, generated Coax protobuf setters construct
`openagents.psionic.mesh-witness.v1`. It binds the signed receipt, original
signed encrypted request/result, exact plaintexts and pinned comparison
context. Its two 76-byte opening keys are specific to the message nonces.
Neither a signing private key nor a reusable conversation key crosses the
boundary.

The witness travels only through an owned, bounded private child pipe. The
checker admits FIFO or Unix-domain socket input, rejects regular files and
TTYs before reading, and caps the whole message at 262,144 bytes. The parent
uses a bounded flush, closes the pipe and owns at most four checker children.
Stop releases only its own children. No private witness, opening key or
request/result plaintext is logged, written to a regular file or sent to
stdout. The [protobuf schema](../../proto/openagents/psionic/mesh/v1alpha1/witness.proto)
is carriage, not an assertion that its contents are verified.

The checker independently:

1. Verifies all three original signatures and the receipt's JCS/tag bindings.
2. Authenticates each signed ciphertext with its nonce-specific opening,
   checking the exact disclosed plaintext and its public digest.
3. Binds buyer, provider, recipients, job event ID, request nonce, attempt,
   original timestamps, model base name and exact service identity.
4. Requires the witness profile and limit to match its own configuration,
   then observes the same signed receipt on the relay.
5. Uses a fresh checker key, distinct from buyer and original provider, and
   replays on the configured different reference provider.

An invalid, unopenable or misbound original produces **no label**. Merely
supplying a plaintext with the right claimed hash is insufficient: the
original signed ciphertext must actually open to it.

## Compare decisions, not their route metadata

`centred-log-f64-max8-v1` compares choice distributions, score distributions
and the Bernoulli form of a noul answer. It pins the probability floor to
`1e-6`, uses fixed-order binary64 arithmetic, and supports at most eight
options per question. It requires the same decisions and checks maximum
centred-log distance against the configured parts-per-billion limit.
This is a named synthetic comparison scope, not empirical calibration of
arbitrary hardware or models. [Chapter 217](ch217-how-far-apart-may-two-honest-answers-be.md)
explains the underlying distance and its limits.

The original result's metadata remains committed by the receipt. Comparison
projects only admitted model/identity and answers; request IDs, attempts,
latency, usage and provider/route metadata do not create automatic mismatch.
The explicit backend context is compatible only for the configured
`psionic-toy-cpu-v1` and this exact toy identity. A payload cannot approve its
own backend.

A valid supported replay can pass or fail. Unsupported shapes, unknown
backend scope, self replay, unavailable replay or a malformed replica are
inconclusive. None is silently converted to a pass. The pure
[`mesh/compare.coax`](../lib/mesh/compare.coax) returns the reason and first
question attaining the maximum distance.

The checker signs a kind `1985` NIP-32 label naming the receipt and original
provider. Its `x` commits the exact complete replay-result plaintext when
one was admitted. If no replay result exists, it instead hashes the precise
JCS `openagents.psionic.mesh-inconclusive.v1` observation specified in
[NIP-PYLON](../../nips/openagents/NIP-PYLON.md); the inert method text identifies
that observation and reason. It is not represented as a model result.
The runtime exposes the owned child's public checker key and verifies its
final signed label before reporting progress. A pool must still pin that
checker explicitly and exclude the receipt's buyer/provider as checkers.

## Totals any reader can recompute

`psionic-mesh-pool` admits original signed public events with the strict
readers, applies explicit provider/buyer/checker allowlists, and calls
[`mt_recompute`](../lib/mesh/totals.coax). Empty lists count nothing. Public
policy is window-independent canonical JSON; its SHA-256 digest is the
pool's policy ID.

Pass a closed whole-minute Unix-second window `[from,to)`, at most an hour.
Choose a window containing both the work and its check after that window
has closed. Recent incomplete windows refuse. For example, with those
operator-selected variables and the actual public buyer/checker keys:

```shell
/tmp/psionic-mesh/psionic-mesh-pool --pool demo --from "$from" --to "$to" \
  --provider "$original" --provider "$reference" \
  --buyer "$buyer" --checker "$checker" --publish --bundle > pool.pb
/tmp/psionic-mesh/psionic-mesh-pool --pool demo --from "$from" --to "$to" \
  --provider "$original" --provider "$reference" \
  --buyer "$buyer" --checker "$checker" --verify "$aggregate_id" --inputs < pool.pb
```

The signed aggregate's public event ID is reported separately on stderr.
`--bundle` stdout contains only generated `PublicPoolInputs` protobuf bytes;
TTY output refuses. This bundle contains original signed **public** records
and public JCS policy, never a private witness. Public stdin transport is
bounded at 2 MiB; the current generated codec has a stricter **1 MiB** limit
and refuses larger messages without truncation. Online verification can
instead fetch public records with the same explicit policy and `--verify`.
`--policy` prints the canonical public policy alone.

The bounded profile admits at most 32 beacon, 64 receipt and 128 check inputs.
Detected overflow or incomplete collection refuses rather than publishing
partial totals. Beacons use the newest generation/observation/event-ID at
or before the window end, including offline/expired tombstones. Receipts use
the earliest `(created_at,event ID)` per `(buyer,request)`, independent of
delivery order; freshly re-signing the same logical job cannot inflate it.
For each trusted checker/receipt pair, the latest label wins. Labels must
not predate the counted receipt and must precede the window end.

The aggregate separates **buyer-reported** accepted jobs from trusted check
observations. Missing checks prove nothing. Capacity remains providers'
claims. Paid buckets are zero. Digests hash sorted counted IDs separated by
newlines without a trailing newline. Any verifier with the same complete
original input cohort and policy can reproduce those commitments. This checks
mathematical claims against the supplied or observed cohort, not global relay
completeness. An aggregate signer does not implicitly authorize its buyers,
providers or checkers. Replaced
or withheld events can make an online view incomplete; that is unverified,
not a negative judgment about a provider. A public bundle preserves the
original cohort for later independent recomputation.

## Check the changed scope

```shell
psionic/check.sh --only mesh --jobs 4
```

This selects the new authority roots, keeps every imported baseline check,
mutates only each selected file's actual source, and runs its native pure
consumers. It bypasses unrelated model and text oracle setup. The actual
signed/encrypted fleet/checker/Pylon flow has a separate native end-to-end
harness in `psionic/tests/mesh21/mesh_e2e.ts`; native builds use ASan/UBSan
and the ordinary Coax and C toolchains. No model weights are downloaded.
The [synthetic comparison example](../examples/mesh-compare.coax) and
[admitted totals example](../examples/mesh-totals.coax) run without a relay.
Their debug/release transcripts are in the closed book inventory.
The [private foundation](../../docs/verification/psionic-mesh-private-foundation-2026-10-30.md)
and [native/public verification record](../../docs/verification/psionic-mesh-native-2026-10-30.md)
identify exact source/artifact hashes,
synthetic cases and the real Mac/Linux public-relay rehearsal separately.


[Chapter 27](ch027-a-public-mesh.md) places the native buyer behind the site's
authenticated public edge, preserving the same model/provider pins and keeping
public API admission separate from private fleet IAM and Pylon transport.
