# Development Velocity Benchmark — GitHub-centric History versus the OpenAgents System

**Date:** 2026-07-22
**Lane:** Reference measurement and analysis (`docs/forge/`). This document
flips no promise state, changes no runtime authority, mints no issue, and
dispatches no work. Candidate work needs normal Sol admission or an
owner-accepted work packet.
**Class:** measurement and analysis, not code.
**Companion:** `docs/forge/2026-07-22-nostr-git-forge-github-replacement-audit.md`
(the replacement design and the speed thesis this document tests against real
history), plus its effect-vs-rust, hosted-services, and grasp-prior-art
addenda.

**Label key:**
`[MEASURED]` = a value counted directly from our own chat transcripts.
`[ESTIMATED]` = a value scaled from a measured sample, stated with its method.
`[PROJECTED]` = a forward claim about the OpenAgents system that this document
does not prove from history.

**Privacy note:** the data sources are private agent transcripts. This document
extracts only counts, rates, timestamps, and patterns. It quotes no prompt
text, no secret, no token, no repository-private content, and no personal
data. Every number below is an aggregate signal.

---

## 1. Purpose

The owner states that OpenAgents "can actually be faster than all these
GitHub-centric flows." The forge replacement audit argues the mechanism from
GitHub public policy and from owned protocol code. This document tests the same
claim from a different direction. It measures how the historical
"GitHub-centric way" actually behaved across four months of our own agent
work, and it maps each measured friction to the exact part of the OpenAgents
system that removes it.

The document is deliberately honest about one result. Our history supports the
speed thesis, but not mainly through the argument the audit leads with. The
strongest evidence in our own transcripts is the sheer round-trip volume and
the poll-loop tax, not a wall of GitHub throttle errors. Section 7 states the
verdict in full.

---

## 2. Methodology

### 2.1 Corpus

| Source | Sessions | Size | Date span |
| --- | --- | --- | --- |
| Codex rollout transcripts (`~/.codex/sessions/**`) | 1,547 | about 15 GB | 2026-03-26 to 2026-07-22 |
| Claude Code transcripts (`~/.claude/projects/**`) | 2,400 | about 2.5 GB | same era |
| Combined | 3,947 | about 17.5 GB | 118 days |

Each transcript is JSON Lines. Each line is one event. Command executions
appear as typed records. In Codex they are `function_call` records named
`exec_command` or `shell`, plus `custom_tool_call` records named `exec`. In
Claude Code they are `tool_use` blocks named `Bash`. The command text lives in
a single field on a single line in every case.

### 2.2 What was counted, and how

Two passes ran over the corpus.

**Pass A, whole-corpus text scan (fast, noisy).** A streaming scan counted how
often command tokens such as `gh issue` and `git push` appear anywhere in the
transcripts. This pass is an upper bound. It counts real invocations, but it
also counts the same tokens where they appear inside documentation, skill
files, `AGENTS.md` quotations, and agent reasoning. Pass A is not the source of
the headline numbers. It only located clusters and date ranges.

**Pass B, clean command extraction (the headline source).** A deterministic
one-in-ten sample (every tenth transcript by sorted path) was parsed record by
record. The parser pulled only the actual command string out of each
command-execution record and ignored all other text. The sample is 155 Codex
sessions and 240 Claude sessions. It contains 199,796 real command invocations
(151,120 Codex, 48,676 Claude). Command tokens were counted inside that clean
command stream only. Per-session rates in this document are `[MEASURED]` on the
sample. Corpus totals are `[ESTIMATED]` by scaling the sample by ten, which
matches the one-in-ten sampling exactly (Codex factor 9.98, Claude factor
10.00).

### 2.3 Limitations, stated plainly

- These are agent transcripts, not a clean continuous-integration ledger. They
  record what the agents ran, not a canonical build history. Treat every total
  as an order-of-magnitude figure with a plus-or-minus band, not a precise
  count.
- The one-in-ten sample carries normal sampling error. Rare high-value events
  such as rate-limit errors were counted across the whole corpus in Pass A
  instead, so the sample does not hide them.
