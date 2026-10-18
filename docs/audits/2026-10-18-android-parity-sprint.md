# Android parity sprint

Status: implementation started, 2026-10-18. The original planning pass
changed documentation and GitHub issues only. CoderOS registration is now
complete, and one Codex app chat is working on A0 (#12079).

The [October 17 Android audit](2026-10-17-android-app.md) supplies the initial
assessment. This plan updates its dependencies and acceptance scope against
`main` at `f4d84c49ce864e748289772be1029e8b7b382e7e`. It does not treat the
audit's recommendations as owner decisions. Planning is tracked in #12076;
the implementation epic is
[#12077](https://github.com/OpenAgentsInc/openagents/issues/12077) on project 22.

## Goal and boundaries

Build the Android app at `apps/openagents/apps/android` over the same
`openagents-mobile` UniFFI core as the current iOS app. Match the current
mobile experience, including voice, recovery, wallet, accessibility, and
adaptive navigation. Tablet split navigation has its own issue so that it
cannot disappear behind a phone-only beta. A smaller beta is a milestone,
not completion of this sprint.

- Product identity comes from `apps/openagents/crates/brand`;
  `com.openagents.app` and existing signing continuity matter for upgrades.
- Rust owns application state, domain logic, permissions, networking, sync,
  identity, wallet, Labs, and agent behavior. Kotlin supplies Android
  presentation and platform services. Do not add TypeScript.
- Rust Native remains the shared card, form, and sheet layer. Read its
  [specification](../../crates/rust-native/docs/spec.md) and
  [architecture](../coder/rust-native/architecture.md) before adding the
  Android adapter. Do not recreate cards separately in Kotlin.
- Reuse root domain crates and the existing SDK. Use the existing Loro rooms
  on `coder-sync` and NIP-HOST grants; do not create another session path.
- Retired Android code is reference material for platform glue and seed
  migration. Do not revive the removed app or add its old Verse/Gym screens
  to this app's scope. Desktop editors and on-phone agent execution are not
  mobile parity requirements.
- Keep the Zeron/Wing MIT notices and About attribution. Record later
  implementation integrations in `apps/openagents/docs/downstream-patches.md`.

## What changed since the audit

Identity, first-launch onboarding, wallet, pairing, website approval, and
account deletion have landed. The audit's `agent/11995`, `agent/11999`, and
`agent/12000` references describe its historical snapshot, not dependencies
waiting to merge. Use the current store traits and shared Rust views.
See the [identity close timeline](2026-10-17-nostr-identity-close-timeline.md),
[account contract](../auth/README.md), and
[downstream change record](../../apps/openagents/docs/downstream-patches.md).

The current wallet uses the focused Rust Native views from #12013. Voice
includes saved voice names, stable active-computer names, and usage for the
computer executing the call (#12014, #12016, #12017). Account deletion
(#12002), website sign-in approval (#12001), and the live computer model
catalog (#12020) need explicit Android acceptance.

The Coder binary freeze is lifted by the
[binding release brief](2026-10-16-coder-1.0-big-bang.md). The old freeze is
not a risk or reason to create a workaround. This Android sprint does not
authorize a release, deploy, or traffic shift.

## Parity matrix

Each row needs an offline journey and links to its candidate-build evidence
before the final parity review. Current shared behavior is the reference;
pending Foreman or voice features do not become implemented merely because
they have issues. Record any change to the baseline before assigning more
work.

| Surface | Required behavior | Work |
| --- | --- | --- |
| Start and identity | Useful first chat without a sign-in wall; proof of work, tutorial once, existing-account linking, Your ID, backup/confirmation/restore, protected secret screens | A3, D3, E1 |
| Chats and navigation | Unified live/saved/web/terminal lists, search, pin/reorder/archive, cross-surface continuation, offline/reconnect, stable selected chat | D1, D3 |
| Transcript | Rust row positions and wrapping; rich markdown/tools/code/tables/images; selection/copy; streaming and scroll anchors; accessible older history | C1, C2, Q1 |
| Session controls | Send/queue/edit/remove/steer/stop, questions and approvals, drafts, mentions, attachments, native IME and keyboard transitions | D2, D4 |
| Projects and computers | Project creation/browser/branches, shared-project invitations and chats, current model/effort catalogs, Run on and computer handoff, cloud chat and cloud computer states | D3, D5, E3 |
| Agents and Home | Custom-agent create/edit/save/run/relaunch/assign, Agent Lab inspect/use, Foreman as supported at the baseline, chat homes, dashboards/widgets | D4, G1 |
| Visuals and Labs | Rust visual pixels/input/lifecycle; current video, comparison, 3D, and report cards through shared Rust Native views | C3, D4 |
| Settings and account | Devices/nicknames, appearance/wallpaper, archive, cloud controls, sharing, notification preferences, About, deletion, paid-credit confirmation, and shared-team refusal/recovery | D5 |
| Wallet and upgrades | Balance, receive, send, review/cancel/failure, activity, information, shared recovery words; exact retired seed retained through signed in-place upgrade | A3, E1, E2, G2 |
| Scanner and links | Computer pairing/revocation, account linking, website sign-in approval, project invitations, bounded Rust validation, verified App Links | A3, E3, G2 |
| Notifications | Push/nopush builds, token rotation, channels/permission, foreground quiet, cold/warm tap to correct session | F1 |
| Voice | Media/audio routes, interruptions, saved voice, active computer/account/usage, stage/orb/captions, minimize/reopen, background microphone service and mute/end notification | F2, F3 |
| Android access and lifecycle | TalkBack semantics/focus, font/display scaling, touch targets, contrast, reduced motion, back, permissions, activity/process recreation, no lost identity/grants/drafts | Q1 |
| Adaptive navigation | Compact phones and expanded tablet/split navigation, width changes with the same chat/draft/anchor | Q2 |
| Build and distribution | Both ABIs, reproducible local builds, safe test extras, signature/version continuity, 16 KiB alignment, APK/AAB and internal-testing handoff | A0–B1, G2 |

## Issue list

All entries are real GitHub issues on project 22 under
[#12077](https://github.com/OpenAgentsInc/openagents/issues/12077).
Dependencies are recorded as native GitHub blockers and in the issue bodies.
Only A0 is claimed. `G1` joins the entire required graph;
it cannot pass after only the first composer works.

| Step | Issue | Estimate | Depends on |
| --- | --- | --- | --- |
| R0 | [#12078: Computers: register CoderOS as a native account target](https://github.com/OpenAgentsInc/openagents/issues/12078) | Separate setup | — |
| A0 | [#12079: Android: prepare the CoderOS build and emulator environment](https://github.com/OpenAgentsInc/openagents/issues/12079) | 0.5–1 day | — |
| A1 | [#12080: Android: build the shared Rust core and Kotlin bindings](https://github.com/OpenAgentsInc/openagents/issues/12080) | 1–2 days | [A0](https://github.com/OpenAgentsInc/openagents/issues/12079) |
| A2 | [#12081: Android: initialize the JVM context and certificate verifier](https://github.com/OpenAgentsInc/openagents/issues/12081) | 0.5 day | [A1](https://github.com/OpenAgentsInc/openagents/issues/12080) |
| A3 | [#12082: Android: persist account identity wallet and paired grants securely](https://github.com/OpenAgentsInc/openagents/issues/12082) | 1–1.5 days | [A1](https://github.com/OpenAgentsInc/openagents/issues/12080) |
| B1 | [#12083: Android: add the Gradle app shell and offline demo](https://github.com/OpenAgentsInc/openagents/issues/12083) | 1–1.5 days | [A1](https://github.com/OpenAgentsInc/openagents/issues/12080), [A2](https://github.com/OpenAgentsInc/openagents/issues/12081) |
| C1 | [#12084: Android: render the Rust transcript layout precisely](https://github.com/OpenAgentsInc/openagents/issues/12084) | 3–4 days | [B1](https://github.com/OpenAgentsInc/openagents/issues/12083) |
| C2 | [#12085: Android: add rich transcript interactions and streaming polish](https://github.com/OpenAgentsInc/openagents/issues/12085) | 1.5–2 days | [C1](https://github.com/OpenAgentsInc/openagents/issues/12084) |
| C3 | [#12089: Android: render agent visuals and shared Lab cards](https://github.com/OpenAgentsInc/openagents/issues/12089) | 1–1.5 days | [C1](https://github.com/OpenAgentsInc/openagents/issues/12084), [D4](https://github.com/OpenAgentsInc/openagents/issues/12088) |
| D1 | [#12087: Android: add the drawer session history and search](https://github.com/OpenAgentsInc/openagents/issues/12087) | 2–3 days | [B1](https://github.com/OpenAgentsInc/openagents/issues/12083) |
| D2 | [#12090: Android: add the composer questions attachments and keyboard flow](https://github.com/OpenAgentsInc/openagents/issues/12090) | 2.5–3.5 days | [C1](https://github.com/OpenAgentsInc/openagents/issues/12084) |
| D3 | [#12095: Android: add new chats projects cloud and computer handoff](https://github.com/OpenAgentsInc/openagents/issues/12095) | 1.5–2 days | [A3](https://github.com/OpenAgentsInc/openagents/issues/12082), [D1](https://github.com/OpenAgentsInc/openagents/issues/12087), [D2](https://github.com/OpenAgentsInc/openagents/issues/12090) |
| D4 | [#12088: Android: render Rust Native forms cards sheets and dashboards](https://github.com/OpenAgentsInc/openagents/issues/12088) | 1 day | [B1](https://github.com/OpenAgentsInc/openagents/issues/12083) |
| D5 | [#12091: Android: add settings sharing and account deletion](https://github.com/OpenAgentsInc/openagents/issues/12091) | 1.5–2 days | [A3](https://github.com/OpenAgentsInc/openagents/issues/12082), [D1](https://github.com/OpenAgentsInc/openagents/issues/12087), [D4](https://github.com/OpenAgentsInc/openagents/issues/12088) |
| E1 | [#12099: Android: add first launch identity backup and restore](https://github.com/OpenAgentsInc/openagents/issues/12099) | 1–1.5 days | [A3](https://github.com/OpenAgentsInc/openagents/issues/12082), [D3](https://github.com/OpenAgentsInc/openagents/issues/12095), [D5](https://github.com/OpenAgentsInc/openagents/issues/12091) |
| E2 | [#12092: Android: add the current wallet and safe retired-app upgrades](https://github.com/OpenAgentsInc/openagents/issues/12092) | 1.5–2 days | [A3](https://github.com/OpenAgentsInc/openagents/issues/12082), [D4](https://github.com/OpenAgentsInc/openagents/issues/12088) |
| E3 | [#12096: Android: add QR pairing account approval and verified links](https://github.com/OpenAgentsInc/openagents/issues/12096) | 1 day | [A3](https://github.com/OpenAgentsInc/openagents/issues/12082), [D5](https://github.com/OpenAgentsInc/openagents/issues/12091) |
| F1 | [#12093: Android: connect FCM notifications to the shared push client](https://github.com/OpenAgentsInc/openagents/issues/12093) | 1–1.5 days | [A3](https://github.com/OpenAgentsInc/openagents/issues/12082), [D1](https://github.com/OpenAgentsInc/openagents/issues/12087) |
| F2 | [#12094: Android: implement voice media and audio routing](https://github.com/OpenAgentsInc/openagents/issues/12094) | 3–4 days | [A2](https://github.com/OpenAgentsInc/openagents/issues/12081), [A3](https://github.com/OpenAgentsInc/openagents/issues/12082), [D1](https://github.com/OpenAgentsInc/openagents/issues/12087) |
| F3 | [#12097: Android: add voice screens and background call controls](https://github.com/OpenAgentsInc/openagents/issues/12097) | 1–1.5 days | [D2](https://github.com/OpenAgentsInc/openagents/issues/12090), [D5](https://github.com/OpenAgentsInc/openagents/issues/12091), [F2](https://github.com/OpenAgentsInc/openagents/issues/12094) |
| Q1 | [#12102: Android: verify accessibility and lifecycle recovery](https://github.com/OpenAgentsInc/openagents/issues/12102) | 1–2 days | [C2](https://github.com/OpenAgentsInc/openagents/issues/12085), [C3](https://github.com/OpenAgentsInc/openagents/issues/12089), [D3](https://github.com/OpenAgentsInc/openagents/issues/12095), [D4](https://github.com/OpenAgentsInc/openagents/issues/12088), [D5](https://github.com/OpenAgentsInc/openagents/issues/12091), [E1](https://github.com/OpenAgentsInc/openagents/issues/12099), [E2](https://github.com/OpenAgentsInc/openagents/issues/12092), [E3](https://github.com/OpenAgentsInc/openagents/issues/12096), [F1](https://github.com/OpenAgentsInc/openagents/issues/12093), [F3](https://github.com/OpenAgentsInc/openagents/issues/12097) |
| Q2 | [#12100: Android: match adaptive phone and tablet navigation](https://github.com/OpenAgentsInc/openagents/issues/12100) | 1–2 days | [D1](https://github.com/OpenAgentsInc/openagents/issues/12087), [D2](https://github.com/OpenAgentsInc/openagents/issues/12090), [D3](https://github.com/OpenAgentsInc/openagents/issues/12095), [D5](https://github.com/OpenAgentsInc/openagents/issues/12091) |
| G2 | [#12101: Android: add signing upgrades and internal-testing release checks](https://github.com/OpenAgentsInc/openagents/issues/12101) | 1 day | [B1](https://github.com/OpenAgentsInc/openagents/issues/12083), [E2](https://github.com/OpenAgentsInc/openagents/issues/12092), [E3](https://github.com/OpenAgentsInc/openagents/issues/12096) |
| G1 | [#12103: Android: complete the parity matrix Design Lab and regression evidence](https://github.com/OpenAgentsInc/openagents/issues/12103) | 2–3 days | [A0](https://github.com/OpenAgentsInc/openagents/issues/12079), [A1](https://github.com/OpenAgentsInc/openagents/issues/12080), [A2](https://github.com/OpenAgentsInc/openagents/issues/12081), [A3](https://github.com/OpenAgentsInc/openagents/issues/12082), [B1](https://github.com/OpenAgentsInc/openagents/issues/12083), [C1](https://github.com/OpenAgentsInc/openagents/issues/12084), [C2](https://github.com/OpenAgentsInc/openagents/issues/12085), [C3](https://github.com/OpenAgentsInc/openagents/issues/12089), [D1](https://github.com/OpenAgentsInc/openagents/issues/12087), [D2](https://github.com/OpenAgentsInc/openagents/issues/12090), [D3](https://github.com/OpenAgentsInc/openagents/issues/12095), [D4](https://github.com/OpenAgentsInc/openagents/issues/12088), [D5](https://github.com/OpenAgentsInc/openagents/issues/12091), [E1](https://github.com/OpenAgentsInc/openagents/issues/12099), [E2](https://github.com/OpenAgentsInc/openagents/issues/12092), [E3](https://github.com/OpenAgentsInc/openagents/issues/12096), [F1](https://github.com/OpenAgentsInc/openagents/issues/12093), [F2](https://github.com/OpenAgentsInc/openagents/issues/12094), [F3](https://github.com/OpenAgentsInc/openagents/issues/12097), [Q1](https://github.com/OpenAgentsInc/openagents/issues/12102), [Q2](https://github.com/OpenAgentsInc/openagents/issues/12100), [G2](https://github.com/OpenAgentsInc/openagents/issues/12101) |
| H1 | [#12086: Android: optionally add Play Integrity account proof](https://github.com/OpenAgentsInc/openagents/issues/12086) | Optional, 1.5–2 days | Optional decision |

`R0` is computer-account setup, outside the Android implementation estimate.
It does not block SSH builds. `H1` is optional: the existing proof-of-work
account path supports the parity baseline. Neither optional Play Integrity
nor a release-store action should hold an unrelated code change open.

## Order and ownership

1. Finish **A0**, then **A1**. Record the first successful cross-build and
   actual disk consumption before scheduling more than one cold build.
2. Finish **A2** and **A3**; **B1** can proceed with A2 while secure-store
   work continues. The first emulator milestone is the isolated offline demo.
3. After B1, split work between the transcript, shell/shared-view adapter,
   and platform services. Land D4 early enough for C3 and E2. One agent owns
   each shared adapter/store interface at a time; define its acceptance
   boundary before dependent work starts.
4. Add identity/recovery, wallet migration, pairing, push, and full voice.
   Start G2's release scripts early, but its signed-upgrade checks wait for
   E2/E3. Start Q1/Q2 fixtures alongside UI work and finish them after their
   feature dependencies.
5. Complete **G1** with every required issue represented in the matrix.
   Produce the internal-testing handoff and separate the remaining physical
   device, account, and publication checks from code completion.

The audit's original app estimate is **29–40 agent-days including voice**.
A0, accessibility/lifecycle coverage, and adaptive layouts add roughly
**2.5–5 agent-days**. Treat **about 32–45 agent-days** as a planning range,
not a calendar promise. Re-estimate after A1 and the first transcript run;
the Android core has not been exercised. R0, optional H1, store approvals,
and device availability are outside that range.

Use lane 8's mobile ownership boundary from the
[master delegation](2026-10-16-master-delegation.md#lane-8-phone).
Before implementation, claim the specific issue and update its board state.
The owner subsequently authorized one Codex worker on CoderOS for A0.

## CoderOS handoff

Initial read-only observations from the planning pass on 2026-10-18 follow.
The registration update below replaces the account-target and package rows.

| Check | Observed result | What remains |
| --- | --- | --- |
| Tailscale and SSH | `coderos-4080`, `100.74.238.61`; `ssh christopherdavid@coderos-4080` succeeds with the saved host key; Tailscale ping 17 ms | Preserve this route; no new tunnel is needed for SSH builds |
| Host | x86_64 NixOS, Linux `6.18.49`; about 125 GB available on a 937 GB filesystem | Recheck disk before builds; free space is a snapshot |
| Android tools | Nix-provided `adb` 37.0.0 and emulator 36.5.11.0; `/dev/kvm` readable and writable | Verify NDK/JDK/build tools and an isolated emulator boot |
| Nix configuration | `~/coderos-hosts/hosts/coderos-4080.nix` enables `coderos.desktop.android` | Extend declarative configuration for building, not only running an emulator |
| Public emulator module | `os/modules/coderos/android.nix` supplies an image/emulator, deliberately omits build tools and CMake | Do not mistake emulator availability for a complete Android build environment |
| Existing host | `com.openagents.desktop.host.service` runs `~/openagents/target/release/coder` | Keep it running and preserve its binary |
| Current account target | This app lists Tron and Our cloud; CoderOS is absent. A legacy CLI enrollment exists, but its link is unreachable | R0: native headless engine and its own account credential |
| Native package | No native engine at the checked standard install/build locations; `https://openagents.com/releases/latest.txt` returns 404 | Build native headless from current main or supply a verified published package; do not install the withdrawn Coder release |

No SDK installation, emulator launch, account-key creation, native service
installation, or build ran in the planning pass. The legacy host-status
probe stalled and was terminated; it supplies no host-health evidence beyond
the unit being active. SSH and the tool/version checks completed.

R0's supported account path is a fresh named sign-in key from
**Settings → Sign-in keys**, held in a private service environment/credential
file as `CODER_DESKTOP_CREDENTIAL`, with
`OPENAGENTS_DEVICE_NAME=coderos-4080`. The native `headless` engine registers
its device. Linux's ordinary native `login` cannot persist credentials yet;
use the injected-credential path in the
[native account README](../../apps/openagents/crates/account/README.md).
Never copy the Mac's credential or owner secret. Use a separate native
service and profile; verify the entry through `list_devices` without making
owner-account test chats.

### Registration completed

At 12:43 UTC on 2026-10-18, the native headless engine joined the owner's
account as `coderos-4080`, device
`c0a120a9-b63b-4df2-9648-0c3ed8dce5e0` (#12078). The build used commit
`d2fa0bba5bae3ee74eb96e354b86e4528ba81b83`, Rust 1.98.1, and
`cargo build --locked -p openagents --no-default-features` from
`apps/openagents`, with dev debug information and dependency optimization
disabled for this installation. The build completed in 1 minute 38 seconds;
the copied binary's version and Linux loader checks passed.

- Service: `systemctl --user status openagents-native-target.service`.
  It is enabled, and user lingering is enabled for starts after logout.
- Binary: `~/.openagents/native-host/versions/<commit>/openagents`, with a
  sibling `COMMIT` file. The service uses this copy, not shared build output.
- Data: `~/.local/share/openagents-native-target`; IPC is loopback port 42941.
- Credential: the dedicated **CoderOS native target (12078)** sign-in key,
  held in `~/.config/openagents/native-target-12078.env` with mode `0600`.
  The Mac credential was not copied. The native launcher removes this key
  from its environment before starting agent children.
- Verification: account device discovery, remote projects, harnesses, and
  the Codex model catalog all answer through the app. The legacy Coder
  service remains active. Optional phone pairing was not changed.

Codex app chat `9f45b787-5171-417c-9ae3-9311e5471bdf` runs #12079 on
CoderOS in `~/.openagents/scratch/process-417853/android-env-12079`. It
confirmed the hostname and app chat, claimed the issue, and moved project
22 to **In Progress**. No other Android issue was delegated.

To remove this native target, stop and disable
`openagents-native-target.service`, then revoke its named sign-in key with
`openagents keys revoke 'CoderOS native target (12078)' --kind signin`.
Preserve the data directory for recovery and leave the legacy Coder service
and its executable alone.

### Android build work

For Android work, reserve dedicated Gradle, Android-user, and AVD directories.
Reuse warm external Cargo targets under the current `AGENTS.md` build
contract. Set `CARGO_INCREMENTAL=0`, choose `CARGO_TARGET_DIR`, and load
`scripts/dev/cargo-build-env.sh` for source-keyed compiler state before
running Cargo. Run one Cargo invocation per chat. Start the first build
serially and record a free-space floor before parallel work.
The existing emulator has its own state under `~/.openagents/android/avd`;
create an isolated test AVD rather than adopting it.

Foreman's host handoff records that build slots can grow to 30–70 GB and
three simultaneous batches previously exhausted the disk. Never delete
`~/openagents/target/release`; its Coder binary is rebuilt by the host's
`oa-terminal-dev` configuration. Do not garbage-collect the Nix store or
remove another agent's targets. Keep SDK, emulator, NDK, and JDK changes in
the Nix modules so they survive system rebuilds.

## Evidence and completion

- Use isolated worktrees, external persistent targets, and the pinned app
  Rust toolchain. Run focused Rust checks for edited crates and relevant
  Kotlin/instrumented checks; do not require the full release gate for an
  ordinary child issue. Keep captures and logs under session scratch.
- Use offline demo/synthetic secret and wallet fixtures for normal checks.
  Live checks use the dedicated test account and `X-OpenAgents-Test: 1` (or
  `OPENAGENTS_TEST_RUN=1`) and delete the data they create. Never test chats,
  projects, pairing, or payments on the owner's account or computers.
- Test the actual signed in-place retired-app upgrade. Do not uninstall or
  clear data to make a migration pass. A locked/corrupt store is not a missing
  seed. Use only public synthetic recovery words in isolated fixtures; no
  real recovery words enter logs, captures, or plaintext fixtures.
- Record baseline/candidate commits, build IDs, device/API/ABI, commands,
  measurements, captures, and failures. Add a platform/device dimension to
  Design Lab. Compare long-history, streaming, keyboard, lifecycle, memory,
  startup, and package-size results; agree measured budgets during A1/C1.
- A code-complete issue closes when merged with its own checks passing.
  Record remaining deploy/live checks in the master delegation's After-close
  checks and owner-only steps in [NEEDS_OWNER.md](../../NEEDS_OWNER.md).
  Do not claim physical-device, push, real-call, or store parity from emulator
  captures. A later defect becomes a new issue.

## Decisions and owner steps

This planning pass does not approve a higher minimum API, a new signing key,
a Firebase project, publication, or Play Integrity. Preserve API 26 upgrade
compatibility while implementation validates dependencies. Pin and verify
SDK/NDK/JDK/Gradle/Kotlin requirements in A0/B1 rather than describing the
audit's versions as current requirements.

Use Android-native system bars, back navigation, and platform controls with
the shared palette/type/layout. The audit's Liquid Glass, API 29, phone-only,
voice deferral, distribution, and cloud-backup recommendations were not
accepted decisions. Full parity keeps voice and adaptive navigation in scope;
an optional cloud copy of recovery words stays off unless separately chosen.

Before distribution, confirm the minimum API, exact signing/Play App Signing
continuity, and testing channel. Owner actions for Firebase/server secrets,
Play Console, privacy/Data safety/content rating, physical devices, and any
funded wallet or real-call checks are listed in
[NEEDS_OWNER.md](../../NEEDS_OWNER.md). A0 is authorized separately; these
remaining decisions do not authorize publication.
