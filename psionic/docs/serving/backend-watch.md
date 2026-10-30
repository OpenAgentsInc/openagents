# Local backend readiness recovery

`openagents-backend-watch.timer` supervises exactly three existing services on
`psionic-rtx6000-pylon`. Coax owns the registry, window, readiness interpretation,
warmup, failure streak and restart decision. C supplies clock/file/argv effects;
the shell collects loopback/systemd observations and dispatches admitted actions.
It reads no model files, API keys, Pylon identities or signing credentials.

| Unit | Loopback readiness | Ready response |
| --- | --- | --- |
| `psionic-elephant-rtx.service` | `127.0.0.1:8796/readyz` | `psionic-health-v1`, `ready:true` |
| `psionic-mammoth.service` | `127.0.0.1:8797/readyz` | `psionic-health-v1`, `ready:true` |
| `psionic-gptoss20b.service` | `127.0.0.1:8795/readyz` | `{"status":"ok"}` |

The shared `fw_window` requires an explicit six-hour start/end pair. The installed
window is **2026-10-30 21:09:43 UTC through 2026-10-31 03:09:43 UTC**
(`1793394583` through `1793416183`). The installer refuses to reset a different
existing window. Outside it, the runner performs no backend observation or
restart. The timer may remain enabled after expiry; its Coax fence stays closed.

## Recovery conditions

A process receives at least **15 minutes** from its systemd monotonic start time
before a failure can count. With no recorded process birth, the grace begins
when the watcher first sees that incarnation. Three consecutive explicit
unready/unreachable observations, each at least **120 seconds** apart, are
required. Healthy, busy or indeterminate observations clear the streak. A boot,
InvocationID or process-start change also resets it; monotonic rollback within
one incarnation fails closed.

HTTP `503` and unavailable transport (`000`) count as failures. HTTP `429` is
busy and clears the streak. This matters because Elephant and Mammoth return
busy to every connection accepted during a GPU decision, including `/readyz`.
Other HTTP errors, malformed/unknown HTTP200 schemas and auth/route errors are
indeterminate and cannot trigger a restart. The GPT readiness handler reads an
atomic health flag in a separate connection thread, outside its generation lock.

Before a planned restart, the runner re-reads both process identity and readiness
against the unchanged held state. A recovered or busy backend cancels that plan.
It then persists state and checks the wall-clock window again immediately before
`systemctl restart --job-mode=fail` on the exact Coax-selected unit. A flock
prevents overlapping local runs. The oneshot has a 90-second systemd bound; its
individual metadata, HTTP and restart commands also have short deadlines.

A ready response does not attest checkpoint bytes, numerical correctness or
hardware. The independent managed mesh probes cover actual model requests and
the separate confidential oracle verifies its own sealed lane. This local
watcher neither reboots the VM nor resets the GPU, changes model/Pylon units,
creates resources, alters IAM or accepts arbitrary unit names. Device faults
that survive a process restart still need operator investigation.

## Build and install

```sh
psionic/platform/backend-watch-build.sh /tmp/backend-watch
sudo psionic/platform/backend-watch-install.sh \
  /tmp/backend-watch/backend-watch 1793394583 1793416183 --enable
```

Build on the target, or emit portable C/header with the qualified Coax compiler
and compile those with `backend-watch-face.c` on Linux. The runtime needs Bash,
`jq`, `curl`, `flock`, `timeout` and `systemctl`, with no compiler, Python, Rust,
cloud SDK or SSH credential. Its service is root-owned and restricted to local
networking and its state directory. Existing backends remain in their own units.

Configuration lives at `/etc/openagents-backend-watch.env`; the binary/runner
are under `/opt/openagents-backend-watch`; state and lock are under
`/var/lib/openagents-backend-watch`.

## Maintenance and observation

Pause and drain this local watcher before intentionally stopping or upgrading a
backend. Stopping only the timer does not stop an already-running oneshot:

```sh
sudo systemctl stop openagents-backend-watch.timer openagents-backend-watch.service
# Perform the intended maintenance.
sudo systemctl start openagents-backend-watch.timer
```

Restarting the timer uses the same existing window. For intentional VM shutdown,
also pause the separate cloud VM recovery controller described in
[cloud-pylons.md](cloud-pylons.md); this local service cannot pause that controller.

```sh
systemctl status openagents-backend-watch.timer
journalctl -u openagents-backend-watch.service --no-pager -o cat
```

Each admitted observation reports `valid`, `restart`, the exact unit and its next
state. Reason codes are `0` healthy, `1` busy, `2` warmup/indeterminate, `3` spacing,
`4` counted failure, `5` restart. Discard the reason when `valid` is false.

[Qualification and the public installation receipt](../../../docs/verification/2026-10-30-backend-watch.md)
record the focused checks and unchanged backend invocation identities.
