# Link devices and host auto-start — September 27, 2026

This record covers
[#9731](https://github.com/OpenAgentsInc/openagents/issues/9731), the
`coder link` command and the live linking of two computers, and
[#9735](https://github.com/OpenAgentsInc/openagents/issues/9735), the owner's
auto-start policy, in the
[linked devices program](https://github.com/OpenAgentsInc/openagents/issues/9736).
It separates synthetic tests from the live run on two real computers and the
production relay. No phone took part; the iPhone enrolls in a later step.

## Delivered behavior

- `coder link` (`crates/coder-setup`) and `scripts/link-device.sh`, since
  removed ([#9978](https://github.com/OpenAgentsInc/openagents/issues/9978));
  the [link your devices guide](../guides/link-devices.md) has the current
  path.
- `coder host init` records WebSocket listener settings in `serve.json`, so
  the host service serves them. An advertised endpoint that repeats a
  listener's own address now replaces that listener's hint; before, the
  duplicate made the whole hint set invalid and the host published none.
- `coder host autostart on|off|show`, described in the
  [auto-start guide](../runtime/host-autostart.md), with the relaxed
  invariant recorded in the [invariant ledger](../../../INVARIANTS.md).
- `coder_computers::live::Live::host_link`, a connected host's supervised
  link for NIP-TERM requests.

## Synthetic tests

| Check | Result |
| --- | --- |
| `cargo test -p coder-setup` | 17 unit tests passed: owner key privacy and idempotency, Tailscale status and certificate parsing, the recorded settings, service decisions, directory edits, SSH quoting, and option handling. `tests/link.rs` passed: a real host on a local test relay, joined by invitation, proves a TCP route, a WebSocket route, and the relay route; another machine is never offered loopback routes; the owner lists the host once and a second listing publishes nothing; another key cannot read the directory; ordering work reaches the host; revocation fails every route. |
| `cargo test -p coder-host` | Passed, including `init_records_listeners_and_refuses_ones_that_could_never_serve`, the settings tests, and `an_advertised_copy_of_a_listener_states_its_class_once`. The WebSocket, `wss`, headless, CJ, and end-to-end tests still pass. |
| `cargo test -p coder --lib autostart` | 6 passed: no policy leaves creation inert and unchanged; an admitted workspace records the engine's model and starts through a closed grant; another workspace stays inert; the concurrency bound holds and releases after the admission grace, recording the unadmitted task once; turning the policy off stops starts, and a cancelled task is skipped; a retry across a policy change returns the original receipt; bounds and the command line. |
| `cargo test -p coder-host --lib cli` | Includes `serve_raises_a_launchd_sized_open_file_limit`. |
| `cargo test -p coder --lib task::remote`, `--test host_serve` | Passed. |
| `cargo clippy -p coder-host -p coder-setup -p coder -p coder-computers --all-targets -- -D warnings`, `cargo fmt --check` | Passed. |

## Live run

Two computers on the owner's tailnet, both through `scripts/link-device.sh`,
with the production relay `wss://relay.openagents.com/`:

| | This Mac (`macbook-pro-m5`) | `coderos-4080` (NixOS) |
| --- | --- | --- |
| Host key | `9c912290…6faf316` | `93235bef…8ccad759` |
| Service | launchd agent `org.openagents.coder-host`, starts at login | systemd user unit `org.openagents.coder-host`, linger on, starts at boot |
| Listeners | loopback TCP `127.0.0.1:47100`; WebSocket `100.127.107.31:47101` with TLS | loopback TCP `127.0.0.1:47100`; WebSocket `100.74.238.61:47101` with TLS |
| Tailnet hint | `wss://macbook-pro-m5.tailaeab8f.ts.net:47101/` | `wss://coderos-4080.tailaeab8f.ts.net:47101/` |
| Workspace `openagents` | `~/work/openagents-host-tasks`, a detached worktree | `~/openagents-host-tasks`, a detached worktree |
| Auto-start | on, `max_running` 1 | on, `max_running` 1 |

The owner key was created on this Mac. Both hosts are listed in the owner
directory at revision 2, read back from the production relay.

Findings on the way, each fixed and rerun:

- The sandboxed macOS Tailscale app cannot write a certificate outside its
  container. `coder link` now reads the chain and key from `tailscale cert`'s
  standard output and writes them itself.
- On Linux, `tailscale cert` needs the user to be Tailscale's operator. The
  run set it on `coderos-4080` with `sudo tailscale set --operator=$USER`;
  without it, `coder link` serves plain `ws` on the tailnet.
- Cargo hard-links release outputs, and the bundle stager takes only an
  unlinked file, so the script stages a private copy.
- The engine refuses a Jev reply whose model differs from the admitted one,
  so a policy naming `jev-latest` failed at the first judgment. The default
  is now the exact `jev-1.13.0`.

Route checks after `coder link peer --ssh coderos-4080`:

| From | To | Direct (`wss` on the tailnet) | Relay |
| --- | --- | --- | --- |
| This Mac | `coderos-4080` | Channel proved both keys and answered a ping, 69 to 176 ms | Signed `device.list` answer, about 0.5 s |
| `coderos-4080` | This Mac | Channel proved both keys and answered a ping, 100 to 158 ms | Signed `device.list` answer, about 0.4 s |

Revoking the Mac's device on `coderos-4080` failed both routes at once
(`revoked: host refused the channel`, and a signed `Revoked` refusal through
the relay); a new `peer` restored them. Re-running `coder link setup` with
nothing changed reported `host service: Nothing` and published no directory
revision.

Auto-start, with work ordered by the other computer as an enrolled device
through `coder link order`:

- On `coderos-4080`, the task was recorded as `eligible` with the Mac's
  device key, `started` in the same second with its grant digest and owner
  process, admitted by the task owner, and ran the engine. The run ended
  `failed` because the host's Codex login returned HTTP 429
  `usage_limit_reached`; its weekly limit resets on October 3. The first
  attempt, under a `jev-latest` policy, ended at the first judgment as
  described above.
- On this Mac, the first auto-started task was refused at admission: the
  launchd agent's soft limit of 256 open files, inherited by the task owner,
  made its workspace snapshot incomplete (`Too many open files`), and the
  task stayed queued. The host now raises its soft limit to 10,240 before it
  serves, and the sweep records such a task as `unadmitted`. After the fix,
  a task ordered from `coderos-4080` was `eligible` and `started` in the same
  second, admitted, and ran the engine through three judgments and two
  generations before the Mac's Codex login also returned HTTP 429
  `usage_limit_reached`.

Two first attempts failed once and passed on retry: a `peer` redemption
refused as `expired` right after `coderos-4080`'s service update, and one
`order` to this Mac timed out before its inbox recorded anything. Neither
reproduced.

## Limits

- No phone enrolled. The iPhone is online on the tailnet; the owner scans
  an invitation in the next TestFlight build, as the workspace
  `NEEDS_OWNER.md` describes.
- No auto-started task finished its work: both hosts' Codex login is out of
  quota until October 3. The record establishes the policy's path from a
  device's order to an admitted, running engine, not the engine's result.
- The auto-start concurrency bound counts only auto-started tasks, and the
  task owner allows one unresolved task per workspace.
