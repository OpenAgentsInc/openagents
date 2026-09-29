# Extension eval runner: live run, publish, and check, 2026-09-29

What we ran: `openagents ext eval run`, `publish`, and `check`
([#9934](https://github.com/OpenAgentsInc/openagents/issues/9934)) on a
MacBook (macOS, `sandbox-exec`), with live Coder (`coder -p`, built from
this change) answering through the Vercel AI Gateway
(`google/gemini-3.8-flash`) and classifying through live Jev
(`POST /v1/systemone`), in both arms. The subject is the fixture extension
`crates/ext-eval/fixtures/repo-map-brief`: a program over the `repo_map`
Wasm guest and one skill. Its suite has two cases, three runs per arm. The
starter suites of [#9935](https://github.com/OpenAgentsInc/openagents/issues/9935)
hadn't landed, so this is the fixture suite.

Trainer A ran the suite and published the result to a local `nostr-relay`
(its own Postgres and Blossom media root). Trainer B, with a different
world key and a fresh copy of the extension, checked it: fetched the
result and the suite release, verified every file against the release
manifest, reran the suite with live Coder, and published a check.

To reproduce, with the door and Jev keys in the shell (`CODER_DOOR_KEY` or
`CODER_AI_GATEWAY_KEY`, `CODER_MODEL`, `TYPESAFE_API_KEY`) and a relay with
`NOSTR_RELAY_MEDIA_ROOT` set:

```sh
CODER=/path/to/openagents/target/debug/coder
cp -R crates/ext-eval/fixtures/repo-map-brief /tmp/a/ && cd /tmp/a
openagents ext eval run repo-map-brief --trust --concurrency 4 --coder "$CODER"
openagents ext eval publish repo-map-brief/evals/results/*/report.json --relay ws://127.0.0.1:17777
# As a second trainer (another VERSE_HOME, or --as PROFILE), in a fresh copy:
openagents ext eval check RESULT_EVENT_ID repo-map-brief --trust \
  --relay ws://127.0.0.1:17777 --coder "$CODER"
```

## Results

| | Trainer A (run) | Trainer B (check) |
| --- | --- | --- |
| Verdict | **Better** (`pass`) | **Better** (`pass`) |
| Tests passed with the tool | 1 of 2 | 1 of 2 |
| Tests passed without it | 1 of 2 | 1 of 2 |
| Mean score, both arms | 0.5 | 0.5 |
| Wall seconds, with / without (6 runs each) | 17.0 / 31.0 | 12.4 / 29.0 |
| Cost | unknown (Coder doesn't price gateway lanes) | unknown |
| Coverage | 6 of 6 runs completed per arm | 6 of 6 per arm |
| Suite digest | `sha256:44dd5a7fd79d…` | the same |
| Subject lock | `sha256:aea9bb2a32d8…` | the same |

The publish uploaded the suite's 17 files to the relay's Blossom server
(the relay's limit of 15 uploads per key per minute made the uploader wait
once and retry), released the suite as a NIP-EXT `3184`, and published the
result as a `3189` with the report inline. The check published its own
`3189` citing the original with the `check` marker, and
`nostr::eval_ext::linkage` read the two signed events as **confirm**.

Per case, every run of each arm answered the same way:

- `greeting` (should not fire): both arms greeted, and neither ran the
  guest (`untouched`, `max = 0`, scored in both arms). 3 of 3 runs passed
  in each arm.
- `overview` (should fire): in the subject arm, Jev selected the
  `repo-map-brief` program and the turn
  ran the `repo_map` guest (`mapped` passed, and the `receipt` grader
  replayed each recorded call exactly), but the turn's reply was the run
  summary ("repo-map-brief ran its 1 steps: …"), which names no language
  and no file, so both outcome graders failed. In the baseline arm, Jev
  routed the turn to `clarify`, and Coder asked a question instead of
  answering. 0 of 3 runs passed in each arm.

## What this says

- The runner, sandbox, both arms, the report, publication, and a check all
  work end to end against live Coder, and a second trainer's rerun
  reproduced the verdict.
- The **Better** verdict is the gate keeping on time alone: both arms
  passed the same tests with the same mean score, and the subject arm was
  faster because a program turn answers with its run summary in under a
  second. `ext-eval-v1` keeps an extension that improves score, cost, *or*
  time beyond the spread, so a tool that makes Coder answer faster and no
  better reads as Better. We record this for the gate's owners rather than
  change the gate here.
- A program turn doesn't put the guest's output in Coder's reply, so an
  outcome grader on `last_message` can't see what a guest found. Suites
  for guest tools (the #9935 starters) should grade files or mechanism
  until Coder's program turn reports its result.
- The skill had no visible effect: the subject's overview turns ran the
  program rather than answering, and the greeting doesn't call for it.

This run also found and fixed three things before the numbers above: the
child needs Coder's question sets (the runner now copies them into both
arms and pins them in the run locks), a `module` step's guest call wasn't
in the trajectory (Coder's runtime now records it), and the uploader now
waits out a relay's upload rate limit.
