# Free on-device inference (Apple FM) across the OpenAgents IDE — possibilities and boundaries

Date: 2026-07-20
Status: analysis / speculation. Not dispatch authority. Not a promise. It reads
the shipped Apple FM bridge, the build-series thesis (episodes 194 and 201), the
Cursor teardown, and the `docs/ide/` design corpus, and asks one question: what
does **free, private, always-available on-device inference** unlock for the
OpenAgents Desktop IDE, and where must it stop.

Companion: [Apple FM analyzer in the BOOT SEQUENCE](./2026-07-20-apple-fm-analyzer-boot-sequence-audit.md).

---

## 1. The one-line thesis

The value of Apple FM is not that it is smart. It is that it is **free, local, and
always there**. A cloud model is metered, remote, and rate-limited; you spend it
carefully. A free on-device model can run on every keystroke, every save, every
test run, every debugger stop, speculatively and redundantly, with nothing
leaving the machine. That changes which tasks are worth doing at all.

The whole opportunity, and the whole risk, follow from that: free inference lets
the IDE do many small helpful things it could never afford to do with a paid
model — **as long as none of them is ever trusted as authority.**

---

## 2. What the model actually is (honest limits)

From `docs/apple-fm/2026-07-19-apple-fm-swift-bridge-full-audit-and-openagents-desktop-plan.md`:

- **Small model.** Apple's on-device `SystemLanguageModel.default` (~3B class).
  It is a capable summarizer, rewriter, classifier, and short-form reasoner. It
  is not a frontier coder.
- **Small context.** A single bounded prompt (the shipped desktop IPC caps it at
  4000 chars). It cannot ingest a repository, a large file, or a long transcript.
- **Plain text today.** The shipped desktop bridge (v0.1.1) does plain-text
  completion only, one `LanguageModelSession` per turn, usage `estimated` from
  character counts. The mature Swift bridge in git history additionally has
  `@Generable` **structured generation**, **tool callbacks**, and real SSE
  **streaming** — a proven growth path, not yet in the shipped subset.
- **Apple Silicon + macOS 26 only**, with Apple Intelligence enabled. Everywhere
  else the bridge reports `not_supported`.

The honest role that falls out of this: **cheap, bounded micro-tasks over small
slices, plus routing to stronger agents** — never whole-repo reasoning. Every
proposal below respects that ceiling.

---

## 3. The hard rule: advisory only

The `docs/ide/` corpus is unusually well-suited to a free local model, because
it already isolates a **non-authority seat** and forbids any model — local or
cloud — from minting a fact. The Zed architecture reserves exactly this seat for
"optional local inference": the native helper "holds no provider credentials,
project or conversation database, policy, approval, command, or receipt
authority… The helper reports native facts; Effect decides, stores, projects,
and signs them."

Every surface enforces the same fence. Quote these as the boundary around any FM
use:

- **Host-only evidence (IDE-08):** "A process saying 'tests passed' or 'pushed'
  cannot create any of these facts." The evaluator is "a non-overridable
  deterministic repository oracle."
- **No fuzzy apply (IDE-08):** proposals require "SHA-256 equality between
  claimed and actual create/edit bytes"; "No hunk is spliced against stale line
  numbers"; "There is no fuzzy apply."
- **Exact-preimage Git (IDE-12):** "It refuses a stale preimage. A process exit
  code does not prove a postcondition." "The delivery record never converts an
  agent statement into review, acceptance, or release proof."
- **Semantic success ≠ opinion (IDE-10):** "A zero exit code is not sufficient.
  The controller must observe an assertion summary." "IDE-10 does not fabricate
  coverage."
- **No guessed debug positions (IDE-11):** "A row fails when it guesses a
  position."
- **Generation fences everywhere:** every range-bearing command carries a
  document generation; a stale generation "must produce a typed stale result…
  Silent best-effort line-number application is not admitted."
- **Redaction before any consumer (IDE-10/11):** output and variables are
  redacted and secrets kept as references before they leave the host; public
  receipts forbid prompts, vectors, query text, paths, and content.

So a free local model lives entirely in the **private advisory plane**: it
summarizes, explains, ranks, drafts, predicts. It never sets acceptance, never
mints evidence, never decides delivery state, never asserts a debug position,
and its text never enters a signed receipt. This is the frame for everything
that follows.

---

## 4. Why "free" is the actual unlock

Cost, not capability, is what changes. Free, local inference makes a class of
tasks economical that a paid model never justifies:

- **High-frequency tasks.** Completions and next-edit fire constantly. A metered
  model makes you ration them; a free model can run per-keystroke.
- **Latency-sensitive tasks.** No network round-trip. The model is a loopback
  call away, already resident.
