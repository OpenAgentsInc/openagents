# Engine and usage verification

Implements [#10018](https://github.com/OpenAgentsInc/openagents/issues/10018),
part of [#10003](https://github.com/OpenAgentsInc/openagents/issues/10003).

The ring and the route cards reimplement Zeron's account usage rings (public
MIT [zeronsh/zeron](https://github.com/zeronsh/zeron)) in Rust Native. The
shared strip lives in `openagents-chat-app`, which the desktop and the phone
library compile. Mounting the strip on a phone remains part of
[#10028](https://github.com/OpenAgentsInc/openagents/issues/10028).

## What the window shows

On a live chat, the 38-point titlebar stays one line. The strip is the
content header under it. The window shows this
computer's `autostart.json` routes in order, the model on each route, whether
Codex and Claude Code are signed in, and each probed usage window. The window
sends `engine_status` and receives percents and reset times. It cannot change
the engine, the model, or a credential, and it does not read a provider token.

The host answers from a cached report. When a reading is due and usage probes
are on, the host runs `coder host autostart status --refresh` in the
background, in the Coder process. A probe reads a token only then, and only
to send it to that provider's own usage endpoint.

## Checks

Rust 1.97.1 on an M5 Max Mac, rebased onto current `origin/main`:

- Connect control: 9 tests passed. An engine report rejects an extra
  `access_token` field.
- Shared application: 96 tests passed. The engine tests cover route cards for
  the committed Codex and Claude usage fixtures (a full primary window, and
  4% and 66% Claude windows), sign-in lines when no route is set, and the
  closed set of usage sentences. The strip has no button.
- Coder auto-start: 30 tests passed in `task::autostart`. The report matches
  the policy and the usage book. Probes stay off until the owner turns them
  on. A cached report does not call the provider. `coder host autostart
  status --help` and a rejected `--probe-usage` return before any sign-in
  read. These checks do not run a successful `status` against a real login.
- Host control: 10 tests passed. Without a Coder program the socket refuses
  with "This computer cannot read Coder's engine." With a stand-in program, a
  cached report returns at once, a due reading starts `status --refresh` in
  the background, and a report that carries `access_token` is refused and
  does not start a refresh.
- Desktop library: 85 tests passed. The header shows the model and the usage
  line, and the strip contains no control that changes them. Desktop binary:
  37 passed, four opt-in checks ignored, including the ring painted from
  `engine-ring:codex:100`.
- The usage-ring unit test passed after the rebase: the ring starts at the
  top and runs clockwise.
- Strict all-target Clippy for `openagents-connect`, `openagents-chat-app`,
  `rust-native-desktop`, `coder-host`, `openagents-desktop`, `openagents-cli`,
  and `coder` passed. Formatting was applied to the files in this slice.

Usage values in these checks come from the committed Codex and Claude
fixtures. The owner's logins were not probed. The shared strip's tests are
the compile the phone library depends on. Native mounting remains #10028.
This slice has no owner-only step, so it does not add a line to
`NEEDS_OWNER.md`.
