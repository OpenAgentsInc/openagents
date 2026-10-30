# Cloud Pylon operations

For joining with your own computer, use the canonical
[Connect a Pylon guide](https://openagents.com/docs/connect-a-pylon).
This page describes the deployed Google Cloud model host; dated proof and
build fingerprints are in [the rollout record](../../../docs/verification/psionic-cloud-pylons-2026-10-30.md).

## Current host

Project `openagentsgemini`, VM `psionic-rtx6000-pylon`, zone `us-central1-b`:
one NVIDIA RTX PRO 6000 Blackwell Server Edition, 96 GB device memory,
176 GiB host RAM, `g4-standard-48`, 200 GB Hyperdisk. It runs two independent
Coax model servers and Pylons. Combined measured device use is 66,590 MiB.
The old L4 VM `psionic-l4-elephant` stays stopped after repeated preemption;
its earlier provider is not the current Elephant route.

| Model | Public provider key | Slug | Backend / Pylon systemd units |
| --- | --- | --- | --- |
| `elephant-1` | `59e443def0b5d7181fad586b2f88a15689d09c91fd3f44d46b98d7a890cf9ee7` | `oct30-gcloud-elephant-rtx` | `psionic-elephant-rtx` / `openagents-pylon-oct30-gcloud-elephant-rtx` |
| `mammoth-1` | `d27ece046e7622357f2385655fe2c1d7dcb634a51eed1a6e69ab23547e3ae829` | `oct30-gcloud-mammoth` | `psionic-mammoth` / `openagents-pylon-oct30-gcloud-mammoth` |

Exact identities:

```
elephant-1@sha256:682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba
mammoth-1@sha256:4570fecb199493c255d7cad9474941ec43609969b1697a5682be7d9ed4cb9019
```

These are declared/checkpoint-checked provider identities, not remote hardware
attestation. The authenticated public endpoint forwards only its configured
models through private native fleets and the relay. The model listeners
remain `127.0.0.1:8796` (Elephant) and `127.0.0.1:8797` (Mammoth).
Ingress is denied except IAP SSH. There are no API keys on the model VM,
no service account/scopes, and no outside inference service.

## Inspect and recover

Use an authorized Google Cloud account and IAP; never open a model port:

```sh
gcloud compute instances describe psionic-rtx6000-pylon \
  --project=openagentsgemini --zone=us-central1-b \
  --format='table(name,status,scheduling.provisioningModel)'

gcloud compute instances start psionic-rtx6000-pylon \
  --project=openagentsgemini --zone=us-central1-b

gcloud compute ssh psionic-rtx6000-pylon --tunnel-through-iap \
  --project=openagentsgemini --zone=us-central1-b \
  --command='sudo journalctl -u psionic-mammoth -u psionic-elephant-rtx -n 30 --no-pager'
```

All four units are enabled. A boot starts the backends and Pylons; the Pylons
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

Each state directory belongs to the unprivileged `openagents-pylon` account.
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
`--elephant-gpu`, port 8796, device budget 20,000 MiB; Mammoth uses
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
