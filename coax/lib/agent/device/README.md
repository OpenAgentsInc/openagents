# coax/lib/agent/device: the iOS Simulator for agents and the person

The one place OpenAgents controls a simulated device (docs/desktop/ios-simulator-spec.md, GitHub
#13811). Android (docs/desktop/android-emulator-vision.md) is meant to come in beside these files,
sharing the wire, the policy and the gesture maths.

| File | What |
| --- | --- |
| `sim-wire.coax` | The one codec for the helper's socket lines (`simhost-lines-v1`): every request, reply and event, both ways |
| `sim-wire-checks.coax` | Reads and writes back every line of `proto/coder/sim-host-lines-v1.json` to the same bytes |
| `sim-policy.coax` | `#authority;` the rules of spec 9.1: which ops need a grant, contested and held devices, the shutdown rule, rate, text and path limits, the `.app` install rule |
| `sim-gesture.coax` | View points to device points (aspect fit, rotation), swipe expansion with easing, key names and ASCII to HID usages, buttons |
| `sim-devices.coax` | `simctl list devices -j` parsing, device picking, and the contested rules `apps/shared/app/preview/loop.coax` also uses |
| `sim-build.coax` | The `xcodebuild` command for the simulator, its DerivedData folder, `error:` lines from its log |
| `sim-host.coax` | The helper (`oasim serve`): one socket, its clients, grants and parked asks, `simctl` jobs (boot, screenshot, launch, appearance, open_url, terminate, shutdown) |
| `sim-shim.coax` | The helper's private CoreSimulator effects (live display, input); stills-only until the shim lands |

The doors: `coax/lib/agent/device/oasim.coax` (the `oasim` command: CLI verbs for any agent's shell, `serve`,
`build`, `run`) and `oasim-mcp.coax`. Every reply is one JSON line.

Checks:

- `coax/web/tool.sh coax/lib/agent/device/sim-wire-checks.coax proto/coder/sim-host-lines-v1.json`
- `coax/bin/coax mutate --root-only coax/lib/agent/device/sim-policy.coax` (zero survivors)
- each file's `#test`s, run whenever a tool loads it
- `coax/web/tool.sh coax/lib/agent/device/simhost-check.coax OASIM` on a Mac with Xcode: makes and deletes its
  own device, starts a helper with a throwaway home and socket, and acts as the app and an agent
