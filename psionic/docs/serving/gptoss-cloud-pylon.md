# GPT-OSS20B cloud Pylon

The original 20B model runs through Psionic's Coax CUDA runtime on the existing
`psionic-rtx6000-pylon` VM in `openagentsgemini/us-central1-b`. It shares the
96 GB RTX PRO with Elephant and Mammoth; no extra GPU or VM was allocated.
The native runtime retains its 16 GiB budget. Actual combined use is 80,783 MiB.

The [original inventory](../../fixtures/gptoss/original/pins.json) pins three
files at revision `6cee5e81ee83917806bbde320786a8fb61efebee`, including the
13,761,300,984-byte original MXFP4 checkpoint. Coax authenticates the complete
original bytes, tensor descriptors and Harmony table before the listener opens.
The runtime executes the model and owns sampling/Harmony; it uses no external
inference service or downloaded inference runtime.

| Item | Value |
| --- | --- |
| Backend unit | `psionic-gptoss20b` |
| Pylon unit | `openagents-pylon-oct30-gcloud-gptoss20b` |
| Private backend | `127.0.0.1:8795/v1/responses` |
| Readiness | `GET /readyz`: 200 `{"status":"ok"}`, 503 if the worker is poisoned |
| Provider | `23a62947ce9913e8d61d166fa72a2e4d2e11b153f9c47c5ee37e583ec702f599` |
| Slug | `oct30-gcloud-gptoss20b` |
| Model | `gpt-oss-20b@sha256:3340a61d1a0391e8c5b5d3463d18d4c48129a84bbc04a554c762c99020aa06ed` |
| Lane | `nip90-5050`, free public text |

The model server is the existing `psionic/serve-gptoss.sh` native build,
copied into a stable deployment directory after compilation. The new Pylon
uses `--nip90`, the private `--engine-key-file`, its own retained private demo
label, and a provider-bound durable epoch. A real completed PONG doctor must
pass before advertising. The production signer audit gate is unchanged.

The backend is enabled with `Restart=on-failure`; the Pylon is enabled with
`Restart=always`, 10 second delays, and `PartOf=psionic-gptoss20b.service`.
A targeted backend restart therefore also rechecks/restarts its Pylon. Existing
VM recovery covers this same instance. The goal's separate readiness timer and
managed watcher own repeated-failure recovery and the six-hour watch window.

## Native monitored request

Build from source with a qualified Coax compiler and the normal platform TLS
dependencies:

```sh
psionic/platform/text-probe-build.sh /tmp/gptoss-probe plain
/tmp/gptoss-probe/pylon-text-probe \
  --relay wss://relay.openagents.com \
  --provider 23a62947ce9913e8d61d166fa72a2e4d2e11b153f9c47c5ee37e583ec702f599 \
  --identity gpt-oss-20b@sha256:3340a61d1a0391e8c5b5d3463d18d4c48129a84bbc04a554c762c99020aa06ed \
  --prompt 'Reply with exactly PONG and no other words. Public probe UNIQUE-LABEL.' \
  --expect PONG --timeout-ms 60000
```

Use a new public probe label for each run. Exit 0 requires a fresh pinned
signed beacon, a verified signed correlated result and the expected final word.
Exit 1 means no acceptable final answer before the deadline or error feedback;
exit 2 means invalid configuration. The final stdout line is a JSON receipt.
The client creates an ephemeral buyer key in memory and has no model or Python
runtime dependency. Its generated C and owned network/TLS/entropy effects can
be compiled inside the managed Debian watcher image.

Prompts are public NIP-90 inline text. The supported subset and result tags
are defined in [NIP-PYLON](../../../nips/openagents/NIP-PYLON.md). The private
Responses bearer stays in mode 0600 files and is read by the Coax host; it is
absent from argv, public events and receipts. Model and buyer HTTP services
remain on loopback.

The [initial verification](../../../docs/verification/gptoss-cloud-nip90-2026-10-30.md)
records native Mac/Linux and sanitizer results, exact source hashes, real
requests/results and the legacy decision-path regression. The six-hour
availability record is owned by the fleet watcher, separately from initial
qualification.
