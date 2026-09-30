# Link your devices

Connect a computer once, and your phone can send it Coder work from a chat and
open a terminal on it.

## Connect a computer with the desktop app

1. Download **OpenAgents** for Mac, drag it to Applications, and open it.
2. The window shows a QR code and **Scan with the OpenAgents app on your
   phone.**
3. On your phone, tap **Connect a computer** (the chip under a reply, or
   Account > Computers) and point the camera at the code.
4. Both screens say the computer is connected. **Run Coder** in a chat now
   sends work to that Mac.

The phone gets full permission, terminals included; there is nothing to
check first. To take a phone's access away, click **Remove** next to it in
the desktop app.

No Tailscale, no terminal, and no key to copy. The phone connects directly
when it can and through the OpenAgents relay when it cannot, so it works on
the same Wi-Fi and on mobile data.

On a computer without a screen, `openagents connect invite` prints the same
code in the terminal; `openagents connect devices`, `remove`, and `status`
manage it.

## How authority works

- Each computer's **host** has its own key and is the only thing that grants
  rights. A scanned code, a network, or a route grants nothing by itself.
- A code is good for one phone, once, and only for a short time. A phone
  that scans it gets full permission: every right, a terminal included.
  To take a phone's access away, **Remove** it.
- The host rechecks a phone's grant on every message, and **Remove** closes
  its channels at once.

Protocol details are in three drafts:
[NIP-HOST](../../../nips/openagents/NIP-HOST.md) enrolls devices with
host-signed grants, [NIP-REACH](../../../nips/openagents/NIP-REACH.md) lists
hosts in an owner directory and proves routes, and
[NIP-TERM](../../../nips/openagents/NIP-TERM.md) carries terminals.

## Computers set up the old way

The desktop app finds a computer set up by the old setup commands (removed in
[#9978](https://github.com/OpenAgentsInc/openagents/issues/9978)) and asks
**Use this Mac's existing Coder setup?** Say yes: it keeps the same host key,
grants, projects, and tasks, so phones you already enrolled keep working
without scanning again. `coder-host:` invitations from `coder host invite` still
redeem on the phone (**Add a computer > Scan invitation**), and grants issued
before are unchanged.

## A computer you reach over SSH

```sh
openagents connect --ssh coderos-4080
```

It installs or updates the `openagents` binary there, starts its host, and
enrolls this computer over the SSH channel; `--import-owner` makes this
computer's owner the new host's owner. See `openagents connect --ssh --help`.

## Order work from a computer

A device holding `operate` creates a task with NIP-HOST `task.create`:

```sh
openagents computer task coderos-4080 --workspace openagents \
  --title "Flaky parser test" Fix the flaky parser test
```

The task only queues. To let the host start work that your devices order,
turn on its [auto-start policy](../runtime/host-autostart.md):

```sh
coder host autostart on --workspace openagents --max-running 1
coder host autostart show
coder host autostart off
```

## Using Tailscale (optional)

Tailscale is a supported alternative route. If your devices already share a
tailnet, they can reach each other directly over it, with
`wss://relay.openagents.com/` as the fallback when no direct route works. A
tailnet address only introduces a device; it grants nothing.

A host serves a tailnet listener when you give it one. For a host you run
with `coder host serve`, record it once with `coder host init` (or pass the
same options to `serve`):

```sh
coder host init --owner OWNER_PUBKEY --relay wss://relay.openagents.com/ \
  --workspace openagents=$HOME/work/openagents-host-tasks \
  --listen-websocket 100.101.102.103:47101 --allow-nonloopback \
  --websocket-tls-cert ~/.openagents/host/tls/chain.pem \
  --websocket-tls-key ~/.openagents/host/tls/key.pem \
  --websocket-name box.tailnet.ts.net \
  --advertise tailnet=wss://box.tailnet.ts.net:47101/
```

Use this computer's tailnet address and MagicDNS name (`tailscale status`).
With **HTTPS certificates** turned on for the tailnet, `tailscale cert` issues
the chain and key; without them, leave out the three `--websocket-*` options
and advertise `ws://` instead. Channel traffic is encrypted and authenticated
either way. The host reads its certificate only at start, and `tailscale
cert` certificates last 90 days: renew and restart the host.

Tailnet admission (`--tailnet-admission RIGHTS`) lets a phone signed in as
the same Tailscale user as the computer get a one-use invitation from the
host, with the rights you chose, and read chat history directly over the
tailnet. See [Host serve](../runtime/host-serve.md) for every option.

## Inspect and undo

| To | Run |
| --- | --- |
| See the host | `openagents connect status` |
| List the phones that can reach it | `openagents connect devices` |
| Revoke a device | `openagents connect remove DEVICE`, or **Remove** in the desktop app |
| Stop a host installed as a service | `coder-service service uninstall` |

## Where things live

| Path | Contents |
| --- | --- |
| The OS keychain | The owner, host, and iroh keys of a host the desktop app runs. |
| `~/.openagents/coder-access/` | Grants, and the host key of a host set up without the desktop app. |
| `~/.openagents/host/serve.json` | Relays, workspaces, and listeners the host serves. |
| `~/.openagents/host/autostart.json`, `autostart.jsonl` | The auto-start policy and its decisions. |
| `~/.openagents/coder-computers/` | This computer's device key and the hosts it joined, shared with the Computers screens. |
