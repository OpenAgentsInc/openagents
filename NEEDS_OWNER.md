# Owner checks

## Shared Everglade on two devices (#10553)

On the Mac that runs the Coder host, grant two devices `world` (and one of
them `observe`), start `openagents chamber host host.json` with
`"transport": {"type": "reach"}` and `"profile": "everglade"`, and join from
two computers with `verse --join FILE` (a build with
`--features remote-chamber`). Confirm both see the studio's seats at the same
places and each other's avatars, that `W`, `S`, `A`, `D`, `Q`, and `E` walk
and turn the avatar in the expected directions, and that the `world`-only
device opens no studio panel. The binary-level two-client check passes over
REACH with a scripted studio; the facing and keyboard mapping need a person
at a window.

## Desktop composer (#10004)

In a Mac build from current `main`, select the Japanese input source using the
system input menu. Start a new chat, compose a Japanese word, change a candidate,
and commit it. Confirm that Enter confirms the candidate without sending the
message; a later Enter sends it. Confirm that clicking away cancels marked text
without replacing the previously committed draft.

Scripted Japanese preedit, commit, Enter suppression, and focus-loss checks pass.
This remaining check exercises macOS's actual input method and input menu.

## Linux transcript performance (#10005)

On Linux with a native Wayland or X11 session, run the matrix in
`docs/desktop/verification/2026-09-30-transcript/verification.md` and retain the
result directory. Confirm transcript scrolling, tool expansion, and code copy.
The eight Mac cases pass; Linux hardware is unavailable in this worktree.
Open a follow-up issue if the Linux run finds a defect or exceeds 8.3 ms.

## Desktop image input (#10011)

On Linux in a native Wayland or X11 session, attach a generated PNG or JPEG
through **Attach image**, clipboard paste, and file drop. Confirm that each
preview appears, **Remove** preserves the caption, and **Send** preserves the
draft while explaining the current hosted text-only limit. A missing portal or
clipboard facility must report a reason. Repeat image clipboard paste on macOS.
The real macOS picker and scripted decoder, clipboard pixel, drop, removal,
and refusal checks pass. Open a follow-up issue if a native adapter check fails.

## Installed local Coder broker, handoff, and task chat (#10014–#10016)

After installing the desktop bundle and its Coder built from current `main`,
exercise the **Run Coder** flow added by #10015 against this computer. Confirm
that the normal OS key source remains in the resident host, saved device
pairings remain available after restart, and the task can be read and stopped.
With the configured engine signed in, verify live tool steps, Stop, the
phase-appropriate steering choice, queued follow-ups, and answers to a question
and an approval in the same task on desktop and phone. The scratch host checks
cover command admission, durable queue editing, exact retries, steering, Stop,
and archive. Shared state tests cover question and approval answers, stale
responses, split ATIF records, and draft acknowledgment. Native painter checks
cover all three task modes at normal and minimum window sizes.
Archive every verification task afterward. The scratch same-user socket,
portable client, durable inbox, restart, history, admission, and cancellation
checks pass without accessing the owner’s computer state.

## Installed saved sessions (#10017)

After installing a desktop and resident Coder built from current `main`, open
**Saved sessions** and inspect a known Codex session and Claude Code session.
Confirm their titles and times against the original tools. Choose a configured
project and continue one with the engine signed in and the existing auto-start
policy enabled. Confirm that its recent context reaches Coder, tool steps
appear, and the original saved session remains unchanged. Stop and archive
every verification task afterward. Scratch tests cover both source formats,
the native reader, real broker admission, exact prompt bytes, acknowledgment
retries, restart identity, cancellation, and archive without owner state.

## Desktop playable Grid (#10038)

Install a desktop bundle built from current `main` on macOS and on Linux
Wayland/X11. Choose **The Grid → Play**. Verify right-drag look, left-drag
orbit, simultaneous WASD, wheel entry and exit from first person, Escape,
Tab, Alt-Tab/Cmd-Tab, minimization, display-scale changes, and returning to
Watch or chat. A failed cursor grab must leave the pointer usable and explain
the keyboard fallback. Confirm the original chat draft survives.

Verify the separate world identity and Gym connection in the macOS login
Keychain or Linux Secret Service. Relaunch Play and confirm the public key
stays the same; deny a key read and confirm Play stays offline without
replacing it. Pairing and the resident host must retain their existing keys.
With a grant created for that world public key, inspect Gym runs, review a
recipe's budget, and explicitly confirm a permitted launch. Walking and
public boards must never launch it. Check desktop/phone movement together
against the chosen relay without creating test chats or tasks.

