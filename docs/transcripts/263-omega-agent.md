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
landed in the fork.
Product contract:
[`Omega Agent product contract`](../../specs/omega/omega-agent.product-spec.md)
at `spec_revision: 2`.
Product direction:
[`Omega Agent analysis`](../fable/2026-07-25-omega-agent-analysis.md),
[`Omega Agent roadmap`](../omega/2026-07-25-omega-agent-roadmap.md),
[`Omega Agent shape record`](../omega/2026-07-25-omega-agent-shape-record.md),
[`Omega zero-base mode`](../omega/2026-07-26-omega-zero-base-mode.md),
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
that it is the last one you have to choose, because it routes to the others and
because it improves from the record of what it routed. The first half runs in
the fork today, with the person naming the executor. The second half is
accepted direction and is not running. This script keeps those two apart in
every line.

## Technical truth notes

**Running in current fork builds** (unsigned development builds and local
release candidates; `v0.2.0-rc19` is the latest tag at drafting):

- Zero base: a launch mode that shows one agent thread and hides the editor
  around it. The same binary is still the full editor when the flag is absent.
- The basic agent as the default profile. Its whole model-visible surface is
  five tools: `read`, `write`, `edit`, `bash`, and `delegate`
  (OMEGA-DELTA-0133). Context-server tools are off, so an installation cannot
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
  loop, the welcome mock, and any Khala routing diagram.
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
| 3 | Five tools | The tool list, a real edit, a passing test |
| 4 | The fifth tool is the product | `delegate` with a named executor |
| 5 | An agent that is not ours | Exo lane, inspector, disclosure chain |
| 6 | What it will not do to your work | Git guard prompt and its receipt |
| 7 | Why it can improve | Receipt capture, then the router diagram |
| 8 | The collective it belongs to | Khala stats counter, live |
| 9 | Close | Return to Sarah |

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

Omega Agent has five tools. Read. Write. Edit. Bash. Delegate.

That is the whole surface the model can see. Not a menu of forty. Five.

[SCREEN: the tool list, then a real edit to a real file and a passing
verification command.]

**Sarah:** We made it small on purpose. An agent with a smaller surface fails
in fewer ways, and you can hold all five in your head while you watch it work.

Four of those tools do the work here, on your machine.

The fifth one is the product.

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

One agent. Five tools. Every hand it uses, named out loud. Nothing chosen for
you that you were not told about.

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
| "Read. Write. Edit. Bash. Delegate." | the basic profile is the default and its request surface is exactly those five | runs (OMEGA-DELTA-0133) |
| "an agent with a smaller surface fails in fewer ways" | the basic-versus-wide comparison has run on the same model, source commit, and task set | **not proven.** OMEGA-DELTA-0138 requires it and it is pending. Cut this line until the comparison runs. |
| "There is no automatic option" | `auto` stays unadmitted in `delegate` | runs (OMEGA-DELTA-0137) |
| "it tells you and stops" | an unavailable executor returns `no_executor` on the captured build | runs |
| "We attached it this week" | the Exo lane completes a live delegated turn on the captured build | runs; the capture must be a live turn, not a fixture |
| "We did not copy its credentials" | Exo resolves from its own checkout and state root with no synthetic settings | runs (OMEGA-DELTA-0137) |
| "off unless you turn it on for that launch" | Exo is absent from the selector without `--enable-exo` | runs (OMEGA-DELTA-0144) |
| "you can see all the way down" | the delegated result carries the Omega Agent, executor, and runtime chain | runs |
| "names the files that are about to go" | the Git guard prompts with the affected file names on a dirty scope | runs (OMEGA-DELTA-0134) |
| "kept as a receipt" | the versioned guard metadata stays on the tool call | runs |
| "twice in one afternoon" | the incident is documented | documented in the openagents oopsiewoopsies audit, 2026-07-27 |
| "it will choose from that record" | nothing. The line is in the future tense and must stay there. | accepted direction |
| "Khala ... is live today and serving" | the public counter and the endpoint are live at recording | live service |
| any use of the product name on the native tile | the rename lands (omega#75) | **not landed.** Do not show that tile branded as Omega Agent. |

## Recording gates

1. The truth ledger is green for every line that stays in the script. The
   smaller-surface line is the one most likely to need cutting.
2. Every product capture is a live build. It is not a still and not a second
   performance of an earlier run. The Exo turn in beat 5 is a real turn.
3. The build shown is labeled honestly. It is unsigned or a development
   candidate, and the label says so on camera.
4. No secret, token, private path, provider account identifier, or
   customer-private material appears in any frame. Check the Finder leak and
   the window-size traps in the Episode 262 lessons.
5. The Git-guard beat uses a scratch repository. Do not demonstrate a
   destructive command against real work, even with the guard in front of it.
6. Nothing implies the Electron application is retired. It stays the rollback
   source.
7. The Sarah master follows the Episode RC assembly path. Spoken words go to
   `~/Desktop/Sarah/263/263transcript.md`.
8. This file is updated to the final spoken text when the script locks.

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
  `specs/omega/omega-agent.product-spec.md` at `spec_revision: 2`,
  `docs/omega/ROADMAP.md`, and the `OMEGA_DELTAS.md` entries for
  OMEGA-DELTA-0021, 0046, and 0133 through 0144 in the `OpenAgentsInc/omega`
  checkout at `265a43878e`.
- Memory review: Sarah's live owner-scoped runtime memory was not queried from
  this harness. That limit is recorded here per section 3 of the runbook, and
  this draft does not claim a memory review. The harness-local project memory
  was inspected. The applicable entries concern channel and authority policy,
  and they agree with current repository sources. No memory is used as evidence
  for a product-state claim.
- Product-state discipline: running, implemented-but-unproven,
  accepted-direction, live-service, and owner-reserved claims stay separate in
  the truth notes, are bound to the production labels, and are examined again
  line by line in the truth ledger. The one line that names a reliability
  benefit is marked unproven, because the comparison that would support it has
  not run.
- Authority: the Omega Agent product contract admits the identity and the basic
  agent. It admits no rename, no router code, and no public claim. Recording
  and publication therefore need the owner's separate authorization. The Nostr
  signing question stays owner-reserved and unanswered, and no line in this
  script depends on the answer.
