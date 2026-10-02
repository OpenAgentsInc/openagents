# The hosted eval runner

The hosted eval runner runs extension test sets for people chatting with
OpenAgents on the phone, on our computers, so a first run needs no
computer of their own. It's `crates/eval-runner`, a NIP-CJ execution
worker whose one target is the `ext-eval` program. This page is the
serving decision, its bounds, its usage log, and the runbook. The wire is
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
  This bucket predates the relay's own media: since 2026-10-02
  `relay.openagents.com` takes Blossom uploads (`PUT /upload` with a
  NIP-98 authorization signed by the publisher's key) into
  `gs://openagentsgemini-relay-media`, so a publisher needs no cloud
  credentials (#10181). The
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
  is a run like any other. `crates/eval-runner/examples/trainer.rs` sends
  one with `--validates RESULT_ID`, with `SUITE_AUTHOR` naming the second
  test set's signer.
- **Jev's doors.** Coder asks Jev whether a turn runs a program before
  anything else, so a run without Jev never reaches the extension and
  both arms score alike. The child reaches Jev only through its run's
  decision proxy, which asks Jev's doors in the chat judge's order: the
  Vercel AI Gateway (`typesafe-ai/jev`, under `AI_GATEWAY_API_KEY`, else
  under `CODER_DOOR_KEY` when the chat door is the gateway), then
  OpenRouter when `OPENROUTER_API_KEY` is set, then TypeSafe
  (`TYPESAFE_API_KEY`) last, leaving a door only for a reason of its own
  (`jev::doors::fails_over`: a 402 for an account out of credits, a 5xx,
  a timeout). The `decision` graders use the same doors. The startup
  log's `decision` line names them in order
  ([#10122](https://github.com/OpenAgentsInc/openagents/issues/10122)).
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

## Bounds and usage

There is no usage limit. The owner decided on 2026-10-01 ("ensure no such
limits anywhere in the app, I never want to see a limit again … just
record all the info so later we can pull usage stats easily";
[#10120](https://github.com/OpenAgentsInc/openagents/issues/10120),
[#10121](https://github.com/OpenAgentsInc/openagents/issues/10121)): a
trainer runs as often as they like, and nobody has a daily count. What
bounds a request is its own shape:

| Bound | Deployed value | Refusal |
| --- | --- | --- |
| Tests, runs per arm, arms per request | 8, 3, 2 | `too_large` |
| Draft size | 64 KiB | `too_large` |
| Suites at once, runs at once in a suite | 2, 4 (`EVAL_RUNNER_JOBS`, `EVAL_RUNNER_CONCURRENCY`) | queued, not refused |
| A request's deadline | 1 hour after signing | `stale` |

**The emergency brake.** For an abuse emergency only, an operator may set
`EVAL_RUNNER_RUNS_PER_DAY` (runs per trainer per UTC day; a check never
counts) or `EVAL_RUNNER_TURNS_PER_DAY` (agent turns, tests × runs × 2
arms, per UTC day for everyone) in the environment file and restart. Both
are unset in the example and on the host, and the journal's start lines
and `eval-runner check` say `limits   no usage limit; 2 suites and 4 runs
at once`. A set brake refuses `over_quota` with a message that names no
count ("the hosted runner can't take this run right now; try again
later"), and the phone shows it as "Our test computers can't take this run
right now. Try again later." The dollar cap stays where it was: on the AI
Gateway key itself (`NEEDS_OWNER.md`).

**The usage log.** Every request the runner answers, admitted or refused,
is one JSON line in `~/.openagents/eval-runner/usage/YYYY-MM-DD.jsonl`
(the UTC day it arrived): time, trainer key, request id, action (`run`,
`check`, `validation`, `publish`), the tool's DefinitionRef id, the test
set (release id or `draft`), tests, runs per arm, turns, outcome and
code, verdict and passes with and without the tool, the published
result's id, the time to the end, and the request's size. No test text
and no key. On `coderos-4080`:

```sh
~/.local/libexec/openagents-eval-runner/eval-runner usage                # by day
~/.local/libexec/openagents-eval-runner/eval-runner usage --by key       # per trainer
~/.local/libexec/openagents-eval-runner/eval-runner usage --by subject --since 2026-10-01
~/.local/libexec/openagents-eval-runner/eval-runner usage --by outcome --json
jq -s 'group_by(.key) | map({key: .[0].key, runs: length})' ~/.openagents/eval-runner/usage/*.jsonl
```

`--by` takes `key`, `action`, `subject`, `day`, or `outcome`; each row
has jobs, completed, failed, refused, distinct keys, turns, and the median
time of a completed job. `quota.json` still counts runs and turns per day,
so a brake set mid-day starts from the day's real counts.

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

**`a3de5c8ff8` (2026-10-01): no usage limit, and the usage log**
([#10121](https://github.com/OpenAgentsInc/openagents/issues/10121)). The
environment file was backed up
(`eval-runner.env.bak-10121-20261001T213105Z`) and its
`EVAL_RUNNER_RUNS_PER_DAY=3` and `EVAL_RUNNER_TURNS_PER_DAY=2000` lines
removed; `install.sh a3de5c8ff8` built and installed at 21:32 UTC.
`eval-runner check` and the journal say `limits   no usage limit; 2 suites
and 4 runs at once` and `usage   ~/.openagents/eval-runner/usage`. The
agent's digest is now `sha256:c3213ec3…`, so results from before it can't
be checked on the hosted runner.

Live proof the same minute: one fresh trainer key
(`e8fa9b8e…`, `crates/eval-runner/examples/trainer.rs` on the host) sent
four runs of the Dependency check test set
(`a6b800a6…`, `--runs 1`) between 21:33 and 21:35 UTC; all four ran and
completed (journal: four `running … for e8fa9b8e6489` and four
`finished` lines, no `refused`), where the old count refused the fourth.
`eval-runner usage --by key` read them back:

```text
key                                                               jobs  completed  failed  refused  keys  turns  total_p50_ms
e8fa9b8e6489241ded98ea5b4d71510435860dbeb222264c2268cf323b8ec83d     4          4       0        0     1     48         24084
```

Each run scored 2 of 6 with the plugin and 2 of 6 without
(`inconclusive`), where the measurement at `1437ede584` scored 6 of 6 with
it; the runs were the proof of admission and that difference was not
investigated here.

**`584d1958e3` (2026-10-01): Jev's doors lead the decision door**
([#10122](https://github.com/OpenAgentsInc/openagents/issues/10122)). The
four runs at `a3de5c8ff8` scored 2 of 6 with Dependency check and without
it because TypeSafe's account was out of credits: every run's
`stdout.jsonl` opened with `the classifier failed (POST …/v1/systemone:
402 Your organization has no available TypeSafe API credits …)`, so no
subject run picked the plugin's program and both arms answered plainly.
The 6-of-6 run at `1437ede584` (job `1329fdc2…`, 06:48 UTC) opened four
subject runs with `program dependency-check`. Nothing in Coder or the
plugin changed between them; the decision proxy simply had TypeSafe as
its only door. TypeSafe still answered 402 at 21:45 UTC, and the gateway
answered Jev under the chat door's key.

Installed with `install.sh 584d1958e3` at 21:52 UTC; the environment file
was not changed (the chat door is the gateway, so its key serves Jev's
gateway door). `eval-runner check` and the journal say `decision
https://ai-gateway.vercel.sh → https://api.typesafe.ai`. The agent's
digest is now `sha256:8ad8bcd2…`, so results from before it can't be
checked on the hosted runner. Live proof: a fresh trainer key
(`1c6d01a0…`) ran the Dependency check test set (`a6b800a6…`, 3 runs per
arm) at 21:52 UTC: **6 of 6 with the plugin, 2 of 6 without, pass**
(job `7edb75fb…`, sealed `f8ae64b8…`), with four subject runs opening
`program dependency-check` as at `1437ede584`.
