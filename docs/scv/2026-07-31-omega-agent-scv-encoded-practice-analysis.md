# Encoding operating practice into the runtime — Omega Agent, SCV, and the 1.91B lesson

- **Date:** 2026-07-31
- **Document type:** engineering analysis with a strategic frame. It changes no
  runtime authority, allocates no delta number, closes no issue, and grants
  nothing.
- **Subject repositories:** `OpenAgentsInc/openagents` at
  `/Users/christopherdavid/work/openagents`, `origin/main` `5c7d0cbe78`;
  `OpenAgentsInc/omega` at `/Users/christopherdavid/work/omega`, `ee4d39d159`.
- **Primary evidence:**
  [`docs/afteraction/2026-07-31-codex-019fb495-overnight-spend-audit.md`](../afteraction/2026-07-31-codex-019fb495-overnight-spend-audit.md).
  Every quantity in this document comes from that audit or from source read in
  the two repositories above. Nothing here is re-measured, and no new
  measurement is claimed.
- **Position in the chain:** the audit established what a 17-hour third-party
  harness run cost and why. This document asks the next question — which of
  those costs a runtime we own can refuse mechanically — and answers it against
  the code that exists today. It is upstream of no roadmap and is not dispatch
  authority.
- **STE governance:** out of scope. `docs/scv/` is not a governed public prefix
  (`docs/ste/checker-config.v1.json`, `governedPrefixes`).

## 0. The claim

A prose instruction is a request. A typed contract is a fact. The audit's
central finding is that a competent agent, given broad authority and no cost
model, spent 1,905,972,584 tokens and did not close its objective — and that
the largest single cost drivers were not decisions the model got wrong so much
as decisions the harness made cheap, invisible, or default. A third-party
harness gives us exactly one lever against that: better prose in `AGENTS.md`.
The audit contains the refutation of that lever. `AGENTS.md` lines 72–83
instructed short waits, and the measured consequence was 657 one-second polls
costing 82,534,501 tokens.

Owning the runtime replaces the lever. Every operating practice the audit
proposes can become a type, an admission predicate, or a receipt, and every one
of those fails a build or refuses a call instead of asking a model to remember.
Omega Agent already demonstrates the pattern on a different axis
(`OMEGA-DELTA-0203`, `OMEGA-DELTA-0209`: a policy with no default and no third
case does not compile when someone forgets it). SCV demonstrates the other half:
work that requires no judgement can be executed by something with no model, at
zero model-token cost, with typed refusals.

This document classifies the audit's failures by owner, states the mechanism for
each, argues what SCV should become, and gives a build order. It also states
plainly where the argument does not reach.

---

## A. Diagnosis — model failure or harness failure

The audit ranks six cost drivers plus a seventh finding (no stop rule). The
useful cut is not "who is at fault" but "what would have had to change for the
cost not to be incurred": if a differently-disciplined model on the same harness
would have paid it anyway, it is structural.

| # | Driver | Measured | Owner |
| --- | --- | --- | --- |
| 1 | Context architecture: 98.6% of input was cached re-read, ~64x amplification | 869,530,540 of 882,135,489 input tokens; median 135,214 per call across 6,794 calls | **Harness** |
| 2 | Polling and waiting | 28.3% — 254,481,925 tokens over 1,881 model calls; 657 one-second polls, 82,534,501 tokens (9.3%) | **Harness affordance + written contract** |
| 3 | Sub-agent fanout and its accounting | 13.2% parent overhead (118,682,325); children 1,022,615,093 = **53.7% of the true total**, invisible to the parent counter | **Harness accounting** (invisibility) + **model** (four audits of one issue, 101M) |
| 4 | Debug-in-production | 21 Cloud Run deploys, 30 GKE rollouts, 23 image builds, 42 live acceptance runs, 14 env-var updates; 9 of 10 root causes were configuration or state-predicate bugs | **Model** (no re-plan) + **platform gap** (no admission predicate, no config-parity environment) |
| 5 | Scope drift into a visual-baseline lane | 17 `view_image` calls, 5,016,094 bytes = 20.2% of all tool-output bytes; one unchanged screenshot loaded five times, ~1.84 MB | **Model** (lane choice) + **harness** (no digest rule, no per-lane image budget) |
| 6 | Command repetition | 628 exact repeats (15.5%), 877 near-duplicates (21.7%), but only **8.1%** of shell-output bytes | **Harness** (compaction symptom), second-order |
| 7 | No stop rule | 400,648,850 tokens (45.4%) after the last human message; still running at ~18 hours | **Harness contract gap** |