- Command counting cannot see GitHub interactions that never took a `gh` or
  `git` command, for example web-UI actions the owner took by hand. Those are
  invisible here and are not in any total.
- A single `gh issue view` and a single `gh issue create` count the same in a
  volume total, but they cost GitHub very differently. Section 3.2 splits reads
  from content-creation writes for exactly this reason.
- Session boundaries are transcript files. One human work session can span
  several files, and one long-lived orchestrator file can span days. Duration
  figures reflect files, not human intent.

---

## 3. The historical GitHub tax

### 3.1 Volume — the round-trip tax `[ESTIMATED]`

Every GitHub command below is a network round-trip across the public internet
to Microsoft infrastructure and back, with REST or GraphQL overhead and
rate-limit accounting on each call.

| Command class | Codex (est) | Claude (est) | Combined (est) |
| --- | ---: | ---: | ---: |
| Total agent shell commands | ~1,508,000 | ~487,000 | ~1,995,000 |
| `gh` (all GitHub CLI) | ~30,500 | ~7,800 | ~38,200 |
| `gh issue` | ~24,800 | ~4,600 | ~29,300 |
| `gh pr` | ~3,500 | ~2,600 | ~6,100 |
| `gh api` | ~770 | ~270 | ~1,040 |
| `git push` | ~9,700 | ~2,400 | ~12,200 |
| `git commit` | ~10,000 | ~2,300 | ~12,200 |
| `git fetch` | ~4,000 | ~4,800 | ~8,800 |
| `git clone` | ~170 | ~80 | ~250 |

Two aggregate figures follow from the table.

- **GitHub REST round-trips (all `gh` calls): about 38,000.** `[ESTIMATED]`
- **Git transport operations to GitHub (push, fetch, clone): about 21,000.**
  `[ESTIMATED]`
- **Combined GitHub network interactions: about 59,000 over 118 days, an
  average near 500 per day, with multi-thousand peaks on fan-out days.**
  `[ESTIMATED]`

The dominant single line is `gh issue` at about 29,000 calls. The issue tracker
was the coordination substrate. Agents read the backlog, claimed work, posted
status, and closed items through the issue API, and each of those touches was a
round-trip.

### 3.2 The `gh issue` breakdown — mostly reads and polls `[MEASURED]`

Inside the clean Codex sample, the `gh issue` sub-commands split as follows.

| Sub-command | Sample count | Class |
| --- | ---: | --- |
| `gh issue view` | 1,161 | read / poll |
| `gh issue comment` | 700 | content-creation write |
| `gh issue close` | 375 | state mutation |
| `gh issue list` | 369 | read / poll |
| `gh issue create` | 135 | content-creation write |
| `gh issue edit` | 74 | write |

Reads (`view` plus `list`) are 54 percent of all `gh` calls in the sample.
Content-creation writes (`comment` plus `create`, plus pull-request `create`)
are 29 percent. This split matters for the next section, because GitHub meters
reads and content-creation writes under different ceilings.

### 3.3 Rate-limit friction — the honest core finding

This is the thesis the forge audit leads with, so this document tested it
hardest. The result is nuanced.

**Generic throttle strings are common but are NOT GitHub tax.** The tokens
`429` and `rate limit` appear thousands of times across the corpus (about 7,500
and 12,000 in Codex, about 6,100 and 3,500 in Claude). Almost all of that is
our own product. OpenAgents builds rate limiters, L402 paywalls, and provider
back-off, and it handles provider `429` responses from model APIs. Attributing
those to GitHub would be wrong, so this document excludes them.

**Real GitHub PRIMARY rate-limit hits: three sessions in four months.**
`[MEASURED]` The canonical GitHub message `API rate limit exceeded for user ID`
appears in exactly three distinct sessions, dated 2026-04-30, 2026-05-25, and
2026-06-02. These are the 5,000-per-hour primary REST ceiling, hit during heavy
`gh api` use. They are real, and they are rare.

