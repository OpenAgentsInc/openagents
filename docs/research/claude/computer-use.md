# Computer use, devices, and the simulator

Computer use is a stack of small windows over a permission grant, plus a
Swift addon that can see the screen and drive accessibility. The addon and
the `app-cu-helper` binary are described in [native modules](native.md). This
chapter is what the JavaScript asks them to do. No screen recording was
taken and no accessibility grant was requested.

## Grants

`ComputerUseTcc` is the grant service:

- `getState`
- `requestAccessibility`
- `requestScreenRecording`
- `requestMicrophone`
- `revokeGrant` with a `bundleId`
- `listInstalledApps`
- `openSystemSettings` with a `pane`

Those three requests match the usage strings in the app's Info.plist for
AppleEvents, microphone, and screen capture. `revokeGrant` takes a bundle id
because the grant is per app the helper might drive, not a single global
bit. `getAppIconForBundleId` on the local-agent session service is how the
recording UI draws that app.

`WindowState.cuDockStateChanged` tells the page when the computer-use dock
is showing. Log tags for the pieces of that UI: `cu-glow`, `cu-esc`,
`cu-side-panel`, `cu-notifications`, `cu-app-indicator`, `cu-teach`,
`cu-live-preview`, `cu-hipaa`. `cu-glow.html` is a document name in the main
chunk. HIPAA is a tag only. No policy text was attached to it in the slice
that was read.

## Teaching and the watch-record pill

`computerUseTeach.js` is one preload that exposes three services, all under
`claude.internal.computerUse`, and only on the top frame.

`CuTeach` is the walkthrough. The page calls `next`, `exit`, `mouseEnter`,
`mouseLeave`. The host calls back `show`, `working`, `hide`, and
`reassertHover`. Reasserting hover is how the overlay gets the pointer back
after the host has moved it.

`CuWatchRecordPill` is the recording chip. The page calls `done`, `discard`,
`toggleMic`, and `reportProcessingWidth`. The host calls back `micState`
(two arguments), `processing`, `timeWarning` (one argument), and `micError`.

`CuWatchRecordChooser` is the sheet that picks a source before recording.
`choose` takes an argument, `setMicPreview` takes an argument,
`stopMicPreview`, `reportContentShortfall`. The host pushes `micState` and
`inputDevices`. The window is created once. A second attempt logs
`chooser already open, refusing` and does not open another. The document is
a temp `index.html` under a `wr-chooser-` directory. The creation site names
both `record` and `skill` as the kinds of choice.

The session service that receives the result is `LocalAgentModeSessions`.
`startWatchRecording` and `takeHeldWatchRecording` start the hold.
`watchRecordingComplete` and `dismissWatchRecordPill` end the pill's view of
it. Copper Heron is the held recording as data: summary, thumbnails, a
frame, a touch, finalize, or discard. `copperHeronHoldDropped` is the event
when the hold goes away without a finalize. `releaseRemoteCuLock` and
`onCuEmergencyStop` are the stop paths. Emergency stop is an event, not a
method the page calls.

## Sable Otter

The same teach preload exposes `claude.internal.sableOtter.SableOtterHud`,
and only when the top frame is `file:`. The methods are a composer:

- `button`, `submit`, `fieldChanged`, `paste`, `escape`, `tab`, `removePill`,
  `settings`, `resize(width, height)`
- host pushes `render` and `levels`

`resize` is how the HUD tells the panel its content size. `levels` is the
host pushing audio levels back. A deep link `claude://sable-otter?k=` does
not open this window itself. The main-process handler loads a chunk and calls
`composeFromOs(k)`, which is the OS-side entry that can lead to the HUD.
The HUD file and that deep link were not executed together.

## Live preview and a remote framebuffer

`ComputerUseLivePreview` is `setFps`, `setSubscribed`, and two event
channels, `browserPreview_` and `computerUsePreview_`. Subscribing is
explicit. The page sets a frame rate and a boolean.

`FramebufferPreview` is the input path for a remote screen, not a
screenshot API:

- `listSources`, `attach`, `detach`, `requestFramePort`, `setStreamHints`
- `sendPointer`, `sendKey`, `sendScroll`
- events `sessionResized`, `sessionFatal`, `openSourceRequested`

`requestFramePort` matches the main view's `framebuffer:port` literal. The
port is a `MessagePort` carrying frames. Pointer, key, and scroll are
invokes, so each input is a round trip rather than a write on that port.
`sessionFatal` is how the page learns the stream died.

`BuddyRemoteFeed` is the small sibling for a paired device's feed:
`permissionDecision` and `sync`. It is not the pairing window.