- **Privacy-sensitive tasks.** Nothing leaves the machine. This is the exact
  inverse of Cursor, which "uploads changed plaintext files for server-side
  chunking/embedding" and keeps a remote index that local deletion cannot erase.
  The IDE context manifest already tags each item `providerEligible` /
  `sensitivity` / `audience`; a local model can operate on the **local-only,
  not-provider-eligible** items that a cloud model structurally must never see.
- **Speculative / redundant work.** With no token budget, the IDE can run the
  model eagerly and throw the result away if unused — pre-summaries, pre-ranked
  candidates, warm explanations — because the marginal cost is zero.

Free inference is therefore not "a weaker Codex." It is a **new ambient layer**
under the paid agents: constant, private, cheap, and disposable.

---

## 5. Where it helps, surface by surface (all advisory)

### 5.1 Completions and next-edit (IDE-09) — the highest-value lane

Inline completion and next-edit are high-frequency, latency-sensitive, and
privacy-sensitive — precisely the profile free local inference was made for. The
Zed analysis already designs the seat: a typed `ContextCandidate` portfolio with
`scoreKind ∈ semantic | lexical | recency | structural | explicit`, where "the
central typed semantic selector or structured query planner chooses among
candidates." A free local model can both **generate** the inline prediction and
**score/select** candidates in that portfolio, entirely on-device, with no
embeddings and no upload — satisfying the corpus rule "Do not require repository
embeddings for code understanding." This is where free inference most changes the
cost equation, and it is where cloud completion products spend the most money.

### 5.2 Code graph and context (IDE-08)

- **Summarize and rank the eleven-source context** before it is disclosed. The
  `AgentContextTray` already carries per-item reason, sensitivity, and token
  estimate; a one-line "what this context is and why it is here" is a natural
  private adjunct.
