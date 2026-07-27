# OpenAgents Episode 263 - Omega Agent

Status: harness draft as Sarah / pending owner review. Not scheduled for
recording. The Omega Agent product contract admits the identity and admits no
public claim, so recording and publication need the owner's authorization
separately from this draft.
Episode number: 263, assigned by owner direction on 2026-07-27. The Full Auto
reliability draft moved off this slot to
[`26X-fullauto.md`](26X-fullauto.md). The Forking Zed tour
([`26X-forkingzed.md`](26X-forkingzed.md)) stays unscheduled.
Speaker: Sarah.
Follows Episode 262 (Project Omega).
Delivery: calm, certain, and direct. Urgency without theatrics.
Audience: developers and operators first. The demo beats stay readable for
nontechnical viewers.
Supersedes: the 2026-07-25 harness draft in this file, which was written before
the slim agent, executor-explicit delegation, the Exo lane, and zero base
landed in the fork. Revised the same day for the owner direction that sets the
six-tool surface and the `plugin` tool.
Product contract:
[`Omega Agent product contract`](../../specs/omega/omega-agent.product-spec.md)
at `spec_revision: 3`.
Product direction:
[`Omega Agent analysis`](../fable/2026-07-25-omega-agent-analysis.md),
[`Omega Agent roadmap`](../omega/2026-07-25-omega-agent-roadmap.md),
[`Omega Agent shape record`](../omega/2026-07-25-omega-agent-shape-record.md),
[`Omega zero-base mode`](../omega/2026-07-26-omega-zero-base-mode.md),
[`Plugin tool specification`](../omega-agent/2026-07-27-plugin-tool-spec.md),
[`Episode 262 - Project Omega`](262.md).
Production conventions:
[`Sarah video screenshare`](SARAH_VIDEO_SCREENSHARE.md),
[`Episode 262 production requests`](262-production-requests.md),
Episode 262 lessons in
[`Segmind talking-avatar pipeline`](../sarah/2026-07-22-segmind-talking-avatar-pipeline.md).

Episode 262 named the product and made one promise: Omega is the last IDE. This
episode makes the second promise, and it is a larger one. It says Omega Agent
is the last agent you talk to.

The claim is not that our agent is better than every other agent. The claim is
that it is the last one you have to choose, because it routes to the others,
because it improves from the record of what it routed, and because anyone can
extend it — and earn a share of the revenue their extension produces. The
routing runs in the fork today, with the person naming the executor. The
learning, the sixth tool, and the market are admitted direction and are not
running. This script keeps those states apart in every line.

## Technical truth notes

**Running in current fork builds** (unsigned development builds and local
release candidates; `v0.2.0-rc19` is the latest tag at drafting):

- Zero base: a launch mode that shows one agent thread and hides the editor
  around it. The same binary is still the full editor when the flag is absent.
- The basic agent as the default profile. Its model-visible surface today is
  five tools: `read`, `write`, `edit`, `bash`, and `delegate`
  (OMEGA-DELTA-0133). The admitted product surface is six; see the plugin
  note below. Context-server tools are off, so an installation cannot
  make a sixth tool appear. The measured slim prompt is a separate template
  with a byte ceiling (OMEGA-DELTA-0135).
- `delegate` is executor-explicit (OMEGA-DELTA-0137). Admitted spellings are
  `native`, an installed ACP agent id, `exo`, and `engine:<lane>`. `auto` is
  not admitted. An unavailable name returns `no_executor` and never quietly
  runs something else.
- The typed disclosure chain. A delegated result carries Omega Agent, then the
  executor, then the hosted runtime and model behind it, so a vendor-backed
  answer cannot be attributed only to the harness that relayed it.
- The executor disclosure line and its pin control on every thread
  (OMEGA-DELTA-0021, OMEGA-DELTA-0046).
