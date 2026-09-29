# Launch roadmap: the OpenAgents app, 2026-09-29

Written 2026-09-28, updated the evening of 2026-09-28. This page ties together what ships to playtesters on
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
| iOS | OpenAgents (`com.openagents.app`) 1.0.0 build 16 (`e7989aa4e1`) from a **public TestFlight link** |
| Android | The signed OpenAgents APK 1.0.0 (16) from [`bins/openagents-android`](../../bins/openagents-android/README.md), from a draft GitHub release the owner publishes ([below](#android-at-launch)) |
| Who | Anyone. The program is open; joining earns nothing by itself, and XP and titles come only from accepted contributions |
| Feedback | In-app **Report a problem** (Account, or a long press on the tab bar), TestFlight feedback, the **Playtest report** GitHub issue template (label `playtest`), or the playtest email. In-app reports stay on the phone until the owner creates the triage key |
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

- **Fast chat loading**: chat reads go directly over the tailnet with the
  same sealed requests (relay as fallback), in parallel and in large pages;
  the chat list loads in well under a second and chats reopen instantly from
  the phone's cache ([M1](#m1-fast-chat-loading-over-the-tailnet)).

Honest limits: you need your own Mac or Linux computer on the same tailnet.
Replies update per step as the engine records them, not token by token. No attachments,
dictation, model picker, or push notifications
([what comes later](../../bins/openagents-ios/docs/chat-later.md)).

### Verse: the Grid

- **Presence**: other players in the same world, signed with a separate
  world key, over `wss://relay.openagents.com`.
- **Tags**: name tags with a pubkey prefix and trainer level (`c25458d5 · lv 3`), your own included.
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
- **Trainer levels and tutorial quests**: six live tutorial quests reward
  reproducing a published pass ([tutorial quests](../verse/tutorial-quests.md)).
- **Lagrange 1 portal**: the **LAGRANGE 1** arch into the station and
  **THE GRID** arch back ([Lagrange 1](../verse/lagrange-1.md)).

Honest limits: no chat in the Grid, and shared state can
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

- **Paying people and deposits**: pay an npub (their published Spark address
  or profile Lightning address), unclaimed deposits with claim and refund,
  and the exit backup (`f0466f1b15`, `d43b304e87`).
- **Amounts in BIP 177 form** (`₿12,345`) with a legacy BTC toggle
  (`dfe066f267`).
- **Agent payments**: an agent on your computer asks to pay; you approve each
  payment on the phone (`c26e37c366`).

Honest limits: real bitcoin; use amounts you can lose. No receiving Lightning
address of your own yet. Android has the Wallet too (`e56d173480`, `e1aeec7413`). See [Wallet design](../breez/wallet-design.md) and
[`INVARIANTS.md`](../../INVARIANTS.md), Phone wallet.

### Account

- **Computers**: enrollment, access, activity, order work, and the terminal.
- **Identity keys**: the device key and the Verse world key.
- **Changelog**: what changed in each build.
- **Links** and **About this device** (version and build).
- **Trainer** card and **Playtest** card, **Report a problem**, **My reports**,
  and **Playtest logging** (on for everyone; a build switch turns it off
  for a release).

### Android at launch

[#9838](https://github.com/OpenAgentsInc/openagents/issues/9838) is closed as
code-complete: Coder chats, Computers, Tailnet, Account, the Grid with the
Gym and RESULTS panels, the Spark Wallet with recovery and the trust note,
trainer levels, Report a problem and the playtest card (`738d1b5248`), and a
transcript painted from Rust's layout with in-place Select Text
(`f55db62c44`, `6722771d93`). The APK is a signed release build, 1.0.0
version code 16. Verified on the emulator only; physical-device checks are
an owner step.

## Done by launch (2026-09-28)

| Item | Evidence |
| --- | --- |
| Spark wallet: receive, send, buy, recovery, trust note, paying people, deposits, BIP 177 | `7bc872cfed`, `854284a38e`, `4cefc1e78d`, `7c8173ebe7`, `f0466f1b15`, `d43b304e87`, `dfe066f267` |
| Agent spending phase 1 | `c26e37c366`, `a0cbbffb68` ([#9863](https://github.com/OpenAgentsInc/openagents/issues/9863), closed) |
| Gym leaderboard in the Grid, signed publication over Nostr | [#9839](https://github.com/OpenAgentsInc/openagents/issues/9839), [#9853](https://github.com/OpenAgentsInc/openagents/issues/9853) closed |
| Capacity routing, usage probes, full-access hosts | `b65f6e3f93`, `4340294fd8`, `acee225991`, `718727d1c0` |
| Fast chat loading over the tailnet | `4ef967aa40`, `2a7c7fae11`, `e1c4fe40b9`, `a1ea5ea9cc` |
| Playtest feedback, triage, and rewards | `d3033ae898`, `74f2f90be0`, `253ca31885`, `c038e1d25b`, `38c2a7f9f0`, `a4aa3013de`, `ecf6cabd3b`, `c9850f5c73`, `cf9d36741b`, `f8b773578a` |
| Trainer leveling phase 1 | `9bb3738bd8`, `b860b3e88a`, `ce3c76257e` |
| Android parity and release APK | `e56d173480`, `e1aeec7413`, `738d1b5248`, `762fb8d36d` |
| iOS build 16 | `e7989aa4e1` |

## Milestones after launch

The owner is taking M1, M2, M3, and M6 through their owner steps tonight,
2026-09-28 (the device, key, and release steps in the workspace
`NEEDS_OWNER.md`). Their code is on `main`; issues close when code-complete
and owner steps never hold them open.

| Milestone | Target | Status | Owning issues |
| --- | --- | --- | --- |
| [M1. Fast chat loading over the tailnet](#m1-fast-chat-loading-over-the-tailnet) | 2026-09-28 (tonight) | Code done; owner verifying | [#9833](https://github.com/OpenAgentsInc/openagents/issues/9833), [#9910](https://github.com/OpenAgentsInc/openagents/issues/9910) |
| [M2. Playtest feedback in the app](#m2-playtest-feedback-in-the-app) | 2026-09-28 (tonight) | Done | #9882, #9883, #9884, #9887, #9903–#9906 (closed) |
| [M3. Android parity](#m3-android-parity) | 2026-09-28 (tonight) | Done | #9838, #9861, #9876, #9857, #9858 (closed); [#9910](https://github.com/OpenAgentsInc/openagents/issues/9910) |
| [M4. Wallet: addresses, paying users, deposits, BIP 177](#m4-wallet-addresses-paying-users-deposits-bip-177) | 2026-09-28 | Done | #9859, #9860, #9862, #9881 (closed) |
| [M5. Gym on the web](#m5-gym-on-the-web) | None | Postponed | #9852 (closed, not planned) |
| [M6. Trainer leveling phase 1 and playtest rewards](#m6-trainer-leveling-phase-1-and-playtest-rewards) | 2026-09-28 (tonight) | Phase 1 done; follow-ups in progress | #9847, #9885, #9886 (closed); [#9896](https://github.com/OpenAgentsInc/openagents/issues/9896), [#9898](https://github.com/OpenAgentsInc/openagents/issues/9898) |
| [M7. Agent spending phase 1](#m7-agent-spending-phase-1) | 2026-09-28 | Done | #9863 (closed) |
| [M8. x402 receive on mainnet](#m8-x402-receive-on-mainnet) | None | Not planned for now | #9832 (closed) |
| [M9. Agent spending, later phases](#m9-agent-spending-later-phases) | None | Deferred (won't do for now) | #9864, #9911, #9912, #9913 (closed, not planned) |
| Microcoder replaces Microluna | Not dated | In progress | [#9878](https://github.com/OpenAgentsInc/openagents/issues/9878), [#9880](https://github.com/OpenAgentsInc/openagents/issues/9880), [#9889](https://github.com/OpenAgentsInc/openagents/issues/9889), [#9890](https://github.com/OpenAgentsInc/openagents/issues/9890) |
| Host store fixes | Not dated | In progress | [#9908](https://github.com/OpenAgentsInc/openagents/issues/9908), [#9909](https://github.com/OpenAgentsInc/openagents/issues/9909) |
| CoderOS (owned by another agent) | Not scheduled here | See [CoderOS](../os/README.md) | Reference only |

### M1. Fast chat loading over the tailnet

Code done. Chat reads go over the tailnet with the same sealed requests,
several at once and in large pages, with the relay as fallback; the host
reuses its book and caches session heads (`4ef967aa40`, `2a7c7fae11`,
`e1c4fe40b9`, `a1ea5ea9cc`). Measured on coderos-4080: chat list 6–7 s →
0.2–0.35 s, uncached open 9.5–11 s → 0.02–0.25 s; reopen is instant. The
transcript's rows come straight from Rust (`5d42b68455`), with Rust shaping
(`dfc4bd02e4`) and Android painting and selection (`f55db62c44`,
`6722771d93`). Owner step tonight: verify on the phone, and lend a phone for
the device benchmarks. The Android streaming fade is
[#9910](https://github.com/OpenAgentsInc/openagents/issues/9910).

### M2. Playtest feedback in the app

Done. **Report a problem** in Account and from a long press on the tab bar,
build-stamped and sealed privately to the triage key; **My reports**; the
playtest log (on by default); the public content-free report record (NIP-XP kind 3197);
the triage inbox with TestFlight feedback; and a "What to test" line per
build (#9882–#9884, #9887, #9903–#9906). Owner step tonight: create the
triage key so reports leave the phone.

### M3. Android parity

Done: the Android app matches iOS for launch (#9838 closed) and ships as a
signed APK. Owner steps tonight: back up the release key, publish the draft
GitHub release, and check it on a physical phone. A Play Store testing track
is an owner decision after this.

### M4. Wallet: addresses, paying users, deposits, BIP 177

Done: Lightning address and LNURL sending, paying users by npub, deposits and
the exit backup, and BIP 177 amounts (#9859, #9860, #9862, #9881). A
receiving Lightning address of our own needs its own LNURL server and issue.

### M5. Gym on the web

Postponed (owner, 2026-09-28): the Gym stays in Verse only for now, and web
work is on hold. A `/gym` page on openagents.com over the same signed
leaderboard publication the Grid reads
([#9852](https://github.com/OpenAgentsInc/openagents/issues/9852)) is built in
the coder repository but stays undeployed. The Nostr publication itself is done
([#9853](https://github.com/OpenAgentsInc/openagents/issues/9853)).

### M6. Trainer leveling phase 1 and playtest rewards

Phase 1 done: the NIP-XP `reproduce` rule, the XP reader, level tags in the
Grid, the Trainer card, and six live tutorial quests (#9847); the NIP-XP
`playtest` rule, titles on name tags, and the playtest card (#9885–#9887);
signing awards from triage decisions (#9906). Owner step tonight: create the
playtest referee key so awards can be signed. Follow-ups: key links
([#9896](https://github.com/OpenAgentsInc/openagents/issues/9896)) and trainer
card export ([#9898](https://github.com/OpenAgentsInc/openagents/issues/9898)).
Awards cover accepted contributions only, never joining.

### M7. Agent spending phase 1

Done: an agent asks to spend, the owner approves each payment on the phone,
and nothing pays without the tap (#9863).

### M8. x402 receive on mainnet

Not planned for now (#9832 closed): it needs a funded MoneyDevKit LSPS4
channel and the owner's go-ahead.

### M9. Agent spending, later phases

Deferred by the owner (2026-09-28): #9864, #9911, #9912, and #9913 are closed
as won't do for now. Two slices had already landed and stay on `main`: phone
wakes for spend requests with spend ops over CAP/CJ (`cf43ac7dff`), and
standing grants that pay a payee without a tap only after the owner chooses
**Approve and trust this payee** (`1d5d9cd7bf`; opt-in, off until then).

## Maintaining this page

When a milestone's issue closes, mark it **Done** with the commit, and move
its limits out of the MVP section. When a target slips, change the date and
say why in the commit message. The playtesting program's weekly note links
here for what each build is meant to test.
