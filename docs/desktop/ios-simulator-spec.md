# iOS Simulator in OpenAgents Desktop

Status: Spec, ready to build (2026-11-20; GitHub #13811)
Scope: `apps/openagents-desktop/` (Swift host and Coax page), a new helper `apps/openagents-desktop/simhost/`,
a new library `coax/lib/agent/device/`, a new tool `coder/tui/oasim.coax`, `apps/openagents-cli/`,
`coax/lib/microcoder/`, `coder/tui/claudeworker.coax`, `proto/`.
Android follows the same design: [android-emulator-vision.md](android-emulator-vision.md).

## 1. What we are building

A live iOS Simulator inside OpenAgents Desktop, in the pane on the window's right. Two parties use it:

- **The person** watches it, clicks and drags on it as touches, types into it, presses Home, Lock and
  the volume buttons, rotates it, picks the device and boots or shuts it down. No `Simulator.app`
  window opens.
- **The agent** (Microcoder, a Claude sub-agent, any worker) sees and drives the same device: list,
  boot, screenshot, tap, swipe, a touch path, a two-finger path, type text, press keys and hardware
  buttons, set light or dark, rotate, open a URL, install and launch an `.app`, and build an Xcode
  project for the simulator and run it. The person sees each agent action happen live in the pane.

The reference is Claude Desktop's built-in iOS Simulator panel. The parts of its design we keep and
the parts we change are in section 2. Everything else in this document is how it fits our app: the
Coax page draws the pane, the Swift host does platform effects only, the rules and the wire are Coax.

### Goals

1. 60 fps live display with no copy of the frame through the page: the simulator's own framebuffer
   (`IOSurface`) is the contents of a layer placed over the page's pane rect.
2. Input from the person in under one frame of added latency: a click is a touch on the device.
3. One tool surface for every engine, with the same verbs and argument names as Claude's simulator
   tool (`control`, `build`), so prompts and skills carry over.
4. Agent runs outlive the app (README, "Runs outlive Coder"): an agent can drive a simulator while
   the window is closed; the pane shows it again when the person opens the chat.
5. The person stays in charge: agent control of a device needs the person's yes once per device and
   chat, the pane always says when an agent is driving, and one press stops it.
6. Never disturb what the person already has: no focus stealing, no `Simulator.app` window, no
   shutting down a device the person booted, no fighting `apps/openagents-ios/dev.sh` for a device.

### Non-goals

- Physical iPhones or iPads. Only simulators.
- A device-frame skin (bezel) and the fold/perspective animations Claude draws. The pane shows the
  screen with rounded corners.
- tvOS, watchOS, visionOS simulators (the helper rejects them in v1; the design does not block them).
- Remote viewing from OpenAgents Mobile (later; section 13).

## 2. What Claude Desktop does, and what we take

Read-only inspection of `/Applications/Claude.app` (v6.6395.1), the helper
`Contents/Helpers/Claude iOS Sim.app/Contents/MacOS/claude-ios-sim` and `app.asar`:

| Claude Desktop | What it is | Ours |
| --- | --- | --- |
| `claude-ios-sim`, a Swift universal binary in an `LSBackgroundOnly` helper app, with Meta's idb licence beside it | Owns the CoreSimulator session out of process. Built on idb's `FBSimulatorControl` approach (CoreSimulator and SimulatorKit private frameworks) | Same split: `oa-simhost`, our own Swift helper (section 4). We read idb's public, MIT-licensed approach; if any idb code is ported, its licence ships beside the helper as Claude does |
| `CoreSimulator.framework` (`/Library/Developer/PrivateFrameworks`), `SimulatorKit.framework` (`SimDisplayIOSurfaceRenderable`, `SimDisplayDescriptorState`) | Device set, boot, the display port and its `IOSurface` | Same |
| HID through `SimulatorIndigoHID` (`IndigoDigitizerEvent`, Purple workspace port) and, on newer Xcode, CoreDevice's remote HID (`com.apple.coredevice.feature.remote.hid.digitizer`) | Touches, keys, hardware buttons, hinge poses | Same two paths, probed at start (section 6.3) |
| `SimulatorFrameworkBridge-iOS`, a guest binary run inside the simulator, plus `AccessibilityPlatformTranslation` | A fast accessibility tree of the device's UI | Phase 3 (`describe`); we start with CoreSimulator's own accessibility translation and add a guest only if it is too slow |
| `IOSurface` Mach port sent to the Electron main process over a per-launch bootstrap name (`...ios-sim.surface.PID.SEQ.RAND`), peer pid checked; an AppKit `NSView`/`CALayer` behind a transparent web page with "hole" cut-outs | Zero-copy display | Simpler for us: our page is already drawn by a native Metal view, and the app already places native views over a page rect (`PleScenePanelView`, `GymWorkbenchView`). The simulator view goes **over** the canvas at the pane's rect, no hole punching (section 5) |
| VideoToolbox H.264 to WebCodecs `<canvas>` fallback | When the native view can't mount; recordings | Not needed for display (we have no web layer). Our fallback is a still-image view fed by `simctl io screenshot` (Phase 0) |
| `disclaimer` plus `sandbox-exec` with a Seatbelt profile; RPC over stdin/stdout | The helper is TCC-disclaimed, cannot read `$HOME` file contents or reach the network | We take the confinement (section 9.4). The RPC is a local socket, not stdio, because agent tools in other processes must reach the same session |
| `hideSimulatorApp` | Keeps `Simulator.app` hidden | We never launch `Simulator.app`; booting through CoreSimulator is headless |
| MCP tools `control` (attach, launch, screenshot, tap, swipe, touch_path, touch2_path, text, button, press_key, set_appearance, appearance, set_keyboard, open_url, detach, rotate, posture...) and `build` (build, build_status) | The agent's surface | Same verbs and argument names (section 7) |
| Android: `emulator-grpc-worker` reads the emulator's discovery file and token, raw RGBA into the same native view | Android | Our vision doc follows the same idea |

## 3. Where it fits in our app today

What exists that this builds on (read before changing anything):

- **The right-side pane.** `page/layout/layout.coax` `cp_side_pane` gives the window's right side to
  the file pane (`page/filepane.coax`), the code review (`page/layout/review.coax`) or a sub-agent's
  transcript (`page/layout/pane.coax`), with `app_shell_layout` doing the widths and the slide.
- **Native views over a page rect.** The file pane's `CP_FP_SCENE` mode reports its body rect
  (`coder_page_file_scene_rect`, `cd_file_scene_rect` in `host.c`) and `Window+Files.swift`
  `syncScenePanelFrame()` keeps a Metal `PleScenePanelView` over it, converting the page's top-left
  logical points into AppKit's bottom-left frame; `Window.swift` calls the sync every frame. Gym does
  the same (`cd_gym_rect`, `Window+Gym.swift`). The simulator view is a third user of this pattern.
- **The page draws images.** `components/paint/list.coax` `prim_image`, so the pane can draw a still
  of the screen when the native view must be hidden.
- **Workers and the daemon.** Workers run under the daemon (`coder/tui/host-runs.coax`), so they live
  after the app quits. Microcoder's tools are its JSON action (`coax/lib/microcoder/action.coax`):
  `commands`, `view`, `edits`, `delegate`, `todos`. A `view` of a `.png`/`.jpg` attaches it to the
  next model call as an image (`coax/lib/microcoder/run.coax` `mc_run_view`, `image.coax`: at most 4
  images, 512 KB each raw).
- **The worker wire.** Every worker line goes through `coax/lib/agent/engine/mcworker-wire.coax`
  (registry row `microcoder-worker-lines-v1`). The `delegate` / `results` ticket pair is the model
  for a request the worker makes of its owner and waits on.
- **MCP.** `apps/openagents-cli/mcp.coax` (`openagents mcp`) already serves tools over stdio and
  answers with PNG images; it accepts only `--domains realm` today. Claude sub-agents get options
  through `coax/lib/agent/claude/options.coax` (`--mcp-config`, `--settings`), and
  `claudeworker.coax` already finds a tool beside itself (`gitguard`).
- **Simulator etiquette.** `apps/shared/app/preview/loop.coax` decides whether a simulator is
  contested (another `dev.sh` holds `~/.openagents/ios-dev/sim-NAME.lock`, an `xcodebuild` or a
  `simctl install`/`launch` targets it) and parses `simctl list`. `apps/shared/cases.coax` launches
  with `simctl`. These move into the shared library (section 10) rather than being copied.
- **Driving the app.** `docs/desktop/macos-drive-and-screenshots.md`: a native sibling view is missed
  by the page's own read-back; the drive's `shot` must composite the simulator view (section 11).

## 4. Architecture

```
 ┌──────────────────────── OpenAgents.app (main process) ─────────────────────────┐
 │ Coax page (page/simpane.coax)        Swift host (Window+Simulator.swift)        │
 │  draws the pane: tab, device menu,    SimulatorPaneView: CALayer.contents =      │
 │  buttons, state, agent badge;         IOSurface, kept over cd_sim_rect;          │
 │  reports the screen rect     ───────► mouse/keys -> touch/key ops                │
 │                                       SimHostClient: socket + surface port       │
 └───────────────▲───────────────────────────────┬─────────────────────────────────┘
                 │ events (state, surface gen,   │ ops (line JSON, simhost-lines-v1)
                 │ agent op, frame size)         │ + IOSurface Mach port (once per gen)
 ┌───────────────┴───────────────────────────────▼─────────────────────────────────┐
 │ oa-simhost  (Contents/Helpers/OpenAgents Simulator.app, LSBackgroundOnly)        │
 │  Swift effects: CoreSimulator device set, boot, display port IOSurface, HID      │
 │  Coax core (sim-core, generated C): wire codec, policy, gestures, key map        │
 │  socket ~/.openagents/sim/simhost.sock (0600, dir 0700), one per user            │
 └───────────────▲─────────────────────────────────────────────────────────────────┘
                 │ ops (same wire)
 ┌───────────────┴──────────────┐   ┌──────────────────────────────────────────────┐
 │ oasim (Coax tool, beside the │   │ workers under the daemon                     │
 │ workers in Resources/)       │◄──┤  Microcoder: `oasim ...` commands, later the │
 │  CLI verbs, `oasim mcp`,     │   │  "device" action member; Claude: MCP server  │
 │  build (xcodebuild, simctl)  │   │  `oasim mcp` in --mcp-config                 │
 └──────────────────────────────┘   └──────────────────────────────────────────────┘
```

### 4.1 Who owns the CoreSimulator session

**`oa-simhost` owns it**, one process per user, not the app and not the daemon:

- *Not the app:* the private frameworks crash or hang on Xcode changes; that must never take down
  the window the person is working in. The helper restarts; the app does not.
- *Not the daemon:* the daemon is Coax with no Objective-C runtime, and it must stay up for every run.
- *One per user:* the app's pane and every worker's tools must see the same booted device, the same
  surface and the same agent-control state.

Start and stop:

- A client (the app, or `oasim`) connects to `~/.openagents/sim/simhost.sock`. If nothing answers it
  starts the helper from its known path (the app's `Contents/Helpers/...`, or `OA_SIMHOST` for a
  worktree build) in a session of its own (`setsid`), as Coder starts the daemon, then retries for
  up to 5 s. A lock file (`simhost.lock`, `flock`) makes the start single.
- The helper exits after 10 minutes with no client connected and no agent grant live. It never shuts
  down a device on exit unless it booted that device itself **and** the setting "Shut down simulators
  OpenAgents booted when idle" is on (default on, 30 minutes idle).
- A newer app with an older helper running: the hello reply carries `version`; the client asks an
  older helper to `quit` (it finishes in-flight ops) and starts its own.

### 4.2 How frames reach the page

The page never sees pixels in the live path. The window shows the frame through a native layer:

1. The helper attaches to the booted device's main display port (`SimDeviceIOClient` ports whose
   descriptor is a `SimDisplayRenderable`), registers for its surface-change and damage callbacks,
   and holds the current `IOSurfaceRef`.
