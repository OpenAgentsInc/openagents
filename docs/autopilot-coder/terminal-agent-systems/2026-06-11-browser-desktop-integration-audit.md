# Browser And Desktop Integration Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

This is system #37 from the Bun/Effect terminal-agent systems list. It captures
how a terminal coding agent should integrate with browsers and desktop GUI
surfaces: safe URL opening, extension-backed browser control, native desktop
control, screenshot capture, clipboard image ingestion, and desktop app
handoff.

The key rule is that GUI control requires a higher trust boundary than normal
file and shell work. Browser and desktop actions need explicit capability
records, permission probes, and scoped approvals.

## Target

Build a browser and desktop integration layer that can:

- Open URLs and local paths safely.
- Connect to a browser extension or native messaging bridge.
- Capture browser and desktop state as evidence.
- Execute approved browser or desktop actions.
- Ingest screenshots and clipboard images.
- Transfer a terminal session into a desktop app.
- Represent GUI permissions in a durable audit record.

The runtime should support these surfaces without making them mandatory for
normal coding-agent operation.

## User-Visible Capability

Users should be able to:

- Open web auth flows.
- Install or reconnect a browser extension.
- Manage browser site permissions.
- Enable or disable browser integration by default.
- Grant or deny desktop control for a session.
- Paste or attach screenshots from the clipboard.
- Transfer a running session to a desktop app when installed.

The agent should always make it clear whether browser or desktop control is
available, disabled, missing permissions, or unsupported on the current
platform.

## Core Model

```ts
interface GuiCapability {
  readonly capabilityId: string
  readonly kind: "browser" | "desktop" | "clipboard" | "handoff"
  readonly platform: string
  readonly status: "available" | "disabled" | "missing_permission" | "unsupported"
  readonly approvalRequired: boolean
  readonly policyRef?: string
}

interface GuiActionRequest {
  readonly requestId: string
  readonly capabilityId: string
  readonly action: string
  readonly target?: string
  readonly displayId?: number
  readonly coordinateMode?: "pixels" | "normalized"
  readonly reason: string
}

interface GuiEvidence {
  readonly evidenceId: string
  readonly kind: "screenshot" | "clipboard_image" | "browser_log" | "network_log"
  readonly artifactRef: string
  readonly capturedAt: string
  readonly visibility: "private" | "team" | "public_safe"
}
```

GUI evidence should be private by default. Public projection requires explicit
redaction and user or policy authorization.

## URL And Path Opening

The basic opener should be deliberately small:

- Validate URLs before opening.
- Allow only `http` and `https` by default.
- Use platform openers for local paths.
- Avoid shell interpolation where possible.
- Return success or failure without throwing into the main loop.

This path is used for login, docs, extension install, reconnect, and desktop
download flows. It should not be confused with browser automation.

## Browser Bridge

Browser control should be mediated through a browser-side extension or bridge.

Recommended flow:

1. Detect supported browsers.
2. Detect whether the extension is installed.
3. Register or verify native messaging host metadata.
4. Establish a local bridge with secure permissions.
5. Connect the runtime through a typed connector.
6. Surface status in the terminal UI.
7. Let the user manage site-level permissions in the browser extension.

The browser extension should own site permissions. The terminal agent should
not bypass the browser's permission model.

Bridge records should include:

- Browser kind.
- Extension status.
- Native host version.
- Local socket or transport status.
- Site permission status where available.
- Last reconnect time.

## Native Messaging Host

If using a native host:

- Create local socket directories with user-only permissions.
- Remove stale socket files for dead processes.
- Validate message shape.
- Enforce max message sizes.
- Forward tool responses only to connected runtime clients.
- Clean up sockets on shutdown.
- Log diagnostics without leaking private payloads.

The native host is a transport bridge, not the policy engine. Runtime policy
still decides whether a browser action is allowed.

## Desktop GUI Control

Desktop GUI control needs explicit gates:

- Platform support.
- Subscription or product entitlement when applicable.
- Feature flag or operator enablement.
- OS-level accessibility permission.
- OS-level screen-recording permission.
- Session-level user approval.
- Target app or display allowlist.

The desktop action executor should expose capabilities such as:

- List displays.
- Capture screenshot.
- List installed or running apps.
- Bring target app forward.
- Move mouse.
- Click.
- Drag.
- Scroll.
- Type text.
- Press keys.
- Read or write clipboard in a guarded way.

Platform-specific native modules should load lazily and fail clearly if
unavailable. Screenshot-only paths should not load input-control modules until
needed.

