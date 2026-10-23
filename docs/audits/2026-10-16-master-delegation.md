# Master delegation: every open issue, where it stands, and who takes it

Written 2026-10-16 21:50 CDT (2026-10-17 02:50 UTC). It covers all 48 open issues and every comment on them, read after the owner's STOP and consolidate order at about 21:35 CDT.

This document governs how the remaining work is split. Read it with the [big-bang release brief](2026-10-16-coder-1.0-big-bang.md), which sets the goal and the rules. Where the two disagree, the brief wins on policy and this document wins on status and assignment.

**Goal:** close every open issue by **Saturday 2026-10-17, 21:00 CDT** (2026-10-18 02:00 UTC). Then the release gate runs through Sunday, and Coder 1.0.0 ships on **Monday 2026-10-19**.

> **2026-10-17 07:45 CDT:** the coordinator handed off. Start with the [handoff](2026-10-17-handoff.md).

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
| Production web | `coder-web-1726945564-20261017074218` at 100% | 0 | Web `sha256:17269455…fc0f`, account-service `sha256:b9c6fe38…eb9c`, source `b50cb54165`. Production smoke 66/0/2; CAD 18, Echo 21, two fresh browser history renders, and cleanup passed. |
| Staging web | `openagents-web-1-staging-b50cb54165-072237` at 100% | 0 | Same images as production. Standard smoke 99/0/2, CAD 18, Echo 21, two fresh browser history renders, and cleanup passed. |
| Coder sync, staging and production | `coder-sync-staging-00010-znx`, `coder-sync-00010-bbv`, both at 100% | 0 | Source `c983085ff5`, image `sha256:b522158f…87ce`; health, exact public origins, and dev-bearer denial passed. Pairing, latency, and push checks remain. |
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

**Taken 2026-10-16 22:04 CDT** by Codex working deploys and releases for #11875, #11934, #11852, #11936, and the after-close follow-ups #11938, #11941, #11950, #11953, #11954, #11955, #11956, and #11957 in `~/work/openagents-bang-lane`.

**Checkpoint 2026-10-17 03:16 CDT.** All source follow-ups held by this agent are closed on `main` with their own checks passing, through #11957 (`84014b7396`). Lane 0 stays claimed. The worktree is removed after this checkpoint; `~/work/openagents-target-0` and branch `bang-lane` stay available. Remaining work is in After-close checks, the API-address rows in `docs/coder/next-release.md`, Saturday night’s candidate gate, and Monday’s final release. The Monday binary must include #11771 (`cd193284a8`); its completed source row is removed from the release-work list.

- **Owns:** every web deploy (staging and production shifts), every release candidate cut, the final 1.0.0 publish, the project boards, and `NEEDS_OWNER.md` hygiene. Other lanes ask lane 0 to deploy. They never shift traffic themselves.
- **Tonight:**
  1. Run `scripts/dev/project-status-sync.sh 22` and `19`. Fix the stale assignees: #11934, #11840, #11762, #11862, #11863, #11770, #11794, and #11799 show AtlantisPleb with no live claim. Leave #11813, #11814, and #11936 alone (lane 6 is live).
  2. Take the next production deploy from current `main`. It carries #11875's CAD platform, the #11934 agents door, and `4a796e8ff8` for #11852. Lane 0 runs the CAD staging check from After-close checks, promotes the tested image, runs the production smoke, and works through the remaining live checks.
  3. Remove the patch files from `docs/audits/` once lane 4 has applied #11877's patch. A `.patch` file isn't a document.
- **Saturday evening:** cut `1.0.0-rc.23` from `main` once lanes 1, 4, 7, and 9 have landed. Run the gate the brief describes, and cut later candidates as fixes land.
- **Monday:** follow the brief's re-cut steps. Reinstall every copy of the withdrawn 1.0.0.
- **Next-release rows that must ship in 1.0.0** (`docs/coder/next-release.md`): the #11771 memfd credential handoff (the source fix is closed, and the binary must carry it), both `api.openagents.com` address moves (check `deploy/production/api-url-map.yaml` first), and #11823. Remove each row as its work lands.

### Lane 1: harness and chats (critical path)

**Done 2026-10-17 CDT** by Claude Code in `~/work/openagents-11823` (removed after completion): #11823 is closed. `openagents computer cloud ...` and Coder's `cloud_computer` tool use `openagents_sdk::computers`; `--on boat` and `--on gce` keep their paths ([cloud computers](../coder/runtime/cloud-computers.md)). Its dedicated-account run is in After-close checks.

**Done 2026-10-17 04:27 CDT** by Codex in `~/work/openagents-labs-finish`: #11896 is closed after its six children closed. #11823 stays with its owner; #11871 is released and blocked on #11948.

**#11896 closed:** its six children are closed, their checked source is on `main`, and cross-device runs are queued below.

**Taken 2026-10-17 00:48 CDT** by a Claude Code subagent working #11823 in ~/work/openagents-11823.

**Released 2026-10-17 01:10 CDT** by Codex (GPT-6) after #11947 landed in `5e187096d5`; all eight attribution tests passed. #11823 stays with its active Claude Code owner, and #11898 stays with its live owner.

**Released 2026-10-17 01:00 CDT** by Codex (GPT-6) after #11945 landed in `92662ccae4`. Both reported unit failures are fixed; `coder-new` unit checks passed 1,055/0/24. The broader run found the separate attribution failures tracked in #11947.

**Done 2026-10-17 04:03 CDT** by Codex in `~/work/openagents-chat-bridge` (removed after completion; both lane 1 targets retained). #11894 is closed; #11898 is closed through `cc9f2bc14c`. #11823 stays with its active Claude Code owner.

**Released 2026-10-16 22:16 CDT** by Codex after landing #11894 prerequisites `d944461951` and `b2ac4a07c6`. #11894, #11898, and #11823 remain open and free; see their handoff comments. The `~/work/openagents-push-lane` worktree is removed; targets `~/work/openagents-target-1` and `~/work/openagents-target-1-app` stay warm.