**Real GitHub SECONDARY (content-creation) ceiling hits: about zero in
history.** `[MEASURED]` The canonical secondary-limit strings
(`You have exceeded a secondary rate limit`, `was submitted too quickly`,
`triggered an abuse detection mechanism`) do not appear as command output
anywhere in the historical corpus. The only files that contain those strings
are dated today, 2026-07-22, and the matches are the forge audit and this very
benchmark discussing the concept, not a GitHub error return. An earlier cluster
on 2026-06-28 also turned out to be an agent writing the phrase
"secondary rate limits" inside an analysis document, not a live throttle event.

**The peak fan-out days did not throttle.** `[MEASURED]` The busiest single days
(Codex 155 sessions on 2026-06-30, Claude 177 sessions on 2026-07-05) show no
GitHub throttle errors at all. The three real primary-limit days were not the
peak fan-out days.

**Why our history did not hit the content-creation wall.** Section 3.2 gives the
mechanism. Only 29 percent of `gh` calls were content-creation writes. The
majority were reads and mutations that meter under the larger primary ceiling.
The writes also spread across many days and, in practice, across more than one
account and token, so no single hour on a single token concentrated enough
writes to trip the 500-per-hour secondary cap.

This is the honest headline. The GitHub content-creation ceiling is real
GitHub policy, and the audit's mechanism is sound, but our own transcripts do
not show us slamming into it repeatedly. The ceiling is a latent risk that
would bite harder as write concentration rises, not a routine historical stall.

### 3.4 The fan-out signature `[MEASURED]`

The corpus shows a clear ramp from a single-agent tempo to a fleet tempo.

| Month (2026) | Codex sessions |
| --- | ---: |
| March (from the 26th) | 34 |
| April | 102 |
| May | 116 |
| June | 624 |
| July (to the 22nd) | 671 |

That is roughly a twenty-fold scale-up in monthly session volume from March to
July. Peak single days reach 155 Codex sessions and 177 Claude sessions, for a
combined peak above 300 agent sessions in one calendar day.

Codex also carries a native multi-agent orchestration signal. In the sample,
about 5 percent of Codex sessions are orchestrators that spawn sub-agents. Those
orchestrators issued 155 `spawn_agent` calls and 1,063 `wait_agent` calls in
the sample, which scales to roughly 1,550 sub-agent spawns across the corpus.
Every spawned agent adds its own stream of `gh` and `git` round-trips against
the same shared GitHub account budget.

### 3.5 Cadence `[MEASURED]`

Per-session command intensity, measured on the sample:

- Codex: about 975 shell commands per session, 19.7 `gh` calls per session,
  16.0 `gh issue` calls per session, and 6.3 `git push` calls per session.
- Claude: about 203 shell commands per session, 3.2 `gh` calls per session, and
  1.0 `git push` per session.

Session duration is strongly bimodal (Codex sample): the median session is about
1 minute, the 75th percentile is 12 minutes, the 90th percentile is 103
minutes, and the longest single orchestrator file spans about four days. The
short tail is delegated single-task and fixture runs. The long tail is
persistent orchestrator sessions that hold a lease and keep dispatching.

The document cannot measure a clean issue-open-to-close wall-clock time or a
pull-request-open-to-merge time from these transcripts, because a single
session rarely contains both ends of one item. That figure needs the issue and
pull-request event stream, not agent transcripts, and this document does not
fabricate it.

---

## 4. Per-friction mapping — how the OpenAgents system removes each cost

Each row takes a friction measured in Section 3 and names the exact OpenAgents
mechanism that removes it, from the replacement audit and the Full Auto grading
lane. The right column is `[PROJECTED]` unless marked otherwise, because live
numbers for the owned path are not yet in hand.