2. On `subscribe` from the app, and on each surface change (boot, rotation, display size change),
   the helper makes a Mach send right for the surface (`IOSurfaceCreateMachPort`) and sends it in a
   Mach message on a per-subscription rendezvous: the app picks a random service name
   `com.openagents.desktop.simhost.surface.PID.SEQ.RAND`, registers a receive right for it
   (`bootstrap_check_in` equivalent via `NSMachBootstrapServer`/`bootstrap_register2`), sends the name
   in `subscribe`, and the helper looks it up and sends `{gen, port}`. The app checks the sender's
   audit token is the helper's pid it spawned or found through the lock (as Claude checks `peerPid`).
   The socket event `{"event":"surface","gen":N,"w":1206,"h":2622,"scale":3}` arrives first, so the
   app knows the size before the port.
3. The app's `SimulatorPaneView` (layer-backed `NSView`) sets `layer.contents = IOSurface` (wrapped
   with `IOSurfaceLookupFromMachPort`), `contentsGravity = .resizeAspect`, rounded corners from the
   device's corner radius, and on each damage tick calls `layer.setContentsChanged()` from a
   `CVDisplayLink`/`CADisplayLink` callback only while the helper reports damage (so an idle device
   costs nothing). Damage ticks are one byte on a second, unbuffered "tick" pipe the helper writes
   per subscription (not JSON on the socket), coalesced to the display's rate.
4. The view sits over the page's canvas at the rect the page reports (`cd_sim_rect`), kept every
   frame as `syncScenePanelFrame()` does. The page draws everything around it: the tab, the toolbar,
   the state and the badge.

**Occlusion.** A page overlay that crosses the rect (a menu, the palette, a dialog, the model
picker, a toast, the drop overlay) must draw above the simulator, but a native view covers the
canvas. The page knows its overlays, so `coder_page_sim_rect` returns a flag `covered`. When covered,
the host hides the native view and the page draws the last still with `prim_image` (the host gives
the page a downscaled still each time the view is hidden: `IOSurface` to `CGImage` to the page's
image upload, at most 2 per second). When uncovered, the native view shows again. The same hide-and-
still happens while the pane slides (`app_presence` between 0 and 1) and during a window resize live
drag if the view lags.

**Fallback (Phase 0, and when the private display API is missing).** `SimulatorPaneView` shows a
`CGImage` instead of a surface, refreshed by `xcrun simctl io UDID screenshot --type=png -` (via
the helper, which reports `"display":"stills"` in its capabilities) at up to 4 fps while visible and
on each agent action. The pane says "Live view unavailable with this Xcode; showing stills."

### 4.3 How input goes back

