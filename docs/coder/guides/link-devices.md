# Link your devices

This guide makes each of your computers a Coder host that your phone and
your other computers reach over Tailscale, with `wss://relay.openagents.com/`
as the fallback when no direct route works. When you finish, your phone can
open a terminal on any linked computer and order coding work from it.
[Issue #9731](https://github.com/OpenAgentsInc/openagents/issues/9731)
delivers the `coder link` command as part of the
[linked devices program](https://github.com/OpenAgentsInc/openagents/issues/9736).

`coder link` builds on three drafts: [NIP-HOST](../../../nips/openagents/NIP-HOST.md)
enrolls devices with host-signed grants, [NIP-REACH](../../../nips/openagents/NIP-REACH.md)
lists your hosts in an owner directory and proves routes, and
[NIP-TERM](../../../nips/openagents/NIP-TERM.md) carries terminals.

## How authority works

- Your **owner key** is the Nostr key every host serves. It lives in one
  private file on one computer. Hosts receive only its public half.
- Each computer's **host** has its own key and grants rights to devices.
  Tailscale, SSH, and the relay only introduce devices; none of them is a
  login, and a tailnet address grants nothing.
- A device joins a host by redeeming a one-use **invitation** that expires
  in five minutes. You choose its rights every time you mint one.
- The host rechecks a device's grant on every message, and revoking the
  device closes its channels at once.

## Before you start

- Tailscale runs and is signed in on every device. With **HTTPS
  certificates** turned on for the tailnet in the Tailscale admin console,
  hosts serve `wss` with a certificate from `tailscale cert`; otherwise they
  serve plain `ws` on the tailnet. Channel traffic is encrypted and
  authenticated either way.
- Each computer has a checkout of this repository and the pinned Rust
  toolchain. Computers you set up remotely accept `ssh DEST` without a
  password prompt.

## Create the owner key

On the computer you keep, run this once:

```sh
coder link owner init
```

It writes the key to `~/.openagents/coder-owner/owner.key` (mode `0600`) and
prints only the public key, in hex and as an `npub`. Set
`OPENAGENTS_OWNER_KEY_FILE` or pass `--owner-key FILE` to keep it elsewhere.
Back the file up; a host established with this owner refuses another.

## Link this computer

From the checkout:

```sh
scripts/link-device.sh --workspace openagents=$HOME/work/openagents-host-tasks
```

The script builds and installs `coder`, `coder-service`, and `microcoder`
into `~/.openagents/bin`, stages the `coder` build as the host service's
bundle, and runs `coder link setup`, which:

1. Establishes the host with your owner's public key (`coder host init`).
2. Reads this computer's tailnet address and MagicDNS name, gets a
   certificate with `tailscale cert` when the tailnet issues one, and records
   a WebSocket listener on the tailnet address, port `47101`, advertised as a
   `tailnet` hint such as `wss://box.tailnet.ts.net:47101/`.
3. Installs the host service (a launchd agent or a systemd user unit), moves
   it to the staged bundle, or restarts it so new settings apply, and waits
   for the listener.
4. Lists the host in your owner directory under its MagicDNS short name, or
   `--label NAME`.

Run it again at any time: it changes only what changed, and it renews the
certificate. A workspace is a label a device names; the device never sends
a path. Give the host a dedicated worktree so work you order from a phone
never touches the checkout you work in:

```sh
git -C ~/work/openagents worktree add --detach ~/work/openagents-host-tasks origin/main
```

On Linux, add `--linger` so the host starts at boot and survives logout.

## Link another computer over SSH

```sh
scripts/link-device.sh --ssh coderos-4080 \
  --workspace openagents=/home/you/openagents-host-tasks --linger
```

This fast-forwards the remote checkout (`--remote-checkout DIR`, default
`~/openagents`) to `origin/main` if it is clean, builds there, and runs
`coder link setup --ssh`, which sets the remote host up with your owner's
public key and lists it with the owner key held here. The owner key never
leaves this computer.

## Let the computers reach each other

```sh
coder link peer --ssh coderos-4080
coder link check
coder link check --ssh coderos-4080
```

`peer` enrolls each computer as a device of the other's host with
`observe,operate,terminal,review,access_read`, passing each invitation over
the SSH channel's standard streams. `check` proves every route from this
computer: each direct hint (a channel that proves both keys, then a ping)
and the relay (a signed `device.list` answer).

## Enroll your phone

```sh
coder link invite --rights observe,operate,terminal
```

The command prints a QR code and the `coder-host:` paste string. Within five
minutes, open **Computers**, tap **Add a computer**, then **Scan invitation**
(or **Paste invitation**). Mint one invitation per computer; for another
computer, add `--ssh DEST`. Add `review` or `access_read` only if you want
the phone to have them. The grant lasts 30 days; `--grant-days N` changes
that.

To see your owner directory on the phone, choose **Enter owner key** and
enter the owner key. Without it, the phone lists the hosts it joined.

## Order work from a device

A device holding `operate` creates a task with NIP-HOST `task.create`. From a
linked computer:

```sh
echo "Fix the flaky parser test" | coder link order --host coderos-4080 \
  --workspace openagents --title "Flaky parser test"
```

The task only queues. To let the host start work that your devices order,
turn on its [auto-start policy](../runtime/host-autostart.md):

```sh
coder host autostart on --workspace openagents --max-running 1
coder host autostart show
coder host autostart off
```

## Inspect and undo

| To | Run |
| --- | --- |
| See the host, its listener, service, devices, joined hosts, and directory | `coder link status` |
| List or edit the owner directory | `coder link directory list`, `add --host KEY --label NAME`, `remove --host KEY` |
| Revoke a device | `coder host revoke --device KEY` |
| Stop the host | `coder-service service uninstall` |

## Where things live

| Path | Contents |
| --- | --- |
| `~/.openagents/coder-owner/owner.key` | The owner secret key, on the owner's computer only. |
| `~/.openagents/coder-access/` | The host key and grants. |
| `~/.openagents/host/serve.json` | Relays, workspaces, and the WebSocket listener the host serves. |
| `~/.openagents/host/tls/` | The certificate chain and key from `tailscale cert`. |
| `~/.openagents/host/autostart.json`, `autostart.jsonl` | The auto-start policy and its decisions. |
| `~/.openagents/coder-computers/` | This computer's device key and the hosts it joined, shared with the Computers screens. |

## Limits

- The host reads its certificate only at start. `tailscale cert`
  certificates last 90 days; re-run `coder link setup` to renew and restart.
- Evidence for this guide is in the
  [link devices record](../verification/2026-09-27-link-devices.md).