### A.1 The three that are unambiguously structural

**Waiting (28.3%).** `write_stdin` was called 1,079 times, 1,049 of them
(97.2%) with `chars: ""` — sending nothing, existing only to learn whether an
already-running process had finished. The distribution is the tell: 657 of the
empty polls waited **one second**. At the thread's median context that is
roughly 135,000 tokens to buy one second of wall clock. The model chose a legal,
documented, cheap-*looking* action whose true price was invisible at the point
of decision, and the repository's own "Constant Motion" clause told it to prefer
exactly that shape. A harness that offers a sub-second yield on a full-context
turn, and prices it nowhere the caller can see, will produce this behavior from
any model that is trying to be responsive. It is not a judgement failure.

**Fanout accounting (53.7% invisible).** Every observed `spawn_agent` used
`fork_turns: "all"`, so each child inherited the entire parent context at fork
time — routinely 130,000–250,000 tokens — and then paid that base on every one
of its own calls. Each child writes its own rollout file with its own counter.
The owner reported ~600M against a measured true total of 1.91B, a 3.2x
under-count, and the audit's explanation is simply that the parent's counter is
the visible one. This is an accounting failure before it is a discipline
failure: *no operator or coordinating agent can manage a budget that is 54%
invisible*, however disciplined they are. The four `audit_9285_*` lanes
(101,185,858 tokens across four full-context forks of one issue) are a genuine
model choice on top of that — but a model choosing a fourth audit while the
first three appear to have cost nothing is choosing under a broken price signal.

**No stop rule (45.4% after the last human message).** The thread never had a
budget object to check against, because none exists. The audit is precise that
its status reporting stayed honest throughout — it consistently distinguished
"code-complete" from "release-complete" and never claimed the community room had
passed. What it never did was treat "I have been one fix away for six hours" as
information. A terminal condition that exists only as an intention in a
conversation is erased by the next compaction; this thread compacted 31 times.
Finding 6 is the proof by demonstration: the Chrome question was resolved by the
owner at `06:59:02`, and the thread re-announced it as a fresh event at least
seven more times over the following five hours. Conversational memory is not a
ledger, and no amount of care makes it one.

### A.2 The two that are genuinely the model's, and what still enabled them

**Debug-in-production.** The audit's §7.1 addendum is the strongest statement
of this and it is the subject agent's own: *"I evaluated each cycle locally:
'this is a new root cause, therefore one more fix is rational.' I did not
evaluate the series globally."* Every individual cycle was defensible; the
series was not. That is a planning failure a harness cannot make for you.

What a harness *can* do is refuse the twenty-first production deploy that is not
preceded by a passing test at the same source revision. Nine of the ten root
causes in the final six hours were configuration or state-machine predicate
bugs — the class an environment with the same configuration *shape* surfaces
without a rollout. The missing admission predicate is a platform gap; the
failure to notice the pattern is the model's.

**Scope drift.** Opening Omega #191 while the release gate was red is a lane
choice, and the thread's own narration at `10:56:29` states the motive. But the
specific cost — the same screenshot loaded five times for ~1.84 MB — is not a
judgement question at all. A content digest makes the second, third, fourth and
fifth loads impossible. `view_image` averaged 295,064 bytes per call against
everything else in the session and was the single worst per-call offender in the
tool census; 17 calls produced 20.2% of all tool-output bytes.

### A.3 Repetition is the one everybody blames and it is second-order

628 exact repeats sounds damning until it is priced: 8.1% of shell-output bytes.
The audit names the diagnostic shape — `sed -n '<start>,<end>p'` on the same
file 8–10 times — and correctly reads it as *"the earlier read scrolled out of
the compacted window"*, a compaction symptom rather than carelessness. Fixing
repetition without fixing context architecture moves 8.1% of bytes, not 8.1% of
tokens, and the two are not the same number.

### A.4 The honest summary

The categories where the harness set the price — waiting, fanout accounting, the
missing stop rule, the compaction-driven re-reads — dominate the measured spend.
The categories that are distinctly the model's — four audits of one issue, the
production-debug loop, the unrelated lane — are real, expensive, and smaller.