- **Person:** `SimulatorPaneView` takes `mouseDown/Dragged/Up` (one finger), Option-drag (two fingers
  mirrored around the centre, as Simulator.app does), `scrollWheel` (a swipe whose length follows the
  delta, ended by a timer), and, while it is first responder, `keyDown/keyUp` and `flagsChanged`.
  The host maps view points to device points with the Coax mapping (`sim_map_view_to_device`: the
  aspect-fit inverse, clamped, rotation applied), and sends ops on the socket: `touch` with
  `phase` down/move/up and a pointer id, `key` with a HID usage and down/up, `button`. Moves are
  coalesced to one per display frame. Command-key shortcuts stay the app's (Command-V pastes into the
  device as `text`, Command-Shift-H is Home, Command-L lock, Command-Left/Right rotate, as in
  Simulator.app); every other key goes to the device while the view has focus. A click in the view
  makes it first responder; Escape or a click on the page gives the keyboard back to the composer.
  *As built (Phase 1):* one finger (`touch`, id 0, mapped by the page's `coder_page_sim_touch` with
  `sg_view_to_device`), and keys while the view has the keyboard: a named key by its Mac key code as
  `press_key`, a typeable character as `text` (`coder_page_sim_key`). Option-drag, the scroll wheel
  and the Command shortcuts into the device are not built yet; Command keys stay the app's.
- **Agent:** the same ops through `oasim`, plus composite ones the helper expands in Coax
  (`tap` = down+up; `swipe` = an eased path over `duration`; `touch_path`/`touch2_path` = sampled
  paths with `dt_ms`; `text` = per-character key events, or the pasteboard plus Command-V for long or
  non-ASCII text). The helper serialises the person's and the agent's events per device. **The person
  wins:** a person's touch-down cancels an agent gesture in flight (the agent's op replies
  `"interrupted":true`) and holds agent input for 1.5 s.

## 5. The pane (Coax page)

New part `page/simpane.coax`, loaded by `page/parts.coax`. State `CpSimPane` in `CoderPage`
(`state.coax`), like `CpFilePane`:

```
CpSimPane :: struct {
  open: bool; front: bool;
  devices: [CP_SIM_DEVICES] CpSimDevice; ndevices: int;   // from the helper's list (name, udid, runtime, state)
  pick: int;                    // the device shown, -1 none
  state: int;                   // CP_SIM_NONE, _NO_XCODE, _STARTING, _SHUTDOWN, _BOOTING, _BOOTED, _FAILED
  why: [512] u8; why_n: int;    // the last failure, as the helper said it
  agent: int;                   // CP_SIM_AGENT_NONE, _ASKING (permission card), _ALLOWED, _DRIVING (an op in the last 2 s)
  agent_op: [96] u8; agent_op_n: int;   // "Tapping (201, 640)", shown in the badge
  live: bool;                   // a surface is mounted (false: stills)
  screen_w: f32; screen_h: f32; // device points in the current orientation, for the aspect
  body_x: f32; body_y: f32; body_w: f32; body_h: f32;   // the screen rect, logical points, top-left
  still: int;                   // the page image of the last still, -1 none
}
```

Drawing (`cp_simpane`, from `cp_side_pane` when the simulator is in front):

- **Tab** (`app_pane_tab_shell`, as the file pane): a phone icon, the device name and runtime
  ("iPhone 17 Pro · iOS 26.1"), the state word (Booting, Shut down, Agent), and Close. Close hides
  the pane; the device keeps running.
- **Toolbar** under the tab: the device menu (a picker over `devices`, with "Boot", "Shut Down" and
  "Manage Devices…" which opens `simctl` device creation in a later phase), Home, Lock, Rotate,
  Screenshot (saves a PNG to the working folder's `.openagents/shots/` and opens it in the file pane
  or copies it, as a press says), and Light/Dark.
- **Screen**: the aspect-fit rect of `screen_w x screen_h` inside the remaining body, centred, with
  `CP_SIM_PAD` around it. Reported by `coder_page_sim_rect(out[5])` (x, y, w, h, covered). While no
  device is booted the page draws the state there: "No simulator booted" with a Boot button, "Booting
  iPhone 17 Pro…" with the working mark, "Xcode is not installed" with what to install
  (`xcode-select -p` and `xcrun simctl` answers), or the failure notice.
- **Agent badge**: a pill at the top of the screen rect's frame (drawn by the page above the native
  view's top edge, never over it): "Microcoder is using the simulator" with **Stop** (revokes the
  grant for this chat at once; the helper refuses that chat's next op). While `_DRIVING`, the native
  view also draws a touch ripple at each agent touch point (a `CAShapeLayer` the host adds, so it is
  over the surface), so the person sees where the agent tapped.
- **Permission card**: when an agent first asks to control a device in a chat, the pane opens (if
  closed), comes to the front and shows a card over the screen area: "Microcoder wants to control
  iPhone 17 Pro" with **Allow for this chat**, **Allow once** and **Don't allow**. Read-only ops
  (`list`, `screenshot`, `appearance`, `describe`, `build_status`) need no card.

Opening the pane:

- View > iOS Simulator (Option-Command-S) and the palette's "Show iOS Simulator".
- An agent op on a device (`attach`, or the first op after a grant) opens it in the chat it came
  from, unless the person closed it during this run, then the tab's dot says it is active.
- A press on a transcript row of a simulator op (section 7.5) brings it to the front.

Pane precedence (`cp_side_pane`): the one most recently brought to the front among file, review,
sub-agent and simulator, each with a `front` flag as the file pane has today, so a press on a
sub-agent row while the simulator is in front shows the sub-agent and the simulator comes back when
it closes. The pane's default width when the simulator is in front is the width that shows the
screen at 0.5 x device points plus the toolbar (about 420 points for a 402-point-wide phone), never
less than `CP_PANE_LEAST`; the person's own drag wins (`pane_user`).

**As built (Phase 0).** The page speaks the helper's wire itself: the host hands it every line the
helper writes (`cd_sim_line`) and writes what it queues (`cd_sim_out`), so grants, Stop, boot and the
stills are Coax. The host calls are `cd_sim_setup` (stills and shots folders), `cd_sim_show`,
`cd_sim_open`, `cd_sim_connected`, `cd_sim_line`, `cd_sim_out`, `cd_sim_tick` (4 Hz while shown),
`cd_sim_rect(out[5])`, `cd_sim_still`, `cd_sim_menu`/`cd_sim_pick` (the device menu is a native
`NSMenu`), `cd_sim_saved` and `cd_sim_device`; the page asks with one `CP_ASK_SIM` (46) and
`cd_sim_op` says which (menu, saved, closed, opened). The person's Screenshot is revealed in Finder.
A helper used by checks and test launches lists only `OA_SIM_ONLY`'s devices.

New host calls as planned (`host.h`/`host.c`, each a `coder_page_sim_*` export in `simpane.coax`):
`cd_sim_open`, `cd_sim_close`, `cd_sim_devices(json)`, `cd_sim_state(state, why)`,
`cd_sim_surface(w, h, live)`, `cd_sim_agent(state, op_text)`, `cd_sim_still(image_id)`,
`cd_sim_rect(out[5])`, `cd_sim_ask_permission(chat, device_name, engine)`. New page asks
(`coder_page_action`): `CP_ASK_SIM_BOOT`, `_SHUTDOWN`, `_PICK`, `_HOME`, `_LOCK`, `_ROTATE`,
`_SHOT`, `_APPEARANCE`, `_STOP_AGENT`, `_ALLOW_CHAT`, `_ALLOW_ONCE`, `_DENY`, `_CLOSE`.

## 6. The helper (`oa-simhost`)

