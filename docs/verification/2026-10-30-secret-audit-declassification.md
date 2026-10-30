# Large source-audit profile: explicit declassification

Part of #4828. Follows [public fields](2026-10-30-secret-audit-public-fields.md).

## Rule

A signer has to make some secret-derived facts public: whether the key is
valid, the public key, the nonce point R, and whether the finished signature
is published. The profile now lets source code do that only explicitly, and
reports every place it happens:

- **Which calls:** a call to a procedure whose name ends in `_declassify` is
  analyzed like any other call, so its body must itself pass.
- **What becomes public:** its scalar result, and any public-field shadow
  passed to it as an argument, such as `work[0].prep[0].public_key[..]`.
  Nothing else becomes public: other views, the view's own region and
  aggregate results are untouched. An aggregate result, or a declassification
  outside the view profile, refuses.
- **How it is reported:** each call site is recorded as an obligation of
  kind `declassification`, with the taint it removed. The run then reports
  `guard-obligations` (`analyzed-with-guard-obligations`), never `accepted`.
  Authored assertions keep kind `authored-assertion`.

Public fields may now also be arrays of primitives, such as
`SignPrepare.public_key`, `SignNoncePoint.r` and `SignWork.challenge`.

I also tried admitting `_vartime` calls whose arguments are all public, but
did not keep it. The inherited check "public calls require opt-in and
forbidden callees stay forbidden" pins that `_vartime` callees are always
refused. Relaxing that gate is a policy decision for the owner, not something
to change silently. The signer's challenge over public data therefore still
needs an analyzable `_public` form.

## Checks

On compiler SHA-256
`cd94879ecc21e72cc0d41a5e66c38194a0638a7167d01eb3a05380b4bf07fcfd`:

- **`aggregate.coax`** (SHA-256
  `3f04ddec3aa7a2ae78a18e6a1fa17f959a328cdf9f141f7565e2217d9801cfe4`): 225 of
  225 checks pass.
  - Five new graph cases in `secret_field_cases.coax` (SHA-256
    `5526ace964fa0e07779e73bea4d3112120bf56f2c8400b99dca468b0ae820cc6`): a
    declassified validity bit and a declassified public byte array are both
    reported; the same programs without the call refuse as secret branches;
    an aggregate declassification refuses.
  - Direct checks of the name rule, shadows, flat field types, obligation
    merging and the 256-obligation limit.
- **Mutation:** an ordinary run in 20 shards killed **3,657 of 3,657, 0
  survived** ([log](data/secret-declassification-2026-10-30/large-mutation.log)).
- **Python suites:** `test_public_views.py` (10 of 10, including a report
  check that a declassification is labelled as one), `test_large_closure.py`,
  `test_correspondence.py` (exact blocks: 37, 1 and 7) and
  `secret_checked_cast_types.py` pass.

## Next

Mark the signer's intended points with `_declassify` calls in `sw_key` and
`sw_nonce`, and give the challenge a `_public` form. This invalidates the
recorded mutation runs of `schnorr_sign.coax` and `event_sign.coax`, which
must be rerun with `--run-steps 250000000`.
