# Unreleased

Entries accumulate here between releases. Appending an entry when your change
lands on `main` is part of the CLAIM-RELEASE protocol — see `README.md` in
this directory for the required format. `pnpm changelog roll` moves these
entries into the next dated release file.

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