- **Owns:** `crates/coder-delegate`, `crates/coder` (turn and delegate door), `crates/chat-home`, the Coder-facing API of `apps/openagents/crates/harness`, the continuation routes in `crates/openagents-web`, and `crates/openagents-sdk` history and continuation methods.
- **Order:** #11894 first, because the bridge unblocks #11898, the keepers in lane 3, and terminal chats in the one list. Then #11898, then #11823.
- **Watch for:** `Wrap` is borrowed and isn't `Send`, `Sync`, or `'static`. Apply it before each process spawns, or add an explicit launch broker. Never bypass it. The harness must build in both Cargo workspaces (root on Rust 1.97.1, `apps/openagents` on 1.98.1).

### Lane 2: sessions on the web

**Done 2026-10-16 23:30 CDT** by a Claude Code subagent. #11897 is closed (`f00ff8f121`, `59251e460b`); its deploy and live check are in After-close checks. The `~/work/openagents-lane2` worktree is removed.

- **Owns:** a new session index in `crates/openagents-web` (history list, summary page) and the publishing hook in `apps/openagents/crates/engine` or `crates/coder-sync`'s registry read.
- **Order:** #11897 alone. It builds on `cca7f94ebe`'s `chat-home` types without changing them. Ask lane 1 before you edit `chat-home`.

### Lane 3: keepers and moving agents

**Done 2026-10-17 04:25 CDT** by Codex in `~/work/openagents-labs-finish`: #11958 is source-complete in `761ad950ed` and `c04314b54c`, with all 1,786 UI tests passing serially, and closed. #11933 and #11951 stay with their owners; #11871 is released and blocked on #11948.

**#11958 source complete:** fixtures enter Chat explicitly, popup geometry comparisons disable motion, and the settings key preserves both earlier spellings.

**Taken 2026-10-17 01:26 CDT** by a Claude Code subagent working #11951 in ~/work/openagents-11951.

**Taken 2026-10-17 01:20 CDT** by a Claude Code subagent working #11933 in ~/work/openagents-11933.

**Released 2026-10-17 01:40 CDT** by a Claude Code subagent. #11946 is closed on code (`20093e312c`): an installed engine service, including the background engine, restarts into a newer binary while runs under keepers continue. The live update check is in After-close checks. Terminal keepers and the other drivers' `attach` moved to #11951. The worktree `~/work/openagents-11946` and its target are removed.

**Released 2026-10-17 01:10 CDT** by a Claude Code subagent. #11932 is closed on code (`a76ba6ab21`): `crates/engine/src/sessions/move_cloud.rs` captures, uploads, restores, and resumes one chat on a cloud computer. Its dedicated-account run is in After-close checks. #11933 remains. Target `~/work/openagents-target-11932-app` stays warm.

**Released 2026-10-17 00:57 CDT** by a Claude Code subagent. #11930 is closed on code (`b49a54a201`): Claude Code and Codex run under keepers by default, with `OPENAGENTS_KEEPERS=0` to turn them off. The update handoff and terminal keepers moved to #11946. The live restart check is in After-close checks. The worktree `~/work/openagents-lane3c` is removed; target `~/work/openagents-target-lane3b-app` stays warm.

**Released 2026-10-17 00:19 CDT** by a Claude Code subagent. #11931 is closed on code (`c85eb539b7`): the background engine service, attach-or-start, the macOS menu-bar item, and Move to background. Its live turn checks are in After-close checks. #11932 and #11933 remain. Target `~/work/openagents-target-lane3b-app` stays warm.

**Released 2026-10-17 00:05 CDT** by a Claude Code subagent. #11930's keeper layer is on `main` (`16629755e6`); Claude Code and Codex attach remain, see the issue. #11931 wasn't started: it needs a test host, because a LaunchAgent can't be installed on the owner's Mac. Target `~/work/openagents-target-lane3-app` stays warm.

- **Owns:** a new keeper module in `apps/openagents/crates/harness` (additive; lane 1 owns the existing API), engine adoption in `apps/openagents/crates/engine`, the LaunchAgent and systemd service plumbing, and the move-plan store and UI.
- **Order:** #11930, then #11931. #11932 waits on #11852 (lane 8), #11813 (lane 6), and #11898 (lane 1). #11933 comes last. Spec: [move running agents](../desktop/move-running-agents.md).
- **Risk:** #11932 and #11933 sit at the end of three other lanes' chains. If those slip, ship #11930 and #11931 and close the other two with their remaining steps recorded. Don't hold the release for them.

### Lane 4: Labs platform

**Released 2026-10-17 04:29 CDT** by Codex after the #11871 closure audit (`8c8b105226`, `011dbb7200`). Still left: #11871, blocked on the existing owner of #11948. All original children are closed. `~/work/openagents-labs-finish` is removed after this update; targets `~/work/openagents-target-9` and `~/work/openagents-target-3-app` stay warm, as do both lane 1 targets.

**Released 2026-10-17 CDT** by a Claude Code subagent. Worker reads of referenced versions and files, the Game, Ship, Video and Robot registrations, and #11882 are on main (`2f7acc29fd`, `4e8463ddff`, `2c6be74847`); #11882 is closed. Production checks are in [After-close checks](#after-close-checks). The worktree `~/work/openagents-lane4b` is removed; target `~/work/openagents-target-agent2` stays warm.

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

**Released 2026-10-17 CDT** by a Claude Code subagent. #11948 is closed on code (`a8b54f4b6d`, `f5a43b4c06`): typed Lab rows in live desktop and phone sessions, the version list on saved and live cards, and Model3d orbit on both. The live account check is in After-close checks. #11871 can close. The `~/work/openagents-11948` worktree is removed; target `~/work/openagents-target-lane5b-app` stays warm.

**#11949 closed by Codex `codex:01a14140` in `2945c7608c`** in `space-infra-publish`: the hosted player and timeline use the public Labs API. Its checks passed; deployment and real render playback are in After-close checks. #11886 is also merged and closed; #11948 stays with the active Claude Code session.

**Taken 2026-10-17 01:20 CDT** by a Claude Code subagent working #11948 in ~/work/openagents-11948.

**Released 2026-10-17 01:10 CDT** by a Claude Code subagent. #11942 is closed on code (`e6ebb67e46`, `a3ef319562`). The hosted player and timeline on `/labs/video/{doc}` are #11949. The `~/work/openagents-11942` worktree is removed; target `~/work/openagents-target-agent2` stays warm.

**#11886 closed by Codex `codex:01a14140` in `052cedc1e2`** in `lnd-work`. The desktop timeline and account-bound editor service passed their checks, including the #11943 notes/event integration. #11949 is merged and closed; #11948 stays with its active Claude Code session. Deployment and real native/web render playback stay with lane 0 in After-close checks.

**Released 2026-10-17 01:05 CDT** by a Claude Code subagent. #11884 and #11943 are closed on code (`29d4e808fd`, `a4c68fd920`, `0e18a669df`); typed Lab rows in live sessions, the version list, and Model3d orbit are #11948, which waits on #11942. #11886 is code complete in the Codex update above. The `~/work/openagents-lane5c` worktree is removed; target `~/work/openagents-target-lane5b-app` stays warm.

**Released 2026-10-17 00:14 CDT** by a Claude Code subagent. #11880 and #11883 are closed on code; their remaining code is #11942 and #11943. #11884 and #11886 are open and free. The worktree `~/work/openagents-lane5b` is removed; targets `~/work/openagents-target-agent2` and `~/work/openagents-target-lane5b-app` stay warm.

**Released 2026-10-16 23:04 CDT** by a Claude Code subagent. #11881 is closed (`e37d66493e`, `ade5341924`). #11882 is open and free: the kind and migration landed in `d08cf1dc58`; the simulate handler, registration, and the live run remain. The worktree `~/work/openagents-lane5` is removed; target `~/work/openagents-target-agent2` stays warm.

- **Owns:** `crates/game-lab`, `crates/ship-lab`, `crates/robot-lab`, `crates/video-lab`, their web adapters, and the Lab rows in `apps/openagents` (GPUI `crates/ui/src/lab_card.rs`, mobile layout, iOS cells).
- **Order:** #11881 and #11882 as soon as #11878 lands. #11880 after #11877 and #11878. The app views #11883, #11884, and #11886 after #11876 and #11877. Each view renders `lab-ui`'s Rust Native card; don't write a card per shell.

### Lane 6: Computers and SPACE (the live Codex session)

**Source scope released by Codex `codex:01a14140`.** Its Computers follow-ups and #11886/#11949 are merged, checked, and closed; it holds no unfinished issue claim. Lane 0 owns the listed live checks, and lane 10 keeps every signet observer. The two isolated checkouts and the root/app target caches stay available.

- **Owns:** `crates/coder-working-computer`, `crates/oa-computers` and the SPACE driver, `deploy/space-service`, `deploy/space-credentials`, the Computers routes, and usage and quota code.
- **Status:** `codex:01a14134` completed #11813 and #11816 and is closing #11800 on source. Its canaries, credentials, workloads, and builder are removed. Lane 0 owns the remaining deployment/live rows and provider-managed empty-network cleanup below. #11823 stays in lane 1.
- **Other completions:** `codex:01a14140` closed #11939 lifecycle recovery, #11940 OpenCode saved agents, and #11944 recoverable-object inventory. It also completed #11886 and #11949 under lane 5; lane 0 owns their live checks.

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

**Done 2026-10-17 04:13 CDT** by Codex in `~/work/openagents-labs-finish`: #11952 is closed in `2a796565b7`. The #11871 claim is released pending #11948; target `~/work/openagents-target-9` stays warm.

**#11952 source complete:** both provider fixtures run in separate test processes through the normal library test target. No production code changed.

**Released 2026-10-17 00:13 CDT** by Codex (Grok 4.7). #11934, #11929, #11925, and #11727 are closed on code. After-close checks remain for the #11934 deploy and Foreman live check, the #11925 controlled deploy, and the #11727 named-sale test-mode check. The `~/work/openagents-lane9b` worktree is removed.

The website agents door uses the shared `place` (`1708f44f49`). Deploy and the Foreman live check stay in After-close checks. #11929 is closed on code (`7fb459a5a5`). #11925 is closed on code (`534ecb5c67`). The controlled deploy check stays in After-close checks. #11727 is closed on code (`ed4c8d05c4`). The named-sale test-mode check stays in After-close checks.

**Released 2026-10-16 22:40 CDT** by a Claude Code subagent. #11935 is closed (`264642e0a6`). #11934's `openagents agents` command is on `main` in the same commit. #11928 is closed; the no-traffic production check is in After-close checks.

- **Owns:** `crates/openagents-cli`, `crates/openagents-login`, `crates/openagents-sdk/src/agents*`, `crates/openagents-web/src/{agents_door,attribution,pages/contact,pages/affiliates}.rs`, the web main and gateway startup paths for #11925 and #11928, and `apps/openagents/crates/engine/src/doc_host/agent_lab.rs` for #11929.
- **Order:** #11935, then #11934 (one patch covers both), then #11929, #11925, #11928, and #11727.
- **Known baseline failures:** these `openagents-web` tests already fail on `main` and aren't yours to fix: copy_guard (OpenAPI wording), route_owners, screens, api_docs guides, environments::computers (6), cad_web::world_physics (1), and work_runs::computers (1).

### Lane 10: signet and LayerTree (mostly waiting)

**Done 2026-10-17 11:45 CDT** by the Claude Code coordinator (`fd130ab5`). #11862, #11770, #11863, and #11762 closed at 07:55–08:10 CDT, with results in `docs/payments/` (`fdcd7f12b7`). The LayerTree soak qualified at 08:34:53 CDT, and the scheduled deletion removed `oa-layertree-signet-tdx` at 08:42 CDT (audited, no HALT). The escape paid all five members exactly 1,000 sats at 326775 (11:23 CDT), with a 934-sat CPFP. The followers saw the payout with no alarm. Results, fixture, archive, and capture are in `docs/research/layertree/2026-10-16-signet-verification.md`. #11793, #11794, #11799, and #11779 close in that order.

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

**#11952 Computers provider-fixture isolation.** Lane 9, closed in `2a796565b7`. All three `computers_api::tests` cases passed together with default parallel scheduling and one test thread. The two provider fixtures each own a child test process, so the process-wide production configuration cannot cross fixtures. No deploy or live check is needed for this test-only change.

**#11928 Deployments: fence Environment work before a candidate receives traffic.** Lane 9, not started.
- **Status:** a held candidate opens `Studio` and requeues unfinished setups before it gets traffic. Its guards are process-local, so it can fail a shared setup or sweep homes.
- **Remains:** a two-process fixture proving a zero-traffic candidate makes no setup change, model call, computer cancel, or home deletion, plus a cross-process execution lease. Keep the candidate hold and the eight WAIT checks.

### Labs platform (#11871 epic)

**#11871 Epic: Labs platform.** Lane 4.
- **Status:** all 15 original children (#11872–#11886) are closed, with their source checks recorded on the child issues. #11948 (typed Lab rows in live sessions, version lists, and Model3d orbit) is closed on code, so no source work remains. #11871 is released; deploy and device checks are in After-close checks.
- **Decided (owner, 2026-10-16):** renders that show MetaHuman or Fab content are never committed to GitHub. They live in the space's private platform storage and stream to users. The plan's other decisions stand as recommended: new hosted Labs start with owner and test accounts, and AVPlayer comes first on Apple platforms. The 1.0 bridge decision is moot.

**#11875 Labs L4: CAD Lab as the first kind on the Labs platform.** Lane 4, closed.
- **Status:** `2f3465b291`, `218a67f92f`, and `5bdf74bba6` are on `main`. CAD 57, CAD web 22, PostgreSQL/fake-GCS store/API, account authority, cookie/project revocation, formatting, schemas, and route inventory passed.
- **After-close:** lane 0 completed the staging and production CAD workflows, browser checks, and cleanup on source `79f8c18f71`; the evidence is linked below. Production history-copy approval remains in `NEEDS_OWNER.md` (1 document, 2 versions, 11 files; checksum `16445f2f…93ae`).

**#11876 Labs L5: `openagents::labs` in openagents-sdk.** Lane 4, closed.
- **Status:** the SDK is on `main` in `1b55b50ae1`; the standalone Git-pinned example, public CAD patches, README pin, and qualification evidence landed in `d5270e882e`, `87ec5faf82`, and `69e4e79f1c`. The outside build, SDK/CAD/Video tests, macOS Engine security regression, and iOS account check passed.
- **After-close:** lane 0 completed the production example, ranged artifacts, notes/events, retry, and cleanup with the qualified binary; see the evidence below.

**#11877 Labs L6: Lab cards in saved history, and the web Labs door on the registry.** Lane 4.
- **Status:** integrated history cards, typed SDK cards, `lab-ui`, generic document/media pages, and registry dispatch. Hosted kinds replace their loopback adapters; app requests carry the current sender's credentials. Source checks pass: API/UI 11, SDK history 5, app account 53, and 17 focused web tests. The OpenAPI export is updated and the saved patch file is removed.
- **After-close:** the production history-card and second-browser check is listed in [After-close checks](#after-close-checks).

**#11878 Labs L7: jobs and placement.** Lane 4, closed.
- **Status:** scoped jobs, durable worker authority, worker SDK, outbound worker, cloud launcher, server/CAD hooks, and source-policy checks landed in `ccd60bee7e`; `7736ea1b36` fixes the policy fixture and `0b6204c6a3` records evidence. Core checks passed 72, SDK/CAD/Video 140 (1 ignored), Gateway 461 (8 ignored), focused web 3, and the native Engine policy regression. PostgreSQL and fake GCS were included.
- **After-close:** lane 0 owns deployment, server/enrolled/cloud Echo, private-input proof, scoped-key retirement, and observed cloud deletion. The owner Mac worker installation is in `NEEDS_OWNER.md`; evidence is in `docs/sdk/labs-worker-evidence.json`.

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

**#11882 Labs R1: Robot Lab documents and runs on the platform.** Lane 4, closed. The simulate handler, registration, and replay card landed in `2f7acc29fd` and `2c6be74847`; the live run is in [After-close checks](#after-close-checks).
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

**#11886 Labs D2: video timeline editor in the desktop app.** Lane 5, code complete.
- **Done:** native video, audio, and text tracks with linked drag previews, frame keys, speed, caption length and text, and a dissolve. The shared Video Lab rules and Labs SDK own edits, version conflicts, preview jobs, and account-bound media.
- **Checks:** account 75 (including 17 editor/model/wire tests), GPUI Lab cards 2, and edited-file formatting passed. Dedicated-account native/web render playback and cleanup stay in After-close checks.

### Chats, sessions, and moving agents (#11896 epic)

**#11896 Epic: OpenAgents chats are normal, resumable chats.** Lane 1, closed.
- **Status:** all six children are closed. Saved transcripts (`cf2ff29e53`), cloud replies (`362d0f4f01`), the shared terminal harness and Loro publisher (`f3267187d5`, `0b84201f39`), the web session index (`f00ff8f121`, `59251e460b`), and linked home changes (`cc9f2bc14c`) are on `main` with their own checks recorded. The UI follow-up #11958 passed all 1,786 tests serially.
- **After-close:** lane 0 runs the deployed round trip and device checks below.

**#11894 Continue a terminal or phone chat on this computer, as a real session.** Lane 1, code-complete.
- **Status:** `f3267187d5` routes terminal sessions through the shared native harness and removes duplicate drivers. `8453e68318` updates the Microcoder consumer, `0a745fa94c` makes the shared room client available without SQLite, `f12035c392` adds SDK chat and room clients, and `0b84201f39` publishes screened terminal text to the same conversation ID in Loro. `574ec67ab8` stabilizes the steering deadline fixture. The earlier continuation foundation remains in `cca7f94ebe`.
- **Checks:** bridge 259, publisher 44, Microcoder 141, SDK chat and continuation fixtures 3, and app sync 56 without the store and 73 with it. The full Microcoder run found an unrelated assertion expecting “signed in” after `95a0145d3a` changed the copy to “logged in”; that one assertion is excluded from the passing focused run. Twelve optional tests are ignored. Changed Rust files are formatted, and `git diff --check` passes.
- **After close:** lane 0 deploys the new binaries and runs the dedicated-account terminal-to-desktop context and phone steering check listed below. Custom account deployments must set `OPENAGENTS_SYNC_URL` to their matching sync service.

**#11897 Show computer sessions as rows on openagents.com.** Lane 2, not started.
- **Remains:** a summary-only session index per account, rows mixed into the web list with an "On <computer>" badge and offline state, a summary page with app and continue actions, a per-chat and per-project transcript opt-in (off by default), `source: "session"` rows in `/v1/history`, and access and revocation checks.

**#11898 "Where it runs" is a property of the chat.** Lane 1, code-complete.
- **Status:** shared controls, typed links, SDK transfers, and admitted host methods are on `main` in `ac0c9386e1`, `d653d6979a`, `9ddc0a0ec9`, `b6d657fad4`, and `16c0fc6875`. `deb7310289` adds server receipts and acknowledged web moves; `1328a4b617` persists host moves and bounded context; `2e6a3a5e98` binds native moves to the account; `cc9f2bc14c` mounts desktop and phone controls and regenerates Swift bindings. Earlier parts are read-only, and pending destinations stay out of lists.
- **Checks:** web continuation 4, client 27, document 149, engine 533 (2 ignored), mobile 69 (1 ignored), protocol 83, RPC 18, sync 74, shared controls 4, SDK fixtures 3, and access 126 passed. Five focused UI checks passed, including preservation of an unsent draft on an old part; all 56 app/shared Swift sources typechecked. The broader UI suite subsequently passed all 1,786 tests serially under #11958 (`761ad950ed`, `c04314b54c`).
- **After-close:** lane 0 deploys the server first and runs the dedicated-account web, Mac, and cloud round trip and the paired-computer/device checks below.

**#11958 Desktop UI test failures.** Lane 3, closed in `761ad950ed` and `c04314b54c`. All 1,786 UI library tests passed with one test thread. Chat fixtures keep their assertions with an explicit starting route; copy and theme expectations match the current app, popup geometry is fixed during its scroll check, and the settings key reads both previous spellings.

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

**#11946 Move running agents: engine update handoff.** Lane 3. Closed on code.
- **Status:** an installed engine service restarts into a newer binary while runs under keepers continue. It refuses new work for the moment of the handoff and waits for runs without a keeper. The background engine exits with code 75 so its service starts the new binary. An engine refuses a keeper whose protocol it doesn't speak and leaves the run alone.
- **Remains:** the live update check in After-close checks. Terminal keepers, the other native drivers' `attach`, and the `openagents daemon` unit's `KillMode` are #11951.

**#11931 Move running agents: background engine by default.** Lane 3. Closed on code.
- **Status:** `c85eb539b7`. `crates/background` writes each data folder's LaunchAgent or systemd user unit and decides attach, start, or embed; the app runs it before embedding. The macOS menu-bar item has Open, Move to background, and Stop all and quit; Move all to a cloud computer is commented out until #11933. On a disposable Debian 12 VM, `openagents background ensure` installed and started the unit, and a second run attached without reinstalling.
- **Remains:** the live turn checks in After-close checks.

**#11932 Move running agents: capture and restore on a cloud computer.** Lane 3.
- **Depends on:** #11852, #11813, #11898.
- **Remains:** commit the tree to `refs/openagents/move/<chat>` or a bundle, upload the harness session file, restore at the same path, and use credential references only.

**#11933 Move running agents: move plans, Move all, Bring back, and agent tools.** Lane 3, last. Closed on code.
- **Status:** the durable move plan and Bring back (`sessions/move_cloud/plan.rs`), the engine's `PlanMove` family and its hooks into the engine's chats (`rpc/moves.rs`), the agent tools `plan_move`, `start_move`, `move_status`, and `bring_back` with no approve tool, and the Move sheet, Move all in the menu-bar item, and Move in the chat menu. #11932's Codex arguments were checked against codex-cli 0.163.0: they parse, and `CODEX_API_KEY` is read, but `codex exec` defaults to a read-only sandbox, so a resumed Codex now gets `--dangerously-bypass-approvals-and-sandbox`. As built, with its differences from the spec: [move running agents](../desktop/move-running-agents.md#as-built).
- **Remains:** the dedicated-account run in After-close checks.

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

- #12574: public `da4497735e` keeps owned face sources exportable when the
  graphics session is unavailable or reconstructed. Thirty host library tests
  and the Wasm host build pass, including account/world mismatch refusals. The
  actual GPU-loss run found the export guard failure before it reached Reload;
  it did not establish a reload or permission defect. Actual export/reconstruction,
  accepted shader versus unfinished draft, and 3 cm checkpoint checks continue
  in #12561 with fresh authorized sessions.
  [Recovery evidence](../verse/aethyrforge-recovery-qualification.json).

- #12573: public `c20ac6e2ff` supplies the scoped Kiln JSON command shapes without
  widening its parser, tools, or files. Seven focused tests and the browser Wasm
  build pass. The real Flint run produced the legal 0.98 world-effect Draft with
  matching observed hosted context after two answers; the six-minute run did not
  finish its agent turn and is not a pipeline-completion pass. After cancellation,
  that retained candidate passed explicit Draft/Compile confirmation through the
  actual native grants service and pinned hosted renderer: four HTTP200 responses,
  revision0→1, matching active source, preserved pose, full-size GPU capture, no
  browser errors, and test-session retirement200. Identity/storage are isolated;
  the static Kiln helper is not the in-product bootstrap or provider-login proof.
  New runtime tags include the full SDK closure, including changed Kiln bytes;
  prior tags and traffic are preserved. Agent-turn completion was not observed
  within the qualification bound; the next request may still have been inside its
  declared wait. No hang or provider failure is established. Current-profile
  integration, storage cleanup, and the remaining journey continue in #12561.
  [Kiln command evidence](../verse/aethyrforge-kiln-command-qualification.json).


- #12572: the local public-API backend was not the production sidecar contract.
  No project permissions or storage-home logic changed. One focused regression
  covers legacy records, current viewer/editor/owner roles, original storage home,
  and independence from GitHub connection state. The current hosted native image
  reads and creates projects through the actual gateway in a separate staging-NFS
  test prefix; raw account records stay private. The production-style candidate
  keeps `GATEWAY_HOLD=serving`, all serving traffic, and prior tags. Actual GCS
  save/open and viewer/editor/owner checks now pass through the native GCS
  adapter, including ownership transfer with the original storage home. The final
  project and both account deletions returned HTTP 200. Qualification-service and
  temporary-prefix cleanup remain in #12561. Earlier timed-out attempts are not
  counted as passes; their sessions are revoked and traces redacted. No production
  accounts were mutated.
  [Account-contract evidence](../verse/aethyrforge-account-contract-qualification.json).


- #12571: the official website upload filter now includes only the public Fantasy
  Props catalog under `docs/verse`. The installed Cloud SDK's actual file chooser
  confirms exactly one added file, no removals, and no private inputs. The original
  native image build failed on the two catalog `include_bytes!` calls; the fresh
  local consumer builds. Both subsequent official image builds pass; the current
  native candidate is `coder-q61-c20ac6e2`, with the gateway hold, serving traffic,
  and prior tags preserved. Integrated checks continue in #12561.
  [Build-context check](../verse/aethyrforge-build-context-qualification.json).

- #12570: private `0a28e7d` keeps the last/default positive render size through
  transient zero-size layout and preserves the existing size limit. Type checking
  and two viewport/player checks pass. Clean hosted house and personal-player runs
  force zero-size startup, verify actual rendered desktop/mobile dimensions,
  hide/show without changing session or revision, and reload the same pinned version.
  The original parent retained its dimensions; the nested preview intermittently
  attached at zero. No native visibility or authorization rule was relaxed.
  Synthetic accounts and Codex-reviewed private captures remain distinct from
  provider/human evidence. Broader real-account/hosted qualification remains #12561;
  the official website build-context failure is tracked separately in #12571.
  [Viewport qualification](../verse/aethyrforge-viewport-qualification.json).

- #12569: optional avatar views now require a loaded character in private `833ed1c`;
  the existing Medieval Town assets and controls remain intact. Type checking and
  15 interface tests pass. Hosted house and awarded personal-world studio checks
  pass on desktop and mobile with the current native consumer: no avatar toolbar
  when `modelLoaded=false`, and Edit/Listen/Stop remain reachable. Accounts are
  synthetic; captures are Codex-reviewed. Pinned-player checks found the separate
  zero-viewport failure tracked in #12570; its subsequent hosted house/personal
  player checks pass with no avatar toolbar and reachable native/chat-toggle controls.
  Full chat-panel qualification remains #12561. No production traffic or prior tags changed.
  [Profile-control evidence](../verse/aethyrforge-profile-controls-qualification.json).

- #12560: version-pinned personal-world invitations and Loro chat are implemented
  on `main` (`8a4ed3168c`). Native access/retry/expiry/rate/isolation checks (1),
  room-token checks (5), existing browser-token routes (3), and a real isolated
  coder-sync/Postgres check (1) pass. A clean native/GPU browser run covers two
  synthetic identities, actual message exchange, independent visibility/mute/block,
  reconnect deduplication, queued exact Send and Leave, focus, host report targets,
  disposal, chat-SDK failure, logout, and revocation. Visits grant no editing and
  create no extra projects. Captures are Codex-reviewed, not human-approved.
  Hosted current adapters and real dedicated-account/provider checks remain in
  #12561; existing provider/microphone/human steps stay in `NEEDS_OWNER.md`.
  No production traffic changed. [Visit qualification](../verse/aethyrforge-visits-qualification.json).


- #12559: bounded seeded personal courtyards, two styles, advancing/paused day cycles,
  scoped editing, durable save/reopen, Undo/Redo controls, and pinned personal players
  are implemented. Host checks (64), native owner/tutorial/save/publication/revocation
  (1), house (3), private kernel (25), private type/source/history/player checks, and
  a clean real-GPU/native browser run pass. The browser verifies exact recipe/layout/
  pose reopening, elapsed Pause, submitted lighting at stable revision/pose, perimeter
  collision, mobile settings/microphone hit tests, cancel/retry, and guest player load.
  Captures are Codex-reviewed; account/recognition fixtures are synthetic. Hosted
  combined qualification, real provider/microphone, and Kiki/human review remain in
  #12561; owner-only steps stay in `NEEDS_OWNER.md`. Real Kiln remains agent-owned.
  No production traffic changed. [Personal qualification](../verse/aethyrforge-personal-qualification.json).


- #12558: one durable account-owned personal planet follows an owner-authored house
  publication, explicit Play/Share, and return. Creation retries preserve edits;
  deletion keeps a receipt and never recreates the project. Three account checks
  (file and Postgres serialization), tutorial/native form checks, gateway, Wasm/Kiln,
  and private type checks pass. Two clean real-GPU/native-store browser runs cover
  old-service 404 and transient failures, concurrent tabs/replay, delayed identity
  responses, account switches, congratulations/resume, rename/reopen, and mobile
  layout. Codex reviewed private captures. Account/provider fixtures are synthetic;
  live Postgres, provider sign-in, microphone, and human review are not claimed.
  Complete hosted and full-journey qualification in #12561; existing owner-only
  steps remain in `NEEDS_OWNER.md`. No production traffic changed.
  [Tutorial qualification](../verse/aethyrforge-tutorial-qualification.json).


- #12556: immutable saved playable versions, explicit publication, version review,
  rollback, revocation, and a guest player are implemented. Host checks (56), the
  native publication regression, Wasm/Kiln builds, private type/contract checks,
  and the full real-GPU/native-store browser check pass. The browser covers lost
  responses, duplicate confirmation, pinned guest walking, early mouse input,
  touch click suppression, held/failed iframe reloads, same-version recovery,
  fatal-status recovery, voice-handler Play/Share, dirty/closed-engine management,
  mobile resizing, and Share-card clearing on project/account changes. Codex
  reviewed private captures. Account and recognition fixtures are synthetic;
  no provider, microphone, hardware device-loss, or human-review pass is claimed.
  Qualify the hosted candidate and full journey in #12561, including the bounded
  real Kiln check using the documented test-account service-key path. Existing
  owner-only sign-in/microphone steps remain in `NEEDS_OWNER.md`.
  [Publication qualification](../verse/aethyrforge-publication-qualification.json).

- #12568: Keep draft acknowledges success only after its exact accepted source
  reaches the host snapshot; draft writes still leave world history unchanged.
  Two focused private tests, TypeScript checks, and the real-GPU delayed-status
  browser regression pass. The latter holds the older snapshot after the command
  receipt, verifies confirmation stays open, then immediately compiles once the
  matching source arrives. Evidence stays in private scratch
  `draft-ack-browser-3/browser-draft-ack.json`. The account service is synthetic;
  no real sign-in provider or microphone pass is claimed. Deployment and the
  integrated player journey remain in #12561.

- #12514: wire v6 adds scoped face previews/apply and complete editable material
  restore. Local Rust scar generation and handmade layers share confirmed changes,
  history, saved PNG resources, and device recovery. Host 53, native bounds/house,
  Wasm, private types/kernel, and focused private authoring checks pass. The real
  GPU/native-store browser flow passes generation/refinement, held-preview feedback,
  delayed imports/Save responses, Undo/Redo/reopen, and missing-image recovery;
  teardown exits cleanly. A second-werewolf renderer capture changes only the
  selected face. Codex inspected private desktop/mobile and per-instance captures.
  Sub-millisecond generation samples mean under 1 ms, not zero; adopted-frame
  receipt and PNG-encoding timings are separate. Complete immutable deployment,
  dedicated-account/provider/microphone and human qualification in #12561;
  owner-only checks remain in `NEEDS_OWNER.md`. No production traffic changed.
  [Material qualification](../verse/aethyrforge-material-qualification.json).

- #12567: main `51d7b36629` serializes project creation and resets recovery/open
  state and history across project changes; `bc8ad3af00` opens replacement frames
  at the pinned release base. Host 53, Wasm, and the native consumer build pass.
  The real GPU/native-store browser regression passes held Save/Create, held
  Create/Save/draft-edit, and delayed recovery across Create and picker changes.
  It uses a synthetic account service and direct fixture session, with scoped
  local-network permission; it is not provider sign-in or microphone evidence.
  Complete deployed/dedicated-account coverage in #12561 and material checks in
  #12514. No production traffic changed.
  [Project qualification](../verse/aethyrforge-project-qualification.json).

- #12566: private main `d6c2e8d` uses the current renderer UV input and rejects
  interface drift before replacing the working effect. Eight authoring checks,
  TypeScript checks, and real GPU compile/failure/repair/undo/redo checks pass;
  Codex inspected private working/failed-candidate captures. Pinned candidate
  `s6c9e53150296a3c0a` with public SDK/native `6af4d05101` passes the
  retained saved-world and readable-contribution browser checks. Codex inspected
  private desktop/mobile captures. Broader #12561 cases remain pending; no
  provider or human-review pass is claimed.
  [Project qualification](../verse/aethyrforge-project-qualification.json).

- #12555: saved-version contributors and readable, non-mutating inspection use
  existing project access. Row summaries name accepted changed fields instead
  of repeating the save note. Exact current signed credits are read only through
  bounded operator links; missing links show Not recorded, and test-ledger
  amounts/states stay labeled. Source drafts and local editor outcomes remain
  distinct. Focused source checks and integrated checks are recorded in
  [contribution qualification](../verse/aethyrforge-contribution-qualification.json).
  The retained desktop/mobile review, accepted row labels, late inspection,
  non-mutation, and exact download pass on `s6c9e53150296a3c0a`; Codex
  inspected private captures. Qualify dedicated-account receipt visibility,
  revocation, and save/return in #12561. No real payout or provider pass is claimed.

- #12554: wire v5 adds bounded editable exports and confirmed transactional
  restores; v1–v4 remain pinned. Existing account projects/private files own
  versioned saves, actor-aware undo, PNG resources, and per-person checkpoints.
  Focused native storage checks (3), house checks (3), public host checks (40),
  Wasm checks, private contract checks (25), and kernel checks (23) pass.
  Late Save text stays unsaved; canceled loads and cross-context receipts cannot
  adopt fetched versions or old tools. The retained check used a fresh native
  consumer and an immutable wire-v5 release. Review
  candidate `p4a25d82b1bf39f001` passed delayed-Save/base/canceled-load checks,
  then hit the renderer UV compile defect tracked in #12566. Its replacement
  `s6c9e53150296a3c0a` passes the retained Save/base/cancel/restore and late
  context-failure checks on fresh native/SDK `6af4d05101`. Still verify
  real dedicated-account save/close/sign-in/resume, file imports, conflicts,
  revocation, and storage/network recovery in #12561. Provider access is in
  `NEEDS_OWNER.md`. [Project qualification](../verse/aethyrforge-project-qualification.json).


- #12563: the preview owns bottom-left zoom/Reset controls. The host keeps Back
  in the header, moves guidance out of that area, and leaves only Full screen in
  its compact mobile footer. Focused route/copy checks and script syntax pass.
  The retained universe browser check now hit-tests and clicks the actual plus
  button on desktop/mobile, retaining keyboard/destination checks. Fresh native
  `6a86568016` passes the full check; Codex inspected the new private desktop/mobile
  captures. This source/guest check is complete; final journey checks remain in #12561. [Universe qualification](../verse/aethyrforge-universe-qualification.json).

- #12565: each handled request clears the previous response and shows current
  option/review/progress feedback. Listen clears the prior transcript and response.
  Host checks (31), Wasm checks, changed-file formatting, and script syntax pass.
  The retained browser check asserts Share -> Show options on desktop and mobile.
  Pinned release `tb86fb66653b0e5545` and fresh native `6a86568016` pass that assertion;
  Codex inspected the new private desktop/mobile captures. Synthetic recognition
  remains explicitly `real_speech_provider=false`. [Voice qualification](../verse/aethyrforge-voice-qualification.json)
  retains the earlier Codex-reviewed defect and the separate ASR gap.

- #12564: the house host allows microphone access only to itself; camera and
  cross-origin engine mic access remain denied, and the read-only Town policy
  stays unchanged. Permission, speech-service, and network refusals have distinct
  messages. Focused website checks (3), host library checks (6), Wasm checks,
  and changed-file formatting pass. The original actual-ASR attempt heard no words under the inherited
  `microphone=()` policy. Rebuild the fresh native consumer, rerun actual ASR with
  the documented synthetic audio, and keep a human microphone/real-provider pass
  separate in #12561. [Voice qualification](../verse/aethyrforge-voice-qualification.json).

- #12553: in-world opt-in voice, spoken options/review/confirmation, shared
  text/button handlers, scoped prop/material/light candidates, bounded stale-input
  cancellation, and hidden-until-registered downstream controls are code complete.
  Host library checks (5), Kiln checks (59), website checks (3), Wasm checks,
  formatting, and browser-script syntax pass. [Qualification record](../verse/aethyrforge-voice-qualification.json)
  keeps actual ASR, synthetic events/audio, real-provider sign-in, and human
  microphone evidence separate. Build/pin the isolated SDK/release and fresh
  native consumer, run the retained browser checks, and complete real spoken
  edit/inspect/Undo and downstream Play/Share/planet controls in #12561.
  The dedicated-account provider and human microphone step is in
  [NEEDS_OWNER.md](../../NEEDS_OWNER.md). No paid generation or performance
  result is inferred from source checks.

- #12552: Jarvis desktop windows/mobile tabs, focus and field checkpoints,
  compile-to-inspect, restricted Kiln cleanup, guest mesh inspection, and touch
  navigation are code complete. Public host checks (27), Kiln workspace checks
  (4), website checks (3), private mesh/protocol checks (13), type checks, and the
  real local GPU mesh/touch/recovery run pass. [Qualification record](../verse/aethyrforge-workspace-qualification.json)
  separates engine mechanics from provider checks. Isolated release
  `j9ba9687b2db62c0a5`, pinned v4 SDK/Kiln companion, and fresh native consumer
  `b23a59cecb` pass desktop/mobile Jarvis, focus, typing, pending-edit cancel,
  mesh, window/draft/selection restoration, and repeated open/close checks.
  Codex reviewed the private captures; no human review is claimed. Real provider sign-in remains in [NEEDS_OWNER.md](../../NEEDS_OWNER.md).
  Voice, material painting, and durable contributor history stay with their
  owning children. #12563 still blocks final qualification.


- #12551: unified project revisions/history, scoped Kiln workspace, confirmed
  source hashes, world-effect compilation/recovery, and bounded capture code are
  complete. Public host checks (24), new Kiln workspace checks (3), website checks
  (3), private kernel checks (22), private authoring/protocol/profile checks (20),
  Wasm type checks, dependency checks, private build, and real local GPU recovery,
  overlap, history, pose, and repeated-capture checks pass.
  [Qualification record](../verse/aethyrforge-authoring-qualification.json)
  distinguishes trusted-service GPU mechanics from real-provider/authorization
  evidence and labels receipt/status timing accurately. The v3 SDK/Kiln
  companion build passes. Check an isolated pinned release carrying the authoring
  changes with a fresh native consumer; complete real Kiln/provider/grant integration in #12561. The dedicated
  account's provider access remains in [NEEDS_OWNER.md](../../NEEDS_OWNER.md).
  Creator controls stay hidden until the loaded release supplies authoring state.
  #12563 still requires desktop/mobile recaptures before final qualification.


- #12550: universe code, isolated release, 20 public host tests, four focused
  website tests, private build, fresh native consumer, and real guest browser
  checks pass. [Qualification record](../verse/aethyrforge-universe-qualification.json)
  records input lock, drift/reduced motion, keyboard/camera control, mobile width,
  house handoff/map return, unsupported-WebGPU links, and 82 credential-free
  private requests. Codex capture review found toolbar overlap: fix and recapture
  desktop/mobile in #12563 before #12561 completes. No human review is claimed.
  Real-provider completion remains with #12561 and `NEEDS_OWNER.md`. No root promotion.

- #12549: code complete: native scoped-grant checks (3), public host checks (18),
  private adapter checks (13), Wasm SDK, and the fresh website consumer pass.
  Wire version 2 isolates extended status/actions from frozen version 1 releases;
  the prior production demo still passes its browser check. The pending-edit browser
  fixture and real-provider completion remain unfinished. [Qualification record](../verse/aethyrforge-access-qualification.json)
  distinguishes partial observations, the isolated popup reproducer, and the
  corrected login-form selector. Complete the integrated flow in #12561; the
  dedicated-account provider-access step is in [NEEDS_OWNER.md](../../NEEDS_OWNER.md).

- #12547: completed the real walkable house, closed ceiling seams, visible-mesh
  picking, wall/doorway collision, shader/startup recovery, and the freshly built
  Rust website against an isolated hosted release. [Qualification evidence](../verse/haunted-house-qualification.json)
  records both source revisions, immutable artifacts, private capture hashes, and
  browser results. Production activation and full journey qualification remain
  in #12550 and #12561; the current homepage and Medieval Town release stay unchanged.

- #12548: completed asset-preparation evidence: 13 public Rust tests, 11 synthetic
  importer tests, matching independent real rebuilds, unchanged source hashes,
  and Chrome 155 / Apple Metal captures pass. Custom-art project terms remain
  separate from Quaternius CC0 rights. [Qualification record](../verse/fantasy-props-qualification.json).
  The integrated house and final art review belong to #12547 and #12561.

- #12546: immutable private engine releases and the public Rust Medieval Town host
  pass signed-out candidate checks, including movement, fullscreen, checkpoint
  reload, cleanup, return navigation, unavailable releases, and rollout/rollback.
  Production revision `coder-web-b3afdd3d3b-20261023043410` serves 100% of traffic;
  the same live browser checks, homepage, and `/health` pass.
  [Deployment record](../verse/aethyrforge-medieval-deployment.json).

- #12545: public SDK and private artifact pass local protocol and browser checks;
  [receipt](../verse/aethyrforge-host-conformance.json). #12546 owns immutable private
  deployment, production origins/headers, and a deployed conformance run.

- #12513: rebuild the Kiln WASI tool bundle during the next normal deployment
  and verify that its patch tool refuses overlapping ASCII and multibyte
  searches. Native crate regressions cover both cases and unique replacements.

- #12536: done on 2026-10-22; production revision
  `coder-web-f9fbed854c-20261022221217` serves `78c82b3af1`. Both real Maya
  heads opened through ordinary scene clicks in an isolated signed-out browser;
  GPUI sign-in states, `/pylons` formatting, zero work-facts CSP errors, and
  candidate-matching JS/WASM hashes passed. The staff visual check remains in
  `NEEDS_OWNER.md`.

- #12532: done on 2026-10-22; `oa-foreman-1` runs verified build `5ccc9402f7`,
  and the live feed showed it walk to the Pylon Bay and arrive at its stop;
  stationary-player detour regressions pass.

- #12530: done on 2026-10-22; production revision
  `coder-web-a7ea5c218d-20261022214909` serves the checked Pylon distance change.
  Its Grid WASM matches the build; the 3-meter boundary and focus regressions pass.

- #12529: done on 2026-10-22; production revision
  `coder-web-a7ea5c218d-20261022214909` serves the gateway and chat label fixes.
  Chrome shows monochrome gates and the correct Foreman and Suggestion Box labels;
  production HTTP checks pass (74), and both chat WASM builds match Cloud Build.

- #12519, #12521, and #12522: install the bounded Kiln workspace binary on `coderos-4080` and run
  real browser-editor work through both explicitly pinned Maya Pylons, with
  external TypeScript and browser checks before draft PRs.

- #12515: done on 2026-10-22; after the old task stopped, the native host on
  `coderos-4080` was upgraded to `86c771017a` and its running path was verified.

- #12512: activate the deadline-aware native Kiln host and confirm that the real
  Foreman task reaches its checked PR as part of #12497 below.

- #12497: deploy the native WORK publisher, persistent Kiln host, and dedicated
  runner; enable the native dispatch route and follow a real objective through
  its checked Nostr patch to a draft PR. Targeted native and dispatch tests pass.
- #12491: done on 2026-10-22; production revision
  `coder-web-b0b54b3bf5-20261022173912` accepts the saved stop and continuation
  commands, preserves assignment history, and records #12493's result.
  Grid Foreman resumed and autonomously started Improve Pylon on
  `coderos-4080` with the saved Codex worker revision 2.
- #12490: done on 2026-10-22; the deployed credential fix reads the complete
  eligible-issue list and enabled #12489's objective dispatch.
- #12489: done on 2026-10-22; Grid host `0fa694b293` dispatched Improve Psionic
  to `coderos-4080`. Codex delivered [draft PR #12493](https://github.com/OpenAgentsInc/openagents/pull/12493)
  with a failing baseline regression and 21 passing CPU backend tests.
  Importer and command checks passed (31 Grid, 12 server).
- #12460: deploy the [suggestion inbox](../../crates/suggestion-inbox/README.md)
  under a dedicated service account, with separate inbox, owner-review, and
  delegated-reader keys. Set `NOSTR_RELAY_SUGGESTION_INBOX` on the relay and
  `OPENAGENTS_SUGGESTION_INBOX` on the website, then deploy the relay, website,
  and both browser chat renderers. With throwaway keys, check first-offline
  delivery and retry, each client's source label, and owner-only quarantine.
  Check the desktop Help menu and phone Settings links after the next app builds.
- #12468, #12470: with a `consumer-q` group and a qualified group live on
  the relay, run `pylon ask --model deepseek-v4.1-flash` with and without
  `--profile consumer-q`, and check each request lands only on the profile
  it names and that the lossy answer is labeled. When the layer-major
  prefill adds Maya's layer 20–39 skip (#12470's bound: keep the last
  2,414 tokens), compare its logits with the full prefill on a 16K prompt.
- #12463, #12466: on a spot `g4-standard-48` with a profile from a few
  chat answers, run `psionic-v41-stage chain --split 0-40 --gpus 0
  --experts file --expert-cache auto --engram file --expert-profile FILE`
  on 32K-context conversations (needs `--max-context 32768` and
  `--prefill layer-major`), and on a consumer NVMe machine for Engram
  latency; record decode tok/s, hit rate, and Engram seconds in
  `docs/psionic/v41-single-box.md`. On a machine with every expert
  page-locked (`--experts host`, 360 GB of RAM), record the measured bus
  and host lane times per expert.
- #12483: after the next web and web chat deploy, signed in as a fresh
  test account with no computers: check that the Grid's top bar says Ask
  OpenAgents, that the chat opens with no Computers, Environments, Agent
  Lab, or Check-ins, shows Open Game Lab and Walk around the Grid on a new
  chat, and that Back to the Grid in the header returns to the Grid in
  one click. Check that Show developer tools brings the sections back.
- #12461, #12484: after the next web, Everglade, and pylons-layer deploy
  (the Grid pack with the reaction clips landed in `5c6d153f2fa1`): watch
  the Foreman fold its arms at the Bay, billboard, and help desk, lean on
  the console while it answers, wave when you walk up, talk while your chat
  with it is open, and shake its head while waiting for someone in its way;
  ask it a question as the test account and check that, once a Pylon
  answers, a line runs from the Foreman to that Pylon in the Bay for the
  result's 20 seconds.
- #12480: after the next web and lander-chat deploy, signed out in Chrome
  on staging and then production: walk under the GAME LAB arch, open it,
  and check that the sign-in prompt sits centered over the Grid; press
  Sign in and check that GitHub opens in a popup, the Grid tab stays
  where it was, and after signing in as the dedicated test account the
  popup closes and Game Lab opens signed in at the same spot. With popups
  blocked, check the full-page sign-in comes back to `/?lab=game`. Check
  that `/login` and `/signup` show Back to the Grid and that Escape
  follows it.
- #12448: after the next web deploy (the host is at `280907fdfb` on
  `oa-foreman-1`; commands stay off until the site sets
  `OPENAGENTS_FOREMAN_COMMANDS=on`): on staging, with
  `OPENAGENTS_FOREMAN_OWNER` set to the dedicated test account, a worker
  agent saved there with no standing authority, and a test computer of that
  account, ask for an agent on an issue in a test repository through
  `POST /v1/foreman/commands/requests` as an admin test account; check that
  within a few minutes `GET /v1/foreman/commands` logs a `started` entry,
  the test account's app shows a new chat with the fixed brief, and that
  chat opens a draft pull request; check the pause and a refused unsigned
  POST. Then delete the chat and close the draft. Production turns
  commands on only when the owner sets the variables (`NEEDS_OWNER.md`).
- #12478: after the next web deploy and an Ice Cream Man build from
  `main` (`start.js` changed), in Chrome signed in as the dedicated test
  account: visit `/games/the-ice-cream-man` directly so the old worker is
  installed, then open Game Lab and check that the card and Play both show
  the running game, and that `chrome://serviceworker-internals` shows the
  worker updated to `icecream-v2`. Block the network mid-load
  (DevTools offline) and check the dark "The game didn't load." note with
  Try again and Back to the Grid, never a browser error or white page.
- #12447: after the next web and Everglade deploy (the host is already
  at `c8dfc45372` on `oa-foreman-1`), as the dedicated test account in two
  browsers: ask the Foreman a question and check that it walks to the
  terminal, that both browsers show `FOREMAN AT WORK:` and
  `ANSWERING A QUESTION` on the terminal's screen within about a second of
  each other with a line from its hand, then `ANSWERED IN …`, and that the
  screen goes back to the Pylon and deploy rows after it leaves; check
  that a player can still open the terminal's chat beside it. Delete the
  test chats.