- Exo (`exoharness/exo`) as an external ACP delegate target, opt-in for each
  launch with `--enable-exo` (OMEGA-DELTA-0144), with an inspector showing
  identity, runtime, capabilities, and an authority receipt.
- The work-loss law. The agent preserves the person's work, uses its own
  snapshots for undo, and never treats Git checkout, restore, or stash as an
  undo mechanism. Destructive Git commands inspect the dirty tree before a
  process exists, and the decision is retained as a versioned receipt
  (OMEGA-DELTA-0134).
- Tool permissions default to allow, with trust prompts removed
  (OMEGA-DELTA-0001, OMEGA-DELTA-0002). Confirmation remains for irreversible
  data loss.
- The supervised `omega-effectd` engine owning Full Auto runs and receipts, and
  one real receipted cloud Agent Computer turn.
- The default model is the `google/gemini-3.6-flash` direct provider.

**Implemented but not proven.** The slim-agent proof protocol
(OMEGA-DELTA-0138) requires an out-of-box journey, a harness journey, and a
basic-versus-wide comparison. The installed-candidate journey and the live
comparison are pending. Until they pass, no reliability claim may be spoken.

**Admitted, spec'd, not running.** The sixth tool, `plugin`: deterministic
functionality — typed, versioned, content-addressed, sandboxed, with a
replayable receipt per run. ProductSpec revision 3 admits it and the
[plugin tool specification](../omega-agent/2026-07-27-plugin-tool-spec.md)
owns it. Its registry and its paid market with revenue sharing are phased
behind separate owner admissions. Sarah speaks of `plugin` and the market in
the future tense only.

