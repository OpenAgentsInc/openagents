# Microcoder in Coax, and the inference gateways

Written 2026-10-28. Project: [Coder](https://openagents.com/projects/coder).
Book: [chapter 9 of Coder: Coding Agents From Scratch](../../coder/book/ch009-microcoder.md).

The desktop Lab sends each message to Mammoth first (`apps/openagents-desktop/router.coax`).
Mammoth picks `claude_code`, `codex` or `other`. **`other` goes to Microcoder**:
a small agent loop written in Coax that handles plain questions, small talk and
small jobs on any model we can reach. It does not need Claude Code or Codex
installed.

Microcoder needs models, so this document also plans moving every inference
upstream from the old Rust gateway into Coax as **gateways**: one interface and
one file per provider.

## What we had before the nuke

Read at `0b401561a0^` (the commit before the Coder service was removed).

| Old crate | Lines | What it did |
| --- | ---: | --- |
| `crates/microcoder-loop` | 15,300 | The loop: state → Jev questions → one structured call → run commands → repeat. Model adapters `vertex`, `openai_compat`, `claude`, `door`, `text_action`, `failover`, `capacity`, `reply` |
| `crates/microcoder` | 24,315 | The binary around it: repository adapters (Claude SDK, Claude and Codex sessions, Devin, Grok, OpenCode, native, Lean), tbench, kbnet |
| `crates/inference` | 27,264 | The gateway: Open Responses types, SSE, Chat Completions translation, the router (up to three model/upstream attempts, fallback before the first token, bench on 401/402), the meter and rates card |
| `crates/openrouter` | 2,907 | OpenRouter client: structured JSON chat, streaming, tools |
| `crates/gateway` | 91,875 | HTTP routes over it: BYOK, public, Pylon, own-Coder, x402, rates |
| `crates/model-access` | 2,574 | Which models a caller may use |

Old upstreams, in `crates/inference/src/upstream/`: `vertex`, `google`/`gemini`
(the Gemini Developer API), `openrouter`, `vercel` (the AI Gateway), `zai`
(Z.ai GLM), `anthropic`, `openai`, `pro` (the door proxy), `psionic` (local),
`pylon`, `coder`. Shared pieces: `chat` (Chat Completions), `responses`,
`http`, `secret`, `gate`, `measure`, `emit`, `progress`, `whole`.

The test fixtures `crates/coder/fixtures/gateway/*.sse` (Google, OpenRouter,
Vertex `gemini-3.8-flash`, Z.ai) are real recorded streams. They come over as
the gateways' test inputs.

The old loop, which this keeps:

```
state = {environment, task}
while not finished:
    answers = decide(state)                 # typed questions (Jev then; Mammoth now)
    action  = generate(state + answers)     # one structured call -> NextAction
    run action.commands, record their output in state
```

## Part 1: Microcoder

### Where it lives

- `coax/lib/microcoder/`: the loop, the state, the action schema and the
  tools. No platform code, so it compiles to C (the Mac app) and to WASM.
- `apps/openagents-desktop/`: the Lab calls it for `route: other` and draws
  its steps in the Lab transcript.
- Later: `coax/web/api` runs the same library as the server side of `/chat`
  for signed-in users without a local agent.

### The action

Each step is **one** model call with a strict JSON schema. The model returns:

```json
{
  "rationale": "why this step, for the log; never shown as the reply",
  "edits": [{"path": "a.txt", "old": "exact text, once in the file", "new": "its replacement"}],
  "commands": ["shell commands to run, in order"],
  "view": ["files to show in full in the next step"],
  "finished": false,
  "reply": "on the finishing step, the answer addressed to the user",
  "ask": "none | question"
}
```

This is the old `NextAction` without `freeze_tests` and `expand`, which return
when tests and the knowledge base do. `ask: question` ends the turn with
`reply` as the question, and the user's answer starts the next turn.

A plain question ("what is 2+2", "hi") finishes in one step with no commands.
That is the common case for `other`, and it costs one call.

### The state

```
MicroState {
  environment   // the Lab's Environment.facts: OS, app, hardware, agents, working directory
  task          // the user's message
  route         // Mammoth's decision: difficulty, effort, local_work
  earlier[]     // older steps, compacted: rationale, edits, commands with exit
                //   codes and the start and end of each output, files viewed by size
  actions[]     // the recent steps whole: rationale and its results: edits,
                //   commands with exit codes and output head and tail
                //   (6,000 bytes each), and the files it viewed (64 KB each)
  notes         // what the loop writes for itself between steps
}
```

The prompt is built for Vertex's implicit prompt cache, which serves a
request's prefix at a tenth of the price. The facts, task and route come
first and never change. `earlier` only grows at its end, and `actions` only
grows at its end until the next compaction. When the recent steps would
fill their 144 KB, all but the last 24 KB of them are compacted into
`earlier` in one go, so the prefix through `earlier` stays cached and
compaction comes once per 72 KB or more of new steps. A request also stays
under the journal's 256 KB. (Until #13231 it kept the last 12 actions in a
ring, then let the earliest whole steps leave; both broke the cached prefix
often.)

