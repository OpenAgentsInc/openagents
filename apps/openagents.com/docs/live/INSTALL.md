# Install OpenAgents Software

This is the public install guide served at <https://openagents.com/INSTALL.md>.
The former `clients/` applications are retired and removed. Do not recover or
distribute them as current products.

## OpenAgents apps

The supported web product is <https://openagents.com>. OpenAgents mobile and
OpenAgents Desktop are owned at `apps/openagents-mobile` and
`apps/openagents-desktop`. Public installation remains gated on their current
signed release receipts.

## Connect Codex capacity with Pylon

```sh
npm install -g @openagentsinc/pylon
pylon auth codex
pylon accounts list --json
```

`pylon auth codex` uses an isolated per-account home and does not touch the
default `~/.codex` session. Each distinct ready account adds separate local
capacity. Never run a device-login flow against an owner’s existing default
Codex home without explicit permission.

Zero-install node startup is also available:

```sh
npx @openagentsinc/pylon
```

Installing or running a Pylon proves capability only. Paid work, acceptance,
earnings, and settlement require their own dereferenceable receipts.

For agent/operator detail, see <https://openagents.com/AGENTS.md>.
