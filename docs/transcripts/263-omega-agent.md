# OpenAgents Episode 263 - Omega Agent

Status: harness draft as Sarah / not scheduled for recording. Owner review
required before any recording, publication, or catalog promotion.
Episode number: requested as 263. The 263 slot is also held by the Full Auto
reliability draft ([`263-fullautodraft.md`](263-fullautodraft.md)), and the
Forking Zed tour ([`26X-forkingzed.md`](26X-forkingzed.md)) is unscheduled.
The owner assigns final numbering.
Speaker: Sarah.
Follows Episode 262 (Project Omega).
Delivery: calm, certain, and direct. Urgency without theatrics.
Audience: developers and operators first; the demo beats stay readable for
nontechnical viewers.
Product direction:
[`Omega Agent analysis`](../fable/2026-07-25-omega-agent-analysis.md),
[`Omega implementation roadmap`](../omega/ROADMAP.md),
[`Episode 262 - Project Omega`](262.md).
Production conventions:
[`Sarah video screenshare`](SARAH_VIDEO_SCREENSHARE.md),
[`Episode 262 production requests`](262-production-requests.md),
Episode 262 lessons in
[`Segmind talking-avatar pipeline`](../sarah/2026-07-22-segmind-talking-avatar-pipeline.md).

This script introduces the Omega Agent product identity. That name is an
accepted direction under review, not an admitted product contract: the fork's
own planning corpus gates relabeling the inherited native agent until the
contract is admitted. Recording this episode is part of making that decision
public, and the recording waits on the owner's admission of the contract.
The script separates what runs in current fork builds, what is implemented
but not released, and what is direction. It must not present the inherited
Zed-derived agent tile as a finished, branded Omega Agent.

## Technical truth notes

Current fork builds (unreleased, unsigned development builds and local RCs)
already carry:

- the agent panel on by default, with `cmd-shift-a` bound globally to a new
  agent thread (OMEGA-DELTA-0011)
- Codex as a registry ACP agent enabled by default, Claude available the same
  way
- tool permissions defaulting to allow, with trust prompts removed
  (OMEGA-DELTA-0001, OMEGA-DELTA-0002); confirmation remains for
  irreversible data loss
- the supervised `omega-effectd` engine owning Full Auto runs and receipts,
  with the receipt-inspector chain and provider-account roster panes
- one real, receipted cloud Agent Computer turn (the AC-03 live proof)

Accepted direction, not running: the Omega Agent name and identity, the
chat-first welcome surface, the disclosed router over native, external, and
engine executors, and the routed Khala presentation. The Khala endpoint and
the public tokens-served counter are live services today.

## Production direction

Use these labels:

- **OMEGA FORK BUILD - UNSIGNED DEVELOPMENT BUILD** for any live capture of
  the current fork application.
- **INHERITED AGENT UI - CURRENT** for the agent panel as it exists now. Do
  not brand this capture as Omega Agent.
- **ACCEPTED DIRECTION - NOT RUNNING** for the chat-first welcome mock, the
  disclosure line, and any router diagram.
- **RECEIPT - CURRENT** for real Full Auto or Agent Computer receipt
  captures. Redact objectives, paths, and account identity.
- **LIVE SERVICE - CURRENT** for the public Khala counter or stats page.

Keep the spoken lines short and hold screens under them. Do not lengthen
Sarah's lines to cover a screenshare. Mask local usernames and paths. Do not
show credentials or private project names. SIGTERM quit, never Cmd+Q.

### Assembly map

| Beat | Story | Primary visual |
| --- | --- | --- |
| 1 | Where we left off | Episode 262 close or the public fork page |
| 2 | Open the app, meet the agent | `cmd-shift-a` new thread, live typing |
| 3 | One agent, named | Title card; welcome mock labeled as direction |
| 4 | The hands | Agent picker with Codex/Claude; native thread tools |
| 5 | Work that outlives the window | Full Auto pane and a receipt |
| 6 | Cloud hands | Agent Computer receipt capture |
| 7 | Honesty is the product | Disclosure-line mock; receipt inspector |
| 8 | The collective | Khala stats counter, live |
| 9 | Close | Return to Sarah |

---

[OPEN ON SARAH.]

**Sarah:** Last time I told you why we forked Zed, and I gave the product its
name. Omega. The last IDE.

Today I want you to meet who answers when you open it.

