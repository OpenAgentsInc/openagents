# Operator vault tier

`vault-operator` implements NIP-VAULT tier `operator` as an explicitly
labeled custody profile. **The operator can read these files.** Anyone
holding decrypt on the KMS key opens them. This tier is never the `user`
(device only) or `sealed` (attested hardware plus your share) guarantee, and
clients must say so before a file is stored in it.

What the service adds is exact control and evidence:

- `oa_vault::operator` makes and opens the `operator` wrap: the data key
  under a named KMS key version, with the object's `core_digest` as
  associated data. The vectors are in `fixtures/nips/vault/vectors.json`
  under `operator`.
- A configured operator signs a grant. The grant names exact objects, one
  registered recipient, a purpose, an expiry within a day, and a use count.
- The recipient signs each use request. It names the grant, one object, and
  the purpose, and is checked against the grant, the object's identity, and
  its tier.
- Every key use is preceded by a durable, hash-chained audit entry. If the
  entry cannot be written, the key is not touched. Entries hold identities
  only (grant, request, object, recipient, key version), never content.
  Opening the service verifies the whole chain, so an edited line stops it.
- A request ID binds its exact bytes. Re-sending it after a lost reply
  re-delivers with a new audited key use and spends no new use. A key-service
  error after the call is `unknown` until a retry of the same request
  reconciles it. The same ID with other bytes refuses.
- Revocation and expiry stop further use, and both are audited.

```sh
vault-operator init --dir DIR --config CONFIG.json
vault-operator grant --dir DIR --kms-key KEY --kms-version NAME --input SIGNED.json
vault-operator use --dir DIR --kms-key KEY --kms-version NAME --input SIGNED.json --object FILE
vault-operator revoke --dir DIR --kms-key KEY --kms-version NAME --input SIGNED.json
vault-operator audit --dir DIR --kms-key KEY --kms-version NAME
```

The command line uses a synthetic local key from an owner-only file. A
production service mounts its KMS behind `OperatorKms`. No private backend
custody code is reused here.
