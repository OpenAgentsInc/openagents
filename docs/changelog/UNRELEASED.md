# Unreleased

Entries accumulate here between releases. Appending an entry when your change
lands on `main` is part of the CLAIM-RELEASE protocol — see `README.md` in
this directory for the required format. `pnpm changelog roll` moves these
entries into the next dated release file.

## High-risk operator procedures complete their STE conversion (#9051)

- issues: #9051
- commits: this change
- contracts-specs: 15 current operator procedures, one superseded Worker deploy runbook
- invariants: human procedures use base STE, protected commands and requirement tokens remain stable
- evidence: docs/ste/p3-high-risk-procedure-conversion-receipt.md, docs/ste/control-semantic-baseline.v1.json
- lane: codex/asd-ste100-migration-20260719-r2

The current release, deployment, authentication, private workspace, and recovery controls now have checked human-facing STE profiles.
The obsolete Worker deploy runbook now points to the current Google Cloud authority.
The ledger keeps later-phase public, planning, evidence, and legacy documents in an explicit migration state.

## Active specifications complete their STE conversion (#9050)

- issues: #9050
- commits: this change
- contracts-specs: 16 active specifications, 12 specification authoring documents
- invariants: no ProductSpec intent change, exact subject digests remain bound, assurance revisions identify binding changes
- evidence: docs/ste/p2-specification-conversion-receipt.md, docs/ste/control-semantic-baseline.v1.json
- lane: codex/asd-ste100-migration-20260719-r2

All active specifications and their active authoring documents now have checked STE profiles.
The conversion keeps technical requirements, code, paths, URLs, identifiers, and protocol values.
Five AssuranceSpec documents now bind the converted ProductSpec bytes with new assurance revisions.

## Agent controls complete their STE review (#9049)

- issues: #9049
- commits: this change
- contracts-specs: 15 P1 control contracts, openagents-agent-compact-v1
- invariants: no requirement reduction, agent density exceptions cannot apply to human or dual-audience text
- evidence: docs/ste/p1-control-conversion-receipt.md, docs/ste/control-semantic-baseline.v1.json
- lane: codex/asd-ste100-migration-20260719-r2

All P1 control contracts now have checked STE profiles.
Agent-only controls can keep a dense sentence or paragraph after an identified review.
The exception helps agents parse one technical control context quickly.
It does not permit semicolons, contractions, unsafe terms, ambiguity, or a weaker requirement.

## Root control documents remove prose semicolons (#9049)

- issues: #9049
- commits: this change
- contracts-specs: AGENTS.md, INVARIANTS.md, docs/sol/MASTER_ROADMAP.md
- invariants: no requirement change, protected control tokens remain equal
- evidence: docs/ste/p1-control-conversion-receipt.md, scripts/check-ste.test.ts
- lane: codex/asd-ste100-migration-20260719-r2

Three central OpenAgents control documents now use approved punctuation instead of prose semicolons.
The change keeps code, URLs, technical words, and protected requirements without changes.

The documents remain in the STE migration state.
Sentence, paragraph, vocabulary, and inspection work is still necessary.

## Agent records have a controlled compact language profile (#9049)

- issues: #9049
- commits: this change
- contracts-specs: openagents-ste-policy-v2, openagents-agent-compact-v1, STE document profile schema
- invariants: human text uses base STE, agent extensions cannot relax authority, safety, evidence, or ambiguity controls
- evidence: docs/ste/agent-compact-profile.v1.md, docs/ste/agent-compact-terms.v1.json, scripts/check-ste.test.ts
- lane: codex/asd-ste100-migration-20260719-r2

OpenAgents now separates human technical text from compact agent records.
Human text continues to use the base STE profile.
Agent records can use controlled technical terms and labeled fields when these forms improve precision.

The checker limits the extension to an identified agent section or agent document.
The policy keeps all conditions, limits, proof states, and authority references.
The RC.25 dual changelog is the reference pattern, and its released bytes remain unchanged.

## Agents can review and safely apply exact code proposals (#9036)

- issues: #9036
- commits: 6883463cbe, fc29ee6df9, f8e423dc0d
- contracts-specs: openagents.desktop.ide-agent-code.v1; Desktop agent editing invariant; IDE roadmap and crosswalk
- invariants: agent context is inspectable and bounded; proposal apply is main-owned, generation-fenced, checkpointed, and independently evidenced
- evidence: apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-08-acceptance.json; apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-08-packaged-agent-code.json; docs/ide/2026-07-19-ide-08-agent-native-code-graph.md
- lane: codex.ide08.20260719.aL9yNF

Desktop agents can now attach to the exact project and worktree, show what
context they included or withheld, and present version-bound file changes for
review before anything touches disk. Owners can accept all or part of a
proposal, apply it through the same workspace authority as manual edits,
follow conversation-to-code links, inspect independent post-change evidence,
and undo to a retained checkpoint.

The implementation fails closed when files, generations, grants, or policies
move; keeps private roots and file bytes out of public receipts; and treats
tests, delivery, verification, and owner acceptance as separate observed facts
rather than trusting an agent's completion text. The packaged macOS journey
proved context disclosure, Pierre review, keyboard apply/undo, backlinks,
evidence separation, and exact disk restoration on the recorded candidate.

## Linux AppImage updates retain a safe rollback image (#8921)

- issues: #8921
- commits: 1658d36548
- contracts-specs: DesktopPlatformUpdateApplier; Linux AppImage retained-slot transaction
- invariants: signed ReleaseSet selection remains authoritative; DEB/RPM remain package-manager handoffs with no app-owned rollback claim
- evidence: apps/openagents-desktop/src/linux-update-applier.test.ts; native x64/arm64 receipt pending this RC
- lane: codex-open-issue-sweep-20260719-linux-distribution

Linux AppImage installs can now apply a verified full-image update without
elevation, restart through one stable selected-image path, retain the previous
image until a healthy launch, and roll back if that launch fails. Foreign or
malformed AppImages fail before the current selection changes. DEB and RPM
downloads continue to use the distribution package manager and do not claim
application-owned rollback.

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