One caveat the audit states about itself and this document inherits: attribution
of tokens to tool types (§2.2) matches each `token_count` event to the tool call
emitted immediately after it. It is *"an inference about which decision the
context was spent on, not a billing record"*, reliable in aggregate, possibly
off by one at boundaries. The token totals themselves are exact.

---

## B. What Omega Agent and SCV can enforce that a third-party harness cannot

For each failure mode: the mechanism, and — separated explicitly — what exists
in the tree today versus what is proposed.

### B.1 Model-free execution for work that needs no judgement

**Exists.** SCV (`crates/scv`, version `0.1.0`, 1,627 lines of Rust across seven
files, tests included) is a leaf crate with no dependency on Omega in its
shipped binary. It is an ACP
server with exactly one tool, `read`, and no model anywhere in its execution
path. It resolves a request, reads bytes, and returns a typed record. A `read`
executed by SCV costs zero model tokens *at SCV*.

**The honest limit.** SCV does not make a read free. The delegating parent still
pays one model call to issue the request and one to receive the result, at
whatever its resident context is. The saving materializes in exactly two ways:
when one delegated call replaces many round trips (a wait; a batched query), and
when the returned record is bounded by construction rather than by the caller's
discipline. Section C is ordered on that basis and not on "reads are free."

### B.2 A structured executor cannot be handed prose

**Exists.** `AgentPromptContract` in `crates/omega_agent_detect` is
`Prose | Structured(StructuredPrompt { tools, request })`. There is no default
and no third case, so a candidate added to `CANDIDATES` without a contract does
not compile. `scv` is the only `Structured` entry; `codex-acp`, `claude-acp`,
`grok`, `github-copilot-cli` and `cursor` are `Prose`.
`SpawnAgentTool::description` generates the structured-contract section from the
same catalog, so an agent added to the catalog is described by the edit that
adds it. `task_for_executor` shapes or refuses **before the process is
launched**, and the refusal is its own failure class,
`DelegateFailureClass::TaskNotInContract`, because it asks for a rewrite rather
than a retry (`OMEGA-DELTA-0209`).

**Why it generalizes.** This is the reusable trick, and it is worth naming apart
from its instance: *the policy that is easiest to forget becomes the one the
compiler will not let you omit.* `AgentLaunch` did it for "offered implies
startable" (`OMEGA-DELTA-0203`); `AgentPromptContract` did it for "offered
implies sends-a-shape-it-accepts". A budget, a terminal condition, and a gate
ledger are the same shape of obligation and can take the same enforcement.

**The limit, stated in the delta itself.** A structured agent still cannot be
delegated prose, and no error message gives it a model. What the mechanism
guarantees is that the delegating model is told the shape before it writes the
task, and told again if it writes the wrong one.

### B.3 Bounded briefs by construction, not by discipline

**Exists, and this is the largest single structural difference from the audited
harness.** `Thread::new_subagent` (`crates/agent/src/thread.rs:1513`) copies the
parent's project, project context, context-server registry, templates, model,
and a linked action log, then calls `inherit_parent_settings` for speed,
thinking, summarization model, and profile. **It does not copy the parent's
messages.** The child starts with an empty conversation and the `task` string is
its entire brief. There is no `fork_turns: "all"` in Omega, and no way to
express one.

The audit's recommendation #2 — *"Default `fork_turns` to a bounded brief, not
`"all"`"*, estimated at 20–30% of the true total — is therefore already true of
Omega's native loop by construction rather than by default setting. That is not
a claim that Omega's fanout is cheap; it is a claim that the specific
1,022,615,093-token mechanism measured in the audit cannot occur there.

**Proposed.** Make the brief a typed record instead of a free string: objective,
contract reference, path set, pinned source revision, verification command,
budget, terminal condition. Today the requirement is prose inside
`DELEGATE_GUIDANCE` — *"An agent does not see your conversation history. Include
all relevant context"* — which is exactly the class of instruction the audit
proves decays. A schema makes a brief checkable, and makes "did the child get a
pinned revision?" a question with an answer.

### B.4 Per-child spend rolled up to the parent

**Exists (the halves).** `Thread` carries `cumulative_token_usage`, and a
subagent carries `SubagentContext { parent_thread_id, depth }`. The tree
structure and the per-node counter both exist.

**Missing.** The sum. A grep of `crates/agent/src/thread.rs` finds no descendant
rollup: there is no aggregate a parent, an operator surface, or an admission
check can read. This is the mechanism whose absence produced the audit's 3.2x
under-count, and in Omega it is a small addition rather than a program, because
the parent link is already durable.

