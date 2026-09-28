# Coder Computers

Coder Computers draws the screens every Coder client uses to manage the
computers you reach: your hosts with an honest status, the ways to add one,
who has access to each, first run, and activity that needs attention. The
screens are Rust Native projections with a closed intent enum. The iOS and
Android hosts mount them, and the terminal adapter in `coder-terminal` draws
them. [Issue #9713](https://github.com/OpenAgentsInc/openagents/issues/9713)
defines the screens and
[issue #9715](https://github.com/OpenAgentsInc/openagents/issues/9715)
connects them to real hosts, within the
[remote access program](https://github.com/OpenAgentsInc/openagents/issues/9704).

## Screens

| Screen | What it shows | Intents |
| --- | --- | --- |
| First run | The ways to add a computer and a **Continue** control that stays disabled until one computer has a current grant. | Every **Add a computer** intent, `continue_onboarding`. |
| Computers | One row per host: label, status, route in use (including the SSH tunnel), the reason when blocked, the owner directory's weight for a listed host or that the owner removed it, and an SSH host's tunnel. Your directory's state follows the rows, and the outcome of a remove over SSH. | `set_enabled` (switch off without forgetting), `retry_now`, `forget` and `confirm_forget`, `show` the host's access, `list_in_directory`, `edit_label`, `edit_weight`, `remove_from_directory` and `confirm_remove_from_directory`, `keep_directory`, `remove_ssh_host` and `confirm_remove_ssh_host`, `import_owner_key`. |
| Add a computer | Scan or paste a `coder-host:` invitation, approve a headless host's 8-character code, connect over SSH with its progress, and run with no local host. A phone doesn't show SSH. | `scan_invitation`, `paste_invitation`, `enter_code`, `deny`, `connect_ssh`, `run_without_host`. |
| Access | This device's rights on the host, the enrolled devices with rights, origin, and the time the host last saw each, and a new invitation with a chosen subset of rights, shown as its string and a QR code. | `refresh_devices`, `toggle_right`, `create_invitation`, `cancel_invitation`, `dismiss_invitation`, `revoke` and `confirm_revoke`. |
| Activity | The newest activity summary per task or session, attention first, marked when its host is not online, with its revision. An open task (queued, running, or waiting) offers **Steer** and **Stop task** when this device may operate its host. | `refresh`, `steer_task`, `cancel_task` and `confirm_cancel_task`. |
| Host | One host, opened from its row: status and route, this device's rights, the workspaces it shares, **Order work**, **Terminal**, **Access**, and its recent work with the same controls as Activity. | `show`, `open_terminal`, `retry_now`, `steer_task`, `cancel_task`. |
| Order work | A workspace the host lists through `workspace.list` (or a typed name when it lists none), a prompt, and **Send task**, which sends NIP-HOST `task.create` and moves to Activity. | `choose_workspace`, `enter_workspace`, `refresh_workspaces`, `write_prompt`, `submit_task`. |

**Terminal** needs the `terminal` right and an online host. The controller
records the host and returns `Outcome::Terminal`; the client takes the host
from `Computers::take_terminal` and shows its own terminal screen. Ordering,
steering, and stopping need `operate`. A task's title is the prompt's first
line, at most 80 characters. Sending a task records it on the host; it runs
only under the host's own execution policy.

## Status

A row's status is one of six words, and the text always says which:

| Status | Derived from |
| --- | --- |
| Online | A `coder-link` supervisor in `Connected`. The line also names the route and whether data is current, catching up, or failed to update. Transport health and data freshness stay separate. |
| Connecting | `Connecting(Establishing \| Probing \| Replacing)`. |
| Offline | Switched off, not connected, no network, retrying after a failure, or blocked because the host refused this device's key or its settings are invalid. |
| Out of date | A NIP-REACH presence that fails the compatibility rule, naming the side to update, or a supervisor blocked as `Incompatible`. |
| Not enrolled | No grant, or an expired one. A host your owner directory lists and this device has no grant for says so. |
| Revoked | The host revoked this device, or the supervisor is blocked as `Revoked`. |

Access outranks compatibility, which outranks transport, so a revoked host
never reads as merely offline.

## Authority

One function, `authority::check`, decides both whether a control is enabled
and whether its intent may run. A disabled control is followed by a status
text node keyed `<control>-reason` that says why. The controller resolves an
activation only against its current validated view, runs the same check
against the current snapshot, and then calls the service. The host still
checks its own grant record for every operation; passing the check grants
nothing. Invitations only narrow: a device can share only rights it holds,
and approval grants the intersection of the request and the approver's
rights. A platform that draws a control natively, such as a row
in a native list, runs its intent through `Computers::perform`, which makes
the same check.

## Services

`ComputersService` is the seam between these screens and a client that owns
grants, connections, and relay traffic. Three services implement it:

- `live::Live`, behind the default `live` feature, is the real client. It
  uses the resident host client in
  [`coder-host`](../coder-host/README.md) (`coder_host::client`), the
  portable `coder-access` device client, `coder-reach` presence, and one
  `coder-link` registry with a supervisor per enrolled host. The iOS and
  Android apps and the terminal example use it.
- `Unavailable` returns an empty list and refuses every effect with a reason
  the screens show. The mobile app falls back to it when its protected store
  cannot open.
- `synthetic::Synthetic` is an offline fixture. Each host's status comes
  from a real `coder-link` supervisor driven by scripted reports, and its
  activity passes through `nostr::activity_summary::encode`. It contacts no
  host, relay, or SSH server.

### The live service

- **Enrollment.** A scanned or pasted `coder-host:` invitation is redeemed
  with this device's key. The verified access record is saved through a
  `live::Store`: the mobile app's store encrypts it under the device key
  from the platform's protected store, and `live::FileStore` writes it
  owner-only for the terminal and desktop. The host is then registered and
  connected.
- **Status.** A background task drains connector reports into the registry
  and ticks its timers, so a row's transport status moves on its own. Data
  freshness is separate: after each connection and every
  `Settings::refresh_every` (30 seconds by default, sooner while a screen
  polls), the service reads presence for compatibility, activity summaries,
  the device list with `access_read`, and enrollment requests with
  `access_admin`, then reports the data current or the update failed.
- **Access.** `device.list`, `invite.create`, `invite.cancel`,
  `device.revoke`, `enroll.approve`, and `enroll.deny` run over the host's
  current link, direct or relay. A signed `revoked` refusal, or a channel the
  host closed as revoked, blocks the supervisor and marks the host revoked on
  this device, which persists.
- **Lifecycle.** `ComputersService::application` passes the application's
  foreground and background to every supervisor, which probes its connection
  after a short absence and replaces it after a long one.
- **Owner directory.** The device reads the owner directory only with the
  owner key, under the owner-authority rule in
  [NIP-REACH](../../nips/openagents/NIP-REACH.md#owner-authority-on-a-client):
  either its device key is the owner a held grant names, or the person enters
  the owner key with **Enter owner key**, and the service accepts it only
  when a held grant names that key as owner. `live::Saved::owner` keeps it in
  the same protected store as the grants. The pump reads the directory from
  the owner's relays every `Settings::refresh_every`, never accepts a lower
  revision than one it trusts, and reports a conflict at the top revision.
  Listed hosts take the directory's label and weight; a listed host with no
  grant here shows as not enrolled, with no controls that need a connection.
  **Add to directory** on an enrolled host asks for a label and publishes the
  next revision with weight 100.
- **Directory editing.** On a listed host, **Rename**, **Change weight** (0 to
  1,000), and **Remove from directory** each publish the next revision. Each
  intent carries the revision the screen showed; the controls are disabled
  without the owner key, before a successful read, and during a conflict,
  and both the controller and the service refuse an edit made against
  another revision as stale. Two different bodies at the top revision stay a
  visible conflict: the list keeps the version this device trusted, and
  **Keep this device's version** publishes that version one revision above
  the conflict, without merging. A removed host this device holds a grant
  for stays in the list and reachable, says it was removed, and leaves
  placement; **Add to directory** lists it again. A removed host with no
  grant here leaves the list.
- **Placement.** `Snapshot::place` and `Snapshot::assess_placement` apply the
  NIP-REACH placement rule to the snapshot: directory weights, weight 0 for a
  host the owner removed from the directory, or weight 100 for a host it never
  listed, the newest accepted presence, and admission only for an online host
  this device may operate.
- **SSH.** With the `ssh` feature and `Settings::ssh`, **Connect over SSH**
  asks for a destination, then runs `coder-ssh` on a thread: install or reuse
  the pinned release, start or adopt the host, and redeem the invitation
  `coder host invite` prints. `ssh` prompts reach the screen through
  `Snapshot::ssh` and become masked input requests; closing one refuses it.
  The setup then opens a `coder-ssh` tunnel and gives its forwarded loopback
  port to the connector as a local route, tried before the host's hints and
  the relay. The route exists only in this process: it is same-machine
  evidence for that one address under NIP-REACH, so it is never saved,
  published, or offered to another device. When the tunnel's `ssh` process
  ends, the route is cleared and the host is reached through its relay; the
  row says so. Nothing stops the host because a tunnel ended, and forgetting
  the computer never stops it either. **Remove over SSH** asks first, then runs
  `coder-ssh`'s explicit remove, which stops a host this app's setup started
  and detaches from one that was already running, forgets the computer, and
  shows which happened. A failed remove keeps the computer in the list.
- **Limits.** A phone claims another machine's locality, so it uses LAN,
  tailnet, or public hints and the relay, never loopback. A tunnel opens only
  during **Connect over SSH**; after the app restarts, or once the tunnel
  closes, the host stays on its other routes until you connect it over SSH
  again. An attempt reads presence from the relay before it tries the
  tunnel, so the tunnel doesn't help while the relay is down. **Remove over
  SSH** doesn't change the owner directory; remove the host there
  separately.

## Terminal

`terminal` is the screen the Host screen's **Terminal** control opens, with
[NIP-TERM](../../nips/openagents/NIP-TERM.md):

- `terminal::model::Model` holds the session's phase (connecting, opening,
  attached, reconnecting, exited, lost, closed, refused, or left), a
  [`coder-vt`](../coder-vt/README.md) emulator, gap counts, and the Ctrl the
  accessory row latched.
- `terminal::view` draws a Rust Native tree in the amber palette: a header
  with the status and **Back**, **End terminal**, or **Open a new
  terminal**; one `terminal`-role text node or run stack per grid row, with
  the cursor as an inverse block; and the accessory row (Esc, Tab, Ctrl, Left, Up,
  Down, Right, Ctrl-C, and Paste). Every color maps onto the ladder, and a busy
  screen stays within the view's node bound by drawing its busiest rows plain.
- `terminal::session::Session` (the `live` feature) runs the session over the
  live service's current link, from `live::Live::terminals`: NIP-HOST
  `terminal.open` in the host's default workspace, then NIP-TERM attach,
  input, resize, and close. It orders frames with `coder_host::client::Ordered`,
  marks gaps in the output, reattaches after the frames it applied when the
  link drops, the supervisor replaces the route, or a frame goes missing for
  two seconds, and reports `lost` when the host restarted. Typed input is never
  queued while detached; the screen says it wasn't sent.

The **Terminal** control is enabled only for an online host on which this
device holds `terminal`; otherwise its reason says which right is missing. The
host checks the right again on every request, and a host refusal shows as the
screen's status.

## QR codes

A created invitation shows as its string and as a QR code rendered on this
device with the local renderer that `coder-connect` uses for pairing codes;
it never reaches a QR-generation service. The terminal and desktop adapters
draw it in the tree as half-block text (`qr::text`). A phone host draws the
modules from `Computers::invitation_qr` natively.

## Input

A Rust Native tree cannot collect text yet. When a screen needs a value, the
controller publishes an `InputRequest`, Rust Native's
[input request](../rust-native/docs/spec.md#input-requests) with this crate's
purposes: a token, a purpose, a label, whether to open the scanner first, and
whether the value is a secret to mask. The platform shows its native field or
scanner and returns the value with the token. A secret request, such as an
SSH password or the owner key, gets a masked field: a SwiftUI `SecureField`
on iOS and a password-type field on Android. **Enter owner key** is offered
on every platform, phones included. Rust validates each value: the
`coder-host:` prefix, the approval code's shape, an SSH destination that
cannot be an option, a directory label's bounds, and a weight from 0 to
1,000.
An SSH password or passphrase passes exactly as typed.

## Try it

```sh
cargo run -p coder-computers --features ssh --example terminal            # interactive
cargo run -p coder-computers --features ssh --example terminal -- --print # each screen as text
cargo run -p coder-computers --features ssh --example terminal -- --live ~/.openagents/coder-computers
```

The example uses the offline fixture unless you pass `--live DIR`, which runs
the live service with its device key and grants owner-only in `DIR`.
`--loopback-test` admits a `ws://` loopback relay for a local test, and
`--same-machine` states that the hosts run on this computer. To offer SSH,
add `--ssh-archive OS/ARCH=PATH` for each `coder` release archive, plus
`--owner KEY` and `--relay URL` for the host it starts. Tab and the arrow
keys move focus, Enter or Space activates, and `q` quits. Secret input shows
as asterisks.

## Checks

```sh
cargo test -p coder-computers --features ssh
cargo clippy -p coder-computers --features ssh --all-targets -- -D warnings
cargo clippy -p coder-computers --no-default-features --lib -- -D warnings
cargo fmt -p coder-computers -- --check
```

`tests/live.rs` runs the live service against a real `coder host serve` on
the synthetic relay: the owner directory, and an SSH setup through the fake
`ssh` harness in `crates/coder-ssh/tests/support/fake_ssh.rs`. The
[verification record](../../docs/coder/verification/2026-09-27-client-directory-and-ssh.md)
lists what it establishes. `tests/edits.rs` adds directory editing, removal,
conflict, and stale refusal, and the SSH tunnel route, relay fallback, and
both remove outcomes; its
[verification record](../../docs/coder/verification/2026-09-27-directory-edit-and-ssh-routes.md)
lists them.
