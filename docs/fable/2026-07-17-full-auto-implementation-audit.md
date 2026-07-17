# Full Auto — Run Failure, Handoff, and Product-Model Audit

- **Date:** 2026-07-17
- **Status:** Corrected after inspecting the failed overnight run
- **Scope:** The current Full Auto implementation on `main`, the failed owner dogfood run, transcript quality, provider handoff coverage, and the interaction model shown in OpenAgents Desktop.
- **Evidence boundary:** Private transcript and local Desktop state were inspected in place. This tracked audit records only bounded findings and identifiers; it does not copy raw private prompts, provider logs, secrets, or full transcripts.

---

## 1. Correction to the first audit

The first version of this audit was wrong to say that the remaining gaps were “almost entirely proof/release gaps, not code gaps.” The owner had already supplied stronger evidence: an overnight Full Auto run visibly stopped. That incident was not incorporated into the assessment.

Full Auto has a substantial durable continuation core, but it is **not yet a dependable unattended product**. The missing work includes runtime resilience, run-level observability, transcript analysis, cross-provider handoff proof, durable objective continuity, and a clear autonomous-mode interface. Release packaging and formal assurance remain gaps too, but they are not the whole gap.

The corrected verdict is:

> Full Auto can autonomously complete useful bounded packets, and the specific overnight thread-eviction defect was fixed after the failure. However, the system has not yet demonstrated that a user can start a clearly defined autonomous run, walk away, return to an intelligible run record, and trust recovery or provider handoff across the failure modes exercised by real use.

---

## 2. What failed overnight

### 2.1 Incident timeline

The failed run was the Desktop conversation titled **“Hello”**, backed by Codex provider session `019f6e70-3379-7431-be1f-79d64b8a510d`.

| Local time (CDT) | Observed event |
|---|---|
| Jul 16, 11:57 PM | The session began with the message “Hello.” |
| Jul 16, 11:58 PM | The owner-admitted Fast Follow program began under Full Auto. |
| Jul 17, 12:12 AM | The first autonomous packet completed successfully after about 14m 40s. |
| Jul 17, 12:12 AM–6:11 AM | No next provider turn started. The Desktop surface showed **Turn failed** and the terminal message **“That conversation no longer exists.”** |
| Jul 17, 6:12–6:21 AM | A separate diagnostic turn reconstructed the failure and landed the fix. |
| Jul 17, 6:38 AM | The provider session began receiving continuations again. |

This was a roughly **six-hour silent loss of autonomy** during the exact window the owner expected the product to work unattended.

### 2.2 Root cause

The provider did not fail while performing the Fast Follow packet. The packet finished and produced a valid terminal outcome. The next Full Auto reconciliation failed before provider execution:

1. Desktop kept only five threads in its mutable local thread cache.
2. Eviction used creation time rather than last access.
3. Concurrent chats displaced the older-created Full Auto thread even though it was still the active continuation target.
4. Full Auto retained a durable enabled record and attempted the next turn against that thread ref.
5. `provider-lane.ts` correctly failed closed when `thread-store.ts` could not open the ref, returning “That conversation no longer exists.”
6. The loop recorded a failed dispatch/backoff, but the owner-facing surface exposed only a generic failure banner and did not recover the thread automatically.

Commit `8cb900bbf9` later changed the mutable cache to last-access LRU while preserving creation-time sidebar ordering, and added restored/between-turn Full Auto regressions. This specific defect is fixed on current `main`.

### 2.3 Why the failure still matters after the fix

The incident invalidates a test-only conclusion that the loop was production-complete. Existing restart, registry, and fixture tests had not exercised the real composition:

- a long autonomous provider turn;
- several concurrent chats;
- bounded mutable-thread eviction;
- the gap between a successful terminal turn and the next reconciliation;
- the owner returning hours later to diagnose the failure from the UI.

The regression closes the exact cache defect. It does not prove the surrounding unattended-run contract. A dedicated replay must recreate thread pressure while a real Full Auto run advances through multiple turns.

---

## 3. What the transcript says about Full Auto quality

### 3.1 Positive evidence

The provider transcript is not empty-loop evidence. Full Auto selected bounded Fast Follow packets, reconciled current `main` and active claims, used isolated worktrees, ran focused and repository checks, published scoped commits, released claims, and stated residual work rather than declaring the whole program complete. After the cache fix, the same provider session continued making useful progress.

