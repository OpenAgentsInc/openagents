# Lab jobs and workers

`lab-jobs` shares deterministic placement, durable worker slots, usage, and
lease checks across Lab kinds. `labs-worker` is the outbound program that runs
installed kind handlers.

Manifests declare minimum CPU and memory, GPU and tools, a maximum duration,
input classes, and any model use. Placement combines these with the immutable
version's inputs. Private files require an enrolled worker that declares their
checksums. Otherwise the job stays queued with a reason. Server placement takes
priority, followed by cloud and enrolled placement; worker IDs break ties.

## Run a worker

1. Sign in through the existing app or device sign-in. Send an authenticated
   `POST /v1/labs/workers` with a named space, capabilities, and a lifetime from
   one minute to one day. Save its response as a `0600` credential file. The
   response contains a job-only `labw_` key, not an account key. Project edit
   access, the issuing sign-in, and kind admission are checked again on every
   worker request. Revoke it with `DELETE /v1/labs/workers/WORKER_ID`.
2. Install trusted handlers for the declared `kind/job` pairs. Each receives
   JSON `worker::HandlerInput` on stdin and returns `worker::HandlerOutput` on
   stdout. The job cannot choose the executable or arguments. The built-in
   `--echo-handler` is a qualification fixture, not a general renderer.
3. Write a `0600` configuration file. Use absolute paths for the credential,
   state directory, and handlers. Map each declared private checksum to an
   existing file on this machine. State must be private to the worker's user.
4. Build with `cargo build -p lab-jobs --bin labs-worker`, then run
   `labs-worker --config /absolute/path/worker.json`. Add `--once` to poll and
   run at most one job before exiting, including when no job is available.

For example, the configuration for an enrolled Echo fixture is:

```json
{
  "origin": "https://api.openagents.com",
  "credential": "/private/worker/credential.json",
  "state": "/private/worker/state",
  "handlers": {
    "echo/shout": {
      "executable": "/opt/openagents/labs-worker",
      "arguments": ["--echo-handler"]
    }
  },
  "local_assets": {},
  "allow_loopback_http": false
}
```

Only outbound HTTPS is used; redirects are refused. Explicit loopback HTTP is
available for disposable qualification fixtures. The worker verifies local
checksums before starting, passes local paths only to its installed handler,
and uploads outputs. It starts one child at a time with an empty environment,
a private working directory, a memory limit, bounded output, and a deadline.
Revocation, cancellation, or a lost lease stops the process group. On Linux,
worker process death also stops its direct child. On macOS, which rejects the
address-space limit, the worker checks the child's resident memory every 25 ms
and stops its process group when it exceeds the job's limit.

The worker syncs a local marker before asking to start and saves its result
before sending it. A restart resends saved results; it never repeats a marked
execution. A lost started worker becomes `uncertain`. An unused expired claim
can return to the queue. Keep an uncertain worker's state for review rather
than deleting its marker to force a retry.

## Protected inputs and usage

Derived outputs inherit `no_training` from protected inputs. Dataset, evaluation,
and trace builders receiving Lab resources use `lab_kit::check_model_use` with
both the version and its artifacts. It also checks older output records whose
flag was absent. Agent Lab's existing trial job applies the evaluation check.
Agent Lab also inserts authorized Lab tool results into model task text. That
boundary must check the pinned source version and the artifacts it consumes;
operator flags cannot grant permission to use protected source data. The current
dataset, trace, and Gym builders do not import Lab resources.

`GET /v1/labs/jobs/JOB_ID/usage?space=me` returns attempts after normal space
and kind authorization. Records distinguish service-observed elapsed time from
unknown execution duration. Charges remain unknown until the billing and quota
service supplies them; no price or provider usage is invented.

Server adapters declare capabilities and implement the optional
`Kind::run_server_job` hook using `ServerExecution`. The existing CAD adapter
keeps its bounded child and semaphore, while common placement blocks private
inputs before dispatch and lease checks guard every output attachment.

Run focused checks with `cargo test -p lab-jobs -p lab-store -p lab-api -p lab-kit`.
The worker tests use a real loopback HTTP server and child processes under a
temporary directory. Their cloud placement test checks worker behavior;
Computers API provisioning and deletion have separate qualification evidence.
