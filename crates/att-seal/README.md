# att-seal

Sealed-job carriage for [NIP-ATT](../../nips/openagents/NIP-ATT.md#hpke-carriage)
endpoints that publish an HPKE key. Suite: RFC 9180 `mode_base`,
DHKEM(X25519, HKDF-SHA256), HKDF-SHA256, AES-256-GCM.

- `keys`: `Admitted::new` takes an endpoint the caller already verified
  (`oa-att` for Confidential Space, `att-platforms` for raw TDX, SEV-SNP
  and NRAS) and admits its HPKE key only if the binding covers it.
  `KeyBook` keeps one current key per endpoint and refuses rollbacks and
  expired records. `WorkerKeys` rotates with an overlap until the old
  record expires.
- `request`: `seal` binds the release, endpoint, recipient, HPKE key,
  request ID, caller, expiry and output limit (the HPKE info carries the
  binding; the header is the AAD). `event` signs it as a `25910`;
  `gift_wrap` hides the caller in a NIP-59 wrap. On the worker, `unwrap`
  checks the carrier and `open` refuses other endpoints or releases,
  unknown or retired keys, expired requests, replays and oversized limits.
- `stream`: per-chunk AES-256-GCM output under a key exported from the
  request's HPKE context, each chunk a `27020` event signed by the endpoint
  key. `ClientStream::accept` verifies each chunk alone and refuses gaps,
  repeats, foreign chunks, chunks after the last, unknown stop outcomes,
  output past the limit and late chunks.

Plaintext buffers and private keys are zeroed on drop and redacted from
`Debug`. `vectors/` holds the RFC 9180 vector for this suite (from the
CFRG test-vector file). The tests run a full local round against an
SEV-SNP endpoint verified by `att-platforms` on its synthetic AMD chain.

```sh
cargo test -p att-seal
```

Not yet: a deployed workload serving this, and wiring into the gateway or
the apps.
