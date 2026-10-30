# Six-hour GCloud Pylon watch

This is the operational work file for the active fleet goal. The goal remains
active until the complete required lineup is verified and has a six-hour
supervision record. Setup time does not replace the requested live window.

## Required lineup

| Role | GCloud host | Provider / state |
| --- | --- | --- |
| Elephant | `psionic-rtx6000-pylon`, `us-central1-b` | `59e443def0b5d7181fad586b2f88a15689d09c91fd3f44d46b98d7a890cf9ee7` |
| Mammoth | same RTX host | `d27ece046e7622357f2385655fe2c1d7dcb634a51eed1a6e69ab23547e3ae829` |
| GPT-OSS 20B | same RTX host | `23a62947ce9913e8d61d166fa72a2e4d2e11b153f9c47c5ee37e583ec702f599`; exact `gpt-oss-20b@sha256:3340a61d1a0391e8c5b5d3463d18d4c48129a84bbc04a554c762c99020aa06ed`, signed public `nip90-5050` text lane |
| Confidential Clef | `oct30-att-tdx-clef`, `us-central1-a` | Endpoint `eadd886db9e84b3fa069e84282fec1943388d84baf1e3ad534254b524ed0dfc0`; fresh vendor attestation and sealed receipt required |

The two learned decision providers and GPT-OSS share one physical GPU. The public site
currently also includes Mac providers; those do not count toward GCloud's
minimum. All four roles above must be satisfied; three existing providers
alone did not prove GPT-OSS; its real text round and website row are now verified.

## Window

- Goal creation: `2026-10-30T19:00:42Z`.
- Complete-model lineup verified: `2026-10-30T21:04:21Z`.
- Independent managed four-role probe passed: `2026-10-30T21:06:50Z`.
- Watch starts: **`2026-10-30T21:09:43Z` (4:09:43 PM Central)**.
- Watch ends: **`2026-10-31T03:09:43Z` (10:09:43 PM Central)**.
- The exact 21,600-second interval is fixed in
  [the create-once window receipt](../../fixtures/fleet-watch/2026-10-30/window.json).
  Activation was acknowledged before its start. Do not shift this window or
  claim that configuring it proves six hours have elapsed.
- Existing providers remain running and are probed during setup.
- Do not claim six hours merely because a timer was configured. Preserve
  timestamped observations, failures, recovery requests and subsequent real
  readiness. Do not fabricate coverage for unobserved time.

## Current verified state

At `2026-10-30T21:04:21Z`, actual production Coax state showed six live
providers, including exactly the four required GCloud providers across two
VMs. GPT-OSS, Mammoth, Elephant and Clef beacons were respectively 38, 3, 8
and 17 seconds old. The other two rows were Mac advertisements and do not
count toward GCloud's requirement. The website needed no code change.

GPT-OSS's first real provider-pinned NIP-90 round passed at `21:03:33Z`:
signed 5050 request, real native Responses generation, signed/correlated
6050 result, exact original checkpoint and `PONG` final text in 907 ms.
Both GPT-OSS units are enabled/active; total GPU use was 80,783 of 97,887 MiB.
This lane is public signed text, not encrypted input or paid work.

Fresh authenticated production calls returned the pinned Elephant and Mammoth
providers with real `yes` answers in 218 ms and 393 ms respectively. The
confidential oracle verified the Google chain, Intel TDX/STABLE measurement,
immutable image and nonce-bound endpoint, then a unique public-state sealed
round: 10,086 ms inference / 10,976 ms relay round. Repeating the old fixed
weather prompt hit its result cache, so subsequent probes must vary public
state to establish new inference.

Fresh unique-state endpoint probes at `20:56:57Z` also passed for Elephant
and Mammoth (130 ms and 471 ms). Operational probes are new observations;
unchanged compiler, model-reference and arithmetic gates are not repeated.

The managed watcher image is
`us-central1-docker.pkg.dev/openagentsgemini/openagents/pylon-fleet-watch@sha256:40c566373f3a74c2f34daab5ff3cf76de14bab977db8327cf741ebe1b803c736`.
Cloud Build `cbadc10b-6599-4654-876b-02867b323656` passed. Recovery policy killed
69/69 mutants; typed probe adapter killed 5/5. Native ASan/UBSan checks passed,
and six effect tests cover expiry, narrow recovery, dry mode, wrong-provider
HTTP200 and refusal to credit only three roles. The embedded GPT-OSS client
and confidential oracle were built from their qualified pinned components.
The actual Scheduler-created bootstrap execution `pylon-fleet-watch-2jw7g`
passed all four real probes. Its creator was the dedicated Scheduler service
account, not an operator bypass. It returned GPT-OSS PONG in 753 ms and a
fresh vendor-verified sealed round in 8,713 ms, as well as the two pinned
decision answers. The job was then activated with DRY0 and the exact window;
Scheduler ENABLED and its runtime image/config were independently read back.
See [activation and bootstrap receipts](../../fixtures/fleet-watch/2026-10-30/).

Runtime grants are scoped to GET on the two hosts, START on the TDX host,
dispatch of the existing fixed RTX recovery job, and invocation of the two
private model fleets. The Scheduler identity can invoke only this watcher.
No model port, Google key, arbitrary target, VM creation, stop/delete or
project-wide compute grant was added. Initial custom-role propagation was
retried with the same grant rather than widening it.

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
container `Always`; the bounded watcher can request a start for this exact
stopped VM. A start request remains distinct from model-ready or attested.
Confidential endpoint keys can rotate on a verified reboot; accept a new
nonce-bound key only through the unchanged publisher/image/release proof,
never by copying a private TEE key or trusting a replacement label.

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

At `03:09:43Z`, audit the full window and a final real probe. Then run the
documented pause/drain for `pylon-fleet-watch`, disable the thread heartbeat,
and preserve all execution/probe/recovery evidence. Leave the standing RTX
controller and existing workers intact. The native time fence stops new
watch actions after expiry even if the supervisory Mac is asleep.