This demonstrates that the repo-grounding prompt plus repository authority can produce real work. It does **not** demonstrate reliable unattended operation.

### 3.2 Product and observability defects exposed by the transcript

1. **No run-level diagnosis.** The user saw “Turn failed” and a low-level missing-conversation message. The UI did not say that the previous packet succeeded, that the continuation target had disappeared from Desktop state, whether Full Auto would retry, when it would retry, or what action would resume it.
2. **No durable run report.** The control API exposes bounded turn metadata but deliberately no transcript. There is no run artifact that summarizes objective, turns attempted, useful outcomes, commits, failures, recovery actions, provider transitions, elapsed unattended time, or final stop reason.
3. **No transcript-review loop.** Nothing automatically evaluates a completed run for repeated reconnaissance, wasted verification, prompt drift, poor packet choice, stalled intervals, or missing evidence. The requested “analyze a full auto run and improve it” cycle is currently a manual forensic exercise over private provider JSONL plus Desktop state.
4. **The title was meaningless.** “Hello” concealed a substantial Fast Follow program in the sidebar. An unattended run needs a run title derived from its objective, not the first throwaway message.
5. **The objective is not a first-class durable record.** The registry persists workspace, lane/account/model/effort, continuation count, lease, and failures, but not a structured objective or acceptance condition. Same-provider continuity can preserve the original prompt in the provider session; a recovered or switched provider receives only bounded Desktop history. Tool traces and the 80-note thread bound can displace the original objective.
6. **A successful turn and a healthy loop are conflated.** The first packet succeeded, but the run failed. Metrics must distinguish turn outcome, continuation dispatch, run liveness, and useful program progress.
7. **Long gaps are not escalated.** A run can remain enabled yet make no progress during backoff, missing state, app shutdown, or a stale target. There is no owner-facing “stalled for 5m/30m” state with a specific cause.
8. **The generic continuation prompt is too weak as sole goal state.** “Look at this repository and do the next concrete useful thing” is acceptable as a fallback policy, not as the only durable mission contract for a multi-hour program.

### 3.3 Minimum run-analysis artifact

Every Full Auto run should produce a private, bounded `FullAutoRunReport` with:

- run ref, thread ref, title, explicit objective, workspace, started/stopped timestamps;
- provider/lane per turn and every provider transition;
- per-turn disposition, duration, selected packet/issue, and bounded outcome summary;
- commits/receipts claimed by the agent, verified independently where possible;
- failure classification, retry/backoff, recovery action, and disabled reason;
- liveness gaps over a threshold;
- objective/acceptance progress and remaining work;
- transcript-analysis findings and recommended prompt/product changes;
- a pointer to private raw evidence rather than raw transcript contents.

Without this artifact, “dogfood Full Auto, inspect it, improve it, repeat” is not an operable product loop.

---

## 4. Provider handoff: implemented seam, unproven experience

### 4.1 What exists

Desktop has real provider-lane infrastructure:

- `provider-lane-registry.ts` durably binds one lane to a thread.
- A compatible switch projects up to 32 messages / 64,000 characters of host-owned history.
- `provider-lane.ts` rebuilds provider input from main-owned thread notes rather than trusting renderer-supplied history.
- Switching Codex ↔ Claude resets the target lane’s native continuity so the next provider starts from the bounded host transcript.
- Capability admission fails closed; an active Full Auto thread cannot switch to a lane that does not advertise Full Auto.
- Codex, Claude, Grok ACP, and Cursor ACP have lane adapters and Full Auto policy entries on `main`.

Therefore **manual Codex ↔ Claude conversation continuation is architecturally supported**.

### 4.2 What has actually been tested

Current tests prove pieces independently:

- registry selection persistence and bounded-history projection;
- rejection of missing auth, unadmitted peers, and capability mismatch;
- composer controls change when the active lane changes;
- each provider adapter can consume history in its own unit tests;
- individual Codex, Claude, and Grok Full Auto/live-loop paths have separate evidence.

They do **not** prove the behavior the owner asked about:

> Provider A says or does something recognizable in a real visible conversation; the user switches that same thread to Provider B; Provider B demonstrably receives the right context and continues; the transcript and sidebar make the handoff legible; restart still preserves it.

