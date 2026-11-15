# One router: no agent, model or effort pickers

Written 2026-11-14. Status: specification, not built. Owner decision of the
same day; the owner settled the open questions that evening
([Decisions](#decisions)).

## The decision

You never choose an agent, a model or a reasoning level in OpenAgents. You
type what you want. One router decides what handles it, and the product shows
what it chose.

This is the idea from [Episode 289](../transcripts/289.md) at 1:00: chat
messages "mapped to the routes" by a typed decision model, with "everything
behind this single agent". The decision model is Google's Gemini Decision 1,
through our Decisions API.

The owner's words: "I don't want any options like that. I don't want to think
about that."

What this removes from every surface:

- The agent or harness choice (Claude Code, Codex, Cursor, Antigravity,
  Microcoder, Psionic).
- The model choice (Opus, Sonnet, Haiku, Fable, GPT, Gemini and the rest).
- The effort or reasoning-level choice.

What stays, because it is not a choice about how the work is done:

- Which accounts you have connected (Settings, Usage). The router reads them.
- Which computer or cloud runs the work, where you already name it by opening
  that computer's conversation.
- What a conversation may touch (access and approvals).

## What exists today

The repository already has most of the router. It has no single owner of the
decision, and every surface still shows pickers.

### Routers

| Where | What it decides | Notes |
| --- | --- | --- |
| `coax/lib/agent/engine/route.coax` | Engine (`claude`, `codex`, `micro`, `psionic`), difficulty (5 levels), effort (`low`, `medium`, `high`) | "The router every agent owner shares" ([Coder book chapter 13](../../coder/book/ch013-choose-an-engine.md)). Gemini Decision 1 first, then Elephant, then Mammoth. Writes a `coder.route.v2` record. |
| `coax/lib/agent/engine/route-policy.coax` (`#authority`) | The Claude model from difficulty: trivial to Haiku, easy and moderate to Sonnet, hard and very hard to Opus. When a turn reroutes. Which decision URLs are allowed. | Codex, Microcoder and Psionic keep their configured model. |
| `coax/lib/agent/engine/availability.coax` and its policy | Which engines routing may pick: installed, allowed, not in a provider cooldown | A typed local observation, no probe. |
| `apps/openagents-desktop/router.coax` | Harness (`claude_code`, `codex`, `other`), difficulty, effort (5 levels), Claude model | The Lab's copy. Same labels, different names and a different effort scale. It shows the decision and runs nothing. |
| `coder/tui/router.coax` | Whether an incoming event asks for work, and which Worker gets it | A different job (triggers and threads, Coder book chapters 17 and 20). Not replaced by this spec. |

Measured so far: the labelled set is 44 messages (22 to tune, 22 held out,
`coder/tui/route-labels.tsv`). Mammoth routes 22 of 22 on each half and
Gemini Decision 1 routes 44 of 44. Chapter 13 says what that does not show:
it measures agreement with a routing policy, not which engine writes the
better patch, and 22 correct labels do not establish calibration.

### Pickers to remove

| Surface | Control today | Where |
| --- | --- | --- |
| Web chat (`/chat`) | A model menu: Opus, Sonnet, Haiku | `apps/openagents-web/chat.coax` (`WC_MODELS`) |
| Web chat, signed-in page | A model select with 11 hosted models, and a model per local agent | `apps/openagents-web/chat-web.coax` (`CW_MODEL_IDS`, `CW_LM_IDS`) |
| Desktop | The harness and model chip, the effort menu, the mode per conversation. The Lab mode already says "Elephant decides". | `apps/openagents-desktop/zeron/modes.coax`, `screen.coax`, `settings.coax` |
| Phone, Your computers | Model and effort menus (`env.model`, `env.effort`), added in 5e86e8ceb7 | `apps/shared/environment-app.coax`, `coax/lib/agent/environment/wire.coax` |
| Terminal (Coder) | Shift-Tab cycles agents; `/engine`, `/model`, `/effort`; `--engine`, `--model`, `--effort` | `coder/tui/coder.coax` |

The Metaverse Engine desktop already replaced its agent, model and effort
dropdowns with the shared router on 2026-11-04 (chapter 13, "Local Elephant
and desktop routing"). That is the pattern for the rest.

## Research

### Gemini Decision 1

From [our note of 2026-11-13](../research/2026-11-13-gemini-decision-1.md)
and `coax/lib/systemone/`:

- It answers typed questions (yes/no with a probability, a choice among 2 to
  255 options, a score over levels) and writes no output tokens.
- One request holds up to 1,000 questions and about 262,144 tokens of input,
  including images, audio, video and PDFs.
- It costs $0.10 per million input tokens. Coder's engine route is about 520
  tokens, so a route costs about $0.00005. Twenty thousand routes cost a
  dollar.
- Tier 1 allows 1,500 requests a minute and 50,000 a day.
- It takes no conversation turns: input is a flat list of blocks, so earlier
  messages go in as text.
- Since 2026-11-13 (#13473) it is the first door for every decision caller,
  with the caller's earlier model as the fallback.

A route decision is cheap enough to make on every message. Cost is not the
constraint. Latency and correctness are.

### Outside prior art

- **RouteLLM** (LMSYS and UC Berkeley, ICLR 2025) trains a small router on
  human preference data to choose between a strong and a weak model for each
  query. Its paper reports cost cuts of more than two times with no
  noticeable quality loss on public benchmarks. The lesson for us: a router
  learns from outcomes (which answer people preferred), not from labels a
  developer wrote.
  ([LMSYS](https://lmsys.org/blog/2024-07-01-routellm),
  [paper](https://proceedings.iclr.cc/paper_files/paper/2025/hash/5503a7c69d48a2f86fc00b3dc09de686-Abstract-Conference.html))
- **OpenRouter's Auto Router** picks a model per prompt from a curated pool,
  charges only the chosen model's rate, and keeps a conversation on one model
  ("session stickiness") because switching mid-conversation gives
  inconsistent behaviour and wastes the prompt cache. Its docs now mark the
  first version deprecated in favour of a newer router.
  ([docs](https://openrouter.ai/docs/guides/routing/routers/auto-router.md),
  [model routing](https://openrouter.ai/docs/guides/features/model-routing))
- **OpenAI's GPT-5 launch** (August 2025) replaced the model picker with a
  real-time router. Users reported the router sending hard questions to the
  weaker path, OpenAI said it had been underperforming, and within days the
  picker came back with Auto, Fast and Thinking settings and the older
  models. Later reporting says the router was removed for free users.
  ([Implicator](https://www.implicator.ai/openai-backs-down-legacy-models-return-after-gpt-5-backlash/),
  [The Decoder](https://the-decoder.com/openais-gpt-5-router-rollback-shows-why-ai-requires-unlearning-old-habits/))

What the OpenAI case teaches, since we are making the same move:

1. A router that is invisible is distrusted. People could not tell what
   answered, so every weak answer was blamed on the router. We show what ran,
   every time.
2. A router that saves the provider money at the user's expense is resented.
   Ours picks for the task. When unsure it picks the stronger option.
3. People need a way out when the router is wrong. Ours is plain words in the
   message ("use Codex for this", "think harder"), which the router reads as
   part of the request. It is not a menu.

## Design

### One module

`coax/lib/agent/engine/route.coax` is the only router. Every surface calls it
(the server for hosted chat, the computer's resident service for work on a
computer). `apps/openagents-desktop/router.coax` is deleted and the Lab calls
the shared module, so one set of labels, one effort scale and one labelled set
remain ([reuse rule](../reuse.md)).

The router is a decision, never a keyword rule. The workspace rule already
says so: no string matching for intent or tool selection. A person naming an
agent in their message is a typed question to the decision model (below), not
a search for the word "codex".

### One request, several typed questions

Each message sends one Decisions request. The questions:

| Key | Type | Answers | Used for |
| --- | --- | --- | --- |
| `route` | choice | The 289 routes: answer directly; answer from the knowledge base; work on a repository or computer; a money action (wallet); show something we have (a deck, a page, a world); something we can't do yet | What kind of thing this is |
| `engine` | choice | The eligible engines only, each with its label | Which agent does the work |
| `difficulty` | score | trivial, easy, moderate, hard, very hard | The model profile |
| `effort` | choice | The effort levels the picked engine takes | Reasoning level |
| `named_engine` | choice | none, or one of the engines | The person asked for a specific agent |
| `named_depth` | choice | none, faster, more careful | The person asked for speed or care |
| `continue` | yes/no | | This message continues the same piece of work as the conversation so far |

`route` is new. The rest exist or are small additions. Asking them together
costs one request, since Gemini Decision 1 takes many questions at once.

The `route` options come from a registry, not a list in the router's source.
A capability adds its route with a label that says what it is for, the way a
289 plugin joins the agent. The first registry holds what the product has
today. "Something we can't do yet" is the door to 289's "do you want to build
it as a plugin?" and that flow is its own spec.

### What the router is told

The state is one JSON object, as today, with more in it:

- The message, in full up to a bound, and its attachments as image or PDF
  blocks.
- A bounded summary of the conversation so far (today: 256 bytes of recent
  context for Elephant; Gemini has room for more).
- The surface (web, phone, desktop, terminal) and where the work would run
  (hosted, this computer, a named computer, a cloud box).
- Each engine's state: installed, logged in, in a cooldown, and whether its
  quota is used up when the usage reader knows
  (`coax/lib/agent/accounts/`).
- Whether a local model server is running, and whether the person asked to
  keep work on this machine.
- The engine and model the conversation is already using, if any.

No credential, token or file content goes to the router.

### Code decides what is possible; the model decides among it

This split exists today and stays:

1. **Eligibility is code** (`availability-policy.coax`, `#authority`). An
   engine is eligible when it is installed or hosted, the account is logged
   in, it is allowed, and it is not in a provider cooldown.
2. **The decision model chooses among eligible engines.** An answer naming an
   ineligible engine cannot dispatch.
3. **Below a probability of 0.5, or with no decision, the default runs.** The
   default is the most capable eligible engine, so an unsure router errs
   toward quality. (Today it is a fixed order starting with Claude Code.)
4. **A named engine wins when it is eligible.** When it is not, the router
   picks otherwise and the result line says why ("Codex isn't logged in on
   this computer").
5. **An account out of quota refuses.** It is not eligible, and nothing is
   quietly downgraded to keep going. When another eligible engine exists, it
   takes the message and the line says why. When none does, the message is
   refused with what ran out and when it resets ("Claude's weekly limit is
   used up; it resets Thursday at 9:00 AM"), and the text stays in the
   composer.

### Model and effort

The engine decides which models exist. The router picks a profile from
difficulty, through one policy table in `route-policy.coax`:

| Engine | Trivial | Easy, moderate | Hard, very hard |
| --- | --- | --- | --- |
| Claude Code | Haiku | Sonnet | Opus |
| Codex | its small model | its default model | its strongest model |
| Hosted chat | Gemini's fast model | Gemini's default model | Opus or Gemini's strongest model |
| Microcoder, Psionic | the one model each has | | |

The Claude row is today's policy. The Codex and hosted rows are new and need
model names filled in from each provider's live list when built, not from this
document.

Google-first applies to hosted chat: Vertex Gemini models answer first and
other providers are fallbacks, unless the task is coding work that a coding
agent takes.

Effort comes from the `effort` answer, clamped to what the picked model takes
(the phone's environment code already knows which models take an effort).
`named_depth` moves it one step. There is one effort scale across the
product: `low`, `medium`, `high`, with a model's higher levels reachable only
through "more careful" on a hard task. The Lab's five-level scale goes away
with its copy of the router.

### When the router runs

- **Every message** gets a decision. It costs about $0.00005.
- **The engine is chosen at a conversation's first message and kept** while
  `continue` is yes. An engine's session holds the conversation's context, and
  switching engines mid-task throws it away (OpenRouter keeps sessions sticky
  for the same reason).
- **Model and effort follow each message** within the engine, when the engine
  can change them between turns. A hard follow-up in an easy conversation gets
  the stronger model.
- **The engine changes mid-conversation only when** the person names another
  one, the current one becomes ineligible, or `continue` is no and the route
  differs. On a change, the new engine receives the conversation's transcript.
- **A failed turn is never resent to another engine automatically.** It may
  have had effects. This is today's rule and it stays.

The decision adds a network round trip before the first token. Budget: the
route answers within the existing eight-second limit, and the surface shows
the message as sent at once. If the decision is late, the default runs and the
line says so. Measure the added delay per surface before removing each
picker, and record it.

### Doors and the daily allowance

**All routing goes through Google for now** (owner, 2026-11-14). Gemini
Decision 1 answers every route on every surface, for hosted chat and for work
on your own computer alike. There is no question to the person first and no
per-computer setting.

- A request rule or a safety block from Gemini is final.
- When Gemini does not answer (a rate limit, a server error, no network, the
  allowance below used up), the default engine runs and the line on the answer
  says routing was unavailable. The router does not ask another decision model
  in this version. The Elephant and Mammoth doors stay in
  `coax/lib/systemone` for the callers that use them; the product router does
  not depend on them.
- The message, a bounded summary of the conversation and the environment
  object go to Google. No credential, token or file content does. The privacy
  page says so in one plain sentence.

**Start with 10,000 decisions a day**, across the whole product, and raise it
as use grows.

- 10,000 a day is also the daily limit of Google's free tier. That tier allows
  200 requests and 50,000 tokens a minute, which is about 90 routes a minute
  at 520 tokens each. If the minute limit binds before the daily one, move to
  the paid tier: 10,000 routes cost about $0.52 a day there, and Tier 1 allows
  1,500 requests a minute and 50,000 a day.
- The server counts decisions for hosted chat. A computer's resident service
  counts its own and shares the same key and total through the Decisions API.
- The allowance is shown, not hidden: Settings, Usage has a line with today's
  count and the limit. When it runs out, answers still come, from the default
  engine, and each says routing was off for the rest of the day.

### What you see

No picker, anywhere. In its place, one quiet line on each answer saying what
ran:

```
Claude Code · Opus · high
```

- It is a fact after the decision, not a control. It does not open a menu.
- When the router fell back, the line says why in plain words: "Codex is
  resting until 3:10 PM, so Claude Code took this."
- Nothing a person reads says "route", "door", "engine" or "probability".
  Those words stay in records.
- The line replaces the per-answer model label the web chat already shows
  ("Opus 5.5 · OpenAgents key · $0.0123"); cost and credential stay.

The way to steer is to say so: "do it with Codex", "use the big model",
"quick answer". That is the 289 demo ("Do it with Codex").

The terminal keeps `--engine`, `--model` and `--effort` as flags for scripts
and tests, because a script needs a fixed run. They are not shown in the
interactive screen. `/engine`, `/model`, `/effort` and Shift-Tab are removed
from the interactive UI, and automatic routing becomes its only behaviour.

### The record

Extend the route record to `coder.route.v3`
(`proto/openagents/coder/route/`), one line per decision, on every surface:
the route, engine, model, effort, the probabilities, the eligible and blocked
sets, the door that answered, whether the person named an engine, what ran,
and why when that differs from the decision. No message text.

The record is what makes the router improvable. Without it, removing the
pickers removes our only signal of what people wanted.

### Getting better

The labelled set has 44 messages. That is enough to launch engine routing for
coding and not enough for a product with no way out.

1. **Grow the labelled set** to several hundred, covering every route and
   every surface, with a held-out half. Keep `coder/tui/route_eval.py` as the
   gate: a change to labels or questions must not lower held-out agreement.
2. **Learn from corrections.** A message that names an engine or asks to think
   harder right after an answer is a label: the router's last pick was wrong
   for that person. Collect these from the records (not message text: the
   decision and the correction).
3. **Measure outcomes, not agreement.** The
   [repository pilot](../coder/repository-pilot.md) compares fixed engines
   with automatic routing on the same tasks. Routing ships to a surface when
   automatic routing is no worse than the best fixed engine there.
4. **Check calibration.** A probability of 0.9 should be right nine times in
   ten. Until measured, treat the 0.5 threshold as a guess.

## Rollout

Each step removes one picker and ends in something a person can use.

1. **One router.** Fold the Lab's copy into `route.coax`. Add `route`,
   `named_engine`, `named_depth` and `continue`. Add the route registry with
   today's capabilities. Write `coder.route.v3`. Grow the labelled set.
2. **Terminal.** Automatic routing is the default and only interactive
   behaviour. Remove Shift-Tab cycling and the three slash commands. Flags
   stay.
3. **Desktop.** Remove the harness, model and effort chips and the mode per
   conversation. Every conversation is what the Lab mode is now, with work
   actually running.
4. **Phone.** Remove `env.model` and `env.effort`. The computer's resident
   service routes each turn. The wire fields for model and effort stay in the
   protocol as what the service reports, not what the phone requests.
5. **Web chat.** Remove both model menus. The server routes hosted turns; a
   connected computer routes its own.
6. **Everything behind it.** Add routes for the wallet, decks, worlds and
   Projects, so "send a thousand sats" and "show the deck" work from any
   conversation, as in 289. Then the "can't do yet" route and plugins.

Steps 2 to 5 are independent after step 1. Each needs its measured delay and
its outcome comparison before the picker goes.

## Decisions

Settled by the owner on 2026-11-14.

| # | Question | Decision |
| --- | --- | --- |
| 1 | Do terminal flags (`--engine`, `--model`, `--effort`) stay? | Yes, for scripts and tests only; never in an interactive screen. |
| 2 | When unsure, which engine runs? | The most capable eligible one. |
| 3 | May the router switch engines mid-conversation on its own? | Only when the current engine can't run, or the message starts different work. Otherwise it keeps the engine. |
| 4 | Routing through Google for work on your own computer: ask first? | No. All routing goes through Google for now, on every surface, with no question and no local decision model in the path. |
| 5 | Does spend shape the choice? | An account out of quota refuses. Nothing is downgraded to save money. |
| 6 | Which Codex and hosted models fill the profile table? | Taken from each provider's live model list at build time; reviewed when a provider ships a model. |
| 7 | Where does the server-side router run for hosted chat? | In the chat API, through the Decisions API. |
| 8 | How many decisions to start? | 10,000 a day across the product, shown in Settings, raised as use grows. |

Still open: whether the 10,000 is one total for everyone or becomes a share
per account once many people use it. This spec starts with one total.

## Not in this spec

- The trigger router (`coder/tui/router.coax`), which decides whether an
  outside event asks for work.
- Plugins, the registry and paying authors (289 at 1:30 to 2:30).
- Which computer or cloud runs a task. Placement is a second routing question
  and needs its own spec; this one takes the place as given.
- Routing between Pylon providers.

## Sources

Read for this spec: `docs/transcripts/289.md`,
`docs/audits/2026-11-10-episode-289-port-audit.md`,
`docs/research/2026-11-13-gemini-decision-1.md`,
`coax/lib/systemone/README.md`, `coder/book/ch013-choose-an-engine.md`,
`coax/lib/agent/engine/route.coax`, `route-policy.coax` and
`availability.coax` (their headers and tables), `coder/tui/router.coax`
(header), `apps/openagents-desktop/router.coax`,
`proto/openagents/coder/route/v2/route.proto`, and the picker code named in
the table above (by search, not read in full). The outside sources are linked
where cited; their figures were not checked against the primary papers beyond
what is quoted.
