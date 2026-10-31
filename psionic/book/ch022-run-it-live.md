# 22. Run it live

The first deployment stays deliberately small: one authenticated Elephant,
one process owning its device context, one decision at a time, on an owned
host's loopback address. JSON and protobuf retain the
[chapter 20 contract](ch020-serve-systemone.md). Public ingress, TLS, access
control and spending decisions require an owner choice; they are not implied
by running the private service.

## A process is not ready just because it exists

The [Coax lifecycle](../lib/serve-run.coax) requires authentication, disjoint
sealed buffers and three successful seeded golden decisions. Every warm-up
uses the retained `Water is wet.` request. The toy has a separately retained
answer; original Elephant keeps chapter 44's frozen 0.0005 probability ceiling
against the recorded CPU result. A candidate cannot choose its own tolerance
or learn a new expected answer from its output.

Warm-up defaults to 30 seconds per seed, independently of request deadlines;
`--warmup-ms` explicitly selects a different bounded startup budget.
CPU warm-up uses bounded child processes; device warm-up checks the deadline
between layers. A failed or timed-out golden exits before the decision listener
opens. The three repeats warm the execution path; they are not a new quality
corpus or resolution of chapter 17's separate model-conformance gates.

| Probe | Meaning |
| --- | --- |
| `GET /healthz` | The live loopback host responds with `psionic-health-v1` metadata |
| `GET /readyz` | The same metadata, HTTP 200 only after the readiness gate, otherwise 503 |
| `checkpoint` | The authenticated checkpoint digest |
| `build` | Identity of emitted Coax C, native sources, kernels and producer/tool configuration |

Before initialization completes, no public listener exists. Busy decisions
retain the single-slot service's refusal behavior. A probe at an occupied slot may receive the ordinary busy refusal.
Readiness does not promise
an available slot, a latency budget or model quality. Shutdown stops admission;
CPU workers are joined/killed and device work stops at its next safe boundary.

## Supervise failures with a bounded policy

The [supervisor](../platform/serve-supervisor.c) only forks, waits, signals,
executes a trusted local binary and sleeps. Coax decides whether a failure may
restart and its exponential backoff. The default is at most five restarts,
starting at 250 ms and capped at ten seconds. Clean exits and an owner stop do
not restart; a crash loop exhausts its budget instead of running forever.

Device safe points also refuse pending connections while computation runs;
up to sixteen already-refused sockets may drain within a bounded cleanup
window. No decision job waits in that cleanup pool.

The [operational checks](../tests/service_live.py) kill an owned test child,
observe a new child warm and become ready, then verify a clean supervisor stop.
They also prove a bad golden never publishes a listener and that repeated
failures stop at the declared retry limit. Signal handlers deliberately avoid
restarting the blocking wait, so SIGTERM reaches a waiting supervisor's child.

```shell
PSIONIC_SUPERVISE=1 psionic/platform/serve.sh --toy --port 8798
```

## Install one immutable build under the user manager

On the qualified Linux CUDA host, build into a fresh directory, then install
the concrete reviewed artifacts:

```shell
psionic/platform/serve-build.sh /absolute/build release plain gpu
psionic/platform/install-systemone.sh /absolute/build \
  /absolute/models/elephant-1-b3f8a934 /absolute/qwen35.pst 8798 1024
systemctl --user status psionic-systemone.service
curl --fail http://127.0.0.1:8798/readyz
```

The installer keeps versioned binaries/kernels and their build record in the
user's data directory, verifies the staged unit before replacing its managed
definition, and starts it through the Coax supervisor. It never overwrites an
unrecognized unit. `Type=notify` becomes active only after the warmed child
binds and sends its optional native Unix-datagram readiness notification.
There is no systemd library or Python runtime dependency, no root installation
and no automatic boot enablement.

The owned RTX 4080 deployment passed readiness and the original golden answer
on port 8798. Killing its child with SIGKILL caused the same supervisor to
start a replacement, authenticate, place, warm and return ready in 13.735
seconds; its answer was identical. The
[managed-service check](../tests/service_managed.py) records both child IDs,
the probe identities and the journal. The unit was then stopped cleanly to
release the shared GPU; it is installed but not boot-enabled. Start it with
`systemctl --user start psionic-systemone.service` when a serving window is
available. This is a checked private deployment, not a public availability
claim.

Promotion means passing the retained golden gate before an endpoint becomes
ready. This first service does not implement zero-downtime rolling replacement,
canaries or availability during restart; those belong to the later serving
part. Keep the prior immutable build for an explicit rollback. The model's
own served identity remains in every decision answer.

## Account without keeping private conversations

`--metrics FILE` appends private `psionic-decision-metrics-v1` JSON records:
build/checkpoint identity, outcome, observed durations, token counts and prefix
reuse. Startup, answers, refusals, busy requests and cancellation are distinct.
There are no states, question IDs, option text, probabilities or answer bodies.
Coax also maintains bounded per-process usage counters. HTTP `usage` stays
`null`; these are operational records for future metering, not signed receipts
or payment authorization.

The [example](../examples/live-service.coax) exercises the policy without
starting a process:

<!-- source: psionic/examples/live-service.coax example -->
```jai
main :: () {
    life:=LsLife.{authenticated=true,sealed=true};print("Ready before warmup: %\n",ls_ready(life));
    for i:0..<3 {life=ls_warmed(life,true);}print("Ready after three retained golden decisions: %\n",ls_ready(life));
    next:=ls_restart(1,5,250,137,false);print("Crash: restart %, backoff % ms\n",next.retry,next.delay_ms);
    #assert !ls_promote(true,"expected-checkpoint","different-checkpoint",true);
    print("A different checkpoint cannot pass promotion.\n");
}
```

The [verification record](../../docs/verification/psionic-partIII-2026-10-29.md)
names the actual host/address, managed build, probes, restart check and first
P50/P95/P99 observations. Continue with
[measured improvements](ch023-make-the-numbers-move.md).
