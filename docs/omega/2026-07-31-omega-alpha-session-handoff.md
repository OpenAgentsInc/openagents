# Omega 0.2.0 Alpha — Session Handoff, 2026-07-31

Status: handoff. Written at the end of a long multi-lane session that was
stopped abruptly at owner direction. This document is the pickup point for the
next agent.

Scope: everything landed, everything left broken, everything left undecided,
and the reasoning behind the calls that were made. Read it before starting any
Omega 0.2.0 alpha work.

Companion documents, all still current:

- `docs/omega/2026-07-29-omega-zero-base-single-experience-plan.md` — the C1–C9
  broken-control inventory and the five-phase single-experience plan.
- `docs/omega/2026-07-29-omega-alpha-roadmap.md` — Waves 0–4 and the issue set.
- `~/work/omega/docs/omega/release-gate.md` — the release gate ledger. **This is
  the binding artifact for shipping.** It lives in the `omega` fork, not here.
- `~/work/omega/docs/omega/release-gate-operator-notes.md` — the human-gate
  operator runbook.
- `docs/afteraction/2026-07-31-codex-019fb495-overnight-spend-audit.md` — the
  prior after-action audit. Its §7.3 rules are binding on all agents.

---

## 1. Stop first: two live defects in production

Both are user-facing. Neither is fixed. Both were mid-repair when the session
was stopped.

### 1.1 Community LiveKit rooms are broken, and we broke them

**Symptom:** every community session that needs shared-room bootstrap fails
with `503 sarah_voice_livekit_unavailable`. Reproduced twice on serving
revision `openagents-monolith-00374-h4p`.

**Actual error, one layer down:**

```
Sarah shared-room authority bootstrap failed (400): {"error":"device_ref_required"}
```

