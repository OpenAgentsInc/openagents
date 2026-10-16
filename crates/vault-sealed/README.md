# Sealed vault tier

`vault-sealed` implements NIP-VAULT tier `sealed` over the existing owners:
`oa-vault` holds the object format and the wraps, and `oa-att` verifies the
attested endpoint. Tier `user` (device only) is unchanged.

## Client (`client`)

- `verify_endpoint` runs the full `oa-att` check (records, Confidential
  Space token chain, measurement, key binding) for workload `vault`. It
  requires level `tee-cloud` and an HPKE session key.
- `accept_system_key` takes a system key certificate only when it is signed
  by that endpoint's key, names the same release, arrives while the
  attestation is current, and is not an older epoch than one already seen.
- `seal_object` makes a tier-`sealed` object with the HPKE `mode_psk` wrap to
  the accepted system key (`oa_vault::sealed`) and, unless turned off, a
  device copy.
- `lease` and `revoke` are signed by the person's enrolled slot key. A lease
  names exact objects, a purpose, a route, its effects, an expiry of at most a
  day, and a use count.
- `request` seals one HPKE `mode_base` request to the endpoint's session key.
  The request carries the person's share `S_user`, and `open_replies` opens
  the answers.

## Workload (`workload`)

- Each person has one system key per epoch. Its private half exists only as
  KMS ciphertext that the KMS releases only to the attested release
  (`kms::Kms`, `LocalKms`). `rotate` starts a new epoch.
- `serve` opens a request only with the endpoint's session key, verifies the
  lease against an enrolled slot key, and refuses when the lease is expired,
  too long, already used, revoked, or wider than the request. It also refuses
  `fast` without declared egress, `device` without `export`, and
  `write_derived` (not built yet). It then opens each object with the system
  key and the request's share. A wrong share or a rotated system key opens
  nothing. Answers are sealed to the request's reply key.
- The share, the system key, and every data key are dropped after the run.
  The audit log records lease, run, object IDs, route, and result, with no
  content and no key bytes.

There is no path for unattended decryption: every opening needs a fresh
request that carries the share under a current lease.

The fixtures run the workload in-process with a fixture endpoint and a local
KMS. The real-endpoint test confirms that the captured TDX endpoint, which
serves another workload and has no session key, is refused. Deploying the
vault workload on Confidential Space with an attestation-bound KMS key is an
owner step.
