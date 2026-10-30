# Six-hour GCloud Pylon watch

This is the operational work file for the active fleet goal. The goal remains
active until the complete required lineup is verified and has a six-hour
supervision record. Setup time does not replace the requested live window.

## Required lineup

| Role | GCloud host | Provider / state |
| --- | --- | --- |
| Elephant | `psionic-rtx6000-pylon`, `us-central1-b` | `59e443def0b5d7181fad586b2f88a15689d09c91fd3f44d46b98d7a890cf9ee7` |
| Mammoth | same RTX host | `d27ece046e7622357f2385655fe2c1d7dcb634a51eed1a6e69ab23547e3ae829` |
| GPT-OSS 20B | same RTX host, integration in progress | Exact model `gpt-oss-20b@sha256:3340a61d1a0391e8c5b5d3463d18d4c48129a84bbc04a554c762c99020aa06ed`; truthful NIP-90 text lane, no decision-model substitution |
| Confidential Clef | `oct30-att-tdx-clef`, `us-central1-a` | Endpoint `eadd886db9e84b3fa069e84282fec1943388d84baf1e3ad534254b524ed0dfc0`; fresh vendor attestation and sealed receipt required |

The two learned decision providers share one physical GPU. The public site
currently also includes Mac providers; those do not count toward GCloud's
minimum. All four roles above must be satisfied; three existing providers
alone do not prove the missing GPT-OSS integration.

## Window

- Goal creation: `2026-10-30T19:00:42Z`.
- Complete-fleet ready time: **pending GPT-OSS end-to-end verification**.
- Six-hour deadline: ready time plus exactly 21,600 seconds, recorded once
  in the immutable window receipt after the full lineup is verified.
- Existing providers remain running and are probed during setup.
- Do not claim six hours merely because a timer was configured. Preserve
  timestamped observations, failures, recovery requests and subsequent real
  readiness. Do not fabricate coverage for unobserved time.

## Current verified state

At `2026-10-30T20:19:18Z`, actual production Coax state showed five live
providers, including exactly the three GCloud providers above across two VMs.
Elephant and Mammoth beacons were 24 seconds old; confidential Clef was 26
seconds old and renewed during observation. GPT-OSS was not yet advertised.

Fresh authenticated production calls returned the pinned Elephant and Mammoth
providers with real `yes` answers in 218 ms and 393 ms respectively. The
confidential oracle verified the Google chain, Intel TDX/STABLE measurement,
immutable image and nonce-bound endpoint, then a unique public-state sealed
round: 10,086 ms inference / 10,976 ms relay round. Repeating the old fixed
weather prompt hit its result cache, so subsequent probes must vary public
state to establish new inference.

## Recovery and probe ownership

- Root owns the bounded Coax control policy, managed fleet monitor, scoped
  IAM and this work file/window record.
- `cloud_pylon_fleet` owns the real GPT-OSS native Responses engine, persistent
  systemd services, NIP-90 Pylon/client integration and model proof.
- `mesh_gateway_integration` owns the optional unchanged legacy confidential
  oracle packaging and bounded sealed-probe wrapper.
- `connect_pylon_guide` verifies admitted production website state and the
  model/provider/freshness projections. No website deployment is needed for
  the existing `nip90-5050` lane or confidential beacon schema.

The existing `pylon-spot-recovery` Cloud Scheduler is enabled every five
minutes and covers only the fixed RTX VM. Its latest observed executions
completed successfully through `20:11:51Z`; it is VM lifecycle recovery,
not model-health verification. Preserve that controller and its pause/dry/
drain procedure. The confidential VM is Standard with automatic restart and
container `Always`; a stopped-VM recovery path still needs explicit coverage.

A thread heartbeat, `six-hour-gcloud-pylon-fleet-watch`, supplies supervisory
follow-up. Managed inference/attestation probes and recovery must survive the
Mac/app sleeping. Do not rely on a heartbeat, an `active` service, a signed
advertisement or a RUNNING VM alone as proof of inference/attestation.

Probe every five minutes with bounded requests. Preserve provider labels,
durable generation counters and model hashes. New services use separate
persistent state. Never reset labels/counters, expose model ports, bypass the
production signing-key audit gate, send sats or restart an unrelated VM.
Do not stop healthy workers to manufacture a preemption test. At the end,
disable only this watch's follow-up/monitoring, leave existing providers
intact, and report the observed window and any gaps honestly.
