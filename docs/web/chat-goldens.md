# Web chat goldens

What we expect the openagents.com chat to do when people ask what they
actually ask, written down as goldens and run as evals through the same
endpoints the live site's chat uses.

- The set: [`bench/web-chat/goldens-v1.json`](../../bench/web-chat/goldens-v1.json)
- The grading: [`crates/coder/src/chat_goldens.rs`](../../crates/coder/src/chat_goldens.rs)
- The runner: [`crates/coder/src/bin/chat-goldens.rs`](../../crates/coder/src/bin/chat-goldens.rs)
- The short command: [`scripts/chat-goldens.sh`](../../scripts/chat-goldens.sh)

The router's own labeled set (`crates/coder/fixtures/chat-router/routes-v5.json`,
run by `crates/coder/tests/router_eval.rs`) measures routing over
first messages on the phone's context. These goldens are different: they
are the product's expectations on the website, end to end, graded on what
the person reads and how fast.

## How a web chat message is answered

1. The browser posts the message to `/chat` (a new chat) or `/chat/{id}`
   with the visitor cookie and the form's ticket
   (`crates/openagents-web/src/pages/chat.rs`).
2. The website sends it to the chat worker through the relay, as a
   NIP-CJ job signed with the visitor's key, asking for the first response
   and the chat router (`"opener": true`, `"router": "chat-router-v2"`,
   `context.surface: "web"`; `openagents_chat::basic_coder::payload`). It is
   the same worker and the same payload the apps use; only the surface
   differs.
3. The worker (`crates/coder/src/bin/coder-worker.rs`) asks Jev, TypeSafe's
   System One model, one question set beside the model call. Code turns the
   readings into a tier (`coder::router::decide`, held to
   `coder::router::policy::for_web` on the website):
   - **canned**: a prepared answer from the bank
     (`crates/coder/answers/chat-answers-v1.toml`), or a product note
     (`knowledge/openagents/*.md`) whose reviewed answer Jev judges to fully
     answer the message. Shown whole, in about a second.
   - **grounded**: the model, answering from the product notes it was
     given. Seconds.
   - **model** / **opener**: the model alone, optionally after a line.
   - **refuse**: a bank refusal (a pasted key, a harmful request).
4. The website stores the reply and shows it. The reply sits between two
   hidden markers, `data-oa-reply` (with `data-oa-tier`, `data-oa-route`,
   and `data-oa-answer` once answered) and `data-oa-reply-end`, so an eval
   reads exactly what the person sees.

