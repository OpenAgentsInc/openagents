# Nitro door: Open Responses transport and the headless Autopilot runbook

Date: 2026-08-30. Status: built, and exercised end to end on issue #374.

This records the CLI side of the Open Responses door — how `--dev` decides
which protocol a local door speaks, which runtimes carry that decision, and the
exact commands that drove a headless Autopilot run to a landed fix. The door's
own half (request translation, event construction, Gemini thought signatures,
compliance, deploy) is written up in the `nitro` repo at
`docs/open-responses-door.md`.

## The door is adopted from its health body

`--dev` adopts a server already running on this machine, and there are two
kinds. `openagents-coder-api` serves the Coder thread API; `nitro` serves Open
Responses and has no threads at all. `coder_dev.rs` (`health_spec`) reads
`GET /health` and answers `DoorSpec::OpenResponses` when the body carries
`"spec":"open-responses"`, `DoorSpec::Threads` otherwise. A door declares its
contract and the client follows it, so a new door needs no CLI release.

## Every runtime has to carry the decision

The selection travels two ways because the CLI has two kinds of caller:

- `OPENAGENTS_DOOR_SPEC`, set in `entry.rs` beside `OPENAGENTS_API_URL` and
  `OPENAGENTS_BASE_URL`, for code that reads the environment
  (`coder_dev::door_speaks_openresponses`).
- A `DoorSpec` threaded through `cli.rs` into `run_headless_coder` and
  `coder::autopilot_run::run`, ending at
  `CoderRuntimeSession::use_openresponses(bool)`.

The second path is the one that broke: the interactive TUI adopted the door
while `--headless` and `--autopilot` built their own `CoderRuntimeSession` and
kept posting to `/api/v1/threads`, which a nitro door does not serve. A door
adoption that only covers the path a human watches is not adoption. On the Open
Responses branch, `runtime.rs` posts to `{api_base}/responses`, converts
chat-style assistant tool calls into `function_call` items, reads
`response.output_item.done` for calls, runs the tool, and continues with a
`function_call_output` item.

## Runbook

The path that produced the `#374` defect-1 fix (`c0e06ddb`).

**1. Door, with a Gemini upstream.** `nitro-echo-1` proves the contract but
cannot write code.

```sh
cd ~/repos/nitro && cargo build --release
NITRO_GEMINI_API_KEY=… setsid nohup ./target/release/nitro --bind 127.0.0.1:4100 \
  > /tmp/nitro-4100.log 2>&1 &
curl -s localhost:4100/health   # "spec":"open-responses","gemini":true
```

Restart it by PID; a `pkill -f target/release/nitro` from the launching shell
kills the shell too.

**2. Probe the handshake headlessly before spending a run on it.** Same
transport Autopilot uses, one turn:

```sh
cd ~/repos/openagents
openagents coder --dev --headless "Read .githooks/pre-push and tell me in two \
  sentences how it reports a failing test."
```

Expect `open-responses door already running at http://127.0.0.1:4100`, an
answer only a reader of the file could give, and no `/api/v1/threads` request in
the door log.

**3. Hand Autopilot one unit, scoped in the directive.** The directive is the
standing pick filter, and it is where scope discipline lives — #374 had two
defects and only one was wanted:

```sh
OPENAGENTS_TOKEN=$OPENAGENTS_GIT_TOKEN setsid nohup openagents coder --dev --autopilot \
  "Work issue #374 defect 1 only: make the pre-push guard name the failing test \
   even when the workspace suite dies on a teardown abort with no named test. \
   Edit .githooks/pre-push. Do not attempt defect 2." \
  > /tmp/autopilot.log 2>&1 &
```

- The directive is positional. `--prompt` is not an Autopilot flag.
- A hosted lane wants a token, or the run stops with `Autopilot on a hosted
  lane needs an OpenAgents token`; otherwise pass `--lane local`.
- Rebuilding the CLI while a run holds the binary fails with `Text file busy`.

**4. Watch the workspace, not the transcript.** The iteration banner says the
loop is alive; `git status` says whether work is happening.

```sh
tail -f /tmp/autopilot.log
git -C ~/repos/openagents status --short
```

The run announced its unit, read `.githooks/pre-push`, edited it, and posted its
own progress comment on the issue under the agent identity.

**5. Review the diff, run the gate, land it.** The run's summary is a claim; the
diff and the gate are the evidence.

```sh
git diff .githooks/pre-push
cargo fmt --all -- --check && cargo test --workspace
git add .githooks/pre-push && git commit && git push origin HEAD:main
```

For this unit the change was one line: take failing test names from live
`test <name> ... FAILED` lines as well as the `failures:` summary block, so a
teardown abort that kills the runner before the summary flushes still names what
failed. The workspace suite reported one failure,
`tools::defect_tests::cancel_stops_the_shell_process_group`, which passes when
run alone — a load flake, and exactly what the retry logic in the edited file
exists to absorb.