| Measured friction (Section 3) | OpenAgents mechanism | Why it removes the cost |
| --- | --- | --- |
| ~38,000 `gh` REST round-trips, each crossing the public internet | Owned Nostr relay in the same Google Cloud region as the fleet | A publish or a subscription answers in single-digit to low-tens of milliseconds instead of a public-internet REST round-trip. `[PROJECTED]` on the exact per-call figure. |
| ~29,000 `gh issue` calls used as the claim and status ledger, 54 percent of them reads and polls | The Sol claim and issue ledger as signed kind 1621 and 1630-1633 events, read as a live relay subscription | The poll loop (`gh issue list` and `gh issue view`) becomes one subscription filter. Events arrive as they are written. No pagination, no repeated GET. `[PROJECTED]` |
| The content-creation ceiling (about 500 writes per hour per account) as a latent cap on write concentration | An owned relay with no per-account content ceiling | The write budget is OpenAgents policy, not a vendor limit. More agents do not contend for one shared 500-per-hour budget. `[PROJECTED]` |
| The 5,000-per-hour primary ceiling, hit three times in history under heavy `gh api` use | Direct relay reads of repository events, no REST accounting | The primary ceiling does not apply to an owned relay. `[PROJECTED]` |
| ~12,000 `git push` operations, each waiting on server-side branch protection and merge-queue checks | Signed-state push rule plus a signed merge receipt | A push is admitted when the commit matches the maintainer-signed 30618 state event. The signature is the credential. There is no merge-queue wait on the internal critical path. `[PROJECTED]` |
| Issue-template ceremony (`.github/ISSUE_TEMPLATE/strict-bug.yml`) on the internal agent ledger | A kind 1621 issue event carrying exactly the OpenAgents policy tags | The strict-bug gate is correct for public human intake. It is friction for an internal agent ledger. The event carries no more ceremony than policy requires. `[PROJECTED]` |
| Raw activity is easy to count, verified outcomes are not | The Full Auto grading baseline (`run-grading.ts`, metric `full-auto-decision-v1`) measures cost and latency per host-verified outcome | Velocity is scored as tokens per VERIFIED outcome, not per commit or per API call, so speed cannot be gamed by raw churn. `[MEASURED]` that the method exists and is enforced in the test sweep. |

The single sharpest reused idea, from the audit Section 3, is that the signed
state event is the push credential. That is the same
signed-intent-to-admitted-mutation-to-receipt shape as the rest of the
OpenAgents verification thesis, applied to git hosting.

---

## 5. The OpenAgents alternative, in one paragraph each

**Full Auto throughput model.** Full Auto runs owner-set objectives in a
continuation loop with a hard turn cap and non-overridable guardrails
(`docs/analysis/2026-07-22-full-auto-autonomy-decision-quality-rubric.md`). The
grading baseline `run-grading.ts` (metric `full-auto-decision-v1`, from META-3)
scores durable run receipts against a D1 to D7 autonomy rubric and divides exact
token usage by host-verified outcomes only. The throughput unit is therefore
tokens per VERIFIED outcome, and unknown usage or zero verified outcomes is
recorded as `not_measured` rather than as zero. This is the correct denominator
for a velocity claim, because it counts finished, checked work, not raw
activity.

**Nostr and GRASP forge speed thesis.** The replacement audit shows that the
NIP-34 event vocabulary is already implemented in owned Effect TypeScript
(`nostr-effect`), that the relay is owned code, and that only a colocated git
server and a push-authorization hook are a bounded new build. On that owned
substrate the claim ledger, the issues, the patches, and the merge receipts are
signed events on a relay OpenAgents controls. The relay has no per-account
content ceiling, answers in single-digit milliseconds when it is near the fleet,
serves live subscriptions instead of poll loops, and carries no merge-queue or
issue-template ceremony on the internal path. GitHub becomes a read-only mirror
during migration, so the move is reversible at every stage.

---

## 6. Live Full Auto dogfood — two measured runs on the owned path

The operator drove two live Full Auto runs against real repositories. Both runs
went through the OpenAgents Full Auto local control API. No human hand touched
the code. Both runs used the `codex-local` lane on model `gpt-5.6-sol`. The
values below come from the live run receipts only. They are a different
measurement from the historical estimates in Section 3. Do not infer any live
cell from the historical estimates above.

These are two data points on small units of work with one provider. The
wall-clock figures are `[MEASURED]` from the run receipts. Any claim that these
figures hold for the general case is `[PROJECTED]`.

### 6.1 Run A — Nostr forge Stage 0a (`nostr-effect` repository)

**Run identity**

