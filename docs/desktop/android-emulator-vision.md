# Android Emulator in OpenAgents Desktop (vision)

Status: Vision, after the iOS Simulator ships (2026-11-20; iOS is GitHub #13811)
Builds on: [ios-simulator-spec.md](ios-simulator-spec.md). Read that first: this document says what
is the same, what differs, and how the Android side is built. Where it says "as iOS", the iOS spec's
section applies unchanged.

## 1. The picture

The same pane on the window's right shows a live Android Emulator (an AVD) next to the conversation.
The person clicks, drags, types and presses Back, Home and Overview on it; the agent lists, boots,
screenshots, taps, swipes, types, presses keys, installs and launches an APK, builds a Gradle project
and runs it. The pane's tab says "Pixel 9 · API 35". A chat that works on both platforms can show
either device in the pane, chosen in the device menu, which lists iOS simulators and Android AVDs
together.

The agent's tools are the same tools. `simulator_control` and `simulator_build` take a device that is
an iOS UDID or an Android serial or AVD name; the verbs, coordinates (device points, top-left) and
reply shapes are the same. A prompt that drives an iPhone drives a Pixel.

Claude Desktop ships the same idea (`.vite/build/emulator-grpc-worker/emulatorGrpcWorker.js` in its
`app.asar`): it reads the emulator's discovery file and token, connects to the emulator's gRPC
controller, and pushes raw RGBA frames into the same native view its iOS panel uses.

## 2. Why Android is easier and where it is harder

Easier: everything goes through **public, documented interfaces**. The emulator ships a gRPC
service (`emulator_controller.proto`, on this Mac at `~/Library/Android/sdk/emulator/lib/`, emulator
37.2.12) that does screenshots, a screenshot stream, touch, mouse, keys, rotation, display and VM
state; `adb` does install, launch, logcat and the UI tree. No private frameworks, no Xcode coupling,
no Seatbelt fight over Mach services.

Harder: frames are not an `IOSurface` we can hand to a layer. The emulator renders in its own
process (QEMU plus its GPU translation); we get pixels through gRPC (copied), or through a shared
memory region the emulator writes into (`ImageTransport.MMAP`). And the emulator needs a window-less
launch (`-no-window`), its gRPC port and its token.

## 3. Architecture

```
 OpenAgents.app ──(same pane, SimulatorPaneView)── frames: IOSurface (iOS) | shm RGBA -> IOSurface (Android)
       │ sim-wire lines (same socket, same ops)
 oa-simhost ── iOS: CoreSimulator (as the iOS spec)
            └─ Android: AndroidBackend
                 emulator lifecycle: `emulator -avd NAME -no-window -grpc-use-token -no-boot-anim ...`
                 gRPC client: EmulatorController (HTTP/2 over loopback, token in metadata)
                 adb: install, am start, uiautomator dump, logcat
 oasim ── the same verbs; build: Gradle (`./gradlew :app:assembleDebug`) instead of xcodebuild
```

**One helper, two backends.** The Android backend lives in the same `oa-simhost` process (a second
Swift backend behind the same Coax core), so the app has one socket, one subscription model, one
grant store and one policy. The backend is chosen by the device id: a UDID is iOS, an `emulator-5554`
serial or an AVD name is Android. If the iOS helper's confinement gets in the way of starting the
emulator (it must `exec` the emulator and connect to loopback TCP), the Android backend runs as a
second helper process with its own profile; the wire and the app do not change.

### 3.1 Lifecycle

- **List:** `emulator -list-avds` (AVDs) plus running emulators from their **discovery files**:
  `pid_NNNN.ini` under the emulator's running folder (on macOS
  `~/Library/Caches/TemporaryItems/avd/running/`, confirm per emulator version), which carry
  `grpc.port`, `grpc.token` (with `-grpc-use-token`), the console and adb ports and the AVD name.
  `adb devices -l` adds serials and states. Physical devices that `adb` lists are **never** offered
  (serials that are not `emulator-NNNN` are filtered out by the policy, as iOS refuses anything that
  is not a simulator).
- **Boot:** `emulator -avd NAME -no-window -no-audio -no-boot-anim -grpc-use-token -gpu host`
  (`-gpu swiftshader_indirect` when the host GPU path fails), started in its own session, its output
  in `~/.openagents/sim/android/NAME.log`. Ready when the discovery file appears, gRPC `getStatus`
  answers `booted`, and `adb shell getprop sys.boot_completed` is 1. Timeout 240 s. An emulator the
  person started with a window is attached, not restarted; its own window stays theirs.
- **Shut down:** `setVmState(SHUTDOWN)` (or `adb emu kill`) only for emulators OpenAgents booted, as iOS.
- **Snapshots** (later): `snapshot_service.proto` `SaveSnapshot`/`LoadSnapshot` give instant boots
  and a "reset to clean" the agent can ask for, with the person's yes.

### 3.2 Frames

Two paths, chosen at subscribe:

