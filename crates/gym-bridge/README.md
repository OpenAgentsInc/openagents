# Gym bridge

Gym bridge connects the Verse Gym to explicitly selected Microcoder and
Terminal-Bench records on a computer. It sends bounded private boards over the
existing Nostr relay. Separately granted recipes let a client request a fixed
local command. Entering the building, viewing a board, and reconnecting never
start work.

The portable client builds with `default-features = false`. The default `host`
feature adds the Unix source readers, CLI, private store, and subprocess
supervisor. No new HTTP server or relay deployment is required.

## Start with read-only access

Create a configuration with absolute paths that exist on the host. Paths below
are examples; JSON does not expand `~` or environment variables.

```json
{
  "sources": [
    {
      "root": "/home/operator/.openagents/microcoder/runs",
      "label": "Microcoder on this computer",
      "kind": "microcoder"
    },
    {
      "root": "/home/operator/.openagents/terminal-bench/jobs",
      "label": "Terminal-Bench on this computer",
      "kind": "terminal_bench"
    }
  ],
  "recipes": []
}
```

Build the host, use the public key shown by your Verse client, and pair it:

```sh
cargo build --release -p gym-bridge
./target/release/gym-bridge pair \
  --state /home/operator/.openagents/gym-bridge \
  --config /home/operator/gym-sources.json \
  --client CLIENT_PUBLIC_KEY \
  --relay wss://relay.openagents.com
```

The parent of the state directory must exist. Pairing creates a private state
directory and a host key. It prints the selected source paths, recipe counts,
grant ID, expiry, and a `gym-connect:` code. Paste the code into Verse's Gym
connection field. The code contains a public host pin and an encrypted grant
for that exact client; it contains no private key. This flow is separate from
chat-history pairing. A history grant cannot launch Gym work.

Keep the host running in the foreground:

```sh
./target/release/gym-bridge serve \
  --state /home/operator/.openagents/gym-bridge \
  --relay wss://relay.openagents.com
```

The default grant lasts 24 hours; `pair --expires-in SECONDS` admits at most
30 days. Production connections require an exact credential-free `wss` URL.
Unencrypted loopback sockets exist only under the explicit Rust fixture policy.

To revoke future requests from one client grant:

```sh
./target/release/gym-bridge revoke \
  --state /home/operator/.openagents/gym-bridge \
  --grant GRANT_ID
```

Revocation stops new reads and launches. It does not cancel an already admitted
process or erase evidence already disclosed to the client. The CLI does not
upload source files, credentials, prompts, transcripts, or command output.

## What the board reads

| Source kind | Supported files | Meaning |
| --- | --- | --- |
| `microcoder` | Immediate run directories with `summary.json` or `events.jsonl` | Summary totals, recorded ending, step count, elapsed time, and bounded model-call cost and latency series. |
| `terminal_bench` | Harbor `job/trial/{config,result}.json`; normalized `job/tbench/attempts/*.json`; retained `job/trial.episode/harbor-result.json` | Recorded task status, reward, reported cost, and available duration. |
| `training_summaries` | Immediate JSON files with `openagents.gym-training-summary.v1` | Explicit operator declarations of model-training progress, never a claim that this bridge trains a model. |

For retained Microcoder copies, select a specific host directory such as
`bench/terminal-bench/microcoder-runs/coderos-4080`. The reader does not recursively
search arbitrary files. For retained Terminal-Bench copies, select the directory
containing job directories, such as `bench/terminal-bench/traces`.

Source roots are canonicalized and pinned by device and inode at pairing.
Reads and enumeration use held directory descriptors with no symlink following
at any component. Replacing a root requires a new grant. A board examines at
most 512 directory entries and 8 MiB of source bytes, with 1 MiB per JSON file.
Each Microcoder tail is limited to the last 64 KiB and 256 decoded events. Series
contain at most 64 known points per metric; missing calls are omitted, not
converted into zero-valued points. A partial scan is labeled incomplete.

Boards contain at most 64 runs, 16 recipes, four metrics per run, and 128 KiB
of canonical JSON. Large training-summary series can exceed the aggregate bound
and refuse the snapshot; the host does not silently remove measurements to make
a claim fit. A board is a host projection, not independently verified EVAL
publication. `completed` means a recorded execution ended; it does not mean the
benchmark passed. Missing or malformed terminal evidence stays `unknown`.

A live file modified in the last 120 seconds is shown as `running`; older live
files are `stale`. These are observations of files, not proof that the process
is alive. Microcoder total cost is known only when model, Jev, and embedding
components are explicitly priced and no unknown charges are recorded. Older
records with missing components remain unknown. List-price cost and provider
charges retain their recorded provenance. Unknown cost is never zero.

## Opt in to executable recipes