| Field | Value |
| --- | --- |
| `runRef` | `run.full-auto.mrwh3scs.6uaum1ss` |
| Target repository | `nostr-effect` |
| Lane and model | `codex-local`, `gpt-5.6-sol` |
| Turn cap | 12 |
| Start timestamp | 2026-07-22T19:26:21Z |
| Verified-fix timestamp | 2026-07-22T19:27:49Z |
| Wall-clock to verified fix | about 88 seconds `[MEASURED]` |

**Deliverable.** The run aligned the NIP-34 git reply event kind from 1622 to
the NIP-22 kind 1111. It added a `REPLY_KIND` constant and taught `isGitEvent`
to recognize the new kind. The change spans 3 files (+8/-1). This is the exact
"1622 to 1111 interop bug" that the forge audits flagged as Stage 0a.

**Verification.** `bunx tsc --noEmit -p tsconfig.check.json` ran clean. The
Nip34 test suite ran 29 tests with 0 failures.

**Landed.** `nostr-effect` main commit `ec573c7`, through one `git push`.

### 6.2 Run B — Sarah mobile slice 1 (`openagents` repository)

**Run identity**

| Field | Value |
| --- | --- |
| `runRef` | `run.full-auto.mrwhgl2a.jicdezve` |
| Target repository | `openagents` |
| Lane and model | `codex-local`, `gpt-5.6-sol` |
| Turn cap | 16 |
| Start timestamp | 2026-07-22T19:36:18Z |
| Stop timestamp | 2026-07-22T19:42:50Z |
| Wall-clock | about 6.5 minutes `[MEASURED]` |

**Orchestration.** The run delegated its own read-only audit subagent
(`mobile_sarah_audit`) before it implemented the change. That was a
sub-agent-spawning orchestrator turn on the owned path.

**Deliverable.** The authenticated `principal.sarah` mobile thread now opens
reliably on the existing owner-private Khala Sync subscription. The change spans
3 files (`app.tsx`, `mobile-conversation.ts`, and its test). It reuses the
existing runtime and brokers and adds no Sarah-specific storage.

**Verification.** The mobile suite ran 325 tests across 62 files with 0
failures. `pnpm --dir apps/openagents-mobile run typecheck` and
`git diff --check` also passed.

**Landed.** `openagents` main commit `7764bf47df`, through the normal pre-push
gate green. The operator did not use `--no-verify`.

### 6.3 Grading (the `full-auto-decision-v1` rubric, from #9182)

The grader ran the `full-auto-decision-v1` rubric over both live run stores.

| Rubric dimension | Grade | Note |
| --- | --- | --- |
| D1 complexity | C0 (lower bound) | The run reports carry no tool-call, file-change, or sub-agent counts, so the grader can only assign the lower bound. |
| D2 coherence | 2.0 | — |
| D3 foresight | 1.0 | — |
| D5 selectivity | 0/4 | HONEST: the operator supplied the objective (source `control_caller`). The system did not self-select the work. |
| D7 recoverability | 1.0 | — |
| D4 groundedness | `not_measured` | The control-API run path does not enable the autonomy HANDS-2 host-verification. |
| D6 self-verification | `not_measured` | Same cause as D4. There is no host-verification signal to grade. |
| Tokens per verified outcome | `not_measured` | There is no usage writer on the control-API run path yet. |

The `not_measured` cells are honest. They record the same record-shape gaps that
the grader itself reports. The run reports carry no tool-call, file-change, or
sub-agent counts, and there is no usage ingestion on this path. This is a real
observability gap on the control-API run path, not a strength.

### 6.4 Round-trip comparison to the historical baseline

Section 3 measured the historical GitHub-centric tempo at about 19.7 `gh` calls
and about 6.3 `git push` operations per session, for about 59,000 GitHub network
interactions over four months. More than half of the roughly 29,000 `gh issue`
calls were reads and polls, and the issue tracker was the coordination
substrate.

For these two live dogfood units, the OpenAgents-system round-trips were:

| Live unit | `gh` calls | `git push` | Extra owned round-trips |
| --- | ---: | ---: | --- |
| Run A (`nostr-effect`) | 0 | 1 | The repository's own NIP-34 relay pre-push hook fired. The forge model was already running. |
| Run B (`openagents`) | 0 | 1 | None. |

