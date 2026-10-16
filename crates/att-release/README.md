# att-release

NIP-ATT release assurance beyond the release head
([NIP-ATT](../../nips/openagents/NIP-ATT.md#attested-workload-release--kind-3202)):

- `rekor`: RFC 9162 inclusion and consistency proofs, Rekor's signed
  checkpoint (signed note) and signed entry timestamp under a pinned log
  key, and a `Witness` that keeps the largest checkpoint per log and
  refuses stale views, forks, and growth without a consistency proof.
  Sigstore's Rekor key ships in `vectors/`, with a real Rekor entry
  (index 3250000000) the tests verify offline.
- `rebuild`: `3203` independent rebuild statements, checked against the
  release's source and image; a changed build refuses the release.
- `approvals`: the personal approval set (`openagents.att-approvals.v1`),
  sealed as a self-addressed `3188`, with notice, emergency and revocation
  behavior and generation rollback refusal.
- `receipt`: sealed-job `3201` receipts (Pylon v2 with
  `requires: ["openagents.attested.v1"]` and the attested block), with
  digests over ciphertext, payment only as an opaque settlement reference
  and acknowledgment, and a leak check.
- `check_release` composes head admission, transparency, rebuilds and
  approvals.

The `att-release` binary:

```sh
att-release verify --release R.json --head H.json --publisher HEX --workload vault --now SECS \
  --rekor ENTRY.json --log-key rekor.pub --witness witness.json --rebuild B.json --min-rebuilds 1 \
  --approvals approvals.json
att-release rebuild --secret-file builder.key --release R.json --image-digest sha256:HEX --now SECS
att-release approve --approvals approvals.json --release ID --workload vault --now SECS
att-release revoke --approvals approvals.json --release ID --now SECS
att-release receipt --receipt E.json --request E.json --result E.json --endpoint ADDR \
  --endpoint-key HEX --release ID --level tee-cloud --measurement sha256:HEX
```

Tests use a synthetic log key, publisher, builders and a fake regtest
payment; no real money moves.

```sh
cargo test -p att-release
```
