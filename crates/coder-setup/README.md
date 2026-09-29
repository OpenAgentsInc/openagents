# coder-setup

`coder-setup` is `coder link`: it makes a computer a serving Coder host that
the owner's other devices reach over Tailscale, with
`wss://relay.openagents.com/` as the fallback. The
[link your devices guide](../../docs/coder/guides/link-devices.md) covers
setup and operation; this README covers the crate.

**Deprecated** ([#9978](https://github.com/OpenAgentsInc/openagents/issues/9978)):
the OpenAgents desktop app's QR code and `openagents connect` replace
`coder link` ([design](../../docs/coder/design/2026-09-29-auto-pairing.md)).
Every command prints `cli::DEPRECATED` on standard error, once per run
(`OPENAGENTS_LINK_NOTICE_SHOWN=1` marks a caller that already printed it,
including the remote side of `--ssh`), and keeps working for one release
after its replacements ship. Then this crate is removed.

## Modules

| Module | What it does |
| --- | --- |
| `owner` | The owner key file: create it `0600` in a `0700` directory, load it only when it is private, and never echo it. Hosts get the public half only. |
| `tailscale` | Reads `tailscale status --json` for the tailnet IPv4 address, the MagicDNS name, and whether the tailnet issues certificates, and runs `tailscale cert`. |
| `plan` | Builds the host's recorded `serve.json` settings: a WebSocket listener on the tailnet address, TLS with the MagicDNS name when a certificate exists, and one `tailnet` hint. |
| `service` | Decides whether to install the host service, update it to the staged bundle, restart it, or leave it, and runs `coder-service`. |
| `directory` | NIP-REACH owner-directory edits under the owner-authority rule: read every retained revision, refuse a conflict, reuse the read revision's mailbox, and publish nothing when nothing changed. |
| `devices` | This computer as a device of other hosts, in the Computers screens' store (`~/.openagents/coder-computers`): join by invitation and prove every route. |
| `ssh` | Runs `coder link` on another computer through the system `ssh`, quoting every word. |
| `cli` | `coder link setup`, `invite`, `join`, `peer`, `check`, `status`, `owner`, and `directory`. |

## Authority

- Tailscale and SSH only introduce computers. The host grants rights, and
  only through NIP-HOST grants it signs; a tailnet address is a route.
- `coder link invite` requires `--rights`; nothing mints a grant with
  implied rights. Invitations print only on the terminal that asked for
  them, or travel on an SSH channel's standard streams. They never appear in
  an argument, a log line, or a file.
- The owner secret key stays in its file on the owner's computer. `setup
  --ssh` sends the remote computer the owner's public key and lists the
  remote host with the local key.

## Tests

```sh
cargo test -p coder-setup
cargo clippy -p coder-setup --all-targets -- -D warnings
```

`tests/link.rs` serves a real host on a local test relay, joins it by
invitation, proves the direct TCP and WebSocket routes and the relay route,
lists the host in the owner directory without duplicate revisions, and sees
every route fail after revocation.
