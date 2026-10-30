# Oct 30 demo runbook

The live page is <https://openagents.com/pylon>. Open it before sending
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

## Stop the retained nodes

Stop the adapter first if ending the whole demonstration. Then stop the
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