- **Fill slot 10.** Semantic retrieval is a live, reserved-but-disabled seat
  ("optional semantic retrieval, currently omitted as `retrieval_disabled`"). A
  local model can rank retrieval candidates without generating, storing, or
  uploading embeddings — the exact custody problem IDE-08 flags ("No embeddings
  are generated, stored, or uploaded by IDE-08").
- **Pre-explain a proposed diff.** When a turn submits a hash-checked proposal
  into the Pierre Changes plane, a local model can draft the plain-language "what
  this change does" shown in the inspector — advisory text beside the
  deterministic, SHA-verified apply/rebase/undo authority, never part of it.

### 5.3 Run graph — terminal and tests (IDE-10)

Bounded, host-redacted Output channels and typed pass/fail/gap facts are ideal
cheap inputs. A local model can **triage failing test output**, **summarize a
long or truncated log** (the graph already marks dropped bytes and gaps), and
**explain a task failure** by correlating the semantic-outcome fact with the
retained redacted tail. Because output is redacted before it leaves the host,
the local summarizer sees only safe text — and its summary stays advisory: the
host still computes semantic success from artifacts and assertions, not from
prose.

### 5.4 Debug (IDE-11)

Explaining a stopped state, narrating a stack, or summarizing a bounded variable
snapshot are natural local tasks over the already-decoded, redacted, depth- and
count-bounded projections. The renderer already receives "a decoded view" with
"Variable depth and count… bounded" and no credentials. The hard limit: the
model may explain a position but "A row fails when it guesses a position" — it
never sources debug state, only reads it.

### 5.5 Source control (IDE-12)

**Draft commit messages, summarize a diff, summarize a review or PR** from the
clean inputs the graph provides (exact status snapshots, hunk/line diffs, decoded
provider review facts). The determinism fence is strict and correct: the commit
tree, the observed remote-OID postcondition for `pushed`, the reviewed-version
fact, and owner acceptance are all deterministic or independent. The model text
is a **draft**; "The delivery record never converts an agent statement into
review, acceptance, or release proof."

### 5.6 Search and navigation

The corpus wants indexing "unbundled and explicit" (`off`, bounded local
lexical, local semantic, disclosed remote semantic — each stating what it reads,
persists, uploads, retains, deletes). A free local model is the natural engine
for the **local semantic** tier and for the tiered fallback the Zed analysis
leaves open: "local parsing may provide immediate outline/symbol context while a
remote LSP is starting or unavailable; the UI states the evidence tier." Free
inference makes the always-available local tier real without an embedding
service and without upload.

---

## 6. The deepest pattern: free triage that routes to the paid agents

The most valuable use is not any single summary. It is putting a **free
on-device coordinator in front of the metered agents**. Before a request goes to
Codex, Claude Code, or Grok, a free local turn can:

- decide whether the task is small enough to answer locally (a rename, a
  one-line explanation, a commit message) or must be delegated,
- gather and rank the bounded context to send, so the paid model gets a tighter,
  cheaper prompt,
- filter local-only / sensitive context out of what reaches a cloud provider,
- give the user an immediate first answer while the paid agent is still
  verifying its account or streaming its first token.

This is not new speculation — it is the pattern OpenAgents already built and
proved. The November 2025 `FMOrchestrator.swift` / `FMTools.swift` had Apple FM
explore a workspace with `code.grep`, `fs.list_dir`, `session.read`, and
`content.get_span`, and a commit literally titled "Enable concurrent delegations
from Foundation Models to Codex/Claude Code" made FM an on-device router fanning
work to stronger cloud agents (later `subagent_router.rs`). The surviving bridge
supports the tool-callback and structured-generation primitives to rebuild it.

It also maps to the strategy the audit records verbatim: Apple FM is "a bounded
local coordinator and delegator around stronger coding agents," and the goal is
to grow the on-device share of the workload over time (episode 194's "start at
~5% cloud offload, drive it down" chronicle). Free triage in front of paid
inference is how that share grows without hurting quality: the cheap model
handles what it can, routes what it cannot, and every task it absorbs is a task
that did not cost a cloud call or leave the machine.

---

## 7. Ambient understanding: the BOOT SEQUENCE analyzer is the same engine

The companion BOOT SEQUENCE audit proposes a free on-device analyzer that, at
open, reads the environment and tells the user where they are. That is the same
engine as everything above, running at a different moment. Together they form an
**always-on ambient layer**: orient the user at boot, then keep helping — rank
context, draft messages, triage failures, explain stops, predict edits —
throughout the session, all free, all local, all advisory. The IDE surfaces
provide the clean, redacted, generation-fenced inputs; the free model provides
the cheap human-readable layer on top; the deterministic host keeps every fact.

---

## 8. What free inference does NOT unlock (and the risks)

- **It is not a repo brain.** Small context means it reasons over slices, not the
  whole codebase. Whole-repo understanding still needs the deterministic graph
  (LSP, Git co-change, structural candidates) and, where the owner admits it, a
  disclosed remote index — not a 3B model pretending to hold the repo.
- **Quality ceiling.** A 3B model's output must stay advisory and, where it makes
  a factual claim, confirmable against the deterministic source. Prefer the
  structured fingerprint for facts and the model for phrasing.
- **Non-determinism is permanent.** Its prose can never become authority — the
  corpus fences make this mechanical, not a matter of trust.
- **Coverage.** Apple Silicon + macOS 26 only. On every other machine it must be
  a pure enhancement that silently does nothing, with an Ollama/other-runtime
  fallback a later, explicitly-placed decision (episode 201's "add Ollama and
  other hardware" direction).
- **Latency is nonzero.** Free is not instant. Keep it off blocking paths (the
  2026-07-20 startup refactor rule): never gate first paint, never gate the
  composer, never gate an apply or a push.
- **Hallucinated context.** A wrong local summary shown as fact is worse than no
  summary. Label it estimated; make destructive-adjacent uses (commit messages,
  diffs) reviewable before they are committed.

---

## 9. Sequencing (if the owner admits this)

Reuse the shipped bridge; add no new authority. Ship the lowest-risk,
highest-value advisory lanes first, each behind the reserved "optional local
inference" seat with explicit model/data/budget policy:

1. **Commit-message draft + test-failure triage + context summary** — small,
   bounded, obviously useful, and each already has clean redacted inputs.
2. Measure quality and latency on a real Apple Silicon device.
3. **Completions / next-edit (IDE-09)** and the **free triage/router** — the two
   highest-value lanes — once the advisory pattern and its trust boundary are
   proven in the smaller lanes.

Every step stays inside the "reports facts, Effect signs them" seat, local-only
by default, advisory by construction, and disposable at zero marginal cost.

---

## 10. References

- Capability + history: `docs/apple-fm/2026-07-19-apple-fm-swift-bridge-full-audit-and-openagents-desktop-plan.md`.
- Boundary contract: `apps/openagents-desktop/src/apple-fm-contract.ts`,
  `apps/openagents-desktop/src/apple-fm-host.ts`.
- IDE surfaces: `docs/ide/2026-07-19-ide-08-agent-native-code-graph.md`,
  `docs/ide/2026-07-19-ide-10-effect-run-graph.md`,
  `docs/ide/2026-07-20-ide-11-effect-dap-graph.md`,
  `docs/ide/2026-07-20-ide-12-effect-source-control.md`.
- Architecture + the reserved local-inference seat:
  `docs/ide/2026-07-18-zed-agent-ide-adaptation-analysis.md`,
  `docs/ide/2026-07-18-zed-quality-ide-effect-rust-architecture.md`,
  `docs/ide/ROADMAP.md`.
- Incumbent comparison (remote-embedding upload vs. the open local lane):
  `docs/teardowns/2026-07-11-cursor-product-teardown.md`.
- Strategic framing: `docs/transcripts/194.md`, `docs/transcripts/201.md`.
</content>
