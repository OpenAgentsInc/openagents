# The v0.5 headless episode contract

`openagents.coder.episode.v1` is the contract between the Terminal-Bench
harness and a Coder Terminal v0.5 artifact. The Harbor custom-agent
adapter `tbench.coder_v05:CoderV05` is the harness's side; this page is
the artifact's.

The contract exists so the benchmark can pin an exact binary, prove the
pin before spending inference, and read one bundle shape back —
independent of how the artifact implements its loop.

## Identity

A run names exactly one artifact:

- `artifact_path` — a local file, or `artifact_url` for a fetched one.
- `artifact_sha256` — the sha256 of the artifact bytes. Required.
- `artifact_version` — the version string the artifact reports, recorded
  per trial.
- `contract` — must be `openagents.coder.episode.v1`.

The digest is checked on the host before upload and again inside the task
environment after upload. A mismatch, a missing file, or an unknown
contract version stops the trial before the environment spends anything.
The adapter never substitutes another executable: the `coder` on `PATH`
is v0.4, and no fallback to it, to a stub, or to an unpinned path exists.

## Layout inside the environment

| Path | Contents |
| --- | --- |
| `/opt/openagents/bin/coder-v05` | The artifact, mode 0755. |
| `/opt/openagents/assets/` | Packaged registries and question sets, when `assets_path` is given. |
| `/opt/openagents/instruction.txt` | The task instruction for this trial. |
| `/opt/openagents/episode/` | The episode bundle the artifact writes. |

Assets live outside the task workdir on purpose: a packaged registry or
question set inside the workspace would be evidence the verifier could
misread as agent output.

## Environment

The adapter forwards these by name; values never appear in a checked file:

- `OPENAGENTS_API_KEY` — the door bearer key.
- `OPENAGENTS_DOOR_URL` — the door base URL the artifact's model and
  decision calls go through.
- `OPENAGENTS_MODEL` — the requested model, when set.
- `OPENAGENTS_ASSETS_DIR` — set to `/opt/openagents/assets`.
- `OPENAGENTS_EPISODE_CONTRACT` — set to the contract id.

A container without a configured credential pair fails `doctor` — a setup
failure, not a task failure.

## Commands the artifact implements

`coder-v05 --version`

: Prints the artifact's version on stdout and exits 0. Runs during install.

`coder-v05 episode doctor --contract openagents.coder.episode.v1`

: Checks the binary's own requirements — door reachability, required
  assets, contract compatibility — without spending inference. Exits 0
  when the episode can run; nonzero output becomes the install failure
  reason. Runs during install, before any episode.

`coder-v05 episode run --instruction-file F --output-dir D --contract C [--model M]`

: Runs one headless episode against the instruction in `F`, in the task
  workdir, and writes the bundle under `D`. A nonzero exit ends the trial
  as an agent error; the adapter's exec timeout ends it as `timeout`. The
  artifact must keep running until it finishes or is killed — it does not
  self-report success it did not earn.

## The episode bundle

`D` after `episode run`:

```
manifest.json
trajectory.atif.json
artifacts/
verification/
evaluation/
```

- `manifest.json` — the episode manifest: artifact identity and digest,
  the contract id, door and model identities, resource bounds, dispatch
  and attempt records, cancellation or recovery records, and references
  into the rest of the bundle.
- `trajectory.atif.json` — one ATIF `Trajectory` document
  (`ATIF-v1.7`), valid against Harbor's Pydantic models. Custom
  observation metadata lives under `extra`; nothing undeclared.
- `artifacts/` — what the episode produced: submitted files, source
  snapshots, selected and omitted context records.
- `verification/` — the artifact's own verification evidence, separate
  from the task's independent verifier.
- `evaluation/usage.json` — the usage record:

```json
{
  "tokens": {
    "input": 1234,
    "cache": 56,
    "output": 789
  },
  "cost": {
    "amount_usd": 0.0123,
    "provenance": "provider_reported | price_estimate | billing_verified | unknown"
  },
  "calls": {
    "generation": 4,
    "decisions": 2,
    "failed": 1,
    "retries": 1
  }
}
```

Every AI operation counts: generation, decisions, context selection or
summarization, reviewers, delegates, and failed or retried calls.
Unavailable fields are absent or `null` — never zero. The adapter reads
this file into Harbor's context; the attempt record keeps its provenance
alongside Harbor's own accounting rather than conflating the two.

## Timing, timeout, cancellation

Harbor owns the trial clock. The adapter runs the episode through
`environment.exec` under the trial's agent timeout; a deadline expiry
kills the episode process and records `timeout`. A job cancellation tears
the environment down; whatever the artifact already wrote under `D` is
collected before deletion. Neither path fabricates a result.

## Failure vocabulary

| Condition | Terminal status |
| --- | --- |
| Episode exits 0, verifier runs | `completed` with the verifier's reward |
| Episode exits nonzero | `agent_error` |
| Artifact missing or digest mismatch, checked on the host | Refused before Harbor starts; a `setup_failure` refusal record, no trial |
| Install or preflight fails | `install_failure` |
| Exec deadline expires | `timeout` |
| Job cancelled | `cancelled`, with the partial bundle collected |
| Episode exits 0 without a trajectory, or the bundle can't be collected | The verifier's status and reward, with `completeness.trace` `absent` or `completeness.bundle` `collection_failed` |
| Provider refuses | `provider_refusal` |
| No verifier result | `unverifiable` |

A failed episode that still wrote its bundle is a working integration:
the bundle, the exit code, and the status are the evidence.