## The simulator

`Simulator` on `claude.simulator` has 64 methods. They split into a device
session and a native view.

Session: `listDevices`, `attach`, `detach`, `checkSetup`, `checkAndroidSetup`,
`openXcode`, `openAndroidStudio`, `openAppStore`, `consentAskShown`.
Appearance and display: `getAppearance`, `displayPointsPerInch`,
`displayPointsPerInchChanged`, `getSlowAnimations`, `hingeAngle`.
Input: `gesture`, `keyboardShortcut`, `getClipboard`. Capture:
`captureScreenshot`, `captureShortcut`, `captureThumbnailAction`,
`deleteCapture`, `describeAccessibility`. Logs: `getLogEntries`, `logEntries`,
`modelActivity`. The `attachment_` and `debugAttaching_` names are event
channels.

The native view is a foldable surface embedded in the window:
`nativeViewMount`, `nativeViewUnmount`, `nativeViewSetBounds`,
`nativeViewSetVisible`, `nativeViewSetOrientation`, `nativeViewSetBend`,
`nativeViewFold`, `nativeViewFoldGo`, `nativeViewFoldPlay`,
`nativeViewFoldConfirm`, `nativeViewFoldEnd`, `nativeViewStill`. Bend and
fold match a device with a hinge. `SimulatorPolicy` is the main-process log
tag for the rules around that view. The scheme `claude-simulator` is
registered privileged, with streaming, CSP bypass, fetch, and CORS, which is
how a frame in the page can show the device without proxying every pixel
through IPC.

The iOS helper and its seatbelt profile are in [the bundle chapter](bundle.md).
The JavaScript service does not name that binary. The connection is the
shared word "simulator" plus `openXcode` and `checkSetup`. It was not traced
as a `spawn` of `claude-ios-sim`.

## Buddy, the pairing window

`buddy.js` exposes `claude.buddy.Buddy` on the Buddy window:

`status`, `deviceStatus`, `setName`, `scanDevices`, `pickDevice`,
`cancelScan`, `pairDevice`, `submitPin`, `forgetDevice`, `pickFolder`,
`preview`, `install`, `onProgress`, `onPairingPrompt`.

That is a pair, name, PIN, pick a folder, preview, install, forget. Progress
and the PIN prompt are events. The preload also exposes `DesktopIntl`, a
reduced `process`, `buddy.getPathForFile`, and
`desktopEssentialTelemetryDisabled`, which is true only when the managed
config JSON says `disableEssentialTelemetry === true`.

Bluetooth is a second object, `buddyBle`, installed by the main view rather
than by the Buddy window. It is a Web Bluetooth client. The service UUID is
`6e400001-b5a3-f393-e0a9-e50e24dcca9e`, RX is `6e400002-…`, and TX is
`6e400003-…`. Those are the Nordic UART service UUIDs. The script does not
use the name Nordic. That identification is inferred from the constants. The
page sees `buddyBle.pair` and `buddyBle.disconnect`.
States pushed across the bridge are Ready, Connected, and Disconnected. The
IPC type is `BuddyBleTransport` with `log`, `reportState`, `rx`, and `tx`.
This is the cable-free serial path. It is not the `Buddy.pairDevice` flow.
Both can be present. The preload does not say which devices speak UART and
which go through the pairing window.

Swift groups on `swift_addon.node` that line up with this chapter, loaded
lazily on first property access: `notifications`, `desktop`, `quickAccess`,
`midnightOwl`, `permissionFixer`, `tearOffHalo`, `sessionHalo`, `slackWatch`,
`wakeScheduler`, `updater`, `keychain`, `trayUsage`, `watchRecorder`,
`memoryPressure`, `osEntryPoints`, `simulatorView`, and `vm`. `computerUse`
is a different native file, `computer_use.node`, and a missing or corrupt
file fails soft. `vm.isRunning` and `vm.isGuestConnected` are forced to
return a Promise so they match the Windows client. The public build does not
include the groups the loader marks nest-only. The loader's own comment says
flavor-gated groups must not be named in this file and stay behind
`nativeGroup()`.

`WakeScheduler` on the settings world is `getStatus` and `openSettings`, the
page's view of the `wakeScheduler` Swift group. `GrandPrix` is a pairing
flow with a verification code: `pair`, `request`, `submitVerificationCode`,
`dismissVerificationCode`, `disconnect`, `grandPrixStatus_`. It is not Buddy
and not Bluetooth. Nothing in the preload says what device Grand Prix pairs.
