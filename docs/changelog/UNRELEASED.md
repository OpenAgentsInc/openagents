# Unreleased

Entries accumulate here between releases. Appending an entry when your change
lands on `main` is part of the CLAIM-RELEASE protocol — see `README.md` in
this directory for the required format. `pnpm changelog roll` moves these
entries into the next dated release file.

## Coder idle frame shows what is new in 0.1.1

- issues: none (direct owner request)
- commits: this change
- contracts-specs: Coder idle welcome chrome
- invariants: the startup facts box keeps one column of inner padding; the changelog card is at most five lines
- evidence: `crates/openagents-cli/tests/coder_frame.rs` startup-facts frame test
- lane: grok welcome-pad-rc10

The idle Coder screen pads the startup facts box by one column and draws a
matching "New in v0.1.1" card under it.

## Gym views render frozen schemas in the terminal and TUI (#165)

- issues: #165
- commits: this change
- contracts-specs: `openagents.gym.*` v1; `docs/coderbench/gym-cli-spec.md` §4
- invariants: unknown optional measurements print as `unknown`, never a fabricated zero
- evidence: `crates/openagents-cli/src/gym/views.rs`; `tests/fixtures/gym/*.plain.txt`; `tests/gym_views_test.rs`
- lane: grok session, issue #165

`openagents gym` suite/run/results/env/corpus/dataset commands now print from
one renderer over the frozen v1 documents. The coder `/gym` pane draws
`results_trend` and `run_status` from that same renderer. Missing costs and
rates stay `unknown`. Web corpus/dataset tabs remain a Phoenix
`openagents.com` follow-on.

## Delegated Grok streams stay in ATIF (#276)

- issues: #276
- commits: this change
- contracts-specs: ATIF-v1.7 extra.subagent; ACP session/prompt idle wait
- invariants: a child that opened a session and streamed is recorded as having run; timeout after work names the session and tool count
- evidence: `crates/openagents-cli/tests/acp_test.rs` idle-reset tests; export coalescing test
- lane: grok ATIF/timeout fix

A delegated Grok child that streams for minutes is no longer killed at a
900-second wall clock and reported as never started. `session/prompt` now
idles out only after silence; inbound `session/update` traffic resets the
wait. The parent `delegate` result counts ACP tool calls, keeps the session
id on timeout, and says the child ran then failed. ATIF export stores the
child transcript in `extra.subagent` keyed by the parent call id instead of
one notice per thought token.

## Coder keeps sign-in links visible

- issues: none (direct owner request)
- commits: this change
- contracts-specs: Coder device authorization and OSC 8 terminal links
- invariants: link repainting preserves the text and style of the completed frame
- evidence: Coder frame and OSC 8 hyperlink regression tests
- lane: codex/coder-visible-sign-in-url

Coder now repaints clickable links from the frame that the terminal displays.
Previously, Ratatui swapped to a cleared buffer before Coder read it, so the
hyperlink pass replaced the sign-in URL with blank, default-styled cells. The
sign-in URL now remains visible in amber and supports terminal link actions.

## CLI credentials use one cross-platform file (#261)

- issues: #261
- commits: this change
- contracts-specs: `openagents_cli.credentials.predictable_file.v1`
- invariants: credentials stay scoped by API origin, never appear in output, and use private permissions on Unix hosts
- evidence: OpenAgents CLI authentication, Computer, and concurrent private-file test suites
- lane: codex/issue-261-credential-file

The OpenAgents CLI now stores account and Computer tokens in
`~/.openagents/credentials.json` on every platform. It no longer invokes
macOS Keychain or Linux Secret Service commands. On Unix hosts, the CLI
restricts `~/.openagents` to mode `0700` and the credential file to mode
`0600`.

The CLI moves an existing `~/.config/openagents/cli-credentials.json` file to
the new path during the first credential read. Concurrent writers use atomic
staged writes and verify that each stored value reached the file before they
report success.

## Headless agents can complete CLI device authorization

- issues: none (direct owner request)
- commits: this change
- contracts-specs: `openagents.repositories.v1` device authorization
- invariants: a headless agent surfaces the approval URL and user code without receiving the user's GitHub token
- evidence: `packages/openagents-cli/test/cli.test.ts`; live staging device authorization start
- lane: codex/qualify-repository-import

`openagents auth login` now works with agents whose shell tools buffer output.
In a headless or noninteractive process, it returns the complete approval URL,
user code, and resume command immediately. After the user approves the request
in any browser, `openagents auth login --resume` completes authorization and
stores the issued OpenAgents token. Interactive use still opens the browser
automatically and prints a manual instruction if the browser does not open.

The credential adapter verifies the stored value before it reports success.
Pending device requests use a private local file and disappear after success
or detected expiry.

Repository imports also report state transitions and attempt counts to
standard error while the CLI waits. Machine-readable JSON remains isolated on
standard output.

## Repository imports ship in CLI 0.1.0

- issues: none (direct owner request)
- commits: 6fe64d6737, a80ab5914818
- contracts-specs: `openagents.repositories.v1`; packed npm consumer contract
- invariants: an import copies one accepted ref snapshot; credentials stay out of Git arguments and logs; npm installs one compatible Effect runtime
- evidence: npm `@openagentsinc/cli@0.1.0`; `packages/openagents-cli/scripts/verify-packed-install.mjs`; `packages/openagents-cli/test/repository-client.test.ts`; server provisioner regression at a80ab5914818
- lane: codex/qualify-repository-import

You can install the OpenAgents CLI from npm and use `openagents repo import`
to copy a GitHub repository into OpenAgents. The command reports bounded
server failures and makes clear that a timed-out client does not cancel an
import that the server already accepted.

The package gate installs the packed tarball in a clean npm project. It checks
the complete dependency tree, executable, version, and repository import help.
The npm registry now serves `@openagentsinc/cli@0.1.0` under the `latest` tag,
and production runs the qualified server revision on all three fleet nodes.

## Mobile source prepares managed Sarah voice (#9273)

- issues: #9273
- commits: this change
- contracts-specs: `openagents.sarah.voice.v1`, `mobile_voice_only`, native PCM audio module
- invariants: protected mobile identity only, no provider key in the app, foreground capture only, no mobile tools, normal credit
- evidence: docs/mobile/2026-07-28-openagents-mobile-build-127-sarah-voice-preflight-receipt.md
- lane: codex/9273-sarah-mobile-voice

The mobile source adds a Sarah voice screen with transcripts and clear mute,
interrupt, end, and retry controls. The app stops capture outside the
foreground and keeps provider credentials off the device.

The managed service and physical-device gates are still open. Build 127 is not
uploaded to TestFlight.

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