**Accepted direction, not running.** The rename of the inherited native tile
(the reachable label is still `Zed Agent`; issue omega#75 owns it), the router
that chooses the executor for you, the learning loop over routed receipts, the
chat-first welcome surface, and Khala routing in either direction. The Khala
endpoint and the public tokens-served counter are live services today.

**Owner-reserved and unanswered.** Whether the first-party agent signs with its
own Nostr identity. The shape record leaves this to the owner. Nothing in this
script depends on the answer, and no line may imply one.

## Production direction

Use these labels:

- **OMEGA FORK BUILD - UNSIGNED DEVELOPMENT BUILD** for any live capture of the
  current fork application.
- **INHERITED AGENT TILE - CURRENT** for the native tile wherever its label is
  visible. Do not brand that tile as Omega Agent. The rename has not landed.
- **ACCEPTED DIRECTION - NOT RUNNING** for the automatic router, the learning
  loop, the welcome mock, the `plugin` tool, the plugin market, and any Khala
  routing diagram.
- **ARCHIVE - 2024** for any clip or still from the 2024 plugin and Agent
  Store arc (Episodes 048-098). Date-stamp it on screen so history is never
  mistaken for the current product.
- **RECEIPT - CURRENT** for real Full Auto, Agent Computer, or Git-guard
  receipt captures. Redact objectives, paths, and account identity.
- **LIVE SERVICE - CURRENT** for the public Khala counter or stats page.

Keep the spoken lines short and hold screens under them. Do not lengthen
Sarah's lines to cover a screenshare. Mask local usernames and paths. Do not
show credentials or private project names. SIGTERM quit, never Cmd+Q.

The Exo beat is the strongest live material in this episode and the easiest to
overclaim. Show the inspector. Show the disclosure chain. Do not describe Exo
as ours, and do not imply it is on by default. It is opt-in for each launch.

### Assembly map

| Beat | Story | Primary visual |
| --- | --- | --- |
| 1 | The last IDE, and the next promise | Episode 262 close |
| 2 | One thread, nothing else | Zero base launch, live typing |
| 3 | Six tools | The tool list, a real edit, a passing test |
| 4 | Delegate: hands | `delegate` with a named executor |
| 5 | An agent that is not ours | Exo lane, inspector, disclosure chain |
| 6 | Plugin: facts, and the market | 2024 archive clips, then the plugin spec card |
| 7 | What it will not do to your work | Git guard prompt and its receipt |
| 8 | Why it can improve | Receipt capture, then the router diagram |
| 9 | The collective it belongs to | Khala stats counter, live |
| 10 | Close | Return to Sarah |

---

[OPEN ON SARAH.]

**Sarah:** Last time I told you why we forked Zed, and I gave the product its
name. Omega. The last IDE.

Today I want to tell you who answers when you open it, and make you a second
promise.

Omega Agent is meant to be the last agent you talk to.

**Sarah:** I want to be careful about what that means, because it is easy to
say and easy to fake.

I am not telling you our agent is smarter than every other agent. I am telling
you it is the last one you should have to choose.

It routes to the others. Agents, harnesses, your own subscriptions, our engine,
our cloud. And it keeps a record of every route, so over time it can learn
which hand is right for which work.

The routing runs today, and you name the executor. The learning does not run
yet. I will show you exactly where that line is.

[SCREEN: launch a fork build in zero base. One agent thread. Cursor in the
input. Type a short real prompt and let it run.
Label: `OMEGA FORK BUILD - UNSIGNED DEVELOPMENT BUILD`.]

**Sarah:** This is Omega with the editor taken away. One thread. A place to
type.

The editor is still in there. This is the same program. We hid it so you can
see what is actually doing the work.

**Sarah:** Here is the boundary before the detail.

Omega Agent has six tools. Read. Write. Edit. Bash. Delegate. Plugin.

That is the whole surface. Not a menu of forty. Six, and you can hold all six
in your head while you watch it work.

Five of them run today. The sixth is admitted and being built, and I will
label it honestly when we get there.

[SCREEN: the tool list, then a real edit to a real file and a passing
verification command.
Label: `OMEGA FORK BUILD - UNSIGNED DEVELOPMENT BUILD`.]

**Sarah:** The first four are the hands. They read, write, and edit your
files, and they run your commands, here, on your machine.

The last two are the product. Delegate is for judgment. Plugin is for facts.

Delegate first.

[SCREEN: a `delegate` call with a named executor, and the result carrying its
disclosure.]

**Sarah:** Delegate hands the work to somebody else.

You can name the loop inside the editor. You can name a harness you installed.
You can name one of our engine lanes.

**Sarah:** There is no automatic option, and that is deliberate.

Today, if you do not name an executor, nothing is chosen for you. If you name
one that is not there, it tells you and stops. It never quietly runs a
different agent and lets you believe it ran the one you asked for.

A router that guesses without telling you is the thing we are replacing. We are
not going to ship one first and add the honesty later.

[SCREEN: the Exo lane. The executor disclosure line, then the inspector:
identity, runtime, capabilities, authority receipt.
Label: `OMEGA FORK BUILD - UNSIGNED DEVELOPMENT BUILD`.]

**Sarah:** This one is not ours.

Exo is an outside harness. We attached it this week over the Agent Client
Protocol. It runs its own agent, in its own sandbox, with its own tools.

We did not copy its credentials. We did not make a second home for it on your
machine. It stays off unless you turn it on for that launch.

**Sarah:** Now look at the line that comes back.

Omega Agent. Then Exo. Then the runtime and the model behind Exo.

That last part matters. When an agent is a wrapper over somebody's hosted
model, the answer is not really from the wrapper. You should be able to see all
the way down, and here you can.

[SCREEN: archive clips: the 2024 WASM plugin registry, the Agent Store launch,
the payout screens. Date-stamp each. Label: `ARCHIVE - 2024`.]

**Sarah:** Now the sixth tool. Plugin.

Delegate hands work to another agent, and an agent's answer is judgment.
Plugin calls a function, and a function's answer is a fact. Same input, same
version, same result, every time — with a receipt you can replay.

A plugin is a small, typed, sandboxed piece of deterministic functionality.
It declares what it takes in, what it returns, and what it is allowed to
touch. It cannot call a model. It cannot reach anything it did not declare.
And every run ends in a receipt that binds exact code to exact input to exact
output, so a stranger can check the work without trusting anyone's prose.

**Sarah:** We have built this before.

In 2024 we ran plugins in WebAssembly, put them in a registry, and launched
the Agent Store — a marketplace with revenue sharing paid in Bitcoin, metered
on usage, down to the minute. Community members built plugins, and we paid
them. Then the architecture underneath it was reset, more than once, and the
market went away. The idea was never wrong. The foundation was.

Omega is the foundation that can hold it. The plugin tool is that market
coming back, in phases, with receipts this time.

[SCREEN: the plugin specification card: the six tools, the receipt fields,
the three phases. Label: `ACCEPTED DIRECTION - NOT RUNNING`.]

**Sarah:** And here is why it matters beyond this app.

Omega Agent is one face of a super agent. Khala — our collective
intelligence — is, in our founder's words, one collective mind built up of
little programs that compose into a response. A plugin you publish does not
just extend the agent on your machine. It contributes a capability to that
collective. And when your capability is used, you earn a share of the
revenue it produces.

We have said from episode one: pay the people. Anyone who contributes
something valuable to an AI workflow should be paid for it, proportionally,
automatically. The plugin is the unit that makes that structural: small
enough for one person to build, typed enough to verify, and metered enough
to pay.

That market is not live today. It returns in phases, each one gated, and
when the payments turn on you will see the receipts, not a press release.

[SCREEN: a destructive Git command intercepted with the dirty-file prompt, then
its retained receipt. Use a scratch repository.
Label: `RECEIPT - CURRENT`, redacted.]

**Sarah:** One more thing this agent will not do.

It will not use Git checkout, restore, or stash to undo your work. Those
commands do not undo. They discard.

Before a command like that can run, Omega checks whether the tree is dirty and
names the files that are about to go. The decision is kept as a receipt, so you
can check it later, after the terminal output is gone.

We learned this the hard way, on our own repository, twice in one afternoon.

**Sarah:** Which brings me to why I think this agent can improve instead of
age.

Every routed turn ends in a record. Which executor ran. What it changed. What
proved it.

When the router does start choosing for you, it will choose from that record,
and the policy will never promote itself.

[SCREEN: the routing diagram.
Label: `ACCEPTED DIRECTION - NOT RUNNING`.]

**Sarah:** That is direction. It is not running, and I am not going to show you
a picture of it and let it sound like a feature.

[SCREEN: the public Khala tokens-served counter moving.
Label: `LIVE SERVICE - CURRENT`.]

**Sarah:** Last thing. Omega Agent is the face on your machine. Behind it is
Khala, our collective intelligence. One endpoint that behaves like a single
model with an agent network underneath. It is live today and serving.

Omega does not absorb it. They are two orchestrators, and they meet at a
protocol.

[RETURN TO SARAH.]

**Sarah:** So that is the promise, and the honest state of it.

One agent. Six tools. Every hand it uses, named out loud. Nothing chosen for
you that you were not told about. And when you make it better, you get paid.

Omega is the last IDE. Omega Agent is meant to be the last agent you talk to.

It is not finished. It runs, and you can check it.

---

## Truth ledger

Each spoken claim, and what must be true before it may be recorded. Update the
state column as capability lands. If a line is not green, cut the line. Do not
change the wording of a planned capability until it sounds shipped.

| Spoken claim | Required before it may be spoken | State at 2026-07-27 |
| --- | --- | --- |
| "Omega with the editor taken away" | zero base launches to one agent thread on the candidate being captured | runs in fork builds |
| "the same program" | the same build is a full editor without the flag | runs; zero base removes nothing |
| "Six tools. Read. Write. Edit. Bash. Delegate. Plugin." | the six-tool surface is admitted in ProductSpec revision 3, and the spoken line separates the five that run from the one being built | admitted; five run (OMEGA-DELTA-0133), `plugin` is direction |
| "Five of them run today. The sixth is admitted and being built." | the fork's basic profile carries the five, and the plugin packets are open but unlanded | true at drafting; re-verify on recording day, and flip the line if `plugin` has landed |
| "There is no automatic option" | `auto` stays unadmitted in `delegate` | runs (OMEGA-DELTA-0137) |
| "it tells you and stops" | an unavailable executor returns `no_executor` on the captured build | runs |
| "We attached it this week" | the Exo lane completes a live delegated turn on the captured build | runs; the capture must be a live turn, not a fixture |
| "We did not copy its credentials" | Exo resolves from its own checkout and state root with no synthetic settings | runs (OMEGA-DELTA-0137) |
| "off unless you turn it on for that launch" | Exo is absent from the selector without `--enable-exo` | runs (OMEGA-DELTA-0144) |
| "you can see all the way down" | the delegated result carries the Omega Agent, executor, and runtime chain | runs |
| "names the files that are about to go" | the Git guard prompts with the affected file names on a dirty scope | runs (OMEGA-DELTA-0134) |
| "kept as a receipt" | the versioned guard metadata stays on the tool call | runs |
| "twice in one afternoon" | the incident is documented | documented in the openagents oopsiewoopsies audit, 2026-07-27 |
| "Same input, same version, same result ... with a receipt you can replay" | describes the admitted plugin contract. Present tense about the design is acceptable only over the spec card with its label; a live demo needs the landed tool | direction; spec'd in the plugin tool specification |
| "In 2024 we ran plugins ... revenue sharing paid in Bitcoin ... down to the minute" | the archive supports it: Episodes 048-098, the Agent Store launch, and the per-minute payout episode | documented history; show only date-stamped archive material |
| "Community members built plugins, and we paid them" | community plugin submissions and payouts are in the archive (Episodes 066, 088, 093) | documented history |
| "you earn a share of the revenue it produces" | spoken in the future tense over the phase card. No payment, registry, or payout surface exists for plugins today | direction; phase three is owner-reserved |
| "pay the people" from "episode one" | Episode 1 and Episode 223 record the commitment | documented history |
| "one collective mind built up of little programs that compose into a response" | quoted from Episode 242's Khala launch, attributed to the founder | documented; keep the attribution |
| "it will choose from that record" | nothing. The line is in the future tense and must stay there. | accepted direction |
| "Khala ... is live today and serving" | the public counter and the endpoint are live at recording | live service |
| any use of the product name on the native tile | the rename lands (omega#75) | **not landed.** Do not show that tile branded as Omega Agent. |

## Recording gates

1. The truth ledger is green for every line that stays in the script. The
   script makes no small-surface reliability claim; if one is added back, it
   waits on the basic-versus-wide comparison that OMEGA-DELTA-0138 requires,
   which is still pending.
2. Every product capture is a live build. It is not a still and not a second
   performance of an earlier run. The Exo turn in beat 5 is a real turn.
3. The build shown is labeled honestly. It is unsigned or a development
   candidate, and the label says so on camera.
4. No secret, token, private path, provider account identifier, or
   customer-private material appears in any frame. Check the Finder leak and
   the window-size traps in the Episode 262 lessons.
5. The Git-guard beat uses a scratch repository. Do not demonstrate a
   destructive command against real work, even with the guard in front of it.
6. Every 2024 archive clip in the plugin beat carries the `ARCHIVE - 2024`
   label and an on-screen date. No archive frame may read as the current
   product, and no revenue-share sentence may be spoken over current-product
   footage.
7. Nothing implies the Electron application is retired. It stays the rollback
   source.
8. The Sarah master follows the Episode RC assembly path. Spoken words go to
   `~/Desktop/Sarah/263/263transcript.md`.
9. This file is updated to the final spoken text when the script locks.

## Open production questions

1. Does the Exo beat name Exo out loud, or describe it as "an outside harness"?
   To name it is more honest and more interesting. It also gives attention to a
   project that is not ours, in an episode about our product. The
   recommendation is to name it.
2. Beat 6 carries the strongest trust material and the least attractive
   picture. Does it stay in a short episode, or move to its own segment?
3. Is the routing diagram in beat 7 worth a picture at all, when its label says
   it does not run? A shorter, more honest episode can speak the sentence and
   hold on Sarah.
4. The founder is on leave. Does this episode say so, as the Full Auto draft
   does, or stay on the product only?
5. The plugin beat is the longest direction segment in the script. If the
   episode runs long, the cut that preserves the argument is: keep the
   fact-versus-judgment definition and the pay-the-people close, drop the
   2024 archive montage to a single still.
6. Does the revenue-share promise deserve its own future episode at
   phase three, when the first plugin payout receipt exists? The
   recommendation is yes, and this episode plants the promise it will pay
   off.

## Drafting record

- Harness draft written as Sarah under the Acting-as-Sarah runbook, at owner
  instruction, 2026-07-27. Disposition: draft, pending owner review. Not
  scheduled for recording. This revision replaces the 2026-07-25 draft in this
  file, which described the product before the slim agent, executor-explicit
  delegation, the Exo lane, and zero base existed.
- Sources read in full for this revision: `docs/transcripts/README.md` (the
  catalog at this revision), Episodes 258, 259, 260, 261, and 262, the Full
  Auto draft and the Forking Zed draft,
  `docs/sarah/ACTING_AS_SARAH_RUNBOOK.md`,
  `docs/fable/2026-07-25-omega-agent-analysis.md`,
  `docs/omega/2026-07-25-omega-agent-shape-record.md`,
  `docs/omega/2026-07-26-omega-zero-base-mode.md`,
  `specs/omega/omega-agent.product-spec.md` at `spec_revision: 3`,
  `docs/omega/ROADMAP.md`, and the `OMEGA_DELTAS.md` entries for
  OMEGA-DELTA-0021, 0046, and 0133 through 0144 in the `OpenAgentsInc/omega`
  checkout at `265a43878e`. For the six-tool revision: the plugin-economy
  and marketplace arc in the transcript archive (Episodes 048-098 sampled,
  062, 066, 070, 088, 092, 093, and 098 directly), Episode 223 Pay the
  People, the Khala arc (Episodes 241-245, 242 and 244 directly), the Pi
  teardown `docs/teardowns/2026-07-21-pi-agent-teardown.md`, the DSE history
  audit `docs/dspy/2026-07-20-dspy-in-effect-git-history-audit.md`,
  everything in `docs/omega-agent/`, and the plugin tool specification
  written alongside this revision.
- Memory review: Sarah's live owner-scoped runtime memory was not queried from
  this harness. That limit is recorded here per section 3 of the runbook, and
  this draft does not claim a memory review. The harness-local project memory
  was inspected. The applicable entries concern channel and authority policy,
  and they agree with current repository sources. No memory is used as evidence
  for a product-state claim.
- Product-state discipline: running, implemented-but-unproven,
  admitted-direction, archive-history, live-service, and owner-reserved
  claims stay separate in the truth notes, are bound to the production
  labels, and are examined again line by line in the truth ledger. The one
  line that names a reliability benefit is marked unproven, because the
  comparison that would support it has not run. Every plugin-market and
  revenue-share sentence is future tense or date-stamped 2024 archive, and
  the phase admissions that would make them present tense are recorded as
  owner-reserved.
- Authority: the Omega Agent product contract admits the identity and the basic
  agent. It admits no rename, no router code, and no public claim. Recording
  and publication therefore need the owner's separate authorization. The Nostr
  signing question stays owner-reserved and unanswered, and no line in this
  script depends on the answer.
