# Microcoder in Coax, and the inference gateways

Written 2026-10-28. Project: [Coder](https://openagents.com/projects/coder).

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
  actions[]     // the last 12 steps: rationale, commands, exit codes,
                //   output head and tail (6,000 bytes each)
  files[]       // what `view` asked for, capped
  notes         // what the loop writes for itself between steps
}
```

The prompt is the state as JSON, plus a short system prompt that names the
schema, the tools and the limits. The model never sees a key, a path under
`.secrets`, or another conversation.

### Tools in v1

- **`commands`**: run in the working directory through
  `coax/platform/process-native.c`, with a timeout per command (30 s) and the
  output capped. With `local_work` under 0.5 the loop is told not to run
  commands, and any it returns are refused with a note in state.
- **`view`**: read files under the working directory, capped at 64 KB each.
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
| `fast` (trivial, easy) | Vertex `gemini-3.8-flash` | Gemini API flash, OpenRouter, Z.ai `glm-5.3-flash` |
| `strong` (moderate and up) | Vertex Gemini Pro | OpenRouter (a strong open model), Anthropic or OpenAI direct |
| `local` (the user chose it) | psionic on this machine | none |

This follows Google-first: Vertex, then the Gemini API, then everything else.
Model names live in one table in `coax/lib/gateway/models.coax`, not in the
loop.

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
| 13 | Pylon | `upstream/pylon.rs` | `pylon.coax` | After Pylon's own Coax client |
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
