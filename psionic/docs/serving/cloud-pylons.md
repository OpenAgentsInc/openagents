# Cloud Pylon operations

For joining with your own computer, use the canonical
[Connect a Pylon guide](https://openagents.com/docs/connect-a-pylon).
This page describes the deployed Google Cloud model host; dated proof and
build fingerprints are in [the rollout record](../../../docs/verification/psionic-cloud-pylons-2026-10-30.md).

## Current host

Project `openagentsgemini`, VM `psionic-rtx6000-pylon`, zone `us-central1-b`:
one NVIDIA RTX PRO 6000 Blackwell Server Edition, 96 GB device memory,
176 GiB host RAM, `g4-standard-48`, 200 GB Hyperdisk. It runs five independent
Coax model servers and Pylons on the shared card. Combined measured device use
after adding original GPT-OSS20B was 80,783 MiB; after the two streaming
Elephant servers (2026-10-30 21:30 UTC) it is 96,070 MiB of 97,887 MiB, so the
card has no room for another model server.
The old L4 VM `psionic-l4-elephant` stays stopped after repeated preemption;
its earlier provider is not the current Elephant route.

| Model | Public provider key | Slug | Backend / Pylon systemd units |
| --- | --- | --- | --- |
| `elephant-1` | `59e443def0b5d7181fad586b2f88a15689d09c91fd3f44d46b98d7a890cf9ee7` | `oct30-gcloud-elephant-rtx` | `psionic-elephant-rtx` / `openagents-pylon-oct30-gcloud-elephant-rtx` |
| `mammoth-1` | `d27ece046e7622357f2385655fe2c1d7dcb634a51eed1a6e69ab23547e3ae829` | `oct30-gcloud-mammoth` | `psionic-mammoth` / `openagents-pylon-oct30-gcloud-mammoth` |
| `gpt-oss-20b` | `23a62947ce9913e8d61d166fa72a2e4d2e11b153f9c47c5ee37e583ec702f599` | `oct30-gcloud-gptoss20b` | `psionic-gptoss20b` / `openagents-pylon-oct30-gcloud-gptoss20b` |
| `elephant-1` | `71e23064bdeebf33d6601dd1b6a18353534359b737a546f198c94487eac0010a` | `oct30-gcloud-elephant-rtx-2` | `psionic-elephant-rtx-2` / `openagents-pylon-oct30-gcloud-elephant-rtx-2` |
| `elephant-1` | `d80386f7cebfb363b7bf05e75407baa3f773e5b248fa1dd3c2933573d1bf01d3` | `oct30-gcloud-elephant-rtx-3` | `psionic-elephant-rtx-3` / `openagents-pylon-oct30-gcloud-elephant-rtx-3` |

Exact identities:

```
elephant-1@sha256:682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba
mammoth-1@sha256:4570fecb199493c255d7cad9474941ec43609969b1697a5682be7d9ed4cb9019
gpt-oss-20b@sha256:3340a61d1a0391e8c5b5d3463d18d4c48129a84bbc04a554c762c99020aa06ed
```

Elephant and Mammoth use SystemOne decisions. GPT-OSS uses public NIP-90
text generation and the native pinned buyer described in the
[GPT-OSS operator runbook](gptoss-cloud-pylon.md).

These are declared/checkpoint-checked provider identities, not remote hardware
attestation. The authenticated public endpoint forwards only its configured
models through private native fleets and the relay. The model listeners
remain `127.0.0.1:8796` (Elephant), `127.0.0.1:8797` (Mammoth), and
`127.0.0.1:8795` (GPT-OSS), `127.0.0.1:8798` and `127.0.0.1:8799` (the two
streaming Elephants). Ingress is denied except IAP SSH. There are no
upstream provider API keys, service-account scopes or outside inference
services. GPT-OSS's local Responses bearer is retained in a private mode 0600
file and never enters public events or argv.

## Inspect and recover

The fixed host now has a keyless five-minute recovery controller. See
[managed Spot recovery](spot-recovery.md) for its tested runtime and minimal
roles. Before an intentional stop or capacity conversion, run its `pause`
command and wait for the dry/cancel/quiescent drain; settle any Compute start
already accepted before stopping. Resume only when automatic starts are wanted.

Use an authorized Google Cloud account and IAP; never open a model port:

```sh
gcloud compute instances describe psionic-rtx6000-pylon \
  --project=openagentsgemini --zone=us-central1-b \
  --format='table(name,status,scheduling.provisioningModel)'

gcloud compute instances start psionic-rtx6000-pylon \
  --project=openagentsgemini --zone=us-central1-b

gcloud compute ssh psionic-rtx6000-pylon --tunnel-through-iap \
  --project=openagentsgemini --zone=us-central1-b \
  --command='sudo journalctl -u psionic-mammoth -u psionic-elephant-rtx -u psionic-gptoss20b -u psionic-elephant-rtx-2 -u psionic-elephant-rtx-3 -n 30 --no-pager'
```

All ten plain-model units are enabled. A boot starts the backends and Pylons; the Pylons
retry while models authenticate/place and advertise only after their real
reference checks pass. First-ever Mammoth authentication reads/hashes the
54.7 GB originals and took about six minutes here. Retained verified-file
records reduce a process restart's authentication to about 148 ms; a cold
machine still has to read/upload the model. Check actual doctor/online
messages, not only systemd's `active` status. Capacity failure and warmup
remain unavailability, never a substituted toy answer.

The current machine is SPOT. STANDARD stockouts prevented both its conversion
and an adaptive L4 size. Bounded managed recovery may request a restart, but
cannot reserve nonexistent capacity or prevent preemption. Keep that limit
visible when assessing endpoint availability. Retrying STANDARD should clear
the old scheduling fields with `--no-preemptible` and
`--clear-instance-termination-action` while the VM is stopped; do not stop a
healthy host merely to probe capacity during a live demo.

## Persistent files and identity

All runtime dependencies survive reboot; nothing depends on `/tmp`:

- Model files: `/home/christopherdavid/models/elephant-1-b3f8a934` and
  `/home/christopherdavid/models/mammoth-1-61cec44c`.
- Native server/kernels: `/home/christopherdavid/build/mammoth-serve`.
  Current source/emission bundle: `/home/christopherdavid/mammoth-source`.
- Tokenizer table: `/home/christopherdavid/fixtures/qwen35.pst`.
- NVRTC 13.2.86: `/home/christopherdavid/venv/lib/python3.12/site-packages/nvidia/cu13/lib/libnvrtc.so.13`.
- Checkpoint receipts: `/home/christopherdavid/.cache/psionic/verified`.
- Pylon binary/references: `/opt/openagents/pylon/991b370452a04c16f10d7e646c6c53eacd97acd3`.
- TLS runtime: immutable Nix OpenSSL/glibc closure in `/nix/store`; system
  CA bundle `/etc/ssl/certs/ca-certificates.crt`.
- Per-provider state: `/var/lib/openagents-pylon/<slug>`.
- GPT-OSS's source, stable native executable/kernels and portable Pylon build
  context: `/home/christopherdavid/gptoss-pylon-source`, `gptoss-runtime`, and
  `gptoss-probe-context` respectively. Its original files are in the pinned
  `~/.cache/psionic/gptoss-original/20b/<revision>/original` directory.
- GPT-OSS's private label, bearer, identity and epoch:
  `/home/christopherdavid/.openagents/pylon-cloud/gptoss20b`.

The Elephant/Mammoth state directories belong to `openagents-pylon`;
GPT-OSS runs as `christopherdavid` and owns its separate private directory.
`demo-label` is private, mode 0600: do not print it, put it in argv, commit it
or clone it into another active provider. `identity` retains the checkpoint
identity; `identity.generation` retains the provider-bound public epoch, and
`identity.generation.lock` excludes concurrent ownership. Keep these together.
Do not reset/restore an older generation while reusing its label. The actual
signer still refuses production `--key-file` until its separate audit closes.

`psionic/platform/cloud-pylon-install.sh` installs a prebuilt source-recorded
Pylon and Nix runtime closure. It checks `SHA256SUMS`, preserves existing
labels, installs references and an optional source receipt, and updates only
the named service. Store files are skipped during updates; executables are
replaced atomically. Model arithmetic and all protocol/admission logic remain
Coax. Existing Python scripts are build tooling only; runtime units execute
native programs.

## Model and upgrade limits

The current servers admit 512 prepared tokens per decision. Elephant uses
`--elephant-gpu`, port 8796, device budget 20,000 MiB (14.79 GiB resident,
16,260 MiB measured). The two added Elephants use ports 8798/8799 with
`--device-mib 7000` each: 6.16 GiB resident plus 8.62 GiB pinned host memory
streamed per decision, 7,620 MiB measured each. Their six-case doctors take
about 1.5-1.7 s against 744 ms fully resident, and fresh relay tasks answered
in 387 ms and 369 ms with the same probabilities as the resident Elephant. A
CPU-only Elephant (`--elephant`) was tried on this host and refused: one
single-threaded decision exceeded the 30 s warmup bound, so it is not
deployed; Mammoth uses
`--mammoth-gpu`, port 8797, requested budget 90,000 MiB, capped by observed
free device memory. Both checkpoints are original pinned weights; Mammoth
uses the frozen manifest covering both shards. GPT-OSS is not advertised
through this SystemOne adapter.

Build with the usual `serve-build.sh ... release plain gpu` when a native
qualified compiler is available. For cross-emission, retain the generated
C/header/CUDA files and actual emitter hash, compile the existing C effects
with release floating-point flags, and regenerate `serve-build.json` on the
target. Do not execute a Mac compiler binary on Linux or invent a compiler
provenance receipt. The captured receipt includes the Mammoth inventory
header and native host sources.

Replace the executable by rename, preserving any healthy process's old inode.
Restart only the changed backend after checking its source; model/checkpoint
hashes remain fixed. A fresh startup doctor and one encrypted relay task
qualify deployment after resume/update. Do not rerun frozen CPU decisions or
full numerical benchmarks on unchanged arithmetic.