**Proposed.** A descendant-usage projection on the parent thread plus budget
events at 50%, 80% and 100%, matching the audit's P0 *"aggregate budget
visibility"*. The rollup is not itself a saving — it is the precondition for
every other saving in this section being *managed* rather than hoped for.

### B.5 Budget as a typed resource with hard admission

**Missing entirely.** A search of `crates/agent/src` for budget, spend, or stop
mechanisms finds byte budgets on tool results
(`TOOL_RESULT_PREVIEW_BYTE_BUDGET`) and on retained user messages during
compaction (`COMPACTION_RETAINED_USER_MESSAGES_BYTE_BUDGET`), and nothing about
tokens, cost, or a terminal condition.

**Proposed.** A budget record attached at work-unit creation — tokens,
wall clock, external-gate attempts — durably written **before** dispatch, on the
shape `OMEGA-DELTA-0179` already establishes for routing: *"Persistence precedes
dispatch... A failed write blocks the send."* That delta is the existence proof
that Omega can make a durable record a precondition of an executor call rather
than a log written after it.

The budget must be an **admission predicate**, not advisory telemetry. A
`spawn_agent` call, or any production-mutating tool call, is refused when the
remaining budget does not cover its projected cost, with a typed class in the
`TaskNotInContract` spirit: this asks for a different action, not a retry. An
advisory budget is a number in a prompt, and the audit is a 1.91-billion-token
demonstration of what a number in a prompt is worth against a model that is
trying to finish.

### B.6 A terminal condition is mandatory or the run does not start

**Missing.** **Proposed.** Every autonomous run carries a close rule as a typed
predicate. A run created without one does not start — the `AgentLaunch` /
`AgentPromptContract` shape applied to autonomy: no default, no third case, a
compile or admission failure rather than an omission nobody notices.

The audit's §7.1 names why "all issues closed" was not usable as a near-term
stop rule: several close rules required installed multi-client evidence,
independent review, and network conditions the loop could not manufacture. A
typed close rule makes that unsatisfiability visible at creation instead of at
hour seventeen.

### B.7 Three-strike gate enforcement as durable runtime state

**Missing.** **Proposed.** An external-gate ledger, durable, keyed by gate and
work unit, carrying exactly the audit's P0 columns: attempt, source revision,
deployed revision, hypothesis, observed failure class, what changed from the
prior attempt, receipt reference. An attempt increments only when the external
gate actually runs. A fourth attempt is refused unless the record shows new
evidence or an explicit owner extension.

Durability is the whole point. Finding 6 is the demonstration that a fact held
only in conversation is re-derived after compaction, and this thread compacted
31 times. A three-strike rule written in `AGENTS.md` is subject to precisely the
same erasure as the Chrome resolution.

### B.8 Production mutation admitted only behind a passing test

**Missing.** **Proposed.** A deploy or production-mutating tool call requires a
named test result whose revision matches the source revision being deployed, and
emits a receipt on both sides. Nine of the ten root causes in the audit's final
six hours were configuration or state-predicate bugs, each costing a full
build-deploy-rollout-acceptance cycle. The audit's own conclusion is that
instrumenting once, first, would plausibly have collapsed the 11:00–12:50 window
— roughly 138M tokens — into two or three cycles.

This one is only half ours. The predicate belongs in the runtime; the
configuration-parity environment it predicates on is an openagents platform
program (audit §7.5, P0). Naming both prevents the platform half from being
silently assumed.

### B.9 An unchanged artifact is never re-read

**Exists (the neighbour).** `Thread::run_tool` centrally bounds text tool
results; anything over `TOOL_RESULT_PREVIEW_BYTE_BUDGET` is retained in a
thread-scoped versioned `ToolResultArtifactRegistry`, and the model receives a
bounded preview plus an address it can spend via `read_tool_result_artifact`.
That is the audit's recommendation #4 already implemented for large text.

**Missing.** The digest sibling. A content hash on observation, so a re-read of
an unchanged path returns the digest and the prior reference rather than the
bytes. This kills the ~1.84 MB unchanged-screenshot class outright and blunts
the `sed -n` compaction-re-read pattern.