The default configuration grants observation only. A recipe is an additional
local execution and potential spending authorization. Use a harmless recipe to
check a new setup before admitting a real evaluation command:

```json
{
  "sources": [],
  "recipes": [
    {
      "id": "host-echo",
      "title": "Host connection check",
      "detail": "Print a fixed message; no model or benchmark call",
      "program": "/bin/echo",
      "args": ["Gym connection works"],
      "cwd": "/home/operator",
      "wall_ms": 5000,
      "max_starts": 1,
      "environment": []
    }
  ]
}
```

Pair again with the new configuration. Each recipe revision digests its complete
configuration, canonical executable bytes, and working-directory identity. The
host refuses a changed executable or replaced working directory. Clients send
only the granted recipe ID, exact revision, and a durable launch ID; they cannot
supply shell arguments, paths, environment variables, or larger limits.

To admit an existing Terminal-Bench run, use your canonical `uv` executable,
the checkout's `bench/terminal-bench` directory as `cwd`, and a fixed argument
list such as:

```json
["run", "--frozen", "tbench", "run", "--profile", "tb4", "--agent", "YOUR_PINNED_AGENT", "--task", "YOUR_PINNED_TASK", "--job-name", "YOUR_UNIQUE_JOB"]
```

Replace the placeholders with a reviewed profile, agent, task, and unused job
name. Set `max_starts` to one for a fixed job name. Check the
[Terminal-Bench runbook](../../docs/terminal-bench/runbook.md) for prerequisites,
credentials, provider identity, task pins, retained results, and Docker setup.
This documentation does not start the run.

The executable pin **does not pin an interpreter, script dependencies, a
Python environment, a mutable checkout, container images, or provider models**.
Keep those dependencies in a separately managed immutable environment when the
comparison requires them. A recipe is operator-approved host execution, not a
sandbox or an immutable experimental closure.

The child receives a cleared environment, a fixed system `PATH`,
`GYM_BRIDGE_RUN_ID`, and only the environment variable names explicitly listed
in the local recipe. Values come from the running host and remain private.
List `HOME`, provider credentials, or runtime-specific variables only when that
recipe needs them. Never place credential values in configuration, arguments,
connection codes, or source control.

The supervisor enforces the wall deadline, captures at most 16 KiB per output
stream, and owns the process group. Memory behavior follows
[`supervise`](../../docs/coder/runtime/subprocesses.md). There is no enforced dollar budget. The
portable recipe reports `spend_limit_usd: null` and `spend_enforced: false`;
unsupported monetary guarantees are refused.

## Retries, restart, and scope

One private service lock permits one foreground host per state directory.
Short transaction locks serialize pairing, revocation, reads, and launch
admission. The host persists each launch intent before dispatch, allows one
active or unresolved launch globally, and retains exact launch IDs and results.
Retry the same launch ID after a lost reply. Changed semantics under that ID
refuse. Choosing a new ID authorizes another launch and must be an explicit
operator action.

A restarted host marks unfinished intents `unknown` and never repeats them.
An unresolved launch blocks new launches until an operator investigates the
retained record. Automatic reconciliation and a repair CLI are not implemented.
Stopping the foreground service drops the supervised worker futures; a hard
process crash cannot establish whether every external effect stopped. Process
exit zero is retained as completion, not as successful benchmark verification.

Command stdout and stderr remain in the private host store. They are not board
content. The store holds at most 64 active grants and 1,024 launch records;
launch tombstones are not discarded automatically. Private artifacts still
reveal relay timing, traffic volume, and routing metadata.

The current UI uses the client only inside the Gym. Leaving or suspending the
scene closes its client connection and cancels pending reads. A cancelled
client request can follow a successful host dispatch: preserve its launch ID.
Only a verified signed host refusal is classified by `confirmed_refusal` as a
confirmed rejection; transport or signature failures leave delivery unknown.

## Verification

Focused synthetic tests cover signed device-bound grants, expiry and revocation,
root and intermediate-symlink confinement, known and unknown costs, malformed
outcomes, live series bounds, normalized and retained Terminal-Bench layouts,
exact launch retries, restart uncertainty, changed executable/FIFO refusal,
expiry during admission, exclusive service ownership, and a real authenticated
loopback WebSocket exchange through snapshot, harmless launch, retry, and revoke.

```sh
cargo test -p gym-bridge
cargo clippy -p gym-bridge --all-targets -- -D warnings
cargo check -p gym-bridge --no-default-features
```

These tests do not execute a model, training run, or benchmark. They do not prove
production relay admission or the correctness of a specific evaluation recipe.
The wire profile is specified in [NIP-EVAL](../../nips/openagents/NIP-EVAL.md#private-gym-boards-and-admitted-recipe-control).
