# Project Loupe — reference study

Status: reference study of an external project. Not an implementation plan and
not authority for any OpenAgents surface.

Upstream: <https://github.com/project-loupe/loupe>
Local reference clone: `~/work/projects/repos/loupe` (manifest entry
`project-loupe/loupe`).
Read at commit `c94aac5` (2026-07-31).

Loupe is the project named on-camera in `docs/transcripts/263.md` ("Bitcoin
Wallets Under Attack"), where the critique is that it is opt-in and
bring-your-own-token, so *"the projects most nervous about security, the ones
that need it most, would be the least likely to opt in."* That episode commits
OpenAgents to a "bad cop" white-hat agentic fuzzing operation as the
complement. This document is the technical read of what Loupe actually is, so
that any such work starts from the code rather than from the tweet thread.

Treat the clone as read-only reference material per the workspace `projects/`
convention. Do not vendor its code.

---

## 1. What it is

A **security-scanning harness for source repositories.** It runs LLM agents
over a codebase, has each agent *self-validate* its findings by writing a
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

| Binary | Role |
| --- | --- |
| `loupe-server` | Long-running daemon. Owns the SQLite DB (repos, jobs, findings, secrets), the scheduler, lease handout, verdict rollup, and reporter dispatch. |
| `loupe-worker` | Stateless fleet. Authenticates with a client cert minted at registration, leases a job, clones the repo into a local cache, runs scanners, submits findings. Doubles as the MCP server via `mcp-serve`. |
| `loupectl` | Operator CLI on an admin client cert. Register repos, mint worker certs, trigger scans, triage findings. |

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
  each submission. A duplicate suppresses *that one candidate* and the agent
  continues — so a re-scan still surfaces bugs ranked below an
  already-reported one. Catches paraphrases, refactor-shifted bugs, renames.
- **Hash (server-side, free).** `blake3(scanner_id | file | normalized_window)`
  with `UNIQUE(repo_id, fingerprint)` and `INSERT OR IGNORE`. Normalization
  lowercases and collapses whitespace, so it survives `cargo fmt`-style edits.

The README calls the hash layer *"the deterministic floor under the agent's
semantic decisions."* That phrasing is the design in one line: the model's
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
— *a prose instruction is a request; a typed contract is a fact* — implemented
at the emission boundary rather than the instruction boundary.

### 4.2 The PoC must fail on HEAD

The agent must write "a unified diff adding a regression test that **FAILS on
HEAD** and would pass once the bug is fixed," then call `validate_poc`, which
runs `git apply --check` against the worktree and returns `{applies, error?}`.

The review surface renders that diff as the primary evidence: *"applying the
diff against a fresh worktree and running the test should fail on HEAD."*

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
skip the patch entirely, and states plainly: *"A wrong patch attached to a real
bug is worse than no patch at all."*

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

Uncertainty must be submitted *and flagged*, not silently resolved. This is the
exact inverse of the failure mode recorded in
`docs/omega/2026-07-31-omega-alpha-session-handoff.md` §3.2 and §4.3, where
receipts asserted properties that were never measured and a hard-coded `true`
was indistinguishable from an observation.

### 4.5 Conservatism about severity

> "Keep claims conservative and non-sensationalist. … very few bugs are true
> vulnerabilities, and only a small fraction of those should be considered high
> or critical severity."

Plus a quality gate: no findings for hardening notes, style issues, or *"bugs
you can't write a regression test for."* Inability to produce falsifiable
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
*who runs it against whom*, not a missing capability — a third party can
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
   What Episode 263 identifies as missing is *coordination* — who scans what,
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