**Also worth stating:** Omega's tool catalog has no image-viewing tool at all
(`crates/agent/src/tools/`), so the single worst per-call offender in the
audited session — 295,064 bytes per `view_image` call — has no current
equivalent. Any future one should ship with the digest rule and a per-lane
budget already attached, not added afterwards.

### B.10 No waiting affordance that costs a model turn

**Exists by absence.** Omega's tool catalog contains no `wait`, no `wait_agent`,
no `list_agents` polling surface, and no empty-`write_stdin` equivalent.
`terminal_tool` takes a `timeout_ms` and its own guidance already says *"For
potentially long-running commands, prefer specifying `timeout_ms` to bound
runtime."* The 28.3% mechanism does not currently exist in Omega.

**Proposed.** Fence the ground we already hold: a delta plus a mechanical check
asserting that no waiting affordance below a yield floor can enter the tool
catalog. This is the cheapest high-value item in the entire document, because it
prevents a regression rather than repairing one — and the audit is the evidence
that this affordance gets added by well-meaning people for latency reasons.

### B.11 Every policy carries a check that fails the build

**Exists, on a different axis.** `OMEGA_DELTAS.md` (9,770 lines) plus
`crates/omega_deltas` (26,964 lines of checks) enforce that every deliberate
divergence from upstream has an ID that appears in the code it governs, its own
programmatic check, and its own test — and that *"a delta whose check cannot
fail is not a check"* and *"removing a delta is a policy change."* The registry's
stated reason is that a rebase silently reverts a comment and a value together.

**Proposed, and honestly labelled as an extension.** The registry today governs
Omega-versus-Zed behavioral divergence, not operating practice. Applying the
same discipline to the nine process rules the audit installed in `AGENTS.md`
§7.3 is a proposal. It is the right one, for the reason the audit itself
supplies: the "Constant Motion" clause was a written operating rule that
survived unchallenged for months and *produced* 9.3% of a 883M-token thread.
Prose policy does not merely decay; it can be actively wrong and stay wrong,
because nothing tests it.

Not every rule has a mechanical form. "No unrelated backlog as motion theater"
probably does not. Record that honestly per rule rather than pretending
uniformity.

---

## C. The SCV thesis

### C.1 What SCV is today, exactly

Verified against `crates/scv` at `ee4d39d159`:

| Property | Current |
| --- | --- |
| Surface | One tool, `read`. `session/new` advertises exactly one available command (`only_read_in_available_commands`). |
| Model | None. `PromptToolRequest::parse` is `serde_json::from_str` on the prompt text. |
| Input | `{ path, offset ≥ 1, limit 1..=2000 }`, `additionalProperties: false` |
| Output bound | `MAX_CONTENT_BYTES = 1_048_576`; `truncated: true` when stopped at the bound; `response_too_large` when no complete line fits |
| Confinement | Absolute path required; lexical check plus post-`canonicalize` root check; symlink escape rejected; regular files only; valid UTF-8 only, no lossy decode; never mutates |
| Refusals | Seven typed codes carrying a public-safe message and the requested path — never file content, never OS error strings |
| State | A session id from an `AtomicU64`. Nothing persists. |
| Dependencies | `agent-client-protocol` (pinned `=2.0.0`), serde, serde_json, thiserror, tokio, tracing. The shipped binary depends on nothing of Omega's. |

Two properties are load-bearing beyond their size. First,
`PROMPT_REQUEST_SHAPE` is one string used three ways — the hint a client reads
at `session/new`, the text a refusal names, and the value
`omega_agent_detect::SCV_REQUEST` is checked against — so the shape Omega tells
a model to emit cannot drift from the shape SCV accepts. Second, the fence rule:
`strip_one_code_fence` removes a fence that opens *and closes* the whole text,
and the test `a_request_inside_prose_is_not_extracted` asserts that a request
buried in a message is **not** dug out, because *"choosing which part of a
message was the request is interpretation, and an agent with no model must not
interpret."*

### C.2 What it should become

SCV should become the deterministic, model-free execution substrate for the
class of work that consumes model calls and requires no judgement. The audit's
own command census is the target list. Of 3,717 categorizable shell commands:
612 file reads (5,727,042 output bytes), 624 searches (4,404,145), 481
git status/diff/log/rev-parse (1,776,502), 213 build/deploy status polls
(296,302) — plus the entire 254,481,925-token waiting bucket, which is not shell
work at all but pure observation.

Every one of those is a bounded question with a mechanical answer.

