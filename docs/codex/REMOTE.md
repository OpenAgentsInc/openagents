# Autopilot Codex Remote

Autopilot Desktop can expose a thin authenticated Codex companion UI for a
second device on the same trusted network.

This is local-first remote access, not a hosted cloud product:

- the repo, tools, Codex auth, wallet, and provider runtime stay on the main
  machine
- the browser is only a control and observation surface into that machine

## Enable

Show current status:

```text
/remote
```

Enable on loopback:

```text
/remote enable
```

Enable on a LAN or Tailnet-style address:

```text
/remote enable 192.168.1.25:4848
```

Rotate the auth token:

```text
/remote rotate-token
```

Disable remote access:

```text
/remote disable
```

When remote access is enabled, Desktop keeps a tokenless base URL and a pairing
URL with `#token=...` in app-owned state. The pairing URL is copied to the
clipboard on enable and token rotation.

The owner-only `control` iOS app should pair from that full pairing URL. It is
not supposed to ask the operator for a separate base URL and bearer token.

When the remote companion is bound to a non-loopback LAN or Tailnet-reachable
address, Desktop also advertises `_openagents-control._tcp` over Bonjour on the
local network. The `control` iOS app can browse that service and consume the
advertised pairing URL directly, while loopback-only binds intentionally remain
non-discoverable.

## Bind Safety

Remote binds are restricted to:

- loopback
- RFC1918 private IPv4
- Tailnet-style CGNAT IPv4 (`100.64.0.0/10`)
- IPv6 loopback or ULA

Public addresses are rejected.

## Browser Surface

Remote v1 supports:

- thread list and active-thread transcript
- pending command/file approvals
- pending tool user-input prompts
- Codex readiness and current session controls
- saved plan artifact and latest diff artifact
- wallet balance and provider online/offline truth
- a narrow follow-up composer

Remote v2 extends that with:

- active workspace and project identity
- git branch and dirty/clean truth
- cached worktree inventory for the active repo
- read-only visibility into the active thread terminal session
- Tailnet roster visibility so a paired phone can discover the current device set

Remote v1 intentionally does not support:

- wallet send or withdraw actions
- destructive config writes
- unauthenticated data endpoints

## Auth Model

- The root page is a thin static shell.
- Snapshot and action endpoints require `Authorization: Bearer <token>`.
- The pairing URL uses `#token=...` so the token stays in browser-side fragment
  state instead of being sent as part of the initial page request.
- Disabling remote access shuts down the listener and invalidates the prior
  token.
