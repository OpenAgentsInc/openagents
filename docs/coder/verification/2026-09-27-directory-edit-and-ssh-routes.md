# Directory editing and SSH routes verification — September 27, 2026

[Issue #9723](https://github.com/OpenAgentsInc/openagents/issues/9723) lists
two client gaps that this change closes in code:

- **Directory editing.** On a listed host, the Computers screens in
  [`crates/coder-computers`](../../../crates/coder-computers/README.md) offer
  **Rename**, **Change weight**, and **Remove from directory**. Each publishes
  the next owner directory revision.
- **Remove an SSH host from the screens, and use the SSH loopback tunnel as a
  route.** On desktop and terminal clients, **Remove over SSH** runs
  [`coder-ssh`](../../../crates/coder-ssh/README.md)'s explicit remove. An SSH
  setup also opens a tunnel, and the connector in
  [`coder-host`](../../../crates/coder-host/README.md) tries its forwarded
  loopback port before the host's hints and the relay.

## Rules the change keeps

- **Owner authority.** An edit needs the owner key held on this device under
  [NIP-REACH, Owner authority on a client](../../../nips/openagents/NIP-REACH.md#owner-authority-on-a-client),
  and a successful read with no conflict. Each edit intent carries the
  revision the screen showed. The controller and the live service both refuse
  an edit made against another revision as stale.
- **Conflicts.** Two different bodies at the top revision stay a visible
  conflict. The list keeps the version this device trusted, and edits are
  disabled. **Keep this device's version** publishes that version one revision
  above the conflict (`Directory::superseding`). Nothing merges the two.
- **Removed hosts.** A host the owner removes stays in the list, reachable,
  when this device holds its grant. It says it was removed, and placement
  gives it weight 0. **Add to directory** lists it again. A removed host with
  no grant here leaves the list.
- **The tunnel is a local route.** Its address is loopback on this machine,
  reached through an `ssh` process this client runs, so it is same-machine
  evidence for that one address. The connector refuses a non-loopback local
  route. The route is never saved, published, or offered to another device,
  and the host's own loopback hints still follow the locality rule. NIP-REACH
  now says so under Selection.
- **Ownership.** A dead tunnel clears the route, and the host is reached
  through its relay. Nothing stops the host because a tunnel ended. Only
  **Remove over SSH** stops a host, and only one that this app's setup
  started. It detaches from a host that was already running.

## Evidence class

All evidence is synthetic. Tests ran on one macOS computer. The live tests
serve a real host through the `coder host serve` command path
(`coder_host::cli::run`) on the synthetic NIP-42 relay fixture from
`coder-control`, over loopback. The SSH tests use the fake `ssh` harness in
`crates/coder-ssh/tests/support/fake_ssh.rs`. The harness runs the real
remote script in a local shell with `HOME` set to a temporary directory, and a
stand-in `coder` program answers `host serve` and `host invite`. The fake
`ssh` records each tunnel and then sleeps, so a test thread stands in for
`ssh -L`: while that process lives, the thread forwards the tunnel's local
port to the real host's loopback listener. When the process ends, the thread
closes the port and every forwarded connection. The SSH tests run the client
as another machine (`Locality::OtherMachine`), so the host's loopback hints
are never offered and only the tunnel or the relay can carry it. No real
`sshd`, remote machine, production relay, simulator, emulator, or physical
device was involved. The real `~/.ssh` and `~/.openagents` were not read or
written.

## Checks that ran

Each command ran from the worktree with `CARGO_TARGET_DIR` set to the
worktree's own `target` and `CARGO_PROFILE_DEV_DEBUG=0`:

```sh
cargo test -p coder-computers --features ssh
cargo clippy -p coder-computers --features ssh --all-targets -- -D warnings
cargo clippy -p coder-computers --no-default-features --lib -- -D warnings
cargo test -p coder-reach
cargo test -p coder-host --lib --test websocket --test end_to_end
cargo clippy -p coder-host -p coder-reach -p coder-ssh --all-targets -- -D warnings
cargo test -p coder-ssh
cargo check -p coder-mobile --lib
cargo test -p coder-mobile --lib
cargo fmt -p coder-computers -p coder-ssh -p coder-host -p coder-reach --check
```

Results: 38 `coder-computers` unit tests, 3 tests in `tests/edits.rs`, and 2
in `tests/live.rs` passed. The two live files passed three runs in a row
before the rebase and once after it. 23 `coder-reach` unit tests passed. 15
`coder-host` unit tests, the WebSocket test, and the end-to-end scenario
passed. 9 `coder-ssh` unit tests and 18 remote tests passed (1 helper
ignored, as before). 41 `coder-mobile` tests passed (1 ignored fixture, as
before). Clippy reported no warnings. The workspace gate did not run.

## Acceptance coverage

| Acceptance item | Test |
| --- | --- |
| Rename, change weight, and remove, with input bounds | `tests::edit::the_owner_relabels_reweighs_and_removes_listed_hosts` |
| Missing owner authority refuses | `tests::edit::owner_edits_need_the_owner_key_and_a_current_read` (no owner key, not read, failed read, conflict, unlisted, unknown host); live: a service without the owner key refuses every edit as `forbidden` |
| Stale revisions refuse | `tests::edit::an_edit_against_a_stale_revision_never_reaches_the_service` (older and newer revisions, a confirmed removal after the directory moved on, and the service's own stale refusal); live: an edit asked at revision 7 is refused once revision 8 arrives, and the relay keeps revision 8 |
| Conflicts stay visible until the owner keeps a version | `tests::edit::a_conflict_stays_visible_until_the_owner_keeps_a_version`; live: this device's revision 6 and another device's revision 6 stay a conflict, with edits disabled, until **Keep this device's version** publishes revision 7 |
| A removed host stays reachable and leaves placement | `tests::edit::a_removed_host_stays_reachable_and_leaves_placement`; live: after revision 4 removes the host, it stays online, its device list loads, and placement skips it as `ZeroWeight` |
| Live label and weight edits | live: `the_owner_edits_removes_and_settles_conflicts_in_the_directory` checks revisions 2 and 3 on the relay |
| Directory helpers | `coder_reach::directory::tests::edits_and_conflict_resolution_produce_the_next_revision` |
| The tunnel route when it's up | live: `the_ssh_tunnel_carries_the_host_until_it_dies_and_remove_stops_a_managed_host`: the row reads **Online through the SSH tunnel** because the live link's route is the tunnel's address |
| Relay fallback when the tunnel dies | the same test: killing the tunnel's `ssh` process moves the row to **Online through a relay**, the host process keeps running, and the relay carries a device list request |
| Remove stops a managed host | the same test: **Remove over SSH** reports **stopped**, the managed process exits, and the computer leaves the list |
| Remove detaches from an external host | live: `remove_over_ssh_detaches_from_an_external_host`: the adopted host keeps running and the computer leaves the list |
| Remove progress, prompts, outcomes, and platform rules | `tests::edit::remove_over_ssh_asks_first_and_shows_the_outcome`, `tests::edit::remove_over_ssh_follows_the_platform_and_the_setup` |
| The route line and tunnel line | `tests::edit::the_ssh_tunnel_route_shows_when_used_and_when_closed` |
| A local route is loopback only and tried first | `coder_host::client::connector::tests::a_local_route_is_loopback_only_and_tried_first` |

## Mobile

The iOS and Android glue didn't change. The mobile library builds
`coder-computers` without the `ssh` feature, so a phone never shows **Remove
over SSH** or opens a tunnel. A phone that holds the owner key shows the
directory edits, with a new `directory_weight` input purpose that the
existing input bridge carries. No simulator or emulator run was repeated for
this change.

## Limits

- A tunnel opens only during **Connect over SSH**. After the app restarts, or
  once the tunnel closes, the host stays on its other routes until it is
  connected over SSH again.
- An attempt reads presence from the relay before it tries any direct route,
  the tunnel included, so the tunnel doesn't help while the relay is down.
- **Remove over SSH** doesn't change the owner directory. A stopped host that
  the directory still lists stays listed until the owner removes it.
- The SSH tests' remote `coder` and `ssh -L` are stand-ins. The invitation,
  relay, host, redemption, direct channel through the forwarded port, and
  remote remove script are real. A run against a real `sshd` remains with the
  platform acceptance checks in #9724.
