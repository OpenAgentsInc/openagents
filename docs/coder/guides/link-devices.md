# Link your devices

Connect a computer once, and your phone can send it Coder work from a chat and,
if you allow it, open a terminal on it.

## Connect a computer with the desktop app

1. Download **OpenAgents** for Mac, drag it to Applications, and open it.
2. The window shows a QR code and **Scan with the OpenAgents app on your
   phone.**
3. On your phone, tap **Connect a computer** (the chip under a reply, or
   Account > Computers) and point the camera at the code.
4. Both screens say the computer is connected. **Run Coder** in a chat now
   sends work to that Mac.

Check **Let this phone open a terminal** before the code shows if you want
the phone to open terminals there. To take a phone's access away, click
**Remove** next to it in the desktop app.

No Tailscale, no terminal, and no key to copy. The phone connects directly
when it can and through the OpenAgents relay when it cannot, so it works on
the same Wi-Fi and on mobile data.

On a computer without a screen, `openagents connect invite` prints the same
code in the terminal; `openagents connect devices`, `remove`, and `status`
manage it.

> **Status (2026-09-29).** The desktop app, **Connect a computer** on the
> phone, and `openagents connect` are being built under
> [#9965](https://github.com/OpenAgentsInc/openagents/issues/9965)
> ([design](../design/2026-09-29-auto-pairing.md)). Until they reach you, use
> [Using Tailscale (optional)](#using-tailscale-optional) below.

## How authority works

- Each computer's **host** has its own key and is the only thing that grants
  rights. A scanned code, a network, or a route grants nothing by itself.
- A code is good for one phone, once, and only for a short time. A phone
  that scans it gets `observe` and `operate`, plus `terminal` only if you
  checked the box first.
- The host rechecks a phone's grant on every message, and **Remove** closes
  its channels at once.

Protocol details are in three drafts:
[NIP-HOST](../../../nips/openagents/NIP-HOST.md) enrolls devices with
host-signed grants, [NIP-REACH](../../../nips/openagents/NIP-REACH.md) lists
hosts in an owner directory and proves routes, and
[NIP-TERM](../../../nips/openagents/NIP-TERM.md) carries terminals.

## Computers set up the old way

The desktop app finds a computer already set up with `coder link` and asks
**Use this Mac's existing Coder setup?** Say yes: it keeps the same host key,
grants, projects, and tasks, so phones you already enrolled keep working
without scanning again.

## Using Tailscale (optional)

Tailscale is a supported alternative to the desktop app. If your devices
already share a tailnet, they can reach each other directly over it, with
`wss://relay.openagents.com/` as the fallback when no direct route works.

Tailnet admission (`coder host serve --tailnet-admission RIGHTS`) stays
available: a phone signed in as the same Tailscale user as the computer gets
a one-use invitation from the host, with the rights you chose, and can read
chat history directly over the tailnet.

The setup commands below, `coder link`, `scripts/link-device.sh`, `coder pair`
and `./pair`, are **deprecated**
([#9978](https://github.com/OpenAgentsInc/openagents/issues/9978)) and print a
notice naming the replacement. Each keeps working for one release after the
desktop app and nearby pairing ship; then they are removed. Until then, this
is how to set up computers over Tailscale.

In this path your **owner key** is the Nostr key every host serves. It lives
in one private file on one computer, and hosts receive only its public half.
Tailscale, SSH, and the relay only introduce devices; a tailnet address
grants nothing. Each invitation is one-use, expires in five minutes, and
carries rights you choose every time.

## Before you start

- Tailscale runs and is signed in on every device. With **HTTPS
  certificates** turned on for the tailnet in the Tailscale admin console,
  hosts serve `wss` with a certificate from `tailscale cert`; otherwise they
  serve plain `ws` on the tailnet. Channel traffic is encrypted and
  authenticated either way.
- Each computer has a checkout of this repository and the pinned Rust
  toolchain. Computers you set up remotely accept `ssh DEST` without a
  password prompt.

### Create the owner key

On the computer you keep, run this once:

```sh
coder link owner init
```

It writes the key to `~/.openagents/coder-owner/owner.key` (mode `0600`) and
prints only the public key, in hex and as an `npub`. Set
`OPENAGENTS_OWNER_KEY_FILE` or pass `--owner-key FILE` to keep it elsewhere.
Back the file up; a host established with this owner refuses another.

### Link this computer

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

### Link another computer over SSH

```sh
scripts/link-device.sh --ssh coderos-4080 \
  --workspace openagents=/home/you/openagents-host-tasks --linger
```

This fast-forwards the remote checkout (`--remote-checkout DIR`, default
`~/openagents`) to `origin/main` if it is clean, builds there, and runs
`coder link setup --ssh`, which sets the remote host up with your owner's
public key and lists it with the owner key held here. The owner key never
leaves this computer.

### Let the computers reach each other

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

### Enroll your phone

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

### Order work from a device

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

### Inspect and undo

| To | Run |
| --- | --- |
| See the host, its listener, service, devices, joined hosts, and directory | `coder link status` |
| List or edit the owner directory | `coder link directory list`, `add --host KEY --label NAME`, `remove --host KEY` |
| Revoke a device | `coder host revoke --device KEY` |
| Stop the host | `coder-service service uninstall` |

### Where things live

| Path | Contents |
| --- | --- |
| `~/.openagents/coder-owner/owner.key` | The owner secret key, on the owner's computer only. |
| `~/.openagents/coder-access/` | The host key and grants. |
| `~/.openagents/host/serve.json` | Relays, workspaces, and the WebSocket listener the host serves. |
| `~/.openagents/host/tls/` | The certificate chain and key from `tailscale cert`. |
| `~/.openagents/host/autostart.json`, `autostart.jsonl` | The auto-start policy and its decisions. |
| `~/.openagents/coder-computers/` | This computer's device key and the hosts it joined, shared with the Computers screens. |

### Limits

- The host reads its certificate only at start. `tailscale cert`
  certificates last 90 days; re-run `coder link setup` to renew and restart.
- Evidence for this guide is in the
  [link devices record](../verification/2026-09-27-link-devices.md).