There is no real-provider, same-thread, sequential cross-provider acceptance test. There is no provider-transition receipt in the transcript. The renderer’s visible provider picker exposes Codex and Claude only; ACP lanes are registered in main but are not first-class choices in that picker. Full Auto also pins one execution profile and does not autonomously decide or execute provider handoffs.

### 4.3 Handoff risks not covered

- The original objective may be outside the 32-message/64k projection or the 80-note mutable thread bound.
- System/tool trace notes share the bounded history and can crowd out semantically important user/assistant context.
- Success switches do not append an owner-visible “Codex → Claude” transition note.
- A target provider may paraphrase context successfully while silently losing files, pending questions, plan state, queued followups, or provider-private session state.
- Full Auto’s pinned profile and a manually changed lane can disagree unless the pause/switch/resume transition is explicitly modeled and receipted.
- ACP handoff cannot be assessed from the current two-choice composer UI.

The honest status is **handoff plumbing present; cross-provider conversation handoff unverified**.

---

## 5. Small visible test batch for the sidebar

The next dogfood cycle should create separate, plainly named conversations in the owner’s real left sidebar. Do not bury these in a headless fixture profile.

| Sidebar title | Steps | Pass condition |
|---|---|---|
| `TEST 01 · Codex → Claude · context` | In Codex, establish marker `ORBIT-17` and a two-step task; switch the same thread to Claude; ask it to state the marker and perform step two. | One thread only; Claude states the marker and continues from Codex’s result; transcript shows both providers and the transition. |
| `TEST 02 · Claude → Codex · context` | Mirror Test 01 with marker `LANTERN-42`. | One thread only; Codex continues Claude’s work with exact context. |
| `TEST 03 · Codex → Claude · objective retention` | Start with an explicit objective/acceptance rule, generate enough tool activity to pressure bounded notes, then switch. | Claude can state the original objective and acceptance rule, or the product visibly reports that context was truncated and requires confirmation. |
| `TEST 04 · Full Auto · Codex · 3 turns` | Launch a three-packet bounded run and walk away. | Three useful terminal turns, no manual message between them, visible progress, explicit stop reason. |
| `TEST 05 · Full Auto · Claude · restart` | Launch, allow one turn, quit/relaunch Desktop, observe two more turns. | Same sidebar run resumes with the same objective and lane; no duplicate turn. |
| `TEST 06 · Full Auto · thread pressure` | Launch Full Auto, then create/open more than five other chats while its turn runs. | The autonomous thread remains addressable and the next continuation starts; this is the real replay of the overnight incident. |

Use the existing rename action immediately after creating Tests 01–03. The Full Auto control API already accepts a `title` and `lane` for Tests 04–06, but a productized UI should provide those fields directly. After each test, rename its prefix to `PASS`, `FAIL`, or `BLOCKED` so the sidebar itself is the run index.

This is intentionally small: two short manual handoffs, one context-pressure handoff, and three three-turn Full Auto runs. It yields direct visible evidence without pretending that a large matrix is required before the basic contract works.

---

## 6. Interaction-model audit

### 6.1 Current behavior

The composer renders Full Auto beside provider/model/reasoning controls as an `aria-pressed` toggle. Its implementation is stronger than its presentation: enabling persists immediately and schedules reconciliation immediately, including on an empty new session. It is not merely a flag on the next sent message.

The placement nevertheless communicates the wrong mental model. In a message composer, a toggle naturally reads as “apply this option to what I send.” The same surface also retains a text editor, Queue, Stop, and ordinary send behavior while Full Auto runs. The product is asking the user to interpret one canvas simultaneously as:

- an interactive chat they steer message by message; and
- an unattended autonomous program they start and leave.

That ambiguity is visible in the owner’s question and is therefore a product defect, regardless of the internal state machine being durable.

### 6.2 Recommended model: Full Auto is a run, not a composer option

Add a lightning-bolt **Full Auto** action directly under or beside **New session** in the left rail. It creates a distinct run-shaped thread.

The launch surface should collect only the mission contract:

- title (auto-suggested, editable);
- objective and explicit done condition;
- workspace;
- provider/lane;
- bounded turn limit (default 20, clearly shown).

After Start, the main canvas becomes a **read-only run view** for v1:

- objective and workspace remain pinned at the top;
- current state is explicit: Running, Pausing, Paused, Retrying, Stalled, Completed, Failed, or Cap reached;
- the primary control is **Pause** while running and **Resume** while paused;
- Stop is terminal and distinct from Pause;
- the transcript remains inspectable, with per-turn provider, duration, outcome, and artifacts;
- the ordinary composer is absent while running.

