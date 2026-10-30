# Confidential Clef recovery, 2026-10-30

The historical NIP-ATT Clef worker is running again on Google Confidential
Space. A fresh caller verified Google's certificate chain, Intel TDX,
production `STABLE` support, the admitted image and the endpoint-key nonce,
then completed an encrypted Nostr decision and checked its sealed receipt.

This is a recovered, immutable legacy Psionic/Pylon workload. The new Coax
public `/api/v1/systemone` gateway serves the separate ordinary model mesh.
It does not claim to hide input from its operator. A Coax NIP-ATT client and
a sealed public gateway remain to be implemented; these checks do not
silently turn the ordinary API into that client.

## Exact deployment

| Field | Value |
|---|---|
| Project / zone | `openagentsgemini` / `us-central1-a` |
| Instance | `oct30-att-tdx-clef` |
| Hardware | `c3-standard-8`, Intel TDX; CPU inference |
| Boot | Google production `confidential-space` family, secure boot |
| Network | Outbound relay/model access; all inbound traffic denied by the dedicated `oct30-att` tag |
| Image | `us-central1-docker.pkg.dev/openagentsgemini/openagents/att-provider@sha256:896b90ac19e38c879782cbf39274c8d440925bfeb62a14f925faf204102f71d0` |
| Image source | `14de46331a801bf4f4a73b7ed2ec47e5a857db2f` |
| Release | `28ac5b5295bd20af4b7649188dc70c6f05cfde3d85b4995148996cbdaf7a74ee` |
| Publisher | `77fabebbeb49a7b9b384422ee6ef5662cf4db7da70acc94981378c0017ecc56e` |
| Endpoint key at this check | `eadd886db9e84b3fa069e84282fec1943388d84baf1e3ad534254b524ed0dfc0` |
| Model | `clef-flash`, original `Clef-Flash-Q4_K_M.gguf` |
| Model SHA-256 | `fd3e90605e8103307dca37cb5a8cdb036267e2fe3cb2d908d80a8ceb9ec0638c` |

The existing publisher key was never read or used. The existing signed
release and head were retained; the workload generated a new endpoint key
inside the TEE. No key-file production signer gate was changed.

## Fresh evidence

- [Signed release, head and endpoint, including the public attestation](data/oct30-confidential/records.json).
- [Certificate / measurement / key-binding report](data/oct30-confidential/verify.json).
- [Encrypted round and verified receipt](data/oct30-confidential/round.json).
- [Changed measurement refusal](data/oct30-confidential/tamper-measurement.json).
- [Unbound key refusal](data/oct30-confidential/tamper-unbound.json).

The test asked whether a sunny-weather sentence was about the weather.
The sealed round took **8,404 ms**, with **7,883 ms** reported inference,
and answered `0.9733219061141002`. The receipt binds the admitted release,
image, endpoint, model digest and request-ciphertext digest. Both tamper
tests refused before encryption; no test input was sent on those paths.

These are dated captures, not a permanent liveness or token-freshness
claim. The signed endpoint must be fetched again and its current evidence
verified before releasing new input.

## Reproduce the historical verification

`scripts/inspect-confidential-pylon.sh verify` builds only the historical
read-only client as an **optional test oracle**, in a temporary directory.
It requires Rust 1.97.1 and Cargo; neither is added to the Coax build or
production gateway. Sources are read from pre-nuke history, and the reduced
dependency lock is pinned beside the evidence. The verifier is not restored
as a current Rust application workspace.

```sh
scripts/inspect-confidential-pylon.sh verify
scripts/inspect-confidential-pylon.sh round
scripts/inspect-confidential-pylon.sh tamper-measurement
scripts/inspect-confidential-pylon.sh tamper-key
```

Each command emits the client's structured report. Successful verification
has `fetch`, `chain`, `measure`, `bind` and no `refused` object. A successful
round ends in `receipt`. The two tamper commands must emit `refused` and
must not reach `encrypt`. The round uses a fresh disposable caller identity
and the fixed public weather sentence, never an operator key or private input.

To stop this recovered CPU worker without touching the model mesh:

```sh
gcloud compute instances stop oct30-att-tdx-clef \
  --project openagentsgemini --zone us-central1-a
```

The historical implementation and its threat model are recorded in
`git show 0b401561a0^:docs/security/private-inference.md`, with deployment
recipes in `scripts/deploy/att-provider.sh` at that same revision. The
historical H100 recipe also exists; this recovery did not deploy or claim
a confidential GPU.

Google's current [token claim reference](https://docs.cloud.google.com/confidential-computing/confidential-space/docs/reference/token-claims)
describes the vendor evidence. It is distinct from an ordinary Pylon's
signed execution receipt. The pinned client trusts Google and Intel for
hardware evidence and checks the logged workload before sealing input.
