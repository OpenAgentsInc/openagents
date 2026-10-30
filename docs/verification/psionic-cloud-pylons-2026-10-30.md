# Native cloud Pylons, 2026-10-30

Two learned-model Coax Pylons are live through `wss://relay.openagents.com`:
Elephant and Mammoth, colocated on one Google Cloud RTX PRO 6000. Their
model HTTP listeners remain loopback only. Nothing routes inference through
an outside model service. The original L4 proof below is retained as history;
that VM was preempted twice and stays stopped.

Current operator instructions: [cloud Pylon runbook](../../psionic/docs/serving/cloud-pylons.md).

## Initial L4 qualification (retired after preemption)

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

This describes the first proof at 17:56–17:57 UTC. Google Cloud preempted
this VM at 18:03:18 UTC (stopped 18:03:33), and again at 18:17:55 (stopped
18:18:05) after recovery. The adaptive `g2-standard-8` STANDARD attempt also
hit stockout. Its original disk and identity remain; it is not an active
provider in the final rollout.

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

## Final colocated Elephant and Mammoth

One additional VM, `psionic-rtx6000-pylon` in `us-central1-b`, was created
for Mammoth: `g4-standard-48` SPOT, one NVIDIA RTX PRO 6000 Blackwell Server
Edition (97,887 MiB), 176 GiB host RAM and a 200 GB `hyperdisk-balanced`
boot disk. It has no service account or OAuth scopes. No autoscaling or
additional active GPU fleet was provisioned. STANDARD capacity was unavailable
for the RTX PRO and both tested L4 machine sizes. The final rollout retains
one running RTX PRO SPOT VM and the stopped L4 disk. SPOT can preempt;
bounded recovery does not guarantee reserved capacity. The Hyperdisk is
provisioned to 12,000 IOPS / 750 MiB/s. Compute and storage incur their normal
charges while retained.

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
not a newly claimed six-case Mammoth qualification.

| Final provider | Elephant | Mammoth |
| --- | --- | --- |
| Slug | `oct30-gcloud-elephant-rtx` | `oct30-gcloud-mammoth` |
| Public key | `59e443def0b5d7181fad586b2f88a15689d09c91fd3f44d46b98d7a890cf9ee7` | `d27ece046e7622357f2385655fe2c1d7dcb634a51eed1a6e69ab23547e3ae829` |
| Loopback backend | `127.0.0.1:8796` | `127.0.0.1:8797` |
| Doctor | 6 cases, 744 ms | 1 frozen CPU case, 266 ms |
| Largest error, billionths | 4,910 | 2,494 |
| Allowed error, billionths | 500,000 | 500,000 |
| Independent relay task | 269 ms | 374 ms |
| `yes` / `no` | `0.757197863` / `0.242802137` | `0.944614404` / `0.055385596` |
| First durable epoch | 2 | 2 |

Both passed NIP-42, advertised free online beacons and answered fresh
kind-25910 jobs with independently verified/decrypted kind-26910 results.
Captured results are `elephant-rtx-relay-result.json` and
`mammoth-relay-result.json` under `psionic/fixtures/cloud-pylons/2026-10-30/`.
No fallback or toy model answered either request. The colocated processes
use 66,590 of 97,887 MiB device memory: two logical providers on one card.

The real Mammoth rollout caught an adapter defect after original checkpoint
authentication: the host used Elephant's default kind-0 device placement.
Commit `730705ed80` selects `eg_place_kind` with the authenticated model kind.
The actual failure was the red check; its doctor and encrypted task above
are the green checks. Generated Coax/CUDA arithmetic did not change. The two
verified-file records let the corrected process authenticate in 148 ms;
it placed 47.73 GiB fully resident, with zero streaming, in 2,463 ms.

Patched native Linux Mammoth executable SHA-256:
`c9fae481f38a30dc410731c166e579b4cf2d9265bb4b3c515ff709fc11e30959`.
Its build ID is
`b610ee3bea2f1b30a9a9bdb8d5348d214505ea16e2ddbd7b0e27e40d3ba759c3`;
the full source/tool receipt is `mammoth-linux-serve-build.json` in that fixture
directory. GCC is Ubuntu 13.3.0; the emitter is the qualified Mac Coax binary,
hashed as metadata and never executed on Linux. The existing healthy Elephant
process retains its pre-fix kind-0 executable inode
`d46eb50cda522e851c235ed2e18448dd6b3e15249727f874fd0336f3684ee717`
(build `17293c1b35aae34215ae28a5aab0245839c023f0008975819d26237d723fa6c8`).
The shared executable path was replaced atomically; Elephant was not restarted.

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

The active Pylons use the generation-safe source `991b370452`, emitted on
the same qualified Mac compiler, compiled with Ubuntu GCC 13.3 and Nix
OpenSSL 3.6.3/glibc 2.42. Native binary SHA-256:
`dc17b0930bdc059602ddbc7bc87aed5dbfd57d37631169e5a9c7d0775a3cf48f`.
Its full receipt is `pylon-generation-build.json` beside the first receipt.
Both state directories are under `/var/lib/openagents-pylon/<slug>`; the
private `demo-label` and public `identity.generation` are mode 0600 and owned
by `openagents-pylon`. The epoch is reserved before any beacon, and survives
restart without relying on the wall clock. The installer skips existing
immutable Nix store files instead of truncating mapped shared libraries.

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