**Cause:** commit `94d49d8bab` (this session's duplicate-participant fix) added
a `device_ref_required` gate to the room-authority handlers.
`bootstrapLiveKitCommunityRoom` in `apps/openagents.com/workers/api/src/index.ts`
synthesizes its shared-room `summon` request with **only a content-type
header**, so it cannot satisfy its own callee's new contract.

**Do not fix this by synthesizing a device ref.** The gate exists for authority
reasons. An internal bootstrap that fabricates a credential to pass its own
check has not satisfied the authority requirement — it has disabled the gate for
exactly one caller and left a synthetic credential shape in the data that looks
real to every downstream reader. The correct fix is one of:

- thread through a legitimate device/participant identity the bootstrap already
  has and simply is not passing, or
- give the gate an explicit internal-authority branch for genuine server-side
  actors.

Whichever is right, **a bootstrap must remain distinguishable in the data from a
real client-supplied device ref.** Collapsing them is precisely how the
`resolveMember` bug happened (three distinct causes collapsed into `undefined`).

**Why tests did not catch it:** the gate's own tests exercise the gated handler
directly, never its internal caller. This is the same "tested but uncalled"
family described in §4.1 — an internal caller that silently stopped satisfying
its callee's contract. An open question worth answering: can
`scripts/uncalled-production-symbol-guard.mjs` be extended to reach this class?

Blocks omega#186 and the community journey.

### 1.2 A user who closes their client mid-turn permanently loses Sarah voice

**Severity: incident class, user-reachable.** This was initially misdiagnosed as
test-harness exhaust. It is not.

**Mechanism:** a Sarah voice session that reaches state `accounting_uncertain`
is never cleared. The production partial unique index
`sarah_realtime_voice_owner_active_idx` covers
`('reserved','connected','accounting_uncertain')`, but `sweepExpired`
(`packages/khala-sync-server/src/sarah-realtime-voice-store.ts:4941`) selects
only `reserved` and `connected`. The uncertain row therefore occupies the
owner's single concurrency slot forever, and every later voice session for that
owner is refused `sarah_voice_concurrency_limit`.

**The sharp version:** `sweepExpired` is *itself* what creates
`accounting_uncertain` (same file, `:5058`). The same function opens the state
that occupies the slot and has no branch that can ever close it. It is not
missing a case — it is missing the other half of a loop it opens. That is why
the per-minute sweep reports healthy the entire time.

**Proof it is user-reachable, not drill exhaust.** Zero faults were ever
injected by any drill lane, and
`SARAH_LIVEKIT_PROVIDER_DISCONNECT_ACCEPTANCE_ENABLED` was never armed. The live
stuck row (`drill-private-de454ad8-87fe-4ec5-86b7-50771f22a9cf`, created
19:13:19Z) has this timeline:

```
19:13:44.968  sarah_voice_client_closed
19:13:45.205  worker close
```

An ordinary client disconnect during bring-up. Any real user who closes a client
mid-turn reproduces it.

**Do NOT "fix" this by adding `accounting_uncertain` to the sweep's terminal
handling.** This is the trap. Migration
`0115_sarah_livekit_accounting_reconciliation.sql` widened that index
deliberately, to *preserve the owner's hold until exact provider accounting is
reconciled instead of fabricating a terminal settlement from the usage seen so
far*. A sweep that terminated that state would invent the exact number the
design refuses to invent — trading a liveness bug for a money bug, which is
strictly the worse direction.

This session's coordinating agent **did** issue that wrong instruction. It was
caught only because a second lane independently verified the claim instead of
accepting it. That is luck, not a control. A regression test asserting the sweep
never terminates `accounting_uncertain` was being landed when the session
stopped — **confirm whether it landed, and land it if not.**

**The genuine defect is narrower:** the index conflates *has unreconciled
accounting* with *has a live session*. Two different facts wearing one lock.

**The two honest repairs are RESERVED for the owner**, because both change
settlement invariants and need `INVARIANTS.md` treatment:

1. stop letting an accounting state act as a per-owner concurrency lock, or
2. give reconciliation a bounded deadline that escalates.

**Clearing the existing stuck row** requires provider evidence through the
sanctioned reconciliation path (`sarah-realtime-voice-store.ts:3335-3353`),
which correctly demands it. Nobody can currently supply it: the drill driver is
deliberately built so the raw provider session ref never leaves the worker —
`assertPublicSafeSarahLiveKitDrillObservation` rejects any observation carrying
a provider/session/room ref, and receipts store digests only. No artifact
anywhere maps to digest `400e2714…`. **Escalate as an owner action requiring an
OpenAI usage export; leave the row uncertain until then.**

### 1.3 Systemic hazard behind 1.2

`sarah_realtime_voice_owner_active_idx` is per-owner, and **every LiveKit lane
shares the same two acceptance identities from Secret Manager**
(`oa-livekit-acceptance-{private,community}-nostr-key`). So one lane's stuck
reconciliation silently denies voice to every other lane using that identity,
with no signal anywhere. This cost three lanes and roughly an hour to diagnose.
An additive alarm for a stuck `accounting_uncertain` older than some bound was
in flight when the session stopped. **Confirm and finish it.** It touches no
settlement semantics and is safe.

---

## 2. What landed and is safe

All pushed to `origin/main`. openagents head at handoff: `31a0d98f7b`. omega
head at handoff: `8e45faa7bc`.

### 2.1 Spend, metering, and money

| Commit | What |
| --- | --- |
| `9933cd8fcd`, `5babaa5e19`, `4adee7da8a` | Per-tester ceiling 200k → **1,000,000**; `GLOBAL_DAILY_LIMIT` 50 → 20. Aggregate 10M → **20M/day**. |
| `7590c0709b` | Vertex Gemini `thoughtsTokenCount` → `reasoningTokens`, both parsers. |
| `4fdc3f3820` | Same fix across fireworks, openrouter, and passthrough (OpenAI shape). |
| `72591c0981`, `e3ca80af34` | Ledger replay under-count + in-flight reservation. |
| `45f2481e9e`, `cedc5adcef`, `f06b040b2b` | Luna gateway: `max_completion_tokens` allow-list, streaming passthrough. |
| `7afca05106` | Gemini `$ref`/`$defs` schema sanitization, streaming `functionCall`/`thoughtSignature` retention, `MAX_TOKENS` mapping. |

**The ceiling is now provably enforced.** It previously read `0` forever:
`COALESCE(SUM(total_tokens),0)` returns a *string* (bigint SUM → numeric →
text), and a `typeof === 'number'` guard was therefore always false. One
identity drew 1.96M against a 1M cap while everything looked healthy. Verified
by temporarily forcing the ceiling to 20,000 and observing:

```
HTTP 429 {"error":"free_tier_daily_token_ceiling_reached",
          "servedToday":22248,"tokensPerDay":20000}
```

`22248` = exactly 5 × 4,422 + 138 of real usage. Token-accurate, decisively not
zero. Restored to 1,000,000 immediately after; the restrictive value was live
~2–3 minutes.

**Gemini 502 root cause** (from a direct Vertex call, not inference):
`maxOutputTokens: 16` against a ~4,400-token prompt returns **HTTP 200**,
`finishReason: "MAX_TOKENS"`, **no `content` key at all**, and
`thoughtsTokenCount: 12` — thinking draws from the same output budget, so the
visible answer never starts. Both dispatch validators classified empty assistant
content as retryable without ever reading the finish reason; the gemini class
has a single lane, so the retryable error had nowhere to fail over and hit the
catch-all 502. Now: empty + `finishReason === 'length'` is served as the 200 it
is. Empty with any other finish reason still 502s.

**Streaming subtlety worth preserving:** intermediate Gemini SSE fragments send
a *present* `usageMetadata` containing only `trafficType` and no counts; only
the terminal fragment carries them. A naive per-fragment read zeroes reasoning
on any later frame. The existing `?? prior ?? 0` carry-forward handles it and
there is now a test that fails if it is removed.

### 2.2 Guards against the dominant defect class

| Commit | Guard |
| --- | --- |
| `ee71183d3b` | `scripts/uncalled-production-symbol-guard.mjs` — "tested but uncalled". |
| `70879ea052` | `scripts/cli-invocability-guard.mjs` — a CLI that cannot survive its own documented invocation. |
| `742c5bb706` | Assessment recommending against a Rust port of the same rule, with reasons. |
| `5fc99108a9` | `scripts/sarah-participant-join-authority-guard.mjs` — named recording authorities must keep a production caller. |
| omega `1868018389` | `OMEGA-DELTA-0219` — evidence manifest digests must match their rows. |

All are wired into `check:fast`.

### 2.3 LiveKit control plane

| Commit | What |
| --- | --- |
| `94d49d8bab` | `duplicate_participant_refused` became a real 409 on both join paths (it previously had **no production code path at all**). Also: nothing ever set `owner_joined_at`, so **no room binding had ever reached state `active`**; and `bindParticipant`'s upsert reset `removed_at = NULL`, **resurrecting removed members**. Both fixed. |
| `de475872af` | `member_removed` / `membership_changed` production emitters. `resolveMember` had collapsed three distinct causes into `undefined`. **Landed but NOT deployed** — the serving image predates it. |
| `8bd12fe9b0`…`c787117d52` | The single-session failure-drill driver. |
| `63af15e22b`, `21b65ad89d`, `067a0720b5`, `7468fb4c58` | Three latent defects in the *shared acceptance path*, found by drill attempts. |
| `9a4e1e1c3b` | Community admission cleared and recorded. |

**Admission is fully cleared.** All three journey identities hold active
`sarah_voice_cohort:alpha_v1` rows and are admitted to the `openagents-public`
NIP-29 group. A separate community admin key we hold outright was minted and
stored as `oa-livekit-community-admin-nostr-key`, granted relay-side `put-user`,
and added *beside* the existing pubkey in
`SARAH_LIVEKIT_COMMUNITY_AUTHORITY_JSON` (Cloud Run revision
`openagents-monolith-00374-h4p`, env-var only, image unchanged). Admission is
repeatable via `admit-sarah-livekit-community-npub.ts`, which refuses early if
its signer is not in the deployed config.

**Drill driver design decisions worth preserving:**

- `faultInjectedAtMs` is stamped separately from session start, with the
  injector bracketed by a stamp before and after; an injector-reported instant
  is accepted only inside that interval. This excludes ~10s of read-only
  `kubectl` target discovery from a 30s bound while making it impossible for an
  injector to talk a bound down. Excluded time publishes as `faultDiscoveryMs`.
- `concurrentBillableSessionCount: 1` is **measured** by the same gauge sweep
  that names the fault target, not attested. Rooms over-count billable
  generations, which fails closed.
- **Failure is a result, not an exception.** Exceeded bound, absent settlement,
  or a fault aimed at the wrong target each return `outcome: "contradicted"`
  with a named reason and preserved evidence. Only a drill that *could not run*
  raises.
- The settlement observer is plain authenticated HTTP, so it survives loss of
  transport, SFU, and worker.

### 2.4 rc30 candidate — prepared, not shipped

omega `849cdcf2b5` "Prepare Omega 0.2.0 RC 30" is pushed. The build reached a
clean launch gate before it was killed:

| Check | Result |
| --- | --- |
| Starts, survives 30s | alive, **no keymap panic** |
| On-screen version | **`v0.2.0 b30`** (not Zed's `1.14.0`) |
| Default agent label | `Omega Agent · Luna` |
| `--user-data-dir` isolation | identity provisioned in isolated profile |
| Platform default root | never created — **no `HOME=` workaround needed** |

Screenshot was at `/tmp/rc30-tools/rc30-launch.png` (ephemeral; likely gone).

**Notarization never ran.** The build process was killed. To resume: re-run
`script/bundle-omega-rc` from omega `849cdcf2b5` or later.

**Correction that matters for anyone comparing candidates:** five commits that
this session initially believed landed *after* rc29 were in fact already in
rc29 (`c7ecf769e8`) — Luna/label-route (`a79167b72d`), SCV delegation
(`c40049b85c`), Sarah voice barge-in (`bc968334ac`), floating title
(`837180604a`), version truth (`a954971025`). **rc30 = rc29 + exactly one
behaviour commit** (`ba6cc833b1`, the `--user-data-dir` identity-registry fix)
plus gate documentation. A gate row that appears to "change" between rc29 and
rc30 in Luna/SCV/voice/title/version behaviour is measuring something else.

Note for build planning: `cargo bundle` sets `ZED_COMMIT_SHA`, which changes the
fingerprint and forces a second full ~7-minute link of the `omega` crate after
the initial release build. Budget for two links, not one.

---

## 3. The critical path, and why nothing closed

**Nine issues remain open.** openagents #9282, #9284, #9285, #9286; omega #160,
#185, #186, #187, #189.

Their acceptance criteria all funnel through the same place:

- **#9284** — "Packaged Omega completes direct UDP, TCP fallback, and TURN/TLS
  sessions."
- **#9285** — "Two rooms run concurrently under one Sarah public identity with
  distinct jobs, provider sessions, contexts, holds, usage, and settlement."
- **#9286** — "Three authenticated room members see one verified Sarah identity,
  transfer the floor between two participants, and hear Sarah's shared answer."

Every one requires a **packaged-desktop journey**. And that is blocked by §3.1.

### 3.1 The blocker: the packaged client has no dispatch surface

rc29 (and rc30) expose **Join room, Summon Sarah, Talk to Sarah, and Stop only
as GPUI `on_click` closures**. There is:

- no registered action,
- no keymap binding,
- no CLI flag,
- no URL-scheme route.

`omega-effectd` is a private stdio pipe to its own child, and
`omega-device-bridge` refuses every command frame by design. **There is no
unattended driver.** The only path found was macOS Accessibility behind the
undocumented opt-in `ZED_EXPERIMENTAL_A11Y=1` — and GPUI-rendered content is
invisible to Accessibility without it.

**Treat this as a product defect, not a testing inconvenience.** A control that
is not a registered action is a control the zero-base action gate
(`crates/omega_zero_base/`: `ADMITTED_NAMESPACES`, `ADMITTED_ACTIONS`,
`refusal()`) **cannot govern** — a bare closure bypasses the admission model
rather than being admitted by it. This is the same family as the C1–C9 inventory
in the single-experience plan.

It also blocks a release row by construction: `three-mode-journeys` requires
each mode be exercised **once keyboard-only and once pointer-only**, and the
keyboard-only half is impossible for a control with no keymap binding.

**Work was in flight on this and was at the break-proof stage when stopped.**
Its last reported state: *"Now the break-proof — three independent regressions,
each must fail the check."* Check whether anything landed; the omega log at
handoff shows nothing, so assume it is lost and redo it.

**Hazard when converting:** the built-in keymap is unwrapped at startup and a
missing action aborts the process. That is exactly how `0.2.0-rc6` died. Every
action named in a keymap must exist; launch the built binary and confirm it
survives startup before claiming success. Register any new divergence in
`OMEGA_DELTAS.md` as entry + mechanical check + test together.

### 3.2 Receipts that assert what they never measured

Two gate rows are blocked by this, independent of §3.1, and both are fixable
without a client:

- **`sarah-livekit-connectivity`** records only a boolean
  `selectedIcePathObserved`. The harness reads `selectedCandidatePairId` and
  packet counters and **never captures candidate type or protocol**, so even the
  one naturally-selected path cannot be classified as direct UDP, WebRTC TCP, or
  TURN/TLS. Without classification, even a successful forced-transport run
  cannot be recorded as evidence.
- **`sarah-livekit-isolation`** has `identityIsolationObserved` as a
  **hard-coded harness literal**. The harness asserts identity distinctness only
  *within* one scenario, and strips every generation digest from the public
  receipt, so no cross-room comparison survives.

A hard-coded `true` in a receipt is the most dangerous form of this bug: it is
indistinguishable from a real observation to every downstream reader, and the
release gate consumes these receipts directly. **A full inventory of
asserted-but-underived receipt fields was in progress and did not land. Redo
it — it is a deliverable in its own right.**

### 3.3 Gate ledger shape

`~/work/omega/docs/omega/release-gate.md`, as of handoff:

- 8 `automated-pass`
- 13 `owner-assisted-pending`
- 6 Sarah LiveKit rows: 2 `inconclusive`, 4 `blocked`

The 13 owner-assisted rows were parked on a human. Four of them need **no
credentials at all** and are the cheapest real wins available:
`menu-honesty`, `work-surfaces`, `update-safety-lifecycle`, `distribution`.
Of those, `update-safety-lifecycle` and `distribution` need **no accessibility
either** — pure filesystem snapshots and HTTP fetches. **Start there.**

`menu-honesty` is the single highest-value row in the file: it is the mechanical
detector for the entire C1–C9 class where controls silently refuse. Menu-bar
items are more likely to be real registered actions than in-pane GPUI controls;
establish empirically whether the menu bar is exposed natively (AppKit) or
through GPUI, because that determines whether the row is cheap or hard.

**An unresolved question worth answering early:** whether Codex / Claude Code /
Grok can be authenticated in *isolated* homes using company API keys and
base-URL overrides (`PROBE_OPENAI_API_KEY`, `OPENROUTER_API_KEY` which serves
Anthropic and xAI models, or our own gateway via `OPENAGENTS_AGENT_TOKEN`)
rather than an interactive subscription login. If yes, five more rows become
observable without a human. That investigation was in flight and its last
reported finding was: *"The defect is duplicated at two more sites — that's why
fixing one didn't help."* Context for that finding is lost; re-derive it.

---

## 4. What this session actually learned

This is the part most worth carrying forward. It generalizes beyond Omega.

### 4.1 "Tested but uncalled" — ten instances in one session

Not "unused code." The sharper contradiction: **a test proves the behavior is
real and intended, and no production path can reach it.** That inversion is why
all ten shipped green — the test is the only caller, so coverage, types, and
lint all agree the code is fine.

1. The delegate tool's description was **empty** — `AgentTool::description`
   reads the input schema, and a hand-written `json_schema!` carries no doc
   comment. No word of delegation guidance reached any model for weeks.
2. **Every hang trace ever written was empty** — deleting `miniprofiler_ui`
   orphaned the only caller of `set_trace_enabled`, and the writer only wrote
   the file when tracing was off.
3. The sealed-visual proof lane built `-p zed`, a package the crate rename had
   deleted, so it died before its first scene: no red baseline, no receipt.
4. `removeParticipant` — zero production callers.
5. Self-provision hooks were never passed to `makeOmegaNostrSessionService`, so
   the kill switch was unreachable and the whole mechanism was dead code.
6. The spend ceiling **always read 0** (see §2.1).
7. `duplicate_participant_refused` had no production code path.
8. `mint-livekit-acceptance-bearer.ts` emitted the wrong ref shape, so every
   bearer it minted was refused 403 — **and its tests asserted the same wrong
   shape**, so the suite was green.
9. `gate-observation-cli.ts` had never once been invokable by its documented
   command (pnpm forwards `--`), which is why the receipts directory did not
   exist.
10. `removeSarahLiveKitRoomMember` has no non-test caller; and nothing ever set
    `owner_joined_at`, so no room binding ever reached `active`.

The guard now enforcing this baselines **1,768** pre-existing findings (1,745
exported-value, 23 interface-member) into a dated `inheritedDebt` ledger that
can only shrink. `--prune` can only *remove* — otherwise any agent could launder
a fresh defect into the baseline with one command. Two named live instances
remain unfixed: `removeSarahLiveKitRoomMember` and
`SarahLiveKitRoomAuthorityStore.removeParticipant`.

### 4.2 Written blockers were wrong — in both directions

Three times, our own records misstated what was possible:

- `kubectl auth can-i` returned **`yes`** for `pods/log` while the real call was
  **Forbidden**.
- A receipt recorded `pods/exec` as *"deliberately not granted"*; running the
  runbook command verbatim showed `pods/exec` **and** `pods/portforward` both
  work and the gauge reads live.
- A lane recorded the NIP-29 community admin key as *"absent from Secret Manager
  with no signer."* It was never lost —
  `openagents-nostr-relay-private-key` derives to
  `e841147f262799821bbaa2930fcca982a575458f0e043e064a26ed8aba2046ed`, exactly
  the pubkey the deployed config lists as group admin. What actually blocked
  writes was the relay demanding **NIP-42 AUTH** and answering `auth-required`,
  which reads like a missing permission.

**Rule: neither the permission API nor our own written blockers are evidence on
this infrastructure. Only the real call is.** If a scope is blocked on a written
claim about a missing permission rather than an attempted call, re-test it.

### 4.3 The dominant failure signature: things that look healthy

Nearly every defect this session was invisible from inside the tests:

- a spend ceiling reading zero behind an always-false `typeof` guard,
- a privacy scan reporting clean because canaries were **never injected**,
- a per-minute sweep running green while structurally unable to close a loop it
  opens,
- four adapters dropping reasoning tokens — with a **captured payload already in
  the repo** (`passthrough-adapter.test.ts`, `reasoning_tokens: 92`) documenting
  the drop while asserting only `completionTokens === 96`,
- a receipt field hard-coded `true`,
- a gate whose tests passed because they exercised the handler and never its
  caller.

**Every one surfaced only by running the real thing and reading what came
back.**

### 4.4 Prove the check can fail

Adopted as standard mid-session, and it repeatedly paid:

- The participant-join guard was proven by deleting its wiring and watching it
  exit 1.
- The uncalled-symbol guard was proven against four historical defects at their
  broken commit and clean at their fix.
- **The CLI-invocability guard's first draft did not flag the defect it was
  written for — twice.** Its rejection pattern matched only `"unknown argument"`
  and missed `"unsupported or incomplete argument"`; and it read
  `next.startsWith("--")` as evidence a token was handled, which is the line
  proving the opposite. Both found only by running it against the pre-fix commit
  and watching it pass.
- The reasoning-token removal test reproduced the exact production symptom
  (`expected +0 to be 180`).

**A check that cannot fail on known-bad input is the disease, not the cure.**

### 4.5 The near-miss worth institutionalizing

The coordinating agent instructed a lane to *"fix the sweep so its selected set
matches the index's blocking set."* That is the money-integrity regression in
§1.2, stated verbatim. It did not land for exactly one reason: **a different
lane independently verified the claim instead of accepting it, and pushed
back.** Not a test, not a gate, not a review process.

That is luck, and it should not be load-bearing twice. The mitigation being
landed when the session stopped was a regression test asserting the sweep never
terminates `accounting_uncertain`, plus a comment at the producer site. **Verify
it exists.**

---

## 5. Decisions reserved for the owner

Do not resolve these unilaterally.

1. **`accounting_uncertain` repair** (§1.2). Both honest fixes change settlement
   invariants and need `INVARIANTS.md` treatment.
2. **Clearing the live stuck row** — needs an OpenAI usage export plus owner
   authority. Leave uncertain until then.
3. **Whether the public tokens-served counter should include reasoning tokens.**
   `publicTokensServedFromRow` is `input + output`. Under Google's shape
   reasoning is a *sibling* of output, so a 198-token Gemini turn credits **18**
   to the public counter — while the Pylon/Codex path deliberately counts
   reasoning into the public total. Same work, counted two ways depending on
   provider. Fixing it makes the number larger and more truthful, but it changes
   what a **published** number means. OpenAI-shape lanes are unaffected
   (reasoning is already inside output).
4. **Test-identity cleanup.** A lane deliberately declined an instruction to
   delete the third cohort row and the NIP-29 admissions, because the receipt
   asserts all three are admitted and deleting them would make the receipt
   false. `d4bb198b…` is the durable admin identity now in production config.
   Standing cost: a bounded 200,000 msat credit. Revocable on request.

---

## 6. Known-red baseline (do not claim these as your own breakage)

`pnpm run check` short-circuits on a pre-existing `pnpm-workspace.yaml` `fmt`
red. Run components individually while iterating.

Other pre-existing reds: `check-fast-follow`, `check-sol-docs`,
`assurance-spec/distribution`, and five process-spawning suites that pass in
isolation but flake in the parallel sweep — `bun-api-perimeter-scan`,
`effect-authority-boundary-scan`, `forge-omega-dogfood`,
`validate-khala-sync-*`, `ide13-real-remote-cohort`.

**Do not run `pnpm run fmt`.** It reformats ~10,533 files into a style the repo
does not use, and `fmt:check` only covers `package.json`, `pnpm-workspace.yaml`,
and `vite.config.ts` — TS sources are not format-gated at all. It cost one lane
its work this session.

---

## 7. Operational cautions

- **Do not drive the owner's GUI.** This session dispatched lanes that drove the
  desktop through macOS Accessibility while the owner was using the machine,
  stealing cursor and focus. That is what ended the session. Automating the
  human-gated rows is correct; doing it on a live owner machine is not. Use a
  separate display, a dedicated host, or a headless path — or ask first.
- **Scope process cleanup to your own PIDs.** A bare `pkill -f omega-effectd`
  killed an unrelated instance's sidecar (pid 69771); its app survived but never
  respawned one.
- **Never touch the owner's credential state.** Not `~/.codex`, not `~/.claude`.
  `codex login` clears `~/.codex/auth.json` at flow-start and destroys a live
  session. Isolated homes only (`CODEX_HOME=$(mktemp -d)`).
- **Deploy queue is contended.** One hypothesis per deploy. Verify
  `NEW_ADMISSIONS_ENABLED=true` and
  `PROVIDER_DISCONNECT_ACCEPTANCE_ENABLED=false` across every deploy. The latter
  was never armed this session, so observing `true` is a genuine anomaly.
- **The canonical checkout is dirty.** `/Users/christopherdavid/work/openagents`
  is detached at `169b97c6c9` with another agent's uncommitted
  `packages/harness-environment/src/run-openagents-cloud-harness.ts`. It was
  deliberately left untouched under multi-agent hygiene. **The CLAUDE.md
  primary-`main` reconciliation gate is therefore NOT satisfied.** Reconcile
  only after that file lands with its owner.

---

## 8. Suggested pickup order

1. **Fix §1.1** — community rooms are broken in production right now. Highest
   urgency, well-understood, small.
2. **Land the §1.2 regression guard and the §1.3 alarm** if they did not land.
   Both are additive and safe.
3. **Escalate the §5 decisions** so they are unblocked while other work runs.
4. **Land `update-safety-lifecycle` and `distribution`** — real gate rows,
   no accessibility, no credentials.
5. **Answer the §3.3 auth-matrix question.** If company API keys authenticate
   isolated agent homes, five rows become observable without a human.
6. **Redo §3.1** — registered actions. This is the critical path for four
   issues, and it is a product fix, not just a test fix.
7. **Redo §3.2** — make the two receipts measure what they claim, and produce
   the asserted-but-underived field inventory.
8. **Cut rc30** from omega `849cdcf2b5`+, notarize, promote `/download`, and
   bind all gate evidence to its digest **once** rather than re-running against
   two candidates.
9. **Footage (#189)** last among capture work — its acceptance is *"the footage
   is evidence, not a mock,"* so it must follow the rows that prove the
   capabilities exist.

Do not fabricate a gate row. The report's law is that no row is ever
fabricated, and an honest `blocked` row is worth more than a fabricated pass.
That principle is the only reason the report is worth anything.
