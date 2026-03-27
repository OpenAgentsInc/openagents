# 2026-03-26 Tailnet Codex Remote Companion Audit

## Question

What would be needed for a user to use Autopilot's existing in-app Codex
integration over Tailnet from another device, instead of falling back to raw
SSH plus remote `co` or `codex` on the target machine?

## Scope

Reviewed against:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/codex/ROADMAP_CODEX.md`
- `docs/codex/REMOTE.md`

Reviewed implementation surfaces:

- `apps/autopilot-desktop/src/codex_remote.rs`
- `apps/autopilot-desktop/src/input/actions.rs`
- `apps/autopilot-desktop/src/panes/codex.rs`
- `apps/autopilot-desktop/src/app_state_domains.rs`
- `apps/autopilot-desktop/src/desktop_control.rs`
- `apps/autopilot-desktop/src/bin/autopilotctl.rs`
- `apps/autopilot-desktop/src/codex_lane.rs`
- `apps/autopilot-desktop/src/codex_exec.rs`

## Bottom Line

OpenAgents already ships the core Tailnet-safe remote Codex primitive.

This is not a greenfield feature request. The desktop app already has an
opt-in authenticated remote companion listener that:

- allows loopback, RFC1918, Tailnet-style CGNAT, and IPv6 ULA binds
- serves a browser companion UI from the desktop app itself
- projects desktop-owned Codex, wallet, provider, workspace, git, and terminal
  truth into that UI
- accepts remote prompt submission, approval responses, tool-user-input
  responses, and basic session-control changes

So the real gap is not "make Codex remote over Tailnet possible." That part is
already shipped. The real gap is "make Tailnet remote Codex a complete,
discoverable, operator-friendly workflow that can be turned on, observed, and
recovered without depending on chat-only commands or manual network knowledge."

## What Already Works

### 1. Tailnet-safe bind validation already exists

`apps/autopilot-desktop/src/codex_remote.rs` explicitly rejects public bind
addresses and allows:

- loopback
- RFC1918 private IPv4
- Tailnet-style CGNAT IPv4
- IPv6 ULA

That is already the right safety model for a personal remote companion over
Tailnet.

### 2. The desktop already owns the remote listener

The current design is consistent with `docs/MVP.md` and
`docs/OWNERSHIP.md`:

- the main machine remains the machine with the repo, local files, Codex auth,
  wallet, and provider runtime
- the browser is only a control and observation surface
- the remote surface lives in `apps/autopilot-desktop`, which is the correct
  owner for app behavior, workspace state, and UX

No Psionic-side transport or runtime changes are required for this product
layer.

### 3. The remote browser surface is already meaningful

The current remote snapshot and actions are not toy placeholders. The shipped
surface already covers:

- thread list and active thread transcript
- prompt submission into the active thread
- turn interrupt
- command and file-change approvals
- tool user-input prompts
- session controls
- readiness and auth summaries
- latest plan artifact
- latest diff artifact
- wallet summary
- provider online/offline summary
- workspace/project identity
- git branch and dirty truth
- cached worktree inventory
- read-only terminal visibility

That is enough for away-from-desk supervision and light continuation now.

### 4. Tailnet setup is already documented inside `openagents`

`docs/codex/REMOTE.md` already says the remote companion can be enabled on a
LAN or Tailnet-style address with:

- `/remote`
- `/remote enable`
- `/remote enable 192.168.1.25:4848`
- `/remote rotate-token`
- `/remote disable`

The doc also already records the auth model and the bind safety rules.

### 5. Pairing and token handling already follow the right security direction

The current implementation:

- keeps the root page tokenless
- requires `Authorization: Bearer <token>` for snapshot and action endpoints
- places the pairing token in the URL fragment via `#token=...`
- supports token rotation
- invalidates the old token when rotated or disabled

That is already compatible with a trusted Tailnet operator model.

## What Is Still Missing For A Strong Tailnet Workflow

### 1. No app-owned Tailnet discovery or recommended-bind UX

Today the operator must already know a usable Tailnet IP and manually type it
into `/remote enable <ip:port>`.

What is missing:

- detect local interface addresses that match the allowed remote-bind classes
- show the active Tailnet candidate addresses in the desktop UI
- provide one-click bind presets such as `loopback`, `current Tailnet IP`, or
  `custom`
- warn when the requested bind address is syntactically valid but not currently
  assigned to the host

Without this, Tailnet support is real but operator-hostile.

### 2. No `autopilotctl` or desktop-control ownership of Codex remote

This is the biggest practical gap.

The Codex remote companion is currently controlled from:

- chat commands in `apps/autopilot-desktop/src/input/actions.rs`
- readback in the Codex Labs pane

But it is not exposed through:

- `apps/autopilot-desktop/src/desktop_control.rs`
- `apps/autopilot-desktop/src/bin/autopilotctl.rs`

That means:

- a user cannot enable, disable, inspect, or rotate the Codex remote companion
  from the repo's standard operator CLI
- a user who is already on the box over SSH or desktop control cannot manage
  the remote companion through the normal programmatic surface
- automation and runbooks have to tell users to type chat commands instead of
  using the app-owned control plane

For real Tailnet use, Codex remote should be operable through the same
desktop-control contract that already owns provider, wallet, sandbox, training,
and pane workflows.

### 3. No persisted remote-companion policy

The current default is correctly `disabled`, but the product does not appear to
distinguish between:

- one-shot temporary enablement for a single away-from-desk session
- "re-enable remote companion on next launch" behavior
- "re-enable only if this same bind address is still present" behavior

Tailnet IPs can move. The correct product behavior is not "always come back on
the last address blindly."

What is needed:

- explicit persisted remote policy state
- honest restart behavior
- stale-bind detection on launch
- clear user-visible messaging when the last configured Tailnet address is no
  longer present