The prompt is the state as JSON, plus a short system prompt that names the
schema, the tools and the limits. The model never sees a key, a path under
`.secrets`, or another conversation.

### Tools in v1

- **`commands`**: run in the working directory through
  `coax/platform/process-native.c`, with a timeout per command (30 s; 120 s in
  Coder, which tells the model so in `environment.command_seconds`) and the
  output capped. A run in Coder has 15 minutes (`MC_RUN_MS`). With `local_work` under 0.5 the loop is told not to run
  commands, and any it returns are refused with a note in state.
- **`edits`**: exact replacements under the working directory (`old` must
  appear exactly once; an empty `old` writes the whole file), applied by the
  loop itself before the step's commands, so there is nothing to quote. A
  gated run refuses them and asks for a command instead.
- **`view`**: read files under the working directory, capped at 64 KB each.
- **A seeded first step** (#13325): in Coder, before the first call, the
  loop looks up what the task names. The files it mentions that exist under
  the working directory are viewed (the first four), and each code name it
  mentions is searched for (`git grep -n -w`, first 8 lines). The results
  are one step in the state, so the first call can already edit.
- A reply that says `finished` and also carries edits, commands or views is
  a working step: they run, and the run finishes in a turn of its own.
- Nothing else in v1: no network tools, no editing outside the working
  directory. Writes happen through commands, so they show in the transcript.

### Limits and endings

The user sets limits; Microcoder adds none of its own beyond safety caps on
output size. A run ends as one of:

| Ending | When |
| --- | --- |
| `finished` | The model says `finished` with a `reply` |
| `asked` | `ask: question`; the reply is the question |
| `bad_replies` | Three replies in a row that fail the schema |
| `no_capacity` | Every gateway in the plan refused or failed before the first token |
| `stopped` | The user pressed STOP |
| `idle` | Three steps with no commands, no views and no reply |
| `failed` | A journaled run (Coder) hit a limit, and the ending says which: 64 model calls, 128 commands and edits, 512 file views, or 15 minutes. Or the journal refused |

A journaled run that ends `failed` also lists what it leaves (#13351), so
the caller does not take a half-done tree for a result. The run records its
start commit (`git rev-parse HEAD`) before the first call. The ending's text
is the reason, then the commits the run made (`git log --oneline` from that
commit, the last 40) and the files left changed or untracked
(`git status --short`). Outside a git checkout it is the reason alone.

### What a commit sees

Before a `git commit`'s result goes into the state, the loop adds
`last_check`: how the last test or check the run ran before it ended
(#13351, `coax/lib/microcoder/commit.coax`). It is one of:

- `passed: CMD`
- `failed (exit N): CMD`, or `failed (killed at its time limit): CMD`
- `none since the last edit: ...`, when an edit was applied after that check
- `none: ...`, when no check ran in the run

The commit still runs. Which commands are checks is read from the shell
command: each simple command (split at `;`, `&&`, `||`, `|` and newlines,
not inside quotes) by its program and first argument that is not an option.
A check is `coax`, `cargo`, `go`, `make` and the like with `test`, `check`
or `mutate`; `pytest` or `ctest`; a script named `*test.sh`, `*tests.sh`,
`*check.sh` or `test*.sh`; `tool.sh *check`; or a Python test file. Leading
`VAR=value`, `env`, `time`, `nice`, `timeout N` and `remote.sh run --` are
read through. Only edits from `edits` make a check stale; a command's own
writes are not tracked.

### What the Lab shows

Each step is a row in the Lab transcript: the gateway and model, the
commands with their exit codes, and a fold with the output. The final `reply`
streams token by token, like the main chat: the gateway's text deltas go
through the same 120 ms commit and push (`CH_PUSHED`). The routing JSON blob
stays above the first step, and `harness_started` becomes `true`.

### Which model

Microcoder asks Mammoth which tier fits, from the route's difficulty and
effort, then maps it to a gateway plan:

| Tier | First choice | Fallbacks |
| --- | --- | --- |
| `fast` (trivial, easy) | Vertex `gemini-4-hydrogen` | Gemini API Hydrogen, OpenRouter, Z.ai `glm-5.3-flash` |
| `strong` (moderate and up) | Vertex Gemini Pro | OpenRouter (a strong open model), Anthropic or OpenAI direct |
| `local` (the user chose it) | psionic on this machine | none |

This follows Google-first: Vertex, then the Gemini API, then everything else.
Model names live in one table in `coax/lib/gateway/models.coax`, not in the
loop.

### Compiler questions and working memory

GitHub #13444 and #13445. Two modes, each off by default, so a run with one
can be compared with a run without. Off, the schema is `MC_SCHEMA`, the system
prompt is `MC_SYSTEM` and a run is what it was. The RLM coder has the same two
modes as `--intel` and `--memory` (`coax/lib/rlm/README.md`), over the same
libraries.

Turn them on in the worker's message (`coder/tui/mcworker.coax`):

```json
{"task":"...","cwd":"/path","intel":true,"memory":true,"memory_dir":"/optional/dir","coax":"/optional/coax"}
```

With `memory` and no `memory_dir`, the store is
`~/.openagents/coder/sessions/ID.workmem`, where ID is the message's `session`,
else the worker's own. After `ended`, the worker writes one line
`{"kind":"aware","intel":{...},"memory":{...}}` with the run's counts.

**`intel`**: the Coax compiler answers, through `coax/lib/agent/intel/`.

- The action gains `questions`, up to 16 lines: `defines NAME`, `uses NAME
  [FILE]`, `exports FILE`, `show NAME [FILE]`, `check FILE`, `explain CODE`,
  `vocab NAME`, `effects FILE [PROC]` and `depends FILE`. (The RLM coder's
  `checks(name)` is not here: with it, the checker stops `coder/tui/coder.coax`
  with E0381; see docs/coder/2026-11-13-compiler-for-agents.md.) Each
  answer is a result named `ask: ...`, bounded, and says how many records it
  left out. When the compiler can't answer, the result starts `no answer:`
  with exit 1 and the model uses `git grep` and `view`.
- A view ends with the file's hash, and `show` gives a declaration's. An edit
  may name that hash in `seen`; when the file is no longer what was read, the
  edit is refused as stale, nothing is written, and the result has the lines
  as they are now.
- An edit with `name` replaces that whole declaration (`ix_replace`).
- An applied edit of a `.coax` file returns the lines it wrote, the file's new
  hash and its check.

**`memory`**: the run store of `coax/lib/agent/workmem/`.

- Every view, edit, question and command is recorded as it happens. The
  action gains `notes`, up to 8 findings, each tied to lines when it starts
  `PATH:A-B`.
- The state gains `memory`: current findings, every change, and what is open.
  It is never compacted.
- A view of lines the store already has, unchanged, starts with a line that
  says so.
- The finish rule (`workmem/finish.coax`): `finished` is refused while a check
  the task quotes in backticks has not passed on the files as they are, or
  while the run has weakened what checks it (fewer `#test`, `#forall` or
  `#assert` in a file, a lost `#authority` line, a changed compatibility
  baseline). It is refused twice at most, then stands, and what was open goes
  into the report's `unproven` for a person.

Code: `coax/lib/microcoder/aware.coax` (the effects), with the pure parts in
`action.coax`, `loop.coax` and `state.coax`. Checks:

```sh
coax/bin/coax test coax/lib/microcoder/aware.coax
coax/web/tool.sh coax/lib/microcoder/aware-selftest.coax   # a scratch Coax repo, the real compiler, a scripted model
coax/web/tool.sh coder/tests/aware-worker.coax             # the worker turns the modes on from its message
```

## Part 2: the gateways

### The interface

`coax/lib/gateway/` holds one request and stream shape for every provider,
modelled on the old `crates/inference` Open Responses types, cut to what
Microcoder and `/chat` use:

```
GwRequest  { model, system, messages[], schema (optional), effort, max_tokens, stream }
GwEvent    = text_delta(s) | reasoning_delta(s) | done(usage) | refused(status, reason)
GwUsage    { input_tokens, output_tokens, cached_tokens, cost_msats }
Gateway    { name, url, key_env, scheme, translate(req) -> body, parse(line) -> GwEvent }
```

- **Transport.** The same curl child as `coax/lib/systemone/transport.coax`:
  the body goes on stdin, and the key is read by curl from the environment
  (`--variable`) so it never passes through Coax or a process listing. A
  streaming variant reads SSE lines from stdout as they come.
- **Translation.** Most providers take Chat Completions. One translator
  (`chat.coax`) serves OpenRouter, the Vercel AI Gateway, Z.ai, OpenAI,
  Vertex's OpenAI endpoint and psionic. Gemini's native API and Anthropic's
  Messages API each get their own.
- **Structured output.** `schema` becomes `response_format: json_schema`
  (strict) on Chat Completions, `responseSchema` on Gemini, and a single
  forced tool on Anthropic. A provider without strict schemas falls back to
  the old `text_action` parser, which takes the JSON out of plain text.
- **The plan.** Up to three (gateway, model) attempts. Move to the next one on
  a refusal or a failure before the first token; never after it, so the user
  never sees two half answers. A 401 or 402 benches that gateway for the
  session. This is the old router's rule.
- **Metering.** Every `done` carries usage. Cost comes from a rates card in
  `rates.coax`, and each attempt is logged (gateway, model, status, latency,
  tokens), never the prompt.

### Keys

- **On the Mac:** the user's own keys in the macOS Keychain, set in the
  app's settings per gateway, put in the curl child's environment only.
  None are needed for psionic.
- **During development:** the files under `~/work/.secrets/` (`openrouter.env`,
  `vertex-sa-inference.json`, `openagents-gemini-provider.env`), loaded the
  way `Router.loadKey` loads `baseten.env` today.
- **Ours, for users without keys:** through `coax/web/api` (`/api/v1/inference`),
  which holds our keys in Secret Manager and meters by account. The Mammoth
  key moves behind the same route before the Mac app ships.

### The migration plan

Each row is a task in the Coder project. Order: what Microcoder needs first.

| # | Gateway or piece | From | Into | Notes |
| ---: | --- | --- | --- | --- |
| 1 | Interface, SSE reader, curl streaming transport | `inference/{event,sse,stream,request,response}.rs`, `systemone/transport.coax` | `coax/lib/gateway/{gateway,sse,transport}.coax` | Tests from the recorded `.sse` fixtures |
| 2 | Chat Completions translation | `inference/chat/*`, `upstream/chat.rs` | `chat.coax` | The base for rows 4–8 |
| 3 | **Vertex (Gemini)** | `upstream/vertex.rs`, `microcoder-loop/vertex.rs` | `vertex.coax` | OpenAI-compatible endpoint, bearer from a service-account token minted per hour; strict `json_schema` |
| 4 | **Gemini API** | `upstream/google.rs`, `gemini.rs` | `gemini.coax` | Native `generateContent`/`streamGenerateContent`, `responseSchema` |
| 5 | **OpenRouter** | `crates/openrouter`, `upstream/openrouter.rs` | `openrouter.coax` | Chat Completions plus OpenRouter's usage and provider fields |
| 6 | Z.ai | `upstream/zai.rs` | `zai.coax` | `glm-5.3-flash` |
| 7 | Vercel AI Gateway | `upstream/vercel.rs` | `vercel.coax` | Fallback only |
| 8 | OpenAI and Anthropic direct | `upstream/{openai,anthropic,responses}.rs` | `openai.coax`, `anthropic.coax` | Anthropic needs the Messages translator |
| 9 | Plan, fallback and benching | `inference/router.rs`, `microcoder-loop/failover.rs`, `capacity.rs` | `plan.coax` | Fallback before the first token only |
| 10 | Rates card and meter | `inference/{rates,meter/*}.rs` | `rates.coax` | Attempt log, no prompts |
| 11 | Text-action fallback parser | `microcoder-loop/text_action.rs` | `coax/lib/microcoder/text_action.coax` | For providers without strict schemas |
| 12 | psionic local | `upstream/psionic.rs` | `psionic.coax` | Chat Completions on localhost |
| 13 | Pylon | `upstream/pylon.rs` | `pylon.coax` | Offline adapter over the text Pylon buyer (free 5050 jobs to a pinned GPT-OSS-20B host); live acceptance is gw-pylon-live |
| 14 | Our route `/api/v1/inference` | `crates/gateway` (BYOK, public, rates) | `coax/web/api` | Keys in Secret Manager; meter by account |

**Not carried over:** the door proxy (`pro.rs`), the `coder` upstream, x402
(not a Bitcoin rail) and the old repository adapters (Devin, Grok, OpenCode).
Claude Code and Codex are reached as harnesses through the Lab router, not as
gateways.

### Milestones

1. **Gateways v1:** rows 1–5 and 9, with tests on the recorded fixtures and
   one live smoke per gateway (`coax/lib/gateway/smoke.sh`, skipped without
   a key).
2. **Microcoder v1:** the loop, state, action schema and tools, with Lab
   `other` wired to it in the Mac app. Acceptance: "what is 2+2" answers in
   one step on Vertex flash; "how many Swift files are in this folder" runs
   one command and answers; STOP works mid-stream.
3. **Gateways v2:** rows 6–8 and 10–12.
4. **Ours:** row 14. Then the Mac app works without the user's own keys, and
   the Mammoth key leaves the app.

## Open questions for the owner

1. Do users bring their own Gemini or OpenRouter keys in the Mac app, or does
   `other` run on our keys (metered) from day one?
2. Should `other` ever start commands on the user's Mac, or should v1 answer
   only, with commands left to Claude Code and Codex?
3. Which strong model on OpenRouter is the default fallback?
