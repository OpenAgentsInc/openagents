# Project Loupe — reference study

Status: reference study of an external project. Not an implementation plan and
not authority for any OpenAgents surface.

Upstream: <https://github.com/project-loupe/loupe>
Local reference clone: `~/work/projects/repos/loupe` (manifest entry
`project-loupe/loupe`).
Read at commit `c94aac5` (2026-07-31).

Loupe is the project named on-camera in `docs/transcripts/263.md` ("Bitcoin
Wallets Under Attack"), where the critique is that it is opt-in and
bring-your-own-token, so _"the projects most nervous about security, the ones
that need it most, would be the least likely to opt in."_ That episode commits
OpenAgents to a "bad cop" white-hat agentic fuzzing operation as the
complement. This document is the technical read of what Loupe actually is, so
that any such work starts from the code rather than from the tweet thread.

Treat the clone as read-only reference material per the workspace `projects/`
convention. Do not vendor its code.

## Current OpenAgents implementation

The short-term product priority is the smaller
[entropy-first Omega repository dashboard](2026-08-02-entropy-first-omega-dashboard-roadmap.md):
traverse Coldcard read-only, let the user edit the entropy prompt, show live
file-by-file analysis, preserve immutable reruns, compare prompt A with prompt
B, and then apply the same source-aware method across the 15-project catalog.
Omega issues [#199](https://github.com/OpenAgentsInc/omega/issues/199),
[#200](https://github.com/OpenAgentsInc/omega/issues/200),
[#201](https://github.com/OpenAgentsInc/omega/issues/201), and
[#202](https://github.com/OpenAgentsInc/omega/issues/202) own that list. All
four closed on 2026-08-02; the exact Omega `main` commits and evidence boundary
are recorded in the dashboard roadmap's
[delivery receipt](2026-08-02-entropy-first-omega-dashboard-roadmap.md#delivery-receipt).
The broader forensic bench follows it; it is not a prerequisite and remains
subject to its separate live-evidence gates.

The OFR-001 through OFR-018 program that followed this study has substantial
checked-in contracts, fixtures, adapters, deterministic tests, and Omega
projections. It is not accepted as a complete live program. OpenAgents issue
[#9300](https://github.com/OpenAgentsInc/openagents/issues/9300) is open after
independent review found missing end-to-end evidence; issues
[#9289](https://github.com/OpenAgentsInc/openagents/issues/9289) and
[#9290](https://github.com/OpenAgentsInc/openagents/issues/9290) retain the
live worker and source-delivery correction gates. Start with
[`Omega forensic analysis: implementation and operator guide`](2026-08-01-omega-forensics-implementation-and-operator-guide.md)
for the component inventory, authority boundary, fixture workflow,
verification commands, metrics, prompt-iteration loop, and current UI gaps.
Use the
[`Omega Coldcard forensic practice-run runbook`](../coldcard/2026-08-01-omega-coldcard-forensic-practice-runbook.md)
for the first end-to-end benchmark.

Implementation does not mean live acceptance or public availability. The native
OpenAgents Cloud forensic profile remains default-off unless separately
admitted, and every third-party run remains private and manual-reporting by
default.

For the 2026-08-02 external-observation and UI reconciliation, read the
[`Coldcard documentation index`](../coldcard/README.md), the
[`wallet-security posts and Omega-thread audit`](../coldcard/2026-08-02-wallet-security-posts-and-omega-thread-audit.md),
and the
[`model-panel and publication-gates audit`](../coldcard/2026-08-02-forensic-model-panel-and-publication-gates-audit.md).
Those audits inspect the supplied images, correct a missed table row, classify
the captured Omega failure as a request/tool-contract fault rather than a
security refusal, and define the first read-only Omega-native forensic bench.

---

## 1. What it is

A **security-scanning harness for source repositories.** It runs LLM agents
over a codebase, has each agent _self-validate_ its findings by writing a
regression-test proof-of-concept and checking that the diff applies, and
dispatches confirmed findings to a reporter — GitHub issues, email via
sendmail, or nothing (manual triage).

The README's own framing: findings "show up where the rest of the team's bugs
live."

Provenance: Rust, MIT OR Apache-2.0, `rust-version = 1.75`,
`unsafe_code = "forbid"` at the workspace root. Authored overwhelmingly by
Elias Rohrer (`tnull`) — an LDK maintainer — which is consistent with the
`Cargo.toml` `repository` field still pointing at `github.com/tnull/loupe`, and
with the Spiral/Block association described in the episode. Roughly a hundred
commits, actively developed as of read date.

---

## 2. Architecture

Three deployable binaries that talk to each other over **mTLS**:

| Binary         | Role                                                                                                                                                                                                    |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `loupe-server` | Long-running daemon. Owns the SQLite DB (repos, jobs, findings, secrets), the scheduler, lease handout, verdict rollup, and reporter dispatch.                                                          |
| `loupe-worker` | Stateless fleet. Authenticates with a client cert minted at registration, leases a job, clones the repo into a local cache, runs scanners, submits findings. Doubles as the MCP server via `mcp-serve`. |
| `loupectl`     | Operator CLI on an admin client cert. Register repos, mint worker certs, trigger scans, triage findings.                                                                                                |

Crates: `loupe-core` (shared types), `loupe-proto` (versioned wire DTOs,
`X-Loupe-Protocol`), `loupe-tls` (internal CA + cert minting), `loupe-storage`
(SQLCipher DAO, FTS5 index, migrations), plus the three binaries' crates.

**Security posture worth noting:**

- The entire database is sealed with **AES-256 + HMAC-SHA512** under a
  mandatory master key — secrets, finding descriptions, PoCs, repo metadata,
  audit trails. "An attacker reading `loupe.sqlite` off disk gets ciphertext
  for every row." The server refuses to start without a key; there is **no
  plaintext-mode fallback**.
- The admin key leaves the machine exactly once, at `init`. The server keeps no
  copy of worker keys either.
- Agents run inside **bubblewrap** (`bwrap`): `--die-with-parent`,
  `--unshare-pid --unshare-ipc --unshare-uts`, optional `--unshare-net`, the
  worktree bind-mounted **read-only** at `/workdir`, and fresh `tmpfs` for
  `/tmp` and `/home/scanner` per invocation.
- The worker **hard-fatals at startup** if the LLM scanner is on and `bwrap` is
  missing. `LOUPE_DISABLE_SANDBOX=1` is the explicit dev override.
- Dependencies all set `default-features = false` with features enumerated,
  explicitly to keep `aws-lc-rs` out of the rustls subtree.

---

## 3. The pipeline

Discovery fans out **one agent session per source file**. Within a session the
agent reads the file, enumerates every exploitable bug severity-ordered, and
for each candidate: searches prior findings, writes a PoC diff, validates it,
and submits.

Finding lifecycle:

```
pending ─┬─ (verify off) ─→ confirmed ──┐
         └─ (verify on)  ─→ validating ─┤
                             │          │
                        confirm/dismiss/│
                        inconclusive    │
                             │          │
                             ▼          ▼
                    (require_approval gate)
                       off ─→ dispatch ─→ reported
                       on  ─→ awaiting_approval ─→ approved/rejected
```

Verdict rollup is applied **in-transaction**: any `dismissed` → finding
dismissed; else any `confirmed` → confirmed and dispatch; else stay
`validating` until a reaper deadline.

Two complementary dedup layers, which the README explicitly ranks:

- **Semantic (agent-driven).** The agent calls `query_prior_findings` before
  each submission. A duplicate suppresses _that one candidate_ and the agent
  continues — so a re-scan still surfaces bugs ranked below an
  already-reported one. Catches paraphrases, refactor-shifted bugs, renames.
- **Hash (server-side, free).** `blake3(scanner_id | file | normalized_window)`
  with `UNIQUE(repo_id, fingerprint)` and `INSERT OR IGNORE`. Normalization
  lowercases and collapses whitespace, so it survives `cargo fmt`-style edits.

The README calls the hash layer _"the deterministic floor under the agent's
semantic decisions."_ That phrasing is the design in one line: the model's
judgment is allowed to be fallible because a mechanical check sits underneath
it.

---

## 4. The prompts — the most transferable part

`crates/loupe-worker/src/llm/prompts.rs`, 463 lines. Several decisions here are
sharper than anything we currently encode.

### 4.1 Emission is a tool call, never prose

> "The worker doesn't parse findings out of the model's text response; emission
> only happens via `submit_finding`." … "Your text response is logged but not
> parsed."

A finding that is not submitted through the typed tool does not exist. This is
the same thesis as `docs/scv/2026-07-31-omega-agent-scv-encoded-practice-analysis.md`
— _a prose instruction is a request; a typed contract is a fact_ — implemented
at the emission boundary rather than the instruction boundary.

### 4.2 The PoC must fail on HEAD

The agent must write "a unified diff adding a regression test that **FAILS on
HEAD** and would pass once the bug is fixed," then call `validate_poc`, which
runs `git apply --check` against the worktree and returns `{applies, error?}`.

The review surface renders that diff as the primary evidence: _"applying the
diff against a fresh worktree and running the test should fail on HEAD."_

This is the "prove the check can fail" discipline — a claim is only evidence if
the artifact backing it demonstrably fails on the unfixed state — promoted from
a working habit into a **product requirement enforced mechanically**.

### 4.3 The verdict is locked before the fix is written

In the verify prompt, `submit_verdict` is phase 1, mandatory, first, and
single-call. The stated reason:

> "Calling this tool LOCKS your verdict for the session — a second call returns
> an error. This ordering is deliberate: it prevents you from rationalising the
> verdict to match a fix you've already started writing."

`submit_patch` is phase 2, optional, and only reachable after a `confirmed`
verdict. The prompt enumerates four conditions under which the agent should
skip the patch entirely, and states plainly: _"A wrong patch attached to a real
bug is worse than no patch at all."_

Ordering a commitment before the work that would bias it is a control we do not
currently have anywhere.

### 4.4 Anti-hallucination scope rules

The discovery prompt bounds what the agent may claim to know:

> "Do not claim to have 'verified against' or 'checked' any out-of-tree source
> you cannot actually open through this worktree." … "If you find yourself
> writing 'I verified against …' about code you have no path to read, stop and
> re-frame."

And the resulting bias is stated explicitly:

> "a false positive a human can dismiss is better than a false negative dressed
> as a confident cross-reference check."

Uncertainty must be submitted _and flagged_, not silently resolved. This is the
exact inverse of the failure mode recorded in
`docs/omega/2026-07-31-omega-alpha-session-handoff.md` §3.2 and §4.3, where
receipts asserted properties that were never measured and a hard-coded `true`
was indistinguishable from an observation.

### 4.5 Conservatism about severity

> "Keep claims conservative and non-sensationalist. … very few bugs are true
> vulnerabilities, and only a small fraction of those should be considered high
> or critical severity."

Plus a quality gate: no findings for hardening notes, style issues, or _"bugs
you can't write a regression test for."_ Inability to produce falsifiable
evidence is itself the filter.

### 4.6 Cross-family second opinion

The default verifier prefers **Codex** while the default scanner is **Claude**,
"so the second opinion comes from a different model family than the default
scanner." Selection is configurable per worker (`[agents].scan` /
`[agents].verify`, `auto | claude | codex`), and role separation is achieved by
capability advertisement rather than by binary — one worker can serve both.

---

## 5. Direct relevance to OpenAgents

**It drives the same harnesses Omega commands.** Loupe shells out to the
`claude` and `codex` CLIs; Codex specifically via
`codex exec --dangerously-bypass-approvals-and-sandbox --skip-git-repo-check`,
contained by bubblewrap rather than by the CLI's own sandbox. Episode 263's
promise — "command all of the top coding agents simultaneously" — and Loupe's
worker fleet are the same primitive pointed at different products. Anything we
build for fleet dispatch, account rotation, or quota handling is applicable
here, and vice versa.

**Its MCP tool surface is mode-split.** Discovery exposes
`query_prior_findings`, `get_finding_by_id`, `submit_finding`, `validate_poc`;
verify swaps the last two for `submit_verdict`, `submit_patch`,
`validate_patch`. Canonical list is `crates/loupe-worker/src/mcp.rs`
`tool_definitions()`. Compare our own harness MCP pilot (FEED-1 #8783), which is
read-only by design — Loupe's is deliberately mutating, but every mutation is a
typed tool with a server-side validator behind it.

**The "bad cop" mode already exists in the code.** `--no-reporting` registers a
repo with no tracker: the full scan → verify → approval pipeline runs and
findings park in `confirmed` for out-of-band triage via `loupectl finding`. That
is precisely the shape Episode 263 describes for scanning a project you do not
own and disclosing responsibly. The opt-in critique in the episode is about
_who runs it against whom_, not a missing capability — a third party can
already point Loupe at any public clone URL and triage privately.

**Constraint that matters for our fleet: macOS cannot run LLM scanning.**
bubblewrap has no macOS port, so "LLM scanning runs on Linux workers only." Our
Pylon/Codex capacity is macOS-resident. Any OpenAgents-side fuzzing operation
either needs Linux workers, a different isolation primitive, or an explicit
decision to run unsandboxed (which the project treats as a dev-only override).

**Cost shape is worth reading before proposing anything.** The README warns
that discovery "launches one configured agent session per discovered source
file, so large repositories may trigger hundreds or thousands of LLM CLI
invocations." Given tonight's spend-ceiling work, note that our own accounting
would need to attribute this correctly — and that per-file fan-out is exactly
the pattern where reasoning-token attribution (see the handoff doc §2.1) stops
being a rounding error.

**Bitcoin-specific context injection.** Optional `bkb-mcp` on PATH adds a
second MCP server exposing `bkb_search`, `bkb_lookup_bip` / `_bolt` / `_lud` /
`_nut` / `_blip`, `bkb_find_commit`, `bkb_get_document`, `bkb_get_references`,
`bkb_timeline`, pointed at `https://bitcoinknowledge.dev` by default. Absence is
silent — the prompt does not mention bkb at all when the binary is missing. A
clean pattern for conditional capability: the instruction text itself changes
with the available tools, rather than advertising tools that may not exist.

---

## 6. Limits and open questions

- **The approval gate's audit trail is coarse.** `approved_by_cn` /
  `rejected_by_cn` record the admin cert's `workers.name`. A verifier `dismiss`
  and a human `reject` both land on `state = 'dismissed'`; only the human path
  stamps `rejected_*`. Distinguishing them after the fact depends on that stamp
  rather than on the state.
- **Email reporting is not reachable from `loupectl`.** `ReportingSetup::Email`
  exists on the wire, but registering an email-backed repo means calling
  `POST /v1/repos` directly or building a client on `loupe-proto`.
- **Clone URL and reporting destination are deliberately not patchable** —
  re-pointing where findings get filed is called out as "too easy a footgun."
  Re-registration is required. Worth copying as a principle.
- **`inconclusive` verdicts do not resolve themselves.** A finding stays
  `validating` until a reaper deadline dismisses it. Whether that default is
  right depends on whether an unresolved uncertain finding should decay to
  dismissed — the same question our own `accounting_uncertain` state raises in
  a different domain (handoff doc §1.2), and Loupe answers it the opposite way.
- **We have not run it.** Everything above is a read of the source and docs at
  `c94aac5`. No claim here is backed by an executed scan.

---

## 7. If we act on Episode 263

Nothing in this document authorizes work. But if the agentic-fuzzing operation
proceeds, the honest starting points are:

1. **Run Loupe first, before building anything.** The `--no-reporting` mode on
   a Linux worker against one public target answers most design questions
   empirically and costs a day. Any "we should build X" claim made without that
   is speculation.
2. **The differentiator is not the scanner.** Loupe already has discovery,
   cross-family verification, PoC validation, dedup, and a disclosure path.
   What Episode 263 identifies as missing is _coordination_ — who scans what,
   who discloses, and how defenders share results. That maps to Omega's NIP-29
   chat and the Armada interop mentioned in the episode, not to a second
   scanner.
3. **Contribute upstream where it overlaps.** Fleet dispatch, account rotation,
   and quota-aware retry are problems we have already solved for Pylon/Codex
   and Loupe currently handles thinly.
4. **Respect disclosure norms.** The episode is explicit about responsible
   disclosure. Any finding produced against a third-party project is subject to
   that, and to `AUTHORITY.md` — publishing a vulnerability claim is a public
   claim, and unsupported public claims are reserved.

---

## 8. Related documents in this folder

Read in this order:

0. [`loupe-in-plain-words.md`](loupe-in-plain-words.md) — **start here.** The
   algorithm step by step and the exact instructions given to the AI, in plain
   language, in about five minutes. No jargon.
1. **This file** — what Loupe is, read from source at `c94aac5`.
2. [`2026-07-31-omega-first-scan-preliminary.md`](2026-07-31-omega-first-scan-preliminary.md)
   — the first run against our own code. The empirical ground under everything
   else here.
3. [`2026-07-31-omega-first-class-pentester-speculation.md`](2026-07-31-omega-first-class-pentester-speculation.md)
   — the technical architecture: where Loupe stops, the eight layers L0–L7, the
   evidence ladder T0–T5, and what a campaign looks like end to end.
4. [`2026-07-31-fix-as-a-service-company-thesis.md`](2026-07-31-fix-as-a-service-company-thesis.md)
   — the commercial thesis: proof-carrying **fixes** rather than findings,
   remediation as the scarce resource, ecosystem-scale variant multiplication,
   attested absence as a product, the corpus-and-trust moat, and the failure
   modes.

5. [`2026-08-01-would-loupe-have-caught-coldcard.md`](2026-08-01-would-loupe-have-caught-coldcard.md)
   — a written-down prediction, made before running Loupe against the Coldcard
   firmware, that it would **not** have caught the entropy bug, with the four
   structural reasons and the experiment that would settle it.

6. [`2026-08-01-coldcard-prefix-experiment.md`](2026-08-01-coldcard-prefix-experiment.md)
   and
   [`2026-08-01-coldcard-prefix-experiment-results.md`](2026-08-01-coldcard-prefix-experiment-results.md)
   — the pre-registered two-arm experiment and its result: default Loupe missed
   the bug with empty submodules, while the dependency-complete arm found the
   full causal chain three times.
7. [`2026-08-01-codex-analysis.md`](2026-08-01-codex-analysis.md) and
   [`2026-08-01-hardening-against-ai-assisted-attacks.md`](2026-08-01-hardening-against-ai-assisted-attacks.md)
   — independent defensive analyses of artifact provenance, security
   invariants, executable evidence, related vulnerability classes, and phased
   implementation.
8. [`2026-08-01-coordination-not-scanners.md`](2026-08-01-coordination-not-scanners.md)
   — why scan coverage, shared profiles, result divergence, campaign rooms, and
   cost pooling are ecosystem coordination problems rather than scanner
   features alone.
9. [`2026-08-01-omega-forensic-analysis-roadmap.md`](2026-08-01-omega-forensic-analysis-roadmap.md)
   — the owner-directed delivery sequence: begin with configurable Loupe-style
   runs and prompt iteration inside Omega on disposable OpenAgents Cloud GCE
   workers, measure qualified identification time, tokens, cost, recall, and
   evidence quality from native events, govern optimized candidates through
   DSPy/Blueprint boundaries, independently reproduce the pinned Coldcard
   postmortem from generator behavior through historical-chain fingerprints
   and provenance graphs, then advance to executable evidence, variant analysis,
   and coordinated remediation.

Documents 3 and 4 are speculation and authorize nothing. Documents 1 and 2 are
factual records — of source read, and of a scan run. Document 5 is a prediction
recorded ahead of the experiment so it can be scored honestly afterwards.
Document 6 records the experiment and its refutation. Documents 7 and 8 are
analysis. Document 9 is a roadmap and retains the authority boundaries stated
in that file.
