# Native cloud Pylons, 2026-10-30

The existing Google Cloud Elephant backend now serves a native Coax Pylon
through `wss://relay.openagents.com`. Its model HTTP listener remains on
`127.0.0.1:8797`. Nothing routes inference through an outside model service.

## Running Elephant

| Item | Value |
| --- | --- |
| Project / VM | `openagentsgemini` / `psionic-l4-elephant`, `us-central1-a` |
| Hardware | Existing `g2-standard-4` SPOT, one NVIDIA L4, 23,034 MiB GPU memory |
| Model | `baselabs/elephant-1`, revision `b3f8a93462fb1051a9de031e94a7ff380e254072` |
| Decision identity | `elephant-1@sha256:682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba` |
| Pylon slug | `oct30-gcloud-elephant` |
| Public key | `335ddc5f2b3c1af33cadf07bfde4430497ecefa161b34d5b94c14246b797b6b4` |
| Service | `openagents-pylon-oct30-gcloud-elephant.service` |
| Backend | Existing `psionic-elephant.service`, not replaced or restarted |

The six cases in `psionic/fixtures/pylon/elephant-1.selfcheck` passed in
2,767 ms, with zero billionths error against their frozen GPU answers.
The Pylon then authenticated with NIP-42 and advertised an online, free
four-slot beacon. A fresh independent buyer completed a real relay task:

```sh
bun psionic/tests/pylon/demo-buyer.ts --model elephant-1 \
  --slug oct30-gcloud-elephant --discover 4 --timeout 45 --json
```

The relay accepted its kind-25910 job; the buyer verified and opened the
kind-26910 result, including its exact provider, request, attempt and model
identity. Answer: `yes`, probabilities `yes=0.757202773`, `no=0.242797227`,
615 ms from publishing to result. This proves the learned-model Pylon path;
the separately deployed authenticated HTTP gateway has its own checks.

## Mammoth rollout

One additional VM, `psionic-rtx6000-pylon` in `us-central1-b`, was created
for Mammoth: `g4-standard-48` SPOT, one NVIDIA RTX PRO 6000 Blackwell Server
Edition (97,887 MiB), 176 GiB host RAM and a 200 GB `hyperdisk-balanced`
boot disk. It has no service account or OAuth scopes. No autoscaling or
additional GPU fleet was provisioned. The existing L4 and this one RTX PRO
VM incur their normal compute/disk charges while retained; SPOT can preempt.

This reuses the machine/model combination already qualified in
[the RTX PRO record](psionic-rtx6000-mammoth-2026-10-30.md). Its exact
original model is `baselabs/mammoth-1` revision
`61cec44c281c693992e3d6ebeb44dd3402dfa186`; the model identity is
`mammoth-1@sha256:4570fecb199493c255d7cad9474941ec43609969b1697a5682be7d9ed4cb9019`.
That fingerprint authenticates the complete frozen manifest, including
both original shard hashes, rather than only the index filename.

`psionic/fixtures/pylon/mammoth-1.selfcheck` adds the existing chapter-11
CPU wet-question golden in billionths, at the same frozen 0.0005 tolerance.
The independent selfcheck parser admits it. This is one pre-existing golden,
not a newly claimed six-case Mammoth qualification. Live service admission
and inference evidence will be appended after the native Mammoth serving
adapter is integrated.

## Deployment and provenance

`psionic/platform/cloud-pylon-install.sh` installs an already built native
Pylon, its Nix runtime closure and reference set as a dedicated unprivileged
systemd service. Coax still owns relay framing, NIP-42, signing, NIP-44,
beacon and task validation; this shell script does filesystem/lifecycle
setup only. It verifies artifact hashes, preserves a private identity across
reinstalls, uses the system CA bundle, enables restart-on-failure and grants
the process writes only to its own state directory.

The Pylon C and entropy header were emitted on the Mac from repository
`5bcb91c5af9f50d5a9ebf7a3323e9124733cd7c7` using the existing qualified
Coax compiler; native C compilation ran on Linux with GCC 15.2.0 and Nix
OpenSSL 3.6.3, using the flags in `pylon-build.sh` without `--test-trust`.
The same Nix closure is unpacked on the Ubuntu model host; installing a
package manager there is unnecessary. The machine/build fingerprints are in
`psionic/fixtures/cloud-pylons/2026-10-30/pylon-build.json`.

The new RTX VM uses Ubuntu image
`ubuntu-accelerator-2404-amd64-with-nvidia-595-v20261021`, driver 595 and
NVRTC 13.2.86. Tags `psionic-cloud-pylon` deny all IPv4 ingress at priority
900 except IAP SSH from `35.235.240.0/20` at priority 800. The existing L4
retains its equivalent `psionic-l4` firewall rules. Pylons make outbound TLS
relay connections; their model listeners are never public. GCloud IAP SCP
uses `--scp-flag=-O` here because default SFTP stalled; SSH host keys remain
verified normally.

Checks for this deployment change: shell syntax/argument usage, artifact hash
verification on the actual host, private label-file ownership/mode, native
doctor, relay authentication/advertisement and the independent encrypted
task round trip. No unchanged compiler/model suite was rerun.

## Identity and trust limit

The existing signer deliberately refuses production `--key-file` pending
the source-model audit in #4828. These are fresh throwaway demo identities,
with random labels in owner-only mode-0600 files, never command arguments
or committed data. Hosting the demo in production does not lift that gate.
Signed beacons prove who advertised an identity; they do not remotely attest
that hardware or model bytes. Payments, confidential attestation and
production signing-key approval are outside this rollout.
