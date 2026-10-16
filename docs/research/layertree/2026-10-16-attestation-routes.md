# GCP attestation routes tested on signet infrastructure

On October 16, 2026, the #11787 probes qualified Confidential Space on
Intel TDX for the next signet service experiment. The client verified a
Google-signed token naming the exact test container and the hash of its
fresh public key and tree parameters. This route trusts Google's
attestation service, launcher, and image measurement policy, in addition
to Intel hardware. It does not establish a vendor-only verification route.

No SEV-SNP route on GCP qualified in this experiment. The raw SNP report
measures firmware, and the current Cloud Attestation service explicitly
rejects SNP. The SNP service experiment therefore needs another cloud or
an additional measured-boot design; it does not block TDX signet work.
Neither result authorizes mainnet use.

## Captured evidence

[Public evidence](evidence/attestation-routes-2026-10-16.tar.gz) contains
raw reports, boot event logs, challenges, summaries, launcher errors,
instance identities, cleanup evidence, and a SHA-256 manifest. It contains
no wallet, seed, private key, or service-account credential. The real TDX
workload response is also the
[`oa-att` fixture](../../../crates/oa-att/tests/fixtures/layertree-tdx-probe.json).
The test image signs no transaction and has no key export API.

| Probe | Configuration | Observed result |
| --- | --- | --- |
| Raw SNP | `n2d-standard-2`, AMD Milan, Ubuntu 24.04 image `v20261009` | 1,184-byte version-5 report; fresh 64-byte challenge echoed; VMPL 0; no SVSM in the boot log |
| Raw TDX | `c3-standard-4`, same Ubuntu image | 8,000-byte version-4 quote; fresh challenge echoed; RTMR3 test extension matches local SHA-384 computation |
| SNP Confidential Space | Production image `confidential-space-260900` | Launcher's attestation request fails with `UNSUPPORTED_CC_TECHNOLOGY`: AMD SEV-SNP is not supported by Google Cloud Attestation |
| TDX Confidential Space | Same production image | PKI token verifies with `oa-att`, hardware `GCP_INTEL_TDX`, debug disabled since boot, expected container digest and nonce |

The test container is:

```text
us-central1-docker.pkg.dev/openagentsgemini/layertree-research/attestation-probe@sha256:92c8b535757b1fa43f2ab85882d0fe58c42dbe29562c3cfcbb5b6953130448c6
```

Its Rust probe and Docker recipe come from commit
`ecd065a0b2c61e67ddedc5cd0934840788bc11b1`. A first launcher attempt
rejected log redirection because the image lacked the required policy
label. That setup failure is captured separately; the corrected image
produced the results above. This build is an experimental image, not yet
an independently reproduced release (#11791).

## What each route proves

**Raw SNP.** Two reports with different challenges have the same
48-byte launch measurement:

```text
0e017d2fba7c4964575fdd8c770061abf07f0753148d030f94e969a2d1fa5a5747ba0ced9174f2d44b5f6663fa2de00f
```

Google documents that this measurement covers the initial guest firmware
and pages, rather than our disk image. The captured guest uses no SVSM.
Its hypervisor-provided vTPM event log is therefore insufficient to claim
AMD-only protection of our application's image identity. Hashing a vTPM
quote into `REPORT_DATA` would bind the supplied quote bytes, but would
still require trust in the vTPM provider and an enforced measured boot
chain. We did not build a dm-verity image or qualify such a chain. We do
not claim that all possible SNP measured-boot designs are impossible.

**Raw TDX.** The quote contains firmware and RTMR values. Writing the
SHA-384 hash of a test marker to the guest's RTMR3 interface produces
`SHA384(old_RTMR3 || marker_digest)` in the next quote. The captured marker
summary records the expected and observed values and
`proves_boot_image: false`. A guest can extend a marker after boot; that
alone does not prove the marker describes the code holding a key. The
Ubuntu command line has no pinned dm-verity root. A vendor-only route
still needs an enforced, independently reproduced boot chain and Intel
collateral verification. Raw report signatures were not verified in this
spike; the raw captures are diagnostic evidence.

**TDX Confidential Space.** The verified Google token supplies the
container digest and debug policy. Its nonce is the 64-character hex
encoding of the first 32 bytes of the lab's exact `report_data(params)`;
the remaining 32 bytes are zero. The probe generates the key inside the
workload and requests this nonce over the launcher's Unix socket. The
client independently recomputes the tagged hash over `Q`, operator key,
tree ID, depth, dust minimum, delay, and migration flag. The test also
rejects another audience, a changed token, and expired evidence. The
client must additionally check an approved reproducible release, current
freshness/nonce policy, and the tree's on-chain output before accepting a
service. This spike does not satisfy those later service checks.

## Decision and follow-up

- Use **TDX Confidential Space** for the first GCP signet service. Name
  Google's attestation service as a trusted party in client policy.
- Keep **raw TDX with measured boot and dm-verity** as an unqualified
  alternative. The marker experiment is not an image attestation.
- Move the **SNP service** to a separately qualified cloud/image route;
  current GCP Confidential Space SNP is unavailable. Do not substitute
  plain SEV for SNP. The vendor-diverse service and mainnet gates remain.
- #11790 implements key lifetime and image-bound restart recovery on the
  chosen TDX route; #11791 reproduces and publishes the image; #11792
  checks the real token, release, parameters, freshness, and on-chain
  output. #11789 can run first; #11788 tracks the SNP route separately.

## Checks, cleanup, and costs

`cargo test --locked -p oa-att` passes, including the two captured-token
checks. The standalone lab's tests and example build pass; formatting and
VM-script syntax checks pass. The fixture is checked at a recorded time
inside the token's validity window; it is not accepted as fresh today by
an online client.

All four final probe VMs and both initial Confidential Space attempts
were deleted. The post-cleanup query for instances labelled `issue=11787`
returns an empty list. They had two-hour automatic deletion limits.
Using the issue's approximate rates ($0.09/hour SNP, $0.22/hour TDX),
compute for this probe run is below $0.70 even if each attempted VM is
charged for its full elapsed time until deletion. This is an estimate,
not a billing export; short-lived 20-GB disks, logs, and registry storage
are additional. The research registry and narrowly scoped workload
service account remain for the next signet image experiment. No
production attestation host or payment rail was changed.

## Primary references

- [Google's measurement register contents](https://docs.cloud.google.com/confidential-computing/confidential-vm/docs/measurement-register-contents)
- [Confidential VM attestation](https://docs.cloud.google.com/confidential-computing/confidential-vm/docs/attestation)
- [Confidential Space token claims and nonce bounds](https://docs.cloud.google.com/confidential-computing/confidential-space/docs/reference/token-claims)
- [Confidential Space PKI tokens](https://docs.cloud.google.com/confidential-computing/confidential-space/docs/connect-external-resources)
- [Google's SNP report tool](https://github.com/google/go-sev-guest/tree/9c5dffcd8230247336e07c70804a31f6b39ab3c9)
- [Google's TDX quote tool](https://github.com/google/go-tdx-guest/tree/23b3eac9db39ba4e1df1426bacb3bfa3a7018e97)