The isolated relay, shared controller, native input and layout, GPU captures,
and Linux binary checks are recorded in
`docs/desktop/verification/2026-09-30-playable-grid/verification.md`.
Native OS cursor grabs, protected-store prompts, and the signed Mac bundle
need device checks. Windows Verse remains tracked by #10027. Open a new issue
if any installed-device check finds a defect; these checks do not keep #10038 open.

## Cloud artifact signing (#10227)

Configure a private GCS artifact bucket with lifecycle retention and an identity
with object-create and URL-signing permissions. Run a non-sensitive Boat or GCE
command using the artifact publisher described in `scripts/cloud/README.md`.
Verify that all four signed links open and expire after 24 hours. Mocked tests
cover upload and comment behavior; a live signing check has not run.

## GCE retirement decision (#10223)

Review `docs/cloud/2026-10-02-orphan-retirement-review.md` and confirm a current
owner and retain-or-stop decision for each of the nine running candidates.
No machines were stopped or deleted. The issue explicitly requires this decision;
its operational work is not complete.

## Raw versus OpenAgents study (#10162)

The standing study still needs implementation and new measured trials for raw
Claude Code, raw Codex, the shipped default path, and a matched arm, including
repository tasks and Terminal-Bench tasks. Existing matched Opus evidence is
not evidence for the current shipped default path. This delegated run cannot
launch another coding engine as a subprocess, so it did not execute raw-engine
trials. Do not report a savings percentage or close the issue based on the
historical pilot alone.

## Boat paid lifecycle (#10218)

Run the ignored `paid_lifecycle` test in `crates/boat/tests/live.rs` with a
scoped, expiring Boat key and its ID. Follow `crates/boat/README.md` for the
command and required variables. The offline suite and cleanup failure-path
check pass; the paid check has not run. Confirm that deletion completes and
final usage is non-running and below $0.01. The check detects cost overruns;
it does not impose a provider-side spending limit. Do not use an unrestricted
key or publish the credential in logs.

## Everglade studio on phones (#10476, #10485, #10486)

Build the Coder iOS app from current `main` and run it in a simulator, then on
a device. Walk through the Everglade arch on the plaza, wait for the pack to
download, and confirm the glade renders textured (grass, trees with cut-out
leaves, the workshop) on Metal. At the notice board, a desk, the podium, and
the merge station, tap **Interact** and confirm that the console, the seat
panel, decisions, and diff review open, and that the close control returns to
the world. Run `testEvergladePortalEntersGladeAndReturns`; it now needs the
pack download and still names its screenshot "Everglade greybox glade".