1. **Shared memory (preferred).** The helper creates a POSIX shared memory region sized for the
   display (RGBA8888, `width * height * 4`), and calls `streamScreenshot(ImageFormat{format:
   RGBA8888, width: W, transport: {channel: MMAP, handle: "file:///..."}})`. Each streamed `Image`
   message then carries only `seq` and `timestampUs`: the pixels are in the region. The helper copies
   the region into an `IOSurface` it owns (one `memcpy` into a locked surface, or a Metal blit from a
   no-copy `MTLBuffer` over the region) and sends that surface to the app exactly as the iOS path
   does (Mach port, generation, tick pipe). The app's view is unchanged. The proto notes the mmap can
   tear; double-buffer two surfaces and flip on each `seq`.
2. **gRPC bytes (fallback).** `streamScreenshot` with RGBA8888 bytes in the message, at the pane's
   pixel size (`width` set to the view's backing width so the emulator scales for us), into the same
   `IOSurface` pair. Fine at 30 fps for a phone-size view.

The image's rows are bottom-up ("left to right and bottom up" in the proto); the copy flips. The
`Image.format.rotation` tells the orientation; a change is a new surface generation with swapped
points, as an iOS rotation is. Foldables report `foldedDisplay`; v1 shows the unfolded display.

**Screenshots for the agent:** `getScreenshot(ImageFormat{format: PNG, width: POINT_WIDTH})`, where
POINT_WIDTH is the display's width in density-independent pixels (dp), so a pixel in the image is a dp
the agent can tap, the same promise as iOS's 1x points.

### 3.3 Input

The gRPC controller takes input directly:

| Need | RPC | Notes |
| --- | --- | --- |
| Touch, multi-touch | `sendTouch(TouchEvent{touches:[Touch{x,y,identifier,pressure}]})`, or the client stream `streamInputEvent` | pressure 0 ends a touch; x/y in device pixels: the helper maps dp (agent) and view points (person) to pixels |
| Mouse (person) | `sendMouse` | used only for a pointer-style AVD (Chromebook, desktop); phones get touches |
| Wheel | `injectWheel` | the person's scroll wheel; agents swipe |
| Keys | `sendKey(KeyboardEvent{codeType, key, eventType})` | `key` names like "Enter", "Backspace", "GoBack", "GoHome", "AppSwitch"; text via `KeyboardEvent.text` for ASCII, else `adb shell input text` / clipboard (`setClipboard`) plus paste |
| Hardware buttons | `sendKey` with Power, VolumeUp/Down; Back, Home and Overview are Android's own | the iOS `button` names map: HOME to Home, LOCK and SIDE_BUTTON to Power; Android adds BACK and OVERVIEW |
| Rotate | `setPhysicalModel` (rotation) or the emulator's rotate | as iOS `rotate` |
| Light/dark | `adb shell cmd uimode night yes/no` | as iOS `set_appearance` |
| Open a URL | `adb shell am start -a android.intent.action.VIEW -d URL` | |
| Install / launch | `adb install -r APK`, `adb shell monkey -p PKG 1` or `am start -n PKG/ACTIVITY` | install rule as iOS: APKs under the working folder or our Gradle outputs |
| UI tree (`describe`) | `adb exec-out uiautomator dump /dev/tty` (XML) parsed to the same `[{role,label,value,frame}]` in dp | Android gets `describe` from day one, before iOS |
| Logs | `streamLogcat` (gRPC) or `adb logcat` | as iOS `logs` |
| Clipboard, GPS, battery, SMS, calls, sensors | `setClipboard`, `setGps`, `setBattery`, `sendSms`, `sendPhone`, `setSensor` | later ops, read-only first, each a policy line |

The person's input wins, as iOS (4.3 there): a person's touch-down cancels the agent's gesture.

### 3.4 gRPC from our code

The helper talks HTTP/2 to `127.0.0.1:grpc.port` with `authorization: Bearer TOKEN`. We have no
gRPC library in `coax/lib`, and we shouldn't take a C++ gRPC dependency. Two options, in order:

1. **A small gRPC-over-HTTP/2 client in Coax** (`coax/lib/net/h2.coax` for framing and HPACK, the
   unary and server-streaming call shapes only, plaintext loopback; protobuf through `#import_proto`
   of the emulator's `.proto`s, vendored under `proto/vendor/android-emulator/` with their licence,
   registry rows `kind: external` and the codec folder as the anticorruption layer). This is the
   Coax-first answer, is testable with recorded frames, and becomes reusable.
2. **Swift `URLSession` HTTP/2** as a thin transport (Firm reason 6), with the protobuf messages still
   encoded and decoded by the Coax codec. Faster to land; keep it behind the same interface so (1)
   can replace it.

The Android console (telnet on the console port, `auth` token in `~/.emulator_console_auth_token`)
is not used: gRPC covers it.

## 4. What is shared with iOS, exactly

| Part | Shared | Android-specific |
| --- | --- | --- |
| Pane (`page/simpane.coax`) | All of it: tab, toolbar, states, badge, permission card, covered/stills, rect | Toolbar shows Back/Home/Overview instead of Home/Lock; the tab says the API level |
| View (`SimulatorPaneView.swift`) | All of it: an `IOSurface` layer, ripples, mouse and keys to ops | — |
| Wire (`sim-wire.coax`, `simhost-lines-v1`) | Every op and event | `devices[].platform: "ios"|"android"`, `api_level`, `abi`; `button` names BACK, OVERVIEW, POWER, VOLUME_UP, VOLUME_DOWN (a new minor version of the profile, never a change in place) |
| Policy (`sim-policy.coax`) | Grants, rate limits, person wins, contested, install rule | Only `emulator-NNNN` serials; APK paths; no `adb root`/`remount`/shell ops for agents beyond the listed ones |
| Gestures (`sim-gesture.coax`) | Mapping and path expansion | dp to pixel by the display density |
| Tools (`oasim`, MCP, Microcoder `device`) | Same verbs and replies | `build` runs Gradle |
| Helper | Socket, grants, subscriptions, the surface hand-off | `AndroidBackend` (lifecycle, gRPC, adb, shm) |

## 5. Build and run (Gradle)

`build` with `project_path` pointing at a folder with `gradlew` (absolute), `task?` (default
`:app:assembleDebug`), `module?`:

- Runs `./gradlew TASK --console=plain` in that folder, detached, log and state as the iOS builds
  (`~/.openagents/sim/builds/ID.*`), with `JAVA_HOME` from Android Studio's bundled JBR when the
  person has none.
- `build_status` parses `e: file:///...:LINE:COL message` (Kotlin), `error:` (javac) and AAPT errors
  into the same `errors` list, and finds the APK under `MODULE/build/outputs/apk/debug/*.apk`.
- `launch` with `app_path` (the APK): `adb -s SERIAL install -r APK`, read the package and launcher
  activity with `aapt2 dump badging` (or `apkanalyzer`), then `am start`.
- We do not run Gradle tasks the project defines beyond the one asked for; a first Gradle run that
  wants to download a distribution or dependencies says so in its status (network is the build's,
  not the helper's).

## 6. Permissions and safety

As iOS (9 there), plus:

- Never a physical device: a serial that is not an emulator's is invisible to every op.
- No `adb shell` passthrough for agents. Each Android op is a fixed command line the helper builds
  from typed arguments; an agent that wants a shell runs its own commands, gated like any command.
- The emulator's gRPC token is read from the discovery file by the helper only and never printed,
  logged or passed to a worker.
- `-grpc-use-token` always for emulators we start; an emulator started without a token (plain
  `-grpc PORT`) is attached only after the person allows it, since anything local could drive it.
- Snapshot load ("reset the device") is an agent op only with the person's yes per time.

## 7. Failure modes

| What happens | Seen / returned |
| --- | --- |
| No Android SDK (`ANDROID_HOME`, `~/Library/Android/sdk`) | "Android SDK not found" with where to install; `no_sdk` |
| No AVD | "No Android virtual device; create one in Android Studio's Device Manager" (later: create one with `avdmanager` with the person's yes) |
| Emulator too old for `-grpc-use-token` or the MMAP transport | Falls back to gRPC bytes, then to `adb exec-out screencap -p` stills; `capabilities` say which |
| Hypervisor unavailable / emulator exits at boot | The emulator log's last lines in the notice |
| Discovery file missing for a running emulator | Attach by `adb` only: stills and `adb shell input` for touches (slower), and the pane says so |
| `adb` server not running or a different `adb` version | The helper starts its own `adb start-server` from the SDK's `platform-tools` |
| Port clash | The emulator picks the next pair; the discovery file names it |

## 8. Phases

1. **A0 — adb only.** List, boot (`-no-window`), stills via `adb exec-out screencap -p`, `adb shell
   input tap/swipe/text/keyevent`, install, launch, `uiautomator` describe, Gradle build. Quick, slow
   frames, works on any emulator.
2. **A1 — gRPC.** The gRPC client (3.4), live frames over MMAP into the shared `IOSurface` view,
   input over `sendTouch`/`sendKey`, `getStatus`, logcat stream. This is the Android counterpart of iOS
   Phase 1.
3. **A2 — the rest.** Snapshots, clipboard, rotation and folding, sensors and GPS as typed ops, a
   device manager in the pane (create an AVD with the person's yes).

Tests mirror iOS (11 there): codec fixtures for the emulator messages we use (recorded with the
person's own emulator, tokens scrubbed), policy mutation, gesture checks in dp, a helper check that
creates its own AVD (`avdmanager create avd -n oa-check-RAND ...`), boots it with `-no-window
-read-only`, drives it, and deletes it, never touching the person's AVDs (`coder_mobile_api35`,
`everglade_api35` and the rest stay theirs).

## 9. Open questions

1. Whether the Android backend can share the iOS helper's Seatbelt profile (it needs `process-exec`
   of the emulator and loopback TCP) or runs as its own helper process.
2. The Coax HTTP/2 client: a desktop-only module or the start of a general one in `coax/lib/net/`.
3. Whether to ship our own copy of the emulator's `.proto` files or read the SDK's at build time
   (the registry prefers a vendored, pinned copy).
