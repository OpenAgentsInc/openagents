# Coder Computers

Coder Computers draws the screens every Coder client uses to manage the
computers you reach: your hosts with an honest status, the ways to add one,
who has access to each, first run, and activity that needs attention. The
screens are Rust Native projections with a closed intent enum. The iOS and
Android hosts mount them, and the terminal adapter in `coder-terminal` draws
them. [Issue #9713](https://github.com/OpenAgentsInc/openagents/issues/9713)
defines the scope within the
[remote access program](https://github.com/OpenAgentsInc/openagents/issues/9704).

## Screens

| Screen | What it shows | Intents |
| --- | --- | --- |
| First run | The ways to add a computer and a **Continue** control that stays disabled until one computer has a current grant. | Every **Add a computer** intent, `continue_onboarding`. |
| Computers | One row per host: label, status, route in use, and the reason when blocked. | `set_enabled` (switch off without forgetting), `retry_now`, `forget` and `confirm_forget`, `show` the host's access. |
| Add a computer | Scan or paste a `coder-host:` invitation, approve a headless host's 8-character code, connect over SSH, and run with no local host. | `scan_invitation`, `paste_invitation`, `enter_code`, `deny`, `connect_ssh`, `run_without_host`. |
| Access | This device's rights on the host, the enrolled devices with rights, origin, and last seen, and a new invitation with a chosen subset of rights. | `refresh_devices`, `toggle_right`, `create_invitation`, `cancel_invitation`, `dismiss_invitation`, `revoke` and `confirm_revoke`. |
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
grants, connections, and relay traffic. The resident host client from issue
#9712 implements it over `coder-access`, `coder-reach`, and a `coder-link`
registry. Until then:

- `Unavailable` returns an empty list and refuses every effect with a reason
  the screens show. The normal mobile app uses it.
- `synthetic::Synthetic` is an offline fixture. Each host's status comes
  from a real `coder-link` supervisor driven by scripted reports, and its
  activity passes through `nostr::activity_summary::encode`. It contacts no
  host, relay, or SSH server.

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
```

The example uses the offline fixture. Tab and the arrow keys move focus,
Enter or Space activates, and `q` quits.

## Checks

```sh
cargo test -p coder-computers
cargo clippy -p coder-computers --all-targets -- -D warnings
cargo fmt -p coder-computers -- --check
```
