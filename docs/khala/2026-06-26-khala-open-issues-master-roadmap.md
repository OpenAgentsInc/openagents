# Khala Open-Issues Master Roadmap — One Solve Sequence

> Status: **internal execution roadmap, 2026-06-26.** Direction-setting, not public
> claim copy and not a product promise. It orders the active
> `OpenAgentsInc/openagents` Khala issues plus their newly closed dependencies into a
> single dependency-aware sequence agents should work top-to-bottom. It flips no
> promise state and ships no code itself.

## How to use this

Work the phases in order. Within a phase, issues marked **‖ parallel** can run
concurrently; issues marked **→ after** must wait for their named dependency. Two GitHub
master trackers already exist and this roadmap threads through both:

- **#6316** — Maximize GLM-5.2-REAP usage in Khala (serving: Phases 0–3)
- **#6303** — Khala GTM push (demand: Phases 4–5)

The ordering principle: **stop the live breakage → make serving reliable → maximize
throughput → automate it → prove quality → drive adoption.** You cannot honestly grow
demand (Phase 4–5) on a coding surface that is hard-down (Phase 0), so adoption sequences
last even though it is the business goal.

The hard invariant throughout (#6318): **real external requests always win** — internal
stress/benchmark load is best-effort, preemptible, instantly-yielding, and tagged
`internal_stress`. It must still be counted in the public Khala token counter
when it is Khala-orchestrated; the tag exists for routing, trace filtering, and
external-wins admission, not for hiding usage.

## Operating model: autonomous parallel burndown + trace-driven triage (added 2026-06-26)

The phases below are now driven by a **continuous parallel loop**, not one-off
batches. Four cross-cutting tracks run *alongside* the phased sequence and feed it:

- **#6355 — Parallel backlog-burndown loop (operator runner).** Closed by the
  `pylon khala burndown` operator command: dry-run plans issue/account slots from
  the roadmap or an explicit list, `--execute` dispatches `codex_agent_task`
  requests, runs no-spend assignments, and verifies exact proof totals before
  checking public token-counter movement and reporting `operator_review_required`
  merge policy. Codex is the master coding agent for now; #6321 (Artanis) is
  the eventual in-product version. The
  presence/busy-load steering prerequisite **#6354** is closed; keep the
  already-closed steering fixes (#6331-#6341, #6349-#6350) in the loop guard.
- **#6356 — Trace review.** Closed by
  `GET /api/operator/khala/trace-review`: systematically reviews ATIF trace
  refs, exact token rows, and Pylon/Codex raw-event metadata from our own runs
  and external testers; reports failure modes, model mix, notable trace refs,
  recurring intents, and triage items for the backlog. Raw trajectories and raw
  SDK payloads stay owner-only.
- **#6357 — Unsupported-request running list.** Capture what testers try that Khala
  **can't do yet** (Forum-first + a tracking doc), triage into bug /
  missing-capability / won't-do, and convert real gaps into issues that thread back
  into this roadmap. Fed by #6356.
- **#6358 — Counter health.** Guarantee the public token counter increments
  correctly + continuously from real closeouts: the heartbeat is internal demand
  but still counts in the all-demand public scalar, the corpus/analytics split is
  what keeps it distinguishable, hold the monotonic / no-double-count invariant
  (`8d66f6be09`), optional labeled in-flight estimate from streamed chunks.

Two standing goals the loop optimizes for **every iteration**:
1. **Maximize the public Khala token counter** — keep real Khala-orchestrated
   closeouts landing (each verified to exact `token_usage_events`) and keep the
   counter honestly incrementing (#6358). Counter movement alone is never proof;
   attribute via assignment `task_ref` / `pylon khala proof`.
2. **Broaden the inference base / maximize GLM usage** — most served tokens today
   are `pylon_codex` (~70%); push the GLM serving track (#6316 umbrella: #6320
   throughput, #6311 durable fleet, #6323 full-model pilot, #6253 quality) so GLM
   carries a growing share, with Fireworks / OpenRouter / GPT-OSS as fallback, not
   the default lane.

Each loop iteration picks work in this priority: Phase-0/1 serving blockers ->
throughput (#6320) -> scheduler (#6318) -> stress (#6317) -> benchmark (#6312); in
parallel, demand/quality (#6253, #6307, #6308, #6309) and the cross-cutting tracks
(#6356/#6357). #6356 + #6357 continuously feed *new* issues back into the sequence.

## Artanis: autonomous owner of this loop (epic #6359)

The end state is **Artanis** — the approval-gated scheduled tick
(`artanis-scheduled-runner.ts`; it already monitors Khala readiness read-only and
no-spend) — **owning this whole loop autonomously**, not a human launching batches.
Money + destructive actions stay owner-gated via `artanis-approval-gates`;
everything else Artanis drives on its own tick.

Artanis's mandate (#6359):
- **Unblock the people trying to use Khala.** Act on the unsupported-request list
  (#6357) and the Khala CLI feedback (#6360) — convert blockers into fixes/issues
  fast.
- **Keep inference solid.** Own the #6316 serving track (#6320/#6311/#6323/#6318/
  #6317/#6312). **Once the current issue set is drained, pull the next optimization
  ideas from `docs/inference/inference-engineering-book/`** and open issues from
  them — continuous improvement, not a one-shot.
- **Drive the parallel burndown loop (#6355)** — dispatch / verify / merge across
  connected Codex accounts, keep the counter honestly incrementing.
- **Read + act on Khala CLI feedback (#6360).** The CLI `/feedback` command writes
  to the `khala_feedback` table (`POST /api/khala/feedback`; operator read
  `GET /api/operator/khala/feedback`). Artanis ingests it on the tick and triages:
  style/behavior feedback (e.g. "too wordy, prefer more conversational") → an
  owner-reviewable Khala response-style change; capability gaps → #6357 → issues;
  bugs → strict-bug issues.
- **Trace review (#6356) + counter health (#6358) + the fleet-overseer loop (#6321).**

Net-new for full autonomy: a tick action that selects + dispatches the #6355 work
within bounded authority (read / dispatch own-capacity Codex / verify / merge
non-spend code / open issues), escalating only spend/destructive via the approval
gates; tick read access to the open issues + #6356/#6357/#6358 + the feedback
store; and the `inference-engineering-book` consultation as the recurring
"what's next" source after the issue set drains. #6359 subsumes #6321.

### Talking to Artanis (the operator channel) — #6363

`I need to speak to Artanis` in the mobile app currently returns **Khala
collective-intelligence roleplay** (the public `inference/khala-identity.ts`
prompt treats "Artanis" as the StarCraft Hierarch — "Hierarch Artanis, leader of
the Protoss... what message do you wish to send to the Hierarch?"), not the real
operator agent. The owner must be able to converse with the **actual Artanis** and
have it know itself. Required (#6363, under #6359):
- **Owner-authenticated Artanis channel.** A "Talk to Artanis" mode/route (mobile +
  CLI) that routes the OWNER to a grounded Artanis operator persona, bypassing the
  public Khala identity. No roleplay; first person as the operator agent.
- **Situational awareness.** Artanis answers "what are you doing?" from live state:
  recent actions (its tick log, recent commits, Pylon-Codex assignments, issues
  opened/closed), current goals (this roadmap + #6359/#6316/#6303), and ongoing
  operations (active assignments, deploys, fleet readiness, public counter).
- **Persistent owner-interaction memory.** Artanis remembers prior owner
  conversations, decisions, and stated preferences across sessions (owner-scoped
  memory store the channel reads + writes).
- **Persona separation.** The public Khala identity stays collective-intelligence;
  the Artanis operator persona is distinct + owner-only; spend/destructive still
  gated by `artanis-approval-gates`.

This is the human-facing front of the #6359 mandate: the owner steers the
autonomous loop by talking to the agent that runs it, and Artanis answers grounded
in what it has actually been doing — not training-data roleplay.

## Current status snapshot

Refreshed from GitHub issue state, `origin/main`, live counter/proof reads, and
the local Pylon state on **2026-06-27 ~13:00Z**. This table is the
operator view of what remains, not a public product claim.

| Issue | State | Current status / next action |
| --- | --- | --- |
| #6310 | **Closed** | P0 OpenCode/tool-calling outage is no longer open. Keep its repro in regression coverage and do not let demand docs outrun the actual tool-call path. |
| #6323 | **Open** | Decision artifact for the full `nvidia/GLM-5.2-NVFP4` single-host pilot landed. The delegated Pylon/Codex retry (`assignment.public.khala_coding.chatcmpl_6a9906e622ad43caa1c9cc3fb2f20d00`, 4,371,243 exact tokens) has now been reviewed, tightened for public-safety, merged in `c7a86d7d06`, and deployed via `deploy:safe` as Worker `12066869-4f81-4b32-ba41-ba3c50b07595`. A later supervised Pylon/Codex pair (`assignment.public.khala_coding.chatcmpl_6bb60a39d229481e9837bd08ce25f8a6`, 1,533,834 exact tokens; `assignment.public.khala_coding.chatcmpl_dac3ccd7c09f4591b42f2528ef43aff6`, 975,671 exact tokens) was reviewed and folded into the current integration patch to add typed public evidence-ref audits, fail-closed observation ref redaction, and stricter tool-call-name validation. Fresh supervised Pylon/Codex work landed `ac94c1afa1` from `assignment.public.khala_coding.chatcmpl_865394ecde434183b28654df6c306617` on `codex-2` (`1,971,655` exact tokens; `64` owner-only ATIF traces; `104` raw SDK events / `635,252` bytes), adding the fail-closed public NVFP4 pilot gate summary. Deployed through `deploy:safe` as Worker `8c03f302-8186-468d-a2b8-cd8d3489ae0b`. This bounded checkout now adds the operator-safe `pilot:glm-nvfp4` evidence-retention path and the later Pylon/Codex run (`assignment.public.khala_coding.chatcmpl_f17c7a883dbe4c26abf8380b8ad512be`, 3,649,823 exact tokens; 80 owner-only ATIF traces; 132 raw SDK events / 1,503,898 bytes) tightens the fixture executor so `toolCallsSucceeded` requires a clean second tool-result turn, not just first-turn `tool_calls` emission. The latest delegated hardening slice (`assignment.public.khala_coding.chatcmpl_44a4a0608a1049d59b41fae44b840433`, 2,738,605 exact tokens; 67 owner-only ATIF traces; 109 raw SDK events / 2,623,114 bytes) adds the required public boot/load evidence ref so the isolated owner gate cannot pass unless the 8x host boot/load proof is present and public-safe. A 2026-06-27T07:00Z delegated audit (`assignment.public.khala_coding.chatcmpl_1702ef0731874c72934c1f95068f8c47`, 810,643 exact tokens; 32 owner-only ATIF traces; 52 raw SDK events / 1,252,108 bytes) made `0` edits and passed the focused verifier, confirming the remaining blocker is not a local code gap. The current local environment has no `KHALA_GLM_NVFP4_PILOT_ARM`, endpoint URL, endpoint ref, owner approval ref, or `KHALA_GLM_NVFP4_BOOT_LOAD_EVIDENCE_REF`, so any local run remains `no_go`. Still open: owner runs the exact armed command in `docs/inference/2026-06-26-nvidia-glm-5.2-nvfp4-evaluation.md`, then records measured boot/load, tool-call, quality, max-context, and tok-s results. |
| #6319 | **Closed** | Reliability hardening/fallback-chain repair is closed. Treat empty responses and dead fallback lanes as regression risks in later serving work. |
| #6313 | **Closed** | Real OpenRouter fallback lane is closed. It is now a dependency assumption for further reliability and benchmark runs. |
| #6311 | **Open** | Partial readiness/watchdog projection work landed, and the Khala -> Pylon -> Codex diagnostic slices are now on `main`: `36ee76689c` (`assignment.public.khala_coding.chatcmpl_b5a0d831027a4c779b1105be73217f29`, 6,415,202 exact tokens), `7f94c0556a` (`assignment.public.khala_coding.chatcmpl_c7358576d7464620ae3da33ed2f473a0`, 7,393,396 exact tokens), `5b11c6eaf5` (`assignment.public.khala_coding.chatcmpl_a5dc55d153ca4642b2836e0fa80005df`, 4,500,934 exact tokens), `d373eaee69` (`assignment.public.khala_coding.chatcmpl_ff2b38153a9c457d8769bb6fc44ad9a5`, 3,653,854 exact tokens), and `88f60cb924` (`assignment.public.khala_coding.chatcmpl_b2051745aee042d08f6d5348282215dc`, 2,905,828 exact tokens). `88f60cb924` is deployed as Worker `ba4ed1d7-f81a-4c82-b63c-2042b6bb4ad3` and adds the fail-closed typed acceptance projection for all-replica watchdog, capacity-floor owner decision, multi-region auto-replace, and quota tracking. The current supervised integration folded in three more accepted Pylon/Codex slices (`assignment.public.khala_coding.chatcmpl_efb67743f96f44b9ad0078763e224fe6`, 1,571,076 exact tokens; `assignment.public.khala_coding.chatcmpl_bfe667383900474aa93652ad09e5d2e8`, 3,136,839 exact tokens; `assignment.public.khala_coding.chatcmpl_8132762906104faaa5fa9a68e7920bec`, 1,209,821 exact tokens) for typed quota state, capacity-floor evidence, all-replica watchdog readability, and multi-region replacement/reserve/prebake evidence. Fresh supervised Pylon/Codex work landed `bb386d8173` from `assignment.public.khala_coding.chatcmpl_f07a0d9b3b684abc80284aa0c818ca8c` on `codex-3` (`963,220` exact tokens; `55` owner-only ATIF traces; `92` raw SDK events / `173,848` bytes), adding the operator-facing readiness readout. Deployed through `deploy:safe` as Worker `8c03f302-8186-468d-a2b8-cd8d3489ae0b`. This follow-up adds the headless `glm-fleet:durability` operator bundle and tightens acceptance so all-replica watchdog requires a public-safe forced Spot STOP recovery ref (`HYDRALISK_GLM_52_REAP_504B_FORCED_STOP_RECOVERY_REFS`). Current live `/v1/gateway/glm-fleet/readiness` still reports `status:"degraded"` and `acceptance.status:"blocked"`, but serving capacity has materially recovered to `readyReplicaCount:8`, `reclaimedReplicaCount:0`, and `warmOrReadyMaxInflight:9`; the operator summary now records that capacity summary, suppresses `recover_reclaimed_replicas` at zero reclaimed replicas, and keeps #6311 open until forced-stop, capacity-floor, multi-region, and quota evidence are complete. A 2026-06-27T07:16Z delegated audit (`assignment.public.khala_coding.chatcmpl_50871eacbf1647439bc273f7b4f118f3`, 917,278 exact tokens; 38 owner-only ATIF traces; 64 private raw SDK events / 795,253 bytes) made `0` edits and passed the focused verifier, confirming the remaining gap is live durability evidence/owner decisions rather than a fresh local code hole. A 2026-06-27T08:44Z delegated audit (`assignment.public.khala_coding.chatcmpl_8280f63b4dad4180a04fd22f32b1b374`, 3,037,332 exact tokens; 72 owner-only ATIF traces; 120 private raw SDK events / 4,158,470 bytes) also made `0` edits, ran `48` commands, and passed the focused #6311 verifier; the issue remains open for actual fleet durability evidence and owner/infra decisions, not another local repo patch. |
| #6259 | **Closed** | Khala -> GLM served-worker disclosure + counter smoke is closed. |
| #6315 | **Closed** | Zero-debit receipt-ref fix for #6259 is closed. |
| #6320 | **Open** | A bounded routed slice landed in `85ca837413` and deployed as Worker `228ac0f9-c891-4ad2-b05f-0dd8894f3c86`: typed throughput-sweep metadata for `max-num-seqs`, prefix cache, chunked prefill, speculative decode, quant gates. Delegated slices landed on `main` in `a8c12aff42` (`assignment.public.khala_coding.chatcmpl_5ccffa5593b84cc09e414d3ad358b9b0`, 2,202,625 exact tokens), `03b6ffa094` (`assignment.public.khala_coding.chatcmpl_7033a7cd0fff4afaaad412e783bab29a`, 4,949,572 exact tokens), `d373eaee69` (`assignment.public.khala_coding.chatcmpl_a8737564357847eca23110d319bf4edf`, 2,656,599 exact tokens), `88f60cb924` (`assignment.public.khala_coding.chatcmpl_9a27a65fa4f5434db8715bcb1288c91d`, 5,505,826 exact tokens), and `7a73ab8d95` (`assignment.public.khala_coding.chatcmpl_ec4be93966804054a0775f1099465a8f`, 2,090,306 exact tokens): typed throughput rollout recommendation/flag selection, owner-armed rollout artifact/guardrails, fail-closed measured-lift rollout readout, and a fail-closed public-safe evidence checklist for owner arm ref, live engine config, before/after throughput, ITL, progress, and public refs. This bounded local slice adds typed operator acceptance evidence that also exposes live vLLM flags, before/after TTFT, progress status/percent, and the exact remaining checks that block #6317/#6312; Khala -> Pylon -> Codex proof: `assignment.public.khala_coding.chatcmpl_a46b5dc9bb4249df8809ac46a23948ea`, `3,672,787` exact own-capacity tokens, `83` owner-only traces, `1` owner-only raw-event closeout row. The 2026-06-27 follow-up adds typed GLM fleet/#6311 durability dependency evidence to the rollout readout, so #6317 stress and #6312 benchmark remain blocked when serving is degraded or #6311 acceptance is not complete; Khala -> Pylon -> Codex proof: `assignment.public.khala_coding.chatcmpl_514e957b47554f1c8a88b593c1992d22`, `6,128,787` exact own-capacity tokens, `115` owner-only traces, and `196` private raw SDK events / `1,179,008` bytes. The current bounded slice updates that dependency evidence for the #6311 readiness shape where serving capacity is materially recovered (`readyReplicaCount:8`, `reclaimedReplicaCount:0`, `warmOrReadyMaxInflight:9`) while public status remains degraded and durability acceptance is blocked; typed readouts now carry the serving capacity summary and remaining durability blockers, and still fail closed for rollout/stress/benchmark until #6311 acceptance is complete. A 2026-06-27T07:29Z delegated audit (`assignment.public.khala_coding.chatcmpl_7deb9e73b6e9428b9d32dc9e8f3654b8`, 1,644,352 exact tokens; 36 owner-only ATIF traces; 60 private raw SDK events / 3,501,512 bytes) made `0` edits and passed the focused verifier, confirming the remaining #6320 gap is live engine rollout plus measured throughput-lift evidence, not a fresh local code/doc hole. A 2026-06-27T08:53Z delegated audit (`assignment.public.khala_coding.chatcmpl_56e7dd24d9a248afbc0533e022d260cd`, 2,019,853 exact tokens; 46 owner-only ATIF traces; 74 private raw SDK events / 2,009,588 bytes) also made `0` edits, ran `28` commands, and passed the focused throughput verifier after the fresh #6311 proof. Current deployed Worker is `d3571d83-ecdb-40e0-8af4-08fe14f7ed1e`; still open for actual live engine rollout and measured throughput lift. |
| #6318 | **Open** | Multiple partials landed (`a26ca1e`, `8ff2e47`, `4de477190c`) covering typed `internal_stress` attribution, route-level admission coverage, and live-headroom admission that rejects stress when reserved external headroom is unavailable. Pylon/Codex produced one earlier standalone preemption-registry patch (`assignment.public.khala_coding.chatcmpl_a5425fa595d642d3831f1670ffd6bb49`, 4,244,593 exact tokens) that was not landed because it was not wired into the live runtime; later follow-ups merged wired demand-class/admission proof in `5b11c6eaf5`, typed scheduler preemption evidence propagation in `d373eaee69` (`assignment.public.khala_coding.chatcmpl_e2f9b5be81a04b529c60b04e57e95f5b`, 3,802,388 exact tokens), typed stress-yield/preemption metadata in `88f60cb924` (`assignment.public.khala_coding.chatcmpl_da50ac2185dd438c82ea05192f27ac56`, 4,656,781 exact tokens), and production route-level abort/preemption wiring in `713b715f8d` (`assignment.public.khala_coding.chatcmpl_00d5a5d8fd7f42e384b6632dfa75d159`, 6,920,836 exact tokens). The 2026-06-27 route-admission follow-up wired a live Hydralisk admission snapshot and then tightened Hydralisk headroom so degraded/reclaimed replicas no longer pad reserved external capacity, but post-deploy proof against Worker `6c81fc7a-5d3c-49af-93a4-1f6876bbed07` still failed three times: external traffic was admitted and counted (`1972` public-counter tokens per stress/external pair), yet no `scheduler_preemption` metadata appeared and the active stress streams completed normally. Root cause is cross-isolate visibility: the in-memory stress registry could not coordinate across Cloudflare Worker isolates. `5810e7d25d` added the SQLite Durable Object scheduler (`GLM_STRESS_SCHEDULER`) for short-lived stress leases, and `a08a890a85` prevented route-admission-rejected stress from registering stale leases. Deploy `ac5af10d-ee04-437b-bfd9-7f7c56354105` then proved the in-flight cross-isolate scheduler path: admitted stress `chatcmpl_9d1bce603c054ad28f65fed0f8711866`, external HTTP `200`, matching `scheduler_preemption.evidence_ref`, `target_outcome: preempted_yielded`, and public counter `+1882` at closeout. `ca2b2e3091` adds the typed `glm_external_wins_proof` evaluator and a typed coding-delegation store-unavailable guard; deployed as Worker `2accb9cf-01de-4701-ac0e-00af67380217`. Post-deploy Pylon/Codex smoke `assignment.public.khala_coding.chatcmpl_4ee7c89308d345ff8a40ad96e174c9bd` passed with `715,584` exact own-capacity tokens, `32` owner-only traces, and no edits. `12ae929546` adds a post-preemption same-lane validation retry for primary GLM `empty_assistant_content`, so a transient empty primary response can recover with `fallbackReason:null`; exhausted retries still overflow explicitly and keep the proof blocked. Deployed as Worker `e7cb0683-58f4-48ac-836e-8bca3082d0ab`; post-deploy Pylon/Codex smoke `assignment.public.khala_coding.chatcmpl_b5ccba76058f48d58b903948cd396672` passed with `514,462` exact own-capacity tokens, `28` owner-only traces, and no edits. This continuation hardens the Khala coding dispatch gate so a generic scoped linked-owner capacity-read failure falls back to the broad registration read before returning typed store-unavailable; the failed #6318 dispatch created no assignment, trace, or token row. Post-deploy retry against Worker `ea97d32f-05e5-4fb0-906a-f9f94495acb9` still hit pre-assignment 503 despite healthy registration/link rows, so the next patch added stage-specific store-unavailable evidence and an agent-owned self-scope fallback when OpenAuth owner resolution flakes; current `main` also has the explicit-target registration fallback. A fresh 2026-06-27T09:04Z runbook retry on commit `3429704d8a` accepted `assignment.public.khala_coding.chatcmpl_c6fc3a68cf5742f194110baf60232fc2`, ran local Codex with `0` edits and `35` commands, passed the focused verifier, and proved `2,060,598` exact own-capacity tokens with `52` owner-only ATIF traces plus one private raw Codex archive (`87` SDK events / `2,267,589` bytes). Public counter baseline was `444,922,456`; post-closeout read was `446,988,289`, so assignment proof remains the exact attribution source. Still open until live saturation proof shows no premature overflow to a weaker lane. |
| #6317 | **Open** | Stress/saturation harness waits on #6318 and the live #6320 rollout. Pylon/Codex preparatory slices landed as `792ec3d56e` and `d373eaee69` (`assignment.public.khala_coding.chatcmpl_77ede33f8f2142f88a16a3e194988e8a`, 2,427,287 exact tokens): typed GLM continuous-stress plan, fail-closed runner dispatch cells, telemetry schema, and stress report aggregation gated on live headroom, external-wins preemption, and rollout-guard evidence. The 2026-06-27 prep follow-up adds canonical `x-openagents-client` dispatch attribution for stress/real-sweep traffic plus TTFT/ITL and per-replica stress-report rollups so the future live run can identify slow replicas without prompt/secret leakage. A 2026-06-27T09:15Z delegated safe-prep slice (`assignment.public.khala_coding.chatcmpl_ca5d087d7ff848178bdc5129f77eb6f4`) found and patched one arming gap: the stress plan now stays blocked unless the #6318 external-wins proof status is `accepted` and the #6320 operator acceptance exposes `canStartIssue6317Stress:true`; non-empty evidence refs alone are not enough. The delegated runner edited 5 files, ran 58 commands, passed the focused verifier, and proved `5,126,512` exact own-capacity tokens with `90` owner-only ATIF traces plus one private raw Codex archive (`153` SDK events / `1,509,222` bytes). Public counter baseline was `447,069,909`; post-closeout read was `452,446,079`, so assignment proof remains the exact attribution source. Still open for a real continuous stress runner after #6318/#6320 live proof. |
| #6312 | **Open** | Decision-grade aggregate max tokens/sec benchmark still waits on the live #6317 stress harness, but the local report plumbing is now stricter. A 2026-06-27T09:36Z delegated slice (`assignment.public.khala_coding.chatcmpl_2af17e7f361344779d3929a1ef7d793d`, `8,787,458` exact own-capacity tokens; `170` owner-only ATIF traces; one private raw Codex archive with `289` SDK events / `3,889,129` bytes) found and patched a benchmark-readout risk: aggregate and per-replica tok/s are no longer computed from summed per-request durations. Reports now require a positive explicit stress-tick measurement window; otherwise tok/s stays unmeasured while token counts and latency rollups remain available. Public counter baseline was `452,845,870`; post-closeout read was `462,740,771`, with assignment proof remaining the exact attribution source. Still open for real continuous-stress data and a decision-grade max tok/s result after #6317. |
| #6321 | **Open** | Artanis fleet-overseer automation still waits on scheduler/stress/reliability proof before any live tick is armed, but the owner-gated destructive-action scaffold now has a safe prep slice: `fleet_mutation` is modeled as a rollback-required `ArtanisRiskyActionKind`, with invariant coverage and regression tests proving replica quarantine-style actions are ineffective without operator approval plus rollback refs. Khala -> Pylon -> Codex proof: `assignment.public.khala_coding.chatcmpl_91fef54d53024affbe6f932e1fc3fb3a`, `5,988,903` exact own-capacity tokens, `148` owner-only ATIF traces, and one private raw Codex archive (`258` SDK events / `4,798,646` bytes). Public counter baseline was `463,848,927`; post-closeout read was `470,504,517`, with assignment proof remaining the exact attribution source. Still open for the actual env-gated overseer tick, health integration, stress orchestration, and live external-yield proof after #6317/#6318. |
| #6253 | **Open** | Isolated Terminal-Bench 2.0 black-box runner, bounded real measurement, and replication path landed in `da472748c5`. The current delegated safe-prep slice (`assignment.public.khala_coding.chatcmpl_6d884178bc734b46911818a4d2d622f8`, `6,719,341` exact own-capacity tokens; `122` owner-only ATIF traces; one private raw Codex archive with `206` SDK events / `1,483,284` bytes) patches a public-safety/staleness gap: the black-box runner no longer prints API-key prefixes, its public summary no longer asserts a current backing lane, and the docs mark the 2026-06-26 route observations as dated snapshots rather than current serving claims. The separately owned full Harbor run must not be disturbed. Still open for decision-grade replicate-and-beat evidence from owner-armed full-89 runs. |
| #6307 | **Open** | Owner-armed real sweep harness and Khala-side run landed in `ff89ecf498`; spendful Fireworks/Vertex comparison remains owner-gated. A 2026-06-27 runbook attempt reached Pylon assignment creation but local Codex refused execution (`assignment.public.khala_coding.chatcmpl_e7ab0dc5a1234971b9a21dc0694b6e80`, `blocker.assignment.codex_agent_execution_refused`), so the safe local follow-up patched the arming math manually: preflight now separates `320` total executable OQ5 samples from the default `240` billable comparator samples, keeps Khala own-capacity samples out of `billableSampleUpperBound`, passes actually armed billable transports into runner preflight, and marks Khala-only configs not decision-grade eligible. Focused benchmark tests, API typecheck, and `check:deploy` passed. Still open for the first owner-armed `decisionGrade:true` full report. |
| #6308 | **Open** | Recurring external head-to-head publication layer landed in `2f2d011c64`. The current delegated safe-prep slice (`assignment.public.khala_coding.chatcmpl_cc92f4c3a5104d5a835ddeacec2af97a`, `1,793,246` exact own-capacity tokens; `55` owner-only ATIF traces; one private raw Codex archive with `96` SDK events / `1,185,209` bytes) patches a publication-freshness honesty gap: the public and operator head-to-head routes now surface stored `publishedAt`, `dataAgeSeconds`, and `staleExceeded` from the snapshot row instead of letting read-time `generatedAt` make old data look current. Focused head-to-head route tests pass. Still open for recurring decision-grade data from #6307-style owner-armed runs. |
| #6309 | **Open** | Gym ladder publication/projection layer landed in `1accb3573b`. The current delegated safe-prep slice (`assignment.public.khala_coding.chatcmpl_3a7f0256b48844f99daacd203442cb8a`, `1,872,932` exact own-capacity tokens; `59` owner-only ATIF traces; one private raw Codex archive with `97` SDK events / `286,042` bytes) patches the same publication-freshness honesty gap for the Gym ladder: public/operator reads now expose stored `publishedAt`, `dataAgeSeconds`, and `staleExceeded`, so old ladder snapshots cannot look current just because the route was read now. Focused Gym ladder tests, API typecheck, and `check:deploy` pass. Still open for decision-grade rung data and recurrence evidence. |
| #6305 | **Closed** | OpenCode -> Khala checklist/recipe is closed. Keep it honest if serving regresses. |
| #6306 | **Closed** | Next ecosystem recipes are closed. Keep them as docs/recipe artifacts, not proof that Phase 4 benchmarks are complete. |
| #6351 | **Closed** | Public model/provider mix endpoint landed in `d373eaee69` and deployed as Worker `8249b442-0a54-4747-9826-c165151bcee9`. Later live verification after `713b715f8d` and latest-main deploy `d3571d83-ecdb-40e0-8af4-08fe14f7ed1e` returned `totalTokens=275,481,209`; `pylon_codex=197,954,210` tokens / 109 events / 71.857609% for `window=30d`. |
| #6352 | **Closed** | Public `/stats` page landed in `a282066552` and deployed via `deploy:safe` as Worker `197f381e-e1fc-4574-a5c0-a06d71c403d4`. It renders the live Khala token counter, America/Chicago daily history, and aggregate model-family mix from the public endpoints. The delegated Khala -> Pylon -> Codex assignment `assignment.public.khala_coding.chatcmpl_9bf5c69c6b53465598b45410b58a9cdd` counted `9,201,324` exact tokens and stored raw Codex events plus owner-only ATIF traces. |
| #6353 | **Closed** | Public stats epic is closed: #6330, #6351, and #6352 are all closed and the live `/stats` route returns HTTP 200 after Worker `197f381e-e1fc-4574-a5c0-a06d71c403d4`. |
| #6303 | **Open** | GTM umbrella remains open: recipe issues are closed and benchmark publication layers exist, but the real decision-grade benchmark/quality evidence and adoption scoreboard are not complete. A 2026-06-27 closeability audit through Khala/Pylon/Codex (`assignment.public.khala_coding.chatcmpl_3438e82cb13a46a79b0232b66a1e281d`, `1,516,751` exact own-capacity tokens; `49` owner-only ATIF traces; one private raw Codex archive with `82` SDK events / `1,627,984` bytes) made `0` edits and passed `check:public-projection-freshness`, confirming #6303 should not close until #6253/#6307/#6308/#6309 produce the required decision-grade recurring evidence and the demand/adoption scoreboard is complete. |
| #6316 | **Open** | Serving umbrella remains open: #6320/#6318 have partial slices and fresh Pylon/Codex proofs, and #6311's scheduled heartbeat rows plus fail-closed readiness acceptance are live. Current deployed Worker before this integration is `d3571d83-ecdb-40e0-8af4-08fe14f7ed1e`, but #6323 pilot, #6311 durability, #6320 live rollout, #6318 live saturation/preemption proof, #6317 stress, #6312 aggregate benchmark, #6321 overseer, and #6359 Artanis ownership epic are not complete. A 2026-06-27 closeability audit through Khala/Pylon/Codex (`assignment.public.khala_coding.chatcmpl_61eda5f58a7b474cbbfead317a485aaa`, `1,034,814` exact own-capacity tokens; `34` owner-only ATIF traces; one private raw Codex archive with `56` SDK events / `1,347,723` bytes) confirmed #6316 and all named serving blockers remain open; no local code gap was found. |
| #6325 | **Closed** | Pylon/Codex delegated sessions are persisted as private traces and exact token events (`c92a5652ab`), with the live raw-chunk follow-up in `74f25e77ad`. |
| #6326 | **Closed** | Complete raw Codex SDK event streams persist privately for Pylon/Codex Khala delegation (`48e43cee02`, deploy `4d1de2d8-6285-41fa-bd9f-7a5a88cf8275`), plus live chunk rows in `pylon_codex_raw_event_chunks`. The 2026-06-27 follow-up adds the first owner-scoped assignment status read API, `GET /api/pylon/codex/trace-status?assignmentRef=...`, which reports assignment lifecycle, assignment event/progress counts, exact-token row status, owner-only trace counts/final trace UUID, live raw-chunk counts, final raw archive metadata, and derived progress without exposing raw SDK payloads. Frontend `/trace/{uuid}` live assignment rendering remains a follow-up. |
| #6331 | **Closed** | The Pylon coding-delegation 500/unavailable path is fixed with typed diagnostics and proof surfaces. |
| #6354 | **Closed** | Pylon `assignment run-no-spend` now refreshes presence before claiming, emits a typed heartbeat recovery diagnostic if stale presence cannot be refreshed, records public-safe active local assignment run markers, and projects those active Codex/Claude runners into heartbeat/go-online busy capacity. Focused Pylon typecheck and assignment/presence regressions passed. |
| #6355 | **Closed** | `pylon khala burndown` formalizes the manual stress batches into a repeatable max-parallel operator loop: dry-run plan, optional `--execute` dispatch/run/proof, exact token-proof verification, public counter before/after evidence, and explicit `operator_review_required` merge closeout. Runbook: `apps/pylon/docs/khala-burndown-runbook.md`. |
| #6356 | **Closed** | `GET /api/operator/khala/trace-review` now returns the recurring owner/admin trace-review report: failure modes, model mix, outcome buckets, notable trace refs, recurring user intents, raw Codex event metadata, and triage items. Runbook: `docs/khala/2026-06-26-khala-trace-review-runbook.md`. |
| #6357 | **Closed** | Operator ledger landed and issue closed at `2026-06-26T22:41:12Z`: `GET/POST /api/operator/khala/unsupported-requests` maintains the Forum-first unsupported-request list from trace review, feedback refs, Forum refs, or operator rows. `bug` / `missing_capability` rows default to `needs_issue` until a GitHub issue ref is attached; runbook: `docs/khala/2026-06-26-khala-unsupported-request-list.md`. |
| #6358 | **Closed** | Counter-health patch deployed through `deploy:safe` as Worker `95d3fcee-f740-477d-b3c4-368f198e8255`, then corrected by the monotonic-counter follow-up: public token-counter projections include all real served-token rows (`internal`, `internal_stress`, `own_capacity`, external, and unlabeled) while keeping demand labels out of the public payload; live sync deltas publish every fresh served-token row as refs + timestamps + counts; `scripts/khala-heartbeat.sh` / `scripts/khala-canary.sh` validate 200 + non-empty usage and monotonic/readable counter health, requiring counter movement by default even for internal probes. The local patched Pylon recovered stale assignment `assignment.public.khala_coding.chatcmpl_fd33103f7b4349218f9b0760e8ca5632` with closeout `assignment.closeout.cfd5d6dd9b2a6140f361a836`, then ran two same-Pylon assignments concurrently to accepted exact-token closeout (`assignment.public.khala_coding.chatcmpl_6d190807e87c4a558dac39a098a9d268`, 161,832 tokens; `assignment.public.khala_coding.chatcmpl_a2bd2121c00d4a2f8e63eb26f48f9148`, 128,873 tokens). Optional labeled in-flight estimate remains deferred. |
| #6359 | **Open** | **NEW EPIC** — Artanis autonomously owns the whole Khala improvement loop (unblock users, ensure inference, drive #6355, act on feedback, consult `inference-engineering-book` once the set drains). Subsumes #6321; coordinates #6355/#6356/#6357/#6358 + the #6316 serving track. See the Artanis section. |
| #6360 | **Open** | **NEW** — Artanis ingests + acts on Khala CLI `/feedback` (`khala_feedback` table): style→response-style change (owner-gated), capability gap→#6357→issue, bug→issue. |
| #6363 | **Open** | **NEW** — operator console: speak to Artanis directly (owner-auth channel + situational awareness of recent actions/goals/ongoing ops + persistent owner-interaction memory), **not** the Khala collective-intelligence roleplay. The human-facing front of epic #6359. |
| #6321 | **Open** | Artanis fleet-overseer control loop (heal/scale/stress/external-yield). Now scoped under the broader Artanis ownership epic #6359. Safe prep has landed the rollback-required `fleet_mutation` approval-gate kind only; no fleet-overseer tick or live fleet action is armed. |
| #6354 | **Closed** | The execution-lane blocker is implemented: `assignment run-no-spend` publishes a best-effort heartbeat before polling/claiming, returns `diagnostic.assignment.presence_heartbeat_required` with the exact heartbeat command when refresh fails and admission remains stale, and maintains per-run active local assignment markers so `provider go-online --json` and heartbeat `load.coding.*.busy` reflect in-flight Codex/Claude runners. |

## Execution notes

- 2026-06-27 #6316 closeability audit at public commit
  `4b02c1c1016d93d6482d8597c71a28d6aa160d7e`: live GitHub issue state shows
  #6316 still open. The serving blockers checked for this audit are also open:
  #6323 full-model pilot, #6311 durability, #6320 live throughput rollout,
  #6318 external-wins saturation/preemption proof, #6317 continuous stress,
  #6312 aggregate max tok/s benchmark, #6321 fleet-overseer tick, and #6359
  Artanis autonomous ownership. Older #6316 body children #6313/#6314/#6315
  and related #6310/#6259 are closed, but they are not sufficient closeout
  evidence for the expanded serving umbrella. No live model/provider calls were
  made for this audit.
- 2026-06-27T13:00Z #6317 live stress continuation: after draining the
  terminated `g4-4g-b-20260625154532` replica in Worker config
  (`2c8aaae59e2e85e7629da576825ab48d641695f2`, deployed as Worker
  `6f694bdc-e0f2-4ebf-8670-823e4b2181b3`) and restarting the stuck private
  proxies on the seven 4-GPU Spot origins, the public gateway again recorded
  exact GLM stress rows in `token_usage_events`. The authenticated tuned run
  `issue6317-glm-tuned2048-20260627T1049Z` ran at concurrency `9` with
  `max_tokens=2048` until its target completed: local artifact final was
  `4859` launches, `1570` successful GLM receipts, `3289` failures, `105`
  HTTP `500`s, `3184` HTTP `502`s, `4222404` exact GLM receipt tokens,
  `534.51` receipt tok/s, and `0` non-GLM receipt tokens. Final D1 proof across
  the four tracked #6317 run ids shows `5637616` exact public-ledger GLM tokens
  (`303153` initial mixed run + `1157398` post-fix 4096-token run + `569`
  authenticated smoke + `4176496` tuned-run persisted total). The local runner
  receipts imply `5683524` exact generated GLM tokens, leaving a follow-up
  ledger closeout gap of `45908` tokens / `17` tuned-run success rows not yet
  present in D1 after the final remote recheck. `GET
  /api/public/khala-tokens-served` returned `507438257` at
  `2026-06-27T12:58:58.821Z` and still rebuilds on `token_usage_events`, so the
  counter path itself is confirmed working for `/khala` and `/stats`. #6317
  remains open for cleaner overload/yield behavior and the final-row ledger
  discrepancy, not for lack of stress volume.
- 2026-06-27T11:03Z #6323 isolated live-host attempt: `us-central1` could not
  allocate another 8x RTX PRO 6000 Spot host because regional preemptible quota
  was exhausted, but a separate `us-west1-b` `g4-standard-384` host was
  provisioned, loaded with the `433G` / `57`-file
  `nvidia/GLM-5.2-NVFP4` checkpoint, and kept isolated from live Khala routing.
  The existing REAP vLLM image rejected the older card flags and then failed
  model config validation on `deepseek_sparse_attention`; the current official
  `vllm/vllm-openai:v0.23.0` image failed sparse-MLA backend selection on the
  RTX PRO 6000 Blackwell `sm_120` host. The
  `lmsysorg/sglang:dev-glm52-nvfp4` image got farther (DSA attention, fp8_e4m3
  KV, TRTLLM DSA prefill/decode, `flashinfer_cutlass` ModelOpt FP4 MoE) but
  failed near the end of weight loading with a MoE `w13` tensor shape mismatch
  (`3072` vs `6144`). No healthy endpoint, tool-loop, quality, context, or
  throughput pass is claimed; the isolated containers were removed and the
  8-GPU VM was stopped. #6323 remains open, now narrowed to the SGLang
  weight-load compatibility path rather than another planning/harness gap.
- 2026-06-27T01:00Z #6323 refresh from a clean detached worktree at
  `0d67f2ae5b1a1605d608aa69478dd8e86da4cb71`: the local environment still has
  no `KHALA_GLM_NVFP4_*` owner-run variables. `pilot:glm-nvfp4 --summary
  --output-dir <tmp>` produced the expected public-safe `decision:"no_go"`
  bundle with all four gates blocked, and the focused pilot/operator tests
  passed (`16` tests). A follow-up Khala -> Pylon -> Codex audit assignment
  (`assignment.public.khala_coding.chatcmpl_4e158d9b7d34452c92d2ee562921cdbd`)
  was interrupted after ~270s of runtime-heartbeat-only progress, then closed as
  stale with `blocker.assignment.local_run_interrupted` and closeout
  `assignment.closeout.d266b387510afb76aef2e2b2`; proof shows `0` exact token
  rows and no raw-event closeout. This does not close #6323. The remaining
  close condition is still the owner-armed isolated 8x-host pilot with measured
  tool-loop, quality, max-context, and tok/s evidence.
- 2026-06-27T05:28Z #6323 delegated boot/load evidence hardening: a fresh
  Khala -> Pylon -> Codex run
  (`assignment.public.khala_coding.chatcmpl_44a4a0608a1049d59b41fae44b840433`)
  completed accepted with exact proof of `2,738,605` own-capacity tokens, `67`
  owner-only ATIF traces, and one owner-only raw SDK archive containing `109`
  events / `2,623,114` bytes. The integrated patch requires
  `KHALA_GLM_NVFP4_BOOT_LOAD_EVIDENCE_REF` / `--boot-load-evidence-ref` before
  the isolated owner-armed endpoint gate can pass and redacts unsafe boot/load
  refs from public summaries. This still does not close #6323; it prevents an
  owner-armed pilot from being reported complete without public-safe evidence
  that the isolated 8x host actually booted and loaded the full-model endpoint.
- 2026-06-27T07:00Z #6323 delegated audit from clean current `origin/main`:
  local env still had no `KHALA_GLM_NVFP4_*` owner-run variables, and
  unarmed `pilot:glm-nvfp4 --summary --output-dir <tmp>` exited `2` with
  `decision:"no_go"` and the expected blocked gates. A fresh
  Khala -> Pylon -> Codex audit assignment
  (`assignment.public.khala_coding.chatcmpl_1702ef0731874c72934c1f95068f8c47`)
  completed accepted with `0` edits, focused verifier pass, exact proof of
  `810,643` own-capacity tokens, `32` owner-only ATIF traces, and one
  owner-only raw SDK archive containing `52` events / `1,252,108` bytes. Public
  counter read moved from `434,328,252` before dispatch to `435,139,462` after
  closeout. This does not close #6323; it confirms the issue is blocked on the
  owner-armed isolated 8x-host endpoint/evidence run, not on a remaining local
  harness/doc implementation gap.
- 2026-06-27T08:34Z #6323 continuation check from a fresh current worktree:
  after installing pinned workspace dependencies with
  `bun install --frozen-lockfile`, the local environment still exposed no
  `KHALA_GLM_NVFP4_*` owner-run variables. The unarmed
  `pilot:glm-nvfp4 --summary --output-dir <tmp>` path exited `2` with
  `decision:"no_go"` and the expected blocked gates
  (`isolated_owner_armed_endpoint_context`, `tool_loop_proof`,
  `quality_parity`, and `throughput_context_tradeoff`). Focused verification
  passed:
  `bun run --cwd apps/openagents.com/workers/api test -- src/inference/glm-nvfp4-pilot.test.ts src/inference/glm-nvfp4-pilot-operator.test.ts`
  (`19` tests). This adds no closeout evidence and does not close #6323; the
  remaining requirement is still the owner-armed isolated 8x-host pilot with
  public-safe boot/load, tool-loop, quality, max-context, and throughput
  evidence.
- 2026-06-27T13:40Z #6323 serving-stack evidence hardening from a fresh
  worktree: the pilot artifact now carries `bootLoadStatus` plus typed
  `servingStackFindings` for bounded no-go evidence such as
  `vllm_sparse_mla_backend_unavailable` and
  `sglang_moe_w13_shape_mismatch`. Unsafe finding refs are redacted, failed
  pre-endpoint attempts keep the isolated endpoint gate blocked, and public
  output stores only failure codes plus evidence refs rather than raw logs,
  endpoint URLs, host paths, or shape-stack payloads. Focused pilot/operator
  coverage passed (`21` tests). This still does not close #6323; it makes the
  next isolated-host retry auditable without overstating a failed boot/load.
- 2026-06-27T03:34Z #6311 refresh: live
  `/v1/gateway/glm-fleet/readiness` now reports `status:"degraded"` with
  `totalReplicaCount:10`, `readyReplicaCount:2`, `reclaimedReplicaCount:8`,
  `warmOrReadyMaxInflight:2`, and `acceptance.status:"blocked"`.
  `glm-fleet:durability --summary` now includes aggregate counts,
  status-specific replica refs, and typed operator action items in the
  headless readout, so reclaimed replicas and missing forced-STOP proof refs
  cannot be mistaken for completed durability. Focused verification:
  `glm-fleet-readiness`, `glm-fleet-readiness-routes`, and
  `glm-fleet-durability-operator` tests passed on the delegated runner. This
  slice was delegated through Khala -> Pylon -> Codex as
  `assignment.public.khala_coding.chatcmpl_8c5d5c98f2544913aea50facfb81ee30`;
  exact own-capacity proof reported `2,780,444` total tokens, `59` owner-only
  traces, and `99` private raw SDK events / `3,399,098` bytes.
- 2026-06-27T07:16Z #6311 delegated no-code-gap audit from clean current
  `origin/main`: the runbook preflight showed local Codex ready, Pylon
  `pylon.33afd48282a649047e3a` online, fresh heartbeat sequence `489`, and
  one available Codex slot. The typed assignment
  `assignment.public.khala_coding.chatcmpl_50871eacbf1647439bc273f7b4f118f3`
  ran through `assignment run-no-spend` with `0` edits, `26` commands, one Codex
  turn, and the focused #6311 verifier passed. Assignment proof reported one
  exact own-capacity token row with `917,278` total tokens, `38` owner-only
  ATIF traces, and one private raw Codex archive containing `64` events /
  `795,253` bytes. The public scalar moved from `435,204,776` before dispatch to
  `436,122,621` after closeout; exact attribution remains the assignment proof,
  because the scalar is global. This does not close #6311: the remaining work is
  still the real live durability evidence and owner/infra decisions for
  all-replica watchdog/forced STOP recovery, capacity floor, multi-region
  auto-replace/reserve/prebake, and quota state.
- 2026-06-27T08:44Z #6311 delegated continuation audit from current `main`:
  runbook preflight showed Pylon `pylon.33afd48282a649047e3a` online, linked,
  not stale, heartbeat sequence `522`, and one available local Codex slot. The
  typed `codex_agent_task`
  (`assignment.public.khala_coding.chatcmpl_8280f63b4dad4180a04fd22f32b1b374`)
  ran through `assignment run-no-spend` with `0` edits, `48` commands, one Codex
  turn, and the focused #6311 verifier passed. Assignment proof reported one
  exact `token_usage_events` row for `3,037,332` own-capacity tokens (`3,028,126`
  input, `9,206` output, `527` reasoning, `2,839,936` cache-read), `72`
  owner-only ATIF traces, and one owner-only raw Codex archive containing `120`
  events / `4,158,470` bytes. Public counter baseline was `439,771,273`; the
  post-closeout read was `442,810,372`, consistent with exact closeout
  accounting plus ordinary global concurrency. This still does not close #6311:
  it confirms the local repo/readout path has no fresh code gap, while closure
  requires real fleet durability evidence and owner/infra decisions for
  all-replica watchdog / forced STOP recovery, capacity floor, multi-region
  auto-replace/reserve/prebake, and quota state.
- 2026-06-27T01:22Z #6318 refresh: the production chat route had the
  `internalStressPreemption` registry wired but did not pass a live
  `routeAdmission` snapshot, so the generic scheduler could not form the
  preemption policy from actual GLM pool headroom. The follow-up introduces a
  Hydralisk pool runtime that exposes both the adapter and the admission snapshot
  from the same in-memory `inflight` map, then wires that snapshot into
  `/v1/chat/completions`. Focused verification passed:
  `hydralisk-adapter` + `model-router` tests (`73` tests), the five #6318
  chat-route admission/preemption tests, and
  `bun run --cwd apps/openagents.com typecheck:api` with only the pre-existing
  `Effect.void` language-service advisories. This still does not close #6318;
  live saturation/preemption proof remains required before #6317 can run real
  continuous stress.
- 2026-06-27T04:15Z #6318 post-deploy proof after the route-admission/headroom
  fix failed in a useful way. Live readiness showed `totalReplicaCount:10`,
  `readyReplicaCount:2`, and `reclaimedReplicaCount:8`. Three authenticated
  stress/external probe pairs all returned external HTTP `200` and moved the
  public token counter by `1972` tokens per pair at request closeout, but none
  emitted `scheduler_preemption` metadata and each stress stream completed
  normally. That proves the public counter moves at accepted closeout, not per
  raw stream chunk, and that the remaining #6318 bug is cross-isolate scheduler
  state rather than only replica-headroom math. This follow-up adds the
  `GLM_STRESS_SCHEDULER` Durable Object so external requests can see and
  preempt active stress leases across Worker isolates.
- 2026-06-27T04:50Z first live DO deploy proof against Worker
  `4537f061-8e94-4579-850a-3c61dbf0126b` confirmed the binding was live and
  the external response could carry scheduler-preemption metadata, with `614`
  external tokens counted at closeout. It also showed the first coordinator
  wiring was too eager: a route-admission-rejected `internal_stress` request had
  still registered a DO lease, so the metadata looked like an in-flight
  preemption even though stress had already yielded. The follow-up patch blocks
  global lease registration when route admission will reject stress.
- 2026-06-27T05:00Z final #6318 scheduler probe after deploy
  `ac5af10d-ee04-437b-bfd9-7f7c56354105`: live readiness was still
  `degraded` / `blocked` with `10` configured replicas, `1` ready replica, and
  `9` draining replicas, but the bounded probe admitted stress
  `chatcmpl_9d1bce603c054ad28f65fed0f8711866`. The external request returned
  HTTP `200`, emitted matching `scheduler_preemption.evidence_ref`, and marked
  `target_outcome: preempted_yielded`. The public counter moved from
  `416491612` to `416493494` at request closeout. #6318 remains open because
  the external response served through Fireworks after
  `fallback_reason: empty_assistant_content`; acceptance still requires no
  premature overflow to a weaker lane under saturation.
- Current local #6318 follow-up adds a typed
  `openagents.khala.glm_external_wins_proof.v0_1` evaluator. It accepts a live
  external-wins proof only when the external response has scheduler-preemption
  evidence, HTTP success, `servedLane:"glm_primary"`, and `fallbackReason:null`.
  The observed `empty_assistant_content` -> weaker fallback shape is now
  projected as blockers
  `blocker.glm_external_wins.fallback_after_preemption`,
  `blocker.glm_external_wins.served_lane_not_glm_primary`, and
  `blocker.glm_external_wins.empty_glm_content_after_preemption`.
- 2026-06-27T06:13Z #6318 dispatch-gate and audit smoke after deploy
  `2accb9cf-01de-4701-ac0e-00af67380217`: the same clean-worktree
  Pylon/Codex runbook accepted
  `assignment.public.khala_coding.chatcmpl_4ee7c89308d345ff8a40ad96e174c9bd`,
  ran local Codex with `0` edits, passed the focused verifier, and proved
  `715,584` exact own-capacity tokens, `32` owner-only ATIF traces, and one
  private raw Codex archive (`51` SDK events / `163,196` bytes). This confirms
  the dispatch path is working again when linked Pylon capacity is available,
  and confirms the public token counter moves at closeout after exact
  `token_usage_events` insertion, not continuously during streamed raw chunks.
- Current #6318 router hardening adds one same-lane validation retry when an
  external request has scheduler-preemption evidence and the primary GLM lane
  returns `empty_assistant_content`. A successful retry stays on the primary lane
  with `fallbackReason:null`; an exhausted retry still overflows explicitly, so
  the `glm_external_wins_proof` remains fail-closed. A follow-up Pylon/Codex
  dispatch for this real task hit the typed transient capacity-read `503` twice
  after fresh heartbeat/capacity preflight, so this slice was implemented
  locally and the gate race remains an operational issue.
- Current dispatch-gate hardening closes the narrow local gap found in that
  failure: a generic `listRegistrationsForOwnerAgentUserIds` read error now
  falls back to the broad `listRegistrations` owner-filter path, while failure
  of both reads still returns the typed store-unavailable 503. No assignment,
  trace, or token row existed for the failed dispatch because it died before
  assignment creation.
- Post-deploy retry after Worker
  `ea97d32f-05e5-4fb0-906a-f9f94495acb9` still failed pre-assignment even with
  fresh Pylon heartbeat and healthy D1 registration/link rows. The follow-up
  hardening adds public-safe stage evidence for OpenAuth owner resolution,
  linked-agent read, linked-owner registration read, assignment-list read, and
  assignment-create failures, and lets direct agent-token requests continue
  through self-agent scope if OpenAuth owner resolution is transiently down.
- 2026-06-27T08:00Z #6318 staged retry after Worker
  `85ebf9c1-6c3a-4d58-87ed-933e64efd46c`: the typed Pylon/Codex dispatch still
  failed before assignment, but now reported the exact stage:
  `linked owner registration read`. That produced no assignment, trace, proof,
  raw-event row, or token row. The current local fix adds an explicit-target
  registration fallback: when `--pylon-ref` is present and indexed linked-owner
  reads fail, the gate reads `readRegistration(targetPylonRef)`, filters it by
  the caller's linked owner-agent ids, and only then admits it as candidate
  capacity. Focused coverage proves the pure gate and chat route recover when
  scoped and broad reads both fail but the direct target read succeeds; if all
  three reads fail, the route still returns typed store-unavailable evidence.
- 2026-06-27T06:28Z #6318 post-deploy smoke after Worker
  `e7cb0683-58f4-48ac-836e-8bca3082d0ab`: Pylon/Codex assignment
  `assignment.public.khala_coding.chatcmpl_b5ccba76058f48d58b903948cd396672`
  ran local Codex with `0` edits, passed `model-router.test.ts`, and proved
  `514,462` exact own-capacity tokens, `28` owner-only ATIF traces, and one
  private raw Codex archive (`45` SDK events / `755,150` bytes).
- 2026-06-27T06:51Z trace-status audit slice: the clean-worktree
  Pylon/Codex runbook accepted
  `assignment.public.khala_coding.chatcmpl_8010cf9064ce400fbd74f4e08969ffa8`,
  ran local Codex with `10` edits, passed the focused trace-status verifier,
  and proved `5,242,969` exact own-capacity tokens, `96` owner-only ATIF traces,
  and one private raw Codex archive (`157` SDK events / `3,558,909` bytes).
  This is the current reproduction of what made delegation work: local token in
  `OPENAGENTS_AGENT_TOKEN`, daemon routing disabled, fresh heartbeat, explicit
  `--workflow codex_agent_task`, explicit Pylon ref, exact commit pin, local
  `assignment run-no-spend`, then assignment-scoped proof. The visible public
  token counter updates at exact closeout/token-row insertion, not continuously
  during raw chunk streaming.
- 2026-06-27T01:30Z #6317 prep: stress and real-sweep dispatch attribution now
  use the canonical `x-openagents-client` header instead of the stale
  `x-openagents-demand-client` field, matching the chat route's typed demand
  attribution. The public-safe stress report now carries overall and
  per-replica TTFT/ITL P50/P90/P99/mean/sample-count rollups plus per-replica
  ok/deferred/preempted/failed counts and goodput/TPS. This is still local
  harness preparation only; live continuous stress remains blocked on #6318 and
  #6320 proof.
- Latest live counter/proof checkpoint after the supervised #6311/#6323 pair:
  `e681fe6ab3` is current `origin/main` and is deployed as Worker
  `8c03f302-8186-468d-a2b8-cd8d3489ae0b`. Live
  `/api/public/khala-tokens-served` returned `307,876,565` at
  `2026-06-26T20:49:13.272Z`; live
  `/api/public/khala-tokens-served/model-mix?window=30d` returned
  `pylon_codex=211,315,496` over `117` usage events. The last two verified
  Pylon/Codex closeouts were #6311
  `assignment.public.khala_coding.chatcmpl_f07a0d9b3b684abc80284aa0c818ca8c`
  (`963,220` exact tokens) and #6323
  `assignment.public.khala_coding.chatcmpl_865394ecde434183b28654df6c306617`
  (`1,971,655` exact tokens), totaling exactly `2,934,875` tokens. The
  `pylon_codex` bucket moved by that exact amount after closeout. If the live
  homepage or `/khala` number only appears to tick a little between refreshes,
  no new large Pylon/Codex exact closeout row has landed in that interval; do
  not infer assignment attribution from aggregate movement because canaries and
  other agents can also write token rows.
- #6354 is no longer the Pylon/Codex execution-lane blocker: runner-owned
  presence refresh, active local busy-load projection, and typed stale-presence
  recovery diagnostics are implemented. More parallel delegation can resume
  while continuing to watch for duplicate active-assignment capacity denials or
  other public-safe blockers.
- 2026-06-26: Supervising agents may briefly prioritize Khala -> Pylon -> Codex
  steering blockers ahead of the next phase item when the blocker prevents honest
  delegation, token attribution, or trace verification. This does not reorder the
  product backlog; it keeps the execution lane usable.
- Pylon auth recovery landed as `ab02d4efaa`. Live delegation now uses fresh
  Pylon ref `pylon.081964280c7710ad0820` after the previous live ref was found
  to be tied to a stale credential; treat older-ref evidence as historical and
  use the fresh ref for current delegation checks.
- Pylon/Codex steering is a usable lane as of `7057e61e0b`:
  `assignment run-no-spend` auto-selects a ready connected Codex account when no
  explicit account is provided, while still supporting `--account` /
  `--account-ref`. On 2026-06-26, `pylon accounts list --json` showed five ready
  Codex accounts (`codex`, `codex-2`, `codex-3`, `codex-4`, and default).
- Same-owner token rotation is now an explicit steering invariant: if an
  OpenAgents agent token is reissued/rotated under the same OpenAuth owner, the
  replacement credential may re-register/heartbeat the existing local Pylon and
  complete same-owner assignments; unrelated agent credentials remain forbidden.
- Closed steering fixes that should stay assumed in future work: #6331 typed
  target-Pylon unavailable diagnostics, #6332 Codex reconnect/link refresh,
  #6333 one-shot `presence heartbeat --json`, #6334 runtime lifecycle streaming,
  #6335 verifier sanitizer false-positive fix, #6336 capacity-gate link repair,
  #6339 pinned checkout/verifier defaults, #6340 live runtime progress, #6341
  `provider go-online` pylonRef/heartbeat proof, #6349 no silent fixture
  fallback, and #6350 proof readout without direct D1 queries.
- #6331 is closed, but its invariant stays live: a targeted linked Pylon whose
  assignment dispatch gate is full must return a typed, diagnosable
  `target_pylon_unavailable` response with gate evidence, not fall through to a
  generic unavailable/500 path.
- #6325/#6326 are closed. Pylon/Codex delegated turns must still be verified by
  exact `token_usage_events` rows, private `agent_traces`, and
  `pylon_codex_raw_events` rows. Verification has a first-class owner-scoped read
  path (`GET /api/pylon/codex/proof?assignmentRef=...` and
  `pylon khala proof --assignment-ref <assignmentRef> --json`). Counter movement
  alone is never proof because other agents may be running.
- Pylon state at refresh: `provider go-online --json` reported
  `pylon.33afd48282a649047e3a`, lifecycle `online`, Codex ready, and
  `ownCapacityDispatch.maxCodexAssignments: 5` /
  `availableCodexAssignments: 5`. After the latest batch, busy was reset to
  `0` and available Codex capacity was again `5`. `pylon accounts list --json`
  showed ready present accounts for `codex`, `codex-2`, `codex-3`, `codex-4`,
  the default Codex homes, `claude-pylon-2`, and `claude-pylon-3`, plus two
  stale/missing Codex registry refs. The practical result: multi-account Codex
  dispatch can work, but the remaining steering work must keep proving the
  assignment gate uses the coding-capacity projection, not the legacy one-flight
  policy field.
- Prior paused delegation: assignment
  `assignment.public.khala_coding.chatcmpl_ffe4aef49ef94614be78bc9c8c7b3b62`
  completed locally on `codex-3` against `91edb870c3` with accepted closeout
  `assignment.closeout.de5c448aa8a73c1639aaff89`. The patch was reviewed, tested,
  merged as `856fc636d023f821480480f11654988ade65e9ca`, pushed to `main`, and
  deployed through `deploy:safe` as Worker version
  `785a7379-10ca-4b1c-9e86-bc734b11e2ec`. Verification was
  `typecheck:web`, `typecheck:api`, focused GLM heartbeat/readiness tests, and
  `check:deploy`, followed by live `/` and `/khala` 200 smokes.
- Owner-scoped proof for that assignment is now good when called with the
  explicit flag form:
  `pylon khala proof --assignment-ref assignment.public.khala_coding.chatcmpl_ffe4aef49ef94614be78bc9c8c7b3b62 --json`.
  It reports one exact token row (`total_tokens: 869982`,
  `provider: pylon-codex-own-capacity`,
  `demand_source: khala_coding_delegation`), one owner-only ATIF trace, and one
  owner-only raw-event row with 71 Codex SDK events / 212,420 bytes.
- Latest multi-account Pylon/Codex batch before pause: four accepted assignments
  completed across `codex-2`, `codex-3`, and `codex-4` with exact owner-capacity
  token rows, owner-only ATIF traces, and owner-only raw Codex event rows:
  `assignment.public.khala_coding.chatcmpl_6a9906e622ad43caa1c9cc3fb2f20d00`
  (#6323 retry: 4,371,243 tokens, 150 raw events / 2,578,167 bytes);
  `assignment.public.khala_coding.chatcmpl_b5a0d831027a4c779b1105be73217f29`
  (#6311: 6,415,202 tokens, 213 raw events / 1,159,118 bytes);
  `assignment.public.khala_coding.chatcmpl_5ccffa5593b84cc09e414d3ad358b9b0`
  (#6320: 2,202,625 tokens, 111 raw events / 1,609,106 bytes); and
  `assignment.public.khala_coding.chatcmpl_29c527d6cc154a52b05279a36fb93e34`
  (Pylon proof/steering: 4,496,928 tokens, 172 raw events / 915,208 bytes).
  Exact accepted-batch total: **17,485,998 Khala-attributed tokens**. A first
  #6323 attempt on `codex` was rejected with
  `blocker.assignment.codex_agent_execution_refused`; use `codex-2+` for the
  next stress batch until `codex` is revalidated.
- Mainline changes from that batch: `a8c12aff42` (#6320 rollout selector),
  `7bf68b4652` (Pylon own-capacity dispatch proof readout),
  `36ee76689c` (#6311 bounded heartbeat probes), plus concurrent mainline
  commits `efaa53424b` and `96c9b91599`. The #6323 retry originally left its
  generated pilot harness dirty in the delegated workspace; that patch has now
  been reviewed, tightened to keep API keys env-only and URL-like refs
  fail-closed, merged as `c7a86d7d06`, commented on #6323, and deployed through
  `deploy:safe` as Worker version `12066869-4f81-4b32-ba41-ba3c50b07595`.
- Latest post-refresh stress batch, launched from clean `origin/main` at
  `e048048a2a` and verified at `2026-06-26T16:42Z`, accepted four useful
  Pylon/Codex assignments and rejected one unhealthy plain-`codex` assignment:
  - #6311: `assignment.public.khala_coding.chatcmpl_c7358576d7464620ae3da33ed2f473a0`
    on `codex-2`, merged/pushed as `7f94c0556a`, exact tokens
    `7,393,396`, trace `da4439ad-06be-45e0-9210-f0d70ee74035`, raw events
    `201` / `2,952,802` bytes
    (`raw.pylon_codex.f2f41231823f31265798c5ebfb29b6a9`).
  - #6320: `assignment.public.khala_coding.chatcmpl_7033a7cd0fff4afaaad412e783bab29a`
    on `codex-4`, merged/pushed as `03b6ffa094`, exact tokens
    `4,949,572`, trace `6276803a-0a42-44a5-9e4c-8682be1cfb64`, raw events
    `165` / `4,646,052` bytes
    (`raw.pylon_codex.2e4f044c933398aa783aa0b4823edb7e`).
  - #6318: `assignment.public.khala_coding.chatcmpl_a5425fa595d642d3831f1670ffd6bb49`
    on `codex-3`, accepted with exact tokens `4,244,593`, trace
    `f0061431-d335-4cbb-b9a2-b09997001f1e`, raw events `176` /
    `3,839,138` bytes
    (`raw.pylon_codex.e4b6da057344f9fe9f11e226642762cb`). Overseer review
    merged only the live steering-safe part as `2f467e3476`; the workspace's
    standalone registry/harness files were not landed because they were not
    wired into production routing.
  - #6317: `assignment.public.khala_coding.chatcmpl_f06a11f5a625470bbc235a6d6ed952df`
    on `codex-2` concurrently with #6311, accepted with exact tokens
    `4,455,618`, trace `ab31c0b5-eff2-4cf3-b627-25db01b4f163`, raw events
    `149` / `1,906,799` bytes
    (`raw.pylon_codex.55dcd51577b7e5afadba8664d7c29b70`). Overseer review
    merged the typed, non-running plan-gate slice as `792ec3d56e`.
  - #6323 plain-`codex` retry
    `assignment.public.khala_coding.chatcmpl_6ea9caf0863f43488dbba5b2aaa30481`
    was rejected with `blocker.assignment.codex_agent_execution_refused`.
  This batch proves at least three connected Codex accounts can run, and one
  account (`codex-2`) can run two assignments in parallel. Treat the plain
  `codex` account as unhealthy until revalidated.
- Public counter state at refresh:
  `/api/public/khala-tokens-served` returned `237,289,920` at
  `2026-06-26T16:59:46.118Z`. In the corrected 30-minute window,
  `token_usage_events` had five `khala_coding_delegation` rows totaling
  `21,090,144` tokens, with the latest at `2026-06-26T16:42:00.830Z`; after
  that, the small visible ticks were heartbeat/canary rows. The current Pylon/
  Codex behavior is closeout-based: the public counter updates when each Codex
  turn posts its exact `token_usage_events` row, not continuously for every
  streamed SDK event while the turn is still running. The exact Pylon/Codex
  attribution must still come from
  `pylon khala proof --assignment-ref ... --json` or token rows filtered to
  `provider='pylon-codex-own-capacity'`, because public counter movement is
  aggregate and other agents may be running.
- Public counter state after the raw-chunk follow-up and freshness fix:
  `4ab28bfdfc` is current `origin/main` and fixes the public counter freshness
  / banner path after `74f25e77ad` added live Pylon/Codex raw event chunks.
  `/api/public/khala-tokens-served` returned `244,636,738` at
  `2026-06-26T17:42:30.969Z`. In the preceding two hours, live D1 showed
  `75` exact `khala_coding_delegation` rows totaling `148,621,006` tokens,
  with the newest Pylon/Codex exact row at `2026-06-26T17:38:29.189Z`
  (`assignment.public.khala_coding.chatcmpl_442269504f884516908431ca167d9ed9`,
  `681,446` tokens). The live raw-event chunk table had `26` chunks / `43`
  events / `411,609` bytes for that same assignment. After `17:38Z`, the
  visible small ticks were canary/background rows, not active large Codex
  sessions; `ps` showed no active `assignment run-no-spend` workers. As of the
  same refresh, `provider go-online --json` reported Pylon
  `pylon.33afd48282a649047e3a` with Codex `available=5`, `busy=0`, `queued=0`,
  and heartbeat sequence `144`.
- Public counter state after the five-delegation stress batch:
  `d373eaee69` is current deployed `main` (Worker
  `8249b442-0a54-4747-9826-c165151bcee9`). Five Khala -> Pylon -> Codex
  assignments completed across `codex-2`, `codex-3`, and `codex-4` with exact
  `token_usage_events` rows totaling **17,938,519** Khala coding tokens:
  #6311 `3,653,854`, #6320 `2,656,599`, #6318 `3,802,388`, #6317 `2,427,287`,
  and #6351 `5,398,391`. Live `/api/public/khala-tokens-served` returned
  `262,631,034` at `2026-06-26T18:01:30.689Z`. Live
  `/api/public/khala-tokens-served/model-mix?window=30d` returned the same
  `totalTokens`, with `pylon_codex=166,561,460` tokens over `102` usage
  events. All five assignments have owner-only ATIF traces and owner-only raw
  Codex event rows; the public counter movement was verified from exact
  assignment `task_ref` rows, not inferred from aggregate movement.
- Public stats closeout after #6352/#6353:
  `a282066552` is deployed as Worker
  `197f381e-e1fc-4574-a5c0-a06d71c403d4`, and `/stats` returns HTTP 200.
  Live `/api/public/khala-tokens-served` returned `271,894,178` at
  `2026-06-26T18:25:04.821Z`. Live
  `/api/public/khala-tokens-served/model-mix?window=30d` returned
  `pylon_codex=175,762,784` tokens over `103` events, `69.39332%` of the
  30-day public mix. The #6352 delegated assignment
  `assignment.public.khala_coding.chatcmpl_9bf5c69c6b53465598b45410b58a9cdd`
  emitted `9,201,324` exact tokens and stored `1` whole-turn raw Codex blob
  (`216` events / `2,714,430` bytes), `130` streamed raw chunk rows
  (`216` events / `2,788,923` bytes), and `131` owner-only ATIF trace rows
  (`258` total steps). The visible homepage / `/khala` / `/stats` counter is
  still exact-closeout based: it moves when `token_usage_events` is written,
  while raw chunks/traces can arrive continuously during the run.
- Post-18:58 delegated GLM/scheduler batch:
  `88f60cb924` is current deployed `main` as Worker
  `ba4ed1d7-f81a-4c82-b63c-2042b6bb4ad3`. Three Khala -> Pylon -> Codex
  assignments completed with exact `pylon-codex-own-capacity` usage rows:
  #6311 `assignment.public.khala_coding.chatcmpl_b2051745aee042d08f6d5348282215dc`
  (`2,905,828` tokens, `75` owner-only ATIF traces, `125` raw SDK
  events / `2,130,616` bytes), #6318
  `assignment.public.khala_coding.chatcmpl_da50ac2185dd438c82ea05192f27ac56`
  (`4,656,781` tokens, `88` owner-only ATIF traces, `147` raw SDK
  events / `4,990,553` bytes), and #6320
  `assignment.public.khala_coding.chatcmpl_9a27a65fa4f5434db8715bcb1288c91d`
  (`5,505,826` tokens, `107` owner-only ATIF traces, `182` raw SDK
  events / `2,421,576` bytes). Exact accepted-batch total:
  **13,068,435 Khala coding tokens**. Live `/api/public/khala-tokens-served`
  returned `285,030,535` at `2026-06-26T18:58:42.968Z`; live model mix returned
  `pylon_codex=188,831,219` tokens over `106` usage events, `70.894758%` of the
  30-day public mix. If the homepage only appears to "tick a little" between
  refreshes, that means no new large delegated closeout row landed in that
  interval; the exact assignment proofs above are the attribution source, while
  public counter movement remains aggregate and can include canaries or other
  agents.
- Post-19:24 supervised Pylon/Codex closeout and deploy:
  `713b715f8d` was deployed as Worker
  `3401d6b3-1c0a-4212-a4ce-0e923ca0a8ce` after `deploy:safe` verified
  `origin/main`, ran `check:deploy`, applied zero pending D1 migrations,
  verified zero pending migrations, built web assets, and uploaded the Worker.
  After another agent pushed `8f3927fffe` and this roadmap refresh landed as
  `88602de796`, latest `main` was deployed again through the same safe path as
  Worker `d3571d83-ecdb-40e0-8af4-08fe14f7ed1e`.
  Three newer Khala -> Pylon -> Codex assignments contributed exactly
  **9,122,991** Khala coding tokens to the `pylon_codex` model-mix bucket:
  a cheap counter fixture
  `assignment.public.khala_coding.chatcmpl_5c2ab15de13b4bdbbed1443971c92919`
  (`111,849` tokens, `12` owner-only ATIF traces, `17` raw SDK events /
  `4,362` bytes), #6320
  `assignment.public.khala_coding.chatcmpl_ec4be93966804054a0775f1099465a8f`
  (`2,090,306` tokens, `67` owner-only ATIF traces, `112` raw SDK events /
  `1,399,908` bytes), and #6318
  `assignment.public.khala_coding.chatcmpl_00d5a5d8fd7f42e384b6632dfa75d159`
  (`6,920,836` tokens, `136` owner-only ATIF traces, `232` raw SDK events /
  `2,581,488` bytes). Live model mix moved from
  `pylon_codex=188,831,219` to `pylon_codex=197,954,210`, exactly matching
  those three closeouts. Live `/api/public/khala-tokens-served` returned
  `294,220,135` at `2026-06-26T19:23:46.120Z`; the model-mix endpoint returned
  `totalTokens=275,481,209`, `pylon_codex=197,954,210` over `109` usage
  events, `71.857609%` of the 30-day mix. Homepage and `/khala` counters are
  exact-closeout counters backed by `token_usage_events`; raw Codex chunks and
  private traces can stream during the run, but the public token number does
  not continuously tick per SDK event. If the product requirement becomes a
  continuously moving public estimate, that is a separate projection from raw
  chunks and must be labeled as estimated until exact closeout tokens arrive.
- Post-19:32 supervised five-run Pylon/Codex batch:
  the counter baseline was `/api/public/khala-tokens-served=294,220,135` at
  `2026-06-26T19:32:57Z`. Five accepted Khala -> Pylon -> Codex assignments
  then closed out with exact `pylon-codex-own-capacity` rows totaling
  **8,427,241 Khala coding tokens**:
  #6323 `assignment.public.khala_coding.chatcmpl_6bb60a39d229481e9837bd08ce25f8a6`
  (`1,533,834` tokens, `51` owner-only ATIF traces, `86` raw SDK events /
  `294,230` bytes), #6323
  `assignment.public.khala_coding.chatcmpl_dac3ccd7c09f4591b42f2528ef43aff6`
  (`975,671` tokens, `52` owner-only ATIF traces, `87` raw SDK events /
  `1,282,405` bytes), #6311
  `assignment.public.khala_coding.chatcmpl_efb67743f96f44b9ad0078763e224fe6`
  (`1,571,076` tokens, `54` owner-only ATIF traces, `88` raw SDK events /
  `2,588,271` bytes), #6311
  `assignment.public.khala_coding.chatcmpl_bfe667383900474aa93652ad09e5d2e8`
  (`3,136,839` tokens, `81` owner-only ATIF traces, `136` raw SDK events /
  `1,742,620` bytes), and #6311
  `assignment.public.khala_coding.chatcmpl_8132762906104faaa5fa9a68e7920bec`
  (`1,209,821` tokens, `51` owner-only ATIF traces, `84` raw SDK events /
  `388,368` bytes). Live `/api/public/khala-tokens-served` returned
  `302,661,428` at `2026-06-26T19:45:48Z`, a `8,441,293` aggregate increase
  from the baseline. The `14,052` token difference is expected aggregate noise
  from other work/canaries; the assignment proofs above are the attribution
  source. The current integration folds in the reviewed #6323 evidence-audit /
  hallucinated-tool-call patch and the reviewed #6311 quota/capacity/
  replacement-reserve-prebake acceptance patch. Focused GLM pilot/readiness
  tests, `typecheck:web`, `typecheck:api`, and `check:deploy` passed before
  landing.
  The #6354 steering blocker from that batch is now fixed in Pylon: local
  no-spend runners refresh presence before claim, produce typed stale-presence
  recovery diagnostics when refresh fails, and count active local assignment
  runs in busy capacity. Remaining steering gaps are the plain `codex` account
  refusal versus `codex-2+`, and whether to add a labeled in-flight estimate to
  the public product counter.
- Current serving observability status: canonical scheduled
  `glm-pool-heartbeat` rows are now live. `5b11c6eaf5` is deployed as Worker
  `7ee46a76-f9ef-42cf-ac9f-31a472d2b3fb`; after one cron interval,
  `SELECT COUNT(*) FROM token_usage_events WHERE demand_source='glm-pool-heartbeat'`
  returned 20 rows, first `2026-06-26T17:16:19.000Z`, latest
  `2026-06-26T17:17:19.000Z`, total_tokens 0. Sampled rows covered all 10 GLM
  replicas with `watchdogStatus=healthy`, `healthStatus=ok`, and
  `modelsStatus=ok`. This proves the fleet-health readout slice, not the whole
  #6311 durability/non-Spot/watchdog/auto-replace/quota program.
- Deployment/live smoke after `2f467e3476`: `deploy:safe` verified local
  `main` matched `origin/main`, ran `check:deploy`, applied zero pending D1
  migrations, verified pending migrations were zero, built web assets, and
  uploaded Worker version `7460419e-2779-4e2e-a809-ec7646b6664a`. Live
  `https://openagents.com/`, `https://openagents.com/khala`, and
  `/assets/index-DZ-c2BZu.js` returned HTTP 200.
- Deployment/live smoke after `792ec3d56e`: `deploy:safe` verified local
  `main` matched `origin/main`, ran `check:deploy`, applied zero pending D1
  migrations, verified pending migrations were zero, built web assets, and
  uploaded Worker version `a8ed7b84-bfbd-40fe-b1e9-9af519e5d27f`. Live
  `https://openagents.com/`, `https://openagents.com/khala`, and
  `/assets/index-DZ-c2BZu.js` returned HTTP 200.
- Deployment/live smoke after `c7a86d7d06`: `deploy:safe` applied zero pending
  D1 migrations, verified pending migrations were zero, built web assets, and
  uploaded Worker version `12066869-4f81-4b32-ba41-ba3c50b07595`. Live
  `https://openagents.com/`, `https://openagents.com/khala`, and
  `/assets/index-DZ-c2BZu.js` returned HTTP 200.
- Deployment/live smoke after `5b11c6eaf5`: `deploy:safe` verified local
  checkout matched `origin/main`, ran `check:deploy`, applied zero pending D1
  migrations, verified pending migrations were zero, built web assets, and
  uploaded Worker version `7ee46a76-f9ef-42cf-ac9f-31a472d2b3fb`. Live
  `https://openagents.com/`, `https://openagents.com/khala`, and
  `/assets/index-DZ-c2BZu.js` returned HTTP 200. Public counter read
  `243,927,568` at `2026-06-26T17:18:18.953Z`; the two fresh Pylon/Codex
  closeout rows at `17:09Z` contributed `6,604,098` exact Khala-attributed
  tokens, while later small ticks were background heartbeat/canary rows.

---

## Phase 0 — STOP THE BLEEDING (P0; the OpenCode wedge is hard-down NOW)

Real external users (via OpenCode) currently get ~100% `provider_error` on tool calls.
This is the only phase that is an active outage. Do it first.

1. **#6310 — GLM tool-calling broken (P0).** Tool requests to the primary GLM lane return
   `provider_error` ~100% of the time; every OpenCode coding request fails. Two acceptable
   resolutions (do the faster one immediately, then the durable one):
   - **Immediate mitigation:** route tool-bearing / coding requests OFF GLM to a working
     tool-caller (DeepSeek-V4 / a healthy GPT-OSS-120B / frontier), keep plain chat on GLM.
   - **Durable fix:** correct the GLM-5.2-REAP vLLM tool path — the `--tool-call-parser`
     value ↔ `--reasoning-parser` interaction ↔ chat template for this checkpoint (the
     parser is set but errors on tool requests).
   - **Done when:** a scripted OpenCode-style tool loop round-trips real `tool_calls` with
     **0 `provider_error`** over N consecutive requests.
   - **Status (2026-06-26): CLOSED.** Treat as a regression gate, not the current
     active work item.
2. **#6319 (fallback-chain repair slice) ‖ parallel with #6310.** The fallback chain is
   itself broken — **GPT-OSS-120B (fallback #2) returns 404**, GPT-OSS-20B (#3) returns
   empty — so GLM overflow degrades two dead hops before a serving lane. At minimum, in
   Phase 0: repair/replace the dead lanes + treat empty content as a failure so a 200 is
   never an empty/no-tool response. (Full #6319 program continues in Phase 1.)
   - **Status (2026-06-26): CLOSED.** The full reliability issue is also closed;
     downstream work should preserve these checks as serving regressions.
3. **#6323 — pilot `nvidia/GLM-5.2-NVFP4` (full 753B) on the 8× host ‖ parallel with #6310,
   as a candidate FIX for it.** Our REAP-504B already uses the canonical `glm47`/`glm45`
   parsers, so #6310 is the pruned checkpoint, not config. NVIDIA's full 753B NVFP4
   (near-FP8, agentic-tool-use-validated, MIT) fits our one `g4-standard-384` 8× RTX PRO 6000
   host (TP-8, ~381 GB weights in 768 GB). Deploy it there and test: does it tool-call clean
   where REAP `provider_error`s? If yes, it's both the #6310 fix and the quality upgrade —
   route the GLM coding lane to it, keep REAP-504B on the 4× hosts. (Eval:
   `docs/inference/2026-06-26-nvidia-glm-5.2-nvfp4-evaluation.md`. Scaling the full model
   beyond one host depends on 8× Blackwell quota/capacity — #6311.)
   - **Decision artifact landed (2026-06-26): GO for a bounded single-host pilot.** Feasible
     today on the one `g4-standard-384` 8× host (NVFP4-capable Blackwell, ~381 GB weights +
     unquantized shared expert + KV in 768 GB at TP-8); **not** feasible on the 4× hosts. The
     full model is a **credible #6310 fix** (same `glm47`/`glm45` parsers as REAP → fault is
     the pruned checkpoint, not config; full model is agentic-validated). Pilot = isolated
     endpoint on the 8× host with the card's exact flags + a measured `--max-model-len` (96 GB
     cards, not B200/B300 — prove the KV ceiling), primary test = OpenCode tool loop +
     #6310 repro with **0 `provider_error`**, then quality + tok/s vs REAP's ~47 tok/s.
     **Rollback is trivial** (separate endpoint; live `openagents/khala` stays on REAP the
     whole pilot). Owner / serving-lane executes the run; this lane stayed **doc/decision-only**
     (no live fleet/gateway/Pylon changes). Full plan + success criteria + conditional routing
     precedence: the "Decision artifact (#6323)" section of the eval doc.
   - **Status (2026-06-26): OPEN.** The delegated Pylon/Codex retry produced an
     executable owner-armed pilot harness, now merged in `c7a86d7d06` and
     deployed. The current public-safe operator path writes scrubbed result,
     summary, bundle, and README files under `--output-dir`, and prints the
     exact owner-armed command with redacted private values. The current
     integration also requires `KHALA_GLM_NVFP4_BOOT_LOAD_EVIDENCE_REF` so a
     pilot pass cannot be claimed without public-safe boot/load evidence from
     the isolated 8x host. This bounded local environment is still missing
     `KHALA_GLM_NVFP4_PILOT_ARM`, `KHALA_GLM_NVFP4_ENDPOINT_URL`,
     `KHALA_GLM_NVFP4_ENDPOINT_REF`, `KHALA_GLM_NVFP4_OWNER_APPROVAL_REF`, and
     `KHALA_GLM_NVFP4_BOOT_LOAD_EVIDENCE_REF`, so no pilot pass is claimed.
     The latest hardening also records failed vLLM/SGLang boot attempts as
     bounded serving-stack failure codes plus public evidence refs while
     redacting unsafe refs and keeping failed-before-endpoint attempts blocked.
     Next action is the actual isolated 8x-host serving-stack fix and pilot run
     with owner endpoint/approval/boot-load refs, then a measured tool-call/
     quality/max-context/throughput decision, not another planning doc.

## Phase 1 — Reliable serving foundation

Make the fleet trustworthy before pushing load through it.

3. **#6319 — reliability hardening program (full).** Per-replica health + circuit-breaker,
   empty-fallback-as-failure, SLO-based shedding, request hedging, and failure telemetry
   (provider_error / empty / fallback / invalid-tool rates) so the next breakage is visible
   without a user mailing screenshots. → continues from Phase 0 slice.
   - **Status (2026-06-26): CLOSED.**
4. **#6313 — real OpenRouter fallback lane ‖ parallel.** No OpenRouter inference lane
   exists today (only resale/identity refs + a key). Build the adapter + registration +
   plan entry, wired as a real fallback tier with fail-over tests. Gives the chain a
   working terminal hop.
   - **Status (2026-06-26): CLOSED.**
5. **#6311 — durable (non-Spot) GLM fleet + keep-warm ‖ parallel.** All 10 replicas are
   Spot; 8 lack the STOP-watchdog. Add keep-warm on every replica, multi-region
   auto-replace, an on-demand reserve, and the us-central1 quota increase. (Cross-refs
   hydralisk #95 durable host, #99 prebake-weights image.)
   - **Status (2026-06-26): OPEN.** Partial route/projection work has landed, but
     the issue remains broad. The Pylon/Codex diagnostic slice for canonical
     scheduled-skip GLM pool heartbeat rows landed in `856fc636d0`, the bounded
     probe slice landed in `36ee76689c`, the cron-ledger-proof slice landed in
     `7f94c0556a`, the D1-wrapper/persistence-order proof landed in
     `5b11c6eaf5`, and the typed fail-closed acceptance projection landed in
     `88f60cb924`. Live `glm-pool-heartbeat` rows are proven on the deployed
     Worker, and live readiness now reports `status:"degraded"` with materially
     recovered serving capacity and durable acceptance `blocked` (`8` ready,
     `0` reclaimed, `warmOrReadyMaxInflight:9`). This follow-up keeps
     `bun run --cwd apps/openagents.com/workers/api glm-fleet:durability` as
     the headless evidence bundle and requires a public-safe forced Spot STOP
     recovery ref before all-replica watchdog can go complete. Current next
     action is the real durability scope: non-Spot/reserve owner decision,
     all-replica STOP-watchdog/keep-warm, forced STOP recovery proof,
     multi-region auto-replace, and quota tracking.
   - 2026-06-27T18:00Z refresh: serving capacity materially recovered, but
     public readiness still reports `status:"degraded"` and #6311 is still not
     accepted. The readiness/operator summary now reports `8` ready replicas,
     `0` reclaimed, `warmOrReadyMaxInflight:9`, and a
     `servingCapacitySummary`; it must not emit
     `recover_reclaimed_replicas` while reclaimed count is zero. Remaining
     blockers are public forced-stop recovery evidence, capacity-floor owner
     decision evidence, multi-region reserve/prebake auto-replace evidence, and
     quota request tracking.
   - Earlier 2026-06-27T03:34Z refresh: the live fleet-health readout was degraded, not just
     acceptance-blocked (`10` total replicas, `2` ready, `8` reclaimed,
     `warmOrReadyMaxInflight:2`). The headless durability
     summary names the reclaimed public replica refs and emits typed
     `operatorActionItems` so the operator can target recovery and missing
     durability proof refs without opening the full bundle. Khala -> Pylon ->
     Codex proof for this public-safe slice:
     `assignment.public.khala_coding.chatcmpl_8c5d5c98f2544913aea50facfb81ee30`,
     `2,780,444` exact own-capacity tokens.
6. **#6259 + #6315 — green end-to-end GLM-serving smoke. → after #6310.** Get the
   Khala→GLM verification smoke passing for real (served-worker disclosure + counter
   increment); #6315 is the receipt-ref fix for the zero-debit operator-exempt token.
   This is the regression gate the rest of the work leans on.
   - **Status (2026-06-26): CLOSED.**

## Phase 2 — Maximize throughput (tokens/sec)

7. **#6320 — inference-engineering throughput optimizations. → after #6319.** THE lever:
   replicas run single-flight, so continuous batching is OFF — raise `--max-num-seqs` to
   unlock it (a multiple, not a percent), then stack chunked prefill + engine-side prefix
   caching + speculative/MTP decode + eval-gated quantization. Biggest tok/s win in the
   whole roadmap; do it before stress-testing so you measure the real ceiling.
   - **Status (2026-06-26): OPEN.** Bounded sweep metadata landed in
     `85ca837413`, and the delegated rollout-recommendation/flag-selector slice
     landed in `a8c12aff42`. A newer owner-armed rollout artifact landed in
     `03b6ffa094`, and measured rollout evidence validation landed in
     `88f60cb924`. The fail-closed rollout evidence checklist landed in
     `7a73ab8d95` and is included in the current
     `d3571d83-ecdb-40e0-8af4-08fe14f7ed1e` deployment. The 2026-06-27
     Khala -> Pylon -> Codex slice
     (`assignment.public.khala_coding.chatcmpl_a46b5dc9bb4249df8809ac46a23948ea`,
     `3,672,787` exact own-capacity tokens) adds typed operator acceptance
     evidence for live vLLM flags, before/after TTFT, progress status/percent,
     and the exact remaining checks that block #6317/#6312. A follow-up now
     consumes #6311's recovered-capacity-but-degraded readiness shape
     (`readyReplicaCount:8`, `reclaimedReplicaCount:0`,
     `warmOrReadyMaxInflight:9`) by surfacing the serving capacity summary and
     remaining durability blockers in typed throughput readouts. Live engine
     flags plus measured throughput lift have not happened, and #6317/#6312
     remain blocked until #6311 acceptance is complete. A 2026-06-27T07:29Z
     delegated audit on current `main`
     (`assignment.public.khala_coding.chatcmpl_7deb9e73b6e9428b9d32dc9e8f3654b8`,
     `1,644,352` exact own-capacity tokens) made `0` edits and passed the
     focused verifier, so this is now explicitly an infra/measurement rollout
     blocker rather than a hidden local code gap.
     A 2026-06-27T08:53Z delegated audit after the fresh #6311 proof
     (`assignment.public.khala_coding.chatcmpl_56e7dd24d9a248afbc0533e022d260cd`)
     also made `0` edits, ran `28` commands, and passed the focused throughput
     verifier. Assignment-scoped proof reported `2,019,853` exact own-capacity
     tokens (`2,012,762` input, `7,091` output, `527` reasoning, `1,853,952`
     cache-read), `46` owner-only ATIF traces, and one owner-only raw Codex
     archive containing `74` events / `2,009,588` bytes. Public counter baseline
     was `442,814,560`; post-closeout read was `444,901,088`, with assignment
     proof remaining the exact attribution because the public scalar is global.
8. **#6318 — external-wins admission/priority scheduler. → before #6317.** Internal load
   must be preemptible and yield to external demand. This MUST land before any continuous
   stress so the stress harness can never starve a real user.
   - **Status (2026-06-26): OPEN.** Admission/attribution slices landed and were
     deployed. The standalone delegated preemption registry was rejected as
     unwired; the latest wired route-level demand-class/admission proof landed
     in `5b11c6eaf5`, typed preemption evidence propagation landed in
     `d373eaee69`, and typed stress-yield/preemption metadata landed in
     `88f60cb924`. Production abort/preemption wiring landed in `713b715f8d`
     and is included in the current
     `d3571d83-ecdb-40e0-8af4-08fe14f7ed1e` deployment; the issue remains the
     hard gate before stress load until actual live saturation/preemption proof
     exists. The current dispatch-gate fix is reliability hardening for
     Khala/Pylon assignment creation only; it now includes staged diagnostics
     and explicit-target registration recovery for typed Pylon/Codex dispatch,
     but it does not satisfy the #6318 live scheduler proof.
9. **#6317 — continuous max-capacity stress/saturation harness. → after #6318, #6320.**
   The self-driving load that saturates the fleet, ramps concurrency to the ceiling, and
   auto-backs-off on external pressure.
   - **Status (2026-06-26): OPEN; blocked by #6318 and live #6320 rollout.**
     The verified preparatory Pylon/Codex patch was integrated as `792ec3d56e`
     and is deployed. The 2026-06-27 local prep adds canonical stress/real-sweep
     attribution and latency/per-replica report rollups. Do not run continuous
     stress until the external-wins guard is real and the throughput rollout has
     live measured proof. A 2026-06-27T09:15Z delegated prep slice now makes the
     arming gate explicit: `externalWinsProofStatus` must be `accepted` and the
     #6320 readout must expose `canStartIssue6317Stress:true`; evidence refs by
     themselves do not arm traffic.
10. **#6312 — max tokens-per-second benchmark. → after #6317.** The decision-grade
    aggregate-throughput number, read from the harness (concurrency sweep, per-replica +
    aggregate tok/s, TTFT, P50/P90/P99, saturation point, in-cloud vs WAN).
    - **Status (2026-06-26): OPEN; blocked by #6317.**

## Phase 3 — Autonomous operation

11. **#6321 — Artanis fleet-overseer automation. → after #6317, #6318, #6319.** The
    autonomous control loop (on `artanis-administrator-tick`, approval-gated): watches fleet
    health + throughput + external demand, orchestrates the stress load (start/scale/
    back-off keyed on external pressure), and triggers heal/scale/quarantine — money +
    destructive actions stay owner-gated via `artanis-approval-gates`. This is the layer
    that runs Phases 1–2 continuously without a human.
    - **Status (2026-06-27): OPEN; blocked by #6318/#6317 and should incorporate
      the already-closed #6319 reliability signals. Safe prep now models
      `fleet_mutation` as a rollback-required Artanis risky action, but no
      fleet-overseer tick, stress traffic, scale-out, or quarantine is armed.**

## Phase 4 — Prove quality (now that serving is reliable + instrumented)

12. **#6253 — replicate + beat GLM-REAP's 69.1% on Terminal-Bench 2.0. → after Phase 1.**
    The competitive goal: a decision-grade Khala-routed run (not the raw-GLM pilot),
    inference-method comparison, beat the baseline.
    - **Status (2026-06-26): OPEN.** Black-box runner and bounded public-safe
      measurement path landed in `da472748c5`. A separate agent may own a live
      Harbor run; do not interrupt it. Remaining work is decision-grade
      replicate-and-beat evidence.
13. **#6307 — owner-armed real sweep: first `decisionGrade:true` Khala-vs-Fireworks/Vertex
    report ‖ parallel.** The minimum decision suite, run for real over realistic traffic.
    - **Status (2026-06-26): OPEN.** Harness/seam and Khala-side run landed in
      `ff89ecf498`; spendful external lanes remain owner-gated.
14. **#6308 — external head-to-head (recurring quality bar). → after #6307.** Khala vs the
    tools/models developers would otherwise use, on our axes (cost-per-accepted-outcome,
    verified-rate).
    - **Status (2026-06-27): OPEN.** Publication layer landed in `2f2d011c64`;
      this safe-prep pass adds stored publication freshness fields so stale
      snapshots cannot masquerade as current. Decision-grade recurring data still
      depends on #6307.
15. **#6309 — gym benchmark ladder as a recurring leaderboard. → after #6307.** Big Pickle
    → free models → paid frontier, published and re-scored on every change.
    - **Status (2026-06-27): OPEN.** Ladder publication/projection layer landed
      in `1accb3573b`; this safe-prep pass adds stored publication freshness
      fields so stale ladder snapshots cannot masquerade as current.
      Decision-grade rung data still depends on #6307.

## Phase 5 — Drive adoption (the demand side; GTM #6303)

16. **#6305 — OpenCode → Khala verification checklist + publish. → HARD-after #6310.** Do
    NOT publish the OpenCode recipe until tool-calling actually works; publishing a broken
    coding agent burns the wedge. This is the first external "point your tool at us" win.
    - **Status (2026-06-26): CLOSED.**
17. **#6306 — next ecosystem recipes (Aider, Cline/Continue, Vercel AI SDK, LiteLLM,
    LangChain). → after #6305.** One tool at a time, each with its test checklist.
    - **Status (2026-06-26): CLOSED.**
18. **#6303 — GTM push tracking (umbrella).** Closes when 16–17 + the Phase-4 benchmarks
    land; keep it updated as the demand-side scoreboard.
    - **Status (2026-06-27): OPEN.** Recipe work and benchmark publication
      layers are present, and the closeability audit found no local code gap to
      patch. Closure still waits on decision-grade benchmark/recurring evidence
      from #6253/#6307/#6308/#6309 plus the demand/adoption scoreboard.

---

## The single sequence (flat list)

Historical full sequence:

`#6310` [closed] ‖ `#6323`(full-model candidate fix) →
`#6319(chain-repair)` [closed] → `#6319(full)` [closed] ‖ `#6313` [closed] ‖
`#6311` → `#6259/#6315` [closed] → `#6320` → `#6318` → `#6317` → `#6312` →
`#6321` → `#6253` ‖ `#6307` → `#6308` ‖ `#6309` → `#6305` [closed] →
`#6306` [closed] → close `#6303`.

Remaining active sequence after the 2026-06-26 ~19:24Z refresh:

`#6323`(run the owner-armed full-model pilot with the landed harness) ‖
`#6311`(finish durability/non-Spot/reserve/quota now that live heartbeat/readiness acceptance exists) →
`#6320`(live engine rollout + measured lift) →
`#6318`(prove the now-wired external-wins preemption path under live saturation) →
`#6317`(run real continuous stress only after #6318/#6320 live proof) → `#6312` → `#6321` →
`#6253`(decision-grade replicate/beat) ‖ `#6307`(owner-armed full comparison) →
`#6308` ‖ `#6309`(recurring evidence) → close `#6316` / `#6303`.

Running **continuously alongside** the above (not gated by it), per the operating
model section: **#6355** (operator tooling now closed and available through
`pylon khala burndown` to *drive* the sequence), **#6356** (trace review now
closed as an admin endpoint/runbook),
**#6357** (unsupported-request triage), and **#6358** (counter health), with
**#6354** now closed as the loop's presence/busy-load prerequisite. #6356/#6357
feed new issues back into the sequence as testers surface gaps. The end state is
**Artanis owning this loop autonomously** (see the Artanis section below / #6321).

(#6323 remains at the front because it could still become the quality/tool-call
upgrade path even though #6310 itself is closed. If the full model tool-calls
cleanly and beats REAP quality/throughput expectations, route it as the premium
GLM coding lane and leave REAP-504B on the 4x hosts.)

## Dependency rationale (the non-obvious edges)

- **#6318 before #6317** — never run a saturation load without the external-yield guard, or
  you DoS your own paying users.
- **#6320 before #6312/#6317** — measuring throughput before turning on continuous batching
  measures the wrong (artificially low) ceiling.
- **#6310 before #6305** — the OpenCode publish is gated on a working tool-caller.
- **Phase 1 before Phase 4** — benchmark numbers are only decision-grade over reliable,
  realistic serving; benchmarking a flaky lane produces noise, not receipts.
- **#6321 last in the serving track** — automation should orchestrate systems that already
  work (the harness, the scheduler, the hardening), not paper over their absence.

## Notes

- Master trackers: **#6316** (serving, Phases 0–3) and **#6303** (demand, Phases 4–5).
- Cross-repo: hydralisk **#95** (durable host) and **#99** (prebake-weights image) back
  #6311; coordinate the fleet-side work there.
- The owner-gated honesty bar still applies: any published benchmark number must come from
  the owner-armed real seam over realistic traffic (`decisionGrade:true`); internal
  dogfood/stress tokens stay segmented (#6298 demand tags) and out of external metrics.
- Khala -> Pylon -> Codex worker status at refresh: the latest supervised batch
  completed across `codex-2`, `codex-3`, and `codex-4`, with simultaneous runs
  on both `codex-2` and `codex-3` across the broader stress sequence. Accepted
  work has exact owner-capacity token rows, owner-only ATIF traces, owner-only
  raw-event refs, and streamed raw chunks. Future launches should record
  assignment refs immediately, verify
  `pylon khala proof --assignment-ref ... --json`, and compare exact token rows
  instead of relying on public counter movement. The #6354 presence/busy-load
  steering gap is fixed in Pylon. The next steering gaps to settle are (1) why
  the plain `codex` account refused one assignment while `codex-2+` succeeded,
  and (2) whether the public product should add a clearly labeled in-flight
  estimate from streamed SDK chunks while preserving exact public accounting at
  assignment closeout.
- 2026-06-26 `23:24Z` stale-lease recovery proof: patched local Pylon
  `pylon.33afd48282a649047e3a` reclaimed abandoned accepted no-spend assignment
  `assignment.public.khala_coding.chatcmpl_fd33103f7b4349218f9b0760e8ca5632`
  with stale closeout `assignment.closeout.cfd5d6dd9b2a6140f361a836`, then ran
  two same-Pylon assignments concurrently to accepted closeout:
  `assignment.public.khala_coding.chatcmpl_6d190807e87c4a558dac39a098a9d268`
  (`161,832` exact tokens, `15` owner-only ATIF traces, `22` raw SDK events)
  and `assignment.public.khala_coding.chatcmpl_a2bd2121c00d4a2f8e63eb26f48f9148`
  (`128,873` exact tokens, `12` owner-only ATIF traces, `17` raw SDK events).
  After closeout, `provider go-online` again reported Codex `available=2`,
  `ready=2`, `busy=0`, `queued=0`; treat this as the current proof that stale
  local no-spend leases no longer poison advertised capacity.