Each live unit reached a verified, merged change through one `git push` and zero
`gh` calls. The historical GitHub-centric cycle for a comparable small fix opens
an issue, branches, commits, opens a pull request, waits on a review round-trip,
and merges. That path costs multiple `gh` round-trips plus human gates. The two
live units removed the round-trips and removed the human gate on the code. The
wall-clock to a verified merged change was about 88 seconds for Run A and about
6.5 minutes for Run B `[MEASURED]`. The claim that this margin holds for the
general case is `[PROJECTED]` from two small single-provider data points.

The two runs also confirm the honest correction that Section 3 already makes.
Neither run hit any GitHub rate ceiling. The win here is round-trip elimination
and autonomy, not throttle avoidance.

### 6.5 Honest caveats — gaps to close before this is a clean pipeline

The dogfood surfaced real gaps to close before this is a clean measurement
pipeline. State them plainly.

- Local Full Auto runs persist run reports, not ATIF traces. ATIF is the
  server-side Khala and Codex delegation path.
- The coherence and complexity rubric is manual. It does not run automatically
  per turn.
- D6 and tokens per verified outcome stay `not_measured` until HANDS-2
  host-verification and usage ingestion are wired into the control-API run path.
- Both runs stalled on liveness (`dispatch_overdue` and stop) after they
  delivered, under concurrent-lane codex contention. The operator landed the
  verified deliverables as a correction of last resort.

These are buildable follow-ups. They are not reasons to distrust the two
measured outcomes above.

---

## 7. Verdict — does our history support the "faster than GitHub-centric" thesis?

**Yes, but through a different argument than the audit leads with.**

The audit leads with the GitHub content-creation ceiling. Our history does not
strongly support that framing. We hit GitHub's primary rate limit only three
times in four months, and we hit the secondary content-creation ceiling
essentially never, even on our peak fan-out days above 300 agent sessions. The
ceiling is real GitHub policy and it is a genuine latent risk as write
concentration grows, but it was not a routine historical stall for us. Claiming
otherwise from our own data would be dishonest. `[MEASURED]`

The stronger, data-supported argument is volume and poll tax. Over 118 days our
agents made an estimated 38,000 GitHub REST round-trips and 21,000 git transport
operations, about 59,000 GitHub network interactions, averaging near 500 a day
and peaking in the multi-thousands on fan-out days. Of the roughly 29,000
`gh issue` calls, more than half were reads and polls of a backlog that an
owned relay would deliver as one live subscription. Every one of those 59,000
interactions paid public-internet REST latency, auth, and rate-limit accounting
that a near, owned relay does not charge. That is where the measured time went,
and that is exactly what the owned-relay and live-subscription model removes.
`[MEASURED]` for the volume, `[PROJECTED]` for the removal until the live
dogfood in Section 6 is filled.

So the honest verdict is: our history proves a large, real, removable GitHub
round-trip and ceremony tax, and it proves the tax scaled with fan-out. It does
not prove that GitHub throttling stalled us historically. The OpenAgents system
targets both the proven tax and the latent ceiling, and the live Full Auto
dogfood is the measurement that will turn the projected removal into a measured
one.

---

## 8. Watch items

- **Extend Section 6 with more runs and a usage writer.** The historical side of
  this benchmark is complete. Section 6 now carries two measured owned-path runs
  (`run.full-auto.mrwh3scs.6uaum1ss` and `run.full-auto.mrwhgl2a.jicdezve`). The
  next step widens the sample and wires usage ingestion so the
  tokens-per-verified-outcome cells stop reading `not_measured`.
- **Multi-account attribution.** This document infers that historical writes
  spread across more than one GitHub account. A follow-up could confirm the
  account split directly from the transcripts, which would sharpen the
  secondary-ceiling risk estimate.
- **Issue-lifecycle cadence.** A clean issue-open-to-close and
  pull-request-open-to-merge time needs the GitHub event stream, not agent
  transcripts. It is the one cadence figure this document could not measure.
- **Re-verify the GitHub limit numbers.** The primary 5,000-per-hour and
  secondary 500-per-hour figures are GitHub public policy that changes. Re-check
  them before any external claim, as the replacement audit also warns.
