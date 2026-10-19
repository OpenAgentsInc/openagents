# Desktop on Linux, and every desktop and phone pairing

On 2026-10-19 the OpenAgents desktop app ran on Linux for the first time
end to end, and each desktop was paired with each phone and with the other
desktop on production openagents.com. Every check used dedicated test
identities, never the owner's account or the owner's running apps. This
file records the setup, each journey's result, the defects found, and the
commits that fixed them.

## Setup

| Item | Value |
| --- | --- |
| Linux desktop | coderos-4080 (NixOS, Linux 6.18, RTX 4080 over Vulkan), on the CoderOS compositor (Wayland) and on Xwayland (X11). Debug build of `openagents` from each fix commit, started from `shell-linux.nix`. Two isolated instances (`lm-data` on IPC 47391, `lm-data2` on 47395), each with its background engine as a systemd user unit. |
| Mac desktop | "OpenAgents Dev" built by `scripts/run-macos-dev.sh` with bundle ID `com.openagents.desktop.devtest`, its own data folders (`lm-mac-data` on 47393, `lm-mac2-data` on 47394), `OPENAGENTS_TEST_RUN=1`, and the engine in the app. The owner's OpenAgents Dev app and its data were not touched. |
| Android | Emulator `oa-parity` (Pixel 7, API 35). Release 1.0.0 (1016) to reproduce, then debug `com.openagents.app.debug` built on coderos from each fix commit, signed in as the dedicated test account. |
| iOS | Simulator "Parity iPhone" (iOS 26.5). A debug build with its own new key (identity I), then the account-fixture build signed in as the dedicated test account. |
| Identity A | The dedicated test account (`acct_83d16a35955ef908`): Android, the iOS fixture, Linux 1, Linux 2, Mac 2. |
| Identity I | A new account the iOS build made from its own key: Mac 1. |
| Pairing | Connect codes came from the desktop's QR (decoded from a screenshot on Linux) or its `CreateConnectCode` call; Android opened the code's App Link, iOS pasted it in the scanner. |
| Chats | Each desktop chat ran Codex at low reasoning in a read-only scratch folder; prompts asked for one capitalized word, so a reply proves the turn ran. |

## Matrix

"Pass after X" means the journey failed until commit X.

| Journey | Linux ↔ Android | Linux ↔ iOS | Mac ↔ Android | Mac ↔ iOS | Mac ↔ Linux |
| --- | --- | --- | --- | --- | --- |
| Desktop starts and signs in | Pass after `79ace15dbf` | Pass after `79ace15dbf` | Pass | Pass | Pass |
| Pair by QR or link | Pass after `79ace15dbf`, `a5109dcb63` | Pass (Linux 2) | Pass after `79ace15dbf`, `f47b54cd48` | Pass after `79ace15dbf` (Mac 1, Mac 2) | n/a (same account) |
| Desktop joins the phone's account and syncs, no relaunch | Pass after `a5109dcb63` | Pass after `a5109dcb63` (background engine) | Pass after `a5109dcb63` | Pass after `a5109dcb63` (engine in app) | n/a |
| Phone lists the computer | Pass | Pass | Pass | Pass | Each lists the other |
| Phone sees the computer's sessions | Pass | Pass | Pass | Pass | Each sees the other's |
| Message from the phone into a desktop chat, reply comes back | Pass after `f47b54cd48` (see open issue) | Pass | Pass | Pass | Pass both ways (Mac → Linux, Linux → Mac) |
| Live updates both ways | Pass | Pass | Pass | Pass | Pass |
| Cloud chat started on the phone shows on the desktop | Pass (listed as "Web" on Linux) | Not rerun (passed in the parity run) | Pass | Not rerun | n/a |
| Same session and project lists | Pass | Pass | Pass | Pass | Pass |
| Move a chat to another computer | Not run: moves go to a paid cloud computer | Not run | Not run | Not run | Not run |
| X11 (Xwayland) | Pass: renders, lists, live updates | — | — | — | — |