Repeat on the Android emulator and a device, on Vulkan or GLES: the same four
stations, the back button closing the panel, and the TalkBack Interact action.
Until the live host source lands (#10492), every panel says the studio has not
loaded. The Rust panel and scene tests pass. Open a follow-up issue for any
rendering or mounting defect.

## Agent Studio mixed-engine run (#10477)

Run the studio once with real engines, on a scratch host, from current
`main`. The code paths are covered by unit tests and by the scratch-host
acceptance test (`cargo test -p verse --test studio_host`), but no real engine
has driven them; the repository's velocity rules leave live engine runs to
the owner. The [Agent Studio audit](docs/verse/agent-studio-audit.md) lists
what a scratch host already showed and the defects to expect. The simulated
team is a read-only replay inside Verse and can't stand in for this run
(#10572).

1. Build `openagents`, `coder`, and `verse` from current `main` into the
   agent slot's target directory and call them by path. The `openagents` on
   `PATH` is an older program with no `studio` command.
2. Start a scratch host with a temporary `HOME` and `--state`, `--root`, and
   `--tasks` under a temporary directory, admitting a scratch Git repository
   as a workspace (`coder host init --workspace scratch=PATH`). The
   repository needs a clean checkout and no remote: a merge fast-forwards the
   checked-out branch locally and pushes nothing.
3. Add seats with `openagents studio seat set`: a Codex lead
   (`--role lead --route codex:MODEL`) and Claude Code (`claude:MODEL`),
   OpenCode (`opencode:PROVIDER/MODEL`), and Codex (`codex:MODEL`) workers.
   Turn on auto-start for the workspace with `full` access and exactly those
   routes. Microcoder isn't a route: a `codex` seat runs Microcoder's loop by
   default (`coder.codex` is `loop`), and a `claude` seat runs a Claude Code
   session (`coder.claude` is `session`). These settings apply to the whole
   host until #10568 lets each seat choose its engine.
4. Open Verse with `--everglade --studio-socket PATH`, pointing at the
   scratch host's control socket, and submit a two-task goal from the notice
   board's console.
5. Confirm that at least two tasks run in parallel, seats walk to the
   stations their work implies, one question and one approval are answered at
   the podium, one change is requested at the merge station with a line
   comment and fixed, and one task is merged from the review. Only
   Microcoder's loop raises questions and approvals; Claude Code and Codex
   sessions run with permission bypass flags. Answering, reviewing, and
   merging need Verse until #10566 adds them to `openagents studio`. Merge
   only finished tasks, because the host doesn't refuse an unfinished one
   yet (#10567).
6. Quit Verse mid-run and reopen it, then restart the host mid-run, and
   confirm that no state is lost.
7. Archive every task the run created (`coder task archive TASK_ID --reason
   "studio trial"` against the scratch task store) before deleting the
   scratch host.

Open a follow-up issue for any defect, with the seat routes and the step it
failed at.

## Everglade from the Grid, as the ranger (#10530, #10534)

On TestFlight build 46 (iPhone) and an Android device: on the Grid, walk
through the arch lettered EVERGLADE. The first visit shows download progress
with Cancel; cancel once, then Retry with the network off and on. In
Everglade, confirm you play the hooded ranger, it idles, walks, runs, and
jumps with the stick, and no spade follows you. Return by the arch lettered
THE GRID and by **The Grid**, and check you land beside the EVERGLADE arch
with other players visible again. A second visit opens from the cache.

## Phone studio panels on Android and a real phone (#10579)

The iOS host builds and, in the simulator, enters Everglade and reports
that no computer is online for the studio. To finish checking:

1. Build the Android app (`bins/openagents-android/build.sh`): this Mac has
   no Android SDK, so the Kotlin wiring (`VerseStudio.kt`, the JNI entries
   in `crates/openagents-mobile/src/android.rs`) was never compiled.
2. On a phone paired with a computer that runs the studio, walk into
   Everglade, stand at the podium, tap Interact, and answer a decision;
   then merge a finished task at the merge station.

## Display names over heads on real devices (#10583)

#10598 adds a **Display name** field to Account on iOS and Android and
sends the name in NIP-MV states. This box has no Xcode and no Gradle
run of the Kotlin host, so the Swift and Kotlin glue was not compiled.
To finish checking:

1. Build both hosts (`scripts/release/testflight.sh start --validate-only`,
   `bins/openagents-android/build.sh`).
2. Set a name under **Account > Display name** on one phone, open the
   Grid on a second device or run `openagents --json verse who`, and read
   the name over the first phone's avatar.
3. Run `openagents verse walkers 20` and check that `walker-0` to
   `walker-19` are readable over heads at a steady frame rate.

## Two players meet in Everglade (#10584)

#10604 re-keys presence to a zone's shared NIP-MV world
(`verse-everglade`, `verse-lagrange-1`) through the arch. Loopback tests
cover the mobile and desktop world switch; no two-device run happened
here. To finish checking:

1. On two phones (or a phone and the desktop), walk through the Everglade
   arch from the Grid and confirm both see each other's avatar, name tag,
   and collide in the glade; walk back and confirm both reappear on the
   Grid.
2. Standing in Everglade, run `openagents verse walkers 5 --world everglade`
   and `openagents --json verse who --world everglade`; the walkers must be
   visible in the glade and absent from the Grid.

## Two desktops in a public chamber through RITUAL (#10585)

The guest chamber host and the Grid's RITUAL arch were checked on one
machine: four guest keys joined a local host, a fifth was refused, and a
reconnect kept its seat. The two-desktop play test needs a second
contributor machine and a desktop display, which this box has not. To finish
checking:

1. Run a host with a `guests` policy on a machine two desktops can reach
   (`openagents chamber host CONFIG.json`, or `chamber service install`),
   and copy its DER certificate to both desktops.
2. Write `~/.verse/ritual.json` on both (address, instance, `trust_der`,
   `pack`, `scene`, `dir`) and start `verse` built with
   `--features remote-chamber,imported-desktop`.
3. Walk both players through the `RITUAL` arch, fight cultists in the same
   chamber window, read `openagents --json chamber status` for
   `population.players: 2`, then close the windows and read that both
   players stand before the arch again with Grid presence restored.

## The phone Grid on the engine renderer, on an iPhone (#10616)

The Grid draws through `verse-engine` on every client: the engine opens on
an iOS Metal layer, an Android window, and a browser canvas, and the Android
package and the browser module were built here. This box has no macOS, so
the iOS build and the Metal surface were not run. To finish checking:

1. Build the OpenAgents iOS app from `main` and open the Grid; the plaza,
   arches, line figures, and name tags must draw as on the desktop, in the
   neutral phone palette, with the stick and HUD unchanged.
2. Rotate the phone and background and foreground the app; the frame must
   follow the new size and resume after the layer is reattached.
3. Walk through the Everglade arch and back; the Everglade draws on the
   legacy renderer and the Grid returns on the engine.
4. With `openagents verse walkers 10` running, confirm ten walkers with
   their names move smoothly on the phone's Grid.

## The phone in the chamber (#10586)

The phone joins the chamber through the RITUAL arch, and the session,
suspend and resume, host loss, and respawn pass against an in-process
chamber host. The frame-time run on a phone did not happen: the build
machine's disk filled during the Android release build. To finish checking:

1. Host a chamber on a scratch directory with a temporary `HOME`:
   `openagents chamber pack DIR/assets`, `openagents chamber tls DIR`, and
   `openagents chamber host DIR/host.json` with
   `"guests": {"cap": 16, "ring": [0, 0, -22], "radius": 3}` and the scene
   `assets/verse/original/ritual.json`. Join five more guests with
   `openagents chamber move` under five scratch profiles, so the chamber
   holds 20 actors with the phone.
2. Write `ritual.json` (address, instance, `trust_der`, `pack`, `scene`,
   `dir`, and `server_name`) and copy it, the certificate, the pack, the
   scene, and the asset directory into the app's zone cache directory: on
   Android, `cache/VerseZones` of `com.openagents.app` (`adb push`, then
   `adb shell run-as com.openagents.app cp ...`); on iOS, `VerseZones` in
   the app's caches directory. An emulator reaches the Mac's host at
   `10.0.2.2`.
3. On an Android phone and an iPhone built from `main` (release Rust), walk
   through the `RITUAL` arch on the Grid, fight for two minutes, die and
   respawn, background and foreground the app, then tap **Leave**. Confirm
   the player returns before the arch with Grid presence restored.
4. Copy `chamber-frames.json` from beside `ritual.json` into
   `docs/verse/verification/2026-10-05-phone-chamber/` for each device.
5. Fight a desktop player in the same chamber (`verse` built with
   `--features remote-chamber,imported-desktop`, through its own RITUAL
   arch): each must see the other's character move, cast, and take damage.

## The world population cap on the public relay (#10588)

The relay now refuses a 21st key's NIP-MV frames in one world
(`NOSTR_RELAY_WORLD_POPULATION_CAP`, default 20) and bounds a world's
frames a second (`NOSTR_RELAY_WORLD_POSE_PER_SEC`, default 300). Unit
tests cover the cap and the budget; the public relay doesn't run it until
it's redeployed. To finish checking:

1. Deploy the relay from `main` (`docs/deployment/runbook-cloud-run.md`).
2. With the Grid empty, run `openagents verse walkers 21 --wait 30`; the
   `done` line must count refusals (`rate-limited: world is full`), and
   `openagents verse who --world verse-bare` must show 20 live.
3. Raise `NOSTR_RELAY_WORLD_POPULATION_CAP` before the 20-player soak
   (#10589), which also brings a phone, a desktop, and a browser.
4. Block a walker with `openagents verse block KEY`, relaunch the Verse
   app on the same computer, and check that the walker stays hidden.

## Block a player from the Grid on a phone (#10638)

Tapping a player's name tag on the Grid opens a card with **Block**,
**Mute**, and **Close**; the desktop Grid opens it with a click. A
loopback test drives the scene; no phone ran it. On an iPhone and an
Android phone built from `main`, start `openagents verse walkers 3`, tap
a walker's tag, and tap **Block**: the walker disappears and stays gone
after the app is relaunched.

## The Grid in the browser at `/grid` (#10587, #10626)

The browser Grid joins the other players over the browser's WebSocket and
draws on WebGL2 as well as WebGPU; headless Chrome on a scratch relay saw
20 walkers with names on both (`docs/verse/verification/2026-10-05-grid-browser/`).
openagents.com was not redeployed. To finish:

1. Deploy `openagents-web` from `main` as the `/druid` deploy did
   (`docs/deployment/openagents-web.md`). On the `new` tag, `/grid` must
   answer 200 with `connect-src 'self' wss://relay.openagents.com` in its
   policy.
2. Open `/grid?name=YOURNAME` in Chrome, in Chrome with WebGPU off
   (`chrome://flags`, or `/grid?gl`), and in Safari. Each must draw the
   Grid with `ONLINE · N HERE` at the top left and your name over your
   head; walk with `W`.
3. With the OpenAgents app on a phone in the Grid, the phone and the
   browser must each see the other move, with names.
4. Click another player's name tag and **Block**: they disappear, and stay
   gone after a reload.
5. Optional: on the Android emulator with `-gpu swiftshader_indirect`, the
   OpenAgents app's Grid must draw instead of the renderer's error card.
