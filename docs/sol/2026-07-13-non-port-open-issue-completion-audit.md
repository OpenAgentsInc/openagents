# Non-PORT open issue completion audit — 2026-07-13

- Class: current-status
- Date: 2026-07-13
- Status: factual completion audit; no completion declaration
- Dispatch: no; live issues and [`MASTER_ROADMAP.md`](./MASTER_ROADMAP.md)
  remain authoritative
- Owner: Sol reliable-client program
- Snapshot: `origin/main` at `b0e579b0bba6ffbaa716f7ec42885a482d3c7ca8`
  plus a live GitHub issue refresh at 2026-07-13T12:34Z
- Scope: the five open `roadmap:sol` issues that are not PORT leaves:
  [#8566](https://github.com/OpenAgentsInc/openagents/issues/8566),
  [#8574](https://github.com/OpenAgentsInc/openagents/issues/8574),
  [#8597](https://github.com/OpenAgentsInc/openagents/issues/8597),
  [#8707](https://github.com/OpenAgentsInc/openagents/issues/8707), and
  [#8741](https://github.com/OpenAgentsInc/openagents/issues/8741)

This is an additive audit, not a substitute issue ledger. It records what is
already proved, what still has to happen, which closure claims are currently
invalid, and which issue/source text must be reconciled at closure. It does not
claim owner acceptance, provider entitlement, physical-device evidence, or
PORT work that does not exist.

## Set and dependency truth

The live `roadmap:sol` query returned eleven open product issues. Six are the
explicitly excluded PORT leaves #8748–#8753. The other five are exactly the
issues in this audit. CUT-01 through CUT-26 (#8681–#8706) are closed, as are
the CUT-27 prerequisite parents #8640, #8676, #8677, and #8678. That leaves
this closure graph:

| Issue | Truth at this snapshot | Honest next transition |
| --- | --- | --- |
| [#8741 AUDIO-8](https://github.com/OpenAgentsInc/openagents/issues/8741) | Implementation/deploy proof is present; owner microphone acceptance is absent | Complete the bounded owner journey, publish the final ref-only receipt, then close |
| [#8707 CUT-27](https://github.com/OpenAgentsInc/openagents/issues/8707) | All implementation leaves and prerequisite parents are closed; literal Claude and per-task cross-device acceptance remain absent | Satisfy the remaining installed Claude and device gates, publish the cutover bundle/docs, then close |
| [#8574 Desktop](https://github.com/OpenAgentsInc/openagents/issues/8574) | D0–D6 implementation is represented by closed CUT leaves; its ordinary local-coding acceptance is still delegated to #8707 | Close only after #8707 and a current body/docs reconciliation |
| [#8597 mobile](https://github.com/OpenAgentsInc/openagents/issues/8597) | Local-cutover implementation is complete except #8707; the issue's literal remote-first exit is PORT-owned | Reconcile the local rung after #8707, but keep the track open for PORT-03–PORT-08 |
| [#8566 program](https://github.com/OpenAgentsInc/openagents/issues/8566) | Program parent only; no independent leaf is hidden here; its body explicitly excludes Sarah/persona/A/V/presentation work from the exit | Close last, after client tracks, PORT/R7 dogfood, reconciliation, and legacy retirement; AUDIO-8 is not a program-close blocker |

Therefore there are two immediately actionable non-PORT finish lanes: AUDIO-8
and CUT-27. They are independent. #8574 is the dependent reconciliation/
closure step for CUT-27 only; AUDIO-8 does not gate #8574 or #8566. #8597 and
#8566 cannot honestly close while the excluded PORT exits remain open.

## #8741 AUDIO-8

### Already complete

- AUDIO-2 through AUDIO-7 (#8735–#8740) are closed; the implementation epic
  #8733 is closed.
- Production fixes are on `main`: `f39ce24a28` moves Google endpointing to
  `SHORT`, fences admitted-turn capture, and bounds renderer lifecycle
  notifications. The deployed audio revision reported by the live issue is
  `openagents-audio-staging-00013-mks` at 100% traffic.
- The issue records 110 focused Desktop tests (611 expectations), a production
  Desktop bundle, nine audio server retention/session/media tests, and a
  successful Cloud Run bundle/deploy.
- `4860380f54` prevents transport records from becoming conversation titles.
  Closed #8744 separately supplies durable accepted-turn process-restart
  recovery. Neither fact is a microphone acceptance receipt.
- The owner action is already public in the workspace `NEEDS_OWNER.md` at
  `AtlantisPleb/workspace` commit `cac3f6659`; no additional implementation
  blocker is presently identified.

### Exact remaining gate

1. In the normal installed `oa` launch, the owner speaks a full sentence and
   confirms that it becomes one final visible message, without fragment
   replacement or duplication.
2. During assistant playback, the owner barges in and confirms that playback
   stops and the new utterance is admitted once.
3. While listening, the owner opens or hovers Details and the sidebar and
   confirms that neither flickers nor collapses.
4. An agent records a public-safe, ref-only machine receipt naming the exact
   Desktop artifact/build, gateway and service revisions, Google models,
   latency/loss/storage outcomes, exercised commands/faults, privacy scan, and
   residuals. It must not include raw audio, transcript content, credentials,
   or private paths.
5. Reconcile the issue's literal requirement for “one owner-reviewed
   recording.” A verbal/UI pass alone must not silently rewrite that criterion:
   either cite privacy-safe recording provenance showing that the owner
   reviewed the journey, or obtain an explicit issue-criterion amendment from
   the owner and record it before closure.
6. Close #8741 only after the owner result and final receipt agree. A failure
   reopens the exact failing implementation slice instead of being waived.

An independent active claim on #8741 at this snapshot is revalidating deployed
services and automatable checks. That work can strengthen step 4 but cannot
manufacture steps 1–3 or reinterpret step 5.

## #8707 CUT-27

### Already complete

- CUT-01 through CUT-26 are closed. Required prerequisite parents #8640,
  #8676, #8677, and #8678 are also closed.
- CUT-26 supplied the signed/notarized/stapled RC5 distribution, update,
  rollback, reinstall, diagnostics, and hardened Electron receipt described in
  [`2026-07-12-cut-26-rc5-installed-artifact-closure.md`](./2026-07-12-cut-26-rc5-installed-artifact-closure.md).
- The latest live issue evidence says the counted named-Codex repository task,
  in-app typed Git review, durable terminal result, and restart continuity are
  proved.
- Packaged Claude defects are fixed. `33add4136e` / `1f597e4395` package and
  sign the pinned darwin-arm64 Claude SDK runtime; `f8035fd0da` resolves its
  executable outside `app.asar`. The signed app launches that exact packaged
  runtime, and the issue records 48 focused tests, Desktop typecheck, strict
  deep code-sign verification, executable arm64 Mach-O verification, and a
  matching `HQWSG26L43` TeamIdentifier.
- The two attempted named Claude accounts did not supply acceptance evidence.
  `claude-pylon-2` and `claude-pylon-3` both reached provider initialization
  and then received Anthropic's organization-policy denial. Those turns are
  correctly not counted.

### Exact remaining gate

1. Enable Claude Code subscription access for one exact named local account,
   or authorize a separately named API-key-backed Claude account without
   weakening named-account isolation or custody.
2. On a current clean installed Desktop build, complete a fresh non-trivial
   repository task with that named Claude account. The same counted journey
   must include project/repository/session/thread/account/model identity,
   history and agent inspection, composer file/diff context, a structured
   question or approval, provider-authored edit, real test or preview, in-app
   Git diff review, a durable terminal result, and process-restart continuity.
3. Satisfy the issue's literal cross-device criterion for **each** counted
   Codex and Claude task. Physical iOS and an Android emulator must reconnect
   to the same stable refs/timeline/agent graph, continue a turn, handle one
   attention/control item, survive one forced app or network interruption, and
   converge with Desktop. The existing #8676/#8677 receipts are prerequisite
   evidence; they do not replace per-counted-task mapping where that mapping is
   absent. If the existing counted Codex task cannot be tied to every row,
   rerun or extend that task rather than infer the receipt.
4. Publish one public-safe CUT-27 bundle that cites the accepted #8640/#8676/
   #8677/#8678 receipts and contains exact artifact/commit/device/OS/app refs,
   timestamps, commands, failures/recovery, the loss and exception register,
   accessibility/privacy/security results, artifact provenance, rollback
   result, and the explicit boundary excluding remote workrooms, host movement,
   managed-provider breadth, and voice.
5. Update the product, Desktop runbook, and Sol roadmap/current issue text so
   OpenAgents Desktop is the default ordinary local coding surface and direct
   Codex/Claude Code UI fallback is unsupported only for the scope actually
   proved.
6. Close #8707 only when all literal rows above have direct evidence and no
   unresolved P0 defect is waived.

An independent active claim on #8707 at this snapshot is rechecking entitlement
and recovering/re-running evidence. Its own claim correctly forbids closure
unless every literal criterion passes.

## #8574 Desktop track

### Already complete

The implementation work represented by D0–D6 is not a new hidden queue. The
closed CUT leaves cover the truthful baseline, Effect service/lifecycle
boundaries, project/session navigation, commands, workspace/files/editor/Git/
PTY, named runtimes, settings/accessibility/diagnostics, Fleet surfaces, and
signed distribution. AUDIO-8 is a parallel voice proof and the master roadmap
explicitly says it does not block CUT-27. The #8566 parent body also explicitly
excludes Sarah/persona/A/V/presentation work from its exit, so AUDIO-8 does not
become a hidden Desktop-track or program-parent close gate merely because it is
attached to the same program area.

### Exact remaining gate

1. Finish and close #8707.
2. Reconcile the live #8574 body before closure. It still says CUT-05 is the
   next leaf and still lists physical auth, live provider streaming, workbench,
   Fleet, distribution, and legacy retirement as “not yet claimed,” despite
   the closed CUT graph and later receipts. Replace those historical statements
   with the exact accepted rung and links; do not merely delete the caveats.
3. Check the issue's Exit paragraph row-by-row against the final CUT-27 bundle
   and the closed CUT receipts. Any row lacking a receipt becomes a bounded
   defect/acceptance leaf; it is not waived in prose.
4. Close #8574 for the proved ordinary local-coding/Desktop track. Do not use
   that closure to claim PORT host movement or the still-open mobile remote
   exit.

## #8597 mobile track

### Already complete

The live body correctly says CUT-01 through CUT-26 are closed and #8707 owns
the remaining installed local Codex/Claude acceptance. It also correctly
assigns remote workrooms, host movement, managed providers, any-host mobile
control, and portable-session voice to PORT-00–PORT-08. PORT-00 through PORT-02
are closed in the master roadmap; PORT-03 through PORT-08 are still open.

### Exact remaining gate

1. Finish #8707's physical-iOS and Android-emulator per-task continuation rows,
   then update #8597 to mark the bounded local-cutover rung complete.
2. Keep #8597 open. Its Exit still requires a real remote workroom, compact
   Thread/Files/Changes/Terminal/Preview/Artifacts experience, safe writeback,
   durable receipt, graph-wide host move/failback, any-host controls,
   session-neutral voice, fault/update survival, signed dogfood, and legacy
   mobile retirement. Those are exactly the excluded PORT-03–PORT-08 outcomes.
3. After the PORT program finishes, reconcile every stable ref and fault row in
   the #8597 Exit against its receipts, retire the deprecated mobile ship path,
   and only then close #8597.

There is no additional unfiled non-PORT mobile implementation task in this
audit. Closing #8597 after CUT-27 but before PORT completion would be false.

## #8566 program parent

### Already complete

#8566 is the sole program parent and already delegates concrete work to its
client, CUT, AUDIO, and PORT children. The foundation, local implementation
graph, simultaneous-provider substrate, first managed workroom, bounded hybrid
routing, and PORT-00–PORT-02 authority/broker layers are represented by closed
issues and current receipts. No implementation should be invented directly in
the parent to make its checklist look green.

### Exact remaining gate

1. Complete CUT-27, then close #8574 as described above. Track and close
   AUDIO-8 independently; it is not a #8566 exit criterion or blocker.
2. Keep #8566 open while #8597 and PORT-03–PORT-08 remain open. The parent
   requires the signed R7 dogfood journey, cross-host convergence/failback,
   legacy product/install/release retirement, and final issue/docs agreement.
3. Reconcile the #8566 body at final close. It names Master Revision 101 while
   the checked-in authority at this snapshot is Revision 105. Replace stale
   revision/queue language with links to the final receipts and exact child
   dispositions.
4. Close #8566 last, only after all child exits and the canonical roadmap/live
   projection agree. Parent closure is reconciliation, not an alternate proof
   route.

## Source reconciliation required at handoff

| Source | Current problem | Required correction |
| --- | --- | --- |
| #8566 body | Cites Master Revision 101 | Reconcile to the final current roadmap revision and child states at program closure |
| #8574 body | Calls closed CUT-05 “next” and preserves a broad stale “not yet claimed” list | Replace with current closed CUT receipts and the exact final CUT-27 acceptance boundary before closing |
| #8597 body | Correct on local/PORT split, but local acceptance remains open | Mark the bounded local rung complete only after #8707; preserve PORT residuals |
| #8707 body | Criteria remain valid; comments carry newer packaged-runtime/account evidence | Consolidate those comments into the final evidence bundle and product/runbook/roadmap declaration |
| #8741 body | Requires an owner-reviewed recording while the owner request currently asks only for a bounded microphone/UI report | Cite real review provenance or explicitly amend the criterion before closing; do not silently reinterpret it |
| [`2026-07-12-cut27-cutover-readiness-audit.md`](./2026-07-12-cut27-cutover-readiness-audit.md) | Intentionally preserves superseded OPEN sections | Keep it historical/non-dispatch; use the final CUT-27 bundle and live issue for closure truth |

## Finish order

1. In the independent audio lane, complete the AUDIO-8 owner journey and final
   evidence, then close #8741. This may happen before or after the cutover lane
   and does not gate #8566.
2. In the cutover lane, unlock one named Claude account; complete Claude plus literal per-task iOS/
   Android-emulator CUT-27 acceptance; publish the consolidated evidence and
   default-surface docs; close #8707.
3. Reconcile and close #8574.
4. Mark only the local rung complete on #8597; leave it open for the excluded
   PORT outcomes.
5. After PORT-03–PORT-08, signed R7 dogfood, and legacy retirement, close
   #8597 and then #8566.

Any later live issue transition supersedes this snapshot. A closed state is
valid only when the corresponding direct evidence exists; neither this audit
nor an issue-body rewrite supplies that evidence.
