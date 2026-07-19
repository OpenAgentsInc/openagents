# Unreleased

Entries accumulate here between releases. Appending an entry when your change
lands on `main` is part of the CLAIM-RELEASE protocol — see `README.md` in
this directory for the required format. `pnpm changelog roll` moves these
entries into the next dated release file.

## macOS updates recognize real RC bundles and preserve offline notarization (#8993)

- issues: #8993
- commits: this change
- contracts-specs: native macOS update applier; two-phase app and DMG notarization pipeline; packaged release acceptance
- invariants: channel-specific signed identity and offline app-ticket verification remain fail-closed
- evidence: docs/sol/receipts/2026-07-19-issue-8993-staging-update-rehearsal.md
- lane: codex-open-issue-sweep-20260719

The Desktop updater now recognizes the signed name and identity used by real
release-candidate apps instead of rejecting them as missing or mismatched.
Future macOS DMGs also capture an already-notarized, already-stapled app before
the image itself is notarized, preserving offline Gatekeeper protection rather
than weakening it to accept older malformed release bytes.

## Full Auto accepts installed non-default Codex models (#9003)

- issues: #9003
- commits: 7a01228b7d
- contracts-specs: installed Codex app-server catalog; durable Full Auto continuation profile
- invariants: provider identity and exact installed-catalog membership remain mandatory
- evidence: docs/sol/receipts/2026-07-19-issue-9003-codex-full-auto-model-admission.md
- lane: codex-open-issue-sweep-20260719

Full Auto no longer relies on the former two-model Codex allowlist. The live
installed Codex catalog now governs the composer, provider lane, and durable
continuations, with explicit regression coverage for GPT-5.6-Terra and
fail-closed coverage for models absent from that catalog.

## Sarah voice is attached to messages (#9013)

- issues: #9013
- commits: 2e4177fe64
- contracts-specs: Sarah Mobile Speech Delivery in INVARIANTS.md; Effect Native Card long-press projection
- invariants: no permanent speech bar; one active owner-private clip; only its exact message shows bounded state
- evidence: docs/mobile/2026-07-19-openagents-mobile-sarah-message-voice-ota-receipt.md
- lane: codex.root.sarah-message-voice

Sarah's permanent “Listen · AI-generated voice” bar is gone. Long-press any
completed Sarah response to prepare and play its AI-generated voice; the exact
message shows a compact preparing, playing, or failed state, and long-pressing
the active message stops it. Sarah's composer also restores native autocorrect.

## Desktop restart failures no longer repeat (#9012)

- issues: #9012
- commits: aaccf71781
- contracts-specs: Desktop Development Restart Authority in INVARIANTS.md
- invariants: restart coordination is one-shot and failure notices are claimed once per request
- evidence: apps/openagents-desktop/tests/oa-dev-supervisor.test.ts; apps/openagents-desktop/tests/electron-boundary.test.ts
- lane: codex.root.restart-notification-loop

A failed Desktop development restart now produces at most one notification and
stays stopped until a new restart is explicitly requested. The running app is
preserved when a handoff fails, so a port conflict cannot become a repeating
macOS notification loop.

## Sarah shows verified tool activity in chat

- issues: none (direct owner request)
- commits: c2ff92159c
- contracts-specs: openagents_mobile.sarah.live_tool_activity.v1; INVARIANTS.md Sarah tool-visibility invariant
- invariants: Sarah tool use and success/failure may no longer be hidden by conversational presentation
- evidence: docs/mobile/2026-07-19-openagents-mobile-sarah-live-tool-activity-ota-receipt.md
- lane: codex-owner-session-sarah-tool-visibility

Sarah now shows a short live activity line when she uses a real tool, and the
line updates when its confirmed result arrives. Internal tool names, IDs,
arguments, raw results, provider plumbing, and token dumps stay out of the
conversation.
