# Mobile pairing, connections, and controls — September 26, 2026

Issue [#9718](https://github.com/OpenAgentsInc/openagents/issues/9718) follows
user reports from build 45: the pairing command was difficult to enter, chats
did not load automatically, metadata crowded out messages, and a world relay
choice disappeared. The user also reported that recenter appeared ineffective
and that movement could not combine with looking or jumping.

Build 46 is in preparation. Final native acceptance and TestFlight delivery
will be recorded in the [evidence directory](../../../bins/coder-ios/verification/2026-09-26-mobile-connections/README.md).

## Causes and changes

### Pairing and live chats

The old `Connect` operation saved its valid grant but performed no catalog
read. Opening a chat and entering the foreground also relied on a later
refresh. Existing Rust acceptance explicitly refreshed after pairing, so it
never required the first list to arrive automatically.

The native chat panel also created a new five-second timer publisher with each
SwiftUI view value. Verse redraws could replace that subscription before it
fired. The panel now uses a task whose lifetime depends on visibility, pairing,
and foreground state; world frames do not restart it.

Rust now fetches the first catalog after pairing and the first transcript page
when a chat opens. Foreground entry refreshes retained state immediately.
A new response field, `pairing_completed`, distinguishes a saved grant from a
successful first read. A temporary read failure leaves the valid grant saved,
closes pairing, and shows the read error. Invalid replacement input cannot
mistake an older valid connection for a new successful pairing. Background
state pauses polling, and skipped backoff ticks no longer erase the error.

The computer commands are `./pair` in an updated checkout and `coder pair`
with current Coder installed. Both use the same read-only observer. There is
no new hosted bootstrap URL or separate execution authority.

### Space for chats

The computer opens chats at the available panel height. Settings, relay state,
device key, and disconnect controls live behind **…**. Transcript **Details**
reveals byte counters, page diagnostics, exact source controls, and reload.
The normal view retains messages, roles, navigation, and follow controls.
Unknown records and missing-cache warnings remain visible; readable text is
not truncated to achieve a smaller interface. Original Markdown moves to a
native context menu instead of occupying a button beneath every message.

### World connection

Previously the relay existed only in the active scene, and the input field
was an empty panel-local value after reopening. Joining also started spawn
recovery, which could move the player away and close the nearby computer.

The native storage adapters now retain only the Rust-validated relay choice
in device-only Keychain or Keystore storage, independently of chat access.
A fresh mount restores the selection. Manual join, reconnect, and foreground
resume preserve position and the open panel; only a fresh mount restores the
player's signed saved position. **Leave** stops the session and forgets the
selection. A storage failure is visible rather than reported as durable success.

Rust reports offline, paused, connecting, connected, or retrying state and a
bounded error. Connected requires both world subscriptions to reach EOSE and
successful authentication when challenged. A refused subscription does not
remain labeled connected. This status establishes the admitted connection,
not the presence of another player. Synthetic tests use **Preview** and create
no network session.

A bounded check against `wss://relay.openagents.com` accepted a fresh identity's
AUTH and both world subscriptions in 350 ms. The probe published no world or
chat events and retained no other player's payload. See the
[receipt](../../../bins/coder-ios/verification/2026-09-26-mobile-connections/public-relay-subscriptions.json).

### Combined controls and recenter

Build 45 reserved every two-finger interaction for pinch. That prevented left
movement plus right camera input. The new arbitration distinguishes an
established control from a deliberate pinch; it keeps a held left control
active while the right side looks or double-taps to jump. A recognized pinch
cancels control input until its fingers lift. Rust retains tap timing, movement,
jump, camera bounds, and lifecycle state.

The explicit crosshair sends `recenter_camera`: it returns the view behind the
avatar at the default pitch, preserves zoom and position, and establishes a
fresh phone-orientation reference. Ordinary sensor/lifecycle `reset_motion`
continues to reset its baseline without unexpectedly moving the view. Camera
mode and recenter use icon-only buttons at the bottom right, with accessibility
labels retained.

### Pinch acceptance correction

The initial simulator test reported that a second pinch did not zoom out.
Retained native contact logs identified a test-coordinate problem: XCTest
started the zoom-out at `(345.60, 815.25)`, inside the Recenter button's
`(342, 784, 44, 44)` frame. Only the opposite finger reached the world surface,
so it became movement rather than a two-contact pinch. The corrected test uses
a transparent, synthetic-only center marker that intercepts no touches. Both
zoom directions must still change distance without moving the avatar or camera
heading. Production map and button contacts retain their own behavior.

## Verification scope

The evidence directory separates local protocol fixtures, a bounded public
relay subscription check, native adapter checks, simulator interaction, and
archive/distribution evidence. No model or benchmark runs are required.
Physical-device gesture comfort, sensor noise, and frame rate remain separate
from simulator acceptance.

The connection checkpoint passed 50 mobile tests. After integrating the map,
companion, and gate changes, the current shared mobile suite passes 61 tests,
with one manual device fixture ignored. Strict Clippy passes for the mobile targets. Ten focused Verse session
tests pass. Android's main and instrumentation sources compile, and its two
native pinch-admission unit tests pass; this change has no new Android emulator
runtime receipt. The shared observer library passes 25 tests, with three
external fixtures ignored, and the installed `coder pair` CLI passes both
focused tests.

Earlier test failures are retained. Two presentation assertions expected the
old verbose interface, and two new gesture fixtures had incorrect setup or
expected camera heading; these assertions were corrected before the final
run. One real-host revocation fixture reported a transport conflict and passed
on the next full run. Its intermittent cause remains unproven; the passing
rerun does not establish that the underlying transport failure was fixed.

The first native pass ran 14 checks: 11 passed and 3 failed. The retained log
records a camera drag after recenter, a second pinch, and an accessibility
snapshot timeout before a second device-key read. Review found a real
monitor-hit capture bug: starting a drag on the monitor kept the gesture
reserved as a tap. Crossing the drag threshold now releases that capture.
The native touch adapter also orders fresh contacts and accepts reversed
contact batches for pinch admission. The identity test uses a targeted key
query and retains the across-relaunch equality assertion. The combined native pass then ran 21 tests: 18 passed, one live-host test
skipped because its separate fixture was absent, and two assertions failed.
One expected the mobile SSH control intentionally removed from the Phone
projection. The other began a movement swipe inside the new gate item strip.
Both tests now check the current UI without changing production input routing;
the focused retest remains pending. Normal startup, background/relaunch,
chats, identity, world-relay persistence, pinch, companion tapping, gate
choices, and per-gate persistence passed in that combined run.

### Release integration with the owner-key field

Main added a phone owner-key input while this release was in acceptance.
The shared request correctly marked the input secret, but the native hosts
did not consume that flag. Both now use password fields for secret requests,
keep invitation input unchanged, disable text suggestions/autofill where the
platform supports it, and clear drafts on submission, cancellation, dismissal,
or backgrounding. Separate native tests check masking and cancel/reopen
behavior with an invalid marker, never a real key. Rust continues to own
validation and the existing owner-grant authority check.

Gate-storage errors also stay in the gate UI; they no longer appear as world
relay errors in Computer settings.