- #12459: after the next web, Everglade, Pylons layer, pylon card, and
  lander-chat deploy, on staging and then production, at desktop and phone
  width: check that no HTML shows over the world (no Game Lab button, no
  HTML pylon labels or walking hint); walk under the GAME LAB arch, press E
  (tap on a phone), and check the GPUI panel's sign-in line signed out and
  the game with Play signed in as the dedicated test account; click a
  pylon and check its GPUI card; check that the transcript has no card
  behind it and New chat sits in the composer's row; as an admin, check
  that Kiln opens in its GPUI frame.
- #12446: after the next web, Everglade, and lander-chat deploy, as the
  dedicated test account on staging and then production: walk up to the
  Foreman, check that "Press E to talk to the Foreman" shows and opens the
  chat with "Ask the Foreman"; ask "what's being worked on?" and check
  that the answer is under 600 characters, speaks as "I", and names real
  open issues; with the Pylon router off on staging
  (`OPENAGENTS_WEB_PYLON_ROUTER=off`), check that the hosted chat answers;
  check that the eleventh signed-out message gets the sign-in line; check
  that `/admin/analytics` shows "Reached the Foreman" and "Asked the
  Foreman". Then publish one record with `openagents foreman publish --file
  -` from a build that has it, as an admin, and check that `GET
  /v1/foreman/status` shows it; the desktop Foreman starts publishing once
  its computer's `openagents` has the command. Delete the test chats.
- #12445: after the next web, Everglade, and relay deploys (the relay
  exempts the Foreman's key from the world cap by default): open `/` in two
  browsers on two machines as signed-out visitors and check that both show
  the Foreman at the same stop within a second, walking its round with its
  status line; stop the service on `oa-foreman-1`
  (`sudo systemctl stop grid-foreman`) and check that both show it parked at
  the terminal with `BACK SOON` within about 10 seconds, then start it and
  check that it walks again; stand in its path and check that it waits, says
  `EXCUSE ME`, and goes around; after 24 hours, check
  `journalctl -u grid-foreman` for restarts the watchdog didn't log. The host
  is already live on `oa-foreman-1` at `6500eed27a`.
- #12445 (relay, deferred by the coordinator to a quiet window): deploy
  the relay from `6500eed27a` or later so its world cap exempts the
  Foreman's key; the exact steps are in `docs/deployment/grid-foreman.md`,
  "Deploying the relay for the exemption". Only the coordinator deploys it.
- #12444: after the next web and Everglade deploy, on staging and then
  production as a signed-out visitor: capture `/` with the Foreman robot
  (about 5.6 m) beside the terminal, `FOREMAN` over it and the status line
  (`N OPEN ISSUES, M IN PROGRESS`); check that walking into it stops at
  its body and the terminal still opens; check that `GET /v1/foreman/status`
  answers counts, a line, and a time, with no token. Optionally set
  `OPENAGENTS_FOREMAN_GITHUB_TOKEN` (read-only issues) on the web service.
- #12427: measure `scripts/deploy/web.sh stage` on staging for a
  content-only commit (a guide edit; expect the content path and about a
  minute), a one-module commit (a GPUI chat source edit; expect the modules
  path), and a server-only commit, now that module keys ignore unrelated
  lock changes. Record the times in docs/deployment/openagents-web.md. Also
  try `WEB_MODULES_FAST=1` once kache holds the staging-profile crates.
- #12443: after the next web deploy, run `openagents browser run --
  python3 scripts/dev/check-grid-home-desk.py --url URL --out DIR` against
  staging, then production (the script refuses a remote address; drop its
  localhost check for that run or run it through a local proxy), as a
  signed-out visitor, and confirm the help desk shows from the spawn, E
  opens the chat, and the answer comes from the hosted agent.
- #12435: production content deploys, once the owner approves them:
  create the live key (`~/work/.secrets/livehost-openagents-live.key`),
  set `OPENAGENTS_LIVE_RELAY`, `OPENAGENTS_LIVE_STORE`,
  `OPENAGENTS_LIVE_PUBKEY` (the live key's) and `OPENAGENTS_LIVE_CHANNEL=live`
  on production's web container, then `scripts/deploy/livehost.sh promote
  HASH` with staging's checked manifest and confirm openagents.com serves
  it. Until then production serves the compiled-in content.
- #12436: after the next web deploy, with a Flint Pylon online, run
  `openagents browser run -- python3 scripts/dev/check-grid-flint.py --out
  DIR --click-pick --pin sha256:ARTIFACT` with the Mac Pylon's INT4 Metal
  artifact, and confirm the turn sends `flint@sha256:...` and the Mac
  Pylon answers it.
- #12449: restart each lasting `kiln-runner` with `--allow FILE` (and
  `--admins`), list each admin's Kiln **Admin key** `npub`, then from
  `/admin/kiln/` run `cargo test -p memory-stream` and confirm it runs.
  From a browser whose admin key isn't listed, confirm the page shows "this
  runner only takes jobs from keys on its list".
- #12450: on the lasting `kiln-runner`, move the crate list into
  `--crates-file`, load `crates/atif,crates/xp-ledger` in `/admin/kiln/`,
  change both, and confirm the patch's check runs `cargo test -p atif -p
  xp-ledger` and passes. Add a crate to the file without a restart and
  confirm the next job accepts it.
- #12451: after the next web deploy, in `/admin/kiln/` paste a `kiln-git
  own` identity for the **Admin key** (made with the test account's own
  profile key, not the owner's) and confirm the page says "Patches show as
  an agent authorized by npub1...". Publish a patch and confirm
  `review.html` shows "agent, authorized by npub1..." and counts the
  runner's check. After the owner announces the repository (NEEDS_OWNER),
  confirm `kiln-git show ADDRESS` lists the maintainers and runner.
- #12452: once the owner announces the repository, have a maintainer run
  `kiln-git apply` on a real Kiln patch, land the branch through the land
  queue, and publish `kiln-git status EVENT applied --commit SHA`; confirm
  `review.html` shows it merged.
- #12453: once the bridge has its token (NEEDS_OWNER), open a checked
  Kiln patch with `kiln-git status EVENT open` and confirm a draft pull
  request appears with the event link and author credit, a Nostr comment
  shows up on it, and a pull-request comment comes back on Nostr. Close it
  unmerged and confirm the patch shows closed.
- #12454: after the owner sets `OPENAGENTS_KILN_RUNNER` and
  `OPENAGENTS_KILN_REPO` (NEEDS_OWNER) and the next web deploy, run
  `cargo test -p memory-stream` from `/admin/kiln/` and confirm `oa-kiln`
  answers (`scripts/deploy/kiln-vm.sh logs`). A day later, confirm
  `kiln-fetch.timer` has run and the runner still passes a fresh base.
- #12455: with the bridge running, push a small crate change to a test
  fork, publish `kiln-git pr`, open it as a maintainer, and confirm the
  draft pull request appears; publish `kiln-git pr-update` with a new
  commit and confirm the branch moves. Close it unmerged.
- #12441: in a staging window the coordinator gives, run the Kiln
  runner with `--publish` against staging's livehost (a dedicated Kiln
  channel), click **Publish Kiln** from `/admin/kiln/` with an admin key,
  and confirm the page reloads into the new build with the workspace and
  conversation kept; then **Revert to last good**, and confirm a non-admin
  publish and a failing build are refused. Also restart the Mac Flint pylon
  on `openagents` `b316b703de`, and relaunch the DeepSeek V4.1 group with
  head `gs://openagents-psionic-models/bin/pylon/e411ee8ce2/pylon`
  (`--pylon-build 504b952b2eb4…`, worker unchanged); confirm each result
  carries `reasoning`.
- #12456: after the next web deploy with `OPENAGENTS_KILN_RUNNER` set,
  sign in as an admin test account, walk up to the terminal on `/`, press
  **Kiln**, load `crates/memory-stream`, and run the default task. Confirm
  the panel shows the cards and the run with no iframe, the terminal's
  screen says "KILN AT WORK:", and the run ends with "Proposed as a patch."
  and an Open review link that shows the patch.
- #12457: after `oa-kiln`'s runner is redeployed from `7fd1e05ef9` or
  later, confirm its beacon is on the relay (`openagents relay req
  '{"kinds":[30200],"#t":["oa:kiln-runner-beacon:v1"]}'`), then clear the
  runner under Kiln's Settings, run `cargo test -p memory-stream`, and
  confirm the log says "Found N test runners by their beacons", the run
  passes, and a `3201` receipt names the runner.
- #12488: give the web deploy `OPENAGENTS_KILN_SITE_KEY` pointing at a
  copy of `~/work/.secrets/kiln-site.key` (site key `cbabc7ec…d89e0`, already
  listed `each=10` in `oa-kiln`'s `/etc/kiln/allow`). After the deploy, sign
  in as the test account (not an admin), walk up to the terminal on `/`,
  and confirm `/home/me` says `"kiln": true`, **Kiln** shows, the runner's
  log says "agent of a listed key, counted alone", and a run on
  `crates/memory-stream` ends with "Proposed as a patch.".
- #12440: after the next web deploy, confirm the image log builds or
  reuses the `kiln` module and `/srv/kiln/index.html` exists. Signed out,
  `/admin` redirects to `/login?return_to=%2Fadmin`; signed in as the test
  account, `/admin` and `/admin/kiln/` answer the staff-only page and
  `/admin/kiln/config.json` answers `404`. As an admin, `/admin/kiln/`
  reports "cross-origin isolated: yes", the Grid terminal shows **Kiln**,
  and one Kiln task finishes.
- #12438: on staging and then production, walk through the Lagrange 2 arch
  on `/`. Confirm three things: the load screen shows with its bar moving,
  the zone panel and `A` turning work, and there is no Pylon overlay. Then
  press **The Grid** and confirm the overlay comes back.
  `scripts/dev/check-grid-home-lagrange.py` under `openagents browser run`
  does this.
- #12417: on the next `web.sh stage`, confirm the web image builds in about
  15 minutes or less, its log prints `oa-module-cache NAME: reused` for
  modules the commit didn't touch, and the staging smoke passes. Then
  confirm `/att/release.json` and `/vault/release.json` report the deployed
  commit, now read at run time.
- #12422: on a quiet Mac (load under about 10), start `rec` for about 15
  minutes and run `scripts/psionic/flint-rec-compare.py
  ../flint/llama.cpp/src/llama-model.cpp` against `psionic-flint serve
  --backend metal` on 127.0.0.1:18431. It alternates the two engines at
  three prompt sizes. Then `rec stop`, and replace the "conditions
  differed" rows in `docs/psionic/flint-metal.md` with the
  same-conditions medians.
- #12422: when the CUDA host is up, compute BF16 reference logits
  (`scripts/psionic/flint-reference.py`, FP32 compute) for
  `crates/psionic/crates/psionic-flint/testdata/int4-reference-request.json`,
  then run `flint-compare.py` on the Metal logits from position 18 on, with
  `--total-variation-max 0.10 --centered-logit-max 1.5` (declared before
  measuring), and record the result in `docs/psionic/flint-metal.md`.
- #12426: on a quiet Mac, start `rec` briefly, run the three scratch tasks
  in `docs/coder/measurements/2026-10-21-microcoder-flint-pylon.md` with
  `--provider openai-compatible --model flint`, add the rows, then
  `rec stop`. Only `implement` completed on Flint so far.
- #12423: deploy the Pylon Bay (website, `everglade-web`, and `pylons-web`
  from the same commit) and check `/` on staging with
  `scripts/dev/check-grid-home-bay.py` and `scripts/dev/check-grid-home.py`.
- #12414: the optional long-prompt profile remains **unqualified**. With the
  frozen G1 v2 policy from #12405 and available RTX PRO 6000 capacity, rebuild
  and pin both bundles, run `qualify --cases p3-2k,p4-20k --max-context 32768
  --max-step-tokens 2048 --prefill-chunk 2048 --observe --worker-build HASH`,
  retain complete forced-token/replay and router/exact-check evidence, and
  qualify a prospectively frozen long-context holdout. Measure device peaks
  for each placement; the retained memory envelope uses provisional W3 data.
  Repeat cancellation, fencing, and limit refusal on the actual group, then
  W14 with `--max-input-tokens 20480 --max-output-tokens 4096
  --deadline-secs 1800 --step-secs 75` and full prompts through
  `run.py --v41-group --v41-budget 1800 --v41-cases-json FILE`. Record the
  input-token counts, TTFT, total time, stop/length behavior, and beacon's
  context/chunk/text limits. #12414's stockout attempts created no machine.
  The short launch remains independent of these checks.