## Defects fixed

| Commit | Defect |
| --- | --- |
| `79ace15dbf` | No phone could pair with the desktop app on any platform: the desktop wrote `wss://relay.openagents.com` into its invitations, phones redeem on `wss://relay.openagents.com/`, and the host refuses a relay that differs by a slash. |
| `79ace15dbf` | Linux: the account control was macOS-only, so Home stayed on "Connecting to your account…". |
| `79ace15dbf` | Linux: the sign-in and the 12-word identity had no store; they now live in `0600` files under the data folder. |
| `79ace15dbf` | Linux without a Secret Service (CoderOS): the device connection didn't start ("the owner key cannot be created"); its keys now fall back to files on a first start. |
| `79ace15dbf` | NixOS: the app panicked with `NoWaylandLib` outside a library path; `shell-linux.nix` now sets it. |
| `79ace15dbf` | Every data folder of a build shared one identity item, so a test instance on the owner's Mac would have signed in as the owner. `OPENAGENTS_TEST_RUN=1` now gives an instance its own item. |
| `79ace15dbf` | Android said "Couldn't finish" for every failed link; it now gives the reason. |
| `a5109dcb63` | After a phone linked it, the computer stayed local until relaunched (engine in its own process), or asked "Sync is ready" again (engine in the app). An empty computer now switches at once. |
| `a5109dcb63` | A lost account-link answer couldn't be asked for again; the computer now answers the same request with the same proof and the phone retries once. |
| `a5109dcb63` | The Rust core logged nothing on Android; warnings now reach logcat (tag `OpenAgents`). |
| `f47b54cd48` | Each device-connection request held the host's access lock while the app answered; others gave up after two seconds with no reply ("the host did not answer in time"). App reads now release the lock. |
| `f47b54cd48` | After a computer restarted, a phone never reopened its device connection until the app restarted. It now dials again from the saved pairing. |
| `f47b54cd48` | Pairing with a computer already on the same account said "hasn't joined your account". |
| `eb35901646` | Android opened a new activity for each App Link, and a link's review could sit behind Your ID; text overflowed the welcome rows and the hidden-code box on Linux. |

| this commit | `openagents chats list` isn't a command, so it sent "list" as a new chat to whatever host answered; a bare `list` or `ls` now points to `chat threads`. |

## Open

- Android emulator to a computer: some device-connection requests arrive
  15 to 30 seconds late (the host logs them after the phone gave up), while
  the Mac reaches the same computer in under 100 ms every time. The QUIC
  path tries the computer's Tailscale address from the emulator's NAT
  (`sendmsg failed`). The phone's retry and redial hide most of it. Filed
  as [#12170](https://github.com/OpenAgentsInc/openagents/issues/12170).
- Moving a chat to a cloud computer wasn't run: it spends paid cloud time
  and needs the owner's approval.
- Physical devices, push, and store builds stay owner steps, as in the
  parity run.

## Cleanup

- Desktop chats on the test account archived; the phone's cloud chat
  deleted; the three desktop keys linked to the test account removed.
- Both Linux instances signed out, their systemd user units disabled and
  removed, and their data folders deleted; the Mac instances signed out,
  their Keychain items (identity, device-connection keys) deleted, and
  their data folders deleted. The owner's identity item and apps were
  untouched.
- The iOS test app uninstalled after clearing its fixture sign-in; the
  Android release app uninstalled and the debug app's data cleared.
- About 120 GB of stale build output was removed on coderos; it has 77 GB
  free. No GCE VM was created.
- Left: two new accounts the phones made from their own keys (one with an
  archived chat), as the parity run's identity B; and one local thread
  titled "list" on the owner's Mac host, sent by mistake through
  `openagents chats list` (the CLI fix above stops that). It isn't on the
  owner's account or phone, and the CLI has no way to delete a host-only
  thread.
