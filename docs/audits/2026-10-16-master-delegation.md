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
| Maintained private SPACE service (#11814) | `oa-space-service-control`, `oa-space-service-worker` | 0 | Leave these hosts and the stopped qualification fixtures intact for the After-close checks. Lane 6's disposable credential hosts and builder are deleted. |
| Signet observers for #11862, #11863, #11770 | `oa-lnd-builder` (us-central1-a); status with `bash /tmp/sigstatus.sh` | 10 | Read only. Never add funding or restart a controller. |
| LayerTree primary TDX tree, 24-hour soak, scheduled deletion | `oa-layertree-signet-tdx`, mirrors `oa-layertree-mirror-0/1/2` | 10 | The clean window can first qualify at **2026-10-17 13:34:53 UTC (08:35 CDT)**. The observer then deletes the VM itself. |
| #11794 followers | `oa-lnd-builder`, `~/layertree-followers-11794-d9e41513b5/` | 10 | Stop time `1792877280` (Unix). |
| Production web | `coder-web-e195c182ff-20261017050408` at 100% | 0 | Web `sha256:e195c182…3b7ea`, account-service `sha256:cccb6583…4b5e9`, source `0b74ec8f79`. Production smoke passed 66/0/2; project lists need #11941. |
| Staging web | `openagents-web-1-staging-0b74ec8f79-045541` at 100% | 0 | Same web and account-service images as production. Standard smoke 99/0/2, CAD 18 checks, browser pick, and cleanup; Echo 21 checks and cleanup passed. |
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
| #11931 | On a test host (never the owner's Mac): quit and relaunch the packaged app during a turn, and check that the turn completes and the app shows it. Then replace the app binary during a turn, and check that the turn isn't interrupted. The fake-turn client-kill check also needs a harness the MCP tools accept, because they hide `mock`. | Unit tests passed: 9 in `openagents-background`, keepers 5 + 4. Debian 12 VM `lane3b-11931`: unit install, start, and attach passed; turn checks not run; VM deleted 2026-10-17. |
| 3. Keepers and moving agents | #11930, #11931, then #11932, #11933 | partly (#11930 changes the app binary) |
| 4. Labs platform | #11878, #11876, #11877, #11875 (live checks), then #11885 | yes (#11885) |
| 5. Labs kinds and app views | #11881, #11882, #11880, then #11883, #11884, #11886 | no |
| 6. Computers and SPACE | #11936, #11814, #11813, #11815, then #11816, then #11800 | no |
| 7. Host authority | #11817, then #11911 | yes |
| 8. Phone | #11852, #11854, #11853, then #11840 | no |
| 9. CLI, Agent Lab, web fixes | #11935, #11934, #11929, #11925, #11928, #11727 | yes (#11934, #11935) |
| 10. Signet and LayerTree | #11863, #11770, #11862, #11762, #11793, #11794, #11799, #11779 | no |

### Lane 0: release and deploys (one agent, the only one that deploys or releases)

**Taken 2026-10-16 22:04 CDT** by Codex working deploys and releases for #11875, #11934, #11852, #11936, and the after-close follow-ups #11938 and #11941 in `~/work/openagents-bang-lane`.

- **Owns:** every web deploy (staging and production shifts), every release candidate cut, the final 1.0.0 publish, the project boards, and `NEEDS_OWNER.md` hygiene. Other lanes ask lane 0 to deploy. They never shift traffic themselves.
- **Tonight:**
  1. Run `scripts/dev/project-status-sync.sh 22` and `19`. Fix the stale assignees: #11934, #11840, #11762, #11862, #11863, #11770, #11794, and #11799 show AtlantisPleb with no live claim. Leave #11813, #11814, and #11936 alone (lane 6 is live).
  2. Take the next production deploy from current `main`. It carries #11875's CAD platform, the #11934 agents door, and `4a796e8ff8` for #11852. Lane 0 runs the CAD staging check from After-close checks, promotes the tested image, runs the production smoke, and works through the remaining live checks.
  3. Remove the patch files from `docs/audits/` once lane 4 has applied #11877's patch. A `.patch` file isn't a document.
- **Saturday evening:** cut `1.0.0-rc.23` from `main` once lanes 1, 4, 7, and 9 have landed. Run the gate the brief describes, and cut later candidates as fixes land.
- **Monday:** follow the brief's re-cut steps. Reinstall every copy of the withdrawn 1.0.0.
- **Next-release rows that must ship in 1.0.0** (`docs/coder/next-release.md`): the #11771 memfd credential handoff (the source fix is closed, and the binary must carry it), both `api.openagents.com` address moves (check `deploy/production/api-url-map.yaml` first), and #11823. Remove each row as its work lands.

### Lane 1: harness and chats (critical path)

**Taken 2026-10-17 00:48 CDT** by a Claude Code subagent working #11823 in ~/work/openagents-11823.

**Released 2026-10-17 01:10 CDT** by Codex (GPT-6) after #11947 landed in `5e187096d5`; all eight attribution tests passed. #11823 stays with its active Claude Code owner, and #11898 stays with its live owner.

**Released 2026-10-17 01:00 CDT** by Codex (GPT-6) after #11945 landed in `92662ccae4`. Both reported unit failures are fixed; `coder-new` unit checks passed 1,055/0/24. The broader run found the separate attribution failures tracked in #11947.

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

**Taken 2026-10-17 01:20 CDT** by a Claude Code subagent working #11933 in ~/work/openagents-11933.

**Taken 2026-10-17 01:00 CDT** by a Claude Code subagent working #11946 in ~/work/openagents-11946.

**Released 2026-10-17 01:10 CDT** by a Claude Code subagent. #11932 is closed on code (`a76ba6ab21`): `crates/engine/src/sessions/move_cloud.rs` captures, uploads, restores, and resumes one chat on a cloud computer. Its dedicated-account run is in After-close checks. #11933 remains. Target `~/work/openagents-target-11932-app` stays warm.

**Released 2026-10-17 00:57 CDT** by a Claude Code subagent. #11930 is closed on code (`b49a54a201`): Claude Code and Codex run under keepers by default, with `OPENAGENTS_KEEPERS=0` to turn them off. The update handoff and terminal keepers moved to #11946. The live restart check is in After-close checks. The worktree `~/work/openagents-lane3c` is removed; target `~/work/openagents-target-lane3b-app` stays warm.

**Released 2026-10-17 00:19 CDT** by a Claude Code subagent. #11931 is closed on code (`c85eb539b7`): the background engine service, attach-or-start, the macOS menu-bar item, and Move to background. Its live turn checks are in After-close checks. #11932 and #11933 remain. Target `~/work/openagents-target-lane3b-app` stays warm.

**Released 2026-10-17 00:05 CDT** by a Claude Code subagent. #11930's keeper layer is on `main` (`16629755e6`); Claude Code and Codex attach remain, see the issue. #11931 wasn't started: it needs a test host, because a LaunchAgent can't be installed on the owner's Mac. Target `~/work/openagents-target-lane3-app` stays warm.

- **Owns:** a new keeper module in `apps/openagents/crates/harness` (additive; lane 1 owns the existing API), engine adoption in `apps/openagents/crates/engine`, the LaunchAgent and systemd service plumbing, and the move-plan store and UI.
- **Order:** #11930, then #11931. #11932 waits on #11852 (lane 8), #11813 (lane 6), and #11898 (lane 1). #11933 comes last. Spec: [move running agents](../desktop/move-running-agents.md).
- **Risk:** #11932 and #11933 sit at the end of three other lanes' chains. If those slip, ship #11930 and #11931 and close the other two with their remaining steps recorded. Don't hold the release for them.

### Lane 4: Labs platform

**Taken 2026-10-17 00:18 CDT** by a Claude Code subagent working the Labs integration gaps (#11882, kind registration, worker file access) in ~/work/openagents-lane4b.

**Released 2026-10-17 00:08 CDT** by Codex from `~/work/openagents-bang-codex`. #11878, #11876, #11877, and #11885 are closed; lane 0 closed #11875. No unfinished issue claims remain. Left: #11871, including the remaining lane 5 kinds and surfaces and the Game/Ship integration under #11881. Production checks are listed below.

- **Owns:** `crates/lab-kit`, `crates/lab-jobs`, `crates/lab-store`, `crates/lab-api`, `crates/lab-ui`, `crates/openagents-sdk/src/labs*`, `crates/openagents-web/src/{labs,lab,cad_web,history_api}.rs` and the cards, `examples/sdk-labs/`, and the Labs tools in Coder for #11885.
- **Order:**
  1. #11878: rebase `codex/11878-jobs-placement` (`0021970e87`) onto `main`. It already contains the SDK worker and cloud launcher slices, so don't cherry-pick them twice. Regenerate the lock, rerun the combined checks, fix the `Uncertain` finish fix's test, and deploy through lane 0.
  2. #11876: fix the external build (root `[patch.crates-io]` Truck pins don't reach a consuming workspace), then the live example.
  3. #11877: apply the retained patch in a worktree and finish it.
  4. #11875: the live CAD checks on staging, then production after lane 0 promotes.
  5. #11885: native Labs in Coder. Coordinate with lane 1 on `crates/coder*` files.
- **Coordinate:** lane 5 depends on your `LabKind` and job hooks. Keep them additive after #11878 lands.

### Lane 5: Labs kinds and app views

**Taken 2026-10-17 01:20 CDT** by a Claude Code subagent working #11948 in ~/work/openagents-11948.

**Released 2026-10-17 01:10 CDT** by a Claude Code subagent. #11942 is closed on code (`e6ebb67e46`, `a3ef319562`). The hosted player and timeline on `/labs/video/{doc}` are #11949. The `~/work/openagents-11942` worktree is removed; target `~/work/openagents-target-agent2` stays warm.

**#11886 taken 2026-10-17 00:30 CDT** by Codex `codex:01a14140` in its `lnd-work` checkout. It owns a separate desktop timeline module and account-side editor service, with additive card-opening hooks coordinated under #11943. The active lane 5 session keeps #11884 and #11943.

**Released 2026-10-17 01:05 CDT** by a Claude Code subagent. #11884 and #11943 are closed on code (`29d4e808fd`, `a4c68fd920`, `0e18a669df`); typed Lab rows in live sessions, the version list, and Model3d orbit are #11948, which waits on #11942. #11886 is open and free. The `~/work/openagents-lane5c` worktree is removed; target `~/work/openagents-target-lane5b-app` stays warm.

**Released 2026-10-17 00:14 CDT** by a Claude Code subagent. #11880 and #11883 are closed on code; their remaining code is #11942 and #11943. #11884 and #11886 are open and free. The worktree `~/work/openagents-lane5b` is removed; targets `~/work/openagents-target-agent2` and `~/work/openagents-target-lane5b-app` stay warm.

**Released 2026-10-16 23:04 CDT** by a Claude Code subagent. #11881 is closed (`e37d66493e`, `ade5341924`). #11882 is open and free: the kind and migration landed in `d08cf1dc58`; the simulate handler, registration, and the live run remain. The worktree `~/work/openagents-lane5` is removed; target `~/work/openagents-target-agent2` stays warm.

- **Owns:** `crates/game-lab`, `crates/ship-lab`, `crates/robot-lab`, `crates/video-lab`, their web adapters, and the Lab rows in `apps/openagents` (GPUI `crates/ui/src/lab_card.rs`, mobile layout, iOS cells).
- **Order:** #11881 and #11882 as soon as #11878 lands. #11880 after #11877 and #11878. The app views #11883, #11884, and #11886 after #11876 and #11877. Each view renders `lab-ui`'s Rust Native card; don't write a card per shell.

### Lane 6: Computers and SPACE (the live Codex session)

- **Owns:** `crates/coder-working-computer`, `crates/oa-computers` and the SPACE driver, `deploy/space-service`, `deploy/space-credentials`, the Computers routes, and usage and quota code.
- **Status:** `codex:01a14134` completed #11813 and #11816 and is closing #11800 on source. Its canaries, credentials, workloads, and builder are removed. Lane 0 owns the remaining deployment/live rows and provider-managed empty-network cleanup below. #11823 stays in lane 1.
- **Other completions:** `codex:01a14140` closed #11939 lifecycle recovery, #11940 OpenCode saved agents, and #11944 recoverable-object inventory. Its next source issue is #11886 under lane 5.

### Lane 7: host authority

**Released 2026-10-17 00:06 CDT** by Codex; #11817 and #11911 are closed. No issues remain in lane 7. Source is on main through `f2b101f35b`; deploy and device checks are queued below. Worktree: `~/work/openagents-host-finish` (removed after completion).

**Released 2026-10-16 22:23 CDT** by Codex; #11817 and #11911 remain. #11817 prerequisites landed in `226ebf63a3` and `1dde3bd99a`; the enrolled adapter and project sharing are unfinished. Worktree: `~/work/openagents-bang-lanes` (removed after handoff).

- **Owns:** `crates/coder-host/src/{serve/*,enroll.rs,authority.rs,computer.rs,tasks.rs}`, `crates/coder-access` (store, enrollment, shared host APIs), and `crates/coder-pty`'s command identity.
- **Keep source-compatible, because lane 8 consumes them:** `Host::handle`, the `authority::Grants` check, and `client::Connector`. Changes there must be additive.
- **Don't edit (lane 8 owns them):** the appended device-channel section of `nips/openagents/NIP-HOST.md`, the new app engine serve module, the app sync transport, `crates/coder-sync/src/server/{auth,proof}.rs`, and the mobile toggle.
- **Order:** #11817 first: workspace-scoped file and command admission, symlink and root-escape refusal, and durable command identities across restarts. #11911 builds the project scope on the same admission.
- **Before you edit:** read `.agents/skills/nostr/SKILL.md` and query the code graph for `coder_host::computer`.

### Lane 8: phone

**Released 2026-10-17 01:13 CDT** by Codex (GPT-6) after #11840 completed on `main` (`f9daecd0e8`). All 14 children are closed; deploy and device checks stay in After-close checks, and the missing push capability step is now in `NEEDS_OWNER.md`. The coordinator also closed #11945 and #11947; no unfinished issue claim remains. The `~/work/openagents-codex-coord` worktree is removed after this update; `~/work/openagents-target-coord` stays warm.

**Done 2026-10-17 00:55 CDT** by a Claude Code subagent (session `a9366ded`) that took over #11853 in ~/work/openagents-push. #11853 is closed (`cdb099df8e`, `b653d7e925`); its deploy and device check are in After-close checks. The worktree is removed. #11852 and #11854 stay with the Codex agent.

**Released 2026-10-17 00:50 CDT** by Codex (GPT-6) after closing #11852 (`d37ed99d0c`, `a67ec60b44`) and #11854 (through `e158a12cc2`). No Codex issue remains unfinished. #11853 stays with the push agent above; #11840 remains unclaimed until its children close. Worktree `~/work/openagents-codex-bang` is removed after this update; both lane 8 target directories stay warm.

- **Owns:** `apps/openagents/crates/mobile`, the iOS app, `crates/coder-sync` notifications and auth (`auth.rs`, a new `proof.rs`), the push registration, and the NIP-HOST device-channel profile.
- **Order:**
  1. #11852: closed. Lane 0 runs the deployed account and TestFlight checks listed below.
  2. #11854: closed. Lane 0 runs the deployed pairing and latency checks listed below.
  3. #11853. The owner-only push capability is recorded below.

### Lane 9: CLI, Agent Lab, and web fixes

**Released 2026-10-17 00:13 CDT** by Codex (Grok 4.7). #11934, #11929, #11925, and #11727 are closed on code. After-close checks remain for the #11934 deploy and Foreman live check, the #11925 controlled deploy, and the #11727 named-sale test-mode check. The `~/work/openagents-lane9b` worktree is removed.

The website agents door uses the shared `place` (`1708f44f49`). Deploy and the Foreman live check stay in After-close checks. #11929 is closed on code (`7fb459a5a5`). #11925 is closed on code (`534ecb5c67`). The controlled deploy check stays in After-close checks. #11727 is closed on code (`ed4c8d05c4`). The named-sale test-mode check stays in After-close checks.

**Released 2026-10-16 22:40 CDT** by a Claude Code subagent. #11935 is closed (`264642e0a6`). #11934's `openagents agents` command is on `main` in the same commit. #11928 is closed; the no-traffic production check is in After-close checks.

- **Owns:** `crates/openagents-cli`, `crates/openagents-login`, `crates/openagents-sdk/src/agents*`, `crates/openagents-web/src/{agents_door,attribution,pages/contact,pages/affiliates}.rs`, the web main and gateway startup paths for #11925 and #11928, and `apps/openagents/crates/engine/src/doc_host/agent_lab.rs` for #11929.
- **Order:** #11935, then #11934 (one patch covers both), then #11929, #11925, #11928, and #11727.
- **Known baseline failures:** these `openagents-web` tests already fail on `main` and aren't yours to fix: copy_guard (OpenAPI wording), route_owners, screens, api_docs guides, environments::computers (6), cad_web::world_physics (1), and work_runs::computers (1).

### Lane 10: signet and LayerTree (mostly waiting)

**Taken 2026-10-17 00:42 CDT** by a Claude Code subagent (Opus 5.5) taking over #11863, #11770, #11862, #11793, #11794, #11799, #11762, #11779 in `~/work/openagents-lane10`.

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
- **Status:** source complete. #11801–#11822 are closed; #11816 landed in `23aa58e189` with Computers 88 and SDK 69 checks passing. The [delivery record](../sdk/computers-delivery-evidence.json) distinguishes source closure from pending combined SPACE live qualification.
- **After-close:** lane 0 owns the #11813–#11816 deployment/live rows and the remaining provider-managed empty-network cleanup. No customer SPACE rollout or pricing is qualified. #11823 remains lane 1's independent source work and does not block this umbrella.
- **Owner-only:** none beyond the children's.

**#11813 Computers: authorize credential references and enforce immutable egress profiles on GCloud.** Lane 6, closed on code.
- **Status:** the opt-in implementation includes scoped public/SDK references, immutable network profiles, durable revocation, request-time Google Secret Manager injection, native execution proof, stream revocation, private node token files, Google CA signing, and a private host-network Unix-socket transport. Customer activation stays disabled and Boat stays the default. The [evidence](../sdk/computers-credential-evidence.json) distinguishes source checks, successful provider probes, and failed client attempts.
- **After-close:** lane 0 deploys the exact checked adapter and private broker, then completes the supported clients, bypass, expiry, outage, and revocation checks listed below. The full live client workflow has not passed.
- **Owner-only:** none.

**#11814 SPACE: deploy a private maintained GCloud service with durable storage and bounded capacity.** Lane 6, closed on code.
- **Status:** cleanup and bounded snapshot collection are on `main` (`3e612ba4`), with 50 checks passed. The private capacity-2 fixture passed same-worker memory continuity and seven guest endpoint denials (`c8980129cb`).
- **After-close:** lane 0 owns cleanup-image promotion, worker replacement, concurrent wakes, drain/rollback/placement disable, complete storage inventory, and fixture cleanup; see the table below.

**#11936 SPACE: frame bodyless lifecycle requests for Cloud Run.** Lane 6, closed on code.
- **Status:** the framing fix is on `main`. Capacity-2 passed through Cloud Run, and lane 0 recovered the primary fixture with #11939 (`f83f2538f836`) on private revision `00003-ffl`. Encrypted checkpoint/resume preserved its process and memory witness; all seven endpoint denials passed.
- **After-close:** both fixtures ended stopped. Worker replacement and complete cleanup remain under #11814 below.

**#11815 Computers: persist usage, quotas, and uncertain reservations across backends.** Lane 6, closed on code.
- **Status:** storage allowances and account cleanup are on `main` (`1654ee4da7`), with 71 checks passed. The dedicated production account's usage read passed with pricing off. Deleting a VM does not release its storage reservation; complete cleanup evidence is required.
- **After-close:** lane 0 deploys the allowances and qualifies complete private storage inventory, limits, and account cleanup on synthetic fixtures.
- **Owner-only:** customer pricing and entitlement remain in `NEEDS_OWNER.md`.

**#11816 SPACE: qualify the public SDK and web-chat path, recovery, and staged rollout.** Lane 6, closed on code.
- **Status:** the offline Rust qualification command checks pinned deployment identities, dated case receipts and raw artifact hashes, finite account scope, measured cold/snapshot/wake coverage, and explicit latency, error, and service-cost budgets. The SDK example supports explicit placement, saved settings, and stage timings; resumed runs cannot become fresh latency samples. A registry regression preserves old SPACE and Boat bindings when new SPACE admission stops.
- **Evidence:** the [runbook](../sdk/space-qualification.md), [packet](../sdk/space-qualification-packet.json), and [assessment](../sdk/space-qualification-assessment.json) retain pending combined-deployment cases. Neither named-group nor broad customer rollout is qualified; pricing stays off and Boat stays the default.
- **After-close:** lane 0 completes the combined public SDK/web flow, recovery and fault cases, deployment rollback, measured budgets, and fixture inventory in the table below. Source completion does not replace those live results.

**#11817 Computers: implement an enrolled-own-machine adapter with safe attach and detach semantics.** Lane 7, code complete.
- **Status:** `226ebf63a3`, `1dde3bd99a`, and `6cb641acc9` establish durable launch identities, descriptor-based workspace files, signed account/root scopes, and bounded host commands. The Core `computer-enrolled` driver and explicit account-bound service placement now connect those pieces. Automatic placement excludes enrolled hosts. Delete and expiry detach logical access; unsupported cloud controls and credentials refuse.
- **Checks:** scoped access and host libraries (120 and 118 tests), driver capability/storage checks (11), lifecycle library and adapter checks (27 and 9), signed direct/relay adapter checks (2), and service library plus disposable-Postgres API checks (23 and 1). Formatting covers changed files only. Two pre-existing Linux-oriented `computer-driver::identified` shell tests time out on macOS, even with GNU tools; they are outside the changed path.
- **After close:** lane 0 deploys the new host and service, then runs the dedicated-account check below. The initial implementation requires a Unix filesystem boundary, an explicitly configured root, and a scoped device grant. Setup: [`computer-enrolled`](../../crates/computer-enrolled/README.md).
- **Owner-only:** none.

**#11823 Coder: consume the public Computers SDK for remote work and agent tools.** Lane 1, not started.
- **Status:** the freeze no longer applies. Its prerequisite #11806 is done.
- **Remains:** replace direct allocation, launch, file, and command transport in `coder remote delegate` with `openagents::computers`. Keep `--on boat`'s meaning, logical retry handles, and the separation of detach from cancel. Add computer tools only for implemented capabilities. Run a dedicated-account staging task.

**#11925 Accounts: keep authenticated reads predictable during gateway handoff.** Lane 9. Closed on code.
- **Status:** on `main` (`534ecb5c67`). An account lookup that fails for transport or a 5xx is tried once more. A refused session is not tried again, and a result is not remembered for a later call. Computer 503 responses include `Retry-After`. A delete that cannot be checked is not sent to the computer service. Trace `ed19af0410b5b236d6ccb7d3cd5d0676`.
- **Checks:** `cargo test -p openagents-web --lib account_handoff_retries_a_read_and_does_not_replay_a_delete` passed (1).
- **After-close:** the controlled deploy check is in [After-close checks](#after-close-checks).

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

**#11880 Labs V2: Video Lab on the platform.** Lane 5, closed.
- **Done:** the kind's job requirements and tests, the `render`, `frames` and `measure` worker handlers (`video_lab::runner`; `video-lab render-job|frames-job|measure-job --ffmpeg PATH --ffprobe PATH`), and `lab_private.json` as asset classes (`video_lab::private`), on `main` in `a8e27eb97c`. The rest of the code is #11942; registration and the live run are in [After-close checks](#after-close-checks).
- **Depends on:** #11877, #11878.
- **Remains:** Rust job runners (ffmpeg, Blender, `film_shot`, whisper.cpp), enrolled-host placement for `private_local` assets, the Rust loopback service on 8792 behind `lab.py serve`, import of the local films, `/labs/video/{doc}` pages, and the three acceptance runs.
- **Simplification:** the freeze is lifted, so Coder can call the platform directly. Keep loopback mode only where the local web chat or `openagents lab` still needs it.
- **Owner-only:** importing `~/work/film-out/lab/films/*` runs on the owner's Mac.

**#11942 Labs V2: Video Lab remainder.** Lane 5, closed.
- **Done:** titles and captions drawn in Rust (`video_lab::text`), push-in on held frames, levelled speech and ducked music in `render`; `measure` lays out titles and captions and reads beats and speech sidecars; `transcribe` (whisper.cpp with `lab_words.json`), `import` and `score` (`music.py`) handlers; jobs read the worker's downloaded stored files, so public inputs work on a cloud worker (`e6ebb67e46`). `video-lab-serve` serves the 8792 routes with `lab.py` as its engine, and `lab.py serve` starts it when built (`a3ef319562`). `cargo test -p video-lab` 37/0, `cargo test -p video-lab-serve` 3/0, and a loopback smoke with a temporary HOME passed.
- **Follow-up:** the player and timeline on `/labs/video/{doc}` over `/v1/labs` are #11949. Shot and scene renders (Blender `studio.py`, `verse --example film_shot`) are in [After-close checks](#after-close-checks): Blender isn't installed here.

**#11881 Labs G1: Game Lab and Ship Lab as platform kinds.** Lane 5, closed.
- **Done:** `game_lab::kind::Game` and `ship_lab::kind::Ship` on `main` (`e37d66493e`, `ade5341924`), with the `game-lab playtest-job` worker handler. Parity tests run the same tool edits through the local services and the kinds. The loopback routes are unchanged.
- **Waiting on lane 4:** registering both kinds in `crates/openagents-web/src/labs_api.rs`, next to CAD, and the `/labs/game/{doc}` page. #11877's patch edits that file, so lane 5 left it alone. The rest is in [After-close checks](#after-close-checks).

**#11882 Labs R1: Robot Lab documents and runs on the platform.** Lane 5, open and free.
- **Done:** `robot_lab::kind::Robot` on `main` (`d08cf1dc58`): the four schemas with the local store's checks and wording, `pose` and `move_object` intents, host-checked dependencies, the `step` note locator, `preview` and `simulate` declarations, and the cards. `robot_lab::migrate::plan` reads a local project in dependency order and rewrites references to platform content digests; `migrate::import` (feature `platform`) writes it into a `lab_store::LabStore` with notes and files. `cargo test -p robot-lab --features platform` passed.
- **Remains:**
  1. The `simulate` handler. A job's `Work` carries only the experiment version, but `robot-sim` needs its scene, robot, and mesh files. A worker needs a way to read the referenced versions and files, which is lab-jobs and lab-api work.
  2. Register the kind in `labs_api.rs` (lane 4's file during #11877), and add the web replay card.
  3. A simulation run on a cloud worker with a test account, shown on the web and through `/v1/history`.

**#11883 Labs D1: desktop Lab row and the standard views.** Lane 5, closed.
- **Done:** saved chats' Lab cards draw `lab-ui`'s Rust Native card in the desktop app, with image and strip, compare, video, Model3d and playable posters, and reports (`19493ba7b6`, `408569fa70`). The rest of the code is #11943; the test-account run is in [After-close checks](#after-close-checks).
- **Status:** the native Video card with inline AVFoundation playback is in `apps/openagents/crates/ui/src/lab_card.rs` (loopback only).
- **Remains:** the image, compare, Model3d, and report views, cards from `/v1/history`, live updates from `/v1/labs` events, and fixture screenshots. Render `lab-ui`'s Rust Native card through the GPUI adapter.

**#11884 Labs P1: iPhone Lab row and standard views, notes by voice.** Lane 5, closed.
- **Done:** the phone's saved transcript draws `lab-ui`'s card as a Lab row in the Rust layout, with a UniFFI facade on `CoreSavedHistory` (card files, media, page, notes at the playing time, following events while visible), and one thin Swift cell per view (`LabCardView.swift`). Playback stops on background, account switch, and sign-out (`29d4e808fd`, `a4c68fd920`). The simulator and test-account runs are in [After-close checks](#after-close-checks).

**#11943 Labs D1b: desktop Lab row remainder.** Lane 5, closed.
- **Done:** a desktop Lab row follows its document's `/v1/labs` events, keeps its pictures and playing video when the card changes, and adds notes at the playing time (`0e18a669df`). Typed rows in live sessions, a version list, and Model3d orbit are #11948, which waits on #11942.

**#11885 Labs T1: Labs in Coder 1.0.0.** Lane 4, closed.
- **New scope:** native Labs in `coder` and `openagents` through `openagents::labs`, not a bridge for a frozen binary. Read tools from `/v1/labs/kinds`, follow events, and keep the `VIDEO_LAB_URL`-style loopback only for local Labs not yet on the platform.
- **Done:** native SDK discovery, calls, and events in Coder and `openagents`, shared `lab-ui` terminal text and links, verified CLI previews, and retained local tools for unhosted kinds (`d37b9a9b6b` through `0cedf5dbf1`). Adapter, native client, generated-tree, and isolated-HOME CLI checks passed. Evidence: `docs/sdk/labs-terminal-evidence.json`.
- **After-close:** the production terminal flow and visual check are listed below. The separate Cursor screen test failure is recorded in the evidence.

**#11886 Labs D2: video timeline editor in the desktop app.** Lane 5, last in the lane.
- **Depends on:** #11880, #11883.
- **Risk:** likely to miss Saturday. Close with what landed if it does.

### Chats, sessions, and moving agents (#11896 epic)

**#11896 Epic: OpenAgents chats are normal, resumable chats.** Lane 1.
- **Status:** #11892, #11893, #11894, #11895, and #11897 are code-complete. #11898 remains open.
- **Remains:** close #11898.

**#11894 Continue a terminal or phone chat on this computer, as a real session.** Lane 1, code-complete.
- **Status:** `f3267187d5` routes terminal sessions through the shared native harness and removes duplicate drivers. `8453e68318` updates the Microcoder consumer, `0a745fa94c` makes the shared room client available without SQLite, `f12035c392` adds SDK chat and room clients, and `0b84201f39` publishes screened terminal text to the same conversation ID in Loro. `574ec67ab8` stabilizes the steering deadline fixture. The earlier continuation foundation remains in `cca7f94ebe`.
- **Checks:** bridge 259, publisher 44, Microcoder 141, SDK chat and continuation fixtures 3, and app sync 56 without the store and 73 with it. The full Microcoder run found an unrelated assertion expecting “signed in” after `95a0145d3a` changed the copy to “logged in”; that one assertion is excluded from the passing focused run. Twelve optional tests are ignored. Changed Rust files are formatted, and `git diff --check` passes.
- **After close:** lane 0 deploys the new binaries and runs the dedicated-account terminal-to-desktop context and phone steering check listed below. Custom account deployments must set `OPENAGENTS_SYNC_URL` to their matching sync service.

**#11897 Show computer sessions as rows on openagents.com.** Lane 2, not started.
- **Remains:** a summary-only session index per account, rows mixed into the web list with an "On <computer>" badge and offline state, a summary page with app and continue actions, a per-chat and per-project transcript opt-in (off by default), `source: "session"` rows in `/v1/history`, and access and revocation checks.

**#11898 "Where it runs" is a property of the chat.** Lane 1, after #11894.
- **Status:** the foundations are in `cca7f94ebe`.
- **Remains:** the "Runs on" control, computer-to-cloud continuation through the #11893 path, paired and cloud computer choice, one conversation row across web, terminal, desktop, and phone, and the web, Mac, then cloud round trip on a test account. The cloud computer choice needs #11852.

**#11911 Computer sharing: owner-approved project-scoped access over NIP-HOST.** Lane 7, closed.
- **Status:** `72a0bc730c` adds device-pinned project invitations, absolute expiry, separate scopes, and scoped revocation. `28994246be` requires `project.redeem` and `project-sharing-v2` so an older host cannot accept a project invitation as broad enrollment. `0d5bca7a54` adds the account-bound client and shared Rust Native form. `f2b101f35b` mounts that form in Settings, cancels work on account changes, and stores device keys and grants separately for each account. It offers only implemented project files and commands; broad HOST tasks, raw terminals, and general app RPC remain unavailable to guests.
- **Checks:** access library 124, host library 118, sharing crate 12 (including a real signed local relay and two synthetic accounts), mobile library 58 with one benchmark ignored, regenerated Swift bindings, and Swift typechecking for all 54 app/shared sources. The mobile account-switch check rejects callbacks from the earlier account after another account signs in. Unsupported files cannot be saved as empty text without an explicit edit.
- **After-close:** deploy and dedicated-account/device checks are listed below. Physical iPhone and accessibility checks are in `NEEDS_OWNER.md`. No owner account or running host was used for these checks.

**#11929 Agent Lab: run tests for agents that don't need a project copy.** Lane 9. Closed on code.
- **Decided (owner, 2026-10-16):** option 1, an experiment-level "no project files" choice that runs the trial in an empty folder. The report says the agent had no project files.
- **Status:** on `main` (`7fb459a5a5`). The website accepts a host report only when it matches that choice. `cargo test -p agent-lab --features views project_files` passed (2). `cargo test -p openagents-web --lib agent_lab::tests` passed (2). `cargo test -p openagents-engine --lib no_project_files` passed (1).

**#11930 Move running agents: keepers.** Lane 3. Closed on code.
- **Status:** the keeper layer (`16629755e6`) and Claude Code and Codex under keepers by default (`b49a54a201`). A new engine replays the journal through a fresh driver, drops events the old engine handled, asks pending questions again, and continues Codex request ids. `OPENAGENTS_KEEPERS=0` starts agents directly.
- **Remains:** the live restart check in After-close checks.

**#11946 Move running agents: engine update handoff and terminal keepers.** Lane 3, after #11930.
- **Remains:** `restart_when_superseded` hands kept runs to the new engine instead of waiting for no live run; terminals (PTYs) under keepers; the other native drivers' `attach`.

**#11931 Move running agents: background engine by default.** Lane 3. Closed on code.
- **Status:** `c85eb539b7`. `crates/background` writes each data folder's LaunchAgent or systemd user unit and decides attach, start, or embed; the app runs it before embedding. The macOS menu-bar item has Open, Move to background, and Stop all and quit; Move all to a cloud computer is commented out until #11933. On a disposable Debian 12 VM, `openagents background ensure` installed and started the unit, and a second run attached without reinstalling.
- **Remains:** the live turn checks in After-close checks.

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
- **Status:** closed on code. All 14 children (#11841–#11854) are closed, and the final cloud, pairing, and push commits are on `main`. Their closing comments record the source checks. Lane 0 owns the remaining child checks below; the push capability and physical iPhone steps are in `NEEDS_OWNER.md`.

**#11852 Cloud computers: run the headless engine on a computer, so the phone works without a desktop.** Lane 8.
- **Status:** closed. Server and SDK source is on `main` in `3063ab036c`, `1cdf329dfc`, `048005fe73`, and `4a796e8ff8`; phone controls and the local acceptance fixture are `d37ed99d0c` and `a67ec60b44`.
- **Finding:** cloud computers suspend after every command, so the engine runs as one command of up to an hour.
- **Checks:** mobile tests passed (57, one existing ignored), the normal simulator core and native app built, and the local UI-to-SDK fixture passed start, status, stop, and sign-out with exactly one start and stop.
- **After-close:** the deployed dedicated-account run, usage and deletion checks, and TestFlight release are listed below.

**#11853 Push notifications through our push gateway.** Lane 8.
- **Status:** closed. The server sends wakes through the push gateway (`cdb099df8e`), and the phone core registers with the gateway (`b653d7e925`). The deploy and device check are in After-close checks.
- **Checks:** push delivery 3 (isolated PostgreSQL), notification rules 17, push-gateway 17, and mobile push 5 passed.
- **Owner-only:** the App ID's Push Notifications capability (before any build sets `CODER_IOS_PUSH`) and the real-device check.

**#11854 Carry the device channel over NIP-HOST, with Nostr device proofs.** Lane 8.
- **Status:** closed. Shared prerequisites are `ca2be8e8dc`, `f99a91e211`, and `becca883a3`; the app, proofs, and packaging landed in `1654cdcc6f` through `e158a12cc2`. The optional app channel uses the existing host and connector, account-scoped keys, explicit phone pairing, admitted unary RPCs, stable command IDs, and exact-request NIP-98 proofs. Sync checks registered host keys and current grants; proved sockets expire independently of account checks. The existing Loro transcript path and default WebSocket route remain in use.
- **Checks:** access 125, host 118, control 16, chat 198 (one existing ignored), ledger 31; app client 26, engine 519 (two existing ignored), mobile 58 (one existing ignored), RPC 18, sync 73; sync service 3 and isolated PostgreSQL auth conformance 4 passed. After merging the push server and migration 7, auth 4, push delivery 3, and push rules 17 passed together. Normal iOS simulator core generation and native build-for-testing passed. The archived Docker build context resolves with the locked Linux dependencies and includes the embedded Nostr schemas. The [NIP-HOST profile](../../nips/openagents/NIP-HOST.md#local-latency-comparison) records a 30-call-per-route local comparison with no failures; the deployed comparison is still required before changing defaults.
- **After-close:** lane 0 deploys the server and runs the dedicated-account, device, revocation, expiry, fallback, and deployed latency checks listed below.

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

**#11727 Affiliate attribution, part 2.** Lane 9. Closed on code.
- **Status:** on `main` (`ed4c8d05c4`). A contact request records the consented click as a reference. A short `/r/{handle}` name stays beside the long link. Settings offers the link under the existing sign-in. The public terms page stays unlinked.
- **Checks:** `cargo test -p tenancy --lib an_owned_handle_survives` passed (1). `cargo test -p openagents-web --lib` for the contact attribution, the link page, and unknown codes passed (3). `cargo test -p gateway --test accounts an_owned_short_handle` passed (1).
- **After-close:** the named-sale test-mode check is in [After-close checks](#after-close-checks).
- **Owner-only:** publishing the terms (`tenant-referrals publish-affiliate`), setting `inference.sats_rate`, linking `/affiliates`, and sending payouts.

## After-close checks

Deploys and live checks that no longer hold an issue open. Lane 0 works through them in order, after each production deploy. Add a row when you close an issue with a check still to run. When a check passes, mark it done with the evidence. When it fails, open a new issue and link it.

| Issue (closed or closing) | Check | Status |
| --- | --- | --- |
| #11817 | Deploy the new host and computer service. Configure a disposable Unix host, scoped test-account grant, and explicit enrolled target. Run the public attach, command retry/reconnect/cancel, relative-file, revocation, unsupported-control, and detach flow; verify files and unrelated grants survive. Repeat the Linux-specific command checks on Linux. | source checks passed; deploy and dedicated-account run to do |
| #11911 | Deploy a host advertising `project-sharing-v2` and the matching mobile build. With two dedicated test accounts and disposable projects, check owner approval, read-only and edit/command permissions, wrong-device and cross-project denial, retry/reconnect, expiry, scoped revocation, and account switching. Confirm unrelated chats, files, terminals, credentials, and account controls remain unavailable. Physical device and accessibility steps are in `NEEDS_OWNER.md`. | Source checks passed: access 124, host 118, sharing 12, mobile 58; deploy, live, and device checks to do |
| #11934 | Deploy the agents door from `1708f44f49` or newer. Run `ask_foreman_cli.py` and `openagents agents ask Foreman …` with test account `acct_500d3e1d125b7782`. The web door's switch to `place` is on `main` (`1708f44f49`). | Deployed in source `0b74ec8f79`, production smoke 66/0/2. Native list/status, native `agents ask`, the website saved-agent request, and an unchanged normal answer passed with an isolated Rust SDK runner; fixtures were removed. [Evidence](../sdk/agents-evidence.json). OpenCode placement fails as unsupported (#11940). The original Claude Code Foreman workflow remains to do. |
| #11940 | Rebuild the headless engine with the OpenCode saved-agent fix. On a disposable host and dedicated account, ask Foreman with the advertised OpenCode model and host-default access; confirm its saved instructions are followed, then remove the agent, assignment, chat, and host. Repeat the original Claude Code flow under #11934. | Source checks passed: saved-agent admission 5, Agent Lab consumer 4, OpenCode harness 73, and the terminal consumer build on Rust 1.97.1. Deploy and dedicated-account run to do. |
| #11930 | On a test host with the dedicated test account, start a real Claude Code turn and a real Codex turn in the app, quit or kill the engine mid-turn, and relaunch. Each turn continues in the same transcript entry with no gap or duplicate, a pending question comes back, and a steer after relaunch reaches the agent. Then repeat with `OPENAGENTS_KEEPERS=0` and confirm the old auto-resume. Delete the chats afterward. | Fake-CLI checks passed: harness `keeper_drivers` 7, `keeper` 5, `claude` 25, `codex` 35; engine `keepers` 5; `cargo check -p openagents`. Live run: to do |
| #11925 | Deploy `534ecb5c67` or newer. With a synthetic account, confirm an account-backed computer read either succeeds or returns 503 with `Retry-After` during a traffic shift, and a delete during that gap is not applied. | to do |
| #11727 | Deploy `ed4c8d05c4` or newer. After Stripe test mode is set up (owner, 2026-10-17), a named sale through a short link records the introduction. | to do |
| #11936 | Redeploy the SPACE candidate with explicit zero-length framing and verify suspend and wake through Cloud Run. | Capacity-2 checks passed on `00002-pch`. The first fixture also passed after #11939 recovery on `oa-space-service-computers-00003-ffl`: encrypted checkpoint/resume preserved boot ID, PID, start time, and memory canary; seven protected endpoints were denied. Both fixtures ended stopped. [Earlier evidence](../cloud/evidence/2026-10-16-space-private-foundation.json); new recovery evidence is in `docs/sdk/computers-usage-evidence.json`. |
| #11939 | Deploy the checked private candidate, run the explicit operator retry for the saved rejected suspend, then qualify encrypted checkpoint and resume. The [usage evidence](../sdk/computers-usage-evidence.json) has the exact request and expected child. | Passed: 31 source checks, private image `sha256:a58dc392…4c8db`, expected retry child, encrypted checkpoint/resume, and seven network denials. Operator execution `oa-space-retry-11939-q2j8l` succeeded; its temporary job was deleted. Evidence: `docs/sdk/computers-usage-evidence.json`. |
| #11813 | Deploy the checked native credential adapter and private broker with pool-scoped Google CA signing, an explicit numeric issuer resource, private Unix-socket transport, pinned public CA trust, registered synthetic sources, and named test-account admission. Complete real Claude Code streaming, Git clone, gh, suspend/wake with fresh references, restored-marker denial, active-stream revocation, expiry, broker/provider outage, redirect/subdomain/path/method/IP/DNS bypass, IPv6 refusal, and host-control versus guest denial checks. Remove every fixture afterward. | source and provider checks are recorded in [credential evidence](../sdk/computers-credential-evidence.json); full client workflow and customer activation remain unqualified |
| #11800 | Remove the empty `oa-sdk-build-11800` subnet and VPC after Google releases managed SERVERLESS address reservation `serverless-ipv4-cloudrun-1792204802249859935`. Verify the scoped [cleanup inventory](../sdk/computers-delivery-evidence.json) before deletion; do not touch the maintained SPACE networks. | All disposable VMs/disks, Cloud Run probes, secrets, CA pool, and firewalls are removed. The credential spike subnet/VPC are deleted. One Google-managed address reservation still holds the empty builder network; no application workload remains. |
| #11816 | Follow the [qualification runbook](../sdk/space-qualification.md) on the combined checked deployment: dedicated-account public SDK flow and useful web task/follow-up; account isolation; actual snapshot mode and shape; memory/workspace and fresh credentials; interrupted or duplicated requests and child cancellation; worker loss, fencing, checkpoint/database recovery, storage/provider outages; credential and network enforcement; capacity, quotas, retention/account deletion/orphans; upgrade, rollback, disabled placement, and legacy Boat. Retain pinned dated receipts, cold/snapshot/wake distributions with cache/concurrency, complete service economics, and cleanup inventory. Assess the packet before any named-group admission. | Source checks and operator tooling complete; combined live qualification pending. No customer rollout or pricing enabled. |
| #11814 | Promote the checked cleanup images, install eleven measurements and nine alert policies, then run encrypted lifecycle, worker replacement, concurrent wakes, drain/rollback/placement disable, guest denial, retention, and fixture cleanup. Use the [private service runbook](../../deploy/space-service/README.md) and [receipt](../cloud/evidence/2026-10-16-space-private-foundation.json). | source checks passed 50/0; capacity-2 memory and seven denials passed. #11944 qualified the read-only recoverable-generation inventory: nine soft-deleted objects still occupy 110,411,986 bytes. Complete cleanup, multipart, replacement, rollback, and remaining deploy checks: to do |
| #11875 | The dedicated-account CAD run on staging and then production: fan bracket, picked face, 5 mm thicker, project versions, STEP download, cleanup. | Closed. Staging passed 18 CAD checks, browser selection, and cleanup. Production smoke passed 66/0/2; CAD operations and browser selection passed, but API project cleanup failed (#11938, then #11941). The leftover test project was removed through the website. Evidence: `docs/sdk/labs-cad-staging-handoff.json`. |
| #11938 | Deploy the sign-in-key account lookup fix, then repeat `/v1/projects` and the complete production CAD run with the original dedicated test key, including project deletion. | Session tests passed 13/0; project API tests passed 6/0. The earlier leftover test project was removed through the website. Deployed with source `0b74ec8f79`; app identity works. Project and thread lists still fail on account metadata files (#11941). |
| #11941 | Deploy the GCS chat-list fix, then check the original dedicated account's project and thread lists and repeat CAD cleanup. | Account files in the chat folder caused the 503. The fix filters to chat IDs; 15 storage tests and six project API tests passed. Deployment and the production recheck remain to do. |
| #11876 | Run the Git-pinned `examples/sdk-labs` CAD flow against production with the dedicated account, including ranged GLB, notes, events, and deletion. | Passed on production image `sha256:51e2d77b…202a7` using the previously qualified Git-pinned binary: CAD edits/refusal, restored job retry, ranged GLB, notes, 14 events, design deletion, and artifact refusal after deletion. Evidence: `docs/sdk/labs-client-evidence.json`. |
| #11877 | A CAD chat's card through `/v1/history`, re-rendered in a second browser session. | to do |
| #11878 | Deploy the account-service worker routes before the web worker routes. Run dedicated-account Echo on server, disposable enrolled, and cloud placements; prove private input bytes and paths stay local, retire scoped keys, and observe cloud computer deletion. The cloud environment needs the pinned `labs-worker` and trusted handlers installed. | Platform API checks passed on staging and production (21 each, with cleanup). Worker placement, private-input, key retirement, and cloud-deletion checks remain; owner Mac installation is in `NEEDS_OWNER.md`. |
| #11885 | After the server deploy and native binary build, use the dedicated test account in an isolated HOME: edit a CAD document with `openagents cad`, edit a hosted Video document with Coder, follow its events, open the page links, and delete the fixtures. Check posters and strips in Kitty and iTerm2. Hosted Video depends on #11880; Game and Ship integration remains under #11881. | Local SDK wire, native dispatch, generated-tree, and actual CLI fixture checks passed; production and visual checks: to do. |
| #11840 | Complete the existing #11852, #11854, and #11853 rows below, including the signed app build and dedicated-account iPhone checks. | All 14 children are closed, and final source commits are on `main`. Deploy and device checks remain with lane 0; the push capability and paired-phone steps are in `NEEDS_OWNER.md`. |
| #11852 | Deploy `4a796e8ff8` or newer. Run `OurCloudLiveTests/testStartRunAndStopOurCloud` with the dedicated test account, check `GET /v1/computer-usage`, delete the computer with `DELETE /v1/cloud-host`, and remove its test registry rows. Validate TestFlight 1011 or the next free build. | to do; phone source `d37ed99d0c`, local control acceptance `a67ec60b44` passed |
| #11854 | Deploy the sync server with migrations 6 and 7 and the correct `CODER_SYNC_PUBLIC_ORIGIN`, then build the opted-in app/headless host. With the dedicated account and disposable devices, pair the phone to the registered computer, run a real model call and a command with its original ID, and confirm the transcript stays in its existing Loro room. Check unsupported and streaming fallback, sign-out, forget/revoke, account switching, and proved-room expiry without a permission downgrade. Compare NIP-HOST and deployed device-room latency, recording revisions, routes, sample counts, and failures before changing defaults. Remove connect-code files and test devices, registry rows, hosts, and chats. | Source and isolated checks passed; normal iOS simulator build passed. Deploy, dedicated-account/device run, and deployed latency comparison: to do. |
| #11853 | Deploy `coder-sync` from `b653d7e925` or newer with `CODER_SYNC_PUSH_*` set, and add its key to the gateway's `PUSH_GATEWAY_RELAY_PUBKEYS` (`docs/deployment/push-gateway.md`). After the owner adds the Push Notifications capability, build with `CODER_IOS_PUSH` set and, on a real device with the dedicated test account, check a finished run, a question, and a failure each wake the phone once, and that an open app and an `X-OpenAgents-Test: 1` write don't. | Source checks passed: push_rules 17, push_notify 3, push-gateway 17, mobile push 5. Deploy and device check to do. |
| #11894 | Deploy the Coder and app builds containing `0b84201f39` or newer. With a dedicated test account and `/sync on`, continue a terminal chat on the desktop, confirm its first turn knows the history, and steer it from the phone. | Source checks passed; deploy and dedicated-account run to do. |
| #11881 | Register `game_lab::kind::Game` and `ship_lab::kind::Ship` in `labs_api.rs` after #11877 lands, host `/labs/game/{doc}`, and install `game-lab playtest-job` as the `playtest` handler. Then, with the dedicated test account on production: a Game Lab edit plays in the web card and appears through `/v1/history`, and a Ship Lab refusal and an accepted edit. Clean up after. | to do |
| #11880 | Register `video_lab::VideoKind` in `labs_api.rs` next to CAD (lane 4's file), and install `video-lab render-job`, `frames-job` and `measure-job` as handlers on an enrolled test host. Then, with the dedicated test account on production and synthetic clips: create a cut, make one edit and one refused edit, render, play the card in web chat and through `/v1/history`, and delete the fixtures. A `private_local`-marked synthetic document renders only on the enrolled host. A cloud render needs public inputs on a worker (#11942). | to do; `cargo test -p video-lab` passed 27/0 with synthetic ffmpeg clips |
| #11942 | Install `video-lab transcribe-job` (with `--whisper` and `--model`), `import-job` and `score-job` (with `--python` and `--music`), and `render-job --fonts DIR` on the enrolled test host, and run each on a synthetic test-account cut. Render a synthetic cut on a cloud worker from stored public clips with the Mac off. Run the MVP script in the local web chat and Coder 1.0.0 against `video-lab-serve` (`full-local.sh --lab`), and check Episode 290 still renders through `lab.py` (owner's Mac). Add `shot.v1` and `scene.v1` render handlers where Blender and the Verse film assets are installed. | to do; source tests passed (video-lab 37/0, video-lab-serve 3/0) |
| #11884 | Build the iOS app with its Rust core phase from `main` (`com.openagents.app`). Run simulator fixture UI tests for each Lab view (Video plays and goes full screen, image and strip, compare, Model3d and playable open the page, report) and check memory stays bounded while scrolling past videos. With the dedicated test account, open a web CAD chat and a Video chat on the phone, dictate a note while the video plays, and see it on the web at that time. Delete the chats afterward. | Rust layout, facade, and account tests passed; the Swift typechecks against the iOS 26 simulator SDK. Simulator and test-account runs: to do |
| #11943 | With the dedicated test account, open a Video chat in the desktop's saved history, play it, add a note at the playing time, and see it on the web; start a job from the web and see the card update without reopening the chat. Capture fixture screenshots of each view. Delete the chats afterward. | GPUI tests passed (fixture cards, newer card keeps its pictures, note with its time) and the account fake-server test for notes and events; live run: to do |
| #11883 | Build the desktop app from `main`. With the dedicated test account, open a web CAD chat and a Video chat in the desktop's saved history: the cards draw and their pictures load, Open document opens the page, and signing out clears them. Delete the chats afterward. | to do; account tests 15/0 (fake history and artifact server), GPUI fixture test over every `lab-kit` card passed |
| #11815 | Deploy storage allowances and account cleanup through lane 0; collect complete private storage inventories, including recoverable versions, and qualify limits/cleanup on synthetic fixtures. | Source checks passed 71/0 on rebased main, including five storage/cleanup cases. Usage read passed with `acct_83d16a35955ef908`: HTTP 200, pricing off, no charge; the earlier account remains outside the rollout. [Evidence](../sdk/computers-usage-evidence.json). Storage deployment and live cleanup: to do. |
| #11928 | On the next production deploy, a candidate with no traffic does not resume setups or delete homes. | to do |
| #11932 | Build the app from `main`. With the dedicated test account, a disposable repository, and a cloud computer with Claude Code and Codex logins registered as credential sources, run `move_cloud::Mover` for a paused Claude Code chat and a Codex chat: each resumes on the computer with its files, untracked ones included, and its conversation. A chat without a registered login stays local with its reason. Check the Codex `exec --json resume` arguments and the login variables (`CLAUDE_CODE_OAUTH_TOKEN`, `CODEX_API_KEY`) against the real harnesses. Delete the computer and chats afterward. | to do; `cargo test -p openagents-engine --lib move_cloud` passed 8/0 (fake Computers server, temporary repository, fake session files) |
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
