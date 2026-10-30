# Large source-audit profile: public fields, void calls, signer reach

Part of #4828. Follows [v3 ranges](2026-10-30-secret-audit-ranges.md) and
[wrapping products](2026-10-30-secret-wrapping-products.md). It does not
complete #4828, and it is not a signer or production approval.

## Why

The Schnorr signer (`schnorr_sign.coax`) keeps public values inside its
secret workspace:

- the `SignWork.stage` counter;
- the SHA-256 length and phase fields `ShaOwned.used`, `phase` and `total`.

Taint was tracked per view, so classifying the workspace as secret made every
branch on the stage a secret branch. Its `int` fields also failed type
admission, because a secret `int` is never admitted.

## Changes to the large profile

- **Public fields.** A new manifest schema,
  `coax.secret-large-classification-views.v4`, adds `public_fields`: up to
  eight `[struct, field]` pairs naming primitive fields that hold public
  values. They reach Coax as extra labels (`access = 3`).
  - **Shadows:** each root view gets a shadow region per named field,
    created at entry and starting public. Reads of that field use the
    shadow's taint.
  - **Writes to the field:** these taint the shadow and also the view's
    region, so copying the whole aggregate keeps every secret.
  - **Other writes:** a write to any part of the view whose type can contain
    the field's struct also taints the shadow.
  - **Where the taint lives:** element and field reads keep the view's taint
    in its region instead of folding it into the value early, and only for
    views that have shadows. A copy of the aggregate has no public fields.
  - **Type admission:** a named field may be an `int` inside a secret
    aggregate.
  - **Fails closed on:** invalid names, non-primitive fields, duplicates,
    more than eight names, and use without the view profile.
- **Loops with public fields.** Shadows are not pre-tainted. A loop pass
  stands only when the shadow taints are unchanged as well as the ranges
  inductive, so a write on one iteration reaches the reads of the next.
  Passes 4–7 use type ranges. After eight passes without a fixed point, the
  analysis fails closed (`limit`).
- **Call statements and procedures without a result.** An expression
  statement must be a call, and its value is discarded. A procedure without
  a result type may end without `return` and may use `return;`. Its effects
  are its analyzed memory writes.
- **Type resolution.** Locals whose type is inferred are matched to an equal
  type node. Candidate nodes this profile cannot represent, such as a test's
  `[1048577]u8`, are now skipped instead of refusing the whole audit.
- **Nesting.** This profile's type validation and containment allow 16
  levels. The shared profile's bound of 8 still applies through
  `AG_valid_type` and `AG_match`.
- **Range-decided comparisons.** A comparison whose result the operands'
  ranges fix is a public constant. Its branch goes the same way for every
  secret value. For example, `cb_store_word` checks `value > 4294967295` on a
  32-bit limb.

The legacy and public-view profiles and the shared tests are unchanged. The
correspondence check now applies an exact, ordered block list per file
(`large-delta.json`): 33 blocks for `aggregate.coax`, 1 for the bridge's
label limit (8 parameters plus 8 public fields), and 6 for `aggregate.py`.

## Source change

In `secp_field32_total.coax`, the second fold now takes `folded.carry & 1`.
A fold's carry is always 0 or 1, so the value is unchanged; the mask makes the
range visible, since ranges are not kept through struct fields. The file's
25 checks pass, mutation killed 388 of 388 mutants, and the signer's 216
checks (BIP-340 vectors included) pass with `--run-steps 250000000`.

## Checks

On compiler `coax0 0.23.0` SHA-256
`cd94879ecc21e72cc0d41a5e66c38194a0638a7167d01eb3a05380b4bf07fcfd`, built
from the current compiler inputs (fingerprint
`c590192e23d79f746261640b4590b7b696437f7d6c6f9eefd9aec5a221dff685`). Earlier
records today used an older build, `03498a…`.

- **`aggregate.coax`** (SHA-256
  `61f3dc35c657d1ef67fae81327cfb797e4676d3d0360dbae4d79582ce7965da2`): 223 of
  223 Coax checks pass. They include
  `coax/tests/nostr/secret_field_cases.coax`: 24 source-shaped cases plus the
  7- and 8-hop fixed-point cases.
- **Mutation:** an ordinary run in 20 shards killed **3,615 of 3,615, 0
  survived** ([log](data/secret-public-fields-2026-10-30/large-mutation.log)).
  - Earlier runs had survivors; each was killed by a new check or by
    removing redundant code.
  - The redundant code: the operator guard and bool exclusion in
    `AG_decide`, an own-type shortcut, the empty-label guards, and
    `r.known` for comparisons.
  - None were marked equivalent.
- **Python suites:** `test_public_views.py` passes 9 of 9, including v4
  manifests with valid and malformed `public_fields`. `test_large_closure.py`,
  `test_correspondence.py`, `secret_checked_cast_types.py` and
  `secret_contained_cast.py` pass. The real ECDH entry still completes its
  dataflow (61 procedures) and leaves only its two public guard obligations.

## Signer reach

Each stage was classified as follows: the key and aux are secret; the 32-byte
message, `out` and the result are public; the workspace is secret, with
`SignWork.stage` and `ShaOwned.used`, `phase` and `total` public. All runs
used `--run-steps 250000000`.

| Stage | Result |
| --- | --- |
| `sw_key` | Analyzes 53 procedures: scalar load and validation, fixed-base multiplication, affine conversion and the public-key bytes. Then refuses at `schnorr_sign.coax:140`, the signer's intended branch on key validity ("its validity is the call's public result"). |
| `sw_nonce` | `type` after 8 procedures; not yet diagnosed. |
| `sw_scalar` | Refuses at the call to `sv_challenge_vartime` (line 149). Its inputs are public, but the `_vartime` name is always refused. |
| `sw_publish` | Refuses at the variable-time self-check (line 161), which #89db replaces. |

The remaining refusals are mostly the signer's designed declassification
points: key validity, the public key and nonce point R written into the
workspace, the challenge over public data, and the publish decision. The
analysis needs an explicit, reported way to express those. That, and
`sw_nonce`'s type refusal, are the next steps for #4828.
