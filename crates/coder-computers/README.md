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
| Computers | One row per host: label, status, route in use, and the reason when blocked. | `set_enabled` (switch off without forgetting), `retry_now`, `forget` and `confirm_forget`, `show` the host's access. |
| Add a computer | Scan or paste a `coder-host:` invitation, approve a headless host's 8-character code, connect over SSH, and run with no local host. | `scan_invitation`, `paste_invitation`, `enter_code`, `deny`, `connect_ssh`, `run_without_host`. |
| Access | This device's rights on the host, the enrolled devices with rights, origin, and the time the host last saw each, and a new invitation with a chosen subset of rights, shown as its string and a QR code. | `refresh_devices`, `toggle_right`, `create_invitation`, `cancel_invitation`, `dismiss_invitation`, `revoke` and `confirm_revoke`. |
| Activity | The newest activity summary per task or session, attention first, marked when its host is not online. | `refresh`. |

## Status

A row's status is one of six words, and the text always says which:

| Status | Derived from |
| --- | --- |
| Online | A `coder-link` supervisor in `Connected`. The line also names the route and whether data is current, catching up, or failed to update. Transport health and data freshness stay separate. |
| Connecting | `Connecting(Establishing \| Probing \| Replacing)`. |
| Offline | Switched off, not connected, no network, retrying after a failure, or blocked because the host refused this device's key or its settings are invalid. |
| Out of date | A NIP-REACH presence that fails the compatibility rule, naming the side to update, or a supervisor blocked as `Incompatible`. |
| Not enrolled | No grant, or an expired one. |
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
rights.

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
- **Limits.** It does not start hosts over SSH, read an owner directory, or
  label a host beyond `Computer` and a key prefix. A phone claims another
  machine's locality, so it uses LAN, tailnet, or public hints and the relay,
  never loopback.

## QR codes

A created invitation shows as its string and as a QR code rendered on this
device with the local renderer that `coder-connect` uses for pairing codes;
it never reaches a QR-generation service. The terminal and desktop adapters
draw it in the tree as half-block text (`qr::text`). A phone host draws the
modules from `Computers::invitation_qr` natively.

## Input

A Rust Native tree cannot collect text yet. When a screen needs a value, the
controller publishes an `InputRequest` with a token, a purpose, a label, and
whether to open the scanner first. The platform shows its native field or
scanner and returns the value with the token. Rust validates it: the
`coder-host:` prefix, the approval code's shape, and an SSH destination that
cannot be an option.

## Try it

```sh
cargo run -p coder-computers --example terminal            # interactive
cargo run -p coder-computers --example terminal -- --print # each screen as text
cargo run -p coder-computers --example terminal -- --live ~/.openagents/coder-computers
```

The example uses the offline fixture unless you pass `--live DIR`, which runs
the live service with its device key and grants owner-only in `DIR`.
`--loopback-test` admits a `ws://` loopback relay for a local test, and
`--same-machine` states that the hosts run on this computer. Tab and the arrow keys move focus,
Enter or Space activates, and `q` quits.

## Checks

```sh
cargo test -p coder-computers
cargo clippy -p coder-computers --all-targets -- -D warnings
cargo fmt -p coder-computers -- --check
```
