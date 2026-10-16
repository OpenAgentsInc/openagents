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

The local fixtures use an assumed endpoint and LocalKms. A separate
[TDX research run](../../docs/research/dhruv/conformance-evidence-2026-10-16.md)
verified an actual Google token, image, signing key, and HPKE binding before
opening a synthetic object. Its public capture and signed completion verify
in `dhruv-conformance`. The earlier captured endpoint for another workload
remains refused. Production files, a remote client, and attestation-bound
Cloud KMS remain owner qualification steps.

`examples/cloud_fixture.rs` is a separate, one-shot research workload. Build
it with `OA_VAULT_RESEARCH_COMMIT` set to the source commit and package the
binary with `deploy/vault-sealed/research.Dockerfile`. Its `prepare IMAGE
COMMIT` command creates public research release and head records with a
600-second notice. After that notice, supply the JSON through
`OA_VAULT_RESEARCH_RECORDS` to a bounded Confidential Space TDX instance.
The `run` command verifies the actual Google token, image, signing key, and
HPKE binding before opening a synthetic object under a one-use lease. It
checks replay, revocation, key substitution, and expiry, then prints public
records and an endpoint-signed completion. It exports no private keys.

The client runs in the same process, and its key service is ephemeral
`LocalKms`. This exercise does not qualify production storage, remote
clients, Cloud KMS policy, independent rebuilding, or recovery after a
machine is lost. Delete the research instance after collecting its result.