## Screen And Coordinate Handling

GUI action records must define coordinate semantics.

Rules:

- Freeze coordinate mode for a session so tool descriptions and executor
  transforms stay aligned.
- Record display id when known.
- Convert logical to physical pixels through platform display scale.
- Resize screenshots for model or API limits without losing original
  dimensions.
- Keep enough metadata to interpret coordinates later.
- Use per-action screenshots or state checks where needed for safety.

Click validation can be a feature-gated enhancement, but lack of validation
should be explicit in the action record.

## Clipboard And Images

Clipboard image ingestion should be a separate service.

Requirements:

- Fast native image read path where available.
- Platform fallback commands for image presence and image extraction.
- Supported image format detection.
- Conversion from unsupported clipboard formats into supported image formats.
- Downsample to model and API limits.
- Include original and display dimensions when known.
- Clean up temporary files.
- Parse image file paths from clipboard text cautiously.

Text paste automation should preserve the user's clipboard:

1. Read current clipboard.
2. Write intended text.
3. Read back to verify.
4. Paste.
5. Wait long enough for the target app to consume it.
6. Restore the previous clipboard in a `finally` path.

If verification fails, do not press paste.

## Desktop Handoff

A desktop handoff flow should:

- Check whether the desktop app is installed.
- Check minimum supported version.
- Prompt to download if missing or too old.
- Flush the terminal session transcript or event log.
- Open a deep link into the desktop app.
- Show success or error.
- Shut down the terminal session only after successful handoff.

The handoff should be treated as a run lifecycle event, not just a UI command.

## Effect Services

Recommended split:

- `BrowserOpenService`: validated URL and path opening.
- `BrowserBridgeService`: extension and native-host state.
- `BrowserActionService`: typed browser actions.
- `DesktopCapabilityService`: platform and permission probes.
- `DesktopActionService`: native GUI actions.
- `ScreenCaptureService`: screenshots, displays, dimensions, resizing.
- `ClipboardService`: image and text clipboard operations.
- `DesktopHandoffService`: app install check and session transfer.
- `GuiApprovalService`: approval prompts and durable grants.
- `GuiEvidenceService`: artifact creation and redaction.

All GUI services should use scoped resources and should be cancellable.

## Safety Rules

- No GUI control without explicit capability and approval.
- No browser automation outside browser-side site permissions.
- No desktop action if OS accessibility or screen permissions are missing.
- No persistent hidden GUI control after session end.
- Restore hidden windows and clipboard state where the runtime changed them.
- Do not expose screenshots publicly without redaction.
- Do not trust clipboard paths without normal workspace path checks.
- Use secure local socket permissions for native bridges.
- Make unsupported platforms visible rather than silently no-oping.

## Tests

Minimum coverage:

- URL scheme validation.
- Platform opener failure paths.
- Browser extension installed and missing states.
- Native host message validation and max message size.
- Socket permission and stale socket cleanup.
- Desktop permission probe states.
- Coordinate mode freeze.
- Screenshot resize and metadata preservation.
- Clipboard image native path and fallback path.
- Clipboard text restore after paste failure.
- Desktop handoff installed, missing, old-version, success, and error flows.
- Redaction of screenshots and browser logs in public projections.

## OpenAgents Translation Notes

Checked the open OpenAgents issue list on 2026-06-11.

Related live roadmap issues:

- #4771 covers provider peer connect flows.
- #4773 covers API parity.
- #4769 covers repo connect and per-mission data-scope UX.
- #4782 covers spare-capacity provider mode.
- #4783 covers lane fanout to the labor market when owned capacity is limited.
- #4786 is the Autopilot MVP ladder epic.

No open issue explicitly names browser control, desktop GUI control, screenshot
evidence, or desktop handoff. These should be treated as proposed capabilities,
not live OpenAgents behavior.

Recommended OpenAgents shape:

- Represent browser and desktop access as `CapabilityGrant` records.
- Store screenshots and GUI logs as private artifacts by default.
- Add GUI evidence to the same mission artifact system used for PR writeback.
- Keep product API parity: any GUI-derived state that affects a mission must
  have a non-browser API projection.
- Treat provider mode and labor-market fanout as separate from local GUI
  control, even if both eventually use desktop agents.

## Decision

Build browser and desktop integration after the core runtime, workspace, Git,
settings, and auth layers. It is high leverage for real-world tasks, but it
must sit behind explicit capabilities, approvals, and private evidence
defaults.