### C.3 Capability roadmap, ordered by the audit's measured savings

| Order | Capability | Measured target | Why here |
| --- | --- | --- | --- |
| 1 | `observe` / `wait` | **28.3%** — 254,481,925 tokens, 1,881 calls | Largest measured category by a wide margin. One handle, a terminal-state set, a deadline, one bounded result. Model-free by construction: SCV cannot narrate a wait, because it has nothing to narrate with. |
| 2 | `digest` | The ~1.84 MB unchanged-screenshot class; the `sed -n` re-read pattern | Smallest possible addition (a content hash), and it is the precondition for B.9 in any caller, not only in Omega. |
| 3 | `search` | 624 searches, 4,404,145 output bytes | Typed grep with a match cap and a count-first mode — the audit's own item #4 discipline (`rg -c` before `rg -n`) as a contract rather than a habit. Returns counts and spans, not files. |
| 4 | `status` | 481 calls, 1,776,502 bytes | A bounded repository-state record — revision, dirty paths, ahead/behind, ancestry predicate — instead of porcelain text. This is also the exact shape the audit's §7.4 worktree cleanup needs (`git status --porcelain` empty **and** `git merge-base --is-ancestor`), and encoding it once removes a procedure people perform by hand under time pressure. |
| 5 | `list` | Directory listing | The `docs/scv/README.md` v0.2 "Inspect" step. Cheap, bounded, obvious, and the least valuable of the five by the audit's numbers — which is why it is last rather than first. |

### C.4 The boundaries that must not move

- **No model, ever.** The moment SCV interprets, it stops being deterministic
  and stops being free. `a_request_inside_prose_is_not_extracted` is the
  codified form of that boundary and must survive every capability addition.
- **No authority widening.** `observe`, `digest`, `search`, `status` and `list`
  are all read-only, which is what lets a model hand SCV a task without an
  approval gate. `observe` on a process handle is the first capability that
  touches something a caller started, and it is the one to design carefully: it
  must observe, never start and never signal. The existing `docs/scv/README.md`
  roadmap places writes at v0.3 and command execution at v0.4 — those steps
  should each carry their own policy gate, and none of the five capabilities
  above depends on reaching them.
- **Refusals stay typed and readable.** Seven codes today, plus the
  `OMEGA-DELTA-0209` rule that a refusal names the shape it wanted. *"expected
  value at line 1 column 1"* states where a parser stopped and nothing a reader
  can act on.
- **No state.** Sessions are a counter and nothing persists. A cache is the
  first thing that can be stale and therefore the first thing that can lie.

### C.5 One drift to reconcile

`docs/scv/README.md` in this repository specifies SCV as a standalone crate
named `scv-acp`. What shipped is `crates/scv` inside the omega workspace, binary
`scv`, with a dev-only dependency on `omega_agent_detect` so the delegation test
drives SCV with the contract Omega actually advertises rather than a copy of it
(`OMEGA-DELTA-0209`), and bundled beside the `omega` executable
(`script/bundle-mac`, `script/bundle-omega-rc`, packaged-executable floor raised
from 3 to 4 in `bbd78b180b`). The design document and the implementation should
be reconciled in one direction or the other; today a reader of the spec would
look for the wrong crate.

---

## D. Migration posture — Codex subordinate, not removed

### D.1 The boundary already exists and is typed

Codex becomes one named external executor behind our router. This is not new
construction:

- `AgentCandidate { id: "codex-acp", name: "Codex", binaries: ["codex"], launch: AgentServerStore, prompt: Prose }`
  — Codex is first in `CANDIDATES` because it is what the first message routes
  to.
- `resolve_subagent_executor` never substitutes. A request for `codex-acp`
  either runs Codex or fails saying it could not, and the typo `codex-acpp` is
  refused rather than becoming a silent inherit — *"a subagent that reports as
  Codex and is not is the same defect class as an undisclosed provider
  handoff."*
- `SubagentExecutorReport { class, agent_id, provider?, model? }` is returned to
  the parent model, so a mixed fanout is attributable result by result. An
  external ACP agent does not report its model, and `provider`/`model` are
  **absent rather than guessed**.
- `OMEGA-DELTA-0179`: the route decision is a durable receipt written before
  dispatch; a selected executor is immutable, and its disappearance is a named
  error rather than a silent fallback to the native loop.