This cleanly separates autonomy from steering. A future steering feature can be introduced deliberately as **Pause → add instruction → Resume**, with the instruction appended to durable goal state. It should not be smuggled in through an always-visible chat box.

### 6.3 State-machine implication

The durable model should graduate from `enabled: boolean` to explicit run intent:

```
draft → running ↔ paused → completed
             ↘ retrying / stalled
             ↘ failed / stopped / cap_reached
```

`Pause` means “finish or interrupt the current bounded turn, then dispatch nothing else.” `Stop` means “terminally end this run.” Neither should be represented as the same toggle-off mutation. Every transition needs actor, timestamp, reason, and previous/next state.

The `impeccable` product-interface review materially changes the recommendation here: moving the existing toggle without changing the surrounding interaction contract is insufficient. The dedicated run view, legible state, and removal of the live composer are the important changes.

---

## 7. Current implementation strengths that should be preserved

The correction above does not erase the useful core already on `main`:

- main-process-owned durable registry and restart reconciliation;
- exact workspace binding and fail-closed mismatch behavior;
- serialized reconciliation plus a durable dispatch lease;
- bounded failure backoff and disable threshold;
- lane/account/model/effort continuity;
- 20-continuation cap;
- typed disable attribution;
- background-turn Stop targeting;
- loopback bearer-gated control API with OpenAPI parity tests;
- corrupt-registry quarantine;
- Codex, Claude, and admitted ACP lane policy abstraction.

These are foundations for the run model, not evidence that the product loop is finished.

---

## 8. Open gaps, ranked

### P0 — unattended-run credibility

1. Add the real thread-pressure replay (Test 06) and keep it in the release gate.
2. Add run-level liveness/stall detection and owner-visible cause/retry state.
3. Make objective/done-condition first-class durable data, independent of bounded transcript/provider continuity.
4. Produce a bounded private run report and transcript-analysis pass after every dogfood run.

### P1 — product contract and provider continuity

5. Replace the composer toggle with the dedicated Full Auto launch/run experience and explicit Pause/Resume/Stop states.
6. Execute and receipt Tests 01–03 as real Codex ↔ Claude handoffs.
7. Add a successful provider-transition transcript event and preserve objective/acceptance context across the switch.
8. Decide whether ACP providers become visible first-class picker options; until then, do not claim ACP conversation-handoff UX.
9. Model pause → provider switch → resume explicitly if cross-provider Full Auto is desired. Current Full Auto is single-lane per bound profile.

### P1 — evidence and release

10. The Full Auto AssuranceSpec remains `proposed` with 37/37 obligations `needs_design`.
11. No Desktop release tag currently contains the Full Auto implementation; the newest tag remains `openagents-desktop-v0.1.0-rc.12`.
12. The owner-observed packaged restart-resume success metric is unmet.
13. Usage/quality metrics are still gated off, so useful-work rate, median consecutive turns, stall time, and stop reliability are unmeasured.
14. Cursor ACP remains pending behind the ACP compatibility/release matrix; ACP claims remain experimental.

### P2 — control and presentation

15. Control API turn history is metadata-only and cannot power run review by itself.
16. Generic/first-message titles make autonomous runs hard to find.
17. Cross-machine control remains a separately scoped future phase.
18. The Full Auto public promise remains red pending real-use evidence.

---

## 9. Required iteration cycle

Do not return directly to feature expansion. Run this loop:

1. **Launch** one named, bounded Full Auto run from the real owner profile.
2. **Observe** it through at least three continuations, including one chosen stressor (restart, thread pressure, provider switch while paused, or transient failure).
3. **Capture** a bounded run report from Desktop state plus provider evidence.
4. **Analyze** the transcript for useful work, duplicated setup, drift, stalls, unclear UI state, and false completion claims.
5. **Classify** every failure as provider, runtime, state continuity, workspace, policy, UI projection, or operator-control failure.
6. **Fix one highest-leverage defect** and add the exact replay as a regression.
7. **Repeat** the same named test and compare report fields, not anecdotes.
8. Only after the six-test sidebar batch is green should release packaging become the primary remaining work.

The immediate program should therefore be: incident replay → run report/analysis → dedicated run UX → real Codex/Claude handoff batch → packaged restart batch → release/assurance.
