# Send to first reply, before and after the Jev first response

Issue [#9920](https://github.com/OpenAgentsInc/openagents/issues/9920) asks
the phone's Coder tab to answer very fast, with Jev (TypeSafe System One)
making the immediate typed judgment. This page traces where the time went,
what changed, and what the phone should use.

## How it was measured

The production chat worker is not deployed yet (an owner step in
`NEEDS_OWNER.md`), so a stand-in `coder-worker` ran on a development Mac with
a fresh worker key, against the production relay `wss://relay.openagents.com`,
on the chat worker's configuration: quota mode, the gateway door's `gemini`
lane (`google/gemini-3.8-flash`), 8 jobs at once, and, after the change,
`TYPESAFE_API_KEY` for the judge (`jev-latest` at `https://api.typesafe.ai`).

The client is `crates/coder/examples/first_reply.rs`. It does what the app's
basic Coder does (`crates/openagents-mobile/src/basic_coder.rs`): a fresh key,
a fresh relay connection per turn (connect and NIP-42), `REQ`, wait for
`EOSE`, publish the `25900` request with the app's instructions, then time
each answer. All times are milliseconds from the start of the turn.

```sh
FIRST_REPLY_WORKER=<worker hex> cargo run -p coder --example first_reply -- "<message>" 3
FIRST_REPLY_RANK=1 FIRST_REPLY_WORKER=<worker hex> cargo run -p coder --example first_reply -- "<conversation>" 3
```

## Before: the model's first token was the first reply

`origin/main` at `38aeec3b19`, "In one sentence, what is a closure in Rust?",
five runs:

| Step | ms |
| --- | --- |
| Relay connect and NIP-42 auth | 224–266 |
| `REQ` answered with `EOSE` | 274–329 |
| First answer of any kind (the model's first partial) | 3,362–4,658 |
| Result | 3,445–4,916 |

Where the time goes: about 0.3 s is the phone's own relay setup, about 0.15 s
is the relay round trip to the worker and back, and everything else, 3.0 to
4.3 s, is the model's time to first token. The worker adds nothing measurable:
admission is immediate and jobs run concurrently, with no queue.

## After: Jev answers first, beside the model

The worker now (`crates/coder/src/bin/coder-worker.rs`, `crates/coder/src/first.rs`):

1. Sends `status: processing` the moment it admits a turn (after the
   allowlist, quota, staleness, and capacity checks, which are unchanged).
2. For a turn that asks with `"opener": true` (the bench sends it; the app
   needs to add it, see below), starts the model call and, in parallel, one
   System One request with three
   independent questions over the same state: `action` (Classify's measured
   route question, word for word), `lane` (`chat` or `computer`), and
   `opener` (which of 21 short openers fits, such as "I'll look into that
   now.", "Let me check on that.", "That needs your computer.", or `none`).
   The judge's call is one attempt with a 2.5 s budget; it never delays the
   model.
3. Sends the typed `judgment` feedback when it arrives, and, if the model has
   not started yet, the argmax opener as partial `seq` 0. The result's text
   begins with the same opener, so the phone's result-replaces-preview rule
   shows the same words.
4. Keeps the door's and the judge's HTTPS connections warm (an unbilled
   `GET /v1/models` at start and every 45 s).

The model is told once, in a line the worker appends to the caller's
instructions, not to open with an acknowledgement of its own.

The first response is opt-in because the same worker is Coder's cloud
fallback (`crates/coder/src/cloud.rs`): each Microcoder step there must come
back as exactly one JSON object, and an opener in front of it would break the
step. A turn that asks for neither `opener` nor `judge` spends no judgment and
gets the model's text unchanged; a worker test pins that.

Same message, same relay, same lane:

| Step | ms |
| --- | --- |
| Relay connect and NIP-42 auth | 214–265 |
| `REQ` answered with `EOSE` | 273–325 |
| `processing` acknowledgement | 424–505 |
| Judgment (the worker logged Jev at 150–192 ms) | 570–670 |
| **Opener shown (partial `seq` 0)** | **589–747** |
| Result | 3,238–5,183 (unchanged: the model's time) |

Across four kinds of message, three runs each, the opener arrived in 598 to
747 ms every time, against 3.4 to 4.7 s for the first words before. What the
judge chose:

| Message | Opener | Verdict and lane |
| --- | --- | --- |
| "In one sentence, what is a closure in Rust?" | "Here's how that works." / "Sure." | respond, chat |
| "Fix the flaky test in crates/coder and open a PR" | "That needs your computer." / "On it." | clarify or respond, computer |
| "hey" | "Hi!" | respond, chat |
| "thanks, that's all!" | "You're welcome!" | end_conversation, chat |
| "Why is my Worker returning 502 after deploy?" | "Let me try to reproduce that." | respond, chat |

A `rank` job over three suggestions answered in 599 to 1,358 ms (the first
one on a cold judge connection) and ranked the repository the conversation
named first.

The worker tests pin the ordering without the network: a judge that answers
before the model yields `processing`, `judgment`, the opener as `seq` 0, and a
result that starts with it; a judge slower than the model changes nothing
about the reply or its timing; `judge: true` alone keeps the judgment and
drops the opener; a turn that asks for neither gets no judgment and the
model's text unchanged; and `rank` orders candidates, refuses `unavailable` with no judge
and `malformed` with bad candidates.

### v2: prepared answers, and no filler

The first set chose "Sure." for "Who are you?", which answered nothing. The
set is now `coder-first-response-v2` (`crates/coder/src/first.rs`):

- The assistant speaks as OpenAgents, in the plural. Every canned line is
  tested for first-person singular pronouns, and the model gets the same rule
  in `MODEL_NOTE`.
- A new `answer` question picks from the `chat-answers-v1` bank (the ids and
  rules of [the chat router design](../design/2026-09-28-chat-router.md)):
  `meta.who`, `meta.model` (its model and host slots come from the worker's
  door, and it is not offered when the worker cannot name them),
  `meta.capabilities`, `meta.limits_chat`, `meta.coder`, `meta.github`,
  `meta.open_source`,
  and `smalltalk.hello`, `.how_are_you`, `.test`, `.thanks`, `.bye`. There
  is no pricing or privacy answer until a tested invariant backs one.
- A Noul, `needs_specifics`, asks whether a good reply must refer to what the
  user named.
- Code decides: an answer at p ≥ 0.80 with `needs_specifics` < 0.30 is the
  whole reply (partial `seq` 0, the same text as the result, `model:
  "bank:chat-answers-v1"`, and the model call dropped); otherwise an opener
  at p ≥ 0.70; otherwise nothing before the model's words.
- Openers are six lines that say what kind of answer is coming: "Here's how
  that works.", "Here's how the options compare.", "Here's a plan.", "Here's a
  draft.", "Here's the short version.", "Sorry about that." "Sure.", "On it.",
  "Hi!", "Good question.", and the "I'll …" and "Let me …" lines are gone.

The live judge (`jev-latest`) on 35 first messages, from
`cargo test -p coder --lib first::tests::live_first_response_eval -- --ignored --nocapture`
with `TYPESAFE_API_KEY` set, 2026-09-28:

| Message | Lane | Tier | Shown first | answer (p) | specifics | opener (p) | ms |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Who are you? | chat | canned | meta.who (whole reply) | meta.who (0.99) | 0.06 | none (0.99) | 261 |
| what are you | chat | canned | meta.who (whole reply) | meta.who (0.98) | 0.06 | none (0.97) | 143 |
| What model are you? | chat | canned | meta.model (whole reply) | meta.model (0.87) | 0.06 | none (0.99) | 153 |
| Are you ChatGPT? | chat | canned | meta.model (whole reply) | meta.model (0.98) | 0.09 | none (0.99) | 183 |
| which LLM is this | chat | canned | meta.model (whole reply) | meta.model (0.94) | 0.13 | none (0.92) | 175 |
| What can you do? | chat | canned | meta.capabilities (whole reply) | meta.capabilities (0.99) | 0.06 | none (0.92) | 187 |
| Can you code? | chat | canned | meta.capabilities (whole reply) | meta.capabilities (0.99) | 0.07 | none (0.94) | 150 |
| can you see my files? | chat | canned | meta.limits_chat (whole reply) | meta.limits_chat (0.97) | 0.14 | none (0.75) | 189 |
| What is Coder? | chat | canned | meta.coder (whole reply) | meta.coder (0.96) | 0.11 | explain (0.79) | 175 |
| Are you open source? | chat | canned | meta.open_source (whole reply) | meta.open_source (0.97) | 0.09 | none (0.95) | 165 |
| hi | chat | canned | smalltalk.hello (whole reply) | smalltalk.hello (1.00) | 0.06 | none (1.00) | 146 |
| Hello! | chat | canned | smalltalk.hello (whole reply) | smalltalk.hello (1.00) | 0.05 | none (1.00) | 146 |
| hey how are you | chat | canned | smalltalk.how_are_you (whole reply) | smalltalk.how_are_you (1.00) | 0.06 | none (1.00) | 155 |
| test | chat | canned | smalltalk.test (whole reply) | smalltalk.test (0.99) | 0.09 | none (1.00) | 152 |
| thanks! | chat | canned | smalltalk.thanks (whole reply) | smalltalk.thanks (0.99) | 0.07 | none (1.00) | 141 |
| Thank you, that helped | chat | canned | smalltalk.thanks (whole reply) | smalltalk.thanks (0.99) | 0.09 | none (1.00) | 162 |
| bye | chat | canned | smalltalk.bye (whole reply) | smalltalk.bye (1.00) | 0.05 | none (1.00) | 155 |
| Fix my repo | computer | model | nothing | none (0.96) | 0.72 | none (0.93) | 146 |
| Fix the failing test in crates/coder and open a PR | computer | model | nothing | none (0.98) | 0.96 | none (0.81) | 140 |
| Explain how Nostr relays work | chat | opener | "Here's how that works." | none (1.00) | 0.09 | explain (1.00) | 181 |
| What's a closure in Rust? | chat | opener | "Here's how that works." | none (0.98) | 0.05 | explain (0.94) | 222 |
| Should I use Postgres or SQLite for a small app? | chat | opener | "Here's how the options compare." | none (1.00) | 0.18 | compare (0.99) | 172 |
| Write a commit message for a change that adds retries to the relay client | chat | opener | "Here's a draft." | none (0.99) | 0.81 | draft (0.99) | 209 |
| Plan a migration from REST to gRPC for our API | chat | opener | "Here's a plan." | none (1.00) | 0.32 | plan (1.00) | 164 |
| How much does this cost? | chat | model | nothing | none (1.00) | 0.53 | none (0.99) | 220 |
| Is this free? | chat | model | nothing | none (1.00) | 0.24 | none (0.97) | 263 |
| Do you store my chats? | chat | model | nothing | none (0.97) | 0.10 | explain (0.51) | 172 |
| That answer was wrong | chat | opener | "Sorry about that." | none (1.00) | 0.53 | sorry (0.98) | 137 |
| Why does my build fail on CI but not locally? | chat | model | nothing | none (0.99) | 0.28 | none (0.73) | 152 |
| Can you work on my Rails app? | chat | model | nothing | meta.limits_chat (0.37) | 0.32 | none (0.93) | 165 |
| summarize this: Rust ownership means each value has one owner, and when the owner goes out of scope the value is dropped. | chat | opener | "Here's the short version." | none (0.99) | 0.30 | summary (0.98) | 167 |
| who made you | chat | canned | meta.who (whole reply) | meta.who (0.90) | 0.07 | none (0.97) | 141 |
| Connect to my GitHub | computer | model | nothing | meta.github (0.96) | 0.37 | none (0.80) | 175 |
| Look at my repo | computer | model | nothing | meta.limits_chat (0.56) | 0.75 | none (0.98) | 167 |
| Open a PR for this | computer | model | nothing | none (0.93) | 0.76 | none (0.89) | 167 |

Every identity, model, capability, and small-talk message got the right
prepared answer at 0.87 or more; every request for work, pricing, privacy, or
a specific project got no prepared answer, so the model answers them. The
requests to connect GitHub, look at a repository, or open a pull request
are judged `lane: computer`, which is what raises the app's Coder action;
the lane's options now name GitHub and other accounts, cloning, and running
code explicitly. "Connect to my GitHub" picks `meta.github` at 0.96 but
`needs_specifics` is 0.37, over the 0.30 ceiling, so the model answers it
(under the app's instructions, which say GitHub work goes through Coder on a
connected computer). A rewording of `needs_specifics` tried to admit it
raised specifics on pricing, planning, and CI questions too and was not
kept. No canned line names a button or screen: the app shows the right
action itself.

### What is left on the phone's side

About 300 of the remaining 600 ms is the app opening a new relay connection
for every turn: connect, NIP-42, `REQ`, `EOSE`. A connection opened and
authenticated when the Coder tab opens, kept for the session, with the
subscription placed before the request, would put the opener at about 300 ms.
That is `crates/openagents-mobile` work.

### A finding on the way

With minimal instructions, Gemini answers "Fix the flaky test in crates/coder
and open a PR" by trying to call a function, and the turn fails
`MALFORMED_FUNCTION_CALL` even with `tools: []` and `tool_choice: none`. With
the app's real instructions, which say the chat cannot run commands and to tap
Run Coder, it answered every time. A client that sends its own instructions
should keep that sentence.

## The computer-backed path (NIP-HOST over CJ `25920`)

Traced in code; no host was run end to end for this page.

1. The phone publishes a NIP-HOST `task.create` wrapped in a CJ execution
   request (`25920`) to its host's key. The host answers it on one
   subscription per relay (`crates/coder-host/src/serve/cj.rs`).
2. That loop handles requests one at a time: each `host_request` is awaited
   before the next frame is read.
3. The loop reconnects every 110 s. Execution kinds are ephemeral, so a
   request published during the gap is lost and the phone waits out its 12 s
   timeout before retrying. That is the largest latency cliff on this path.
4. `task.create` reaches `coder::task::remote::Inbox::create`: one durable
   store write. Timed locally over eight creates: 13 to 84 ms, dominated by
   the disk sync.
5. With the owner's auto-start policy on, the task is journaled as eligible
   and a sweep starts on a new thread; the sweep reads the whole auto-start
   journal, may probe provider usage over the network (off by default), and
   launches the owner process, whose engine then makes its first model turn.
6. The phone's reply to `task.create` is a receipt. The first words it can
   show come from the run's activity, after the launch and the first model
   turn: seconds at best.

`coder::first` is the piece this path can share: the host can ask the same
judgment when a task is created and hand the opener, `lane`, and ranking back
with the receipt, so the phone shows a first line in the judge's time while
the run starts. That change is in `coder-host`'s reply shape, which other
work owns right now; the recommended next steps for it are a persistent CJ
subscription that overlaps the old connection before closing it (no gap), and
concurrent handling of requests.

## What the phone uses

On the conversation family (`25900` / `27000` / `26900`), all additive, all
specified in `nips/openagents/NIP-CJ.md`:

- **Ask for it: add `"opener": true` to the `25900` payload**
  (`basic_coder::payload` in `crates/openagents-mobile`). That is the one
  change the app needs. The opener then arrives as partial `seq` 0 and the
  result begins with it, which the current app already renders. (`"judge":
  true` instead asks for the typed judgment without an opener.)
- **`status: processing`** (27000) right after admission. The app can show
  that Coder is working; today it only counts as having heard the worker.
- **`judgment`** (27000):

  ```json
  {"v": 2, "requires": [], "type": "judgment",
   "verdict": "respond", "line": "That needs your computer.",
   "set": "coder-first-response-v1", "lane": "computer",
   "opener": "computer", "confidence": 0.81}
  ```

  `lane: "computer"` is the typed signal to raise the Run Coder / connect a
  computer action for this turn. `verdict` is one of `respond`, `clarify`,
  `end_conversation`, `unrouted`.
- **`rank`** for the suggestions above the input. Request:

  ```json
  {"v": 2, "requires": [], "type": "rank", "draft": "",
   "transcript": [{"role": "user", "content": "…"}],
   "candidates": [{"id": "openagents", "label": "OpenAgentsInc/openagents"},
                  {"id": "new_chat", "label": "Start a new chat"}]}
  ```

  Result (26900):

  ```json
  {"v": 2, "type": "result", "text": "openagents", "model": "jev-…",
   "set": "coder-first-response-v1",
   "ranked": [{"id": "openagents", "p": 0.7}, {"id": "new_chat", "p": 0.2}]}
  ```

  A rank job is admitted and metered as a turn (per key: 6 a minute, 40 a
  day on the deployed worker), so ask once when the tab opens or the
  candidate set changes, not on every keystroke. Up to 16 candidates; IDs 1
  to 64 bytes and not `none`; labels up to 200 bytes.

In Rust, a host or client in the workspace calls `coder::first::request`,
`triage_of`, and `feedback` for the turn judgment, and `candidates_of`,
`rank_request`, and `ranking` for suggestions, through any `jev::Client` the
decision profile resolves.