### 4. No Tailnet health or reachability diagnostics

The current UI reports bind address, base URL, pairing URL, and token preview.
That is useful but incomplete.

For actual Tailnet operator use, the app should also surface:

- whether the requested bind is currently assigned
- whether the listener is actually bound on that address
- whether the selected address looks like loopback, LAN, Tailnet CGNAT, or ULA
- the last local network validation error
- whether the listener had to fall back from `:0` or another requested address

This does not require a full Tailscale integration. It requires honest network
state reporting in the app-owned remote companion status.

### 5. No audit trail for remote actions by source device/address

The remote companion currently acts as a thin control plane, but there is no
clear evidence in the reviewed surfaces that remote actions are recorded with
their source network address or source-session metadata.

For Tailnet use this matters because approvals and prompt submissions can now
come from a second device.

The desktop should log:

- remote session connect/disconnect
- snapshot auth failures
- token rotations
- prompt submissions from remote
- approval actions from remote
- tool-user-input responses from remote

And it should attach at least:

- timestamp
- source remote address
- action type
- affected thread/turn/request ids when applicable

This is operator visibility, not security theater.

### 6. The current pairing flow is good but not yet polished for second-device use

The pairing URL is copied to the clipboard and shown in Labs. That works for a
same-machine operator. It is weaker for a phone or second laptop flow.

Useful product additions:

- QR code rendering for the pairing URL
- separate "copy base URL" and "copy token" actions
- a visible note when the pairing URL uses a non-loopback Tailnet address
- explicit "share this only with devices on your trusted Tailnet" copy

This is not a protocol blocker. It is completion work.

### 7. Remote continuation exists, but remote `exec`-style operator flow does not

OpenAgents already has:

- the live Codex lane
- `autopilot-codex-exec` for app-owned one-shot execution

But the remote companion does not expose a first-class one-shot automation
entrypoint comparable to "run this exact Codex task now and give me the
result."

That is not required for Tailnet supervision. It does matter if the product
goal is to replace the user's current "SSH into the machine and run `codex exec`
there" habit with a first-party remote Autopilot flow.

This should be treated as follow-on work, not as a prerequisite for Tailnet
remote companion viability.

## Recommended Implementation Order

### Phase 1: Make Tailnet remote operable through the standard control plane

Add Codex-remote ownership to desktop control:

- extend `DesktopControlSnapshot` with a `codex_remote` status section
- add `DesktopControlActionRequest` variants for:
  - `GetCodexRemoteStatus`
  - `EnableCodexRemote`
  - `DisableCodexRemote`
  - `RotateCodexRemoteToken`
- add `autopilotctl remote status|enable|disable|rotate-token`

This is the highest-leverage change because it lets the user manage the
existing remote companion through the app's standard operator surface.

### Phase 2: Add bind discovery and Tailnet-friendly presets

Add app-owned interface inspection and recommendation:

- enumerate current host interface addresses
- classify each as loopback, RFC1918, Tailnet CGNAT, or IPv6 ULA
- expose the candidates in Codex Labs and desktop control
- allow one-click bind selection for the recommended Tailnet address

This removes the need to manually know the Tailnet IP before using the feature.

### Phase 3: Add diagnostics and remote audit logging

Add explicit remote companion observability:

- bind/reachability validation status
- source-address logging for remote actions
- auth-failure counters
- last remote action summary

This turns the feature from "works if you know how to trust it" into "works and
is inspectable."

### Phase 4: Improve second-device pairing UX

Add:

- QR code pairing
- explicit copy actions
- clearer mobile-friendly pairing status

This is product polish, not architectural risk.

### Phase 5: Decide whether remote `exec` belongs in scope

Only after phases 1-4:

- decide whether a remote one-shot execution action should wrap the app-owned
  `autopilot-codex-exec` surface
- keep that flow bounded and app-owned
- do not turn the remote companion into a hosted IDE clone

## Non-Goals

Tailnet Codex support should not require:

- public-internet exposure
- a cloud-hosted OpenAgents control service
- a second browser-owned state store
- wallet send or withdraw on the remote surface
- destructive config editing from remote
- Psionic changes
- a full browser terminal before the safety model is stronger

## Ownership

This work belongs in `openagents`, not `psionic`.

Why:

- `docs/OWNERSHIP.md` already assigns app wiring, product behavior, snapshots,
  and provider orchestration to `apps/autopilot-desktop`
- `docs/codex/ROADMAP_CODEX.md` already says remote access, workspace state,
  and terminal/git product behavior belong in `apps/autopilot-desktop`
- the current remote companion is already implemented there

The only cross-repo dependency is documentation/runbook alignment in the root
workspace repo for Tailnet bootstrap and SSH fallback workflows.

## Acceptance Bar

Tailnet Codex support using the in-app Autopilot integration should be
considered complete when all of the following are true:

1. A user can open Autopilot on the main machine and enable the Codex remote
   companion on the current Tailnet address without manually discovering that
   address outside the app.
2. A second device on the same Tailnet can open the pairing URL, authenticate,
   and continue a thread.
3. The user can approve commands, answer tool prompts, and steer the active
   session remotely.
4. The same remote-companion lifecycle is visible and controllable through
   `autopilotctl`, not only through chat commands or Labs.
5. The app records enough remote action and network state to make failures and
   away-from-desk activity auditable.
6. The product still remains a thin remote companion to the local desktop
   runtime rather than drifting into a hosted browser product.

## Conclusion

The current repo is already past the hardest part.

Autopilot does not need a new Tailnet remote Codex architecture. It already has
one. The required work is to operationalize that shipped companion surface so
users can discover it, control it through the standard operator path, diagnose
it when Tailnet/network state changes, and trust it as part of normal daily
Codex use.