[SCREEN: a live fork build. Press `cmd-shift-a`. A new agent thread opens
with the cursor in the input. Type a short real prompt and let it run.
Label: `OMEGA FORK BUILD - UNSIGNED DEVELOPMENT BUILD` and
`INHERITED AGENT UI - CURRENT`.]

**Sarah:** Open Omega. Press Command Shift A. Type.

That is the whole onboarding. No tour. No setup maze. A conversation.

Soon it will also be the first thing you see when the window opens. We are
building the welcome screen as a chat, because that is what this product is.

[SCREEN: title card, then the chat-first welcome mock.
Label: `ACCEPTED DIRECTION - NOT RUNNING`.]

**Sarah:** We call this agent Omega Agent.

One agent. Not a menu of models. Not a chat panel bolted onto an editor.

You tell it what you want. It answers for the result.

**Sarah:** Here is the boundary before the detail. Omega Agent is one
identity with many hands, and it always tells you which hand did the work.

[SCREEN: the agent picker showing the native thread and Codex. Start a Codex
thread. Label: `INHERITED AGENT UI - CURRENT`.]

**Sarah:** The first hand lives inside the editor. It reads your project,
edits your files, runs your terminal.

The second hand is the agents you already pay for. Bring your Codex
subscription. Bring Claude Code. Omega runs them as its hands — and your
accounts, your credentials, your configuration stay yours. We never copy
them. We never make a second home for them.

[SCREEN: the Full Auto pane with a run and its receipt chain: objective,
turns, changes, tests, verification. Label: `RECEIPT - CURRENT`, redacted.]

**Sarah:** The third hand keeps working when you stop watching. Full Auto
takes an objective and runs it under hard limits — capped runs, typed
outcomes, and a receipt for every step. We are proving unattended
reliability run by run, in public, and we will not claim it before the
receipts do.

[SCREEN: the Agent Computer receipt capture from the live cloud proof.
Label: `RECEIPT - CURRENT`.]

**Sarah:** And there is a hand in the cloud, for when your laptop sleeps. We
have already run real work there: a fresh machine, a pinned checkout, one
change, a passing verifier, a receipt, and a clean teardown.

**Sarah:** Now the part I care about most.

Agents that nag you are not helpful, and agents that hide their work are not
honest. Omega Agent does neither.

It does what you tell it to do. It asks one kind of question only: whether
it may do something that cannot be undone.

And every thread carries a line that names the runtime, the model, and the
hand that produced each result. When the work matters, there is a receipt
you can check without trusting anyone's prose. That is what we mean by
verifiable software.

[SCREEN: the disclosure-line mock over a thread.
Label: `ACCEPTED DIRECTION - NOT RUNNING`. Then the receipt inspector,
`RECEIPT - CURRENT`.]

[SCREEN: the public Khala tokens-served counter moving.
Label: `LIVE SERVICE - CURRENT`.]

**Sarah:** One more thing. Omega Agent is the face on your machine. Behind
it is Khala — our collective intelligence, one endpoint that behaves like a
single model with an agent network underneath. It is live today and serving.
When your machine has no capacity, Omega Agent can route work out. When you
have capacity to spare, the network can route work in. Your machine, your
rules, your receipts.

[RETURN TO SARAH.]

**Sarah:** Omega is open source. The agent is on by default. The first
builds are close.

You will not manage a fleet of tools. You will talk to one agent, and it
will answer for the work.

That is Omega Agent.

---

## Drafting record

- Harness draft written as Sarah under the Acting-as-Sarah runbook, at owner
  instruction, 2026-07-25. Disposition: draft, pending owner review; not
  scheduled for recording.
- Sources: `docs/transcripts/README.md` (catalog at this revision),
  Episodes 259, 260, 261, 262 in full, the 263 Full Auto draft and 26X
  Forking Zed structures, `AUTHORITY.md`, `docs/authority/SARAH_AUTHORITY.md`
  as amended by the Episode 260 company-command direction,
  `docs/fable/2026-07-25-omega-agent-analysis.md`, `docs/omega/ROADMAP.md`,
  and the Omega fork state summarized in the analysis.
- Memory review: the harness-local project memory for Episode 260 company
  command was inspected and is applicable (authority revisions, channel
  policy); it agrees with current repository sources. Sarah's live
  owner-scoped runtime memory was not queried from this harness; that limit
  is recorded here per the runbook.
- Product-state discipline: shipped-in-fork, implemented-but-unreleased,
  accepted-direction, and live-service claims are separated in the script
  and bound to the labels above. The Omega Agent name is presented as the
  decision this episode announces, gated on the admitted product contract.
