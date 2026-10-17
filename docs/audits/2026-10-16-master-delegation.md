# Master delegation: every open issue, where it stands, and who takes it

Written 2026-10-16 21:50 CDT (2026-10-17 02:50 UTC). It covers all 48 open issues and every comment on them, read after the owner's STOP and consolidate order at about 21:35 CDT.

This document governs how the remaining work is split. Read it with the [big-bang release brief](2026-10-16-coder-1.0-big-bang.md), which sets the goal and the rules. Where the two disagree, the brief wins on policy and this document wins on status and assignment.

**Goal:** close every open issue by **Saturday 2026-10-17, 21:00 CDT** (2026-10-18 02:00 UTC). Then the release gate runs through Sunday, and Coder 1.0.0 ships on **Monday 2026-10-19**.

## How to use this document

1. Pick a lane in [Lanes](#lanes). Each lane is one agent, with the issues, files, and order it owns.
2. Read every issue in your lane in [Every open issue](#every-open-issue). Then open the issue's latest handoff comment on GitHub. The comments hold exact commits, check counts, and paths.
3. Claim before you start: `openagents issue claim N`, then `scripts/dev/issue-board.sh claim N`.
4. Stay inside your lane's files. If you need a file another lane owns, ask that lane, or make an additive change and say so in both issues.
5. **Close the issue when its code is on `main` and its own checks pass** (owner, 2026-10-16). Don't wait for a deploy, a live check, or a test-account run: add it to [After-close checks](#after-close-checks), say so in the closing comment, and close with `scripts/dev/issue-board.sh close N "..."`. Owner-only steps still go in `NEEDS_OWNER.md`. Where an issue below says "Remains: … live run …" or "deploy", that part moves to After-close checks.

## Where everything stands

### At the stop

- **48 issues were open at the stop** (47 after #11788 closed). 27 closed since 2026-10-16 18:00 CDT, including all of Agent Lab (#11914–#11921), Foreman (#11926, #11927), the app rename (#11899), the iPhone crash (#11889), memory suspension (#11812), Labs L2 and L3 (#11873, #11874), and the SDK policy issues (#11819–#11822).
- **Every handoff is on `main`.** The stopped agents pushed their work, posted a handoff on each issue, and released their claims. Unfinished code is saved in three ways:
  - **Checkpoint branches on GitHub:** `codex/11878-jobs-placement`, `codex/11878-sdk-worker`, `codex/11878-cloud-launcher`, `codex/11876-labs-sdk`, `codex/agent-lab-model-use-20261017`, `codex/11817-enrolled-adapter` (empty, starts at `e6f468c332`), and `codex/11812-11817-computers`. Don't delete them.
  - **Saved patches:** #11877's checkpoint is now applied and its working patch file is removed. The SDK archive `docs/sdk/evidence/grid-stop-checkpoints-2026-10-17.tar.gz` and its JSON index retain five overlapping snapshots; don't apply them blindly.
  - **Patches inside issue comments:** the `openagents agents` command and the #11935 guard (in a #11934 comment), and the phone side of Our cloud (in a #11852 comment). Neither has been compiled.
- **The disposable SPACE test host is gone.** `oa-space-spike-11812` and its network were deleted at about 21:40 CDT ([teardown receipt](../cloud/evidence/2026-10-17-space-spike-teardown.json)). Any handoff path under `/var/lib/space-spike/home/...` or `~space-spike/...` no longer exists. That includes remote targets `agent2`, `agent3`, and `agent6`, the `labs-11877` checkout, and the #11876 and #11878 remote logs. The saved hashes and in-Git archives remain.
- **Disk:** the Mac reached 315 MiB free during the night, which stalled every lane. It has 139 GiB free now. See [Shared rules](#shared-rules).

### Still running (don't stop these)

| What | Where | Owner lane | Notes |
| --- | --- | --- | --- |
| A Codex session on SPACE (#11813, #11814, #11936) | sessions `codex:01a14140` and `codex:01a14134`; hosts `oa-space-service-control`, `oa-space-service-worker`, `oa-sdk-build-11800`, `oa-sdk-credentials-11813`, `oa-space-spike-11813` | 6 | It pushed `916a8a838c` at 21:44 CDT and claimed #11936 at 21:45, after the STOP. Treat it as the live owner of those three issues. Confirm with the owner before you take any of them. |
| Signet observers for #11862, #11863, #11770 | `oa-lnd-builder` (us-central1-a); status with `bash /tmp/sigstatus.sh` | 10 | Read only. Never add funding or restart a controller. |
| LayerTree primary TDX tree, 24-hour soak, scheduled deletion | `oa-layertree-signet-tdx`, mirrors `oa-layertree-mirror-0/1/2` | 10 | The clean window can first qualify at **2026-10-17 13:34:53 UTC (08:35 CDT)**. The observer then deletes the VM itself. |
| #11794 followers | `oa-lnd-builder`, `~/layertree-followers-11794-d9e41513b5/` | 10 | Stop time `1792877280` (Unix). |
| Production web | `coder-web-51e2d77b5a-20261017033819` at 100% | 0 | Image `sha256:51e2d77b…202a7` from `646d5fb6bd`: CAD, the agents door, and cloud inference. Production smoke passed 66/0/2. |
| Staging web | `openagents-web-1-staging-646d5fb6bd-032744` at 100% | 0 | Image `sha256:51e2d77b…202a7` from `646d5fb6bd`: CAD, the agents door, and cloud inference. Standard staging smoke passed 99/0/2. |
| Production Computers and Boat | Computers `c50225e11b`, Boat `76d096ef26` | 6 | From #11815's last deploy. |

### Decisions already made (don't reopen)

- **Harness** (`abd722e4d5`, #11896): `crates/coder-delegate` calls `openagents-harness` in `apps/openagents/crates/harness`. The app harness owns engine arguments, protocols, resume, steering, approvals, and event decoding. Coder keeps policy, leases, deadlines, budgets, and its command `Wrap` boundary. Delete the terminal's duplicate Claude Code, Codex, and OpenCode drivers in the same push. Keepers (#11930) also go in `openagents-harness`.
- **Chats** (#11895): one chat list everywhere, one home per chat, and "where it runs" switched by a linked continuation. Computer sessions show on the web as summary rows, with the transcript opt-in.
- **Cloud computers:** every cloud computer is a Computers API computer, reached only through `openagents::computers` with the signed-in session. Never call Boat, SPACE, or GCE directly.
- **Labs:** the SDK and domain data stay UI-free, and `lab-ui` owns the shared Rust Native card. #11885 becomes native Labs in Coder, not a bridge.
- **#11854 and #11817 file boundaries:** listed under lanes 7 and 8.

## Lanes

Eleven lanes. Lanes 0 to 9 need a full agent each. Lane 10 is mostly waiting, so one agent can tend it with wakeups. Before you start an agent, run `openagents capacity check claude`.

| Lane | Issues, in order | Critical for the binary? |
| --- | --- | --- |
| 0. Release and deploys | the release; deploy steps for #11875, #11934, #11852; board and `NEEDS_OWNER.md` | yes |
| 1. Harness and chats | #11894, then #11898, then #11823 | yes |
| 2. Sessions on the web | #11897 | no |
| 3. Keepers and moving agents | #11930, #11931, then #11932, #11933 | partly (#11930 changes the app binary) |
| 4. Labs platform | #11878, #11876, #11877, #11875 (live checks), then #11885 | yes (#11885) |
| 5. Labs kinds and app views | #11881, #11882, #11880, then #11883, #11884, #11886 | no |
| 6. Computers and SPACE | #11936, #11814, #11813, #11815, then #11816, then #11800 | no |
| 7. Host authority | #11817, then #11911 | yes |
| 8. Phone | #11852, #11854, #11853, then #11840 | no |
| 9. CLI, Agent Lab, web fixes | #11935, #11934, #11929, #11925, #11928, #11727 | yes (#11934, #11935) |
| 10. Signet and LayerTree | #11863, #11770, #11862, #11762, #11793, #11794, #11799, #11779 | no |

### Lane 0: release and deploys (one agent, the only one that deploys or releases)

**Taken 2026-10-16 22:04 CDT** by Codex working deploys and releases for #11875, #11934, #11852, #11936, and the after-close follow-up #11938 in `~/work/openagents-bang-lane`.

- **Owns:** every web deploy (staging and production shifts), every release candidate cut, the final 1.0.0 publish, the project boards, and `NEEDS_OWNER.md` hygiene. Other lanes ask lane 0 to deploy. They never shift traffic themselves.
- **Tonight:**
  1. Run `scripts/dev/project-status-sync.sh 22` and `19`. Fix the stale assignees: #11934, #11840, #11762, #11862, #11863, #11770, #11794, and #11799 show AtlantisPleb with no live claim. Leave #11813, #11814, and #11936 alone (lane 6 is live).
  2. Take the next production deploy from current `main`. It carries #11875's CAD platform, the #11934 agents door, and `4a796e8ff8` for #11852. Lane 0 runs the CAD staging check from After-close checks, promotes the tested image, runs the production smoke, and works through the remaining live checks.
  3. Remove the patch files from `docs/audits/` once lane 4 has applied #11877's patch. A `.patch` file isn't a document.
- **Saturday evening:** cut `1.0.0-rc.23` from `main` once lanes 1, 4, 7, and 9 have landed. Run the gate the brief describes, and cut later candidates as fixes land.
- **Monday:** follow the brief's re-cut steps. Reinstall every copy of the withdrawn 1.0.0.
- **Next-release rows that must ship in 1.0.0** (`docs/coder/next-release.md`): the #11771 memfd credential handoff (the source fix is closed, and the binary must carry it), both `api.openagents.com` address moves (check `deploy/production/api-url-map.yaml` first), #11823, #11911, and #11817. Remove each row as its work lands.

### Lane 1: harness and chats (critical path)

**Taken 2026-10-16 22:21 CDT** by Codex working #11894, #11898, #11823 in `~/work/openagents-chat-bridge`.

**Released 2026-10-16 22:16 CDT** by Codex after landing #11894 prerequisites `d944461951` and `b2ac4a07c6`. #11894, #11898, and #11823 remain open and free; see their handoff comments. The `~/work/openagents-push-lane` worktree is removed; targets `~/work/openagents-target-1` and `~/work/openagents-target-1-app` stay warm.

- **Owns:** `crates/coder-delegate`, `crates/coder` (turn and delegate door), `crates/chat-home`, the Coder-facing API of `apps/openagents/crates/harness`, the continuation routes in `crates/openagents-web`, and `crates/openagents-sdk` history and continuation methods.
- **Order:** #11894 first, because the bridge unblocks #11898, the keepers in lane 3, and terminal chats in the one list. Then #11898, then #11823.
- **Watch for:** `Wrap` is borrowed and isn't `Send`, `Sync`, or `'static`. Apply it before each process spawns, or add an explicit launch broker. Never bypass it. The harness must build in both Cargo workspaces (root on Rust 1.97.1, `apps/openagents` on 1.98.1).

### Lane 2: sessions on the web

**Done 2026-10-16 23:30 CDT** by a Claude Code subagent. #11897 is closed (`f00ff8f121`, `59251e460b`); its deploy and live check are in After-close checks. The `~/work/openagents-lane2` worktree is removed.

- **Owns:** a new session index in `crates/openagents-web` (history list, summary page) and the publishing hook in `apps/openagents/crates/engine` or `crates/coder-sync`'s registry read.
- **Order:** #11897 alone. It builds on `cca7f94ebe`'s `chat-home` types without changing them. Ask lane 1 before you edit `chat-home`.

### Lane 3: keepers and moving agents

**Taken 2026-10-16 23:31 CDT** by a Claude Code subagent working #11931 in `~/work/openagents-lane3b`.

**Released 2026-10-17 00:05 CDT** by a Claude Code subagent. #11930's keeper layer is on `main` (`16629755e6`); Claude Code and Codex attach remain, see the issue. #11931 wasn't started: it needs a test host, because a LaunchAgent can't be installed on the owner's Mac. Target `~/work/openagents-target-lane3-app` stays warm.

- **Owns:** a new keeper module in `apps/openagents/crates/harness` (additive; lane 1 owns the existing API), engine adoption in `apps/openagents/crates/engine`, the LaunchAgent and systemd service plumbing, and the move-plan store and UI.
- **Order:** #11930, then #11931. #11932 waits on #11852 (lane 8), #11813 (lane 6), and #11898 (lane 1). #11933 comes last. Spec: [move running agents](../desktop/move-running-agents.md).
- **Risk:** #11932 and #11933 sit at the end of three other lanes' chains. If those slip, ship #11930 and #11931 and close the other two with their remaining steps recorded. Don't hold the release for them.

### Lane 4: Labs platform

**Taken 2026-10-16 22:03 CDT** by Codex working #11878, #11876, #11877, #11875, #11885 in `~/work/openagents-bang-codex`.

- **Owns:** `crates/lab-kit`, `crates/lab-jobs`, `crates/lab-store`, `crates/lab-api`, `crates/lab-ui`, `crates/openagents-sdk/src/labs*`, `crates/openagents-web/src/{labs,lab,cad_web,history_api}.rs` and the cards, `examples/sdk-labs/`, and the Labs tools in Coder for #11885.
- **Order:**
  1. #11878: rebase `codex/11878-jobs-placement` (`0021970e87`) onto `main`. It already contains the SDK worker and cloud launcher slices, so don't cherry-pick them twice. Regenerate the lock, rerun the combined checks, fix the `Uncertain` finish fix's test, and deploy through lane 0.
  2. #11876: fix the external build (root `[patch.crates-io]` Truck pins don't reach a consuming workspace), then the live example.
  3. #11877: apply the retained patch in a worktree and finish it.
  4. #11875: the live CAD checks on staging, then production after lane 0 promotes.
  5. #11885: native Labs in Coder. Coordinate with lane 1 on `crates/coder*` files.
- **Coordinate:** lane 5 depends on your `LabKind` and job hooks. Keep them additive after #11878 lands.

### Lane 5: Labs kinds and app views

**Taken 2026-10-16 23:31 CDT** by a Claude Code subagent working #11880, then #11883 in `~/work/openagents-lane5b`.

**Released 2026-10-16 23:04 CDT** by a Claude Code subagent. #11881 is closed (`e37d66493e`, `ade5341924`). #11882 is open and free: the kind and migration landed in `d08cf1dc58`; the simulate handler, registration, and the live run remain. The worktree `~/work/openagents-lane5` is removed; target `~/work/openagents-target-agent2` stays warm.

- **Owns:** `crates/game-lab`, `crates/ship-lab`, `crates/robot-lab`, `crates/video-lab`, their web adapters, and the Lab rows in `apps/openagents` (GPUI `crates/ui/src/lab_card.rs`, mobile layout, iOS cells).
- **Order:** #11881 and #11882 as soon as #11878 lands. #11880 after #11877 and #11878. The app views #11883, #11884, and #11886 after #11876 and #11877. Each view renders `lab-ui`'s Rust Native card; don't write a card per shell.

### Lane 6: Computers and SPACE (the live Codex session)

- **Owns:** `crates/coder-working-computer`, `crates/oa-computers` and the SPACE driver, `deploy/space-service`, `deploy/space-credentials`, the Computers routes, and usage and quota code.
- **Order:** #11936 (live), #11814 (live), #11813 (live, uncommitted work on `oa-sdk-build-11800`), then #11815's remainder, then #11816, then the #11800 umbrella.
- **If the live session has stopped,** a new agent recovers #11813 from `/home/christopherdavid/work/openagents-sdk-implementation` on `oa-sdk-build-11800`, and #11814 from its last comment.

### Lane 7: host authority

**Taken 2026-10-16 23:06 CDT** by Codex working #11911 in `~/work/openagents-host-finish`. #11817 is closed; its code is on main through `52ecf579fe`, with deploy checks queued below.

**Released 2026-10-16 22:23 CDT** by Codex; #11817 and #11911 remain. #11817 prerequisites landed in `226ebf63a3` and `1dde3bd99a`; the enrolled adapter and project sharing are unfinished. Worktree: `~/work/openagents-bang-lanes` (removed after handoff).

- **Owns:** `crates/coder-host/src/{serve/*,enroll.rs,authority.rs,computer.rs,tasks.rs}`, `crates/coder-access` (store, enrollment, shared host APIs), and `crates/coder-pty`'s command identity.
- **Keep source-compatible, because lane 8 consumes them:** `Host::handle`, the `authority::Grants` check, and `client::Connector`. Changes there must be additive.
- **Don't edit (lane 8 owns them):** the appended device-channel section of `nips/openagents/NIP-HOST.md`, the new app engine serve module, the app sync transport, `crates/coder-sync/src/server/{auth,proof}.rs`, and the mobile toggle.
- **Order:** #11817 first: workspace-scoped file and command admission, symlink and root-escape refusal, and durable command identities across restarts. #11911 builds the project scope on the same admission.
- **Before you edit:** read `.agents/skills/nostr/SKILL.md` and query the code graph for `coder_host::computer`.

### Lane 8: phone

**Taken 2026-10-16 22:10 CDT** by Codex (GPT-6) working #11852, #11854, #11853 in `~/work/openagents-codex-bang`. #11840 has an existing project claim.

- **Owns:** `apps/openagents/crates/mobile`, the iOS app, `crates/coder-sync` notifications and auth (`auth.rs`, a new `proof.rs`), the push registration, and the NIP-HOST device-channel profile.
- **Order:**
  1. #11852: after lane 0 deploys `4a796e8ff8`, apply the phone patch, regenerate bindings with `scripts/ios/build-core.sh`, and run `OurCloudLiveTests`. TestFlight 1010 is used, so the next build is 1011.
  2. #11854.
  3. #11853. The owner-only push capability is recorded below.

### Lane 9: CLI, Agent Lab, and web fixes

**Taken 2026-10-16 22:47 CDT** by Codex (Grok 4.7) working #11934, #11929, #11925, #11727 in `~/work/openagents-lane9b`.

The website agents door uses the shared `place` (`1708f44f49`). Deploy and the Foreman live check stay in After-close checks. #11929 is closed on code (`7fb459a5a5`). #11925 is next.

**Released 2026-10-16 22:40 CDT** by a Claude Code subagent. #11935 is closed (`264642e0a6`). #11934's `openagents agents` command is on `main` in the same commit. #11928 is closed; the no-traffic production check is in After-close checks.

- **Owns:** `crates/openagents-cli`, `crates/openagents-login`, `crates/openagents-sdk/src/agents*`, `crates/openagents-web/src/{agents_door,attribution,pages/contact,pages/affiliates}.rs`, the web main and gateway startup paths for #11925 and #11928, and `apps/openagents/crates/engine/src/doc_host/agent_lab.rs` for #11929.
- **Order:** #11935, then #11934 (one patch covers both), then #11929, #11925, #11928, and #11727.
- **Known baseline failures:** these `openagents-web` tests already fail on `main` and aren't yours to fix: copy_guard (OpenAPI wording), route_owners, screens, api_docs guides, environments::computers (6), cad_web::world_physics (1), and work_runs::computers (1).

### Lane 10: signet and LayerTree (mostly waiting)

**Taken 2026-10-16 22:40 CDT** by a Claude Code subagent working #11863, #11770, #11862, #11793, #11794, #11799, #11762, #11779 in `~/work/openagents-lane10`.

- **Owns:** reading the observers on `oa-lnd-builder` and the LayerTree service, writing the final results into the reports, and closing each issue. It changes no live controller.
- **Schedule** (signet has run about 5–6 blocks an hour; height was 326691 at 02:45 UTC):

| Event | Height or time | Expected (CDT) | Then |
| --- | --- | --- | --- |
| LDK delayed output spendable (#11863 funding) | 326696 | Fri ~22:40 | The observer funds both nodes, opens the asset channel, runs RFQ both ways, and the cleanup closes it. Close #11863 when the results are written. |
| Wavelength unilateral exit matures (#11770) | 326721 | Sat ~03:00 | Record the sweep fee, then close. The offline-refresh window (327055–327067, about 65 hours away) goes to `NEEDS_OWNER.md` as a follow-up observation. |
| Peer CSV matures (#11862) | 326754 | Sat ~08:30 | The observer sweeps, returns coins, and stops three containers. Record txids and fees, then close. |
| LayerTree soak qualifies (#11793) | 2026-10-17 13:34:53 UTC | Sat 08:35 | The VM deletion runs 5 minutes later, then the unroll at Δ=12 blocks (about 2–2.5 hours) with CPFP. Check every payout, then close. A late heartbeat restarts the 24 hours, which would miss the deadline. |
| Final unroll capture (#11799) | after #11793 | Sat ~12:00 | Render the physical-loss unroll still and clip, then close. |

## Every open issue

The status lines below summarize each issue and its comments. Open the issue for exact hashes and check counts.

### Computers and SPACE (#11800 umbrella)

**#11800 SDK: deliver the public Computers API and first Rust client.** Lane 6.
- **Status:** M1 (Boat, #11801–#11806) is done. In M2, #11809–#11812 are done. #11813–#11816 are open.
- **Remains:** close #11813–#11816, then close this umbrella. #11817 and #11823 aren't blockers.
- **Owner-only:** none beyond the children's.

**#11813 Computers: authorize credential references and enforce immutable egress profiles on GCloud.** Lane 6, claimed by `codex:01a14134`.
- **Status:** an opt-in implementation exists but isn't committed, in `/home/christopherdavid/work/openagents-sdk-implementation` on `oa-sdk-build-11800`. It's a patch to pinned SPACE `de87b436` (`gateway.patch`), applied by `deploy/space-credentials/apply.py`, plus a small Rust `computer-credentials` crate. It adds request-time Google Secret Manager injection, stream revocation, host-only node token files, and a `credential-context` proof route. The guest sees only an opaque `oa-ref:UUID`. Core, driver, and PostgreSQL checks passed. Defaults stay off, and Boat stays the public default.
- **Remains:** a separate canary worker with the adapter (coordinate with #11814), live supported-client qualification, the bypass and revocation acceptance list in the issue, commit, and deploy.
- **Owner-only:** none.

**#11814 SPACE: deploy a private maintained GCloud service with durable storage and bounded capacity.** Lane 6, claimed by `codex:01a14140`.
- **Status:** private native SPACE is deployed. Eight workers are healthy on pinned images. Private TLS frontends return 404 externally. The Cloud SQL candidate is isolated with least-privilege logins. The seed and runtime were published, and a backup restore was checked. Guest quota now bounds pod count. Commits run through `916a8a838c`.
- **Remains:** guest lifecycle, memory and checkpoint recovery, worker replacement, saturation and concurrent wake, drain, upgrade and rollback, placement disable, guest denial, monitoring, retention, and fixture cleanup. Also fix the native `deleted`-row DELETE path (`begin_delete_against_active_fork` rejects it) and bound snapshot collection, both found in #11812.
- **Owner-only:** none.

**#11936 SPACE: frame bodyless lifecycle requests for Cloud Run.** Lane 6, claimed at 21:45 CDT.
- **Status:** Cloud Run returns 411 for a bodyless POST without `Content-Length: 0`, so suspend fails permanently.
- **Remains:** send explicit zero-length framing on bodyless mutations, add a wire-level regression test, redeploy the candidate, and verify suspend and wake.
- **Owner-only:** none.

**#11815 Computers: persist usage, quotas, and uncertain reservations across backends.** Lane 6, unclaimed.
- **Status:** two increments are in production: usage events, allowances, stopped retention, `GET /v1/computer-usage`, explicit unknown usage, storage observations, and a deleted-allocation observer. Computers runs `c50225e11b` and Boat runs `76d096ef26`. The dedicated production account got 403 on its usage read, so production usage fields aren't verified.
- **Remains:** a complete storage inventory with byte and count quotas, private-object and account-deletion cleanup, useful-task versus idle economics, and customer units, rates, rounding, and settlement rules (pricing stays off). Also investigate the production 403.
- **Owner-only:** customer pricing and entitlement.

**#11816 SPACE: qualify the public SDK and web-chat path, recovery, and staged rollout.** Lane 6, not started.
- **Depends on:** #11813, #11814, #11815.
- **Remains:** all of it: the dedicated-account public flow and web task on SPACE, fault injection, disaster cases, rollback and placement disable, thresholds, and phased admission.
- **Risk:** the most likely Computers issue to miss Saturday. If it does, close with what was qualified and list the rest, under the brief's rule.

**#11817 Computers: implement an enrolled-own-machine adapter with safe attach and detach semantics.** Lane 7, code complete.
- **Status:** `226ebf63a3`, `1dde3bd99a`, and `6cb641acc9` establish durable launch identities, descriptor-based workspace files, signed account/root scopes, and bounded host commands. The Core `computer-enrolled` driver and explicit account-bound service placement now connect those pieces. Automatic placement excludes enrolled hosts. Delete and expiry detach logical access; unsupported cloud controls and credentials refuse.
- **Checks:** scoped access and host libraries (120 and 118 tests), driver capability/storage checks (11), lifecycle library and adapter checks (27 and 9), signed direct/relay adapter checks (2), and service library plus disposable-Postgres API checks (23 and 1). Formatting covers changed files only. Two pre-existing Linux-oriented `computer-driver::identified` shell tests time out on macOS, even with GNU tools; they are outside the changed path.
- **After close:** lane 0 deploys the new host and service, then runs the dedicated-account check below. The initial implementation requires a Unix filesystem boundary, an explicitly configured root, and a scoped device grant. Setup: [`computer-enrolled`](../../crates/computer-enrolled/README.md).
- **Owner-only:** none.

**#11823 Coder: consume the public Computers SDK for remote work and agent tools.** Lane 1, not started.
- **Status:** the freeze no longer applies. Its prerequisite #11806 is done.
- **Remains:** replace direct allocation, launch, file, and command transport in `coder remote delegate` with `openagents::computers`. Keep `--on boat`'s meaning, logical retry handles, and the separation of detach from cancel. Add computer tools only for implemented capabilities. Run a dedicated-account staging task.

**#11925 Accounts: keep authenticated reads predictable during gateway handoff.** Lane 9, not started.
- **Status:** during a traffic shift, the outgoing web revision returns 503 on account-backed reads after its account service hands over the store. Trace `ed19af0410b5b236d6ccb7d3cd5d0676`.
- **Remains:** an outgoing and incoming handoff fixture, bounded safe-read recovery or deploy coordination, accurate retryable errors, mutations never replayed, and a controlled deploy with a synthetic account.

**#11928 Deployments: fence Environment work before a candidate receives traffic.** Lane 9, not started.
- **Status:** a held candidate opens `Studio` and requeues unfinished setups before it gets traffic. Its guards are process-local, so it can fail a shared setup or sweep homes.
- **Remains:** a two-process fixture proving a zero-traffic candidate makes no setup change, model call, computer cancel, or home deletion, plus a cross-process execution lease. Keep the candidate hold and the eight WAIT checks.

### Labs platform (#11871 epic)

**#11871 Epic: Labs platform.** Lane 4.
- **Status:** #11872, #11873, #11874, and #11879 are done. The rest is open.
- **Decided (owner, 2026-10-16):** renders that show MetaHuman or Fab content are never committed to GitHub. They live in the space's private platform storage and stream to users. The plan's other decisions stand as recommended: new hosted Labs start with owner and test accounts, and AVPlayer comes first on Apple platforms. The 1.0 bridge decision is moot.

**#11875 Labs L4: CAD Lab as the first kind on the Labs platform.** Lane 4, with lane 0 for the deploy.
- **Status:** source is on `main` (`2f3465b291`, `218a67f92f`, `5bdf74bba6`). Focused checks passed: CAD 57, 22 CAD web cases, store and API with PostgreSQL and fake GCS, and the cookie-revocation regression. The combined image is on staging and passed 99/0/2.
- **Remains:** the dedicated-account live run (fan bracket, pick a face, make it 5 mm thicker, project versions, STEP download, cleanup) with `scripts/smoke/sdk-labs-cad.py` and `run-labs-cad-live.py`, on staging and then production. Browser checks go through `openagents browser run`.
- **Owner-only:** approve the production CAD copy (1 document, 2 versions, 11 files, checksum `16445f2f…93ae`). It's already in `NEEDS_OWNER.md` and doesn't block closing.

**#11876 Labs L5: `openagents::labs` in openagents-sdk.** Lane 4.
- **Status:** the client is on `main` (`1b55b50ae1`). SDK 59, Labs 10, and CAD preview 1 passed. The draft example is on `codex/11876-labs-sdk` (`6281e709f3`).
- **Remains:**
  1. Fix the external CAD build: 18 `solidcraft-kernel` errors, because root Truck patches don't reach a consuming workspace. Either pin the patched crates as Git dependencies or document the external `[patch]`.
  2. Build the Git-pinned example, then run the live CAD flow against production with the dedicated account.
  3. Run the macOS and iOS consumer check, and update the README pin.
- **Note:** a 02:35 UTC read showed production didn't list CAD yet, which waits on #11875's production deploy.

**#11877 Labs L6: Lab cards in saved history, and the web Labs door on the registry.** Lane 4.
- **Status:** integrated history cards, typed SDK cards, `lab-ui`, generic document/media pages, and registry dispatch. Hosted kinds replace their loopback adapters; app requests carry the current sender's credentials. Source checks pass: API/UI 11, SDK history 5, app account 53, and 17 focused web tests. The OpenAPI export is updated and the saved patch file is removed.
- **After-close:** the production history-card and second-browser check is listed in [After-close checks](#after-close-checks).

**#11878 Labs L7: jobs and placement.** Lane 4.
- **Status:** the combined branch `codex/11878-jobs-placement` (`0021970e87`) has `lab-jobs`, placement, scoped worker keys, worker routes, `labs-worker`, the SDK worker, the cloud launcher, and the Agent Lab source policy. `ModelUse` alone is on `main` (`6c8df90ee9`). In the last combined run, `restart_after_execution_marker_never_replays_the_effect` failed. Its fix is committed in `8bffde9fbb` but hasn't been rerun. The gateway, SDK worker, web consumer, and app engine tests haven't run. PostgreSQL and GCS sections were skipped.
- **Remains:**
  1. Rebase and regenerate the lock.
  2. Rerun everything, including an explicit SDK `Uncertain` regression.
  3. Deploy through lane 0.
  4. Run live echo on server, enrolled, and cloud placements. The enrolled host is a disposable VM. The cloud worker needs either a registered image with the pinned `labs-worker` or the proposed chunked-upload path. Boat's workspace is `$HOME/workspace`.
- **Owner-only:** installing `labs-worker` on the owner's Mac. Add the steps to `NEEDS_OWNER.md`.

**#11880 Labs V2: Video Lab on the platform.** Lane 5, not started.
- **Depends on:** #11877, #11878.
- **Remains:** Rust job runners (ffmpeg, Blender, `film_shot`, whisper.cpp), enrolled-host placement for `private_local` assets, the Rust loopback service on 8792 behind `lab.py serve`, import of the local films, `/labs/video/{doc}` pages, and the three acceptance runs.
- **Simplification:** the freeze is lifted, so Coder can call the platform directly. Keep loopback mode only where the local web chat or `openagents lab` still needs it.
- **Owner-only:** importing `~/work/film-out/lab/films/*` runs on the owner's Mac.

**#11881 Labs G1: Game Lab and Ship Lab as platform kinds.** Lane 5, closed.
- **Done:** `game_lab::kind::Game` and `ship_lab::kind::Ship` on `main` (`e37d66493e`, `ade5341924`), with the `game-lab playtest-job` worker handler. Parity tests run the same tool edits through the local services and the kinds. The loopback routes are unchanged.
- **Waiting on lane 4:** registering both kinds in `crates/openagents-web/src/labs_api.rs`, next to CAD, and the `/labs/game/{doc}` page. #11877's patch edits that file, so lane 5 left it alone. The rest is in [After-close checks](#after-close-checks).

**#11882 Labs R1: Robot Lab documents and runs on the platform.** Lane 5, open and free.
- **Done:** `robot_lab::kind::Robot` on `main` (`d08cf1dc58`): the four schemas with the local store's checks and wording, `pose` and `move_object` intents, host-checked dependencies, the `step` note locator, `preview` and `simulate` declarations, and the cards. `robot_lab::migrate::plan` reads a local project in dependency order and rewrites references to platform content digests; `migrate::import` (feature `platform`) writes it into a `lab_store::LabStore` with notes and files. `cargo test -p robot-lab --features platform` passed.
- **Remains:**
  1. The `simulate` handler. A job's `Work` carries only the experiment version, but `robot-sim` needs its scene, robot, and mesh files. A worker needs a way to read the referenced versions and files, which is lab-jobs and lab-api work.
  2. Register the kind in `labs_api.rs` (lane 4's file during #11877), and add the web replay card.
  3. A simulation run on a cloud worker with a test account, shown on the web and through `/v1/history`.

**#11883 Labs D1: desktop Lab row and the standard views.** Lane 5.
- **Status:** the native Video card with inline AVFoundation playback is in `apps/openagents/crates/ui/src/lab_card.rs` (loopback only).
- **Remains:** the image, compare, Model3d, and report views, cards from `/v1/history`, live updates from `/v1/labs` events, and fixture screenshots. Render `lab-ui`'s Rust Native card through the GPUI adapter.

**#11884 Labs P1: iPhone Lab row and standard views, notes by voice.** Lane 5, not started.
- **Remains:** a small UniFFI facade over `openagents::labs`, a Lab row and a cell per view, dictated notes, stopping media on background, and fixture UI tests. The bundle is now `com.openagents.app`.

**#11885 Labs T1: Labs in Coder 1.0.0.** Lane 4, not started.
- **New scope:** native Labs in `coder` and `openagents` through `openagents::labs`, not a bridge for a frozen binary. Read tools from `/v1/labs/kinds`, follow events, and keep the `VIDEO_LAB_URL`-style loopback only for local Labs not yet on the platform.
- **Remains:** request and response tests, and an isolated-HOME run where `openagents cad` and the Coder video tools edit account documents.

**#11886 Labs D2: video timeline editor in the desktop app.** Lane 5, last in the lane.
- **Depends on:** #11880, #11883.
- **Risk:** likely to miss Saturday. Close with what landed if it does.

### Chats, sessions, and moving agents (#11896 epic)

**#11896 Epic: OpenAgents chats are normal, resumable chats.** Lane 1.
- **Status:** #11892, #11893, and #11895 are done. #11894 is partial. #11897 and #11898 are open.
- **Remains:** close the three children.

**#11894 Continue a terminal or phone chat on this computer, as a real session.** Lane 1.
- **Status:** `cca7f94ebe` has `chat-home` types, the continuation route, SDK methods, app engine seeding and recovery, read-only sources, and mobile metadata. Earlier app crate tests passed (account 53, mobile 55). Remote checks ran on `04231f589c`, before the final rebase. The worktree `/Users/christopherdavid/work/openagents-resumable-chats` is clean.
- **Remains:**
  1. The harness bridge: `coder-delegate` calls `openagents-harness`, and the duplicate drivers are deleted.
  2. Terminal chats on the SDK and Loro path.
  3. The live acceptance: a test account's terminal chat continues on the desktop, the first turn knows the history, and the phone steers it.
- **Starting points:** `crates/coder-delegate/src/adapter.rs` (about line 334), `delegate.rs` (`Wrap`, about line 2550), and `apps/openagents/crates/harness/src/lib.rs`.

**#11897 Show computer sessions as rows on openagents.com.** Lane 2, not started.
- **Remains:** a summary-only session index per account, rows mixed into the web list with an "On <computer>" badge and offline state, a summary page with app and continue actions, a per-chat and per-project transcript opt-in (off by default), `source: "session"` rows in `/v1/history`, and access and revocation checks.

**#11898 "Where it runs" is a property of the chat.** Lane 1, after #11894.
- **Status:** the foundations are in `cca7f94ebe`.
- **Remains:** the "Runs on" control, computer-to-cloud continuation through the #11893 path, paired and cloud computer choice, one conversation row across web, terminal, desktop, and phone, and the web, Mac, then cloud round trip on a test account. The cloud computer choice needs #11852.

**#11911 Computer sharing: owner-approved project-scoped access over NIP-HOST.** Lane 7, claimed by Codex.
- **Status:** `72a0bc730c` adds owner-approved project invitations pinned to the recipient account/device, roots resolved from host configuration, absolute expiry, and revocation of one share without advancing the device epoch or revoking unrelated scopes. Scoped guests still cannot use broad task/read/terminal/app RPC paths. Host sharing advertises `project-sharing-v1`; old hosts refuse. Checks: access library 123, host library 118, and signed direct/relay adapter checks 3.
- **Remains:** the native owner sharing controls and recipient account view, with account-isolated device keys and saved grants. Map only implemented project operations into that view; project membership alone grants no host permission. Coordinate with lane 8's app device-channel integration. The issue stays open while that code remains.

**#11929 Agent Lab: run tests for agents that don't need a project copy.** Lane 9. Closed on code.
- **Decided (owner, 2026-10-16):** option 1, an experiment-level "no project files" choice that runs the trial in an empty folder. The report says the agent had no project files.
- **Status:** on `main` (`7fb459a5a5`). The website accepts a host report only when it matches that choice. `cargo test -p agent-lab --features views project_files` passed (2). `cargo test -p openagents-web --lib agent_lab::tests` passed (2). `cargo test -p openagents-engine --lib no_project_files` passed (1).

**#11930 Move running agents: keepers.** Lane 3, spec only.
- **Remains:** a keeper subcommand that owns each harness run's stdio and process group, journals frames, and serves a 0600 socket. The engine adopts keepers and replays on boot, the MCP bridge reconnects, and an update becomes a handoff.
- **Done when:** a fake harness survives an engine kill with no gap, a real Claude Code and Codex turn survives a restart, and pending permission questions come back.

**#11931 Move running agents: background engine by default.** Lane 3, after #11930.
- **Remains:** the LaunchAgent or systemd service per data folder, the app attaching over IPC, a menu-bar item, and handoff to the service.

**#11932 Move running agents: capture and restore on a cloud computer.** Lane 3.
- **Depends on:** #11852, #11813, #11898.
- **Remains:** commit the tree to `refs/openagents/move/<chat>` or a bundle, upload the harness session file, restore at the same path, and use credential references only.

**#11933 Move running agents: move plans, Move all, Bring back, and agent tools.** Lane 3, last.
- **Depends on:** #11932, #11931.

**#11934 Ask a saved agent (Foreman) from `openagents chat send`, web, and phone chats.** Lane 9, with lane 0 for the deploy. Closed on code.
- **Status:** `openagents agents list|ask|status` is on `main` (`264642e0a6`). The website agents door calls `openagents_sdk::agents::place::place` (`1708f44f49`). `cargo test -p openagents-web agents_door` passed (2 tests).
- **Remains:** lane 0 deploys that commit or newer and runs `ask_foreman_cli.py` and `openagents agents ask Foreman …` with test account `acct_500d3e1d125b7782`. The check is in After-close checks.

**#11935 An `OPENAGENTS_TOKEN` that isn't a sign-in token falls back to the saved login.** Lane 9, first.
- **Status:** `token_env_set` is on `main` (`bb3fe7c578`, 9 tests). The guard in `keys.rs` and `mac.rs` is in the #11934 patch.
- **Why first:** this bug made a test command run as the owner and create a project on the owner's account. Fix it before anyone runs another CLI check.

### Phone (#11840 epic)

**#11840 Epic: the OpenAgents mobile app on coder-sync.** Lane 8.
- **Status:** P0–P5 and the crash fix are done. #11852 is partial. #11853 and #11854 are open.

**#11852 Cloud computers: run the headless engine on a computer, so the phone works without a desktop.** Lane 8.
- **Status:** `/v1/cloud-host` (`3063ab036c`), the pinned Linux engine (`1cdf329dfc`), and the SDK methods (`048005fe73`) are deployed. `4a796e8ff8` (opencode on OpenAgents inference, because production has no server Claude key) isn't deployed. The phone patch is in the 02:44 UTC comment: `cloud_host.rs` FFI, a Settings row, the "Run on" choice, and `OurCloudLiveTests`.
- **Finding:** cloud computers suspend after every command, so the engine runs as one command of up to an hour.
- **Remains:** the deploy (lane 0), the patch, regenerated bindings, `OurCloudLiveTests`, checking `GET /v1/computer-usage`, `DELETE /v1/cloud-host`, and TestFlight build 1011.

**#11853 Push notifications through our push gateway.** Lane 8.
- **Status:** the pure decision rules are in `crates/coder-sync/src/server/notifications.rs`.
- **Remains:** call `notification_for` on registry writes, send through the push gateway (`docs/deployment/push-gateway.md`), register the phone's target, and port the `push-notify.test.ts` cases.
- **Owner-only:** the App ID's Push Notifications capability (before any build sets `CODER_IOS_PUSH`) and the real-device check.

**#11854 Carry the device channel over NIP-HOST, with Nostr device proofs.** Lane 8.
- **Remains:** the NIP-HOST profile for app RPC, engine serving through `coder-host::client::Connector`, an opt-in phone transport with the WebSocket room as fallback, NIP-98 proofs in `coder-sync`, and a written latency comparison.

### Signet, LND, and LayerTree

**#11762 Epic: LND as our server-side Lightning backend.** Lane 10.
- **Status:** P0–P4 and #11861 are done. #11770, #11862, and #11863 are waiting on blocks.
- **Owner-only:** the plan's §7 decisions and every mainnet step. They're already in `NEEDS_OWNER.md` and don't block closing.

**#11770 Wavelength signet spike.** Lane 10.
- **Status:** the checkpoint is on `main` (`81a012cfaa`) with the verdict "keep LDK for P4."
- **Remains:** the exit at 326721. Record it and close. The refresh window is after the deadline, so it goes to `NEEDS_OWNER.md` (the observer keeps running).

**#11862 LND: remote signing and SCB recovery on signet.** Lane 10.
- **Status:** the remote-signer exercise (`2c2f1004b1`) and the anchor repair with a 14,316-sat return (`cae42db176`) are done.
- **Remains:** the peer sweep after 326754, the returns, three containers stopped, and the final txids and fees in the report.

**#11863 LND: Taproot Assets mint, transfer, and RFQ on signet.** Lane 10.
- **Status:** the mint, transfer, and proof are done (`ce7e3096df`).
- **Remains:** after 326696, funding, the private asset channel, RFQ both ways, close, returns, and shutdown.
- **Deadline:** 2026-10-19 12:14 UTC. If LDK's sweep hasn't appeared a few blocks after 326696, read `~/wavelength-spike/ldk-home/ldk/ldk_node.log`. Never add funding.

**#11779 LayerTree PoC: P0 regtest and P1 signet.** Lane 10.
- **Status:** P0 is done. P1 is done except #11788, #11793, and #11794. Visualizations are done except #11799.

**#11788 LayerTree: run the enclave on an SEV-SNP Confidential VM.** Closed 2026-10-16 on the owner's decision: GCP Confidential Space rejects SNP, and TDX satisfies P1.

**#11793 LayerTree: public signet tree, enclave kill, and timed unroll.** Lane 10.
- **Status:** the HALT run is done and paid (`cc6388ca3e`). The primary tree's clean 24-hour window qualifies at 13:34:53 UTC Saturday.
- **Remains:** the observed deletion, the unroll at Δ, exact payouts, the txids in the issue, and the event-log fixture.

**#11794 LayerTree: light-client alarms on signet.** Lane 10.
- **Status:** the follower code is on `main` (`be91753d34`). Three followers are running. The staged theft, withheld-record, and mismatch alarms fired on every follower (`371e538ac2`). The opencode session that claimed it at 16:39 UTC has no live claim.
- **Remains:** three followers in sync for 24 hours (owner decision, 2026-10-16), following the primary tree through #11793's deletion and unroll. Then record the results and close.

**#11799 LayerTree: headless captures for docs and film.** Lane 10.
- **Status:** regtest, normal signet, HALT, and alarm captures are done.
- **Remains:** the primary physical-loss unroll still and clip after #11793.

### Growth

**#11727 Affiliate attribution, part 2.** Lane 9, last.
- **Status:** audited only.
- **Remains (code):** carry consented attribution on `/contact` requests, owned short `/r/{handle}` handles, and a self-serve link page under the existing auth.
- **Remains (needs Stripe):** the named-sale end-to-end check. There's no Stripe yet; the owner sets it up on 2026-10-17. Run the check after that, or close with it recorded in `NEEDS_OWNER.md`.
- **Owner-only:** publishing the terms (`tenant-referrals publish-affiliate`), setting `inference.sats_rate`, linking `/affiliates`, and sending payouts.

## After-close checks

Deploys and live checks that no longer hold an issue open. Lane 0 works through them in order, after each production deploy. Add a row when you close an issue with a check still to run. When a check passes, mark it done with the evidence. When it fails, open a new issue and link it.

| Issue (closed or closing) | Check | Status |
| --- | --- | --- |
| #11817 | Deploy the new host and computer service. Configure a disposable Unix host, scoped test-account grant, and explicit enrolled target. Run the public attach, command retry/reconnect/cancel, relative-file, revocation, unsupported-control, and detach flow; verify files and unrelated grants survive. Repeat the Linux-specific command checks on Linux. | source checks passed; deploy and dedicated-account run to do |
| #11934 | Deploy the agents door from `1708f44f49` or newer. Run `ask_foreman_cli.py` and `openagents agents ask Foreman …` with test account `acct_500d3e1d125b7782`. The web door's switch to `place` is on `main` (`1708f44f49`). | to do |
| #11936 | Redeploy the SPACE candidate with explicit zero-length framing and verify suspend and wake through Cloud Run. | Candidate deployed as `oa-space-service-computers-00002-pch`; the saved pre-deploy rejection blocks the existing fixture. Recovery defect: #11939. Suspend/wake remains unqualified. |
| #11939 | Deploy the checked private candidate, run the explicit operator retry for the saved rejected suspend, then qualify encrypted checkpoint and resume. The [usage evidence](../sdk/computers-usage-evidence.json) has the exact request and expected child. | 31 final focused checks and the default build passed; image build/deploy and live recovery remain to do |
| #11814 | Promote the checked cleanup images, install eleven measurements and nine alert policies, then run encrypted lifecycle, worker replacement, concurrent wakes, drain/rollback/placement disable, guest denial, retention, and fixture cleanup. Use the [private service runbook](../../deploy/space-service/README.md) and [receipt](../cloud/evidence/2026-10-16-space-private-foundation.json). | to do; source checks passed 50/0 |
| #11875 | The dedicated-account CAD run on staging and then production: fan bracket, picked face, 5 mm thicker, project versions, STEP download, cleanup. | Closed. Staging passed 18 CAD checks, browser selection, and cleanup. Production smoke passed 66/0/2; CAD operations and browser selection passed, but API project cleanup failed (#11938). The leftover test project was removed through the website. Evidence: `docs/sdk/labs-cad-staging-handoff.json`. |
| #11938 | Deploy the sign-in-key account lookup fix, then repeat `/v1/projects` and the complete production CAD run with the original dedicated test key, including project deletion. | Session tests passed 13/0; project API tests passed 6/0. The earlier leftover test project was removed through the website. Deploy and production recheck: to do. |
| #11876 | Run the Git-pinned `examples/sdk-labs` CAD flow against production with the dedicated account, including ranged GLB, notes, events, and deletion. | Passed on production image `sha256:51e2d77b…202a7` using the previously qualified Git-pinned binary: CAD edits/refusal, restored job retry, ranged GLB, notes, 14 events, design deletion, and artifact refusal after deletion. Evidence: `docs/sdk/labs-client-evidence.json`. |
| #11877 | A CAD chat's card through `/v1/history`, re-rendered in a second browser session. | to do |
| #11878 | Deploy the account-service worker routes before the web worker routes. Run dedicated-account Echo on server, disposable enrolled, and cloud placements; prove private input bytes and paths stay local, retire scoped keys, and observe cloud computer deletion. The cloud environment needs the pinned `labs-worker` and trusted handlers installed. | to do; owner Mac installation is in `NEEDS_OWNER.md` |
| #11852 | Deploy `4a796e8ff8` or newer. Run `OurCloudLiveTests/testStartRunAndStopOurCloud` with the dedicated test account, check `GET /v1/computer-usage`, delete the computer with `DELETE /v1/cloud-host`, and remove its test registry rows. Validate TestFlight 1011 or the next free build. | to do; phone source `d37ed99d0c`, local control acceptance `a67ec60b44` passed |
| #11894 | A test account's terminal chat continued on the desktop: the first turn knows the history, and the phone steers it. | to do |
| #11881 | Register `game_lab::kind::Game` and `ship_lab::kind::Ship` in `labs_api.rs` after #11877 lands, host `/labs/game/{doc}`, and install `game-lab playtest-job` as the `playtest` handler. Then, with the dedicated test account on production: a Game Lab edit plays in the web card and appears through `/v1/history`, and a Ship Lab refusal and an accepted edit. Clean up after. | to do |
| #11815 | Deploy storage allowances and account cleanup through lane 0; collect complete private storage inventories, including recoverable versions, and qualify limits/cleanup on synthetic fixtures. | Source checks passed 71/0 on rebased main, including five storage/cleanup cases. Usage read passed with `acct_83d16a35955ef908`: HTTP 200, pricing off, no charge; the earlier account remains outside the rollout. [Evidence](../sdk/computers-usage-evidence.json). Storage deployment and live cleanup: to do. |
| #11928 | On the next production deploy, a candidate with no traffic does not resume setups or delete homes. | to do |
| #11897 | Deploy web, and build the desktop app, from `main`. With the dedicated test account, a desktop session appears on openagents.com within about 30 seconds as a summary row; its transcript isn't on the server until opt-in (`openagents::sessions::set_transcript`), then renders on the summary page; a second account sees nothing; removing the computer removes its rows. Clean up the test computer and chats afterward. | to do |

## Shared rules

These add to AGENTS.md and the brief.

- **One deployer.** Only lane 0 shifts staging or production traffic, and only one release agent cuts candidates. Ask lane 0. Don't wait idle while you ask; keep working on source.
- **Disk floor.** Keep at least **30 GiB** free on the Mac. Check with `df -h ~/work` before each cold build. Below the floor, start no new build and tell lane 0. Reuse a warm target. The existing ones are `~/work/openagents-target-agent1` (70 GB), `agent3` (40 GB), `labs-sequential` (48 GB), `coderdev` (14 GB), `agent2` (10 GB), and `foreman-app` (6 GB). One target per lane, never shared by two running builds. The app workspace (`apps/openagents`, Rust 1.98.1) needs its own target.
- **At most six concurrent Cargo builds** on the Mac (a 2026-10-07 build-load crash rebooted it). Lane 10 and waiting lanes don't count.
- **Test accounts only.** Use `~/work/.secrets/openagents-test-account.env` or the per-issue test accounts named in the handoffs. Set `OPENAGENTS_TEST_RUN=1`. Until #11935 lands, unset any saved login in a scratch HOME before every CLI check, because a non-login `OPENAGENTS_TOKEN` silently falls back to it.
- **No owner-machine tests.** Scratch hosts get a temporary HOME, state, root, and tasks.
- **Label machines** you create with `python3 scripts/dev/agent-run.py labels`, and delete them before you close.
- **Patches in comments aren't proof.** A handoff's check counts apply to the exact commit it names. Rerun on your rebased source before you claim a pass.

## Owner decisions (answered 2026-10-16)

1. **#11788 SNP:** closed as unavailable on GCP. TDX satisfies LayerTree P1. If Confidential Space adds SNP, open a new issue.
2. **#11794:** 24 hours of follower sync, not a week, matching #11793.
3. **#11929:** option 1. Add a "no project files" choice that runs the trial in an empty folder.
4. **#11871 Labs renders with MetaHuman or Fab content:** never commit them to a GitHub repository. Store them in the Lab space's private platform storage and stream them to users through `/v1/labs/artifacts`. Raw assets never leave the owner's machines.
5. **#11727:** there's no Stripe yet. The owner sets it up on Saturday 2026-10-17 (reminder in `NEEDS_OWNER.md`). Build the code parts now, and run the named-sale check after Stripe exists.
6. **Lane 6:** the Codex session on #11813, #11814, and #11936 keeps running. Don't take those issues.

Already in `NEEDS_OWNER.md`, and they don't block closing: the CAD history copy (#11875), the LND and Taproot mainnet steps, the push capability and device check (#11853), the `labs-worker` Mac install (#11878, to be added), and customer pricing (#11815).

## Done means

- Every issue above is closed, or the owner explicitly moved it out of 1.0.0.
- `main` builds every Coder program, `1.0.0-rc.23` or later is green on the brief's gate, and the go or no-go is called on Sunday night.
- Production web runs a commit from Saturday's `main`, and the patch files are gone from `docs/audits/`.
- No test resource is left on the owner's account, and every machine created for a check is deleted or labeled for the reaper.
