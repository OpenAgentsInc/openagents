# Computers screens verification

Date: September 26, 2026. Issue:
[#9713](https://github.com/OpenAgentsInc/openagents/issues/9713), part of
[#9704](https://github.com/OpenAgentsInc/openagents/issues/9704).

This record covers the Computers screens: host status, adding a computer,
access, first run, and activity, as Rust Native projections in
[`crates/coder-computers`](../../../crates/coder-computers/README.md). It also
covers their mounts on the iOS and Android hosts and the terminal adapter
slice in [`coder-terminal`](../../../crates/coder-terminal/src/native.rs).
Evidence is split into synthetic Rust checks, simulator and emulator checks,
and physical-device checks. No physical-device check ran.

## What landed

- `coder-computers`: the snapshot model, a closed `Intent` and `Screen` enum,
  the projection, one authority check shared by projection and controller,
  the `Computers` controller for one surface lifetime, typed input requests,
  the `ComputersService` seam, the `Unavailable` service, and an offline
  `Synthetic` fixture. Each fixture host's status comes from a real
  `coder-link` supervisor, and its activity passes through
  `nostr::activity_summary::encode`.
- `coder-terminal::native`: the RN1 terminal slice. It draws stacks, lists,
  text, buttons, and surfaces on the amber ladder, keeps text literal and
  strips control characters, reports unsupported properties, and turns
  keyboard focus into revision-bound activations. A disabled control is
  drawn in parentheses so the difference survives a colorless terminal.
- `coder-mobile`: the Computers screens as a second surface beside the
  reader, with its own instance and revisions, four `computers_*` requests,
  and packet fields for the view, the input request, and a one-shot first-run
  exit. Synthetic mode uses the fixture; the normal app uses `Unavailable`.
- iOS and Android hosts: a **Computers**/**Chats** control in the Computer
  panel, a scrolling mount of the Computers tree, and the native field or QR
  scanner that an input request names. Android's `NativeRenderer` takes the
  packet field it mounts.

No generic element was added to `rust-native`. A switch is a button whose
label says **Switch off** or **Switch on**, and a disabled control's reason is
a status text node keyed `<control>-reason` that follows it. Text entry uses a
typed input request rather than an editable element, because the shared
editable-input contract (RN4) is not defined yet.

## Synthetic checks

Toolchain: Rust 1.97.1, separate `CARGO_TARGET_DIR` in the worktree.

| Command | Result |
| --- | --- |
| `cargo test -p coder-computers` | 16 passed. |
| `cargo test -p coder-terminal --lib native` | 2 passed. |
| `cargo test -p coder-mobile` | 30 passed, including 2 new Computers surface tests and the existing reader, pairing, and connection tests. |
| `cargo clippy -p coder-computers -p coder-terminal -p coder-mobile --all-targets -- -D warnings` | Passed. |
| `cargo fmt -p coder-computers -p coder-terminal -p coder-mobile -- --check` | Passed. |

The `coder-computers` tests cover:

- Every status and its wording: online with current, catching-up, and failed
  data; connecting while establishing, probing, and replacing; offline when
  switched off, not connected (with and without a supervisor), with no
  network, retrying, refused, and misconfigured; out of date on the host, on
  this app, and unknown; not enrolled with no access, expired, and past its
  expiry; and revoked by grant and by supervisor.
- The compatibility rule naming which side is behind.
- Every one of the 21 intent kinds, each resolved from a real control in a
  current view and checked against the snapshot.
- Stale activations: every control on every screen, for phone and desktop,
  refuses a revision one below and one above the current one, and no stale
  activation reaches the service. A foreign instance, a text node, a missing
  node, and a disabled control are refused too.
- Disabled controls on every screen and platform, and with the unavailable
  service, are followed by a nonempty reason.
- A host refusal (`missing_right`) is shown and stores no invitation.
- Invitations only narrow, and the grant expiry never outlives this device's
  own grant. Approval is limited to the owner or an `access_admin` device and
  grants the intersection of the request and the approver's rights.
- SSH and **Run with no local host** are refused on phones with a reason.
- First run keeps **Continue** disabled until one computer has a grant.
- Activity keeps the newest summary per subject, orders attention first,
  marks summaries from hosts that aren't online, and shows no redacted
  content.
- The terminal adapter draws the same tree and activates **Continue** by
  keyboard; the same key press against the old revision is stale.

`cargo run -p coder-computers --example terminal -- --print` prints every
screen from the fixture. The Computers screen reads:

```text
[ Computers ]  [ Add a computer ]  [ Activity ]
Computers
Studio Mac
Online over the local network. Up to date.
[ Switch off ]  [ Access ]  [ Forget ]
Build server
Connecting.
[ Switch off ]  [ Access ]  [ Forget ]
Home NAS
Offline: the computer didn't answer. Retrying automatically.
[ Switch off ]  [ Try now ]  [ Access ]  [ Forget ]
Old laptop
Out of date: the computer runs an older Coder. Update Coder on the computer.
[ Switch off ]  [ Access ]  [ Forget ]
Lab box
Not enrolled: this device has no access. Add it with an invitation.
[ Switch off ]  [ Access ]  [ Forget ]
Former work PC
Revoked: this computer removed this device's access. Add it again with a new invitation.
[ Switch off ]  [ Access ]  [ Forget ]
Travel mini
Offline: switched off. It stays in your list.
[ Switch on ]  [ Access ]  [ Forget ]
[ Add a computer ]  [ Refresh ]
```

## Simulator and emulator checks

These use the synthetic fixture. They check native mounting, activation,
scrolling, and text input, not a real host, relay, camera, or accessibility
service.

| Platform | Build | Test | Result |
| --- | --- | --- | --- |
| iOS Simulator, iPhone 17 Pro, iOS 26.5 (a dedicated simulator created for this run and deleted afterward) | `scripts/build-coder-mobile.sh sim-build`: succeeded. | `xcodebuild test -only-testing:CoderUITests/ComputersUITests -only-testing:CoderUITests/ReaderUITests/testNativeChatSelectionAndExactSource` | Both passed (76 s and 15 s). |
| Android emulator `coder_mobile_api35`, API 35, arm64-v8a | `scripts/build-coder-android.sh package`: succeeded. | `connectedDebugAndroidTest` filtered to `computersScreensShowStatusesAccessAndPastedInvitation` and `readerPagesStayPinnedAndExactRecordsRemainAccessible` | Both passed. |

Both UI tests open the world computer, open **Computers**, pass first run
back to the chats flow, read all six statuses, confirm that revoking this
device is disabled with its reason, switch a host off, confirm that SSH is
disabled on a phone with its reason, paste a `coder-host:` invitation into
the native field, and open **Activity**. The reader tests confirm the
existing chat reader still works beside the new surface.

Screenshots:

- [iOS Simulator: statuses](2026-09-26-computers-screens/ios-simulator-statuses.png)
- [iOS Simulator: access](2026-09-26-computers-screens/ios-simulator-access.png)
- [Android emulator: statuses](2026-09-26-computers-screens/android-emulator-statuses.png)

The full iOS UI suite and the full Android instrumentation suite did not run;
only the tests named above did.

## Physical-device checks

None. No physical iPhone or Android device ran these screens, and nothing
was uploaded to TestFlight or a store.

## Limits

- No host client implements `ComputersService` yet. The normal mobile app
  shows every Computers action as unavailable with its reason. The resident
  host client from #9712 is expected to implement the trait over
  `coder-access`, `coder-reach`, and a `coder-link` registry.
- `DeviceRow::last_seen` has no source in NIP-HOST's `device.list` today; the
  screen says "last seen: unknown" when a service does not supply it.
- A created invitation shows as its `coder-host:` string. Phones do not draw
  it as a QR code.
- Desktop has no mount yet; the terminal adapter slice runs through the
  example, not the `coder` terminal client.
- The escaped HTML adapter from RN1 is not part of this change.
- VoiceOver, TalkBack, dynamic type, and physical camera scanning into the
  Computers input were not checked.
