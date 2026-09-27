# Client directory and SSH verification — September 27, 2026

[Issue #9719](https://github.com/OpenAgentsInc/openagents/issues/9719) lists
two client-reach items that this change closes in code:

- **Discovery through the owner directory in the app.** The live Computers
  service in [`crates/coder-computers`](../../../crates/coder-computers/README.md)
  reads the owner host directory with the owner key, lists directory hosts
  with their owner labels and weights beside enrolled hosts, shows a listed
  host this device has no grant for as not enrolled, publishes the next
  revision when the owner adds an enrolled host, and feeds directory weights
  to placement.
- **SSH hosts from the desktop and terminal clients.** **Add a computer**,
  then **Connect over SSH**, runs
  [`coder-ssh`](../../../crates/coder-ssh/README.md) to install or reuse the
  release and start or adopt the host, then redeems the invitation that
  `coder host invite` prints. Password prompts reach the screen as masked
  input requests. A phone doesn't show SSH.

It also corrects the **Use an invitation** help text: on the computer you run
`coder host invite`, not `coder-access invite`.

## Owner authority

The rule is specified in
[NIP-REACH, Owner authority on a client](../../../nips/openagents/NIP-REACH.md#owner-authority-on-a-client).
A device holds the owner key only locally: its device key is the owner a held
grant names, or the person enters the owner key and the service accepts it
only when a held grant names that key as owner. The key is kept in the same
protected store as the grants (`live::Saved::owner`): the mobile store
encrypts it under the device key from the platform's protected store, and the
terminal and desktop store writes it owner-only. The owner key never travels
over a relay, and there is no re-encryption of the directory to device keys.

## Evidence class

All evidence is synthetic. Tests ran on one macOS computer. The live tests
serve a real host through the `coder host serve` command path
(`coder_host::cli::run`) on the synthetic NIP-42 relay fixture from
`coder-control`, over loopback. The SSH test uses the fake `ssh` harness in
`crates/coder-ssh/tests/support/fake_ssh.rs`: it runs the real remote script
in a local shell with `HOME` set to a temporary directory, and a stand-in
`coder` program answers `host serve` and prints the real host's invitation for
`host invite`. No real `sshd`, remote machine, production relay, simulator,
emulator, or physical device was involved. The real `~/.ssh` and
`~/.openagents` were not read or written.

## Checks that ran

Each command ran from the worktree with `CARGO_TARGET_DIR` set to the
worktree's own `target` and `CARGO_PROFILE_DEV_DEBUG=0`:

```sh
cargo test -p coder-computers --features ssh
cargo clippy -p coder-computers --features ssh --all-targets -- -D warnings
cargo clippy -p coder-computers --lib -- -D warnings
cargo clippy -p coder-computers --no-default-features --lib -- -D warnings
cargo test -p coder-ssh
cargo clippy -p coder-ssh --all-targets -- -D warnings
cargo clippy -p coder-host --all-targets -- -D warnings
cargo clippy -p coder-host --lib --no-default-features -- -D warnings
cargo test -p coder-host --test end_to_end
cargo test -p coder-mobile --lib
cargo clippy -p coder-mobile --all-targets -- -D warnings
cargo fmt -p coder-computers -p coder-ssh -p coder-host --check
```

Results: 28 `coder-computers` unit tests and 2 live tests passed, and the live
tests passed three runs in a row; 9 `coder-ssh` unit tests and 18 remote
tests passed (1 helper ignored, as before); the `coder-host` end-to-end
scenario passed; 39 `coder-mobile` tests passed (1 ignored fixture, as
before). Clippy reported no warnings. The workspace gate did not run.

## Acceptance coverage

| Acceptance item | Test |
| --- | --- |
| Directory hosts listed with owner labels and weights | `tests::directory_hosts_show_labels_weights_and_unenrolled_rows`; live: `directory_hosts_show_owner_labels_weights_and_revisions` |
| An unenrolled directory host shown as not enrolled | the same two tests: status, weight line, and no switch, retry, or forget control |
| A label change after a directory revision | live: another owner device publishes revision 2 and the row relabels from **Studio** to **Studio Mac** |
| Adding an enrolled host to the directory (owner action) | live: **Add to directory** publishes revision 1 with the chosen label |
| A lower revision never replaces a higher one | live: a republished revision 1 leaves the revision 2 label |
| A key that isn't the owner is refused before saving | live: a random secret is refused and the state stays **no owner key** |
| Weights reach placement | `tests::directory_weights_reach_placement` (exact scores: 300 against the local 100, and weight 0 excluded); live: the enrolled host's score carries its directory weight, and the unenrolled host is `NotAdmitted` |
| SSH add-computer end to end | live: `add_a_computer_over_ssh_enrolls_through_its_invitation`: destination input, each password prompt answered through a masked input request, install and start, invitation redemption, and the row **Online** |
| The ownership rule | the same test: forgetting the computer leaves the managed remote host running |
| SSH progress, prompts, cancel, and failure on the screen | `tests::ssh_setup_progress_prompts_and_result_show_on_the_add_screen` |
| SSH hidden on phones | `tests::ssh_and_local_host_follow_the_platform`, `tests::disabled_controls_carry_reasons_on_every_screen_and_platform` |
| Help text names `coder host invite` | `tests::the_invitation_help_names_the_host_command` |

## Mobile

The iOS and Android glue didn't change. The mobile library builds
`coder-computers` without the `ssh` feature. A phone's Add screen no longer
draws the SSH section it used to show disabled; the Computers screen gains
**Your directory** with its state line. No simulator or emulator run was
repeated for this change.

## Limits

- A phone doesn't offer **Enter owner key**: the native input fields don't
  mask secrets yet. A phone reads the directory only when its device key is
  the owner key.
- The screens don't edit a listed host's label or weight, or remove a host
  from the directory. A new entry gets weight 100.
- The screens don't offer `coder-ssh`'s explicit remove, so they never stop a
  remote host.
- An SSH-launched host is reached through its relay; the loopback tunnel
  isn't used as a route.
- The SSH test's remote `coder` is a stand-in; the invitation, the relay, the
  host that answers it, and the redemption are real. A loopback `sshd` and a
  Linux remote run remain the platform acceptance item in #9719.
- Directory reads poll the owner's relays on the refresh period and when a
  screen polls, like host data. There is no live subscription.
