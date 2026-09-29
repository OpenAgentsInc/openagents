# The hosted eval runner

The hosted eval runner runs extension test sets for people chatting with
OpenAgents on the phone, on our computers, so a first run needs no
computer of their own. It's `crates/eval-runner`, a NIP-CJ execution
worker whose one target is the `ext-eval` program. This page is the
serving decision, its limits, and the runbook. The wire is
[NIP-EVAL, Hosted runs](../../nips/openagents/NIP-EVAL.md#hosted-runs);
the product rules are
[Where runs execute](../extensions/evaluation.md#where-runs-execute).

## The serving path

```text
phone (trainer key) --25920, NIP-44--> relay.openagents.com --> eval-runner --> coder -p (sandboxed, both arms)
phone <--27020 accepted, progress------ relay.openagents.com <-- eval-runner --> AI gateway, Jev
phone <--26920 result, 3188 report----- relay.openagents.com <--
```

- **Wire.** A tap on **Start the test** sends a `25920` signed by the
  trainer's key, encrypted with NIP-44 to the runner. The runner answers
  `27020` `accepted`, then `progress` with the case runs finished of those
  planned, then one `26920` result naming the report and the `3188` it
  sealed the report in. `nostr::eval_ext::hosted` builds and reads every
  body; the phone never writes one by hand.
- **Endpoint.** The runner's key is compiled into the app as
  `nostr::eval_ext::hosted::RUNNER`:
  `a7cff3ee1ff0209f971b9f24673db310ab858899c9d9a99b640e6cb29b1753f0`. Its
  relay is `wss://relay.openagents.com`.
- **What runs.** The same engine as `openagents ext eval run`
  (`ext_eval::run::run_suite`): each run is one `coder -p` turn inside
  `coder-boundary` (`bwrap` on Linux), with the extension admitted and
  without it. The grant is fixed: read and sandbox write, never `exec` or
  `network`, so Coder's shell is off in both arms.
- **What it admits.** The catalog, the extension directories the operator
  lists (`EVAL_RUNNER_CATALOG`): today Project map, Code finder, and Test
  reader under `crates/plugin-*`. A request may name a catalog tool by its
  extension's DefinitionRef or by the DefinitionRef of the Wasm guest it
  runs, which is how the chat catalog names it. Chat-made tools are a
  skill that may turn on catalog tools. Everything else is refused
  `not_admitted`.
- **Publishing.** Nothing is public until the trainer who asked sends a
  publish request naming the report. The runner then releases the suite
  if it isn't released (its author is the runner), and publishes the
  `3189` signed by its own key, naming the trainer with a `p` tag and the
  `request` tag, and carrying the trainer's signed request in
  `meta.ext_eval_request`. Relays keep no `25920`, so that inline copy is
  how the referee and the phone's ledger credit the trainer.
- **Starter test sets.** The runner's key also released the three starter
  test sets (`<runner>:project-map-tests`, `code-finder-tests`, and
  `test-reader-tests`), so `knowledge/quests/ext-eval.*.json` list it as
  their publisher.

The runner runs on `coderos-4080`, the NixOS machine that already runs
the XP referee: it has `bwrap`, 28 cores, and a Rust toolchain, and its
user services outlive a logout. The Coder worker VM has no toolchain and
no sandbox backend, and its chat worker must not share a host process
with runs.

## Limits

| Limit | Deployed value | Refusal |
| --- | --- | --- |
| Tests, runs per arm, arms per request | 8, 3, 2 | `too_large` |
| Draft size | 64 KiB | `too_large` |
| Runs per trainer per UTC day | 3 (`EVAL_RUNNER_RUNS_PER_DAY`); a check doesn't count | `over_quota` |
| Agent turns per UTC day, everyone together | 2,000 (`EVAL_RUNNER_TURNS_PER_DAY`) | `over_quota` |
| Suites at once, runs at once in a suite | 2, 4 (`EVAL_RUNNER_JOBS`, `EVAL_RUNNER_CONCURRENCY`) | queued, not refused |
| A request's deadline | 1 hour after signing | `stale` |

The turn ceiling is the spend bound: one turn is one `coder -p` turn on
the Gemini Flash lane plus its Jev calls. The owner sets the dollar cap on
the AI Gateway key itself (`NEEDS_OWNER.md`); the runner can't see the
gateway's charges, so it counts turns.

**The admission switch.** `touch ~/.openagents/eval-runner/closed` refuses
every new run `not_admitted` at once, without a restart; runs already
admitted finish. Remove the file to open admission again.

## Deploying

On `coderos-4080`, once:

1. The runner key is `~/.openagents/nostr/eval-runner-key` (64 hex, mode
   0600). It was made on the host on 2026-09-29 and has never left it.
   `eval-runner pubkey` prints its public key, which must equal
   `hosted::RUNNER`.
2. Copy `deploy/eval-runner/eval-runner.env.example` to
   `~/.config/openagents/eval-runner.env`, mode 0600, and fill in the door
   key (`CODER_DOOR_KEY`, the AI Gateway key) and `TYPESAFE_API_KEY`.
   Copy keys between machines with a pipe (`ssh host 'cat > file'`), never
   through a terminal or a log.

Then, and for every upgrade, from the checkout at `~/openagents`:

```sh
deploy/eval-runner/install.sh            # origin/main, or a commit
journalctl --user -u openagents-eval-runner -n 20 --no-pager
cat ~/.local/libexec/openagents-eval-runner/REVISION
```

`install.sh` builds `eval-runner` and `coder` in their own worktree and
target directory (`~/.cache/openagents/eval-runner`), runs
`eval-runner check` against the environment file, and restarts the user
service. The service reads the catalog, the question sets, and the gates
from that worktree, so they always match the binaries. The first log lines
name the runner key, the relay, the catalog, and the agent's digest, then
`subscribed; requests arrive live from here`.

**Don't edit a released gate or suite.** A published suite's digest covers
the gate file's bytes and its case files. The runner reruns a published
suite only when it reproduces that digest, so changing
`crates/gym/gates/ext-eval-v2.json` or a starter case means releasing new
starter suites (`eval-runner release crates/plugin-repo-map …`) and new
quest versions.

## Releasing the starter test sets

With the environment file loaded, on the runner host:

```sh
set -a; . ~/.config/openagents/eval-runner.env; set +a
~/.local/libexec/openagents-eval-runner/eval-runner release \
  ~/.cache/openagents/eval-runner/src/crates/plugin-repo-map \
  ~/.cache/openagents/eval-runner/src/crates/plugin-code-search \
  ~/.cache/openagents/eval-runner/src/crates/plugin-test-report
```

It uploads each suite's files to the relay's Blossom server and publishes
one `3184` per suite, and prints each release's event ID. A second run
reuses the releases. The measured first runs are in
[the live run](../extensions/measurements/2026-09-29-hosted-runner-live.md).

## Checking the deployed path

`crates/eval-runner/tests/hosted.rs` runs the whole path against a local
relay (it needs `NOSTR_RELAY_TEST_DATABASE_URL`). Against production, send
a request from a fresh key with the phone's code path and watch the
service's journal: each request logs one line when it's admitted or
refused, and one when it finishes, with ids and counts only.