- `OMEGA-DELTA-0202`: one routed decision behind every model label, with
  `Thread::active_turn_model` as the single model-level authority.

Codex keeps its own home, credentials, sessions, tools, and loop. What moves to
our plane is orchestration, budget, admission, receipts, and stop rules.

### D.2 What we lose, honestly

- **Per-tool-call control inside a Codex turn.** What Codex does mid-turn is
  Codex's. We gate what we hand it and what we accept back; we do not gate its
  seventh shell command. Replacement: brief plus budget plus gate ledger at the
  delegation boundary, and a receipt on return. That is strictly less control
  than a native turn and should not be described otherwise.
- **Token accounting.** Making Codex a subagent does not fix the audit's central
  accounting failure. An external ACP agent does not report usage to Omega
  today, which the disclosure record already models honestly by carrying
  `None` — *not disclosed*, distinct from empty. Replacement options are
  wall-clock and call-count proxies plus the provider's own billing, or an
  explicit unmeasured-spend marker on the work unit. **Do not fabricate a
  number**; an invented child cost is worse than a visibly absent one, because
  it makes a broken budget look enforced.
- **Context and compaction discipline inside the child.** We cannot impose a
  resident-context ceiling on a runtime we do not own.
- **Nothing about model capability.** Subordinating Codex is a claim about who
  owns the loop, not about who owns the model. The native loop runs whatever
  model is configured. Any argument of the form "our runtime is cheaper" that
  rests on a different model is a different argument and is not made here.

### D.3 What we do not lose

The audited session landed 161 commits on `openagents` `main`, 130 of them
LiveKit or Sarah related, and the audit spot-verified two. Codex's value as an
implementation agent is not in question and is not being replaced. The failure
was, in the subject agent's own words, *"not competence. It was the absence of a
cost model and a stop rule around a competent loop running at 135,000 tokens per
decision."*

---

## E. What to build first

Expected waste reductions are the audit's estimates carried forward, not new
measurements. Items marked **partial** have an implementation already in the
tree.

### E.1 Small mechanisms — days each

| # | Item | Target | State |
| --- | --- | --- | --- |
| 1 | Descendant token rollup on the parent thread, plus 50/80/100% budget events | The 53.7% invisibility; audit P0 "aggregate budget visibility" | **Partial** — `SubagentContext { parent_thread_id, depth }` and per-thread `cumulative_token_usage` exist; the sum and its surface do not |
| 2 | Digest-before-read in the tool layer | The ~1.84 MB unchanged-artifact class; residue of the est. 5–8% output-bounding item | **Partial** — `TOOL_RESULT_PREVIEW_BYTE_BUDGET` and `ToolResultArtifactRegistry` already bound large text and give the model an address |
| 3 | SCV `digest` capability | Same, for any caller | New; one tool, no new authority |
| 4 | A delta and check forbidding any waiting affordance below a yield floor | The mechanism behind 9.3% | New; cheap because Omega has no wait tool today — this fences held ground |

Item 1 buys no tokens by itself. It is listed first because every later item is
unmanageable without it.

### E.2 Bounded mechanisms — a week or two each

| # | Item | Target | State |
| --- | --- | --- | --- |
| 5 | SCV `observe` / `wait` | **28.3%**, est. 20–26% saving | New; first capability touching a caller-started process, design accordingly |
| 6 | Typed subagent brief — record, not string, with pinned revision and verification command | Makes the already-bounded brief checkable | **Partial** — `Thread::new_subagent` already passes no conversation; `DELEGATE_GUIDANCE` states the requirement as prose |
| 7 | External-gate ledger with fourth-attempt refusal | Finding 7 (45.4% after last human input) and Finding 6 (re-derivation across 31 compactions); est. 15–25% | New; small state, large behavioral effect |
| 8 | SCV `search` and `status` | ~6.2 MB of measured tool-output bytes | New |

### E.3 Programs — a quarter, and they should be named as programs

| # | Item | Target | Note |
| --- | --- | --- | --- |
| 9 | Budget as a typed resource with hard admission across `spawn_agent` and production-mutating tools, persisted before dispatch | Converts items 1 and 7 from visibility into enforcement | Touches routing, the tool layer, persistence, and an operator surface. `OMEGA-DELTA-0179` is the shape to copy: persistence precedes dispatch, a failed write blocks the send |
| 10 | Configuration-parity integration environment plus the production-mutation admission predicate | Est. 10–15%; audit §7.5 P0 | **The environment half is an openagents platform program, not an Omega Agent one.** Naming both halves prevents the platform half being silently assumed |
| 11 | Operating practice migrated from prose into checked policy | Prevents the "Constant Motion" failure mode from recurring in a new clause | Extension of the delta registry's discipline to a new axis. Record per rule whether a mechanical check is possible, and say so when it is not |