On the website, a few answers have a `.website` variant
(`meta.who.website`, `meta.capabilities.website`, `meta.tools.website`,
`meta.limits_chat.website`, `meta.coder.website`, `meta.github.website`):
the apps' wording sends Coder to "a computer you connect", which the
website can't do; on the website Coder is the terminal agent from
openagents.com/download. An account question on the website reads the
product notes (sign-in, Settings, the Claude key, Coder's sign-in) instead
of being told "the app does that".

## The goldens

Each golden is one expectation: several phrasings of one question
(including casual and misspelled ones), the routes and prepared answers or
product notes that are right, the tier, the speed (`instant` or `model`),
the facts the reply must contain (each a list of alternatives), and words
it must never contain. Every reply is also checked against the set's
forbidden list (wrong claims such as "we don't train on", "there's no
account", a `.dmg`, Coder on "a computer you've connected") and for machine
talk (`oa-copy`).

A golden may also name `grounded` sources (today `rate_card`, the public
rate card). Then every number and URL in its reply must come from those
sources (`inference::grounded::untraced`, #11114). A retyped or made-up
price fails the `grounded` check, and that failure counts as a wrong
answer. The offline `check` runs the same check on each listed answer's
own text, and it rejects a source name it doesn't know.

| Flow | What people ask |
| --- | --- |
| starters | the four questions under the chat box, exactly as the chips send them (What is OpenAgents?, What models do you use?, How do I connect my codebase?, What are plugins?), plus two casual phrasings each (#11095) |
| about | who we are, what OpenAgents is, what we can do, the models, Jev, open source |
| github | connecting a GitHub repository, projects |
| account | signing in, whether an account is needed |
| coder | what Coder is, installing it, `coder login`, `/sync` and replying from the website |
| environments | what an environment is, running Claude Code on a repository, availability |
| claude_key | adding your own Claude key; a key pasted into the chat |
| pricing | cost, the paid plan, message limits |
| chats | finding, archiving, renaming, and deleting chats |
| privacy | training, who sees messages, storage |
| plugins | which plugins to try (the built-in plugins' cards) |
| limits | files, running code, browsing |
| smalltalk | greetings and thanks |
| general | general questions, and code work the website can't do |
| followup | a question after another in the same chat |
| interactive | answers the model writes with components, and a follow-up that edits them (#11113) |
| project | a chat in a project with a connected GitHub repository: "Summarize this repo." and similar are answered from the repository, never with the install text. These goldens carry the repository as the website reads it (`repository`), so only `router` mode asks them (`http` and `local` skip them: a visitor's chat has no project); `note` checks the model is told to answer from it |

### Components (#11113)

A golden may also say what the reply's components must draw, in `ui`:
`required` (a reply in prose alone fails `ui`), the catalog `components`
that must appear, the `links` a button or link must point to (site paths
or URLs), and the `commands` a code block or command must let the reader
copy. Two checks come of it:

- `ui_valid`: every ```` ```openui-lang ```` block in the reply parses with
  nothing fixed or dropped. A block that needed fixes is a wrong reply.
- `ui`: what `ui` asks for is drawn. A right reply in prose where
  components were expected is right but slow, like a model reply where a
  prepared answer was expected.

The checks read the reply as written: `check` reads each accepted answer's
own text (its `ui` included), `router` reads a prepared answer's text, and
`local`/`http` fetch `/chat/{id}/messages/{n}/original` after the page
draws the reply. A grounded reply in `router` mode has no written text, so
`ui` is skipped there. The how-tos whose accepted answers all carry
components (`starters.codebase`, `github.connect_repo`,
`account.sign_in`, `account.connect_computer`, `coder.install`,
`coder.login`, `coder.sync`, `followup.repo_after_hello`) ask for the
components every one of those answers shares.

Budgets: an instant answer shows its first words within 3 s and is whole
within 3.5 s on the page; a model answer within 10 s and 60 s. The router
mode's budget for Jev alone is 2 s.

## The launch bar (#11106)

Before a chat deploy, run

```sh
scripts/chat-goldens.sh local
```

It exits 0 only when the run meets the set's `gate`
(`bench/web-chat/goldens-v1.json`):

- **At least 90 % of the cases are right.** A case is right when it passes,
  or when the person read a right reply that wasn't instant: every failed
  check is a time, or the model wrote the reply (from the product notes or
  not) instead of a prepared answer shown whole, and the reply has every
  required fact, no forbidden word, and no machine talk. The report lists
  these under **Right but slow**, apart from the **Failures**.
- **No wrong case in a critical flow:** the four questions under the chat
  box (`starters`), what models we use (`about.model`), the how-tos
  (`coder.install`, `coder.login`, `coder.sync`,
  `account.connect_computer`, `github.connect_repo`), pricing and plans
  (`pricing`), and privacy and training (`privacy`).

A run with `--flow`, `--golden`, or `--first` exits 1 when any case
fails. The router mode can't read a written reply, so there a case the
model would answer counts as wrong; gate on `local`.

When the Vercel AI Gateway can't pay (every call answers 402),
`CHAT_GOLDENS_NO_GATEWAY=1 scripts/chat-goldens.sh local` keeps it out
where it can: Jev through OpenRouter and TypeSafe, the notes' embeddings
and the chat model through OpenRouter (a turn OpenRouter hasn't started
in 4 s still falls back to the gateway, and fails).

## Running them

```sh
scripts/chat-goldens.sh check                          # offline; also cargo test -p coder --test chat_goldens
scripts/chat-goldens.sh router                         # Jev and the router here
scripts/chat-goldens.sh local                          # this checkout's worker and site, end to end
scripts/chat-goldens.sh http --base http://127.0.0.1:4300
```

Every mode takes `--flow ID`, `--golden ID` (each repeatable), and
`--first N`, and writes `report.json` and `report.md` to `--out` (default
`target/chat-goldens/<mode>-<unix seconds>/`): the pass rate, each flow's
numbers, first-answer percentiles, and each failure with its route, tier,
answer, the failed checks, and the reply.

- **check** needs nothing: every prepared answer and product note a golden
  accepts exists, shows on the website, and says what the golden requires.
  A golden whose accepted answers can't all be right fails here before
  anything is sent. It runs in CI as `crates/coder/tests/chat_goldens.rs`.
- **router** asks Jev (`TYPESAFE_API_KEY`) and decides the tier as the
  worker does for a website turn, and reads the product notes when an
  embeddings key is set (the script uses the AI Gateway key). It reports
  routes, tiers, answers, the prepared text, and why
  (route and answer probabilities, the needs-specifics reading, and the
  notes' relevance); it doesn't write model replies.
- **http** sends each phrasing through a running site's chat endpoints as a
  visitor's browser does: a fresh visitor from `GET /`, `POST /chat` (later
  turns `POST /chat/{id}`), then `GET /chat/{id}/transcript` until the reply
  is whole, timing the first words and the whole reply. Each chat is
  deleted afterwards (`--keep` keeps them). A local site
  (`openagents-web`) answers through the production chat worker unless
  `OPENAGENTS_WEB_CHAT_WORKER` (a worker's public key in hex) and,
  optionally, `OPENAGENTS_WEB_CHAT_RELAY` point it at another.
- **local** builds this checkout's `coder-worker` and `openagents-web`,
  starts the worker on a fresh key on relay.openagents.com with the shipped
  configuration, starts the site on `127.0.0.1:4399` pointed at it, runs
  **http**, and stops both: the answers in this checkout, end to end,
  before they ship. It needs `TYPESAFE_API_KEY`, `AI_GATEWAY_API_KEY`, and
  `OPENROUTER_API_KEY` (read from `~/work/.secrets` when unset).

Run against a local server. The production site
(`--base https://openagents.com`) writes each chat to the production store
before deleting it; run it there only as a single, deliberate smoke
(`--golden ID --first 1`), never in a loop. A production run grades route,
tier, and answer only once the site serves the reply markers.

## Changing the goldens

- A golden is an expectation of the product, not a description of today's
  replies. When a golden is right and the reply is wrong, fix the product
  knowledge (the bank or a note) or the routing, not the golden.
- Don't tune a bank entry's `when` or `examples`, or a note's
  `applies_when`, against the goldens' phrasings one by one.
- Add a golden when people start doing something new with the product;
  give it the casual phrasings people really type.
- `check` must pass: every accepted answer's text has to meet the golden.

## Results, 2026-10-09

Jev's readings vary from run to run, so numbers move by a few cases.

### The launch bar (#11106), 113 cases

| Run | Pass | Right (bar) | Critical wrong | Instant first words p50 / p90 |
| --- | --- | --- | --- | --- |
| `router`, `main` at `5af7b4257b` | 86 of 113 | - | - | Jev alone 0.5 / 1.2 s |
| `local`, `main` at `5af7b4257b` with only the Jev door fix, gateway kept out | 87 of 113 | 88 % | 5 | 1.8 / 3.3 s |
| `local`, after #11106's first fixes, gateway kept out | 91–92 of 113 | 97–98 % | 0–2 | 1.9 / 3.3–4.1 s |
| `local`, final (`main` after #11106), gateway kept out | 90 of 113 | 99 %, met | 0 | 2.0 / 3.6 s |
| `local`, final, again | 96 of 113 | 99 %, met | 0 | 1.9 / 3.3 s |

The first `local` run with the shipped configuration failed half its cases
with "We couldn't answer this time": the Vercel AI Gateway answers every
call with 402 (no credit), and Space Bunny Alpha, the worker's OpenRouter
primary, is gone from OpenRouter (404). The runs above keep the gateway out
(`CHAT_GOLDENS_NO_GATEWAY=1`), so the chat model is Gemini 3.8 Flash on
OpenRouter.

What #11106 fixed:

- The `product.kb` route says it covers projects, environments, the Claude
  key in Settings, and getting, installing, and signing in to Coder;
  `general` leaves a feature named by an everyday word ("what is a
  project") to it, `work.dispatch` leaves getting Coder to it, `codebase.kb`
  leaves "where is your source code" to `meta`, and `clarify` leaves a short
  question about price to `meta`. The route question set is regenerated and
  the calibration refit by the published eval (held out: route accuracy
  90.2 %, canned precision 98.6 %, dispatch precision 97.7 %, as before).
- `openagents.chat-privacy` v11 says it covers training and opting out, so
  "can I opt out of training" finds it instead of "we have no documented
  answer".
- On the website, Jev's needs-specifics reading no longer keeps a product
  note it judged to fully answer the message from showing whole: the
  website knows nothing of the visitor's computer or repositories that a
  model writing from the same note could add (`router::grounded`).
- Jev's OpenRouter door asked for `typesafe/jev-latest`, which OpenRouter
  refuses with a 400 that never fails over: with the gateway down, every
  routed turn went unrouted. `jev-latest` now asks OpenRouter for
  `typesafe/jev-1.13`.
- Goldens: the page draws code without backticks and may show a link
  without `https://`, so the text checks ignore both; "connect my repo" and
  "run Claude Code on my repo" accept the connect-your-codebase answer added
  in #11095, and "connect your GitHub" or "sign in with GitHub" for
  "connect GitHub". "can you work on my github repos?" moved from the
  connect-a-repository how-to to its own `limits.repos` golden: it asks
  whether we can do the work, and a right reply names Coder and how to get
  it or add the repository.

Still failing in the final runs, and why that's acceptable for launch:

- `coder.what#2` "how does coder work" (second run): the model, writing
  from the notes, said "Remote dispatch", which the machine-talk check
  flags. Not critical; the note it read says "dispatch" for the phone.
- `followup.repo_after_hello#1` (first run): the site answered 409 "still
  answering your previous message" to the second turn: the runner's wait
  for the first reply, not the reply.
- Earlier runs also missed `environments.when#2` (the notes lookup ran past
  its 2 s budget on OpenRouter and the model said "no release date") and
  lost one or two turns to the gateway fallback's 402.

Sixteen to twenty-two more were right but not instant: mostly a product
note the model wrote from (3 to 4 s on OpenRouter) because Jev's
whole-answer reading kept it from standing alone (projects, sign-in,
environments, "connect my repo", "can you read my repo from here?"), a
note shown whole that took more than 3 s through OpenRouter, or a right
model reply on another route ("what is the code at openagents.com/device",
"can you open this link").

### Earlier, 97 cases

| Run | Pass | Instant first words p50 / p90 |
| --- | --- | --- |
| `http`, local site → the production chat worker (release `bbed5d89af`, 2026-10-03) | 25 of 97 (26 %) | 2.3 s / 18.2 s |
| `local`, this checkout's worker and site | 69 of 97 (71 %) | 1.8 s / 7.0 s |
| `http`, local site → the production chat worker after deploying `42fe20c01b` | 67 of 97 (69 %) | 1.5 s / 8.2 s |
| `router`, this checkout | 67–68 of 97 | Jev alone 0.3–1.1 s |

Until `42fe20c01b` was deployed on 2026-10-09, the production chat worker
ran a release from 2026-10-03 (Coder "dispatched" to "a computer you've
connected", a Mac `.dmg`, and none of the notes on projects, sign-in, the
Claude key, environments, Coder's sign-in and sync, or deleting chats).
