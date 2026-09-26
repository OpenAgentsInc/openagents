# Microcoder repository adapter

`microcoder repository` runs the existing Microcoder loop against an explicitly
admitted local repository through the [common task owner](task-owner.md). It
uses the same task journal, execution lease, cancellation requests, boundary,
ATIF transcript, retained candidate artifacts, and reconstructed views as the
bounded-command host. It does not introduce a second task journal or a second
agent algorithm. [Issue #9674](https://github.com/OpenAgentsInc/openagents/issues/9674)
tracks the remaining adapter gates.

This first profile is opt-in and uses the operator's Codex login for generation
and the configured Jev client for judgments. Model-written acceptance tests,
routing to a stronger model, dynamic knowledge retrieval, and the loop's
experimental completion gates are disabled. An explicit `frozen-context` grant
can include exact knowledge documents and their declared source lineage. Independent verification remains a separate
`coder task check` operation under requirements pinned before execution.
Neither a model's `finished` action nor a successful shell command supplies that
verification.

## Supported entry points

The library entry points are
[`coder::task::adapter::Host`](../../../crates/coder/src/task/adapter.rs) and
[`microcoder::repository`](../../../crates/microcoder/src/repository.rs).
The host owns admission, effects, and evidence. Microcoder supplies the existing
`run::run` algorithm through a repository `Env` and recorded generator, judge,
and observer adapters. A synthetic in-process provider exists for offline
fixtures; the command-line entry point requires the Codex provider.

The foreground command is:

```sh
microcoder repository --grant grant.json --store /absolute/private/task-store
```

Add `--detach` to start the same model host in a new operating-system session.
The launcher retains the exact grant bytes and a private diagnostic file before
spawning. Its response reports `admission: "pending"`; inspect the task and
diagnostic path for the later admission or refusal. The detached host survives
the launching client's exit. Without `--detach`, keep the foreground process
alive or supervise it explicitly. `coder task start` remains the separate
bounded-command entry point. A separate client can inspect the run:

```sh
coder task show TASK_ID --store /absolute/private/task-store
coder task view TASK_ID --store /absolute/private/task-store --limit 100
```

Use the [task command format](../guides/tasks.md) to request cancellation or a
correction at the current revision. On owner loss, `coder task recover` records
unknown execution and does not replay an effect. Closing an observing client
has no effect on the foreground host; losing the host itself does.

## Prepare the request and grant

1. Create an isolated Git worktree for writable execution. Its common Git
   directory and the private task store must be outside the writable tree.
   A read-only grant can use a normal checkout.
2. Submit an inert task with requested adapter `microcoder-repository` and an
   exact model identifier, such as `gpt-6-luna`. Include the actual user prompt
   and canonical workspace path. A supplied source revision must be a full
   commit ID.
3. Read the accepted task's `intent_digest` and current `revision` with
   `coder task show`. Create a separate operator grant using those values.
4. Run the foreground entry point. A repeated grant cannot restart a finished
   or unresolved attempt. Use a new task and a new explicit grant for new work.

The following is a grant template, not an executable grant. Replace the task,
intent, revision, program path, and decision configuration with the exact local
values. `program` must be the canonical system `/bin/bash` or `/bin/sh` path;
`arguments` must be empty. The host supplies each admitted command as one script
argument without interpolating it into another command string.

```json
{
  "schema": "openagents.coder.task-execution-grant.v1",
  "task_id": "TASK_ID",
  "intent_digest": "sha256:REPLACE_WITH_ACCEPTED_INTENT_DIGEST",
  "expected_revision": 1,
  "expected_source_snapshot": null,
  "program": "/bin/bash",
  "arguments": [],
  "write_workspace": true,
  "wall_seconds": 600,
  "stream_bytes": 65536,
  "memory_bytes": 536870912,
  "requirements": null,
  "adapter_configuration": {
    "schema": "openagents.microcoder.repository-config.v1",
    "provider": "codex",
    "model": "gpt-6-luna",
    "effort": "medium",
    "generation_endpoint": "https://chatgpt.com/backend-api/codex",
    "decision_endpoint": "https://REPLACE_WITH_CONFIGURED_DECISION_ORIGIN",
    "decision_model": "REPLACE_WITH_CONFIGURED_DECISION_MODEL",
    "max_steps": 16,
    "acceptance": false,
    "route": "never",
    "knowledge": "off",
    "dollar_limit_micros": null,
    "expected_controller_digest": null
  }
}
```

The closed configuration rejects unknown fields and unsupported combinations.
`knowledge` can be `off` or `frozen-context`. The former requires an empty
requirements knowledge inventory; the latter requires a nonempty inventory.
The host validates exact document ID, version, digest, path, and declared
sources before retaining the text in Context. It rejects current-task and
source-excluded provenance. The existing loop receives those retained bytes;
it does not query an ambient base or call an embedding service.
The admitted model must equal the task's requested model. Generation uses the
fixed Codex transport endpoint. The decision endpoint and model must equal the
actual Jev client configuration before admission. The returned decision model
name must also match exactly. An alias such as `jev-latest` that resolves to a
different returned name is refused; select and admit an explicit version when
you need an exact identity. Endpoints cannot contain URL
credentials, query strings, or fragments. Other benchmark providers remain
available through the existing Terminal-Bench command; they are unsupported by
this repository profile.

The host records the controller executable's canonical path and SHA-256 digest
in the admission transcript. Set `expected_controller_digest` to its exact
`sha256:` value to require that executable before admission. This is a local
identity check, not remote attestation. The requested effort is retained;
provider confirmation of the effective effort and immutable model artifact
identity is unavailable.

`expected_source_snapshot`, when supplied, pins uncommitted source as well as
tracked content. The host always records a complete observed source snapshot
and rechecks it before the epoch's first effect. It also captures the user
prompt and root and declared scoped instruction files through the common
context builder. Repository context cannot widen the grant. Local OS-user
access remains the trust boundary; an external writer is not globally fenced.

## Isolated container commands

An optional closed `container` field in `adapter_configuration` selects a local
Docker boundary. An omitted or null field keeps local command execution.

```json
{
  "schema": "openagents.microcoder.container.v1",
  "docker_program": "/canonical/path/to/docker",
  "docker_digest": "sha256:REPLACE_WITH_EXECUTABLE_DIGEST",
  "socket": "/absolute/path/to/docker.sock",
  "image": "sha256:REPLACE_WITH_LOCAL_IMAGE_ID",
  "uid": 501,
  "gid": 20
}
```

Replace the template values with the exact admitted executable, Unix socket,
image ID, and non-root container user. The host requires the image to exist
locally and never pulls it. It refuses image-declared implicit volumes. The
controller rechecks its Docker executable digest before each control operation.
Docker clients run through `supervise` with an empty environment and an explicit
local socket and configuration directory; container workloads receive no host
credentials or Docker socket.

Each command gets a new container with the source mounted at `/workspace` and
that working directory selected. The source's Git metadata pointer is mounted
read-only. The root filesystem is read-only; `/tmp` is a bounded private tmpfs.
The container uses network `none`, no Linux capabilities, no new privileges,
one CPU, 128 PIDs, and the grant's memory limit without additional swap. Its
program is the pinned image's `/bin/bash`; the host retains the exact image and
script. The environment contains explicit tool paths, `HOME` and `TMPDIR`, and
the conventional read-only Rust toolchain path. The image must provide the
required tools.

Only workspace files persist between commands. Shell state, package installs
outside that tree, and background services do not persist. Model file reads
map `/workspace` back through the same confined host reader. The final candidate
uses the existing source snapshot and retained-artifact machinery.

The host records creation, start, inspection, kill when needed, and removal in
the existing ATIF trace. It verifies the exact image and ownership label before
cleanup. Stopping the Docker CLI alone is never sufficient: the host inspects
and, if necessary, kills the whole container before removing it. A lost creation
reply is reconciled against the retained name and ownership label; it is never
silently retried as a new container. Unavailable cleanup leaves execution
unknown and prevents another effect. Host loss still requires explicit recovery
and inspection; it does not prove that a container disappeared.

## Effects, credentials, and cancellation

The host records an epoch effect intent in the task journal before execution.
Within that epoch, each model request, read, command, and result receives a
sequence number and a fsynced ATIF record. Failure to retain an intent prevents
its dispatch. Failure to retain a later observation stops further effects and
leaves explicit incomplete or unknown evidence.

Credentials stay in the controller. Shell children receive a cleared
environment, an explicit system `PATH`, and private scratch for `HOME` and
`TMPDIR`. Filesystem writes are restricted to the granted worktree and scratch;
the task store and common Git directory stay protected. The existing boundary
also permits its documented system read paths. On macOS its offline policy
blocks external IP traffic but permits localhost; on Linux it uses an isolated
network namespace. This profile does not claim those policies are identical.
Controller model calls use the explicitly admitted endpoints.

The owner checks cancellation before dispatch and while awaiting commands and
model calls. A command's supervisor result records process-group cleanup.
Cancelling an in-flight remote model request stops local waiting; it cannot
prove that remote inference stopped or that no charge occurred. Those costs
remain unknown. Corrections preserve old evidence and stop reuse of the previous
context; a replacement run needs a new admission.

File reads use descriptor-relative confinement and refuse traversal, symlinks,
and hard-linked regular files. Missing files, denied reads, read errors, and
truncation are distinct retained observations. A read error after dispatch
marks observation incomplete and stops further effects.

## Evidence and cost

The trace retains full bounded command observations separately from the loop's
short prompt projection. It retains exact generated request inputs, each Codex
attempt's parsed native output items and token usage before action reduction,
and the successful Jev response body. Failed or interrupted calls explicitly
label unavailable response bodies or partial items. HTTP authentication headers
and credential files are not recorded. Parsed native items are not a recording
of every original network frame.

The repository Jev wrapper disables SDK retries, so an unrecorded retry cannot
turn a possibly charged failure into a known final cost. Codex retries retain
each attempt separately. A successful Codex reply with no usable token counts
has unknown cost, preserving any known lower bound and earlier uncertainty.
Missing or mismatched served model identity refuses the generated action after
retaining the response. Synthetic fixture calls can explicitly cost zero.

The final task result reports billing as unknown. Individual calls can carry
list-price estimates, measured tokens, known subtotals, and unknown reasons;
these are not a hard dollar budget or a provider invoice. A non-null
`dollar_limit_micros` is refused. Wall time and step limits remain enforceable
host controls.

Ordinary evidence is limited to 48 MiB with at most 8 MiB per serialized step.
The final disposition has reserved space so a retention refusal can still be
sealed within the common reader's 64 MiB limit. Model outputs are materialized
by their existing transports before the evidence cap is applied; this is not a
hard bound on controller memory. The grant's `memory_bytes` applies to
supervised shell processes. Commands retain bounded streams and stop at the
host's stdout cap. Retained candidate files use the common
[artifact limits and exact-byte checks](task-owner.md#retained-candidate-artifacts).
An omitted or incomplete record never becomes complete evidence.

## Verification and current limits

Offline fixtures exercise the unchanged loop with synthetic models and real
local worktree boundaries, supervisor cancellation, ATIF reconstruction,
retained output retrieval, unsafe-path refusal, evidence limits, unknown model
cost, and native identity refusal. Real Docker fixtures additionally check the pinned local image, isolated
command output, whole-container cleanup, and cancellation of a delayed
background writer. These tests do not establish real-model coding quality,
paid-provider completion, Linux host behavior from a macOS run, or a
Terminal-Bench result.

[The retained acceptance record](../verification/2026-09-26-repository-adapter/README.md)
preserves four fresh attempts, including two explicit model-identity refusals.
The operator stopped further model and benchmark work before independent checks
were run on the later candidates. One later loop reached its step limit; the
other reported model completion. Neither is an independently verified pass.
Issue #9674 remains open with that live acceptance gate unmeasured.

Hard limits on model-response buffering and additional provider adapters remain
unsupported capabilities. They require separate implementation and validation;
this profile does not silently emulate them.

The common task store supplies local authority and evidence. Remote Nostr
admission, multi-device control, paid labor, and automatic candidate integration
remain separate suite gates.
