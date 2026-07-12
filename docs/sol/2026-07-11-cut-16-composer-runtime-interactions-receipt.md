# CUT-16 composer and runtime-interaction foundation receipt

- Date: 2026-07-11
- Issue: [#8696](https://github.com/OpenAgentsInc/openagents/issues/8696)
- Status: shared authority, native persistence, both native interaction UIs,
  mobile runtime-control and canonical draft UI, Desktop gateway, and Claude
  provider injection active; mobile native attachment acquisition active;
  selector authority and live/physical acceptance remain open
- Implementations: `a58af4dbfb`, `7b1b9bb066`, `cd5c0dd737`, `1768e8bb35`,
  `11a8d2481a`, `06122c04ed`, `1875b06cac`, `9cd14cef1b`, `2f302d8e1a`,
  `43c5bf6df7`, `c7cf2bf758`, `05ce0e1044`, `b72bf6acbb`, `835c689c4a`,
  `97f90832bb`, `21d56199bd`, `88f692fe00`, `400c649904`, `600228f230`, and
  `2fae80b1ec`, `9ca4b21828`, `3b42dbddf9`, `4a9db8347b`, `23a190905f`,
  `d2d9ee8907`, `e4d903c602`, `4b20fe2b67`, `9fa76c3b09`, and
  `94deab6705`, and `c3ad8bee34`

CUT-16 now builds on the existing rich `@openagentsinc/composer-state` kernel
instead of creating a second composer. The additive private coding-draft
envelope binds its structured text and file/image/snippet attachments to stable
draft/session/thread identity, ref-only repository/worktree/editor/diff
context, explicit provider/model/account/execution-target readiness, and an
editing/queued/accepted/failed/canceled submission lifecycle. Queueing refuses
stale context, unfinished attachments, and unavailable/revoked/offline targets.
Exact duplicate sends reconcile; a failed/canceled retry preserves the same
submission/intent/idempotency identity. Its bounded receipt contains counts,
context kinds, and readiness—not prompt, attachment, account, path, or diff
content.

Shared Sync clients now expose deterministic continue/retry/close controls over
the already-landed server authority. Existing-turn controls are fenced to the
durable provider lane before any insert or state transition.

Mobile now consumes all four existing-turn controls through one typed
`cancel | resume | retry | close` action boundary. The host re-reads the exact
confirmed thread/run, derives only a known Codex/Claude/hosted lane, refuses an
unknown lane or invalid state transition, and queues the corresponding shared
intent. UI state remains submitting until both the exact command outcome and a
newer matching run projection are confirmed. Running/queued/waiting turns show
Cancel; canceled turns show Resume, Retry, and Close; completed/failed turns
show Retry and Close. All are shared Effect Native `Button` controls with
disabled reconciliation semantics. The legacy mobile interrupt method now
delegates to the same boundary rather than hard-coding the Codex lane.

`openagents.runtime_interaction.v1` is the provider-neutral private authority
for questions, tool approvals, and plan reviews. It carries exact
interaction/thread/turn identity, requested sequence, deadline, bounded
display-safe choices, and pending/resolved/expired/revoked lifecycle. Kind-
matched decisions have stable decision/idempotency refs: exact retry is a
duplicate, conflicting reuse rejects, and late/revoked decisions cannot
resolve.

Migration 0062 and `runtime.requestInteraction` / `runtime.decideInteraction`
store and mutate only the exact private thread post-image. Admission verifies
owner, thread, turn, durable lane, next sequence, state, and server-future
deadline. The confirmed client hides cached interactions outside live thread
authority and merges grouped question/approval/plan facts into the canonical
thread timeline without introducing a new renderer discriminant.

Both native hosts now expose the same live-only interaction client and a
signed-out-capable device-local draft store. Draft persistence is bounded to
128 canonical snapshots of at most 1 MiB each, rejects stale/conflicting/
foreign writes, withholds malformed rows, and survives SQLite restart without
entering hosted Sync. Desktop Runtime Gateway protocol v9 carries bounded
exact-thread reads and confirmed-only decisions through the production host
adapter. Mobile consumes that same authority with typed grouped selection,
tool approve/deny, plan accept/request-changes/replan, disabled reconciliation,
and read-only resolved/expired/revoked states.

Mobile selected coding sessions now open the canonical device-local draft by
exact session/thread identity. New drafts bind the local owner ref, stable
session/thread refs, ref-only repository/worktree context revisions, known
provider/execution-target refs, and the exact confirmed Codex/Claude/hosted
lane; absence of that lane is explicit `unavailable`, never a default guess.
Restored text and file/image metadata lower through the shared Effect Native
Composer, while a compact context line names the repository/worktree and
truthfully marks unselected model/account facts. Text edits and accepted clears
advance the canonical local revision; a failed send restores the prior draft.
A real SQLite close/reopen returns the same draft ref, body, and context.
Unavailable-target drafts remain editable but omit submit authority; both RN
and SwiftUI lowerings disable and announce Send unavailable when `onSubmit` is
absent.

Mobile native acquisition now uses the SDK 57 `expo-file-system` multi-file
picker. It refuses more than eight selections or any file above 25 MiB before
reading, checks the byte bound again after reading, computes SHA-256, and copies
the content into the app document sandbox under that digest. The picker URI and
platform file handle remain inside the native adapter closure. The canonical
draft receives only manual attachment metadata plus a ready
`attachment.native-local.sha256.*` ref through the shared stage/apply/ready
transactions. Duplicate content is coalesced, cancellation is inert, failures
are public-safe, and the Effect Native plus control is disabled while the
picker reconciles. The UI says only that the attachment is stored on this
device; it does not claim hosted upload or runtime delivery.

Desktop now consumes the confirmed projection as Effect Native question,
tool-approval, and plan-review cards. It preserves canonical thread, turn,
interaction, question, and option identities; refuses missing or ambiguous
display-to-ref mappings; and sends decisions through the protocol-v9 gateway.
The renderer waits for a gateway event and re-reads the exact confirmed
post-image. Neither an enqueue receipt nor a different decision ref can render
the interaction resolved, and expiry/revocation remain distinct terminal
states. The frozen Fable-local question IPC keeps its original outbound shape.

The standing owner-local runtime-intent supervisor now constructs a trusted
HTTP authority from its existing internal Worker credential and fixed owner.
POST executes the real `runtime.requestInteraction` mutator; GET returns only
the exact owner/ref post-image. Claude dispatch injects `canUseTool` only when
that authority is explicitly present, uses the exact current durable event
sequence, and switches that supervised invocation to SDK `default` mode so
the callback genuinely runs. Without the authority, the pre-existing
permission path is unchanged. Only a confirmed matching approval returns raw
tool input to the same SDK call. A separate `runtime.expireInteraction`
mutator uses the database clock to persist deadline expiry without inventing a
deny decision.

Verification:

- composer-state: 23 pass, 0 fail, 163 assertions; shared composer UI: 7 pass,
  0 fail, 69 assertions;
- agent-runtime-schema: 40 pass, 0 fail, 275 assertions;
- Khala Sync schema: 191 pass, 0 fail, 2,705 assertions;
- khala-sync-server after migration/admission: 519 pass, 0 fail, 4,590
  assertions; local Postgres exercises request/decision/retry/conflict/expiry/
  owner/lane/sequence boundaries;
- khala-sync-client: 187 pass, 0 fail, 12,780 assertions (three env-gated live
  smokes skipped);
- Desktop Runtime Gateway focused: 21 pass, 0 fail, 81 assertions; production
  composition/host focused: 51 pass, 0 fail, 445 assertions; Desktop typecheck
  and build pass; after authoritative Desktop interaction controls, the full
  Desktop suite is 468 pass, 0 fail, 2,487 assertions;
- mobile full suite after native attachment acquisition: 91 pass, 0 fail, 452
  assertions; focused composer/picker/Home plus composer-state contracts: 35
  pass, 0 fail, 236 assertions; mobile typecheck passes;
- Effect Native RN renderer: 9 pass, 0 fail, 26 assertions and project
  typecheck (existing Effect advisories only). The optional-submit regression
  proves the input remains editable while Send is disabled and announced
  unavailable;
- built iOS simulator after native attachment acquisition: Expo/Xcode built and
  signed the Debug iPhone-simulator app with 0 errors and 0 warnings, including
  the `ExpoFileSystem` native pod, installed and launched it on the booted
  iPhone 17 Pro simulator, Metro bundled 1,328 modules, and the Khala/Effect
  Native composer rendered without a red screen. The simulator had no
  authenticated coding session, so this is a native-link/render smoke rather
  than a claimed picker-tap receipt. The physical iPhone still reported
  offline, so this is not the issue's physical-device receipt.
- Pylon typecheck and full suite pass; focused HTTP-authority/runtime-dispatch
  coverage is 59 pass, 0 fail, 208 assertions. The API Worker typecheck and
  focused authority/route-manifest suite pass (6 tests). The runtime mutator
  local-Postgres suite is 15 pass, 0 fail, 112 assertions, including early
  expiry refusal and durable post-deadline expiry.
- deterministic native accessibility acceptance: mobile interaction controls
  remain on the shared Effect Native `Button` primitive; React Native lowers
  it to a screen-reader button with disabled state, Desktop lowers it to a
  native keyboard button, the mobile host remains keyboard-avoiding with
  blur-and-submit, and Desktop globally honors reduced-motion preference.
  Focused mobile Home/component coverage is 14 pass, 0 fail, 122 assertions;
  Desktop shell coverage is 66 pass, 0 fail, 390 assertions. Terminal
  resolved/expired/revoked cards expose no action controls on either host.

The deterministic CUT-16 edge matrix is therefore covered: offline authority
fails closed, restart-safe draft stores reconstruct bounded snapshots,
duplicate decisions are idempotent, revoked/expired authority is terminal,
mobile keyboard avoidance remains active, and the shared semantic controls
carry keyboard/screen-reader/reduced-motion behavior. These tests do not
substitute for the issue's required physical assistive-technology receipt.

Named-provider live evidence:

- Claude: a real retained `claude_agent` daily-driver turn for #8696 completed
  through Pylon's composer path with 121 events, 37 turns, 1,081 exact tokens,
  zero edits, a passed dev check, and a clean redaction scan. The refs-only
  artifact is
  [`2026-07-11-claude-agent-daily-driver-proof.json`](../../apps/pylon/docs/proofs/2026-07-11-claude-agent-daily-driver-proof.json).
- Codex: the first real attempt exposed the pinned `0.139.0` SDK as too old
  for `gpt-5.6-sol`; `e4d903c602` upgraded all owned consumers to `0.144.1`.
  After device authorization/quota recovery, a real retained Codex composer
  turn completed on 2026-07-12 through `composer.run_stream`: 34 events,
  7 commands, zero edits, 443,773 exact reported tokens, passed focused
  verification, and a clean house redaction scan. The refs-only artifact is
  [`2026-07-12-codex-daily-driver-proof.json`](../../apps/pylon/docs/proofs/2026-07-12-codex-daily-driver-proof.json).
- Deployment: the original Cloudflare staging D1 attempt failed safely with
  storage-limit code 7500. The authority subsequently moved with the monolith
  to Cloud Run/Cloud SQL. Production revision
  `openagents-monolith-00085-k4v` serves 100% of traffic; the direct migration
  runner applied pending migrations `0061_runtime_control_intent_expiry.sql`
  and `0062_runtime_interactions.sql` to `khala_sync_prod` under the dedicated
  migration role. An unauthenticated request returned the required 401, then
  an authenticated exact-ref null read returned HTTP 200 with
  `route.internal.khala_sync.runtime_interaction.v1`, `ok: true`, and
  `interaction: null`. The deployed trusted-authority gate is complete.

CUT-16 remains open. The completion audit still finds literal composer gaps:
mobile native file/image acquisition and byte-bearing runtime transport are
landed and production-proven; real provider/model/account selection remains.
The remaining external receipt is physical-device/assistive-technology acceptance.
Restart, revocation, provider injection, expiry, mobile runtime controls, and
mobile canonical draft restoration are covered deterministically; those proofs
and the simulator launch do not substitute for the remaining receipts.
The default non-interactive provider safety policy must not be weakened merely
to manufacture an approval receipt.

## Addendum (2026-07-12): Desktop durable runtime controls — Stop, queue-until-idle, lane-exact control intents

The Desktop chat surface previously exposed Stop and queue-until-idle only on
the local Fable/Codex IPC lanes: the authoritative runtime conversation host
omitted `interruptActive` and `queueFollowup`, so while signed in both
affordances silently no-oped on durable turns. Separately, the Desktop control
wire hard-coded `target.lane = codex_app_server` for interrupt/continue/
retry/close control intents in Electron main, which the durable lane fence
(`runtime_target_lane_mismatch`, landed `11a8d2481a`) rejects for Claude and
hosted turns — Desktop fleet-cockpit controls on a Claude turn were
unadmittable.

Landed in this slice:

- additive optional `lane` (`codex_app_server | claude_pylon | hosted_khala`)
  on the protocol-v10 gateway `conversation.interrupt/continue/retry/close`
  commands (same additive-optional precedent as `conversation.start.lane`; no
  protocol version bump), threaded by main into the shared control-intent
  builders instead of the hard-coded Codex default;
- the runtime conversation host now implements `interruptActive`: Stop acts
  only on the exact durable thread/run this renderer has in flight, derives
  the lane from the confirmed run runtime (falling back to the dispatched
  harness lane), sends the confirmed run `expectedVersion`, and treats the
  acknowledgement as admission truth only — the confirmed canceled terminal
  finalizes the turn and reverts the composer;
- the runtime conversation host now implements `queueFollowup`: a mid-turn
  submit enqueues a follow-up that is promoted only at the previous turn's
  confirmed terminal, as a real `conversation.append` plus
  `conversation.start` on the same lane; a failed promotion reports honestly
  instead of dropping text;
- the shell restores the cleared draft when an enqueue reports `queued:false`;
- fleet-cockpit run controls carry the card's exact confirmed provider lane.

Contract: new enforced
`openagents_desktop.chat.durable_runtime_turn_controls.v1` in the Desktop UX
registry (version `2026-07-12.3`); the existing composer-stop-button,
OpenCode-composer-shape, and queue-chip oracles are unchanged and still pass.

Verification: Desktop typecheck pass; focused runtime-conversation controls
17 pass / 0 fail / 60 assertions; shell 82 pass / 476 assertions; gateway
e2e (lane pass-through + decode + main source oracle) 22 pass / 91
assertions; fleet workspace suites pass with the strengthened lane-bearing
control expectation; full Desktop suite 992 pass / 3 env skips / 0 fail /
5,309 assertions; built Electron smoke green (see the landing commit).

Still open after this historical slice: later addenda supersede its composer
residuals. The current gates are real provider/model/account selectors, the
production image receipt, and physical assistive-technology acceptance.

## Addendum (2026-07-12): grant-scoped Desktop file mentions close I4

The Desktop editor can now attach its active ready document to chat without
granting the renderer ambient filesystem authority. The chip carries only the
workspace-relative path ref, document revision, language mode, bounded current
draft (including an explicit dirty marker), and a remove action. Submission
lowers that payload through the shared ChatHost boundary for both local
Fable/Codex and durable hosted conversations. The provider boundary labels it
untrusted data, instructs the model not to treat file contents as instructions,
and uses explicit begin/end delimiters. Skill parsing still examines only the
raw user message, so attached content cannot invoke a skill.

Capability I4 therefore moves from `missing` to `ui_available`; its UI and
programmatic oracles cover attach, visible/remove state, local/durable delivery,
clear-after-send, and absence of the action when the shell has not supplied the
typed intent. Verification: focused 129 pass / 1 existing H5 skip / 0 fail;
full Desktop 1,021 pass / 1 H5 skip / 0 fail / 5,503 assertions; typecheck,
build, built-Electron smoke, and lifecycle teardown (`active: 0`) pass.

This is not a mobile picker or physical-device receipt. Later addenda deliver
the mobile attachment transport; selector and physical acceptance remain.

## Addendum (2026-07-12): mobile attachment submission truth boundary

Mobile no longer treats an attachment-bearing draft as a successful text-only
send. At submission, the native host re-opens each digest-addressed sandbox
file, verifies its exact byte count and SHA-256 against the canonical draft,
and, for bounded UTF-8 text, lowers the actual bytes into the same authoritative
`chat.appendMessage` body referenced by the runtime intent. Provider-visible
begin/end markers identify the payload as untrusted data; neither native URI
nor sandbox path crosses the boundary. File-only turns use an explicit review
request.

Unsupported image/binary payloads, changed or unreadable files, invalid UTF-8,
and content that would exceed the 20,000-byte chat authority fail before any
Sync mutation. The exact draft remains visible and durable with a bounded
reason. After confirmed acceptance the complete submitted document is cleared,
including attachment refs; a failed send preserves it. Deterministic coverage
proves byte/digest verification, untrusted lowering, binary fail-closed
behavior, and whole-document accepted clearing. Full mobile verification is
112 pass / 0 fail / 599 assertions with typecheck green.

## Addendum (2026-07-12): durable mobile image transport

The authoritative message path now carries actual image bytes rather than
metadata or a prompt-embedded approximation. `chat_message` has one additive,
legacy-optional owner-private attachment array. Admission permits at most four
PNG/JPEG/GIF/WebP images and 2 MiB decoded per image; the server decodes base64
and verifies exact length, file signature, and SHA-256 before writing either
the business row or thread-scope changelog. No native URI/path enters Sync and
no public or view receipt carries base64.

Mobile re-reads and verifies each digest-addressed sandbox file immediately
before the authoritative append. The runtime intent references that exact
confirmed message. The trusted Pylon reader decodes the canonical schema;
Codex receives private OS-temp turn-scoped local-image files (removed in
`finally`),
Claude receives documented streaming user-message base64 image blocks, and
hosted Khala receives OpenAI-compatible data URLs in Pylon or Gemini
`inlineData` in the production monolith. Images-only turns receive a non-empty
explicit review instruction. Binary non-images still fail closed and preserve
the complete draft.

Verification before production deployment: Khala Sync 192 pass / 3 gated
live skips / 0 fail / 12,803 assertions; Khala Sync Server full suite and
typecheck green, including real-Postgres storage/integrity/runtime-read tests;
Pylon focused image/app-server/enforcement suites 80 pass / 0 fail / 305
assertions with typecheck; mobile 112 pass / 0 fail / 600 assertions with
typecheck.

Production is live-proven. Migration
`0063_chat_message_image_attachments.sql` is recorded in `khala_sync_prod`;
Cloud Run revision `openagents-monolith-00087-5gn` serves 100% of traffic. The
first deployed probe was a useful counterexample: Sync read the generated PNG
back with the exact SHA-256, but hosted Gemini answered that no image was
attached. That located a second production-only dispatcher outside Pylon.
`7e0b21302f` then carried the authoritative attachment through that dispatcher
and lowered it to Gemini `inlineData`. Repeating the same test against
production produced `red` for an otherwise unlabeled solid-red PNG, with
exact SHA-256 readback and the durable event sequence `turn.started` →
`text.delta` → `text.completed` → `usage.recorded` → `turn.finished(stop)`.
The opt-in `HOSTED_CHAT_SMOKE_IMAGE_PATH` path in
`hosted-chat-e2e-smoke.ts` makes this receipt repeatable without printing
credentials or image bytes.

## Addendum (2026-07-12): exact mobile execution-target selector

[CUT-16A #8717](https://github.com/OpenAgentsInc/openagents/issues/8717)
ports the already-live personal model-preference authority into the active
Effect Native mobile app. Strict catalog decoding produces real Khala, Codex,
and Claude choices with public-safe readiness. The canonical draft persists
the selected provider, pinned model, opaque account, lane, and exact execution
target; removed/revoked targets retain the draft but withhold Send. New turns
carry the exact `executionTargetId`, while active-turn steering stays pinned to
the confirmed lane. The retained sub-issue receipt is
[`2026-07-12-cut-16a-mobile-execution-target-selector-receipt.md`](./2026-07-12-cut-16a-mobile-execution-target-selector-receipt.md).

CUT-16 now remains open for physical cross-client/assistive-technology
acceptance.

## Addendum (2026-07-12): Android TalkBack structural-focus repair

An Android 15/API 35 Release-host TalkBack pass found a real renderer defect:
the labelled application-root Stack became one full-screen accessibility focus
target, so swipe navigation could not reach Open navigation, the composer, or
Send. Effect Native React Native stacks are layout containers, not controls;
the renderer now lowers every Stack with `accessible=false` and
`importantForAccessibility=no` while retaining independently accessible child
controls. A focused renderer oracle proves a labelled root remains outside the
focus order and its button child remains discoverable.

After rebundling and clean reinstalling the Release APK, TalkBack reported
touch exploration active with its spoken/haptic service bound. The initial
full-screen green focus frame disappeared; the next swipe placed the green
focus frame tightly around the Open navigation button, and TalkBack attempted
the bounded utterance `Open navigation. Button`. This supplies real Android-
emulator assistive evidence and fixes the discovered trap. It does not replace
the remaining physical-iOS VoiceOver and authenticated cross-client acceptance
required to close CUT-16.
