# Verify in a browser

When an agent checks a change in a browser, it runs its check under
`openagents browser run`. The command starts Chrome with a fresh profile and
a debugging port that Chrome picks, runs the check, and then ends Chrome and
removes the profile. Two agents on one machine can verify at the same time,
because neither shares a fixed port (such as the old 9333) or a profile
directory with the other.

The design is in
[Many agents on one machine](../design/many-agents-one-machine.md), and the
code is [`crates/openagents-cli/src/browser.rs`](../../../crates/openagents-cli/src/browser.rs).

## Run a check

```sh
openagents browser run [--headed] [--browser PATH] [--timeout SECONDS] [--json] -- CMD [ARGS...]
```

The command exits with the status of `CMD`. `CMD` gets two variables:

- `OPENAGENTS_CHROME_PORT`: the DevTools port on `127.0.0.1`.
- `OPENAGENTS_CHROME_WS`: the browser's DevTools WebSocket URL, such as
  `ws://127.0.0.1:53817/devtools/browser/ID`.

For example, the first command asks the browser for its version, and the
second runs a check script that connects to the port it was given:

```sh
openagents browser run -- sh -c 'curl -s "http://127.0.0.1:$OPENAGENTS_CHROME_PORT/json/version"'
openagents browser run -- python3 check.py
```

A helper that reads the port should fall back to starting its own browser
when `OPENAGENTS_CHROME_PORT` is unset, as the retained smoke helpers in
`bench/verse/2026-10-05/platform-clients/` do.

With `--json`, the command prints a summary after the output of `CMD`:
`browser`, `headed`, `port`, `ws`, `profile`, `profile_removed`, and `exit`.

## What it starts

- **The browser.** `--browser PATH`, else `$OPENAGENTS_CHROME`, else Google
  Chrome or Chromium in `/Applications` or `~/Applications` on macOS, else
  `google-chrome`, `google-chrome-stable`, `chromium`, or `chromium-browser`
  on `PATH`.
- **The profile.** A new `chrome-<random>` directory in the session's scratch
  directory, which `openagents scratch` prints (see
  [Durable scratch](scratch.md)). Chrome's own log is `browser.log` inside
  it.
- **The flags.** `--user-data-dir` set to the profile,
  `--remote-debugging-port=0`, `--headless=new` unless `--headed` is given,
  and flags that keep Chrome off the network at startup and away from the
  owner's keychain: `--no-first-run`, `--no-default-browser-check`,
  `--disable-background-networking`, `--disable-sync`,
  `--use-mock-keychain`, and `--password-store=basic`.

The command waits up to `--timeout` seconds (30 by default) for Chrome to
write `DevToolsActivePort` into the profile. If Chrome exits first or the time
runs out, the command fails with the last lines of Chrome's log, and `CMD`
doesn't run.

## Cleanup

Chrome runs in a process group of its own. When `CMD` ends, whether it
succeeds or fails, the command stops that group, kills anything left after a
short grace period, and removes the profile. An interrupt or a termination
signal is passed to `CMD`, and the same cleanup follows.

## Headed runs

A headless run needs no lease. A run with `--headed` opens a window on the
real screen, so it first takes the `screen` lease and then the `browser`
lease (see [Leases](../runtime/leases.md)). The screen lease needs the
owner's grant. Without one, the command fails before Chrome starts and names
`openagents lease grant screen`, which only the owner can run on an
interactive terminal. `CMD` also gets the lease variables, and
`OPENAGENTS_LEASES` names both leases.
