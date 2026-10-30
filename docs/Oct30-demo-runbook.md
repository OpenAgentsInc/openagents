# Oct 30 demo runbook

## Current public entry points

The homepage at <https://openagents.com/> includes the Pylon HUD. The
canonical user/agent quickstart is
<https://openagents.com/docs/connect-a-pylon>, with its exact Markdown at
<https://openagents.com/docs/connect-a-pylon/index.md>.

`POST https://openagents.com/api/v1/systemone` is now live with existing
browser-session or body-bound NIP-98 authentication. Both `elephant-1` and
`mammoth-1` have passed real authenticated production calls. Follow the
quickstart's native client command; model ports are not public.

Both learned-model Pylons currently share **one RTX PRO 6000** on
`psionic-rtx6000-pylon` in `us-central1-b`. The old L4 was preempted twice
and remains stopped. These are two providers, not two independent GPUs.
Current keys, persistent paths, units and stop commands are in the
[cloud operator runbook](../psionic/docs/serving/cloud-pylons.md).
Before intentionally stopping that VM, use its
[managed recovery pause-and-drain procedure](../psionic/docs/serving/spot-recovery.md).
The [public edge proof](verification/psionic-public-edge-production-2026-10-30.md)
records actual responses, auth refusals and immutable deployment identities.

The recovered confidential Clef worker is a separate CPU NIP-ATT lane on
`oct30-att-tdx-clef` (Intel TDX / Google Confidential Space). Its
[fresh attestation and sealed-round proof](verification/2026-10-30-confidential-pylon.md)
does not change the ordinary API's trust boundary or claim a Coax NIP-ATT
client has shipped.

## Retained local walkthrough

The standalone page is <https://openagents.com/pylon>. Open it before sending
jobs: its counters show activity observed during that page session.
The [agent guide](https://openagents.com/pylon/index.md) has prerequisites
and troubleshooting.

## Join a computer

```sh
curl -fsSL https://openagents.com/pylon/join.sh | bash
```

Keep that terminal open. The script clones the source, builds the checked
Coax compiler when needed, builds the native toy engine and Pylon, creates
a private temporary identity, and prints its slug/npub after relay admission.
The measured native joins took 94.5 s on Mac and 137 s on Linux with matching
checked compiler caches; a cold compiler bootstrap takes additional time.
Bun is needed for the operator buyer, not for joining.

This is a free toy-model demonstration with throwaway identities. A serving
Pylon reads the request. Use synthetic prompts and keep the private demo
label file out of screenshots.

## Send jobs from the operator Mac

The retained checkout is `/tmp/oct30-pylon-demo`.

```sh
cd /tmp/oct30-pylon-demo
bun install --cwd psionic/tests/pylon --frozen-lockfile
bun psionic/tests/pylon/demo-buyer.ts --slug oct30-mac-demo --slug oct30-coderos-demo --json
```

Leave the public page open while this runs. The buyer prints the checked
model identity, answer and round-trip time. `--list` discovers nodes without
sending a job; `--all` sends to each eligible toy Pylon in its snapshot.
The two retained demo slugs are `oct30-mac-demo` and `oct30-coderos-demo`.

## Use the Chapter 20 HTTP adapter

The guarded native adapter is listening on the operator Mac:

```sh
curl -sS http://127.0.0.1:8798/v1/systemone \
  -H 'content-type: application/json' \
  -d '{"model":"psionic-toy","state":"Water is wet.","questions":{"wet":{"type":"choice","criteria":{"yes":null,"no":null}}}}'
```

`service.door` and `service.provider` identify the remote Pylon.
`service.identity` carries its exact checkpoint identity. The configured
local fallback must serve that same identity.

[Chapter 20](../psionic/book/ch020-the-api-fans-out.md) contains the source
build and configuration commands. The retained adapter is loopback-only;
its current launch/PID/stop details are in `/tmp/oct30-fleet-live/run.json`
and its log is `/tmp/oct30-fleet-live/fleet.log`. The guarded successor
started as PID 48404; its recorded stop command is `kill -TERM 48404`.

## Use Chapter 21 receipts and replay

A separate adapter is retained on `http://127.0.0.1:8799/v1/systemone`.
Send the same JSON request shown above to port 8799. It pins the original
Linux Pylon, publishes an accepted-work receipt, and selects every result
for an independent replay on the Mac. The configured toy CPU comparison
then publishes a signed pass/fail/inconclusive label. This remains free,
with throwaway keys and synthetic prompts.

Its current command, binary hashes, log and recorded stop target are in
`/tmp/oct30-mesh-public/run.json`; the retained PID is 38998. Check that
metadata before stopping a historical PID. Stop this adapter with
`kill -TERM 38998` before shutting down its Pylons. Port 8798 remains the
ordinary Chapter 20 adapter.

[Chapter 21](../psionic/book/ch021-check-the-mesh.md) gives the source build,
explicit trust flags, pool publication and independent verification commands.
The retained pool binary is `/tmp/oct30-pool-native-v4/psionic-mesh-pool`.
The public capacity proof is in `/tmp/oct30-mesh-public-capacity/`: exact
argv, signed public inputs, generated protobuf bundle and independent
recomputation. That snapshot has **2 Pylons, 8 total/free slots, 1 accepted job
and 1 check-pass**. Its aggregate ID is
`36669dc4fd9b01ffaa4e66e7df63cc9750404f97d0a59150987fe42bac25f459`.

Use a closed minute window containing both work and check, with explicit
provider/buyer/checker keys. Capture close to a minute boundary if current
capacity is part of the demonstration: relays can replace older addressable
beacons before a historical query reads them. Keep the public bundle for
stable offline verification. The aggregate expires after five minutes;
offline historical verification still reports `verified:true` with
`current:false`. A missing historical beacon does not show a machine was
offline. Recomputing a snapshot does not prove global relay completeness.

## Stop the retained nodes

Stop both HTTP adapters first if ending the whole demonstration. Then stop the
script-owned Pylon/engine pairs:

```sh
touch /tmp/oct30-pylon-mac-live/state/stop
ssh christopherdavid@100.74.238.61 'touch ~/.openagents/oct30-linux-demo/state/stop'
```

Normal joiners use Ctrl-C or `~/.openagents/pylon/state/stop`. The scripts
drain their Pylon, publish an offline beacon and stop their own engine.

## Evidence

[Oct30MVP results](Oct30MVP.md#4-results-log) records the public relay
proof, source/compiler/binary identities, both native joins, deployed
page/browser proof and the guarded Chapter 20 successor. Chapter 20's 419
authority mutants were all killed; prior and successor end-to-end records
retain their distinct source hashes.

Chapter 21 adds 1,268 killed authority mutants with zero survivors, real
Linux→Mac public replay, and independently recomputed signed pool/bundle
proofs. Exact policy inputs and scope are in
[the pool policy record](verification/psionic-mesh-pool-policy-2026-10-30.md)
and [the native mesh record](verification/psionic-mesh-native-2026-10-30.md).

The [independent end-to-end record](verification/2026-10-30-mesh-e2e.md)
retains the actual signed public capacity bundle and exact replay arguments.