- #12408: on an RTX PRO 6000 group (`four-single` or `two-dual`), run
  `group_stage_worker` with `PSIONIC_V41_STAGE_REQUIRED=1`,
  `PSIONIC_V41_STAGE_BACKEND=cuda`, and the build's
  `PSIONIC_V41_STAGE_SHA256`; send a 2,872-token prompt (accepted) and a
  2,873-token one (refused `limit_exceeded`, no stage replaced); and check
  that the head beacon carries `request_limits`. W14's prompt is recounted
  (1,526 tokens, #12389). A spot attempt was reclaimed mid-request
  (`deepseek-v41-w3-w2-2026-10-21.json`, `limit_check`); exact-length
  chat-mode prompts are easy to rebuild with the checkpoint tokenizer.
  **W16 (2026-10-21): done, except `group_stage_worker`.** On pinned
  one-quad, two-dual, and four-single groups, an exact 2,872-token prompt
  was answered in about 108 s with `--prefill-secs 150`. Under the 75 s
  step limit alone it failed at step 0 (fixed in 3d44d55ace). A 2,873-token
  prompt was refused before any stage ran, and no stage was replaced. Each
  head beacon carries `request_limits` (`max_input_tokens` 2872). Receipt:
  `docs/psionic/verification/deepseek-v41-w16-groups-2026-10-21.json`.
- #12390 (W3): once G1 v2 qualifies the runtime (#12405), or a fix changes
  allocation, dispatch, or the prefill schedule, rerun
  `scripts/bench/deepseek-v41/w3w2_vm.sh answers|memory|long` with a pinned
  bundle (#12410), regenerate `placement_memory_measured.json` with
  `placement_memory.py`, and record the worker's `build.bundle_sha256`.
- #12399 and #12401: once the G1 v2 runtime is qualified (#12405) and
  #12408's context budget and bounded queue land, rerun both placements
  (`pylon cloud up four-single|two-dual --head stage0 --transport direct
  --deadline-secs 600 --max-output-tokens 4096 --step-secs 75`, spot in
  us-west3-a, with schema v2 builds from `scripts/pylon/v41-stage-build.sh`
  pinned by `--worker-build` and `--pylon-build`) with that worker: G6's numerical checks, G7's kill, freeze,
  cancel, and machine-loss checks, a request past #12408's budget, and the
  W14 harness (`run.py --v41-group --v41-budget 600`) to requalify the
  profile. Receipts for the first runs:
  `docs/psionic/verification/deepseek-v41-g6-g7-cross-machine-2026-10-21.json`
  and `docs/psionic/verification/deepseek-v41-w14-answer-profile-2026-10-21.json`.
  **W16 (2026-10-21): done on the unqualified runtime with the pinned
  worker `185eb68c…`.** In both placements, a killed stage and a frozen
  stage failed the request visibly with no replay and recovered under a
  new generation. An 8 s buyer cancel freed the slot. W14's nine prompts
  passed (9/9 in each placement). A 2,873-token request was refused.
  Remaining: machine loss on the pinned build, and G6's numerical checks,
  which need G1 v2. Receipt: `docs/psionic/verification/deepseek-v41-w16-groups-2026-10-21.json`.

- #12407: rebuild the stage worker for `sm_120` (`scripts/pylon/v41-stage-build.sh
  build`) and pylon (`... pylon`) so `BUILD.json` is schema v2, then launch the
  next group with `--worker-build` and `--pylon-build` set to the printed
  hashes. On the first G4 boot, check `/mnt/stage/ready` names the boot,
  manifest, and build, that each stage's hello reports `profile`
  `cuda-device-sm_120` and the worker's SHA-256, and that the `sm_120`
  build's CUDA tests pass on the RTX PRO 6000 (`v41-stage-build.sh test
  --host` a G4 builder, or the stage VM). Rerun the synthetic CUDA split
  comparisons there with the corrected generator.
  **W16 (2026-10-21): launch checks done.** Each ready mark named the boot,
  manifest, and build. Each group logged "runs build bundle 185eb68c…
  (sm_120) with profile cuda-device-sm_120, as pinned". The pinned
  commit's `sm_120` test pack (`v41-stage-build.sh tests`) passed on an
  RTX PRO 6000: 82 passed, 0 failed, including the split comparisons
  (`deepseek-v41-w16-sm120-tests-2026-10-21.log`). Nothing remains.

- #12410 and #12412: rebuild the `sm_120` worker and pylon from a commit
  that includes them, then run the pinned bundle on the RTX PRO 6000
  group. Pass the printed `BUILD.json` hashes to `pylon cloud up
  --worker-build ... --pylon-build ...` and `qualify --worker-build ...`.
  Check that each stage's hello reports `build.bundle_sha256` equal to the
  pin and that `status` shows `build    worker bundle ...`. Check that the
  G1 v2 report records `stage_bundle_sha256` equal to
  `expected_bundle_sha256` with `launch` `pinned`. Also measure the last
  stage's per-token time at 4,096 output tokens against W14's
  8.8-to-10.9 ms slowdown.
  **W16 (2026-10-21):** pinned groups ran worker `185eb68c…` and pylon
  `f1882776…`, and `status` shows `build    worker bundle 185eb68c…`. At
  4,096 output tokens, the last stage averaged 9.36–9.37 ms a token, down
  from W14's 10.9 ms. The G1 v2 report check stays with #12405.

- #12398: after #12407 binds the build and arithmetic identity and #12405
  lands the direct GPU comparison harness under the frozen G1 v2 policy,
  rerun the one-machine group (`pylon cloud up one-quad --head stage0
  --transport direct --temperature 0`, sweeping zones for spot
  `g4-standard-192`) and finish G5 under that contract: direct-runtime
  numerical checks, G2 v2 complete answers (chat-mode p2 needs #12408),
  DS0's six runtime checks, and G7's faults on the real group. The lifecycle
  checks don't wait for numerical qualification. Receipt for the first run:
  `docs/psionic/verification/deepseek-v41-g5-one-quad-2026-10-21.json`.
  **W16 (2026-10-21):** pinned one-quad in asia-northeast1-b. DS0's runtime
  checks passed: answer, continuation, cache reset, truncation, and an 8 s
  buyer cut standing in for stream truncation. The cap was reached 99
  tokens into the final answer, not inside reasoning. Remaining: G1 v2's
  numerical checks and G2 v2. Receipt: `docs/psionic/verification/deepseek-v41-w16-groups-2026-10-21.json`.

- #12418: qualify each new V4.1 device class under the frozen G1 v2 policy
  (#12405) before any mixed group is advertised as qualified, and pin
  each stage of such a group to its qualified bundle in its own lane
  entry: `cuda-sm_90` (H100, bundle built from `b13ec37306`), `cuda-sm_80`
  (A100, bundle
  `d32559a5cec5abc798b24ae1fbd5d2e01090f79afe191ce974ea1a201fd6522e` from
  `8c0350f07c`), `cuda-sm_89` (RTX 4080, bundle
  `6331bafa6fb83efdeff62425154021d9ef3bf30808aa8eb3f35a885572dc8f8e`,
  including its `--experts host` placement). Groups don't use CPU stages
  (owner, 2026-10-21). The kernel parity suites already
  pass on sm_80, sm_89, sm_90 (`docs/psionic/verification/deepseek-v41-mixed-device-2026-10-21.json`);
  G1 v2 needs the oracle comparison per class. Then rerun a mixed group
  with every stage pinned and the W14 harness.

- #12420: qualify the Metal device class `metal-apple10` under the frozen
  G1 v2 policy (#12405) with the other classes of #12418 before a group with
  a Mac stage is advertised as qualified, and pin the Mac stage to a bundle
  from `scripts/pylon/v41-stage-build-metal.sh` in its lane entry. The Metal
  last stage already equals the CUDA stage bit for bit on captured real
  inputs, and `pylon cloud up quad-mac` answered through Pylon as a
  development group (`docs/psionic/verification/deepseek-v41-metal-2026-10-21/`).

- #12394: rerun G4 on layers 0–4 and 0–9 with
  `scripts/bench/deepseek-v41/stage_vm.sh` under the G1 v2 direct-runtime
  contract (#12405), not the CPU's B200 arithmetic order, adding an isolated
  per-layer check (the CPU fed the GPU's layer inputs and state) as a
  diagnostic.

- #12375: rebuild the website with its wallet builder included in the upload.
  The Cloud Build file chooser includes the executable script, and the build
  checks it before compilation. The production image and live checks continue
  under #12307.

- #12374: after the #12307 website deployment, repeat the funded browser wallet
  send, forget, and seed-only restore. Refresh must sync the SDK and show the
  received amount minus completed sends and their quoted fees. The isolated
  mainnet check passed with 100 sats received, two 5-sat sends, and two 2-sat
  fees: the restored balance was 86 sats.

- #12366: after `27e05e78cf` is deployed to staging, open `/home` signed out in a
  fresh browser profile, walk to the terminal, and send one message. The answer
  shows a live pylon's name and model over it, and the website log has one
  `pylon router:` line naming the pylon. With a Clef pylon online, that line reads
  `decided pN … by pylon:<slug>`; without one, `ranked (no decision model online)`.
  Then set `OPENAGENTS_WEB_PYLON_ROUTER=off` on staging (or stop every
  conversation pylon) and confirm the same message gets the hosted chat's answer.
  Delete the test chats afterward.

- #12307: deploy the self-hosted LNURL backend, gateway, website, and native
  wallet/profile integration following `docs/payments/lightning-address-operations.md`.
  With the dedicated test account and a separately funded test wallet, receive
  an explicit profile tip, verify one Pylon/plugin receipt, publish and hide it,
  and confirm that a payout deposit is not counted again as a tip. Check address
  disable/rename, wallet transfer, and optional NIP-57 delivery. Verify an opt-in
  browser restore, receive, quoted send, refresh, and forget/reload. Infrastructure
  activation and funded wallet steps are in `NEEDS_OWNER.md`; never use the owner's
  words or leave test accounts or wallet data on the owner's devices.

- #12339: On macOS, run the stock transcript and sidebar PNG captures; Linux has no headless PNG renderer in the pinned GPUI. The dark/light transcript and sidebar native drawing data, including glyph pixels and transforms, matched byte-for-byte before and after the extraction. After the web deploy, check the shared input and sidebar at `/gpui` and `/demo/gpui` with the dedicated test account.

- #12358/#12360/#12364/#12367: DS0 and the [serial Pylon llama.cpp GPU flow](../psionic/deepseek-support-plan.md#serial-pylon-gpu-qualification-12360) pass 27 checks; the [managed recovery flow](../psionic/deepseek-support-plan.md#automatic-serial-pylon-recovery-12364) passes 13 more, including automatic crash/frozen-worker replacement and subsequent answers. The explicit [DeepSeek `pylon up` profile](../psionic/deepseek-support-plan.md#deepseek-profile-through-pylon-up-12367) passes 20 lifecycle checks, including saved resume and `down`. Clef/network Pylon stayed unchanged. The [native CUDA greedy profile, #12368](../psionic/deepseek-support-plan.md#native-psionic-cuda-for-the-r1-distill-12368), passes six model, four real HTTP-disconnect/admission, and 20 managed recovery checks; sampled native decoding, longer reasoning within the deadline, broader native selection, and cloud deployment remain separate.

- #12377–#12380: after the independent CPU component implementations land,
  compare their hyper-connection coefficients/residuals, router ids/weights and
  expert activations, attention/compression/index selections, and Engram map,
  hashes, and gated output against #12371's pinned original-weight oracle. Record
  BF16/FP8/FP4 activation boundaries and tie behavior explicitly. The complete Engram token map now matches the pinned reference function
  (see the model plan); synthetic NumPy fixtures qualify float32 equations
  only. Full attention wiring, 40-layer
  decode, native CUDA, and a V4.1 serving model remain separate gates.

- #12381–#12384: compare named projections and grouped output, RoPE/YaRN at
  long positions, BF16/FP8/FP4 activation codes/scales and GEMM boundaries, and
  the integrated shared-cache attention path against #12371's original-weight
  oracle. Pin device conversion/tie rules and reduction tolerances. Local
  equation/state tests do not qualify a full CPU decoder or V4.1 CUDA serving.

- #12385–#12387: on the original pinned V4.1 checkpoint, compare the complete
  CPU transformer's selected block residuals, router/hash choices, full logits,
  greedy token IDs, and complete final answers with #12371's mode-matched
  oracle. Check prefill/chunk/replay and reset behavior at numerical boundaries,
  retain hashes and timings, and report any divergence. Tiny synthetic models
  qualify composition only; they do not qualify original-weight CPU or CUDA
  serving. The checkpoint runner's full-hash option must pass before treating
  its output as original-weight qualification evidence.

- #12373: on the #12371 oracle VM with the V4.1-Flash weights, from
  `crates/psionic`, run `cargo run --release -p psionic-models --example
  deepseek_v41_weights_check -- CHECKPOINT_DIR --verify-sha256 --oracle
  DEQUANT_PARITY.json`. It must exit 0: all 96,085 tensors open, no NaN in
  the dequantized rows, every shard hash matches the pin, and every parity case
  has 0 mismatches. Post the JSON on #12373. Without the oracle file, run it
  without `--oracle` first and add parity when #12371 writes the file.
  **Done (2026-10-20):** on `oa-dsv41-weights-1` (the original files from
  Hugging Face on a Cloud disk), built from `4383d409f4`: 96,085 tensors open,
  0 NaN, 0 failures, all 48 shard hashes match, and 19 of 19 parity cases have
  0 mismatches. Both JSON outputs are on #12373.

- #12335: After the web deploy, use a dedicated test-admin account to open analytics, attribution, and Drive access with `?ui=gpui`. Confirm reports render, Drive entries can be added and removed, and a non-admin receives 404. Capture the pages on a hardware WebGPU browser; software captures here were blank for baseline Traces as well. Never use the owner’s account.

- #12344: After the web deploy, use the dedicated test account and a disposable computer chat with a saved chart and 3D visual. Confirm the visual module downloads only when a visual is shown, attachment reads follow the computer grant, controls work, resizing and scrolling keep the canvases in their rows, and changing chats removes them. Delete the chat afterward.

- #12337: After the next web deploy, confirm 308 redirects from `/lander3`–`/lander9` and `/landing` to `/home`, `/lander8/chat/{id}` and `/gpui/chat/{id}` to `/chat/{id}`, `/gpui` and `/cloud` to `/`, `/cloud/app/settings*` to the corresponding Settings page, and `/demo` and its HTML demo chats to `/demo/gpui`. Confirm `/`, `/home`, `/chat/{id}`, `/lander10`, `/lander11`, `/verse`, `/verse/hands/*`, `/demo/gpui`, and the HTML fallback still load.

- #12320, #12321, #12323 (WorkOS sign-in): after the staging WorkOS keys exist (`NEEDS_OWNER.md`, "WorkOS sign-in"), run `scripts/deploy/web.sh stage`; it turns WorkOS on in `dual` mode. Then, with a throwaway email (never the owner's), sign up with a password on `https://staging.openagents.com/signup` reading the verification code through the API (`Workos::email_verification_code`), log out, log in with an emailed code read the same way (`POST /user_management/magic_auth` answers it), approve a Coder 1.0.0 device code on `/device`, sign in by key on the phone and confirm the account's WorkOS User appears with the synthetic email, and confirm `GET /v1/account` answers with the WorkOS access token as the bearer. Leave staging in `dual` for 48 hours and confirm no `workos: ... not synced` lines in the gateway logs. Delete the throwaway WorkOS Users and accounts afterward. Then promote with production left in `legacy`.
- #12322 (WorkOS backfill): on staging, set `OPENAGENTS_WORKOS_BACKFILL=dry-run` on the gateway container, read the report (`workos-backfill-dry-run.json` and the gateway log line), then `apply`, and confirm the reconcile counts show every person's account linked, the smoke account `acct_dc7a799879686fc5` and SPACE test account `acct_4570d993d9897bdd` included, and that `GET /v1/account` answers the same body before and after for the smoke account. The production backfill is the owner's step in `NEEDS_OWNER.md`.
- #12325: After the web deploy, as the dedicated test account on staging,
  open `/settings?ui=gpui`, add and remove a Claude credential, connect a
  subscription if allowlisted, download an export, then delete disposable
  scratch-project chats from Data. Check `/settings?ui=html` and a browser
  without WebGPU still show the HTML page. Remove any test data afterward.
- #12327: After deploying the GPUI Settings memory and schedules pages, use only the dedicated test account on staging with `?ui=gpui`: add, edit, and delete a memory; add and edit a scheduled prompt, pause/resume it, and delete it. Confirm a scratch desktop signed in as that account sees the same changes through memory/schedule sync. Check the HTML addresses without JavaScript and without WebGPU, and remove all test records.
- #12328: After the staging web deploy, use the dedicated test account with `?ui=gpui` to connect and remove Google Drive, open and close the vault panel, and approve a `coder login --pair` request from a scratch host. Confirm chat sync choice persists. Remove the test connection and sign out the scratch host afterward.
- #12326: After deploying the web build, use staging's test account at `/settings/api-keys?ui=gpui`, `/settings/sign-in-keys?ui=gpui`, and `/settings/agent-accounts?ui=gpui` in Chrome, Safari, and Firefox; create a scratch key with chosen permissions and a monthly limit, copy and revoke it, reload to confirm the copy control is gone, paste/remove a scratch provider key in an empty provider slot, and check the HTML fallback. The model catalog is public; verify revocation on `/v1/usage/{id}` (401), not `/v1/models` (200). Staging and production test-account API checks passed locally; staging recovered after an initial HTTP 502 during the continuation check on 2026-10-20. Local Chromium WebGL2 checks against staging’s test-account APIs passed for all three pages, sign-in-key creation, masked copy, reload, confirmed revocation, agent listing, and API-key permission and limit controls.

- #12224: after #12263's room-token key is set on the web service and coder-sync, and a web deploy at `6d6ff7fb04` or later, run a scratch desktop app (temporary `HOME` and data folder, never the owner's computer) signed in as the test account with a scratch project, start a chat there, and allow the browser (see #12264). In Chrome, signed in as the same account, open that chat with `?ui=gpui`: confirm its transcript loads (the page fetches `openagents_web_room_bg.wasm` once), grows live while the desktop answers, and that a reply typed in the browser reaches the desktop and its answer streams back. Reload after the room has compacted (or after the desktop posts a checkpoint) and confirm the transcript still loads (`/v1/sync/chat2/{id}/checkpoint` answers 200). Remove what the check made.
- #12223: With `?ui=gpui` on production, as the dedicated test account with a Coder computer online, open a Coder chat whose Coder is waiting on an approval and answer it from the GPUI page's side panel (Approve or Deny), and start a Claude Code run with two agents from the HTML chat, then Stop and Message one from the side panel. Archive the Coder task and delete the chats afterward.
- #12298: Deploy the web (it adds `GET`/`PUT`/`DELETE /v1/device/name`, which forward to the account service's `/v1/account/name`). Then, as the dedicated test account in the desktop app, open Your ID > Public profile, set a name and an `https://` picture, save, and confirm `/@NAME` shows them, `/.well-known/nostr.json?name=NAME` answers the key, and a third-party Nostr client resolves `NAME@openagents.com`. Release the test name afterward.
- #12301: After the same web deploy, as the dedicated test account, publish a saved agent from the desktop Agent Lab (**Publish NAME**), and confirm a Nostr client sees the agent's kind 0 with a valid `auth` owner tag naming the test account's key, and that `/agent/{npub}` and `/agent/{id}` draw it. Retire it afterward (`DELETE /v1/published-agents/{id}/owner`).
- #12219: After the next web deploy, confirm the hashed module URLs and immutable caching on `/gpui`, `/demo/gpui`, and `/agent/foreman`, then check first-frame timing and image/icon appearance in Chrome, Safari, and Firefox with the dedicated test account. Local brotli sizes are 1,446,032 B (chat), 1,408,006 B (demo), and 1,292,635 B (AgentGraph); browser decoder tests pass. Native hardware screenshots remain a deploy check because this Linux headless GPU capture is black.

- #12142: Deploy oa-computers (migration 0019), then rerun the `lost_command_reply` and `duplicate_delivery` cases from `docs/sdk/space-qualification/2026-10-18/` with an `Idempotency-Key` header on `POST /v1/computers/{id}/turns`. Expect one execution and the original turn ID on the retry, and `409` for a changed body. The Postgres store test needs `OA_COMPUTERS_TEST_DATABASE_URL`; it was not run in the sandbox.
- #12145: Rebuild the shim with `deploy/space-service/shim-build.sh` (it applies `shim-boot-mode.patch`), update the pinned shim SHA-256 in `shim-install.sh`, install it on the drained worker `oa-space-service-worker`, then run 20 `small` creates against a ready clean base snapshot. Confirm with the worker observer that none passes `--config-file`, and record the distribution under `docs/sdk/space-qualification/`.
- #12091: On an isolated Android emulator and a physical device, with the dedicated test account, open Settings, rename a device, change appearance and wallpaper, share the app and a project invitation through the Android chooser, restart the app and confirm the settings persist, then delete the test account (paid-credit confirmation and shared-team refusal included). Source and Rust fixtures passed; the Kotlin/Gradle build, UniFFI binding regeneration, and the wiring of `SettingsHost` into `MainActivity` (it lives with the unmerged Android shell) have not run.
- #12122: `sync.openagents.com` serves from the VM `coder-sync-1` since 2026-10-18 23:10 UTC. After 24 hours, confirm no 429 or 5xx bursts in the `coder-sync-vm` load balancer logs, then run one `vm-deploy` of a new staged image and confirm the registry, chat tails, and device relay each reconnect on the first try (`docs/deployment/coder-sync-vm.md`). Rollback is `scripts/coder-sync/deploy.py cutover run`.
- #12150: Deploy coder-sync with `scripts/coder-sync/deploy.py deploy --staging`, then `deploy.py vm-deploy`, then `deploy.py deploy` (see `docs/deployment/coder-sync-vm.md`). Then check that `/health` reports `cached_admissions` and that a 503 burst from the account service no longer drops signed-in devices.
- #12052: As the dedicated test account against a test repository, run `cargo run -p openagents-sdk --example foreman -- check-in --repo OWNER/TEST_REPO --rounds 2 --interval 5` and confirm the decision log shows `look`, and a `nudge`, `ask_owner` or `release_claim` for a seeded stalled run, question and orphan claim. Delete anything it created.
- #12141: On the private SPACE deployment with the test owners, confirm the driver's assumed session endpoints (`GET /v1/sessions/{id}` returning `state` and `exit_code`; `POST /v1/sessions/{id}/kill` with `{"signal":"KILL","process_group":true}`) match SPACE, then run 100 consecutive turns (all end with a known outcome) and stop a 40 s turn after 3 s (ends `Stopped.`, a `setsid nohup` child writes nothing afterward).
- #12050: On a cloud computer running the headless engine, with the dedicated test account, bind a test agent's chat that declares check-ins (`openagents agents bind`), start a check-in, restart the desktop app on a test Mac (scratch `HOME`, never the owner's), and confirm the check-in finishes on the cloud host and the phone and website still show and steer the chat. Delete the chat afterward.
- #12040: After deploying the web proxy, coder-sync migration 8, and the push gateway attention routes, use the dedicated test account and a test device to grant a saved agent notification permission, force the last stall rung once, and confirm exactly one attention banner. Tap it and verify the private title and body appear and its chat opens. No push to the owner's devices is part of automated testing.
- #12208, #12210: The integrator on `oa-dev-env-1` runs `d013fd5ada`. On the next code entry, confirm with `openagents land show ID` that its checks list `cargo check -q -p PKG` before `--tests`, that a stale generated file it touches lands as a "Regenerate generated files" commit, and that a change under `apps/openagents/crates/mobile/` names the Android binding under `regenerate_skipped`.
- #12206: When the 1.0 push ends, restore `oa-dev-env-idle-minutes=30` on `oa-dev-env-1`. Then queue an entry while a slot is busy and confirm `scripts/cloud/dev-env-agent.sh check` says busy until it lands; stop the VM, submit from a pool host, and confirm the `warning:` line, `integrator_down` in the run result, and the `ALERT:` line in `land status`.

- #12285: After the web deploy, use the dedicated test account at `/?ui=gpui` in Chrome, Safari, and Firefox. Open Agent Lab, edit an agent and its tests, save, run on a scratch Lab host, read the result, add a review note, open the trial's conversation through the paired browser, and choose Use agent. Remove the test experiment and agent afterward. This Linux session's browser launcher points to an unavailable `coder-browser` binary.

Deploys and live checks that no longer hold an issue open. Lane 0 works through them in order, after each production deploy. Add a row when you close an issue with a check still to run. When a check passes, mark it done with the evidence. When it fails, open a new issue and link it.

| Issue (closed or closing) | Check | Status |
| --- | --- | --- |
| #12524 | Deploy the GPUI Pylon work view, enable the read-only observer on 4080, and verify both real Maya work records plus signed-out/test-account access. Preserve the production gateway and Foreman settings. | Done: coder-web-f9fbed854c-20261022221217 serves 100% traffic from 78c82b3af1. Both real feeds advance and Maya 1's new-attempt handoff passes; ordinary card clicks, formatting, private signed-out reads, 108 staging checks, and 74 production checks pass. Gateway and native settings are preserved. The signed-in staff visual check is in NEEDS_OWNER.md. |
| #12531 | Complete the web image build with Kiln's embedded asset catalog and deploy it with #12524. | Done: Cloud Build 13e13e5e-2e00-45c8-ac1a-7d9d47e6312b built the repaired catalog into the live a7ea5c image. Staging smoke: 108 passed; production smoke: 74 passed, no failures. |
| #12534 | Complete the web image build after refreshing the nested WebGPU lockfiles, then deploy it with #12524. | Done in the same live a7ea5c image: all locked builds passed, with no package version changes. Staging and production smoke checks pass. |
| #12365 | The deploy chats (417ed388 and ee638911) rebuild the web image and run `scripts/smoke/gpui-ready.py` signed out before promotion. | Failed f8 image reproduced: root-owned 0600 fonts return 500; permission repair passes with the same server and Wasm bytes; image readability and browser gates added. |
| #12354 | On staging, start a WorkOS provider flow and return with `oauth_failed` or another provider error. Confirm the login page asks the person to try again, the provider description stays in server logs, and only `access_denied` or `canceled` shows cancellation. Preserve dual mode and leave startup backfill off. | Passed 2026-10-20 on `openagents-web-1-staging-f8d9b3afc2-204131`: four isolated browser callback cases, return path, cleared flow, no session, and server-only diagnostic logs. The web image was later rolled back for a separate GPUI readiness failure. |
| #12351 | Chat `417ed388` deploys the tombstone-aware WorkOS sync to staging, runs dry-run then apply backfill, and confirms tombstones are absent from missing-user counts and never regain WorkOS users or memberships. Keep production auth in legacy. | Passed 2026-10-20 with `f34e55f88f`: 240 users created, 241 people linked, no missing users, dangling links, or failures; six deleted test accounts have no WorkOS users or links, startup backfill is off, and production remains legacy. |
| #12346 | On a scratch host with the dedicated test account, switch a saved coordinator from Claude Code to Codex using its chat header, ask what issue and worker it was watching, then switch back through Agent Lab and ask again; confirm the answer uses the handoff and fresh provider sessions. Do not switch the owner's Foreman as a test. | Automated handoff, archive, restart, and revision-check coverage runs with the implementation; live provider acceptance follows the next app build. |
| #12342, #12343, #12347, #12348 | Keep Gemma/Clef running and verify a clean Nemotron 340B short reply beside Clef in a bounded quiet window. | Done 2026-10-21 UTC: Nemotron returned only `4` with a normal stop (42.3 s), and Clef passed 8/8. Four transformer blocks use GPU memory and output weights stay in RAM; context 256, output budget 16. Quiet held 154.6 s (294.2 s including wait), requested with no queued builds. Load, memory, raw replies, and receipts are in `scripts/bench/pylon-models/nemotron-rtx4080-2026-10-20.json`. Live engine PIDs, restart counts, and settings stayed unchanged; Gemma remains the default and `verify` passed. |
| #12340 | Install the final native Psionic binaries and repeat Gemma 4 E4B's nine text jobs on the RTX 4080, then restore the verified live Gemma/llama.cpp and Clef pylon. | Done 2026-10-20: binaries from `c3ad12d298` installed; native Gemma returned 9/9 answers but passed only 1/5 exact checks. Native measurements are in `scripts/bench/pylon-models/native-rtx4080-2026-10-20.json`; #12341 tracks native Gemma/GPT-OSS parity. The live Gemma/llama.cpp plus Clef service is restored, serving, and passes `verify`; all eight compatible GPU routes passed 72/72 jobs. |
| #12254 | Deploy web and gateway from `a47ace8d31` or later to staging. With a throwaway key (never the owner's), sign in with a NIP-07 extension, claim a name in Settings > Public profile, publish a name and picture, and check `/@{name}`, `/@{name}/picture`, and `/.well-known/nostr.json?name={name}`; resolve `{name}@<staging host>` from a third-party Nostr client. Release the name and sign the key out afterward. Then promote; the pages are additive. | Not run. |
| #12270 | After deploying the web chat and a scratch host from this change, use the dedicated test account to capture paired-host and chat2-room chats on desktop and `?ui=gpui` at desktop and narrow widths: checklist updates, both usage rings and account switch, prompt rail, running tools and tool images, file links opening Files, fork/source navigation, delivery retry, reconnect, and saved-agent mark/name. Remove the browser grant and verify the next read/action fails. | Local browser fixture exercises accounts, context, delivery retry, and forks; the full paired-host/desktop comparison and grant-revocation check await deployment. Local crate-test results and the package budget are recorded in the closing comment. |
| #12255 | Set the secret `OPENAGENTS_WEB_AGENT_KEY_SEED` (64 random hex characters, never logged) on the `web` container, staging first. With the test account, publish a saved agent, `PUT /v1/published-agents/{id}/owner` with a NIP-OA tag its key signed, confirm a Nostr client sees the agent's kind 0 with a valid owner tag on `relay.openagents.com`, open `/agent/{npub}` and `/agent/{id}`, then retire it with `DELETE` and confirm the owner's profile drops it. Without the secret the owner routes answer 503 and nothing else changes. | Not run. |
| #12257 | Set `OPENAGENTS_WEB_XP_REFEREES` to the NIP-XP referee keys on staging's `web` container, then open `/@{npub}` of a key with a counted award and check its XP, receipts, and reviews each open `/event/{id}`. Without the variable, profiles show no XP and nothing else changes. | Not run. |
| #12259 | After the next staging web deploy, open `/explore` and compare its plugins with `openagents registry search` against `wss://relay.openagents.com`, and its pylons with `/pylons`. | Not run. |
| #12256 | On staging: create the NIP-29 groups `openagents` and `agent-<first 16 of the published ID>` as the relay (as `scripts/verse-relay.sh` does locally), set `OPENAGENTS_WEB_AGENT_REPLIES=on` with the agent key seed, then with the test account follow and fork a demo agent, mention it in its room from a Nostr client and see a reply signed by its key with a valid NIP-OA tag, and send it a NIP-17 message from that client (after `PUT .../messages {"from":"anyone"}`) and read the reply there. | Not run. |
| #12260 | On staging with `OPENAGENTS_WEB_FRONT_DOOR=on` and the `/lander5` and `/lander8` builds in the image: measure `/` cold at 20 Mbit/s with the first-frame measurement (headless Chrome, cache off, 5 runs) and confirm first paint under 1 s, send a message signed out and get an answer, and check every `/lander3`–`/lander9` URL redirects. Turning it on in production waits on the owner's confirmation. | Not run. |
| #12261 | After the next staging image (which rebuilds everglade-web), open `/verse` in Chrome desktop and confirm it opens the shared Grid, and that opening `/` and a chat downloads no `everglade_web` file. | Not run. |
| #12309 | On staging (with `/lander9`'s landmarker in the image): open `/verse` signed in with the test account and confirm its avatar shows its profile name; press Hands in Chrome desktop and confirm pinch-and-move looks and pointing walks. | Not run. |
| #12304 | On staging with the test account and a throwaway key: share a chat, use Post it on Nostr from its share step, and confirm the note reaches `relay.openagents.com`; follow a key from its profile and confirm the follow list keeps earlier petnames. | Not run. |
| #12302 | On staging, open a published agent whose model has Terminal-Bench boards and confirm Benchmarks lists them with working report links. | Not run. |
| #12313 | On staging with the test account: open `/verse` signed in, then open Public profile in the phone or desktop app and press "Show my name there"; within a minute the browser's console says the avatar is owned by your key, and a second browser on `/verse` shows the test account's profile name over that avatar. Agents and pylons from `/explore` stand on the plaza. | Not run. |
| #12336 | On production as the dedicated test account: open `/?ui=gpui`, open Settings and confirm there's no Earn page, open `/settings` and confirm there's no Plan section, and open a computer chat's row menu and confirm there's no "Move to a cloud computer". | Routes passed 2026-10-20 on `coder-web-8a2c3c67df-20261020173058`: `/affiliates`, `/live`, `/stats`, `/cad-shop`, `/cad-purchases`, `/settings/plan/subscribe`, `/v1/wallet/browser` answer 404; `/pricing` redirects; the sitemap lists neither `/live` nor `/stats`; `/gpui` carries an empty `openagents-features`; `/verse` has no Hands button. Signed-in part not run. |
| #12303 | On staging with the agent key seed set: publish a skill through the test account, wait ten minutes or open `/explore` twice, and confirm a kind 30188 listing by the directory key on `relay.openagents.com` and the skill on `/explore` linking the test account's profile; withdraw it and confirm it leaves. | Not run. |
| #12294 | On a scratch host (temporary `HOME`, `openagents` installed, test account) with the desktop app paired to Chrome: open Settings, then Earn (`?settings=earn`), and choose that computer; confirm the status and earnings read, Turn on reaches Serving (or Setting up while the model downloads), Turn off returns to Off, and both pauses toggle. Turn it off before ending. | Not run. |
| #12292 | On a scratch host with the desktop app paired to the browser and the dictation model downloaded (Settings, Voice): open a chat on that computer in Chrome, then Safari; press the microphone, say a sentence, press it again, and confirm the text lands in the composer. In the browser's network panel, confirm the audio leaves only on the computer's device room connection, and that a computer without the model says so. | Not run. |
| #12329 | On staging with the test account and `?ui=gpui`: upload a trace with `openagents traces upload`, open `/settings/traces` (it opens the GPUI page), open the trace and one of its agents, share it, open the public link signed out, and delete it. | Not run. |
| #12330 | On staging with the test account, a scratch host, and `?ui=gpui`: start a background agent on the host, open `/settings/agents` (it opens the GPUI page), message the agent, stop it, and see a landing entry in the landing queue. | Not run. |
| #12331 | On staging with the test account, a second test account, and `?ui=gpui`: open `/projects` (it opens the GPUI page), create a project, make an editor link, accept it as the second account through `/projects/join/{code}`, change its role to viewer, remove it, then add and remove a repository. Delete the project afterward. | Not run. |
| #12334 | After the staging deploy, use the Pro test account and `?ui=gpui` to open `/cad`, open a design, pick a face, save a new version, open its configurator, create and revoke a free link, and verify the signed-out link stays HTML. | Source and isolated-account browser checks recorded on the issue; deployed staging and visual verification remain to run. |
| #12332 | On staging with the test account and `?ui=gpui`: open `/environments/{id}`, `/work/{id}`, and `/settings/mac-jobs/{id}` directly, land in the chat's work panel on that item, and Stop from each; the left panel's Environments entry opens the panel. | Not run. |
| #12333 | On staging with the test account and `?ui=gpui`: open an image, a video, and a playable Lab document at their `/labs/{kind}/{document}` addresses (they open the GPUI page), and play each in its player; on the video, Edit the video opens the site's editor. | Not run. |
| #12310, #12308 | On the next TestFlight build and an Android emulator, with the test account and test values only: Wallet > Browser (iOS) and More > Payments from your browser (Android) show the card; a browser x402 ask appears, Pay completes it (Face ID or the device check above 1,000 sats), and the browser reads the preimage. On Android, More > Public profile saves a name and picture that `/@NAME` shows. | Not run. |
| #12288, #12289, #12290 | Deploy web from the Settings commit or later to staging (the web chat build now has a third module, `openagents_web_settings`). With the test account on `/?ui=gpui&settings=general`: pick a computer and change Thread naming and the worktree folder (the computer must run an engine with this commit, which lets a browser call `GetTitleSettings`, `SetTitleSettings`, and `SetWorktreeSettings`), then set them back. On Appearance pick Light, a theme, an accent, a background image, and import a VS Code theme file; reload and see them kept, then remove the image and the theme. On Shortcuts rebind Toggle left sidebar and use it. On Your ID see the npub and keys; link a throwaway key, remove it there (never one of the account's own keys), and sign out. Then promote; it stays behind the GPUI chat's flag. | Not run. |
| #12085 | On isolated API 26/35 and a physical test device, check selection, copy/Process Text, TalkBack/IME, viewport restoration, links, image zoom/dismiss, and streaming/reduced-motion paint. | Android arm64/x86_64 core builds passed; generated bindings, all Kotlin source sets, and 116 JVM tests passed at `212f64b3e1`. Device checks remain unrun; owner checklist is in `AtlantisPleb/workspace` `NEEDS_OWNER.md` at `0d90af0d75`. |
| #12093 | With the dedicated test account and provider configuration, check token replacement, sign-out, foreground/background attention, notification taps, and process recreation without stale delivery. | Android arm64/x86_64 core builds passed; generated bindings, all Kotlin source sets, and 116 JVM tests passed at `212f64b3e1`. Device checks remain unrun; owner checklist is in `AtlantisPleb/workspace` `NEEDS_OWNER.md` at `0d90af0d75`. |
| #12094 | On API 26/35 and physical test devices, check microphone/focus permissions, route changes, Bluetooth denial/SCO, interruptions, reconnection, and live calls with the dedicated test account. | Android arm64/x86_64 core builds passed; generated bindings, all Kotlin source sets, and 116 JVM tests passed at `212f64b3e1`. Device checks remain unrun; owner checklist is in `AtlantisPleb/workspace` `NEEDS_OWNER.md` at `0d90af0d75`. |
| #12097 | On test devices, check call capsule/full-screen/settings, minimize/reopen, service/notification mute and end, background account changes, and process recreation. | Android arm64/x86_64 core builds passed; generated bindings, all Kotlin source sets, and 116 JVM tests passed at `212f64b3e1`. Device checks remain unrun; owner checklist is in `AtlantisPleb/workspace` `NEEDS_OWNER.md` at `0d90af0d75`. |
| #12100 | On API 26/35 and a physical test device, check transcript paint/selection, scrolling, pause/resume, recreation/anchors, retired callbacks, and mounted account routes using only the test account. | Android arm64/x86_64 core builds passed; generated bindings, all Kotlin source sets, and 116 JVM tests passed at `212f64b3e1`. Device checks remain unrun; owner checklist is in `AtlantisPleb/workspace` `NEEDS_OWNER.md` at `0d90af0d75`. |
| #12102 | On API 26/35 and physical devices with TalkBack, check Rust Native roles/actions, focus/traversal, transcript paging/selection, large text, reduced motion, and lifecycle cleanup. | Android arm64/x86_64 core builds passed; generated bindings, all Kotlin source sets, and 116 JVM tests passed at `212f64b3e1`. Device checks remain unrun; owner checklist is in `AtlantisPleb/workspace` `NEEDS_OWNER.md` at `0d90af0d75`. |
| #12136 | Build and publish the `archil-worker:v3` image (`crates/coder-cloud/archil/cloudbuild.yaml`), then run `openagents chat work --on archil --issues N --parallel 1 --land queue` on one small issue and confirm its sandbox forks, submits a land entry, and is deleted. | Source landed at `7c9b86121a`; `archil`, `coder-cloud`, and `openagents-cli` tests passed on oa-dev-env-1 (one load flake rerun green; one test already failing on main). |
| #12128, #12137 | After the next desktop app build from `main` at `268a8c0b44` or later, with the dedicated test account, confirm the app backs off on sync 429/503 and other computers' chats stay listed, and that a chat checkpoint over 24 MiB uploads in parts. | App back-off `6ae1d54c0b` and part uploads `bb8e0c506a` landed; land-check prefetch `c22a53665e`. `openagents-client`, `openagents-engine`, and `openagents-sync` tests passed apart from load flakes that pass when rerun alone and tests that already fail on main. |
| #12159 | With the dedicated test account's three protocol fixture devices online against the sync VM, keep each device's host and client relay sockets open for a full hour with zero reconnects (the issue's acceptance). | Fix landed at `22a4be88a6`; `openagents-rpc` tests passed. The patched client's live retry regression passed on VM revision `ac7ee6dae3`. The full-hour soak restarted at 00:44 UTC on 2026-10-19 after a sync deploy interrupted the first run at 31 minutes with no subsecond reconnects. |
| #12103 | After #12100 integrates the Android host, run the offline emulator journeys, Design Lab captures, and budget baseline in `docs/audits/2026-10-19-android-parity-evidence.md` on API 26 and 35, and fill in its run record. Physical device, FCM, real call, and store checks are listed there with owners. | Android arm64/x86_64 core builds passed; generated bindings, all Kotlin source sets, and 116 JVM tests passed at `212f64b3e1`. Device checks remain unrun; owner checklist is in `AtlantisPleb/workspace` `NEEDS_OWNER.md` at `0d90af0d75`. |
| #12081 | On an isolated Android API 26 device or emulator, repeat cold start and process recreation, then create a client through the generated Kotlin boundary without owner-account data. | Bootstrap code and unit tests are on main at `f8b7484808`; the source matches the checked A2 checkpoint `3cff389b73`. Android x86_64 mobile test-source checking and all three host bootstrap tests passed. B1 later verified cold start and direct API 35 instrumentation. API 26 and Activity recreation remain unrun. |
| #12084 | On isolated offline API 26 and API 35 emulators, run the three transcript instrumentation tests for Latin, CJK, emoji, combining marks, RTL/fallback metrics, Rust wrapping, and Canvas origins using the bundled Geist bytes. Check earlier-history insertion, stream resize, font/display scale changes, horizontal overflow, fast scrolling, and Activity recreation; record cache bounds, allocations, frame percentiles, and missed deadlines. Qualify the same behavior on a physical device without owner-account data; see NEEDS_OWNER.md. | On B1 integration base `fb59d69ea8798b94dc7ea69d23d637efd75be8e3`, the focused app check passed in 13 seconds (86 tasks); 15 JVM tests passed (nine transcript, four bootstrap, two push), with zero failures, errors, or skips in the XML reports. The isolated offline API 35 AndroidJUnitRunner passed 14/14 tests, including all three transcript glyph-advance, Rust-wrapping, and Canvas-origin fixtures. API 26, viewport runtime and performance/frame pacing, Activity recreation, and physical-device qualification remain unrun. |
| #12083 | On the isolated offline API 35 emulator, install the debug APK and verify cold start loads the Rust/JNI core, demo rendering works, and Activity recreation preserves the current screen state. Run the connected debug Android tests for the ZXing camera fixtures; also check camera permission denial, cancellation, retry, and callback cleanup. On an API 26 device or emulator, install and launch the app. Use no owner account or production service. | The app.sh check passed Android test-source compilation, JVM tests, debug/release assembly, release lint, and APK policy checks. On the isolated offline API 35 guest, `am start -W -n com.openagents.app.debug/com.openagents.app.MainActivity` returned `Status: ok`, `LaunchState: COLD`, and the expected activity; the window dump showed “OpenAgents” and “A private demo, ready to explore.” The direct AndroidJUnitRunner passed all 11 tests (2 SemanticViewAdapter and 9 scanner tests), exit 0. The permission-denial fixture now checks `NOT_SCANNING` before denial, then `STALE_SESSION` and no classifier call after denial. Debug and AndroidTest APK signing certificates match. `connectedDebugAndroidTest` could not resolve offline UTP artifact `com.android.tools.utp:android-test-plugin-host-additional-test-output:31.7.3`; direct instrumentation ran the same test APK successfully. Activity recreation, live camera permission flow, and API 26 launch remain unverified. Evidence is in `/home/christopherdavid/.openagents/scratch/codex-01a14f97-7f49-70c1-b0ea-785fa2a6655f/android-runtime-12083/evidence/`. |
| #12082 | On a dedicated Android device with synthetic account, wallet, and grant data, check that the Keystore-backed store fails closed when locked, invalidated, or unreadable; that legacy wallet migration preserves the exact seed through process recreation and a signed in-place upgrade; and that backup and device transfer exclude the encrypted store. Do not make a funded transfer. | Rust account tests (70) and mobile tests (127, one ignored) passed. B1's Android Kotlin, AndroidTest, JVM, lint, and both APK checks passed at `e32f8e736b5ebc79a8a171ced64a1f09776f528a`. Real Keystore failures, process recreation, signed upgrade, backup/restore, and device transfer were not exercised. Owner device steps remain in [NEEDS_OWNER.md](../../NEEDS_OWNER.md#android-parity-account-signing-and-device-handoff-12077). |
| #12133 | On the next iOS test run, run the demo computer-picker regressions. On a test iPhone, check visible computer/project choices, switching before Send, the actual chat destination, and persistence after relaunch; see `NEEDS_OWNER.md`. | `cargo test -p openagents-mobile --lib`: 129 passed, 1 ignored; edited-file rustfmt passed. iOS build and device check deferred. |
| #12044 | Deploy the web from `main` at `10bb8805e3` or later (grant routes and `Schedule.agent`; definitions with `autonomy.wakes`). Then, with the dedicated test account on a scratch host (never the owner's): save a fork of Foreman under another name, **Use agent**, and confirm Settings → General → Check-ins lists it and it checks in. Grant it standing authority from the card and confirm `GET /v1/agent-definitions/{id}/grants` lists it and a check-in can steer another test chat; revoke it and confirm the next steer is refused. `POST …/grants` with `["deploy"]` answers 400 `not_grantable`. On the owner's Mac after the next host upgrade, confirm the old `foreman/` folder became `foreman.migrated` and Foreman's decision log and working state show under its definition ID. Delete the test chats and agent afterward. | Code on main (`10bb8805e3`); engine 50/0 (per-agent engines, grants, declared wakes, migration of old state with its log, fork with the same grants checks in and steers, never-list refused, #12054 hook), mcp 6/0, proto 8/0, UI 3/0, voice runtime test, agent-contract, agent-lab, web 15/0 (grant routes end to end), SDK 5/0. Deploy and scratch-host check to do. |
| #12116 | After the owner signs Codex in on `gce-agent-host-1` (NEEDS_OWNER.md): in chat `afb01cec` (on that device, Codex), ask it to run `. scripts/dev/cargo-build-env.sh && cargo test -p coder-cloud --lib budget` in `~/openagents` and confirm `test result: ok`; then confirm a Foreman-dispatched issue chat lands there. | Device registered and reachable (`list_devices`, `list_harnesses`, `create_chat` accepted); engine survives reboot; on-host `cargo test -p coder-cloud --lib budget` 9/9 in 91 s. Codex login (owner) and the chat run to do. |
| #12117 | After the next host upgrade, fulfill the owner's request with `pin_chat { chat: "c4211116", pinned: true }` for "Cloud fan-out on the automation SA". | MCP fixtures cover pin/unpin, unchanged pin order, and pinned/archived visibility; first use waits for the host update. |
| #12087 | Deploy the web (`/v1/history?query=`, `PATCH /v1/threads/{id}`, `/v1/history/pins`, indexed `seen` route). Then regenerate the Kotlin UniFFI bindings from the Android ELF, wire `apps/android/drawer` into the B1 shell (source roots plus live/saved/indexed navigation callbacks), run the `DrawerPresenterTest` JVM fixtures, and on a device/emulator with a test account check paging, search, swipe/long-press, pin/archive/rename/reorder, offline/reconnect, predictive back, and process recreation without duplicate sessions. | Open. Rust/SDK/account/mobile fixtures pass; Kotlin, bindings and device checks unrun. |
| #12046 | After deploying the web from `main` with the decision routes, use the dedicated test account on a scratch host (temporary `HOME`, `--state`, `--root`, `--tasks`): record a decision for a saved agent, read it through the SDK (`Agents::decisions`) from a second scratch host within a minute, then start a third host with an empty `decisions.jsonl` and see it fill from the account. Delete the test agent afterwards. | Route, SDK, and mirror tests pass against fakes; the live run is open. |
| #12086 | After B1 wires the Android callback: deploy the opt-in Play Integrity gateway config with explicit server credential, package/certificate/version pins; qualify a signed internal-testing device with a dedicated test account, replay/bad-release/outage cases, unchanged signup caps, and proof-of-work fallback. Keep rollout off until qualification. Owner provider/device steps are in NEEDS_OWNER.md. | Focused synthetic checks passed: auth 59, gateway gate/provider 13 and account route 5, OAuth 2, SDK 2, app account 9, mobile identity 1; rebased host build and Kotlin binding generation passed. No live provider or device check. |
| #12089 | Source checkpoint is on `main`; Rust projection tests pass, but Kotlin was never compiled. Regenerate UniFFI Kotlin bindings (`CoreSavedHistory.lab_projection`/`CoreLabCard`), compile the `apps/android/platform` sources and run `AndroidVisualFrameBoundsTest`, `LabSurfaceResourcesTest`, `RustNativeLabCardBridgeTest`, `VisualFrameMailboxTest` once B1 wires the app module. On API 26/35 devices check loading/failure, rotation, offscreen reuse, GPU fallback, video/comparison/model playback, full-screen navigation, touch/scroll, IME/TalkBack. Contract: `apps/openagents/apps/android/platform/README-c3.md`. | Pending |
| #12101 | After the release owner approves an internal build, confirm Play App Signing continuity and that the installed app's certificate matches `assetlinks.json`. Install the signed APK over the retired app on a dedicated device without clearing data; confirm the synthetic account, wallet seed, grants, and settings survive. Review internal testers, privacy/Data safety, content rating, Firebase configuration, and the supported halt/rollback path in Play Console. Do not upload or publish without owner approval; keep signing credentials and Firebase service credentials out of Git. | Local artifact checker and release handoff are in source. Signed build, Play Console, internal tester, upgrade device, and publication checks remain for the owner after the app shell is integrated. See `NEEDS_OWNER.md`. |
| #12095 | After Android project/session integration and focused checks: dedicated test-account/header qualification of stable default cloud chat, live selected-host catalogs, projects/branches/sharing, cloud entitlement/lifecycle, and continuation/grant/account-switch behavior. Remove all synthetic resources; see NEEDS_OWNER.md. | Typed device origin and the Rust-owned, target-fixed continuation selector are integrated; default mobile tests passed (256, one ignored), both Android ABIs and binding generation passed, and Kotlin source sets plus 119 JVM tests passed. Device/account flows remain unrun in AtlantisPleb/workspace NEEDS_OWNER.md (0d90af0d75). |
| #12099 | After shared Android recovery integration and offline checks: synthetic first-launch/provider/seed continuity, backup/restore, real device authentication, task-preview/screenshot protection, keyboard/autofill, clipboard deadline/process death, and restart checks on API 26/current Android. See NEEDS_OWNER.md. | Android arm64/x86_64 core builds passed; generated bindings, all Kotlin source sets, and 116 JVM tests passed at `212f64b3e1`. Device checks remain unrun; owner checklist is in `AtlantisPleb/workspace` `NEEDS_OWNER.md` at `0d90af0d75`. |
| #12113 | After the next desktop update, confirm an agent build's `RUSTC` points to the selected toolchain binary and its shared build directory has `.openagents-compiler` with the matching version. | Offline two-compiler regression reproduces E0514 without kache and checks compiler-specific state; host activation remains with the ordinary app update. |
| #12062 | After the next app update, on a scratch host with the dedicated test account and a test repository, use `work_issues` to start a GCE issue under a saved agent budget, retry the same request ID, read progress, and cancel. Check the owner command when the budget or pool grant is absent. Grant the test agent before starting hosts, then tear down hosts and delete test chats and issues. | Public SDK, durable host service, engine RPC, and app tool checks pass with fake servers and runners. App activation and the dedicated-account pool check remain. |
| #12132 | After the next TestFlight build: with the dedicated test account, open a chat hosted on a scratch computer, send a message, and confirm assistant text and tool rows appear while the turn runs and "Sending to host…" clears when the host takes the message, without tapping Stop. Repeat a few times, since the bug only shows when the phone and host reach different sync-server instances. | `cargo test -p openagents-client` live 10/10, including the new other-instance test that fails without the fix (`6ad37f2025`). Device check to do. |
| #12038 | After the web and app updates, on a scratch host with the dedicated test account and a test repository: resave the coordinator from its template, let it dispatch code work for one working day, and confirm every change reaches `main` through a landing entry with attempts in `land show`. Each check-in calls `land_status`, lists waiting and failed changes, and puts a stopped integrator first. Delete test chats, issues, and queue fixtures afterward. | SDK and HTTP routes share the CLI queue; app tool uses the signed-in account and public SDK. Local fixtures cover retries, permissions, queue states, and attempts. Live working-day check to do. |
| #12036 | After the next host update, on a scratch host with the dedicated test account, call `machine_status` with fixture owner documents and a runtime account without actAs. Verify each check-in includes `owner_wait`, its sources and ages, and "production deploy: owner only". The host feeds `NewEntries::observe` to the #12040 owner notification hook; verify one notification per new entry per agent definition ID. | Fixture checks cover parsing, ages, read-only IAM, prompt facts, and per-agent new-entry tracking. Host wiring and the fake notification test pass; deployed check to do. |
| #12037 | On a scratch host with the dedicated test account (never the owner's) and a test repository: save Foreman from the updated template, grant its agent definition ID a budget (`openagents cloud budget --agent ID --daily 5`), and let a check-in see a new startable issue. It starts one `openagents chat work --on gce --land queue --agent ID` run in the background, the run shows on the OpenAgents Cloud board, and `pgrep -x cargo` on the orchestrating computer stays at 0. Revoke the grant (`--agent ID --revoke`): the next `cloud up --agent ID` is refused with the owner command. Delete the test chats and issue afterward. | Code on main (`102afc059d`): template dispatch, `chat work --on gce` on the pool (briefed:codex), budget grants keyed by agent ID; `an_agent_starts_hosts_only_under_its_own_grant` covers the refusal. The live pool run of #12058 passed (#12098 landed as `9a70ce012b` from a pool host). The `work_issues` app tool is #12062. Scratch-host check-in run to do. |
| #12041 | On a scratch host with the dedicated test account, match a throwaway issue claim to a terminal run with `list_runs`: recent activity keeps the claim, while a dead run or one silent past the configured window gets a release note and a new dispatch. Confirm the decision log names the run and action, then delete the test issue and chats. | Fixture and crate checks recorded in the closing comment; scratch-host check to do. |
| #12032 | The 2026-10-17 replay (#12034): the coordinator stall that started at 18:25 gets a message at 18:45 and a restart by 18:55. On a scratch host with the dedicated test account (never the owner's) and check-ins on for a saved Foreman: have it start a chat that runs `sleep 1800`; the check-in at 20 minutes lists the stall with "rung 1: message it", one at 30 with rung 2, one at 40 with rung 3, and the Decision log records each rung. Claim a throwaway test issue from a session with no app chat and no commits: the same ladder runs for the claim; start a `cargo build` in a worktree named for that issue and the claim stops counting as stalled. The final-rung hook now calls `notify_owner_from_chat` (#12040); check that one rung-3 push reaches the test account's phone. Delete the test issue and chats afterward. | Code on main (`4a767cd3ed`); engine 35/0 (silent chat raises one stall and climbs the ladder at 20/30/40, a printing chat raises none, claim with no progress climbs the ladder while an active build or a chat with output counts as progress, a commit resets it, release ends it, owner told once per stall, configurable window), proto, agent-contract and agent-lab pass; the claims GraphQL query checked read-only against the public repo. Replay, scratch-host check and phone push to do. |
| #12045 | Deploy openagents-web (owner-authorized through Foreman) so the account takes the `log.write` grant and serves `/labs/agent/ID/card`. Then, with the dedicated test account (never the owner's): save a non-Foreman agent, open its card on the website and turn **Decision log** on and off; on a scratch host, its chat's `record_decision` is refused while off and written to that agent's own log while on, and today's Foreman keeps logging and gains the grant once. Run the Design Lab captures (`apps/openagents/scripts/design-lab-capture-desktop.sh` for Settings → General, `design-lab-capture.sh` for the phone's `agents--agent-card`) off the owner's busy Mac. | Engine 33/0, agent-lab autonomy 4/0, ui agent_wakes 2/0, mobile custom_agents 8/0, web 10/0, the iOS simulator build passed. Deploy and live checks to do. |
| #12110 | On the next host upgrade (Foreman's `upgrade_host`, never by hand): before it, note a running chat's `openagents mcp` pid (`pgrep -f 'openagents mcp'`). After the swap, have that chat call any tool: the pid is the same, the call succeeds (a call made while another of its calls was still running may say it's restarting; it works when called again), and `record_decision` with `working` now saves the working state. A call with an undeclared field is refused and names the field. Servers started before `babcf08eed` can't do this; they still need their runtime restarted once. | Code on main (`babcf08eed`); openagents-mcp 73/0 (framing, hand-over, replacement detection, exec once with the fallback answering, list_changed, unknown-field refusal). Live upgrade check to do. |
| #12042 | On a scratch host with the dedicated test account (never the owner's), after the next host upgrade: in a saved agent's chat, call `judge` with `run-progress` over a silent 20-minute window and with `failure-on-main` over one failure. Each returns typed answers and a receipt; pass both receipts to `record_decision` and confirm the Decision log keeps them and the next check-in prompt shows `(judged: ...)`. Rerun the four baselines on examples from a new day (`cargo run -p jev --example judge_baseline`). Turn on `OPENAGENTS_LAND_OVERLAP_JUDGE=1` on the integrator only after `land_plan` reads the app workspace's graph, then measure `change-overlap` again on pairs the rule finds apart. | Code on main; jev, coder (land_plan), openagents-mcp, proto and engine tests pass; the live baselines are recorded (`docs/decision-models/measurements/2026-10-18-coordination-judgments.md`, Clef answered all 90). Scratch-host check to do. |
| #12104 | Install the release build sweep with `scripts/dev/install-build-sweep.sh` under Foreman's maintenance grant; observe lock handoffs and free disk above 100 GiB under the current build load. The explicit-shared-target shim change activates with the next desktop update. | Sweep installed from `9987920740`; 122 tests pass. From 02:35:55–02:41:57 CDT on Oct 18, 25 samples stayed at 165.1–180.7 GiB free with 8–11 Cargo processes. Live profile locks still held; kernel waiters queued. Desktop shim activation remains for the next update. |
| #12106 | Reinstall the bounded build sweep under Foreman's maintenance grant, remove its pause file, and verify completed summaries within 10 minutes with at least 100 GiB free. | Installed from `02f31996c9`, pause removed; passes at 03:18:21 and 03:22:01 CDT on Oct 18 finished in 140.6s and 41.2s and freed 67.8 and 3.9 GiB. All 128 checks pass. In 42 samples from 03:16:19–03:26:36 CDT, disk stayed at 249.3–314.5 GiB free with 12–14 Cargo processes. A later metadata scan stalled under compiler I/O load; tracked separately in #12107. |
| #12071 | On the next ordinary desktop update, install the build sweep from this commit with `scripts/dev/install-build-sweep.sh`; confirm an agent build uses one shared target with source-keyed `worktrees/` state and incremental off, and the sweep reports those states inside the target's cap. | Code checks and real two-worktree regression pass; activation after the app and sweep update. |
| #12049 | Deploy the web from `main` at `c655c2621d` or later (adds `POST /v1/published-agents/{id}/fork`, `POST /v1/agent-definitions/{id}/publish`, `GET /v1/agent-definitions/{id}/upstream`). After the official Foreman is published (#12048, NEEDS_OWNER.md), on a fresh scratch host (temporary `HOME`, `--state`, `--root`, `--tasks`) signed in as the dedicated test account (never the owner's): Agent Lab shows **Fork Foreman**; fork it, change the model and stall rule, and **Use agent**. The new agent has no grants (`GET /v1/agent-definitions/{id}/grants` is empty) until you grant `chat.message` over one scratch chat. Then its first check-in arrives, a steer to that scratch chat is allowed while one to another chat is refused, and its decision log has an entry. `GET …/upstream` shows `newer: null`. Delete the test agent, chats and experiments afterward. | Code on main (`c655c2621d`); root crates 1,677 passed, the two failures fixed and rerun (two-account route test now passes: fork has no grants, the original keeps its own; SDK reference regenerated); engine `agent_wakes` (fork wakes the same), MCP (same tools for every agent chat), desktop copy and agent-lab pass. Deploy and scratch-host check to do. |
| #12048 | Deploy the web from the commit that closes #12048 (adds `/v1/published-agents`). After the owner publishes the official Foreman (NEEDS_OWNER.md), `curl -s https://openagents.com/v1/published-agents/04288eb136553674824afea5992734e56e9ede0508bf8a931b9d5023d5255a2c` signed out returns revision 1 with eight skills. Then on a fresh scratch host (temporary `HOME`, `--state`, `--root`, `--tasks`) signed in as the dedicated test account (never the owner's): **Start from Foreman** in Agent Lab gets that revision (its draft has eight skill references), and with a small project **Run tests** passes the three automatic tests, with the two reviewed ones waiting for review; **Use agent** saves it. When the owner publishes revision 2, a second **Start from Foreman** on the same scratch host, after **Refresh**, gets it with no app update. Delete the test agent and experiments afterward. | Code on main; agent-lab 32/0, agent-contract, openagents-sdk (lib and example), openagents-web published agents 2/0 (stable ID, revisions, retry, one publisher, signed-out SDK read). Deploy, publish and scratch-host check to do. |
| #12030 | After the web deploy for #12029 (definitions accept `autonomy`), on a fresh scratch host (temporary `HOME`, `--state`, `--root`, `--tasks`) signed in as the dedicated test account (never the owner's): **Start from Foreman** in Agent Lab, save, and **Use agent**. With no settings change, the agent's chat shows the host's note that check-ins and standing authority are on, Settings → General → Foreman says this computer turned them on, the first check-in arrives within 10 minutes, and the Decision log shows "Standing authority turned on." Turn authority off: the next check-in says it can only report. Turn check-ins off and ask the agent something: its answer starts with "Check-ins are off on this computer, so I only wake when you ask." Delete the test chats afterward. The owner's existing Foreman gets this only after it is saved again from the updated template. | Code on main (`405348f763`); engine 29/0 (declaring agent picked whatever its name, a name alone never picked, two declaring chats pick none, grant logged, report-only wording, check-ins-off note), proto, UI card, agent-contract and agent-lab pass. Scratch-host check to do. |
| #12070 | Owner, after the next host upgrade (none until the production deploy reports done): quit and reopen the desktop app a few times. It opens on your synced workspace with your account's chats, without the "Sync is ready" prompt. If the prompt does show (the server was slow or offline at launch), it says your account's chats are on your synced workspace; pick Later, reopen the app, and it stays closed until tomorrow (Finish sync setup in the account menu still opens it). | Code on main; `openagents-account` 115/0, engine `openagents_account` 3/0 (new: a saved login opens the synced workspace when the server takes 1.2 s; a blocked Keychain read still doesn't hold up startup), `openagents-ui` shell and settings 159/0 (remembered Later, copy guard). Owner look: to do. |
| #12029 | Deploy the web from `main` at `5161465aa1` or later, so saved agent definitions accept the optional `autonomy` block (until then, saving a definition that sets it is refused as an unknown field). Then, with the dedicated test account on a scratch host (never the owner's): **Start from Foreman** in Agent Lab, save and **Use agent**, and confirm the saved definition carries `autonomy.shell_timeout_secs: 120` and the chat's Claude Code process has `BASH_DEFAULT_TIMEOUT_MS=120000` and `BASH_MAX_TIMEOUT_MS=120000`. Pick that chat in Settings → General → Foreman, ask it to run `sleep 900`, and confirm the shell call stops at 2 minutes; with the cap removed, confirm the host stops the turn after 10 minutes of no output and the next check-in names `Bash` (`sleep 900`). An existing saved Foreman gets the cap only after it is saved again from the updated template. The 2026-10-17 replay (#12034) checks the 14:11 hang is caught by 14:22. | Code on main (`5161465aa1`); engine 24/0 (stuck turn, busy turn, once per turn, host interrupt and check-in), harness env test, agent-contract and agent-lab pass. Deploy and scratch-host check to do. |
| #12059 | Owner, after updating desktop and phone: choose a saved agent or renamed fork (or use the configured default chat), call with chat X open, and ask a question. The agent chat shows the owner’s request marked Spoken with X’s ID and title, keeps its full answer, and voice speaks a short summary. Navigate to chat Y, ask while the agent is busy, and check that nothing is dropped. Change the voice choice and check the next question reaches that chat. Without a choice or default, the call offers Voice Settings. | Code on main; 55 passed, 0 failed fixture tests (engine 14 + 6 integration, harness 2, MCP 3, mobile 12, proto 6, shared session 12). UI examples and changed libraries checked; iOS simulator core and headless Xcode test build passed. Owner real voice check: to do. |
| #12061 | Deploy the voice-agent account route, then owner: choose a saved agent and a renamed fork from desktop and phone Settings and call controls; check the choice agrees on both and the next spoken question reaches the chosen chat. | Fixture checks only; owner real voice check pending. |
| #12060 | Owner: on an updated build, make two voice calls and check that each starts fresh, user Codex plugins stay absent, and a late report to the ended call is refused. | Fixture checks only; owner real voice check pending. |
| #12293 | Deploy the web chat and computer host, then use Chrome with the dedicated test account and a scratch-host Codex chat. Choose the voice agent, allow the browser in Settings → Devices, and make a two-minute call with captions. Check mute, the timer, end-call capture cleanup, denied microphone access, a lost connection, and grant revocation. Delete the test chats and browser grant afterward. | WASM compilation, 15 client tests, and Chrome synthetic MediaStream playback under the existing CSP passed. The two-minute call is to do. |
| #12039 | On a scratch host with a dedicated test account, call `machine_status` for two agent definition IDs and compare local files, landing queue, and GCE budget readings. The check-in host uses `machine_status::collect`, `Status::prompt_block`, and `Alerts::observe` as described in `docs/coder/runtime/leases.md`; verify its separate integration includes the block and wakes once per shortage. | Fixture checks cover the tool block and alert API. The two Foreman files stay with their current editor; host integration and deployed checks remain. |
| #12026 | Deploy web from the commit that closes #12026 (`PATCH /v1/threads/{id}` takes `pinned` and `archived`). Then, on a phone build from `main` with that commit, signed in as a dedicated test account (never the owner's): open a cloud chat, send a message, and use ⋯ > Pin, Copy Transcript, and Archive. The website's sidebar for that account shows the chat pinned, then archived; the copied text holds both sides of the chat; Archive starts a new chat. | Code on main; web, SDK, account, and mobile tests pass, and the iOS Simulator build passes. Deploy and phone check to do. |
| #12031 | On a test host (never the owner's Mac or account) with the dedicated test account and the app running, run `openagents chat open "Say hello"` inside a checkout that is an app project. It prints a chat ID; the chat is in that project, shows in `list_chats`, in the app's Sessions, in the phone's chat list, and on the website, and answers the brief. A message sent to it from the phone reaches it within 10 seconds. Delete the chat afterward. | Code on main; `chat_open` unit tests pass (argument parsing, the `create_chat` request, the checkout's project, the printed receipt). The create path itself is the engine's `create_chat` tool and its tests. Live run: to do |
| #12053 | Deploy the run presence routes from `main`. With a dedicated test account and scratch host, publish through `Client::runs`, verify another account sees no rows, stop heartbeats, and confirm `live=true` stops listing the run after three minutes. End the test run afterward. | SDK, server routes, and OpenAPI implemented. Local: 81 SDK and 913 web tests passed (18 ignored), including 10 new tests; the final focused check passed 47 tests (0 failed), with verbose compilation of both crates from `~/work/oa-12053`, `CARGO_INCREMENTAL=0`, and source-keyed build settings inside shared target-2. Sender integration is separate from this API change. The Python public-reference validator still rejects the preexisting session-move routes. |
| #12055 | Deploy web from `main` with `/v1/attention`. With the dedicated test account and a scratch host, open `Client::attention(after=0)`, send one `working` heartbeat whose output time is 21 minutes old, and confirm exactly one `run_stalled` event arrives within about 15 seconds; reconnect with the last `seq` and confirm it is not resent; a second test account's stream stays empty. End the test run afterward. | Code on main; SDK and web attention tests pass (ordering, cursor and `Last-Event-ID` resume, one stall per episode, account separation, dropped-connection resume). Live check to do. |
| #12054 | Deploy the run command route and current app host. With a dedicated test account and scratch host, bind the native caller identity, use the shared #12044 account-grant checker, and send Message from another device through the SDK in under 10 seconds; then check Steer, Stop, Restart, terminal refusal, and revocation. Delete the test chats and runs afterward. | Local checks: SDK commands 2, web commands and refusal mapping 4, engine commands 2, sync 85, schema 1, and SDK reference 2 passed. Full app checks: 945 passed, 4 failed, 27 ignored; unrelated failures are tracked in #12118 and #12119. The post-rebase app focus was skipped at Foreman’s direction due to the shared-target queue; main CI and the next host upgrade cover it. Agent commands default to refusal until the shared grant checker is installed; #12044 owns that wiring if it lands after this change. |
| #12020 | On the next iOS build from `main` with this fix, signed in as a dedicated test account (never the owner's) that has a project, with that account's OpenAgents Cloud started and online: New session > Run on lists **OpenAgents Cloud** ("No project · Online") beside the project's computers, and the account choice **OpenAgents** shows the OpenAgents mark. Pick OpenAgents Cloud: the chips read No project and an OpenAgents model (Fast, Code, Reason or Gemini Flash Lite). Send `Who are you?`: Working shows at once, the answer streams in, tool and status rows appear as in a desktop session, and the chat is listed on that cloud device. | Code on main; Run on unit tests 4/0 on a Simulator, client catalog tests pass. That the reported turn went to the account chat is inferred (no chat exists on the owner's cloud device, which synced the same registry before the report); not proven. Live stream/Working on the cloud engine to check. Do not use the owner's account or cloud computer. |
| #12019 | Owner, on the next iOS build from `main` at `92fc250db2` or later: open a saved OpenAgents chat (one hosted on OpenAgents, not a computer) and send a message. Working shows under it before any text arrives and stays while the answer runs; it goes away when the answer finishes and when you tap Stop. Switch to another chat mid-answer and back: no Working row is left behind. A computer session's Working row is unchanged. | Code on main (`92fc250db2`); adapter and layout tests 2/0 (`saved_reply_activity_survives_complete_message_snapshots_and_clears_when_idle`, `working_row_follows_turn_activity_even_with_complete_saved_entries`). Device check to do. Not shown to be the cause of the selected cloud-computer report (#12020). |
| #12017 | On the next iOS build, use a disposable test call: rename its host in Devices and confirm the timer header updates while the call continues, including after leaving and returning to the call screen. | Focused host-label tests cover rename, another next-call host, and missing metadata; live device check to do. Do not change the owner’s active call or device preferences. |
| #12014 | On the next iOS build, open Settings > Voice (Talk with Codex), choose Spruce, and confirm the closed row shows Spruce. Repeat during a disposable test call, return to the screen, and choose Codex default; the row follows each saved choice and the next-call explanation stays visible. | Source and headless build checks recorded in the issue; device check to do. Do not interrupt the owner’s current call or change their preferences. |
| #11999 | On the owner's iPhone, install a build from `main` with the wallet commit over the current OpenAgents app (same bundle ID `com.openagents.app`, team `HQWSG26L43`; don't delete the app first, which would remove the Keychain item). Open Wallet from the drawer and confirm the old balance and history appear. If it says the wallet is locked, unlock and tap Try again; it must never offer Make a wallet while the old seed exists. | Code on branch `agent/11999`; coordinator build, then device run. |
| #12005 | Deploy the gateway and web from `main` with #12005 (the gateway config no longer renders `accounts.apple`). Then on production: `POST https://api.openagents.com/v1/device/apple` answers 410 `apple_removed`; `/login` leads with "The OpenAgents app needs no sign-in." and keeps Continue with GitHub; a dedicated GitHub test account that already has an OpenAgents account still signs in; a GitHub account with no OpenAgents account lands on "No account uses that GitHub account" and no account is made; Coder 1.0.0's `coder login` device flow still approves from a website session made by an existing account. | Code complete; the coordinator's build and checks, then deploy and live checks, to do. Supersedes the #11980 row's Apple checks. |
| #12002 | Deploy the gateway and web from `main` with the account-deletion commit. Then, with a throwaway account made for the check (never the owner's): `DELETE https://api.openagents.com/v1/account` without `confirm` answers 400 `confirm_required` with `export_url`; with `{"confirm": "delete my account"}` it answers 200, the account's chats, projects, connections, and cloud computers are gone, and its token answers 401. On a phone build and a desktop build from `main` with another throwaway account: Settings > Delete Account shows the 12-words warning, deletes, and returns to the first screen; the wallet still opens from its 12 words. | Code complete on branch `agent/12002`; needs the coordinator's build and merge, the deploy, then the checks. |
| #12007 | Deploy the web from `main` with the SDK docs. Then `https://openagents.com/docs/sdk`, `/docs/sdk/api/chats`, `/docs/sdk/rust/chats`, and `/docs/sdk/search?q=reply` answer 200; `/docs/sdk/api/chats#replyToThread` shows its request, errors, and `curl` example; `/docs` links the SDK; `/openapi.yaml`, `/v1/openapi.json`, and `/v1/docs` answer as before. | To do |
| #11980 | Deploy the gateway and web from `main` at `104a00ba0c` or later (the gateway config renders `accounts.apple` with aud `com.openagents.app`). Then: `POST https://api.openagents.com/v1/device/apple` with a bad token answers 400 `access_denied`, not 404. On a phone build from `main` with the iOS commit (Sign in with Apple capability is on the App ID; the App Store profile `OpenAgents App Store` was regenerated with it on 2026-10-17 and installed locally), with a dedicated Apple ID test account: Sign in with Apple signs in and a second sign-in reaches the same account; Sign in opens the approval page in the in-app sheet, Approve closes the sheet and the app is signed in; closing the sheet leaves the code and Open website as the fallback. Delete the test account's chats afterward. | Server done 2026-10-17 13:56 CDT: gateway `openagents-stack:56932eae75` and web `stg-56932eae75` on production; `POST https://api.openagents.com/v1/device/apple` and `https://openagents.com/v1/device/apple` with a bad token answer 400 `access_denied` (the gateway's `/v1/sessions/apple` is first-party and 404 from outside by design). Phone device run with a dedicated Apple ID still to do. |
| #11978 | Deploy web from `main` at `c40a1a4019` or later. Then on production: `POST /v1/visitors` answers 201; a visitor chat is answered and refuses with `sign_in_required` after 10 messages; `POST /v1/visitors/claim` from a dedicated test account moves it; `/v1/cloud-host` and `/labs/files/…` are answered by the web, not the gateway. | Done 2026-10-17 13:56–13:59 CDT on production `coder-web-e8c3eeb00a-20261017185349` (web `stg-56932eae75`): `POST /v1/visitors` 201; a visitor chat answered in 10 s; message 11 refused 409 `sign_in_required`; `POST /v1/visitors/claim` from an operator-made test account moved 1 chat (20 messages under the account, 404 for the visitor token); `/v1/cloud-host` and `/labs/files/…` answer the web's `signed_out` 401. Staging the same (56932eae75). |
| #11978 (app), #11979 | After the web deploy above, on a phone build from `main` at `7d6cee982f` or later with a dedicated test account: signed out, the first screen is the OpenAgents chat; a message gets a streamed answer; near the limit the "messages left" line shows, and at the limit the sign-in sheet; Sign in, and the open chat continues and appears in OpenAgents chats on the web. Signed in with no computer online, New session runs on OpenAgents and the chat appears in the list. Simulator flow passed against a local visitor server (`CloudChatTests`). | Code on main; needs the web deploy, then a device run. |
| #11982, #11984 | On a phone build from `main` with a dedicated test account and no computer: ask a question in the OpenAgents chat; after the answer, the "Run this on your own computer" card shows once, the command copies, the links open `/download` and `/device`; close it and it stays closed after a relaunch; signed in, it goes away when a computer joins. Retake the store shots with `MarketingShotsTests` (iOS README) on the largest iPhone, light and dark. Simulator: `MarketingShotsTests` passed and saved the demo shots. | Code on main; needs a device run. |
| #11896 | Complete the #11894 terminal-to-native and #11898 linked-home checks, plus #11897 native publisher timing, using dedicated accounts and disposable hosts. Keep the web transcript off until the chat or project opts in, then remove the fixtures. | Child source and checks complete; lane 0 owns the listed cross-device checks. |
| #11898 | Deploy the server changes before distributing the updated native apps. With the dedicated account and disposable hosts, continue a web chat on a Mac, then back on OpenAgents; check one latest conversation row, ordered links, read-only earlier parts, bounded context, and transcript opt-in on web, desktop, and phone. Repeat with another paired computer and a cloud computer, interrupted requests, host restart, and account switching; delete all fixtures. | Source checks passed; #11958 repaired the broader UI suite (1,786 passed serially). Dedicated-account and device checks: to do. |
| #11817 | Deploy the new host and computer service. Configure a disposable Unix host, scoped test-account grant, and explicit enrolled target. Run the public attach, command retry/reconnect/cancel, relative-file, revocation, unsupported-control, and detach flow; verify files and unrelated grants survive. Repeat the Linux-specific command checks on Linux. | source checks passed; deploy and dedicated-account run to do |
| #11911 | Deploy a host advertising `project-sharing-v2` and the matching mobile build. With two dedicated test accounts and disposable projects, check owner approval, read-only and edit/command permissions, wrong-device and cross-project denial, retry/reconnect, expiry, scoped revocation, and account switching. Confirm unrelated chats, files, terminals, credentials, and account controls remain unavailable. Physical device and accessibility steps are in `NEEDS_OWNER.md`. | Source checks passed: access 124, host 118, sharing 12, mobile 58; deploy, live, and device checks to do |
| #11934 | Deploy the agents door from `1708f44f49` or newer. Run `ask_foreman_cli.py` and `openagents agents ask Foreman …` with test account `acct_500d3e1d125b7782`. The web door's switch to `place` is on `main` (`1708f44f49`). | Deployed in source `0b74ec8f79`, production smoke 66/0/2. Native list/status, native `agents ask`, the website saved-agent request, and an unchanged normal answer passed with an isolated Rust SDK runner; fixtures were removed. [Evidence](../sdk/agents-evidence.json). The earlier OpenCode placement failure has a source fix in #11940; its live recheck remains below. The original Claude Code Foreman workflow remains to do. |
| #11940 | Rebuild the headless engine with the OpenCode saved-agent fix. On a disposable host and dedicated account, ask Foreman with the advertised OpenCode model and host-default access; confirm its saved instructions are followed, then remove the agent, assignment, chat, and host. Repeat the original Claude Code flow under #11934. | Source checks passed: saved-agent admission 5, Agent Lab consumer 4, OpenCode harness 73, and the terminal consumer build on Rust 1.97.1. Deploy and dedicated-account run to do. |
| #11930 | On a test host with the dedicated test account, start a real Claude Code turn and a real Codex turn in the app, quit or kill the engine mid-turn, and relaunch. Each turn continues in the same transcript entry with no gap or duplicate, a pending question comes back, and a steer after relaunch reaches the agent. Then repeat with `OPENAGENTS_KEEPERS=0` and confirm the old auto-resume. Delete the chats afterward. | Fake-CLI checks passed: harness `keeper_drivers` 7, `keeper` 5, `claude` 25, `codex` 35; engine `keepers` 5; `cargo check -p openagents`. Live run: to do |
| #11946 | On a test host (never the owner's Mac) with the dedicated test account, run the background engine from a packaged build, start a real Claude Code turn and a real Codex turn, then install a newer build at the same path. Within about a minute the engine restarts into it, and each turn continues in the same transcript entry with no gap or duplicate. A message sent during the restart shows the restart notice, and an open terminal holds the restart until it closes. Delete the chats afterward. | Fake-agent checks passed: engine `keepers` 9 (update during a live turn, drain, previous-release keeper, unknown protocol), engine lib `handoff` 1, harness lib `keeper` 5; `cargo check -p openagents`. Live run: to do |
| #11951 | On a test host (never the owner's Mac) with the dedicated test account, start a real turn with each of Grok, Devin, Hermes, Antigravity, Pi, Cursor, and OpenCode (1.x and 2.x) in the app. Kill the engine mid-turn and relaunch. Check that each turn continues in the same transcript entry with no gap or duplicate, and that a steer after the relaunch reaches the same agent. Check that a pending ACP or Pi question comes back. For Antigravity installed from its archive, check that it still runs without a keeper. For OpenCode 2.x, the turn resumes from the bus only, with no history catch-up. With an open terminal, check that an update no longer waits for it. Delete the chats afterward. | Fake-agent checks: harness `keeper_drivers` 18 (ACP mid-turn, later turn, and question; Pi mid-turn and dialog; Cursor; OpenCode 1.x; keepers off for each), `acp` 75, `pi_rpc` 14, `cursor` 17 of 18 (the failure is a serde key-order issue in the test build, not this change). Live run: to do |
| #11925 | Deploy `534ecb5c67` or newer. With a synthetic account, confirm an account-backed computer read either succeeds or returns 503 with `Retry-After` during a traffic shift, and a delete during that gap is not applied. | Passed on `b50cb54165`: the zero-traffic candidate returned 503 and `Retry-After: 1` for the dedicated account’s read and delete; serving production returned 200 before and after the shift. No computer was created; the delete targeted a unique nonexistent ID. [Receipt](../sdk/labs-cad-staging-handoff.json). The separate grouped-test failure is #11952. |
| #11727 | Deploy `ed4c8d05c4` or newer. After Stripe test mode is set up (owner, 2026-10-17), a named sale through a short link records the introduction. | to do |
| #11936 | Redeploy the SPACE candidate with explicit zero-length framing and verify suspend and wake through Cloud Run. | Capacity-2 checks passed on `00002-pch`. The first fixture also passed after #11939 recovery on `oa-space-service-computers-00003-ffl`: encrypted checkpoint/resume preserved boot ID, PID, start time, and memory canary; seven protected endpoints were denied. Both fixtures ended stopped. [Earlier evidence](../cloud/evidence/2026-10-16-space-private-foundation.json); new recovery evidence is in `docs/sdk/computers-usage-evidence.json`. |
| #11939 | Deploy the checked private candidate, run the explicit operator retry for the saved rejected suspend, then qualify encrypted checkpoint and resume. The [usage evidence](../sdk/computers-usage-evidence.json) has the exact request and expected child. | Passed: 31 source checks, private image `sha256:a58dc392…4c8db`, expected retry child, encrypted checkpoint/resume, and seven network denials. Operator execution `oa-space-retry-11939-q2j8l` succeeded; its temporary job was deleted. Evidence: `docs/sdk/computers-usage-evidence.json`. |
| #11813 | Deploy the checked native credential adapter and private broker with pool-scoped Google CA signing, an explicit numeric issuer resource, private Unix-socket transport, pinned public CA trust, registered synthetic sources, and named test-account admission. Complete real Claude Code streaming, Git clone, gh, suspend/wake with fresh references, restored-marker denial, active-stream revocation, expiry, broker/provider outage, redirect/subdomain/path/method/IP/DNS bypass, IPv6 refusal, and host-control versus guest denial checks. Remove every fixture afterward. | source and provider checks are recorded in [credential evidence](../sdk/computers-credential-evidence.json); full client workflow and customer activation remain unqualified |
| #11800 | Remove the empty `oa-sdk-build-11800` subnet and VPC after Google releases its managed SERVERLESS reservation. Verify the scoped inventory; do not touch maintained SPACE networks. | Passed at 08:05 UTC: no address reservations, VMs, disks, firewalls, routers, or Cloud Run service attachments remained. Ordinary subnet and VPC deletion succeeded; both describe calls now return not found. [Receipt](../sdk/computers-delivery-evidence.json). Maintained SPACE infrastructure was not changed. |
| #11816 | Follow the [qualification runbook](../sdk/space-qualification.md) on the combined checked deployment: dedicated-account public SDK flow and useful web task/follow-up; account isolation; actual snapshot mode and shape; memory/workspace and fresh credentials; interrupted or duplicated requests and child cancellation; worker loss, fencing, checkpoint/database recovery, storage/provider outages; credential and network enforcement; capacity, quotas, retention/account deletion/orphans; upgrade, rollback, disabled placement, and legacy Boat. Retain pinned dated receipts, cold/snapshot/wake distributions with cache/concurrency, complete service economics, and cleanup inventory. Assess the packet before any named-group admission. | Final rerun 2026-10-19 on Computers `85141f12a6` and the patched node image `7aae0e41`: dedicated-test stage passes; named group does not (wake p90 33.5 s in the packet series; no snapshot-create samples because encrypted organizations boot cold). 31 cases passed, 0 failed, 2 pending (web task needs a SPACE environment artifact; credential adapter not deployed). #12141, #12142, #12143, #12146, #12151, #12164, and #12169 fixed, deployed, and closed; 30 wakes on `7761ec5326` after the #12169 fix had p90 4.3 s. See the [status](../sdk/space-qualification.md#status-2026-10-19). The rerun's worker reset confirmed that both reset sandboxes' node files were erased after deletion (#12151). After-close: rerun the packet on `7761ec5326` or later. No customer rollout or pricing enabled. |
| #11814 | Promote the checked cleanup images, install eleven measurements and nine alert policies, then run encrypted lifecycle, worker replacement, concurrent wakes, drain/rollback/placement disable, guest denial, retention, and fixture cleanup. Use the [private service runbook](../../deploy/space-service/README.md) and [receipt](../cloud/evidence/2026-10-16-space-private-foundation.json). | Source checks passed 50/0; capacity-2 memory and seven denials passed. Lane 0 installed the current collector: eleven measurements publish, nine policies are enabled, and the automatic timer reports no collector errors; no application service or IAM changed. The latest read-only inventory has zero active snapshot objects, zero unfinished uploads, and 30 soft-deleted generations across both buckets (753,282,292 bytes). [Receipt](../cloud/evidence/2026-10-16-space-private-foundation.json). Cleanup-image promotion, replacement, rollback, and combined qualification remain; the listings had no write fence and do not prove cleanup. |
| #11875 | The dedicated-account CAD run on staging and then production: fan bracket, picked face, 5 mm thicker, project versions, STEP download, cleanup. | Passed on source `79f8c18f71`: staging 99/0/2 and production 66/0/2; both passed 18 CAD checks, browser face selection, and complete cleanup. Evidence: `docs/sdk/labs-cad-staging-handoff.json` and `docs/sdk/labs-cad-evidence.json`. |
| #11938 | Deploy the sign-in-key account lookup fix, then repeat `/v1/projects` and the complete production CAD run with the original dedicated test key, including project deletion. | Passed with #11941 on source `79f8c18f71`: the original test key reads account, projects, and threads (all 200); all 18 CAD checks and complete cleanup passed. |
| #11941 | Deploy the GCS chat-list fix, then check the original dedicated account's project and thread lists and repeat CAD cleanup. | Passed on source `79f8c18f71`: staging metadata fixture reproduced 503 before and 200 after the fix; production lists return 200 and the full CAD fixture cleans up. Source checks: 15 storage tests and six project API tests. Evidence: `docs/sdk/labs-cad-staging-handoff.json`. |
| #11876 | Run the Git-pinned `examples/sdk-labs` CAD flow against production with the dedicated account, including ranged GLB, notes, events, and deletion. | Passed on production image `sha256:51e2d77b…202a7` using the previously qualified Git-pinned binary: CAD edits/refusal, restored job retry, ranged GLB, notes, 14 events, design deletion, and artifact refusal after deletion. Evidence: `docs/sdk/labs-client-evidence.json`. |
| #11877 | A CAD chat’s card through `/v1/history`, re-rendered in a second browser session. | Passed after #11950 on `b50cb54165`, on staging and production: typed card, requested project/document, two fresh browser sessions with visible 3D models and pixel changes after camera rotation; design, chat, and project cleanup passed. [Receipt](../sdk/labs-cad-staging-handoff.json). The earlier `79f8c18f71` failure remains recorded there. |
| #11954 | Rebuild the complete web image with the Video Lab embedded resources, then repeat staging and the saved-card check before promotion. | Passed: build `217103a6-4c04-4e37-ac8a-1e6aaa03f231`, source `b50cb54165`, includes all six resources. Staging 99/0/2, production 66/0/2, CAD 18 and Echo 21 on both, saved cards in two fresh browsers on both, and cleanup passed. [Receipt](../sdk/labs-cad-staging-handoff.json). |
| #11950 | After the saved-card fix, repeat a hosted CAD chat and load its card through `/v1/history` and two fresh browser sessions. Remove all fixtures afterward. | Passed on staging and production `b50cb54165`: typed saved CAD card, correct account/project/document binding, two isolated browser renders with camera rotation, and complete cleanup. Eleven focused source checks passed before deployment. [Receipt](../sdk/labs-cad-staging-handoff.json). |
| #11878 | Deploy the account-service worker routes before the web worker routes. Run dedicated-account Echo on server, disposable enrolled, and cloud placements; prove private input bytes and paths stay local, retire scoped keys, and observe cloud computer deletion. The cloud environment needs the pinned `labs-worker` and trusted handlers installed. | Platform API checks passed on staging and production (21 each, with cleanup). Worker placement, private-input, key retirement, and cloud-deletion checks remain; owner Mac installation is in `NEEDS_OWNER.md`. |
| #11886 | With a dedicated account and synthetic clips, edit a hosted cut from its saved chat card, verify a new version on the web, render and play the preview on both surfaces, check conflict reload and account switching, then remove every fixture. The hosted video page depends on #11949. | Source checks passed on the main integration: account 75, GPUI Lab card/frame/sign-out fixtures 2, and formatting. The hosted player is on main in `2945c7608c`. Dedicated-account native/web playback and cleanup: to do. |
| #11885 | After the server deploy and native binary build, use the dedicated test account in an isolated HOME: edit a CAD document with `openagents cad`, edit a hosted Video document with Coder, follow its events, open the page links, and delete the fixtures. Check posters and strips in Kitty and iTerm2. Hosted Video depends on #11880; Game and Ship integration remains under #11881. | Local SDK wire, native dispatch, generated-tree, and actual CLI fixture checks passed; production and visual checks: to do. |
| #11840 | Complete the existing #11852, #11854, and #11853 rows below, including the signed app build and dedicated-account iPhone checks. | All 14 children are closed, and final source commits are on `main`. Deploy and device checks remain with lane 0; the push capability and paired-phone steps are in `NEEDS_OWNER.md`. |
| #11852 | Deploy `4a796e8ff8` or newer. Run `OurCloudLiveTests/testStartRunAndStopOurCloud` with the dedicated test account, check `GET /v1/computer-usage`, delete the computer with `DELETE /v1/cloud-host`, and remove its test registry rows. Validate TestFlight 1011 or the next free build. | to do; phone source `d37ed99d0c`, local control acceptance `a67ec60b44` passed |
| #11956 | Rebuild sync on the explicit high-CPU worker within the expanded time limit, then continue staging and production deployment. | Passed: build `3d62ae40-2e00-4da7-a30e-784404eb90fe` finished in 697 seconds on `E2_HIGHCPU_32`; staging and production now serve `c983085ff5` at 100%, pass health, and reject dev bearers with 401. [Receipt](../mobile/coder-desktop-phone-evidence.json). |
| #11955 | Rebuild sync with its server crate in the Git export, then deploy the checked image with the correct public origins. | Passed: image `sha256:b522158f…87ce` built from `c983085ff5`; staging and production started successfully, report the exact revision, use their correct public origins, and reject dev bearers. Startup includes migrations 6 and 7; no direct SQL inspection was performed. [Receipt](../mobile/coder-desktop-phone-evidence.json). |
| #11953 | Deploy sync with the exact staging and production public origins, preserving optional push environment and secret bindings. | Passed on `c983085ff5`: staging origin matches its public `run.app` URL; production uses `https://sync.openagents.com`. Both retain the database secret binding and pass health/auth checks. Neither had optional push settings before deployment; preservation of configured optional values was checked by intercepted deploy arguments. Pairing and push remain under #11854 and #11853. [Receipt](../mobile/coder-desktop-phone-evidence.json). |
| #11854 | Deploy the sync server with migrations 6 and 7 and the correct `CODER_SYNC_PUBLIC_ORIGIN`, then build the opted-in app/headless host. With the dedicated account and disposable devices, pair the phone to the registered computer, run a real model call and a command with its original ID, and confirm the transcript stays in its existing Loro room. Check unsupported and streaming fallback, sign-out, forget/revoke, account switching, and proved-room expiry without a permission downgrade. Compare NIP-HOST and deployed device-room latency, recording revisions, routes, sample counts, and failures before changing defaults. Remove connect-code files and test devices, registry rows, hosts, and chats. | Sync deployed from `c983085ff5` to staging and production with correct origins; both pass health and dev-bearer denial. Source/isolated checks and the normal iOS simulator build passed. Dedicated-account paired-device flow and deployed latency comparison remain; see the [server rollout receipt](../mobile/coder-desktop-phone-evidence.json). |
| #11853 | Deploy `coder-sync` from `b653d7e925` or newer with `CODER_SYNC_PUSH_*` set, and add its key to the gateway's `PUSH_GATEWAY_RELAY_PUBKEYS` (`docs/deployment/push-gateway.md`). After the owner adds the Push Notifications capability, build with `CODER_IOS_PUSH` set and, on a real device with the dedicated test account, check a finished run, a question, and a failure each wake the phone once, and that an open app and an `X-OpenAgents-Test: 1` write don't. | Source checks passed: push_rules 17, push_notify 3, push-gateway 17, mobile push 5. Sync source is deployed, but push is not configured. APNs key, key ID, Apple team ID, and App ID capability are owner steps in `NEEDS_OWNER.md`; gateway deployment, signer configuration, and the physical-device check remain. |
| #11894 | Deploy the Coder and app builds containing `0b84201f39` or newer. With a dedicated test account and `/sync on`, continue a terminal chat on the desktop, confirm its first turn knows the history, and steer it from the phone. | Source checks passed; deploy and dedicated-account run to do. |
| #11881 | Done in code: Game and Ship are registered in `labs_api.rs` behind the experimental gate (`4e8463ddff`); a hosted Game edit queues the `playtest` job and waits for it, and `game-lab playtest-job` plays the job's patch. Remains: deploy, install `game-lab playtest-job` as the `playtest` handler on a test worker, then, with the dedicated test account on production, a Game Lab edit plays in the web card (generic `/labs/game/{doc}`) and appears through `/v1/history`, and a Ship Lab refusal and an accepted edit. Clean up after. | code done; `cargo test -p openagents-web labs` and `-p game-lab` passed; production run to do |
| #11880 | Done in code: Video is registered in `labs_api.rs` behind the experimental gate (`4e8463ddff`), and a worker now reads a job's public stored files (`2f7acc29fd`), so cloud and server renders can run. Remains: deploy, install `video-lab render-job`, `frames-job` and `measure-job` as handlers on test workers. Then, with the dedicated test account on production and synthetic clips: create a cut, make one edit and one refused edit, render, play the card in web chat and through `/v1/history`, and delete the fixtures. A `private_local`-marked synthetic document renders only on the enrolled host. | code done; `cargo test -p video-lab` passed 27/0; production run to do |
| #11882 | Done in code (`2f7acc29fd`, `2c6be74847`): leased worker reads of referenced versions and files, `robot-workbench simulate-job --library PATH --library-sha SHA [--catalog DIR]` as the `simulate` handler, Robot registered behind the experimental gate, and the replay card. Remains: deploy, install the handler on a cloud test worker with a pinned MuJoCo library, then, with the dedicated test account, move a synthetic project on (`robot_lab::migrate`) with its scripted controller uploaded as a source file of the experiment, run it, and check the replay card on the web and through `/v1/history`. The handler's MuJoCo run hasn't run yet; its local project setup and replay are tested. Clean up after. | code done; `cargo test -p lab-api`, `-p lab-jobs --all-features`, `-p openagents-sdk --test labs_worker`, `-p robot-lab --features platform`, `-p robot-workbench platform`, and `-p openagents-web labs` passed; live run to do |
| #11942 | Install `video-lab transcribe-job` (with `--whisper` and `--model`), `import-job` and `score-job` (with `--python` and `--music`), and `render-job --fonts DIR` on the enrolled test host, and run each on a synthetic test-account cut. Render a synthetic cut on a cloud worker from stored public clips with the Mac off. Run the MVP script in the local web chat and Coder 1.0.0 against `video-lab-serve` (`full-local.sh --lab`), and check Episode 290 still renders through `lab.py` (owner's Mac). Add `shot.v1` and `scene.v1` render handlers where Blender and the Verse film assets are installed. | to do; source tests passed (video-lab 37/0, video-lab-serve 3/0) |
| #11884 | Build the iOS app with its Rust core phase from `main` (`com.openagents.app`). Run simulator fixture UI tests for each Lab view (Video plays and goes full screen, image and strip, compare, Model3d and playable open the page, report) and check memory stays bounded while scrolling past videos. With the dedicated test account, open a web CAD chat and a Video chat on the phone, dictate a note while the video plays, and see it on the web at that time. Delete the chats afterward. | Rust layout, facade, and account tests passed; the Swift typechecks against the iOS 26 simulator SDK. Simulator and test-account runs: to do |
| #11949 | Deploy the hosted player with lane 0. With a dedicated account and synthetic cut, verify authenticated media range reads, finished render playback, project/deep links, comparison and review notes, then remove every fixture. | Web library and tests compile; 15 Labs API tests and seven transport tests passed. A fresh-profile loopback browser fixture saved v2, submitted its preview, and drew three tracks at 1080/390 px with no script exceptions; profile removed. Real render playback remains to do. |
| #11943 | With the dedicated test account, open a Video chat in the desktop's saved history, play it, add a note at the playing time, and see it on the web; start a job from the web and see the card update without reopening the chat. Capture fixture screenshots of each view. Delete the chats afterward. | GPUI tests passed (fixture cards, newer card keeps its pictures, note with its time) and the account fake-server test for notes and events; live run: to do |
| #11883 | Build the desktop app from `main`. With the dedicated test account, open a web CAD chat and a Video chat in the desktop's saved history: the cards draw and their pictures load, Open document opens the page, and signing out clears them. Delete the chats afterward. | to do; account tests 15/0 (fake history and artifact server), GPUI fixture test over every `lab-kit` card passed |
| #11815 | Deploy storage allowances and account cleanup through lane 0; collect complete private storage inventories, including recoverable versions, and qualify limits/cleanup on synthetic fixtures. | Source checks passed 71/0 on rebased main, including five storage/cleanup cases. Usage read passed with `acct_83d16a35955ef908`: HTTP 200, pricing off, no charge; the earlier account remains outside the rollout. [Evidence](../sdk/computers-usage-evidence.json). Storage deployment and live cleanup: to do. |
| #11928 | On the next production deploy, a candidate with no traffic does not resume setups or delete homes. | to do |
| #11932 | Build the app from `main`. With the dedicated test account, a disposable repository, and a cloud computer with Claude Code and Codex logins registered as credential sources, run `move_cloud::Mover` for a paused Claude Code chat and a Codex chat: each resumes on the computer with its files, untracked ones included, and its conversation. A chat without a registered login stays local with its reason. The Codex `exec --json resume` arguments and `CODEX_API_KEY` were checked against codex-cli 0.163.0 under #11933, which added `--dangerously-bypass-approvals-and-sandbox`; confirm them, and `CLAUDE_CODE_OAUTH_TOKEN`, live. Delete the computer and chats afterward. | to do; `cargo test -p openagents-engine --lib move_cloud` passed 8/0 (fake Computers server, temporary repository, fake session files) |
| #11897 | Deploy web, and build the desktop app, from `main`. With the dedicated test account, a desktop session appears on openagents.com within about 30 seconds as a summary row; its transcript isn't on the server until opt-in (`openagents::sessions::set_transcript`), then renders on the summary page; a second account sees nothing; removing the computer removes its rows. Clean up the test computer and chats afterward. | The deployed API and website checks passed on `b50cb54165`: summary discovery without a transcript, transcript-off refusal, second-account denial, opt-in read/upload, turning off removes the copy, and complete synthetic-row cleanup. [Receipt](../sdk/history-client-evidence.json). The native publisher, 30-second observation, and desktop-to-web visual flow remain. |
| #11957 | Validate the deployed public OpenAPI document with the corrected session-control rules. | Passed: staging and production each validate 220 operations. Ten reference regressions, four System One consumer checks, the 162-operation app slice, and regenerated inventory passed. No Rust changed or build ran. [Receipt](../sdk/public-reference-evidence.json). |
| #11948 | Build the desktop app and the iOS app from `main`. With the dedicated test account signed in, run a Lab tool from the platform in a live session (desktop and phone) so it returns a `lab.card.v1` card: the row draws the typed card (not the title-only visual), follows its document's events, takes a note at the playing time, and its Versions list shows another version. A Model3d card turns its model by dragging on both. Signing out turns the row back into the title visual. Also confirm which field the platform tool result uses for the card's space; the app reads `space`, else `me`. Delete the test documents afterward. | to do; source checks passed: account 79/0, mcp 36/0, mobile lib 74/0 (1 ignored), UI 1,802/0 serially, and an iOS simulator build (a8b54f4b6d, f5a43b4c06) |
| #11863 | Asset channel and RFQ both ways on signet. The 2026-10-17 run stalled: node B's 20,000-sat funding send (`ed91ae57…`) was created but never broadcast, so the observer waits in `wait_channel_funds` until its 2026-10-19 12:14 UTC limit. A new run needs the owner's approval for funding (no new funds by hand). Confirm each funding send on chain before the next, then open the private channel, run RFQ both ways, close, return BTC, and stop both nodes. See [the report](../payments/2026-10-16-lnd-signet.md#synthetic-taproot-assets-11863). | to do (needs an approved re-run) |
| #11770 | Read the Wavelength observer after height 327067 (`~/wavelength-spike/observer-state.json` on `oa-lnd-builder`): the stopped-daemon refresh of the 20,000-sat VTXO in the 327055–327067 window, then record it in [the analysis](../payments/2026-10-15-wavelength-analysis.md#10-signet-spike-11770). | waiting on blocks (~2026-10-19) |
| #11968 | Deploy the website from `3f5f13f1b6` or newer before a desktop engine build that relies on 16 KiB assignment answers ships. Until then the engine retries at 4 KiB when the server refuses. With the dedicated test account, `openagents agents ask` a saved agent for an answer over 4 KiB and confirm it arrives whole (or, over 16 KiB, ends with the cut note). Delete the assignment and chat afterward. | Source checks passed: engine `custom_agents` 5/0, web `agent_assignments` 1/0; `docs/sdk/openapi.app.json` regenerated (`a3dc4db81a`). Deploy and live check: to do |
| #11959 | Owner: rebuild the dev app from `main` (`scripts/run-macos-dev.sh` from `~/work/openagents-app/apps/openagents`, as in [OPENAGENTS.md](../../apps/openagents/OPENAGENTS.md)) and look at a chat: the composer footer shows a small "Runs on This Mac" control whose menu lists where the chat can move (with a reason only while it can't), nothing is drawn under the input, and the sidebar's bottom-left is one account row whose menu holds Agent Lab and Sign out. | Offscreen captures passed (`examples/chrome-fixture.rs`, before/after on the issue); owner look: to do |
| #11823 | Build `openagents` and Coder from `main`. Signed in as the dedicated test account (`OPENAGENTS_TEST_RUN=1`, never the owner's account), run `openagents computer cloud capabilities`, `create --wait 300`, `exec ID -- echo ok`, a retry of `create` and `exec` with only `--key` (same computer and command, no second one), `write`/`read` of a small file, `output`, `cancel` of a `sleep 600`, `stop`, `wake`, and `delete`; then the same through Coder's `cloud_computer` tool with approvals. Confirm `openagents mcp serve` refuses `cloud create`/`exec`/`delete`, and delete every computer and `~/.openagents/computers/cloud-requests/` file the run made. | Source checks passed: `openagents-cli` cloud fake-API tests 8, `coder-new` `cloud_computer_tool` 3, MCP admission, and the bundled command tree. Dedicated-account run: to do |
| #11933 | Build the app from `main`. With the dedicated test account, a disposable repository, and Claude Code and Codex logins registered as credential sources in `<data>/moves/logins.json` (`claude_code`, `codex`, each `{ "source", "rule" }`), start three chats and use Move all from the menu-bar item: the sheet shows the estimate, and after approval each chat reaches "Running on the cloud computer" on one new computer. Follow the continued chats on the phone; the continuation is an account chat that doesn't show the cloud agent's output yet, so record what the phone shows. Quit the app mid-move, relaunch, and press Try again: still one computer, one continuation per chat. From an agent, call `plan_move` and `bring_back`: nothing happens until the sheet's approval. Bring one chat back with Stop: its work is on `openagents/cloud-<chat>`, a new local chat continues with a first message naming that branch, and the computer stops. Check that Claude Code resumed in its default permission mode can still work unattended; if it can't, open an issue. Delete the computer and chats afterward. | to do; `cargo test -p openagents-engine --lib -- move_cloud rpc::moves` passed 13/0, `cargo test -p openagents-mcp --lib` 36/0, `cargo test -p openagents-ui --lib -- move_sheet background chat_menu` 49/0, `cargo test -p openagents-rpc --lib moves` 1/0 |
| #11961, #11962, #11963 | Owner: rebuild the dev app (`scripts/run-macos-dev.sh` from `~/work/openagents-app/apps/openagents` after updating that worktree to `main`). Check that the session card has Fork chat and no New side chat, the sidebar footer has no Pull requests button, Fork chat opens a new chat at the top of Sessions with "Forked from <title>" linking back, and older side chats list as ordinary chats. | Source checks passed (see the closing comments); owner rebuild: to do |
| #11969 | Owner: rebuild the dev app from `main` (`scripts/run-macos-dev.sh` from `~/work/openagents-app/apps/openagents`, as in [OPENAGENTS.md](../../apps/openagents/OPENAGENTS.md)) so its agents get the new MCP server. Then, in a chat that is waiting on another chat, check that the working row reads "Waiting for “<chat title>”…" with a timer instead of "Working…", and that a message you send it mid-wait is answered within seconds rather than after the wait (waits now last at most 2 minutes). | mcp 38 (cap, mid-wait message in transcript and queue), doc 151, ui `tool_progress` 2 passed; headless capture passed (`examples/tool-progress-capture.rs`, [image](../../apps/openagents/docs/screenshots/tool-progress/waiting-label.png)); owner look: to do |
| #11965 | On a test host (never the owner's Mac) with the dedicated test account: save Foreman from Agent Lab, choose **Use agent**, then in Settings → General → Foreman pick its chat and leave check-ins at the 10-minute default. Confirm the schedule appears at `/settings/schedules` for that computer, a scheduled check-in starts one Foreman turn that posts a 3–6 line status even when nothing changed, a chat Foreman messaged finishing starts one coalesced turn, and each adds Wake, Outcome and Foreman's `record_decision` entries to the card's decision log, still there after an engine restart. With standing authority off, a check-in's `send_message` is refused; turn it on and the same send goes through; the grant and revocation appear in the log. Pause and delete the schedule on the website and confirm the app follows. Remove the agent, chats, schedule and host afterward. | Source checks passed: engine `foreman` 8 (including the schedule-tick and finished-chat fixture, and the 10-minute default with a status on quiet check-ins), mcp 38, proto 1, ui 1, SDK schedules 2. Live run: to do |
| #11990 | Owner: rebuild the dev app from `main` (`scripts/run-macos-dev.sh` from `~/work/openagents-app/apps/openagents` after updating that worktree, as in [OPENAGENTS.md](../../apps/openagents/OPENAGENTS.md)) and check that the Foreman chat in the sidebar and header, the empty window, and the sign-in card show the OpenAgents power mark, not Zeron's squares. The phone picks it up in TestFlight 1012: check the Sessions row and the agent picker. The website shows it after its next deploy. | Code on main; checks pending. |
| #11966 | (1) Done: the engine recheck after the rebase passed at `bd23065f92` (`custom_agents` and `foreman`, 18 tests). Done: the headless capture (`crates/ui/examples/agent-identity-fixture.rs`) shows Foreman with the OpenAgents mark and its name in the sidebar row and header, with Claude Code beside the name. Before and after images are in `apps/openagents/docs/screenshots/agent-identity/`. (2) Deploy the website from `6baf4ba340` or newer before a desktop engine that publishes `agent` on session rows ships; an older server refuses the field. (3) Phone: with the dedicated test account, open Agent Lab → an agent → change **Runs with** and add a fallback, save, and check the chat's next turn says which runtime ran; the session list shows the OpenAgents mark and the agent's name. (4) Web: the same account's chat list on openagents.com shows that session with the mark and name. | Code on main; checks pending. |
| #11971 | After the owner next updates the desktop app (never relaunch it for this check), start eight agent chats in one disposable worktree that each run `cargo check -p coder-lease`. At most the limit (5, or what `build_status {limit}` set) hold a build lease in `openagents lease list`, the rest print that they wait, `build_status` shows both counts, and `~/work/agent-targets` holds one folder per workspace. Archive the chats afterward. | Source checks passed: coder-lease 77 (agent_builds 7), openagents-mcp 43, harness `build_gate` 1, `cargo check -p openagents`; the shim took a real 1.0.0 `openagents lease build` lease in a scratch root. Live run: to do |
| #11981 | Deploy oa-computers (`deploy/oa-computers/deploy.sh staging` then `production`; it sets `OA_COMPUTERS_FREE_ALLOWANCE=1`, which applies migration 17 and records the free allowance at start) **before** the web (staging `render.py`, production `render.py` / `scripts/deploy/web.sh promote`, which set `OPENAGENTS_CLOUD_HOST_FREE_ALLOWANCE=1`). Before turning it on, give every admin and listed account that should keep the ordinary limits an explicit policy if it ever used Our cloud without an invitation (`oa-computers-usage account-limits ACCOUNT limits.json`); invited accounts are never enrolled, so this is normally empty. Then, with a fresh dedicated test account that is **not** an admin and not in `OPENAGENTS_WEB_AGENT_ACCOUNTS`: start Our cloud from the phone (or `POST /v1/cloud-host`), run one session turn on it, stop it; confirm `oa-computers-usage read ACCOUNT` shows the boot's 14,400,000 CPU-ms allowance and `free_allowance` (120,000,000 CPU ms, one computer), the computer reaches `stopped`, and `app_cloud_host_started` counted on `/admin/analytics`. Delete the computer and account fixtures afterward. | Deployed 2026-10-17: oa-computers `3d30b4ec68` (migration 17, `OA_COMPUTERS_FREE_ALLOWANCE=1`) on staging 12:33 CDT and production 13:53 CDT, then the web with `OPENAGENTS_CLOUD_HOST_FREE_ALLOWANCE=1` (13:53). Production 13:59–14:01 CDT, fresh operator-made account `acct_47f03fccea8e60d0` (not admin, not listed): start 202, computer booted and reached `stopped`, a `meter` event recorded, held 14,400,000 CPU ms, `free_allowance` 120,000,000 CPU ms / 1 computer, `app_cloud_host_started` 1 on `/admin/analytics`; computer deleted (staging the same at 13:01–13:08 with `acct_46de00901547a67f`). Not run: a session turn on the engine (the phone's coder-sync path). Known limit: at most 8 boots per free account until an operator settles boot usage (each boot holds 14,400,000 of the 120,000,000 CPU ms). Turning it off is in the workspace NEEDS_OWNER.md. |
| #11991 | (1) Staging, now: remove the leftover drain tag that keeps an old revision's worker on the relay (`gcloud run services update-traffic openagents-web-1-staging --region us-central1 --project openagentsgemini --remove-tags oa-drain-bcb559cdaf-0`; its gateway handed over the store, so it declines every chat with `internal` before the serving worker answers). (2) Deploy staging from `main` with the #11991 commit or later (`scripts/deploy/web.sh stage`): the worker's log says `door    live (openagents/chat at http://127.0.0.1:8791 ...)` with no `ai-gateway.vercel.sh` doors after it, and `worker: the gateway answers; on the relay`. (3) With throwaway visitors only: `POST https://staging.openagents.com/v1/visitors`, `POST /v1/threads` `{request_id, text}` with `Bearer ovis_…`, and a website chat from `/new` with a fresh `oa_visitor` cookie are each answered (no "We couldn't answer this time"), `job … answered … by google/gemini-4-argon` (or 3.8 Flash) on the gateway; the 11th app message refuses with `sign_in_required`. (4) During the 300 s drain of that deploy, the outgoing revision logs `worker: this instance's gateway stopped answering; leaving the relay` and a visitor chat sent then is still answered. Production (`coder` service) has no worker sidecar: its signed-out website chat was answered on 2026-10-17 by the Vertex-direct chat worker VM, and `/v1/visitors` arrives with the #11978 web deploy, answered by the same worker. | (1) done 13:31 CDT; (2) staging `openagents-web-1-staging-56932eae75-185006` since 13:50 CDT, worker `door    live (openagents/chat)` and `the gateway answers; on the relay`, smoke 99 passed 0 failed 2 skipped; (3) app and website visitors answered by `google/gemini-4-argon` at 127.0.0.1:8791, message 11 refused `sign_in_required`. (4) to do on the next staging deploy: this drain's outgoing revision (`9511ee9555`) predates the fix, kept declining with `internal` and logged no leaving-the-relay line. |
| #11993 | Deploy the account service (gateway) and then the website from the #11993 commit or later; no config change is needed (events are accepted for `https://openagents.com`, `https://api.openagents.com`, `public_origin`, and the request's own host). Then, with a throwaway key only (never the owner's account): `GET https://api.openagents.com/v1/device/nostr/challenge`, sign a kind 27235 event over `POST https://api.openagents.com/v1/device/nostr` with the `challenge` and `payload` tags, and confirm a `sess_` app session and `created: true`, a second sign-in with `created: false`, and a replay refused `409 proof_used`. Remove the throwaway account's sessions afterward. | Code on branch `agent/11993`; coordinator build pending. |
| #12013 | Upload OpenAgents iOS 1.0.0 (1013) from the wallet presentation change, wait for App Store Connect processing, and inspect the wallet on a dedicated test iPhone as recorded in [NEEDS_OWNER.md](../../NEEDS_OWNER.md). This Codex session performs the build and upload. | Wallet Rust tests 14/0; native fixture flow and large-text checks 2/0, release smoke 3/0, and 12 captures reviewed. Signed archive checks passed; OpenAgents 1.0.0 (1013) uploaded and VALID / IN_BETA_TESTING on 2026-10-17, with What to Test set. Physical-device check remains in NEEDS_OWNER.md. No new wallet operations. |
| #11994 | Deploy the account service (gateway), the Computers service (runs migration 0018), and then the website from the #11994 commit or later. No config change is needed: key-account limits use their defaults (20-bit proof of work, 3 new unattested key accounts per address, 10 per /24, 500 a day, a $20 daily breaker on unproven key accounts' free model use, App Attest for `HQWSG26L43.com.openagents.app`), and the attested free cloud tier defaults to 40,000,000 CPU ms (`OA_COMPUTERS_FREE_ATTESTED_BOOT_CPU_MS`). With throwaway keys only: a new key without `pow` gets `400 proof_required`, one with a solved `pow` signs in, and `GET /v1/account` shows `free_cloud: "none"`. The phone must send `app_attest` (`key_id`, `attestation` over `SHA-256(challenge)`) on its first sign-in; that client work and a device run belong to the phone identity issue. | Code on branch `agent/11994`; coordinator build pending. |
| #12016 | On a dedicated test account after the next iOS build, open Talk with Codex without starting a call: check account, plan, percentages, reset times, and last-updated time for the selected computer. Compare with desktop Accounts on that computer; check offline and failed refresh states retain labeled last-known usage. During a separately authorized call, confirm usage follows the call computer even if next-call preferences change. | `cargo test -p openagents-mobile --lib voice` 11/0 (6 usage tests: call host over next-call choice, late replies from an old host dropped, offline/failed refresh keep labeled last-known usage, account change clears old windows). Simulator (headless, demo data only): `VoiceFlowTests/testVoiceSettingsShowCodexUsageWithoutACall` and `testVoiceSettings` pass; voice unit tests 9/0. Device look: to do. |
| #12025 | On a dedicated test account after the next iOS build, during a separately authorized voice call: minimize the call stage while on the chat (the first page). The orb in the chat's top bar shows the live call and tapping it reopens the stage; with no call, the waveform there starts one. | Simulator (headless, demo data only): `VoiceFlowTests` 4/0, including both `testLiveStripReopens*`; `RemoteVoiceLifecycleTests` and `VoiceHostLabelTests` 9/0. Device look: to do. |
| #12027 | Owner: rebuild the dev app from `main` (`scripts/run-macos-dev.sh` from `~/work/openagents-app/apps/openagents`, as in [OPENAGENTS.md](../../apps/openagents/OPENAGENTS.md)) and open the Foreman chat: the header reads the chat title with "Foreman · custom agent · Claude Code" beside it, the composer says "Message Foreman", and the Foreman chip beside the model picker opens Agent Lab. A plain Claude Code chat looks as before. On the next TestFlight build, the phone session screen shows the same line under the title and a Foreman chip whose menu opens Agent Lab. | `cargo test -p openagents-proto saved_agent` 2/0, `openagents-ui` 1749/0, `openagents-client --lib` 30/0; desktop headless captures of the Foreman and plain chats reviewed (`agent-identity-fixture`); phone bindings regenerated and iOS simulator build-for-testing passed (no phone capture). Desktop and phone look: to do. |
| #12056 | At the next host upgrade the in-app path runs (the app records `watcher.pid`; the Foreman upgrade on hold until the owner's production deploy is done): in `~/Library/Application Support/OpenAgents Dev/data/logs/openagents-headed.log*` the `restarting into a host upgrade` line's `chat=` must match the `reopened after a host upgrade` line's `chat=` and the chat that was open; Foreman's chat and a running agent chat keep going. | Three real upgrades on 2026-10-17: 11:11:51 PM (outside path, 102afc059d), 11:25:26 PM (f33ce725f5) and 11:32:03 PM (17bc2700cf), the last two by the in-app path. All installed and logged; Foreman's chat (session 3be3ff24) and the running chats under keepers 13313, 56966 and 57430 kept going. The chat each restart reopened on wasn't logged until `7d0b98abe4`. `cargo test -p openagents-host-upgrade` 12+4+2/0, `openagents-mcp` 45/0. |
| #12138 | Build oa-computers with `--features archil` and deploy it with an `archil` deployment in `OA_COMPUTERS_REGISTRY_FILE` (`kind: "archil"`, `scope: "aws-us-east-1"`, `credential_env: "ARCHIL_API_KEY"` from the service's secrets, placement `fast-sandbox`, `rollout_accounts` limited to the dedicated test account). Boat stays the default. With the test account only: create a computer with placement `fast-sandbox`, run a command with an idempotency key twice and confirm one effect, stop and wake it, delete it, and confirm no `oa-c-*` sandbox remains in the Archil listing. | Driver conformance 10/10 live on aws-us-east-1 with no leftovers; lifecycle in `docs/research/archil/assessment.md`. Deploy: to do. |
| #12183 | On a builder with Docker running, time a cold native stage of `crates/openagents-web/Dockerfile` (`--target builder`) before and after the #12183 change and add both to `docs/audits/2026-10-19-rust-compile-times.md`. | Code on main; `cargo test -p openagents-web` 964/0 and `cad-world` 37/0 on pool host oa-pool-pb22a59-3b81. Docker timing: to do. |
| #12416 | (1) Owner: approve opening the GCE L4 Pylon on `oa-pylon-gpu-1` (a production VM) to the Grid terminal: in `/etc/systemd/system/oa-pylon.service` replace `--allow npub1mkr8…,npub1fmhl…,npub1pr3y…` with `--allow-any` (rate limits still apply; `--allow` would add the keys again on each start), then `sudo systemctl daemon-reload && sudo systemctl restart oa-pylon`. The Mac and CoderOS network-lane Pylons were restarted with `--allow-any` on 2026-10-21. (2) Staging and production need no config, secret, or key: the website defaults to `wss://relay.openagents.com` for Pylons. Deploy the website from `56a8c5ffb2` or newer, send a turn from the terminal on `/` with a throwaway visitor, and check the log for `pylon router: answered by <slug>`; a fallback line now names each Pylon's reason. (3) When the network-lane Pylons are next rebuilt from `main`, their beacons carry `max_input_bytes` and take 96 KiB jobs; until then the router sends them at most 16 KiB. | Code on main (`56a8c5ffb2`): `pylon` 157 lib and all integration tests passed, `openagents-web` `pylon_door` 7/0 (full crate 1107/2; the two failures, `/logout` route ownership and the admin connections directory, don't touch this change). Local end-to-end Pylon answer on 4341: see the issue. |
| #12537 | Foreman adopts the checked Kiln build at a safe campaign boundary and confirms `view` continues from each reported next line. Preserve both Maya jobs, saved counters, and budgets. | Done: Foreman built and verified 2b1352208f, then both drivers adopted it at safe boundaries with their saved work, counters, and limits preserved. All 20 host tests and prompt-retention checks pass. |
| #12538 | Adopt the checked stop CLI and the observer's generic stopped message. Stop a live job only when its campaign owner requests it. | Done: Foreman used the pinned `78fb33865e` CLI for the owner-directed #12503 stop; only phase and the private reason changed, with all other saved fields preserved (21 turns, 38 tool calls). The same observer build publishes the generic stopped message in the original session with fresh advancing feeds. All 22 host and 13 observer tests pass. |

## Shared rules

These add to AGENTS.md and the brief.

- **One deployer.** Only lane 0 shifts staging or production traffic, and only one release agent cuts candidates. Ask lane 0. Don't wait idle while you ask; keep working on source.
- **Disk floor.** Keep at least **30 GiB** free on the Mac. Check with `df -h ~/work` before each cold build. Below the floor, start no new build and tell lane 0. Reuse a warm target. The existing ones are `~/work/openagents-target-agent1` (70 GB), `agent3` (40 GB), `labs-sequential` (48 GB), `coderdev` (14 GB), `agent2` (10 GB), and `foreman-app` (6 GB). One target per lane, never shared by two running builds. The app workspace (`apps/openagents`, Rust 1.98.1) needs its own target.
- **At most six concurrent Cargo builds** on the Mac (a 2026-10-07 build-load crash rebooted it). Lane 10 and waiting lanes don't count.
- **Test accounts only.** Use `~/work/.secrets/openagents-test-account.env` or the per-issue test accounts named in the handoffs. Set `OPENAGENTS_TEST_RUN=1`. Until #11935 lands, unset any saved login in a scratch HOME before every CLI check, because a non-login `OPENAGENTS_TOKEN` silently falls back to it.
- **No owner-machine tests.** Scratch hosts get a temporary HOME, state, root, and tasks.
- **Label machines** you create with `python3 scripts/dev/agent-run.py labels`, and delete them before you close.
- **Push by file area.** Serialize pushes only between chats whose changed paths overlap (the app's `changed_paths` tool computes this against `origin/main`). Every other chat pushes as soon as its own tests pass; don't hold it behind another chat's rebase.
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
- [ ] #12143: rerun the 60 s native API outage cases (wake, stop, delete) against the private SPACE deployment; the driver now resends an uncertain request under its saved key.
- [ ] #12161: after the next pool deploy, start two runs on one pool host within the same second and confirm both pass the checkout fetch.
- [ ] #12162: once two or more non-overlapping code entries are queued, confirm `openagents land status` on `oa-dev-env-1` shows them landing in separate code slots at the same time (fast lane + 3 code slots since 2026-10-19 01:19 UTC).

- #12090 (D2): Android composer needs Kotlin compile/JVM tests, binding regeneration, and device checks for CJK IME, keyboard insets, rotation, hardware keyboard, camera and picker denial.
- [ ] #12097: on an Android device, run a voice call, minimize to the ongoing-call notification (mute and end actions), reopen, and check the microphone foreground service under background restrictions; regenerate UniFFI bindings and compile the Kotlin shell. Main-shell mount and listener wiring is still open.
- #12170: on a phone, check that device calls no longer arrive 15-30 s late over the relay (emulator, and a physical phone on cellular).
- #12172: build the desktop app (the sandbox lacks the `ort-sys` download, so `openagents-ui` did not compile here), open it with and without chats, and confirm it lands on the most recent chat, or on a new chat when there is none, with no Home entry in the sidebar. Then restart the owner's desktop app on that build.
- #12175: rebuild CoderOS (`nixos-rebuild switch`) so the Coder compositor from `daa5f85c24` and the Adwaita cursor theme are installed, then log out and back in to restart the session. Confirm the desktop app keeps its pointer over the model picker and survives the "Connect your phone" account switch while the mouse moves over it.
- #12179: relaunch the owner's CoderOS desktop app on a debug build of `bbe706ee9f` or later. Confirm its log says it replaced the background engine, the unit `openagents-desktop-engine-d8837cbbbbf6` runs the new program, the composer targets `coderos-4080` with no "connection recovers" line, the update notice shows no stale device, and the "From …" / Cancel row is gone.
- #12176: when CoderOS has disk room again: `scripts/dev/remote-build-cache.sh install-kache`, then `enable --key ~/work/.secrets/oa-build-cache-sa.json --local-max-size 8GiB --set-wrapper`, then `scripts/dev/check-build-cache.sh`. Then repeat the release measurement in [remote-build-cache.md](../coder/runtime/remote-build-cache.md) there (`OA_BUILD_CACHE=remote cargo build --release -p coder` in a fresh checkout and target) to confirm hits from a second machine and home path.
- #12177: on the next production promote, confirm Cloud Run takes the first web and stack images pushed by buildx (`scripts/deploy/cloud-buildx.sh`), which staging ran on `0a67be2c2d` (smoke 99 passed), and that the candidate smoke passes.
- #12178: After the next land-queue entry and the next desktop host upgrade on the Mac, confirm both built with the new dev profiles (line tables, the incremental crate list in `Cargo.toml` and `apps/openagents/Cargo.toml`) and compare their build times with [the audit](2026-10-19-rust-compile-times.md). On every other kache machine (CoderOS, new Macs), set `preserve_incremental = true`, and on Macs `cache_executables = false`, in `~/.config/kache/config.toml` ([kache.md](../background/kache.md)). Repeat #12176's debug-build measurement now that `coder`'s debug library is 346 MB instead of 670 MB.
- #12213: upgrade the desktop app on `coderos-4080` to `3e7151829f` or later so it answers `ReadWorkspaceBytes` (until then a download from that computer says it needs an update). Then, in the chat "Agent Lab Code Summary" (`ecfbcf1e`), click the "Download all nine screenshots" link and confirm `foreman-screenshots.zip` lands in `~/Downloads` and Finder shows it; open `foreman-agent.png` and confirm it renders; open a local zip and confirm Open works.
- #12209: on the next `chat work --on gce` (and `--on archil`) run that fails a check, confirm its summary line and issue comment name `gs://openagentsgemini-coder-artifacts/runs/RUN/`, that the folder holds `change.patch`, `failure.json` and `result.json`, and that `openagents chat work --resume-from RUN` on the Mac applies the diff and names the failing check in the engine's prompt (`~/.openagents/work/runs/*/briefed/system.md`).
- #12212: after the owner's desktop app runs `f32f98e431` or later, confirm Foreman's first check-in after 13:00 UTC carries the "Daily velocity" block and that Foreman repeats it in its check-in; run `openagents velocity` on the Mac and compare with the day's numbers.
- #12253, #12252, #12245: on the first pool hosts started by a CLI at `2128724abb` or later (`openagents cloud up` writes the new host agent), confirm that an idle host deletes itself about 10 minutes after its last run even while `openagents cloud status` or Foreman's `pool_status` polls it, that a run started on a host that is deleting itself says "full; trying …" instead of exit 3, and that three runs started at once spread over the hosts (`runs/*/reserved` on each).
- #12233: on the first staging image after `dcd1024cac`, confirm the `gpuiweb` stage wrote `/srv/gpui-fonts` (ten `*.ttf` with `.br` copies and `fonts.json`), that `/agent/foreman` and `/demo/gpui` draw (not the server map or "isn't available"), and that `/gpui-fonts/Geist.*.ttf` answers brotli with `cache-control: public, max-age=31536000, immutable`.
- #12240, #12250: on the next `work_issues` pool job after `d5b87ddb0f` reaches the hosts, confirm its events start with "getting the repository" (and "waiting for a free slot" when the host is full) and that its briefing line says `finder filefind (#11210)`, or names why the finder was unavailable.
- #12247: on the next briefed run that edits a file outside its issue, confirm `git restore` of that file runs in the run's worktree and that `git -C ~/openagents restore .` is still refused.
- #12215: once the owner's desktop app runs `f80a01c2a9` or later and the capacity book holds a named login (`claude setup-token | openagents capacity add-login NAME`), confirm that when an app chat (Foreman included) hits a usage limit, `openagents capacity logins` shows that login out, the chat shows "… reached its usage limit, so this chat continues on …", and the turn reruns in the same session on the next login. Confirm the agent gate refuses a subagent in a Claude Code session whose own login is out.
- #12201: once the desktop app runs `de14976391` or later, start chats with `create_chats` (or `openagents chat open` in the openagents checkout) while an always-on computer with the openagents project is online. Confirm each result's `placement` names that computer and the reason, that a batch spreads over several computers, and that `--here` keeps a chat on the Mac. Confirm `openagents chat work` without `--on` says where it runs.
- #12218: after the next web deploy, open `/demo/gpui`, go to the end of "Streaming veil on transcript rows", and confirm the approval card draws (title, step with its risk chip, the three choices) and that Deny shows "Denied". Checked locally in headless Chrome with WebGPU at `ce7ea3ae1d`.
- #12287: after the next web deploy, open the recorded visuals chat at `/demo/gpui` in Chrome, Safari, and Firefox at desktop size. Compare its latency chart and materials scene with the desktop demo; change the dashboard controls, hover the chart, and drag the 3D camera. Confirm the visuals retain their state when the transcript redraws. Use only the dedicated test account and a scratch host for any signed-in checks.
- #12243: on the first staging image after this change, confirm `/srv/agent-graph/first-frame/first-frame-capture` runs in the runtime image (`--frame 1092 738 2` prints a gzip stream), then open `/agent/foreman` at a window size other than 1440 × 900 and confirm the graph paints before the module loads (a `first-frame-fetched` performance mark) and `/agent/foreman/first-frame?...` answers from the cache on a second load.
- #12244: after the next web deploy, as the test account in Chrome, open `/gpui`, wait for the chat to draw, reload, and confirm the reload paints the kept frame before the module (`first-frame-kept` and `first-frame-painted` performance marks) and swaps on `gpui-ready` with nothing moving; sign in as another account in the same browser and confirm the first account's frame is gone from Cache Storage (`openagents-first-frame`).
- #12262: after the next web deploy, as the staging test account in Chrome, open `/gpui` and run `fetch("/v1/history")`, `fetch("/v1/computers")`, and `fetch("/v1/account")` from the console: each answers with that account's data. Then confirm a `POST /v1/agent-definitions` without the `x-openagents-csrf` header answers 403, and that the Video Lab player still saves an edit.
- #12263: deploy steps, then a check. Make a room-token key (`openssl rand -hex 32`) and keep it in Secret Manager as `openagents-web-room-token-key`. Set it as `OPENAGENTS_WEB_ROOM_TOKEN_KEY` on the staging and production web services, read the public key the site prints at startup ("Room tokens are signed with the public key …"), and add it to `CODER_SYNC_ROOM_TOKEN_KEYS` in coder-sync's `coder-sync-env` metadata. After both deploy, sign in to staging as the test account and run `fetch("/v1/sync/room-tokens",{method:"POST",headers:{"content-type":"application/json","x-openagents-csrf":document.querySelector('meta[name=openagents-csrf]').content},body:'{"room":"registry"}'})` on `/gpui`. Open the returned `url` as a WebSocket, send `{"t":"hello","device":"web-check"}`, and confirm a `state` frame lists the test account's rows. Confirm the same token is refused on `/device/ID/ws`, and that the socket closes with 4401 after five minutes.
- #12299: after the next web deploy, open `/gpui`, `/demo/gpui` (its multilingual chat), `/lander8`, and `/lander10` in Chrome, Safari, and Firefox on a Mac and a phone; confirm Geist draws (not a system font), CJK, Korean, and emoji fall back to the system fonts, and typing (including a Japanese IME) shows text in the composer.
- #12264: after #12263's key setup and a web and coder-sync deploy at `8c69dfc2a0` or later, run a scratch desktop app (temporary `HOME` and data folder, never the owner's computer) built from that commit and signed in as the staging test account. In Chrome, signed in to staging as the same account, open `/gpui`, then in the console `const m = await import(document.querySelector('link[rel=modulepreload]').href)` and `await m.computerCall(DEVICE_ID, "ListHarnesses", "{}")`. In the scratch app, open Settings, Devices, and click Allow on the browser's request. Confirm the call resolves with the harness list, and that `ListWorkspaceDirectory` with `{"path":"/"}` answers too. Click Remove and confirm the next `computerCall` fails with "access … ended". Confirm IndexedDB `openagents/keys` holds `browser-device`, and that nothing was written for the owner's account.
- #12305: after the next web deploy, in Chrome, Safari, and Firefox, drag a selection in the `/gpui` transcript and press Cmd+C (Ctrl+C on Windows and Linux); confirm the text pastes elsewhere, and that holding the drag at the top or bottom edge scrolls the transcript.
- #12267, #12268: after a web deploy at `9e793c79bb` or later, run a scratch desktop app (temporary `HOME` and data folder, never the owner's computer) built from the same commit, signed in as the staging test account, with a chat on a scratch project and the browser allowed (see #12264). In Chrome, Safari, and Firefox, signed in to staging as the same account, open `/gpui/chat/CHAT?computer=DEVICE&harness=claude`. Type `/` and `$` and confirm the list shows the computer's commands and skills, and `@ma` its files; Enter and Tab pick one, Escape closes it. Confirm `**bold**`, `*italic*`, and `` `code` `` draw as typed, a new line after `- item` continues the list, and the line under the field switches Enter to Cmd+Enter (Ctrl+Enter elsewhere) and stays after a reload. Type a draft, open another chat and come back: the draft is there. Type Japanese and Korean with the system IME and paste text. While the agent works, send three messages: the queue panel lists them and the desktop shows the same queue; move one up, edit one, Steer now another, and confirm the desktop follows. Ask the agent to use `AskUserQuestion`: the wizard replaces the composer, and the answer reaches the computer. Remove what the check made (`openagents chats delete ID`).
- #12271, #12272: after a web deploy at `630abdc688` or later, with the scratch desktop app and browser from the #12267 check, open that chat (from the sidebar, or with `?computer=DEVICE&harness=claude`) in Chrome, Safari, and Firefox and press the folder button in its header. Confirm the panel loads (`openagents_web_files_bg.*.wasm` fetched only then), browses the scratch project, shows git badges for a changed file, finds a file by name and by contents, previews a Markdown file (and its source) and a PNG, and downloads a zip with Download. Edit a text file and save with Cmd+S (Ctrl+S); on the scratch host, change it with `echo >> FILE` and confirm the open editor shows the change; change both and confirm Reload and Save mine. Turn on Autosave in the panel's Settings and confirm a pause saves. Rename a file, drag it onto a folder, delete it, and confirm the tree follows each time. Remove the scratch files and chat afterward.

- #12279: after the web and scratch desktop host include this change, use the
  dedicated test account at `/?ui=gpui` in Chrome, Safari, and Firefox. Allow
  the browser on the scratch computer, find a chat by title/project/computer,
  find a file, and open a content hit at its highlighted source line. Check
  All/Actions/Chats/Files/Content, arrows, Enter, Escape, Ctrl+Alt+Left/Right,
  and each Settings → Shortcuts binding (including opening the palette while
  the file pane has focus). Revoke browser access and confirm the next search
  fails. Use a temporary HOME/project and delete all test chats/files afterward.

- #12265, #12266: after a web deploy at `9120d26cf8` or later (staging has the room-token key and coder-sync its public key, see #12263), run a scratch desktop app (temporary `HOME` and data folder, never the owner's computer) built from that commit, signed in as the staging test account, with a scratch git project, and allow the browser (see #12264). In Chrome, signed in to staging as the same account, open `/?ui=gpui`. Confirm the sidebar lists the scratch app's chats under its computer with a green dot, and a working chat shows its dot. Pin two chats, move one up, rename one, file one in a new section, fold the section, archive one and show it with the view menu's Archived, then delete one; confirm the desktop's sidebar matches. Switch the view menu to By project, One list, Created, and Compact rows, and confirm a reload keeps the choice. Then start a new chat: pick the scratch computer, its project, a branch, New worktree, Claude Code (then again with Codex and OpenCode if installed), a model, and a reasoning level, and send a message; confirm the chat opens, streams from the computer, and appears on the desktop with that harness, and that a new chat's pickers don't show for an existing chat. Remove what the check made (`openagents chats delete ID`).
- #12282: after a web deploy at `b4f5bba754` or later and a scratch desktop app built from it (temporary `HOME` and data folder, never the owner's computer), signed in as the staging test account with cloud computers available and the browser allowed (see #12264), open `/?ui=gpui` in Chrome. On a scratch-host chat, choose Move to a cloud computer in its menu: the card shows the destination and estimate; click Move and follow the steps to Running on the cloud computer, then confirm the chat continues there. If a step stops, Try again resumes it. Reload, choose Bring back on the moved chat, and confirm it comes back with its work on a branch and the cloud computer stops. Delete the cloud computer and the chat the check made.
- #12318: with the #12265/#12266 setup (scratch desktop app built from a commit with this change, test account, browser allowed), open `/?ui=gpui`, pick the scratch computer, open the project picker and choose Add a project…. Browse into a folder, go up with `..`, search the home folder by name, and click Add this folder on a git folder: confirm it becomes the new chat's project, shows its branches, and appears in the desktop's projects. Start a session in it, then remove the project and chat the check made.
- #12225: after a web deploy at `df50eaecb5` or later, sign in to staging as the test account (allowed agent work, for Environments) and open `/?ui=gpui`; click the header's work button. Work: the landing queue line shows; enter a scratch repository's issue as `owner/repo#N`, watch its lines arrive, and Stop it. Environments: open one, watch the setup log update, send a message, open a Claude Code run, and Stop it (and, when a usage limit pauses one, choose Continue). Mac jobs (with a test Mac): open a job, watch its lines, answer its question with Deny, and Stop another. Remove what the check made.
- #12283: with the #12265/#12266 setup (scratch desktop app built from a commit with this change, test account, browser allowed), open `/?ui=gpui&settings=providers`. Confirm the scratch computer's agents list with their versions; turn one off and on and see the desktop's Providers page follow; set its update policy, click Check for updates, and see its status change live. Add a test agent account (the sign-in opens in a new tab; paste the code when asked), switch to it, then remove it, and confirm the desktop's Accounts page matches. Never sign in to the owner's agent accounts.
- #12314: on a pool host started or used by a CLI at `e0dfa1e7b0` or later, confirm `/tmp/oa-pool-filefind.log` shows the build step's index, `~/.openagents/work/briefing/filefind/indexed-*` holds the host's revision, and the first run's briefing line says `finder filefind (#11210)` within about 15 s.
- #12281 (closed by the land queue on `21b511fbc6`, without its acceptance run): signed in to the GPUI web app as the dedicated test account, open the new-session computer picker, create a cloud computer, start a session on it, stop it, and remove it at the end. Confirm the usage line reads plainly.
- #12319: after the next openagents-web deploy, confirm `curl -sI -H 'Accept-Encoding: br' https://openagents.com/everglade/everglade_web_bg.wasm` says `content-encoding: br` at about 7.5 MB, that `/everglade/space/earth_day.png` answers 200, and that `/verse` and `/lander11` draw the Grid. Open a space zone (Lagrange 1) in the browser and confirm Earth and the Moon appear once the sky files arrive. Rerun `bench/verse/2026-10-20/module-size/first_frame.py` against the deployed files for the live first frame.
- #12216: after the next web deploy, in Chrome, Safari, and Firefox on a Mac and a phone, open `/gpui` signed in as the staging test account and as a visitor: a reply with a table and code streams in the desktop's transcript (working row, fade, follows the end), Settings → Appearance's width moves the column, a reply's site cards sit under it, and a second visit paints the kept frame first. Also check `/demo/gpui`, `/lander6`, `/lander7`, `/lander8`, and `/lander10`, and that `/lander10`'s pylons stay clear of the card.
- #12345: after rebuilding the web assets for the next staging deploy, open `/home`: confirm the header paints first, then the actual Verse Grid and live pylons appear, with pylon cards on click. Check the named profile and Open chat as the dedicated test account. The classic page is `/home/classic`; `/home` has no preliminary shader (#12362).
- #12352: rebuild both the pylons layer and Everglade web assets, refresh their compressed copies, and run `scripts/dev/check-grid-home.py` on `/home`. Confirm the world anchors stay fixed while orbiting and resizing; the helper checks each visible foot against Verse's camera matrix.
- #12353: rebuild the pylons layer and Everglade (refresh compressed copies), then run `scripts/dev/check-grid-home.py --movement` on `/home` from a fresh spawn. Confirm one centered shared terminal, WASD walking, a proximity-faded message bar for all pylons, typing without movement, and draft recovery after walking away and back. Message delivery is intentionally deferred.
- #12356: Mac WebGPU passed `scripts/dev/check-grid-home-fixture.py`: the player obscures the shared computer; GPUI autofocus, typing, Escape, draft recovery, and Enter work, with one fixture POST, two visible transcript messages, and zero context losses or uncaught exceptions. After the next web deploy (matching web binary, Everglade, pylons layer, and both lander chat variants), check `/home` on staging with the dedicated test account and the existing chat backend.

- #12362: Mac real-worker guest check passed eight answers across six chats, including new chats and original-chat continuation after the old allowance; all six test chats were deleted (204/404). Linux and Mac actual WebGPU fixtures passed two replies, the Home surface header, continuous drag (8/8), click-away WASD, typing, and no preliminary shader or context loss. After the next web deploy with matching lander chat variants, rerun `scripts/dev/check-grid-home-fixture.py` and `scripts/dev/check-grid-home-guest.py` (fresh `openagents browser run` profile; deletes its test chats) against the preview. Signed-in chat remains free; sign-in is an explicit choice, never an automatic redirect.
- #12402 (W16, #12403, runs it): the cold launch of a real DeepSeek-V4.1 group. Once W11 has a stage worker in a bucket that speaks the stage-group control protocol, W12 has stage-to-stage transport between machines, and W14 has set the deadline, run `pylon cloud up four-single --dry-run` and then `pylon cloud up four-single --worker gs://…` from the one budget host. Record the launch receipt, a first answer through the router within the declared deadline, `pylon cloud down`, and the costs in the `deepseek-v41` ledger; then repeat for `two-dual`. Spending goes through `scripts/dev/cloud-budget.py` only.
- #12297: after the next web deploy, use the staging test account in Firefox or Chrome with WebGPU disabled; confirm `?ui=gpui`, the demo, and AgentGraph fetch only WebGL modules, draw text/images, accept input, open lazy panels, and recover after WebGL context loss; also check inline visuals with WebGPU enabled.
- #12406: after the next web deploy (openagents-web, Everglade, and the pylons layer rebuilt from the same commit, compressed copies refreshed), open `/home` signed out in two browsers. Confirm the overview shows the other browser's player with its `npub1…` tag, that a click glides into third person, that the wheel and `=`/`-` zoom to first person and back, that the two spawns don't overlap, and that one player can't walk through the other. Then sign in as the dedicated test account in one browser and confirm its avatar shows the profile name under the npub prefix. Run `scripts/dev/check-grid-home.py` against production and confirm the only WebSocket is `wss://relay.openagents.com`. The local three-browser receipt is `docs/verse/verification/2026-10-21-grid-home-players/`.
- #12413 (staging): deploy origin/main with `scripts/deploy/web.sh stage` (a timed-out Cloud Build doesn't count; run it again). On the staging URL, signed out: `/` draws the Grid overview with the walk hint, `/home?chat=x` answers `308` to `/?chat=x`, `/new` answers `308` to `/chat`, `GET /chat` shows the new-chat composer and sending starts `/chat/{id}`, which reloads, and `/index.md` describes the Grid and links `/chat`. One signed-out terminal turn on `/` is answered by a Pylon: the answer starts with the Pylon's name, or the web log says `answered by 30200:…`. Delete the test chat.
- #12413 (production): after staging passes, `scripts/deploy/web.sh promote DIGEST` and `shift`, then repeat the staging checks on openagents.com. Also confirm the link preview of `https://openagents.com/` shows "The Grid" (`/og/home.png?v=2`), and that the Grid's first frame still fits #12406's budget (Everglade's brotli module about 7.5 MB).
- #12415: after the next web deploy (openagents-web and the pylons layer from the same commit), open `/` signed out in a normal browser window. Press W without clicking: the camera glides in and the avatar walks. Click a pylon: its card opens, no white outline appears on the pillar or the card's close button, and WASD still walks. Click inside the card, then press WASD: the avatar still walks. Walk toward a pylon: it stays visible until it leaves the screen.

- #12419: original-weight CUDA qualification passed; [initial receipt](../psionic/verification/flint-2026-10-21/receipt.json). The later #12425 qualification covers the public context/deadline, load, and app flow and replaces the development expiry with a maintained service. Metal and MLX weights remain separate in #12422.
- #12425: completed on GCP and production: 22 full-checkpoint profile checks, a 328-second relay job, frozen-worker recovery without overlapping GPU processes, stopped-VM recovery, and a dedicated-account two-turn Grid chat (`42`, then `37`). The homepage lists **Flint on GCP** online. Both test chats were deleted (204); [results and evidence](../psionic/flint-native.md#live-pylon-qualification-2026-10-21). Source `9d34624102` serves the model/Pylon/controller; web image `4f6cd516e0` is live. Spot capacity remains a restart limitation; compute is about $53.60/day before disks and network.
- #12422: Flint's MLX INT4 checkpoint runs natively on the Mac through Psionic's Metal kernels, which adapt `rec`'s decode design. It serves through `psionic-flint serve --backend metal` and `pylon serve --text-profile flint-metal`. Kernel tests pass. Teacher-forced top-1 agrees with `rec` on 126–127 of 128 tokens. Decode measured 179/183/137 tok/s against `rec`'s recorded 125/140/109, under different load. Time to first token was 0.14/0.92/9.69 s against 0.35/1.11/8.82 s; 16K prefill matches `rec` (1,639 against 1,695 tok/s) after attention moved to Metal 4 tensor ops. Six-position INT4-vs-BF16 logits fail the declared limits at user-text positions, a property of the INT4 checkpoint; the assistant-position check is in After-close checks. A Grid terminal turn was answered by the temporary **Mac M5 Max Flint** Pylon. A permanent Mac service is in `NEEDS_OWNER.md`. [Details](../psionic/flint-metal.md).

- #12433: deploy matching web, Everglade, lander-chat, and Ice Cream Man builds. With the dedicated test accounts, walk into GAME LAB from `/`, sign in and return there, edit → play → compare, check account isolation and five concurrent edits, and return to the same Grid position. Promote after staging passes. Local Chromium checks use synthetic accounts and a scripted model; captures and results are in `docs/verse/captures/2026-10-21-game-lab/`.
- #12439: deploy matching web, Everglade, Pylons-layer, and lander-chat builds. Add a bucket lifecycle rule deleting `visits/` objects after 31 days in both analytics buckets (`docs/deployment/analytics.md`). On staging, walk `/` (join, terminal, a message, a Pylon card, the Lagrange 2 portal), then check that `/admin/analytics` shows **Grid visits** with the funnel and that `?visit=ID` shows the timeline; with `Sec-GPC: 1`, check that no `/a/v` beacon is sent. The local headless Chromium check passed: 19 events, funnel 1/1/1/1/1/1.
- #12479: after the next web deploy (openagents-web, Everglade, and the Pylons layer from the same commit), open `/` signed out several times, including after leaving the page beside the shared terminal. Each time, the player arrives away from the terminal, the help desk, and the GAME LAB arch, and W, A, S, and D walk without a click. Walking up to the terminal focuses its composer.
- #12482: after the next web deploy (openagents-web, Everglade, and the Grid Foreman host from the same commit), open `/` signed out. From the spawn, the terminal screen reads "ASK US ANYTHING" and "GAME LAB: TO YOUR LEFT" with no Pylon count or deploy line, and the billboard shows only its back. Inside the Pylon Bay, facing south, its staging and production lanes read. The Foreman's billboard stop is now off the board's east end at (47, -5).
- #12481: after the next web deploy (openagents-web and Everglade from the same commit), open `/` signed out. From the spawn, GAME LAB, LAGRANGE 2, EVERGLADE, the hosted arches, and the Gym's doorway each show their own colour and picture. Within a few meters of each, its line shows under the lintel.
- #12472: once staging or production serves livehost game tenants at `/t/TENANT/` (today only the `openagents` content tenant is followed), open the Grid with `?hosted=icecream,haunt` and repeat the local check: HAUNTED HOUSE stands beside ICE CREAM MAN, each arch prefetches and enters its own tenant, and Back to the Grid returns in front of the arch walked through. Local receipt: `docs/verse/verification/2026-10-22-haunted-house-arch/`.
- #12474: after the next web deploy (the image now builds `haunt-web` into `/srv/haunt`), open `/games/the-haunted-house` signed out on a desktop and on a phone: you arrive facing away from the house, the creak and crow come from behind about 2 s later with the bass line, and you can walk all six areas and leave by the back door (keys and mouse; the touch joystick and drag-to-look on the phone). G or the menu's Gentle softens the vignette. Once staging or production serves game tenants at `/t/haunt/`, deploy the house with `icecream-live deploy --game haunt` and repeat, and change one area's light: an open page shows it without a reload. Local receipt: `docs/verse/verification/2026-10-22-haunted-house/`.
- #12475: after the next web deploy, open `/games/the-haunted-house#library=0&sounds` and listen: each of the 36 sounds plays in turn; page through `#library=N` (0 to 9) and look at every card and mesh. Local receipt: `docs/verse/verification/2026-10-22-haunted-house-library/`.
- #12476: after the next web deploy, as a dedicated test account, open Game Lab at its arch, ask "Add a werewolf in the long hall that howls when you get near": the panel shows the house with the werewolf within a couple of seconds. A second change from the same account is refused with "You've made your one change to the house for this test." With the page open, say "turn the music down" through the phone's or the paired browser's dictation into the Game Lab chat: the music drops within 2 s. Set `OPENAGENTS_GAME_LAB_UNLIMITED` to Kiki's account id on `openagents-web` so Kiki has no limit (admins have none). Local receipt: `docs/verse/verification/2026-10-22-haunted-house-game-lab/`.
- #12486: create the tenant deploy keys `~/work/.secrets/livehost-haunt.key` and `~/work/.secrets/livehost-icecream.key` (`cargo run -p livehost -- keygen --out FILE`), run `scripts/deploy/games.sh deploy haunt` and `deploy icecream`, and set `OPENAGENTS_LIVE_GAMES=haunt=PUBKEY,icecream=PUBKEY` (from `games.sh pubkey GAME`) with `OPENAGENTS_LIVE_GAMES_CHANNEL=staging` on staging. After the web deploy, `https://staging.openagents.com/live/games` lists both, and `/t/haunt/` and `/t/icecream/` load. For production: `games.sh deploy GAME --live`, and on `openagents-web` set `OPENAGENTS_LIVE_GAMES`, `OPENAGENTS_LIVE_GAMES_RELAY=wss://relay.openagents.com`, `OPENAGENTS_LIVE_GAMES_STORE=gs://openagentsgemini-livehost`, and give its runtime account `roles/storage.objectViewer` on the bucket. Local receipt: `docs/verse/verification/2026-10-22-game-tenants-on-the-website/`.
- #12487: after #12486's check, on openagents.com walk through the HAUNTED HOUSE and ICE CREAM MAN arches: each opens full screen with the chat's Back to the Grid at the top, which returns in front of that arch.
- #12485: as the test account in desktop Chrome, open Game Lab, open the house, press the mic button, and say "turn the music down": the house says it sent the words, the chat shows them as your message, and the music drops. Browsers without the Web Speech API show no button.
