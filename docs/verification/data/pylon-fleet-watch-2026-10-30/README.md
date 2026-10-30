# Managed watch and production Pylons: October 30, 2026

The four Google Cloud roles are live: Elephant, Mammoth and GPT-OSS share
`psionic-rtx6000-pylon` (one 96 GB RTX PRO); confidential Clef runs on the
separate Intel TDX CPU VM `oct30-att-tdx-clef`. This is **four Pylons on two
physical cloud hosts**. Two additional Mac advertisements make the production
website's six live providers. Per-Pylon advertised slots/memory are claims,
not additional physical hardware.

## Production website

`production-browser.json` contains two actual production Coax-state reads,
20 seconds apart, and the original signed public beacon frames. The driver
asserted the compiled ABI layout before reading, waited for an actual submitted
WebGPU frame, and opened PYLONS by ordinary pointer input. It changed no app
data and used a muted headless browser. `production-browser.png` is the actual
rendered panel; GPT-OSS is visible with its real model identity.

The final observation at **21:04:21 UTC** had no browser errors, six online
providers and 19 advertised free slots. The signed cloud rows were:

| Role | Provider | Age / lease remaining | Model |
| --- | --- | --- | --- |
| Elephant | `59e443def0b5d7181fad586b2f88a15689d09c91fd3f44d46b98d7a890cf9ee7` | 8 / 172 seconds | `elephant-1@sha256:682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba` |
| Mammoth | `d27ece046e7622357f2385655fe2c1d7dcb634a51eed1a6e69ab23547e3ae829` | 3 / 177 seconds | `mammoth-1@sha256:4570fecb199493c255d7cad9474941ec43609969b1697a5682be7d9ed4cb9019` |
| GPT-OSS | `23a62947ce9913e8d61d166fa72a2e4d2e11b153f9c47c5ee37e583ec702f599` | 38 / 142 seconds | `gpt-oss-20b@sha256:3340a61d1a0391e8c5b5d3463d18d4c48129a84bbc04a554c762c99020aa06ed` |
| Confidential Clef | `eadd886db9e84b3fa069e84282fec1943388d84baf1e3ad534254b524ed0dfc0` | 17 / 223 seconds | `clef-flash@sha256:fd3e90605e8103307dca37cb5a8cdb036267e2fe3cb2d908d80a8ceb9ec0638c` |

GPT-OSS appears as `GPT-OSS20B CLOUD`, `oct30-gcloud-gptoss20b`, generation 2,
lane `nip90-5050`. The existing generic beacon projection displayed it without
a frontend deployment. The confidential row's label is a signed claim; the
browser does not turn it into a vendor-verified attestation badge. Separate
sealed verification below establishes that claim for the test.

`beacons` in the browser record is a received-event counter (six initial events,
then renewals), not a count of distinct machines. The homepage's answer counters
observe decision kinds 26910/27010; they do not measure GPT-OSS kind-6050 replies.
An online beacon alone is not evidence of a successful inference.

## Actual Scheduler and four-model preflight

Image:

```
us-central1-docker.pkg.dev/openagentsgemini/openagents/pylon-fleet-watch@sha256:40c566373f3a74c2f34daab5ff3cf76de14bab977db8327cf741ebe1b803c736
```

The root owner qualified this image with Cloud Build
`cbadc10b-6599-4654-876b-02867b323656`. Deployment wrapper source was
`fceed8a7c0`; its 14 focused effect checks passed 47 assertions. No unchanged
compiler/model gate was rerun for this infrastructure deployment.

The initial role creation briefly hit IAM propagation at the first Compute
binding; repeating exactly the same resource-bound grants succeeded. `iam.json`
records the actual three one-permission role definitions and bindings filtered
to the two new principals. Neither has a direct project-level binding. This is
not an inherited organization/effective-access audit.

`scheduler-preflight.json` records actual execution
**`pylon-fleet-watch-2jw7g`**, created by
`pylon-watch-scheduler@openagentsgemini.iam.gserviceaccount.com`, with one
successful task and no retry. It ran dry, observed both VMs RUNNING and performed
all four real probes. `preflight-probes.json` preserves the runtime output:

- Elephant: exact provider/checkpoint, `yes`, reported inference 333 ms;
- Mammoth: exact provider/checkpoint, `yes`, reported inference 642 ms;
- GPT-OSS: signed, correlated NIP-90 `PONG`, 753 ms;
- Confidential Clef: fresh vendor/workload verification and sealed receipt,
  CPU round 8,713 ms, inference 8,614 ms;
- the combined four-role cycle reported `ok:true` at **21:06:50 UTC**.

The verification command restored the initially paused Scheduler. It did not
stop a healthy VM to exercise a recovery branch. Start/dispatch decisions are
separately checked by Coax and the bounded effect tests.

## Explicit six-hour activation

The demo owner selected exactly:

```
START 1793394583 = 2026-10-30T21:09:43Z
END   1793416183 = 2026-10-31T03:09:43Z
```

Activation was read back by **21:09:05 UTC**, before START. `active-job.json`
asserts the exact window, real GPT-OSS provider, immutable image and dry flag 0;
`active-scheduler.json` records ENABLED and the five-minute OAuth schedule.
No earlier controller settings changed. The interval is six hours; this setup
record does not claim that the future six hours have already passed.

The managed watcher remains active through that window. Use the
[deployment/maintenance runbook](../../../../psionic/docs/serving/fleet-watch-deployment.md)
to pause and drain it before maintenance; GPU maintenance also requires the
older Spot recovery controller to be paused. The window policy refuses recovery
after END; pause the Scheduler then to avoid idle invocations. Healthy Pylons
are not stopped by expiry.