### E.4 What is deliberately not on this list

Deleting gates. The audit is explicit that `check:fast` on pushes to `main`
caught real drift twice in that session, that the live two-room acceptance is
load-bearing, and that the fail-closed refusals at `10:02` and `11:24` were
correct and worth their cost. *"None of these are the expensive part. The
expensive part is the 135,000-token context they run inside."* Batching pushes
by landed unit (142 pushes for 161 commits) is a scheduling change, not a gate
removal, and belongs in E.1 as an operating rule rather than a mechanism.

---

## F. What this analysis does not establish

- **No number here is new.** Every quantity is from the after-action audit or
  from source read at the two pinned commits. The audit's own limitations carry
  forward: its tool-to-token attribution is a stated inference; the `$900` is
  owner-reported and appears in no transcript; sub-agent model-call counts are
  estimated while their token totals are measured; the session was still running
  at the snapshot, so every total is a lower bound.
- **Omega's own per-call context cost is unmeasured.** If Omega Agent runs at a
  comparable resident context, the same ~64x amplification applies to it, and
  none of the mechanisms proposed here changes that. They change call count,
  fanout accounting, and stop behavior — not the price of a call. Reducing
  resident context is a separate problem and this document does not solve it.
- **No claim that the native loop is cheaper per unit of work than Codex.** The
  argument is about who can enforce a budget and a stop rule, not about model
  economics.
- **SCV has no production usage data.** Its roadmap is ordered by the audit's
  measured shares of a *Codex* session, which is a defensible proxy for where
  model calls go and is not the same thing as SCV telemetry.
- **The five capabilities in C.3 are proposals.** SCV today has exactly one
  tool. Nothing in section C describes shipped behavior beyond C.1.
- **Sections B.5 through B.8, and B.11's extension, do not exist in any tree.**
  They are named as proposals throughout and should not be cited as
  capabilities.

---

## Cross-references

- After-action audit:
  [`docs/afteraction/2026-07-31-codex-019fb495-overnight-spend-audit.md`](../afteraction/2026-07-31-codex-019fb495-overnight-spend-audit.md)
  — §2 measurements, §3 findings, §4 recommendations, §7.3 installed process
  rules, §7.5 refactor backlog.
- SCV design and roadmap: [`docs/scv/README.md`](./README.md).
- Engine contract priorities and autonomy trap:
  [`docs/teardowns/2026-07-17-full-catalog-synthesis-what-openagents-should-incorporate.md`](../teardowns/2026-07-17-full-catalog-synthesis-what-openagents-should-incorporate.md)
  §2 (Work Unit and Receipt above Thread/Turn/Item; durable admission before
  execution; backpressure as protocol contract) and §7.4 (*"persisted metadata
  is not a durable continuation lease"*, and autonomy as three separate
  concerns).
- Fleet orchestration context:
  [`.agents/skills/khala-fleet/SKILL.md`](../../.agents/skills/khala-fleet/SKILL.md)
  and the Khala → Pylon → Codex delegation runbook in `AGENTS.md` — where "done"
  already means a closeout checklist plus exact `token_usage_events` rows, and
  counter movement alone is never completion evidence. The same discipline,
  applied to a local runtime, is section B.4.
- Omega deltas cited: `OMEGA-DELTA-0150` (a conversation keeps its executor),
  `OMEGA-DELTA-0179` (one exact ready executor, persistence before dispatch),
  `OMEGA-DELTA-0187` (drawn implies working), `OMEGA-DELTA-0202` (one routed
  decision behind every label), `OMEGA-DELTA-0203` (offered implies startable),
  `OMEGA-DELTA-0209` (offered implies sends-a-shape-it-accepts). Registry:
  `OMEGA_DELTAS.md` and `crates/omega_deltas` in the omega repository.
- Open objective the audit hands forward: `OpenAgentsInc/openagents#9282`,
  comment `5143388289` — the community-room `404 community_room_not_active`
  hypothesis and the protocol for the next agent. That work is not this
  document's subject and must not be pulled into it.