**As built (Phase 0, #13811).** The helper is `oasim serve`: the same Coax program as the CLI,
its loop in `coax/lib/agent/device/sim-host.coax` (socket, clients, policy, grants, parked asks,
`simctl` jobs polled without threads). There is no Swift helper and no `sim-core` C unit: the
wire, the policy and the gestures are called directly from Coax. The private-framework effects
(live display, input) go in one Objective-C file, `simhost/shim.m`, compiled into `oasim` and
reached through `#foreign` calls from `sim-shim.coax` (a stub that reports `display: stills`,
`input: none` until it lands). It still runs as its own process, started detached by the first
client, so a CoreSimulator crash never takes the window down. The table below is the original
plan; read `Server.swift` as `sim-host.coax` and `Simctl.swift` as its `simctl` jobs.

The person: a connection whose `hello` says `"client":"app"` may send `grant`, `deny` and
`revoke` and is never asked for a grant. An agent's control op with no grant, while an app is
subscribed, is parked (two minutes) and the app hears `{"event":"ask",...}`; the person's
answer runs or refuses it. With no app connected it is refused `no_grant` at once.

`apps/openagents-desktop/simhost/`, Swift with an Objective-C shim where the private API needs it.
`Firm reason: 6 (CoreSimulator and SimulatorKit are private Objective-C frameworks)` on each file.
It links one generated Coax unit, `sim-core` (section 10), and does effects only.

### 6.1 Files

| File | What |
| --- | --- |
| `simhost/main.swift` | Arguments (`--socket`, `--home`), the lock, the confinement check, the run loop |
| `simhost/Server.swift` | The socket: accept, read lines, hand each to `sim_core_request` (Coax: decode, validate, policy), run the effect, write the Coax-encoded reply and events |
| `simhost/CoreSim.swift` | Load CoreSimulator from `/Library/Developer/PrivateFrameworks` and SimulatorKit from `$(xcode-select -p)/Library/PrivateFrameworks`; `SimServiceContext`, the default device set, devices, `boot`/`shutdown`, state notifications |
| `simhost/Display.swift` | The display port, the `IOSurface`, surface-change and damage callbacks, the Mach port hand-off, the tick pipes, stills (`IOSurface` to PNG/JPEG via ImageIO at a given scale) |
| `simhost/HID.swift` | The HID client and Indigo message builders (`dlsym` from SimulatorKit), or the CoreDevice remote HID path; touch, key, button, hinge |
| `simhost/Simctl.swift` | Public `simctl` for what it does well: `install`, `launch`, `terminate`, `openurl`, `ui appearance`, `io screenshot` (the stills fallback), `spawn log stream` (Phase 3) |
| `simhost/shim.h`, `shim.m` | Objective-C declarations of the private classes and the selectors we call, every call behind `respondsToSelector:` |
| `simhost/Info.plist` | `com.openagents.desktop.simhost`, `LSBackgroundOnly` |
| `simhost/sandbox.sb` | The Seatbelt profile (section 9.4) |

### 6.2 Private API we rely on (probe every one)

At start the helper resolves each class and selector and reports `capabilities` in its hello reply:
`{"display":"surface"|"stills","input":"indigo"|"coredevice"|"none","describe":false,...}`. A missing
piece degrades that capability; it never crashes. The set (as idb uses it; names can change between
Xcode releases, so `shim.m` is the one place that knows them):

- `SimServiceContext +sharedServiceContextForDeveloperDir:error:`, `-defaultDeviceSetWithError:`,
  `SimDeviceSet -devices`, `-registerNotificationHandler:`.
- `SimDevice` `-UDID`, `-name`, `-state`, `-runtime`, `-deviceType`, `-bootWithOptions:error:`,
  `-shutdownWithError:`, `-io` (`SimDeviceIOClient`) `-ioPorts`, each port's `-descriptor`.
- Display: a descriptor conforming to `SimDisplayRenderable`/`SimDisplayIOSurfaceRenderable`:
  `-framebufferSurface` (or `-ioSurface` on older Xcode),
  `-registerCallbackWithUUID:ioSurfacesChangeCallback:`,
  `-registerCallbackWithUUID:damageRectanglesCallback:`, and the unregister pair.
- Input (Indigo): `SimDeviceLegacyHIDClient -initWithDevice:error:`,
  `-sendWithMessage:freeWhenDone:completionQueue:completion:`, and the C builders
  `IndigoHIDMessageForMouseNSEvent` (touch at 0..1 ratios of the screen), `IndigoHIDMessageForKeyboardArbitrary`
  (HID usage, down/up), `IndigoHIDMessageForButton` (home, lock, side, Siri, Apple Pay).
- Input (CoreDevice, newer Xcode): the remote HID digitizer and vendor-defined services; used when
  the Indigo client refuses or is absent.

The helper is checked against the installed Xcode at every start, and the test suite (section 11)
runs on every Xcode the team uses. A new Xcode that breaks a selector shows in the pane as "Input
unavailable with Xcode 27.0" with stills still working, never as a crash.

**As built (Phase 1, Xcode 26.6, 2026-11-20).** `coax/lib/agent/device/sim-shim.m` (linked into
`oasim` by `coax/web/tool.sh`'s `// tool.sh: objc` line) holds every private name; `sim-shim.coax`
runs an op's plan (`sim-gesture.coax`'s `sg_plan_*`, tested) through it one event at a time from the
helper's loop, so a gesture never blocks the other clients. What the real system said:

- `IndigoHIDMessageForMouseNSEvent(&point, NULL, 0x32, type, ...)` takes the point as a **fraction of
  the screen** (0..1), and builds nothing for a drag (NSEvent type 6): a moved finger is sent as
  another down (type 1) at the new place, as idb does; up is type 2. Verified by tapping Settings'
  rows and by a swipe that scrolls a list.
- Buttons: `IndigoHIDMessageForButton(source, 1 down / 2 up, 0x33)` with sources home 0x0, lock 0x1,
  side 0xBB8, Siri 0x400002, Apple Pay 0x1F4. Keys: `IndigoHIDMessageForKeyboardArbitrary(usage,
  1 / 2)`.
- The framebuffer is the port descriptor's `framebufferSurface` (BGRA, device pixels). Reading it
  and writing a point-size PNG takes 12–18 ms, and `IOSurfaceGetSeed` changes only when the screen
  does. The live view is therefore built as **frames** (capability `"display":"frames"`): the page
  asks for a still on each of the host's fifteen ticks a second, naming the `gen` (seed) it has; the
  helper answers with no `path` when the screen is unchanged, so an idle device costs one seed read
  per tick. The Mach-port surface rendezvous (4.2) is not built; frames reach the glass in under
  about 70 ms, which is enough for the person's taps and an agent's gestures.
- Not yet: `rotate` (no Indigo message turns the device), `set_keyboard`, `touch2_path` (the
  two-finger Indigo message), and non-ASCII `text` (the pasteboard path). Each answers
  `input_unavailable` (or `bad_args` for text) with a sentence saying so. The CoreDevice input path
  and the surface and damage callbacks are not used.
- Confinement (9.4) is not built: the helper runs as the user, like `oasim build`.

### 6.3 Boot

`boot` uses `SimDevice -bootWithOptions:` with `{"persist": true}` (the device stays up when the
helper exits) and no `Simulator.app`. The helper waits for the device state `Booted` and then for
the boot to settle (`bootstatus`-equivalent: SpringBoard up), reporting progress events. Timeout 180 s.
A device already booted by someone else is attached, not rebooted.

## 7. Agent tools

### 7.1 One surface, three doors

| Door | Who uses it | How |
| --- | --- | --- |
| `oasim VERB ...` commands | Microcoder (Phase 1), Codex, any shell | A Coax tool shipped beside the workers in `Contents/Resources/` (`oasim`, `oasim-HASH` from `tool.sh` in a worktree, found as `claudeworker` finds `gitguard`). The app adds `Contents/Resources` to the worker's `PATH` and the skills note names it. Prints one JSON object (`--json`, the default for non-TTY) |
| `"device"` action member | Microcoder (Phase 2) | Only when the task line has `"device":true`. Typed ops in the step; the run talks to the helper itself; the screenshot attaches as an image with no `view` step |
| `oasim mcp` | Claude sub-agents, and `openagents mcp --domains device` for any MCP client | stdio JSON-RPC; tools `simulator_control` and `simulator_build` with Claude's argument names; images answered as MCP image content (as `mcp.coax` does for `realm_shot`) |

All three call the same library (`coax/lib/agent/device/`), the same helper ops and the same policy.

*As built (Phase 2, `coder/tui/oasim-mcp.coax`):* `oasim mcp` offers one tool a verb
(`simulator_list`, `_boot`, `_shutdown`, `_screenshot`, `_tap`, `_swipe`, `_text`, `_press_key`,
`_button`, `_launch`, `_terminate`, `_open_url`, `_set_appearance`, `_build`, `_build_status`, `_run`)
rather than two tools with an `action` member: each has a typed schema the model can't get wrong, and
a call runs `oasim VERB WORDS...` as a child, so it is the command with the same grants (the child's
hello carries the worker's `OA_SIM_CHAT`). `_screenshot` answers with the PNG as image content.
`claudeworker` passes it to Claude Code in `--mcp-config` (server `simulator`) when `oasim` is beside
it or `OA_SIMHOST` names one. The Microcoder `device` member is built (7.6, as built).
`openagents mcp --domains device` serves the same sixteen tools: `mcp.coax` loads `oasim-mcp.coax`
and runs its server with `oasim`'s path (`OA_SIMHOST`, else beside the CLI, else on `PATH`), so a
call is still `oasim VERB ...` and needs no second implementation.

### 7.2 `control` ops (verbs, arguments, replies)

Arguments and names follow Claude's `control` tool. Coordinates are **device points, origin top-left,
in the current orientation**. Every screenshot is taken at 1x points, so a pixel in the image the
model sees is a point it can tap.

| Op | Arguments | Reply (beyond `ok`) | Grant |
| --- | --- | --- | --- |
| `list` | | `devices: [{udid, name, runtime, state, booted_by_us}]` | none |
| `boot` | `device` (name or UDID) | `udid, points:{w,h}, scale` | control |
| `shutdown` | `device` | | control, and refused for a device the person booted unless they allow it |
| `attach` | `device?` | opens the pane in the asking chat; `udid` | none (shows, changes nothing) |
| `screenshot` | `device?`, `scale?` (0.25 to 1, default 1x points), `format?` png/jpeg | `path` (PNG/JPEG in the run's spill folder), `points`, `bytes`; MCP: the image | none |
| `tap` | `x, y`, `duration?` (> 0.5 s is a long press) | | control |
| `swipe` | `x, y, x2, y2`, `duration?` (default 0.3) | | control |
| `touch_path` | `points: [{x, y, dt_ms}]` (at most 30 s in all) | | control |
| `touch2_path` | `points2: [{x1, y1, x2, y2, dt_ms}]` | | control |
| `text` | `text` (at most 4,096 bytes) | | control |
| `press_key` | `key` (return, delete, forward_delete, arrows, home, end, page_up, page_down, select_all, undo, redo), `count?` (1 to 20) | | control |
| `button` | `name` (HOME, LOCK, SIDE_BUTTON, SIRI, APPLE_PAY) | | control |
| `set_appearance` / `appearance` | `state` light/dark | `appearance` | control / none |
| `set_keyboard` | `state` show/hide (the software keyboard) | | control |
| `rotate` | `direction` left/right | `points` after | control |
| `open_url` | `url` | | control |
| `launch` | `app_path?` (a built `.app`), `bundle_id?`, `args?` | `bundle_id, pid` | control; `app_path` must pass the install rule (9.2) |
| `terminate` | `bundle_id` | | control |
| `describe` (Phase 3) | `device?` | the accessibility tree: `[{role, label, value, frame:{x,y,w,h}, enabled}]` in points | none |
| `logs` (Phase 3) | `bundle_id?`, `since_s?`, `max_lines?` | log lines from `simctl spawn log show` | none |

Errors are one shape: `{"ok":false,"code":"...","why":"one sentence a person can read"}` with codes
`no_xcode`, `no_device`, `not_booted`, `contested`, `denied`, `no_grant`, `interrupted`,
`out_of_bounds`, `input_unavailable`, `bad_args`, `helper_down`, `timeout`, `install_refused`.

### 7.3 `build` ops

Claude's `build` tool, run by `oasim` (no private API; xcodebuild and simctl only):

| Op | Arguments | Reply |
| --- | --- | --- |
| `build` | exactly one of `project_path`/`workspace_path` (absolute), `scheme`, `configuration?` (Debug), `device?` | `build_id` at once |
| `build_status` | `build_id` | `state` running/succeeded/failed, `elapsed_s`, `errors: [{file, line, col, message}]` (at most 50, parsed from `error:` lines), `app_path` on success, `log_path` (whole log) |

`oasim build` runs
`xcodebuild -project P -scheme S -configuration C -destination 'platform=iOS Simulator,id=UDID' -derivedDataPath ~/.cache/openagents/sim-dd/HASH build`
(HASH of the project path and scheme), detached, its log in `~/.openagents/sim/builds/ID.log`, state in
`ID.state`. No device booted: `-destination 'generic/platform=iOS Simulator'`. **We do not pass
`-skipMacroValidation`** by default: a build that stops on untrusted Swift macros fails with that
reason, and the person can allow macros per project in the permission card ("Trust this project's
Swift macros"), which adds the flag for that project from then on. An `apps/openagents-ios` build
goes through `apps/openagents-ios/dev.sh` when the working folder is this repository, so its lock and
its incremental generation are kept.

Build-and-run is then `build`, poll `build_status`, `launch` with `app_path` (the helper runs
`simctl install UDID APP` then `simctl launch --terminate-running-process UDID BUNDLE`), and
`screenshot`. `oasim run` does the four in one call for the shell door.

### 7.4 Wire shapes (helper socket, `simhost-lines-v1`)

One JSON object a line each way, written and read only by `coax/lib/agent/device/sim-wire.coax`.
Requests carry an `id` the reply echoes; events carry no `id`.

```json
{"id":1,"op":"hello","client":"oasim","version":1,"chat":"8f3c...","engine":"microcoder","grant":"g-5d1e..."}
{"id":1,"ok":true,"version":1,"helper_pid":4123,"capabilities":{"display":"surface","input":"indigo","describe":false},"xcode":"26.1"}

{"id":2,"op":"tap","device":"6A1F...","x":201,"y":640}
{"id":2,"ok":true}

{"id":3,"op":"screenshot","device":"6A1F...","scale":1,"format":"png","out":"/Users/me/.openagents/coder/sessions/X.spill/sim-3.png"}
{"id":3,"ok":true,"path":"...sim-3.png","points":{"w":402,"h":874},"bytes":183402}

{"id":4,"op":"subscribe","device":"6A1F...","surface_service":"com.openagents.desktop.simhost.surface.812.1.9f2c","tick_fd":"pipe"}
{"event":"surface","device":"6A1F...","gen":1,"w":1206,"h":2622,"scale":3,"points":{"w":402,"h":874},"corner":55}
{"event":"state","device":"6A1F...","state":"booted"}
{"event":"agent","device":"6A1F...","chat":"8f3c...","engine":"microcoder","op":"tap","x":201,"y":640}

{"id":5,"op":"touch","device":"6A1F...","phase":"down","pointer":0,"x":120.5,"y":300}
{"id":6,"op":"grant","chat":"8f3c...","device":"6A1F...","scope":"chat","grant":"g-5d1e...","expires_s":0}
{"id":7,"op":"revoke","chat":"8f3c..."}
```

`touch` from the app is fire-and-forget (`"id":0` gets no reply) so a drag never waits on a round trip.

### 7.5 What the person sees in the chat

Phase 1: an `oasim` command is a command row like any other; the page recognises an `oasim` command
(the program's last path part) and labels the row from its verb: "Tapped (201, 640)", "Took a
screenshot", "Built OpenAgents (Debug) — 2 errors", with the screenshot as a thumbnail when the
command's JSON names a `path`. Phase 2: the worker writes a `device` line
(`{"kind":"device","op":"tap","args":{...},"ok":true,"shot":"PATH"}`) added to `mcworker-wire` and its
registry row, and the page draws it as a tool row whose press brings the simulator pane to the front.

*As built (Phase 2):* the device line is `{"kind":"device","op":"tap","ok":true,"shot":"PATH"}`
(`mww_device`, `mww_device_of` in `mcworker-wire.coax`), written by `mcworker` right after the command
line of any `oasim` command, a device op's or one the model ran itself; `shot` is the path the
command's JSON line names (a screenshot), absolute. Its args are the command's words, so the line
does not repeat them. The page draws the command row as before ("Tapped (201, 640)"); a press on an
`oasim` row that went brings the pane to the front (`CP_SIM_ROW`), and a failed one opens onto why.
Owners that do not know the kind skip it, so older consumers lose nothing.

### 7.6 Microcoder's `device` member (Phase 2)

With `"device":true` on the task line, `mc_schema` adds:

```json
"device": {"type":"array","items":{"type":"object","properties":{
  "op":{"type":"string","enum":["list","boot","screenshot","tap","swipe","touch_path","text","press_key","button","set_appearance","rotate","open_url","launch","terminate","build","build_status","describe"]},
  "device":{"type":"string"},"x":{"type":"number"},"y":{"type":"number"},"x2":{"type":"number"},"y2":{"type":"number"},
  "duration":{"type":"number"},"text":{"type":"string"},"key":{"type":"string"},"name":{"type":"string"},
  "url":{"type":"string"},"app_path":{"type":"string"},"bundle_id":{"type":"string"},
  "points":{"type":"array","items":{"type":"object","properties":{"x":{"type":"number"},"y":{"type":"number"},"dt_ms":{"type":"number"}},"required":["x","y"],"additionalProperties":false}},
  "project_path":{"type":"string"},"workspace_path":{"type":"string"},"scheme":{"type":"string"},"build_id":{"type":"string"}},
  "required":["op"],"additionalProperties":false}}
```

At most 8 ops a step, run in order after the step's edits and before its commands; a step whose last
op changes the screen gets a screenshot taken 300 ms after it settles and attached as the step's
image (replacing the oldest of the 4 image slots). Ops' results go into the next step's state as
`device` results, fenced as untrusted like command output (`fence.coax`). The system prompt gains a
short paragraph: coordinates are points in the last screenshot; take a screenshot before acting on a
screen you have not seen; prefer `describe` when it exists.

*As built (Phase 2, `coax/lib/microcoder/device.coax`):* the page puts `"device":true` on the task line
while the simulator pane is open and its helper connected (not merely when Xcode is present: the person
who opened the pane is the one who wants the chat to drive it); `mcworker` then adds the member
(`mc_device`). The run does not talk to the helper itself: each op becomes the `oasim` command that does
it, run first among the step's commands, so it goes through the same command path, journal, fence,
grants (`OA_SIM_CHAT`) and transcript row as any command, and its result in the next step's state is
the command's (one JSON line). A step with an op that can change the screen ends with
`oasim screenshot --settle 300 --out .openagents/shots/device-STEP.png`, which the step views, so the
image is attached with no view step; a `screenshot` op asks for that closing screenshot (taken once,
with no settling when nothing moved) rather than running on its own. When the 4 image slots are full
a device screenshot drops the oldest device screenshot, never an image the person attached
(`mc_image_drop_oldest`). `describe` is left out of the enum until Phase 3. Checked live on
2026-11-20: a real mcworker run on Vertex Gemini, on a throwaway iPhone 17 Pro, opened Settings,
then General and About, and read the iOS version from the screenshots.

## 8. Lifecycle and state

- **Pane state** is per window, kept across launches: open or closed, the picked device. A chat
  remembers the device its agent used (in the chat's file: `"sim_device":"UDID"`, a new optional field
  in the saved-chat shape, registered as such) so opening the chat shows that device.
- **Grants** are per chat and device, kept by the helper in memory and in
  `~/.openagents/sim/grants.json` (0600): `{chat, device, scope: once|chat, engine, at}`. A run's
  workers get the chat id and a grant token in their environment (`OA_SIM_CHAT`, `OA_SIM_GRANT`), which
  the app writes into the run spec. "Allow once" lasts until the run ends. Deleting a chat revokes its
  grants.
- **App closed:** ops from a run with a live `chat` grant keep working (the helper does not need the
  app). An op that needs the permission card while no window is open fails with `no_grant` and a why
  that says to open OpenAgents to allow it; the daemon's auto-yes for commands never answers a
  device permission.
- **Multiple windows / chats:** one helper, one device, many subscribers; each window's pane mounts
  its own layer from the same surface (an `IOSurface` can back many layers). Two chats with grants on
  one device: ops are serialised; the badge names the chat whose op ran last.

## 9. Permissions and safety

### 9.1 Rules (in `coax/lib/agent/device/sim-policy.coax`, `#authority;`)

1. Only a simulator: a UDID the helper's device set lists. Never a physical device; no op takes a
   device by any other name.
2. An op that changes the device needs a live grant for (chat, device), except ops from the app's own
   pane (the person). Read-only ops need none.
3. `shutdown` of a device the person booted (any device an agent did not boot through this helper
   since it started; a helper restart forgets, so every device then counts as the person's), `erase`, deleting a device and changing a device's
   language, region or privacy settings are never agent ops in v1.
4. A device is **contested** (refuse with `contested`) when `loop.coax`'s rules say so: another
   `dev.sh` holds its lock, an `xcodebuild` or `simctl install`/`launch` from elsewhere targets it, or
   another chat's agent is mid-gesture on it.
5. `launch app_path` and `install` take only an `.app` under the chat's working folder, its
   worktree, or our DerivedData (`~/.cache/openagents/sim-dd/`), resolved with symlinks followed.
6. `text` at most 4,096 bytes; `touch_path` at most 30 s and 2,000 samples; ops at most 20 a second
   per chat; a gesture's points must lie in the screen (else `out_of_bounds`).
7. The person's input always wins (4.3) and Stop revokes at once.
8. Typing a credential: the agent is told (system prompt, skills note) never to type the person's
   real passwords, payment details or one-time codes into the simulator; test accounts from the
   project's own fixtures only. The policy refuses `text` that matches a secret already known to the
   app (the Claude and OpenRouter keys) as a backstop.

`coax mutate` on the policy goes to zero survivors before it ships.

### 9.2 What the person is asked

- First control op per (chat, device): the permission card (5). Engine named: "Microcoder",
  "Claude (sonnet)", the sub-agent's name.
- Trusting a project's Swift macros for `build` (7.3).
- Shutting down a device they booted (only from the pane's own menu; agents can't).

### 9.3 What is never done

The helper never opens `Simulator.app`, never moves the cursor, never takes focus (it is
background-only and has no windows), never posts global `CGEvent`s, and never asks for
Accessibility or Screen Recording permission: CoreSimulator input goes to the simulated device only.

### 9.4 Confinement

The helper is started through `sandbox-exec -f simhost/sandbox.sb` (as Claude does): `mach-lookup`
and `mach-register` for CoreSimulator, the surface rendezvous and `IOSurface`; `iokit-open` for
`IOSurface`; `process-exec` for `xcrun`/`simctl` only; file reads of Xcode, `/Library/Developer`,
`~/Library/Developer/CoreSimulator`, the `.app`s passed to it and `~/.openagents/sim/`; file writes to
`~/.openagents/sim/`, the spill folders it is told about and `~/Library/Developer/CoreSimulator`; no
network. If the profile can't be applied on a macOS release, the helper runs unconfined and logs it
once; the pane's info says so. `oasim build` runs `xcodebuild` as the user, unconfined, like any
command the agent runs.

## 10. File-by-file changes

New:

| File | What |
| --- | --- |
| `coax/lib/agent/device/sim-wire.coax` | The one codec for `simhost-lines-v1`: every request, reply and event above, both ways |
| `coax/lib/agent/device/sim-wire-checks.coax` | Writes and reads each fixture line to the same bytes |
| `coax/lib/agent/device/sim-policy.coax` | `#authority`: rules 9.1, grants, rate limits, the install rule |
| `coax/lib/agent/device/sim-gesture.coax` | View-to-device mapping (aspect fit, rotation), tap/swipe/path expansion with easing, key names and ASCII to HID usages, `#forall` checks (mapping round trip, paths stay in bounds and end where asked) |
| `coax/lib/agent/device/sim-devices.coax` | `simctl list` parsing and the contested rules, moved from `apps/shared/app/preview/loop.coax` (which then loads it; no second copy) |
| `coax/lib/agent/device/sim-build.coax` | The xcodebuild command line, the build state files, `error:` parsing |
| `coax/lib/agent/device/sim-host.coax` | (As built, instead of `sim-core`.) The helper's loop: socket, clients, policy calls, grants and parked asks, `simctl` jobs; `oasim serve` runs it |
| `coax/lib/agent/device/sim-shim.coax` | The helper's private-framework effects as `#foreign` calls into `simhost/shim.m` (Phase 1; a stills-only stub before) |
| `coax/lib/agent/device/README.md` | What the library is, its doors and checks |
| `coder/tui/oasim.coax` | The tool: CLI verbs (7.2, 7.3, `run`), `oasim mcp`, helper start, the shell door's JSON output |
| `apps/openagents-desktop/simhost/*` | The helper (6.1) |
| `apps/openagents-desktop/page/simpane.coax` | The pane (5) |
| `apps/openagents-desktop/page/checks/simpane-checks.coax` | Its checks (11) |
| `apps/openagents-desktop/SimulatorPaneView.swift` | The layer-backed view: surface or still, ripples, mouse/keyboard to ops |
| `apps/openagents-desktop/SimHostClient.swift` | The socket client (lines through `sim-wire` via `host.c`), helper start, the Mach rendezvous, the tick pipe |
| `apps/openagents-desktop/Window+Simulator.swift` | Open/close, `syncSimulatorFrame()` from `cd_sim_rect`, covered/stills, the permission card's answers, menu and palette items, grants into run specs |
| `proto/coder/sim-host-lines-v1.json` | The JSON profile and pinned fixture lines |
| `docs/desktop/ios-simulator.md` | The user-facing behaviour, written when Phase 1 lands (this spec stays the design record) |

Changed:

| File | Change |
| --- | --- |
| `proto/registry.json` | Rows `simhost-lines-v1` (owner `coder`, codec `sim-wire.coax`) and, in Phase 2, the `device` line in `microcoder-worker-lines-v1`'s profile and the saved chat's `sim_device` field |
| `apps/openagents-desktop/page/state.coax` | `sim: [1] CpSimPane`, `CP_ASK_SIM_*`, the keyboard owner `CP_KB_SIM` |
| `apps/openagents-desktop/page/parts.coax` | Load `simpane.coax` |
| `apps/openagents-desktop/page/layout/layout.coax` | `cp_side_pane` precedence with `front` flags; the shell's pane is open when the simulator is |
| `apps/openagents-desktop/page/input.coax` | Presses on the pane's controls; the covered flag from overlays |
| `apps/openagents-desktop/page/transcript.coax` | Label `oasim` command rows (7.5); Phase 2 the `device` row |
| `apps/openagents-desktop/page/palette.coax` | "Show iOS Simulator" |
| `apps/openagents-desktop/host.c`, `host.h` | `cd_sim_*` calls |
| `apps/openagents-desktop/Window.swift` | `simView`, `simClient`; call `syncSimulatorFrame()` with the other syncs each frame |
| `apps/openagents-desktop/AppDelegate.swift` | View > iOS Simulator (Option-Command-S) |
| `apps/openagents-desktop/Worker.swift` | `PATH` gains `Contents/Resources`; `OA_SIM_CHAT`/`OA_SIM_GRANT` in the run spec; the skills note's simulator paragraph; Phase 2 `"device":true` on the task line when Xcode is present |
| `apps/openagents-desktop/build.sh` | Build `oasim` (as the workers), the helper (`swiftc` plus the `sim-core` C unit), bundle `Contents/Helpers/OpenAgents Simulator.app`, sign it |
| `apps/openagents-desktop/README.md` | A "Simulator" section and the Files table rows |
| `coder/tui/claudeworker.coax`, `coax/lib/agent/claude/options.coax` | `--mcp-config` with `oasim mcp` found beside the worker, only when a grant can be asked for (Xcode present) |
| `coax/lib/microcoder/action.coax`, `run.coax`, `loop.coax` (Phase 2) | The `device` member, its results and its image |
| `coax/lib/agent/engine/mcworker-wire.coax`, `coder/tui/mcworker.coax` (Phase 2) | `"device":true` in, the `device` line out |
| `apps/openagents-cli/mcp.coax`, `commands.coax` | `--domains device` serves the same tools (through `oasim`'s library); `openagents sim ...` as an alias of `oasim` |
| `apps/shared/app/preview/loop.coax` | Load `sim-devices.coax` instead of its own `simctl list` parsing |
| `apps/shared/host/drive.c` / desktop drive | `shot` composites the simulator view's current still into the window picture (11). *As built:* nothing to composite. The Swift app's drive (the Tier 3 probe, `evals/coder-desktop/tier3/probe/probe.m`) takes its pictures with WindowServer capture of the window (`screencapture -l WINDOW`), which already holds the native simulator view over the page; checked in the Phase 1 test launch, whose shots show the device screen. The offscreen `coax drive` host (`host-main.c`) draws no native views at all, so it shows the pane's frame and notices only, as it does for the 3D scene view |
| `docs/reuse.md` | The device library as the one place for simulator control |

## 11. Test plan

Fast (every change; `coax/bin/coax test` / `coax/remote/check.sh only`):

- `sim-wire-checks.coax`: every fixture line in `proto/coder/sim-host-lines-v1.json` reads and writes
  back to the same bytes; `coax/web/tool.sh wire-check` passes.
- `sim-policy.coax`: each rule with a passing and a refused case; `coax mutate` zero survivors.
- `sim-gesture.coax`: `#forall` view point to device point and back within half a point; every
  expanded path's samples inside the screen and its last sample at the target; swipe duration kept
  within a frame; key map covers printable ASCII and the named keys.
- `sim-devices.coax`: the moved `loop.coax` tests, unchanged, plus runtimes and device types parsing.
- `sim-build.coax`: the command line for project/workspace/no device; `error:` parsing on recorded
  xcodebuild logs (fixtures under `coax/lib/agent/device/fixtures/`).
- `page/checks/simpane-checks.coax`: the rect is aspect-correct for portrait and landscape at narrow
  and wide panes; `covered` is set by each overlay kind; controls dispatch the right asks; the
  permission card's three answers; pane precedence with file, review and sub-agent; the state notices.
  Run through `apps/openagents-desktop/check.sh` and `page-slots.sh`.

Helper (on a Mac with Xcode; `coder/tui/simhost-check.coax`, run with `coax/web/tool.sh`):

- (As built, Phase 0: `coax/web/tool.sh coder/tui/simhost-check.coax OASIM [--project P --scheme S]`
  covers hello, list, boot, point-size screenshots, a parked ask refused and then granted,
  appearance, `open_url`, the install rule, the shutdown rule, contested, revoke, the helper killed
  and restarted by `oasim`, `oasim run`, shutdown and delete; `--wire-log` captured the fixture.)
- Creates its own device (`simctl create oa-simhost-check-RAND "iPhone 17" RUNTIME`), never uses
  the person's, starts a helper from the worktree build with `--home` in a throwaway folder and its
  own socket, then: hello and capabilities; boot (under 180 s); subscribe and receive a surface of the
  device's size; screenshot is a PNG of the point size; tap the Settings icon (found by `describe` in
  Phase 3, by a recorded coordinate before) and see the screenshot change; `text` into Safari's
  address bar after `open_url`; rotate and receive a new surface `gen` with swapped points; a grant
  refused, given and revoked; contested refusal while a fake `dev.sh` lock is held; helper killed
  mid-gesture and restarted by the next client; shutdown and delete the device.
- Runs on each Xcode the team uses, and is the first thing run after installing a new Xcode.

App (with the mock worker, `evals/coder-desktop/mock_worker`, which issues scripted `oasim` commands):

- A build of the worktree's app opened with `--home` a throwaway folder (never
  `/Applications/OpenAgents.app`, never the person's daemon or helper: `OA_SIMHOST` and a socket of
  its own), driven by `coax drive`: open the pane, boot the check device, the permission card appears
  on the mock's first tap, allow for the chat, the badge shows and the ripple layer gets the point,
  Stop revokes and the mock's next tap fails with `no_grant`; a menu over the pane hides the view and
  shows the still; the drive's `shot` contains the composited screen.
- Performance: frame-to-glass latency of a damage tick under one display frame; a person's drag
  produces touch moves at the display rate; screenshot op under 150 ms at 1x; helper idle CPU under
  1% with a booted, idle device.

Manual, once per phase, on the owner's Mac only with their go-ahead: build-and-run
`apps/openagents-ios` from a chat ("build the iOS app and show me the login screen"), watch it boot,
build, install, launch and tap through.

## 12. Phases

**Phase 0 — public tools only (days).** `oasim` with `list`, `boot`, `shutdown`, `screenshot`
(`simctl io`), `launch`, `install`, `terminate`, `open_url`, `set_appearance`, `build`,
`build_status`, `run`. The pane with stills (`SimulatorPaneView` showing `CGImage`s, refreshed at up
to 4 fps while visible). The agent can build, run and look, not touch. Grants and the badge exist.
Ships the library, wire, policy and page parts for real.

**Phase 1 — the helper (the core of this issue).** `oa-simhost` with CoreSimulator: live `IOSurface`
display, the person's mouse and keyboard, agent `tap`, `swipe`, `touch_path`, `touch2_path`, `text`,
`press_key`, `button`, `rotate`, `set_keyboard`; capability probing and the stills fallback;
confinement; the helper check. `oasim` commands work for every engine; command rows are labelled.

**Phase 2 — first-class tools.** Microcoder's `device` member with auto screenshots; the `device`
worker line and its transcript row; `oasim mcp` for Claude sub-agents; per-chat device memory;
the drive `shot` composite.

**Phase 3 — understanding the screen.** `describe` (accessibility tree), `logs`, screen recording
(`simctl io recordVideo` to the working folder), several devices (a device tab strip), the pane on
the phone (later).

**Then Android** on the same library, pane and tools: [android-emulator-vision.md](android-emulator-vision.md).

## 13. Failure modes

| What happens | What the person sees | What the agent gets |
| --- | --- | --- |
| No Xcode, or `xcode-select` points at the Command Line Tools | "Xcode is not installed" with `xcode-select -s` advice | `no_xcode` |
| No iOS runtime installed | "No iOS simulator runtime; install one in Xcode > Settings > Components" | `no_device` |
| Private display API missing | Stills, and the pane's info says why | screenshots work; `capabilities.display = stills` |
| Private input API missing | "Input unavailable with Xcode N" | `input_unavailable` for touch ops |
| Boot fails or takes over 180 s | The failure notice with CoreSimulator's words; Boot again | `timeout` or the boot error |
| Helper crashes | The pane shows the last still and "Reconnecting…"; the client restarts the helper and resubscribes | the op in flight fails `helper_down`; the next one restarts it |
| Device rotated or resized | A new surface `gen`; the view swaps contents in one frame | `points` in the next reply |
| Device shut down from elsewhere (Xcode, `simctl`) | "Shut down" with Boot | `not_booted` |
| `dev.sh` or another tool is using the device | "In use by a build" | `contested` |
| Mac sleeps / display asleep | Ticks stop; nothing to do | ops still run |
| Two chats drive one device | Serialised; the badge names the last chat | each op waits its turn, at most 5 s, else `timeout` |
| App closed while an agent drives | Nothing (no window) | ops keep working under a `chat` grant |
| Grant missing while no window | — | `no_grant`, "open OpenAgents to allow it" |
| Memory pressure (each booted device about 1.5 GB) | Devices OpenAgents booted shut down after 30 idle minutes (setting) | — |
| Xcode updated under a running helper | The helper notices the developer dir changed and restarts itself | ops in flight fail `helper_down` |

## 14. Open questions for the implementer

(Answered for Xcode 26.6 in 6.2's "As built (Phase 1)": the selectors are as listed, the touch
point is a fraction of the screen, and a drag is repeated downs. Question 2 is open: the live view
shipped as damage-gated frames instead of a surface over Mach, so no rendezvous is needed yet.)

1. The exact private selectors on the installed Xcode: confirm each with `class-dump`-style
   inspection of the frameworks (read-only), record the Xcode version in `shim.m`'s header.
2. Whether `bootstrap_register2` is permitted under our Seatbelt profile on the current macOS, or
   whether the surface should move over an XPC anonymous listener endpoint passed through the socket
   (`IOSurfaceCreateXPCObject`); either is fine if the peer check holds.
3. Whether Microcoder's 4-image limit should rise for device runs (a run of screenshots), or the
   oldest device screenshot should always be the one dropped (the spec says the latter).
4. Keyboard focus rules when the composer and the simulator both want typed text: the spec gives it
   to the simulator only after a click in it, back to the composer on Escape or a click elsewhere.
