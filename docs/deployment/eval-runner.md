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
- **What it admits.** The catalog, the extension directories listed in
  [`deploy/eval-runner/catalog`](../../deploy/eval-runner/catalog), which
  `install.sh` turns into `EVAL_RUNNER_CATALOG`: Project map, Code finder,
  and Test reader, and the example plugins Explain this error, Release
  notes, and Dependency check
  ([#10086](https://github.com/OpenAgentsInc/openagents/issues/10086)), all
  under `crates/plugin-*`. A request may name a catalog tool by its
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
- **Suite files.** A released suite's files live in the public-read
  bucket `gs://openagentsgemini-eval-blobs`, read like a Blossom server at
  `https://storage.googleapis.com/openagentsgemini-eval-blobs/<sha256>`.
  `relay.openagents.com` runs on Cloud Run with Blossom media off (media
  needs persistent storage), so there's no relay-hosted store yet. The
  runner writes the bucket with `gcloud storage cp` as the service account
  `oa-eval-runner`, which may only create and read objects in that bucket;
  its key and gcloud configuration stay on the runner host. To check a
  hosted result from a terminal, pass
  `--blossom https://storage.googleapis.com/openagentsgemini-eval-blobs`
  to `openagents ext eval check`.
- **Catalog tools by release.** The runner's key publishes each catalog
  tool as a NIP-EXT release (the package records name it as publisher),
  once, when it starts, and a hosted result names its subject by that
  release. The referee opens an `eval-check` quest version per test set
  release and tool release, so a result whose tool has no release earns
  nothing.
- **Starter test sets.** The runner's key also released the three starter
  test sets (`<runner>:project-map-tests`, `code-finder-tests`, and
  `test-reader-tests`), so `knowledge/quests/ext-eval.*.json` list it as
  their publisher.
- **Validations.** A run request may name a published result with
  `validates` instead of `check`: the run then executes a second,
  published test set on the same catalog tool, and the runner publishes
  the result with the `validates` marker. It refuses `not_admitted` a
  validation of a result that isn't on the relay, that ran the same test
  set (that would be a check), or that tested another tool. A validation
  is a run for the quota. `crates/eval-runner/examples/trainer.rs` sends
  one with `--validates RESULT_ID`, with `SUITE_AUTHOR` naming the second
  test set's signer.
- **Coder's defaults.** At each admitted run the runner reads the
  `openagents:coder-defaults` releases of the package root
  (`EVAL_RUNNER_DEFAULTS_ROOT` overrides it for a test), their documents
  from `EVAL_RUNNER_DEFAULTS_DOCS` (default
  `~/.openagents/coder-defaults/documents`, which `microcoder xp adopt`
  writes on this host) and then from the adopter's NIP-94 locators, and
  admits every adopted extension it holds in its catalog in **both**
  arms, so reports are marginal (`meta.ext_eval.defaults` names the
  release; each arm's lock names the defaults lock). It logs `defaults
  release … admits …` once at start and again whenever the release
  changes, naming what it doesn't hold and what lapsed. See
  [the policy](../../packages/coder-defaults/policy.md#how-a-release-reaches-runtimes).

- **Liveness.** The runner never trusts a quiet socket, the same way
  the chat worker doesn't
  ([#9946](https://github.com/OpenAgentsInc/openagents/issues/9946)).
  `relay.openagents.com` is a Cloud Run domain mapping, so the runner's
  WebSocket ends at Google's front end, and when the relay instance
  behind it restarts, the front end can keep the connection established
  while nothing reaches it. So the runner sends a probe every 30 seconds
  (a `REQ` with `limit` 0 on its requests filter, which the relay answers
  with `EOSE` at once) and treats a probe still unanswered at the next
  one as a dropped connection: it logs `relay: the relay stopped
  answering: …; reconnecting in 1 s` and subscribes again. Every 45
  minutes, before the relay's one-hour request timeout, it opens a second
  connection, subscribes there, and only then closes the first
  (`renewed the requests subscription on a new connection`), reading the
  old one 5 seconds longer, so no request falls into the gap. A request
  delivered on both connections is handled once: the runner drops an
  event it has seen, and the ledger answers a retransmission with its
  recorded answer and never runs it again. `EVAL_RUNNER_PROBE_MS` and
  `EVAL_RUNNER_RENEW_MS` change the two periods. Each answered probe also
  sends systemd `WATCHDOG=1`; the unit's `WatchdogSec=120` restarts a
  runner that stops proving its subscription. The probe and renewal code
  is shared with the chat worker (`coder::relay::liveness`).

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
2. The bucket's uploader: the key of `oa-eval-runner@openagentsgemini`
   is `~/.openagents/eval-runner-gcs.json` (mode 0600), activated in the
   gcloud configuration `~/.openagents/eval-runner-gcloud`, which only the
   runner uses.
3. Copy `deploy/eval-runner/eval-runner.env.example` to
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
target directory (`~/.cache/openagents/eval-runner`), writes the catalog
from `deploy/eval-runner/catalog` to
`~/.cache/openagents/eval-runner/catalog.env` (the unit reads it after the
environment file, so the catalog moves with the code and the file that
holds the keys never changes for it), runs `eval-runner check` against
both, and restarts the user service. The service reads the catalog, the question sets, and the gates
from that worktree, so they always match the binaries. The first log lines
name the runner key, the relay, the catalog, and the agent's digest, then
`subscribed; requests arrive live from here`.

**Don't edit a released gate or suite.** A published suite's digest covers
the gate file's bytes and its case files. The runner reruns a published
suite only when it reproduces that digest, so changing
`crates/gym/gates/ext-eval-v2.json` or a starter case means releasing new
starter suites (`eval-runner release crates/plugin-repo-map …`) and new
quest versions.

**A redeploy starts a new line of results.** A result's subject lock
pins the Coder binary both arms ran, and a check counts only when its
lock equals the original's. A redeploy that rebuilds `coder` with other
bytes means earlier hosted results can no longer be checked on the
hosted runner; checks of new results work as before. Deploy only when
there's a reason to. An adoption does the same: once a `coder-defaults`
release admits a tool the runner holds, every lock names it, so results
from before the adoption can't be checked or validated on the runner
either, and an adopted tool's own suite reads inconclusive from then on
(both arms hold it).

## Releasing a catalog test set

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
reuses the releases. A new catalog extension's test set is released the
same way, naming its directory (for example
`…/src/crates/plugin-explain-error`). The measured first runs are in
[the live run](../extensions/measurements/2026-09-29-hosted-runner-live.md).

## Checking the deployed path

From a checkout, send the hosted request the phone and the desktop share (`openagents-chat-app::hosted`) from a fresh key; the
runner refuses it `not_admitted` at once, which proves it receives
requests:

```sh
cargo test -p openagents-chat-app --lib \
  live_the_runner_answers_the_phone -- --ignored --nocapture
```

`crates/eval-runner/tests/hosted.rs` runs the whole path against a local
relay (it needs `NOSTR_RELAY_TEST_DATABASE_URL`). Against production, send
a request from a fresh key with the phone's code path and watch the
service's journal: each request logs one line when it's admitted or
refused, and one when it finishes, with ids and counts only.

### When the phone gets no answer

A tap on **Start the test** that is never acknowledged, with the unit
still `active`, means the runner isn't receiving requests. On
`coderos-4080`:

1. `journalctl --user -u openagents-eval-runner --since -15min
   --no-pager`. A healthy runner logs nothing between requests except
   `renewed the requests subscription` every 45 minutes. `relay: …
   reconnecting` lines mean the relay is failing and the runner is
   rejoining it; check the relay (`openagents-nostr-relay` on Cloud Run,
   [runbook-cloud-run.md](runbook-cloud-run.md)).
2. `systemctl --user show openagents-eval-runner -p WatchdogTimestamp -p
   NRestarts`. A `WatchdogTimestamp` that isn't recent, or restarts
   climbing, means the runner can't prove its subscription.
3. Run `live_the_runner_answers_the_phone` (above). If it fails while the
   journal shows no fault, save `ss -tnpi` for the runner's PID (the
   relay socket's `lastrcv` says how long it has heard nothing) and the
   journal, then `systemctl --user restart openagents-eval-runner` and
   open an issue with both.

## Release records

**`c815433291` (2026-09-29): relay liveness.** Before it, the runner
(`0b39640d66`, up since 07:43 UTC) had no probe. The relay was
redeployed at 09:07 UTC (revision `00034-pit`); the runner's connection
from 08:43:57 UTC stayed on the old instance, and it still received
requests, because the relay fans events out across instances through
Postgres: `live_the_runner_answers_the_phone` got `not_admitted` in 0.68 s
at 09:27 UTC. That connection ended at the relay's one-hour request
timeout, 09:43:58 UTC (`relay: IO error: peer closed connection without
sending TLS close_notify; reconnecting in 2 s`), the runner subscribed
again at 09:44:01, and the live test answered in 1.85 s at 09:44:05.
Nothing was lost that time only because the old instance kept serving
until the timeout.

`c815433291` was installed with `install.sh c815433291` at 09:45:21 UTC.
Its log names `liveness a probe every 30 s; the subscription is renewed
every 2700 s`, then `subscribed; requests arrive live from here` at
09:45:23. The live test answered in 1.02 s at 09:45:35 (journal: `refused
496fbf00597e … not_admitted`). `systemctl --user show` gave
`WatchdogUSec=2min`, `WatchdogTimestamp` 09:47:53 UTC (the fifth answered
probe), and `NRestarts=0`. The rebuild changed the agent's digest to
`sha256:fbf73c42…`, so results from before it can't be checked on the
hosted runner (see "A redeploy starts a new line of results").

**`1437ede584` (2026-10-01): the example plugins.** Installed with the
new `install.sh` (run from that commit, so it wrote `catalog.env` from
`deploy/eval-runner/catalog`) at 06:42 UTC; `eval-runner check` and the
journal list six catalog plugins, and the agent's digest is now
`sha256:71e9341d…`, so results from before it can't be checked on the
hosted runner. `eval-runner release` then released the three new plugins
and their test sets; the runs and results are in
[the measurement](../extensions/measurements/2026-10-01-example-plugins.md).
