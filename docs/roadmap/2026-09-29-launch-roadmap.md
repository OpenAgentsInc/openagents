# Launch roadmap: the OpenAgents app, 2026-09-29

Written 2026-09-28. This page ties together what ships to playtesters on
**Tuesday 2026-09-29** (the MVP), its honest limits, and the staged
milestones after it, each with a target date and the issues that own it. The
[master roadmap](../roadmap.md) keeps cross-project priorities; the
[playtesting program](../game/playtesting.md) runs the launch and season 1.
Target dates here are planning targets, not release promises: a milestone
ships when its issue's acceptance passes.

Status words follow the [glossary](../glossary.md): **Done** means landed on
`main` (with the commit); **In progress** means claimed and partly landed;
**Planned** means an open issue with no landed slice.

## The launch

| | |
| --- | --- |
| Date | Tuesday 2026-09-29 |
| iOS | OpenAgents (`com.openagents.app`) 1.0.0 build 14 or later from a **public TestFlight link**; build 15 (`be94321643`) with the 2026-09-28 fixes follows after Beta App Review |
| Android | The signed OpenAgents APK from [`bins/openagents-android`](../../bins/openagents-android/README.md), linked publicly; partial parity ([below](#android-at-launch)) |
| Who | Anyone. The program is open; joining earns nothing by itself, and XP and titles come only from accepted contributions |
| Feedback | TestFlight feedback, the **Playtest report** GitHub issue template (label `playtest`), or the playtest email; in-app **Report a problem** is [#9882](https://github.com/OpenAgentsInc/openagents/issues/9882) |
| Runbook | [Day-0 launch checklist](../game/playtesting.md#day-0-launch-checklist-2026-09-29) and [season 1, week by week](../game/playtesting.md#season-1-week-by-week) |

## MVP: what ships to playtesters

### Coder: chats on your computers

- **Chats on your computers over the tailnet.** Sign in on the **Tailnet**
  screen; a computer running `coder host serve --tailnet-admission standard`
  adds itself and its Claude, Codex, and Coder chats, with no QR code.
- **New chat**: a NIP-HOST `task.create` in the computer's workspace; the
  chat follows the task's ATIF transcript through the read-only history
  observer.
- **Follow-ups, steer, queue, and answers**: each is a durable
  `task.command`; a long press on send offers the other ways to send,
  **Edit queue** edits what waits, and Coder's questions and approval
  requests are answered in the chat. Stop is `task.cancel`.
- **Full-access hosts**: the host's auto-start policy runs Coder's engine at
  once, with the access the owner granted that computer.
- **Capacity routing and usage probes**: host tasks go to a connected
  provider with capacity and fail over (`b65f6e3f93`); each route's capacity
  is recorded when a repository run starts (`4340294fd8`).

Honest limits: you need your own Mac or Linux computer on the same tailnet.
Replies arrive by polling, not token streaming; a chat keeps its newest 240
rows; long chats can load slowly over the tailnet (see
[fast chat loading](#m1-fast-chat-loading-over-the-tailnet)). No attachments,
dictation, model picker, or push notifications
([what comes later](../../bins/openagents-ios/docs/chat-later.md)).

### Verse: the Grid

- **Presence**: other players in the same world, signed with a separate
  world key, over `wss://relay.openagents.com`.
- **Tags**: name tags with a pubkey prefix (no levels yet).
- **Sticks**: a walk stick and a look stick, both thumbs at once; jump,
  pinch zoom, motion look, and recenter.
- **Physics toys with shared state**: the ball, the stack, and the
  dominoes, shared by everyone and found where they were left; the **reset
  pillar** puts them all back.
- **The Gym**: the **RESULTS** board over the published leaderboard, and the
  trace viewer with its timeline and tabs
  ([Gym leaderboard](../verse/gym-leaderboard.md)); results publications are
  signed with NIP-EVAL kind 3195 (`03cf0aa44b`,
  [#9853](https://github.com/OpenAgentsInc/openagents/issues/9853), closed).
- **Lagrange 1 portal**: the **LAGRANGE 1** arch into the station and
  **THE GRID** arch back ([Lagrange 1](../verse/lagrange-1.md)).

Honest limits: no chat in the Grid, no levels on tags, and shared state can
lag between players. The Gym results aren't shown as "signed by OpenAgents"
until the owner creates and pins a publisher key.

### Wallet on Breez Spark mainnet (iOS)

- **Receive**: Lightning invoice, Spark address, and Bitcoin deposit
  address, each with a QR code.
- **Send**: paste or scan an invoice, Lightning address, LNURL code
  (`854284a38e`), Spark or Bitcoin address, with a confirm screen showing
  the amount and fee; the scanner reads payment codes (`7c8173ebe7`).
- **Buy**: with dollars through MoonPay or Cash App
  ([#9865](https://github.com/OpenAgentsInc/openagents/issues/9865); MoonPay
  fix `4cefc1e78d`).
- **Recovery**: **Show recovery words** behind a warning, and restore from 12
  or 24 words ([#9857](https://github.com/OpenAgentsInc/openagents/issues/9857),
  iOS done in `7bc872cfed`); the trust note and a balance warning
  ([#9858](https://github.com/OpenAgentsInc/openagents/issues/9858), iOS
  done).

Honest limits: real bitcoin; use amounts you can lose. No receiving Lightning
address, no paying users by npub, and no unclaimed on-chain deposit handling. Android has the
Wallet too (`e56d173480`). See [Wallet design](../breez/wallet-design.md) and
[`INVARIANTS.md`](../../INVARIANTS.md), Phone wallet.

### Account

- **Computers**: enrollment, access, activity, order work, and the terminal.
- **Identity keys**: the device key and the Verse world key.
- **Changelog**: what changed in each build.
- **Links** and **About this device** (version and build).

### Android at launch

[#9838](https://github.com/OpenAgentsInc/openagents/issues/9838) is open.
Landed: the JNI surface and app (`098ccb1bb0`, `49f5363860`), Verse on OpenGL
ES (`85a91f63cc`), the Gym and RESULTS panels
(`82663b935d`, [#9876](https://github.com/OpenAgentsInc/openagents/issues/9876)),
Coder chats, Computers, Tailnet, About, and the Spark Wallet (`e56d173480`,
`e1aeec7413`; [#9861](https://github.com/OpenAgentsInc/openagents/issues/9861)
closed). The transcript is painted from Rust's layout (`f55db62c44`). Missing:
**Report a problem** and **Playtest session** (iOS only so far). The playtest APK is a signed
release build, 1.0.0 version code 16 (the iPhone build number), from a draft GitHub release the owner
publishes (see the Android README's Release section). Verified on the emulator only: a live tailnet chat, QR scanning, the
terminal, motion look, and Vulkan on a physical device haven't been checked.

## Done by launch (2026-09-28)

| Item | Evidence |
| --- | --- |
| Spark wallet on the phone: receive, send, buy, recovery, trust note (iOS) | `7bc872cfed`, `854284a38e`, `4cefc1e78d`, `7c8173ebe7`; epic [#9854](https://github.com/OpenAgentsInc/openagents/issues/9854) stays open for Android and the later wallet issues |
| Gym leaderboard in the Grid, signed publication over Nostr | [#9839](https://github.com/OpenAgentsInc/openagents/issues/9839) foundation; [#9853](https://github.com/OpenAgentsInc/openagents/issues/9853) closed (`03cf0aa44b`) |
| Android Gym panels | `82663b935d` ([#9876](https://github.com/OpenAgentsInc/openagents/issues/9876) still open for closure) |
| Capacity routing and failover for host tasks | `b65f6e3f93`, `4340294fd8` |
| Faster chat reads on the host | `4ef967aa40` |
| Rust-laid-out transcript, stage 2 and CoreText painting | `21b8774ec3`, `386c106598`, `3943b45f62` ([#9833](https://github.com/OpenAgentsInc/openagents/issues/9833)) |
| BIP 177 amount formatter (shared, not yet on screen) | `8d89ddf293` ([#9881](https://github.com/OpenAgentsInc/openagents/issues/9881)) |
| Playtesting plan | [playtesting.md](../game/playtesting.md), `926564d073`, epic [#9888](https://github.com/OpenAgentsInc/openagents/issues/9888) |
| iOS build 15 | `be94321643` |

## Milestones after launch

| Milestone | Target | Status | Owning issues |
| --- | --- | --- | --- |
| [M1. Fast chat loading over the tailnet](#m1-fast-chat-loading-over-the-tailnet) | 2026-10-02 | In progress | [#9833](https://github.com/OpenAgentsInc/openagents/issues/9833) |
| [M2. Playtest feedback in the app](#m2-playtest-feedback-in-the-app) | 2026-10-09 | Planned | [#9882](https://github.com/OpenAgentsInc/openagents/issues/9882), [#9883](https://github.com/OpenAgentsInc/openagents/issues/9883), [#9884](https://github.com/OpenAgentsInc/openagents/issues/9884), [#9887](https://github.com/OpenAgentsInc/openagents/issues/9887) |
| [M3. Android parity](#m3-android-parity) | 2026-10-12 | In progress | [#9838](https://github.com/OpenAgentsInc/openagents/issues/9838), [#9861](https://github.com/OpenAgentsInc/openagents/issues/9861), [#9876](https://github.com/OpenAgentsInc/openagents/issues/9876), [#9857](https://github.com/OpenAgentsInc/openagents/issues/9857), [#9858](https://github.com/OpenAgentsInc/openagents/issues/9858) |
| [M4. Wallet: addresses, paying users, deposits, BIP 177](#m4-wallet-addresses-paying-users-deposits-bip-177) | 2026-10-16 | Planned (BIP 177 in progress) | [#9859](https://github.com/OpenAgentsInc/openagents/issues/9859), [#9860](https://github.com/OpenAgentsInc/openagents/issues/9860), [#9862](https://github.com/OpenAgentsInc/openagents/issues/9862), [#9881](https://github.com/OpenAgentsInc/openagents/issues/9881) |
| [M5. Gym on the web](#m5-gym-on-the-web) | None | Postponed | [#9852](https://github.com/OpenAgentsInc/openagents/issues/9852) |
| [M6. Trainer leveling phase 1 and playtest rewards](#m6-trainer-leveling-phase-1-and-playtest-rewards) | 2026-10-26 (end of season 1) | In progress | [#9847](https://github.com/OpenAgentsInc/openagents/issues/9847), [#9885](https://github.com/OpenAgentsInc/openagents/issues/9885), [#9886](https://github.com/OpenAgentsInc/openagents/issues/9886), [#9887](https://github.com/OpenAgentsInc/openagents/issues/9887), epic [#9888](https://github.com/OpenAgentsInc/openagents/issues/9888) |
| [M7. Agent spending phase 1](#m7-agent-spending-phase-1) | 2026-10-30 | Planned | [#9863](https://github.com/OpenAgentsInc/openagents/issues/9863) |
| [M8. x402 receive on mainnet](#m8-x402-receive-on-mainnet) | 2026-10-30 | Planned | [#9832](https://github.com/OpenAgentsInc/openagents/issues/9832) |
| [M9. Agent spending, later phases](#m9-agent-spending-later-phases) | After M7, not dated | Planned | [#9864](https://github.com/OpenAgentsInc/openagents/issues/9864) |
| CoderOS (owned by another agent) | Not scheduled here | See [CoderOS](../os/README.md) | Reference only |

### M1. Fast chat loading over the tailnet

Opening a long chat on the phone should show its newest rows at once, and
scrolling and following a running chat should stay smooth. Landed so far:
host reads in milliseconds (`4ef967aa40`), transcript rows pulled straight
from Rust (`5d42b68455`), Rust layout with exact row heights (`386c106598`),
and CoreText painting of Rust's layout (`3943b45f62`). Remaining in
[#9833](https://github.com/OpenAgentsInc/openagents/issues/9833): finish the
native painting path and measure open time on a real tailnet. Ships in the
first post-launch build.

### M2. Playtest feedback in the app

**Report a problem** in Account and from a long press on the tab bar,
build-stamped and sent privately
([#9882](https://github.com/OpenAgentsInc/openagents/issues/9882)); the
opt-in local session log with its `INVARIANTS.md` rows
([#9883](https://github.com/OpenAgentsInc/openagents/issues/9883)); the
triage inbox that drafts `playtest` issues and keeps the triage log
([#9884](https://github.com/OpenAgentsInc/openagents/issues/9884)); and a
"What to test" line in each build's Changelog
([#9887](https://github.com/OpenAgentsInc/openagents/issues/9887)). Until
then, the launch's three channels apply.

### M3. Android parity

The Android Wallet screen with a Keystore-wrapped Spark seed
([#9861](https://github.com/OpenAgentsInc/openagents/issues/9861)), then
recovery and the trust note on Android
([#9857](https://github.com/OpenAgentsInc/openagents/issues/9857),
[#9858](https://github.com/OpenAgentsInc/openagents/issues/9858)); close
[#9876](https://github.com/OpenAgentsInc/openagents/issues/9876); check a
live tailnet chat, QR scanning, the terminal, motion look, and Vulkan on
physical devices; then close
[#9838](https://github.com/OpenAgentsInc/openagents/issues/9838). A Play
Store testing track is an owner decision after this milestone.

### M4. Wallet: addresses, paying users, deposits, BIP 177

A receiving Lightning address and LNURL
([#9859](https://github.com/OpenAgentsInc/openagents/issues/9859)); paying
other OpenAgents users by npub, Lightning address, or QR
([#9860](https://github.com/OpenAgentsInc/openagents/issues/9860));
unclaimed on-chain deposits and the unilateral-exit backup
([#9862](https://github.com/OpenAgentsInc/openagents/issues/9862)); and BIP 177
amounts as integer base units with a legacy BTC toggle
([#9881](https://github.com/OpenAgentsInc/openagents/issues/9881); the shared
formatter landed in `8d89ddf293`).

### M5. Gym on the web

Postponed (owner, 2026-09-28): the Gym stays in Verse only for now, and web
work is on hold. A `/gym` page on openagents.com over the same signed
leaderboard publication the Grid reads
([#9852](https://github.com/OpenAgentsInc/openagents/issues/9852)) is built in
the coder repository but stays undeployed. The Nostr publication itself is done
([#9853](https://github.com/OpenAgentsInc/openagents/issues/9853)).

### M6. Trainer leveling phase 1 and playtest rewards

Phase 1 of [agent trainer leveling](../verse/agent-trainer-leveling.md): the
read-only XP reader and trainer card in the app, and the first trainer who
levels up in the Grid ([#9847](https://github.com/OpenAgentsInc/openagents/issues/9847)).
On top of it, the playtest rewards: the NIP-XP `playtest` rule and referee
key ([#9885](https://github.com/OpenAgentsInc/openagents/issues/9885)),
titles and cosmetics on name tags
([#9886](https://github.com/OpenAgentsInc/openagents/issues/9886)), and the
playtest card in Account
([#9887](https://github.com/OpenAgentsInc/openagents/issues/9887)). Week 3's
paper-prototype sessions can change the phase 1 design before it ships. The
first signed playtest awards are season 1's
[first milestone](../game/playtesting.md#success-metrics-and-the-first-milestone);
they cover accepted contributions only, never joining.

### M7. Agent spending phase 1

An agent asks to spend, and the owner approves each payment on the phone
([#9863](https://github.com/OpenAgentsInc/openagents/issues/9863)). Depends on
M4's wallet surfaces being stable in playtest.

### M8. x402 receive on mainnet

Verify x402 receive on mainnet through the MoneyDevKit LSPS4 channel
([#9832](https://github.com/OpenAgentsInc/openagents/issues/9832)).

### M9. Agent spending, later phases

Allowance wallets, standing grants, and operator allowances
([#9864](https://github.com/OpenAgentsInc/openagents/issues/9864)), after
phase 1 has run in playtest with no lost funds.

## Maintaining this page

When a milestone's issue closes, mark it **Done** with the commit, and move
its limits out of the MVP section. When a target slips, change the date and
say why in the commit message. The playtesting program's weekly note links
here for what each build is meant to test.
