# CUT-16 composer and runtime-interaction foundation receipt

- Date: 2026-07-11
- Issue: [#8696](https://github.com/OpenAgentsInc/openagents/issues/8696)
- Status: shared authority, native persistence, both native interaction UIs,
  mobile runtime-control and canonical draft UI, Desktop gateway, and Claude
  provider injection active; mobile native attachment acquisition active;
  attachment delivery/selectors and live acceptance remain open
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
  for `gpt-5.6-sol`; `e4d903c602` upgrades all owned consumers to `0.144.1`
  and its focused executor/composer suites pass. Both ready local subscription
  accounts then reached the provider but were quota-limited until the
  provider-reported 20:31 CDT reset; no API-key fallback is configured. A
  completed named Codex receipt remains required after reset.
- Deployment: the safe pipeline passed local architecture/type/test gates but
  Cloudflare refused staging D1 migration with account storage-limit code
  7500. The new authority route therefore remains undeployed; no staging gate
  was bypassed and no remote data was deleted. Deployed-authority evidence
  remains required after the quota is raised or storage is remediated.

CUT-16 remains open. The completion audit still finds literal composer gaps:
mobile native file/image acquisition is landed, but attachment-bearing runtime
submission/delivery is not yet proven; real provider/model/account selection
remains, Desktop still needs full canonical rich-draft UI adoption, and editor/
diff capture is not yet wired end to end in both hosts. The
remaining external receipts are a named Codex turn after provider quota reset,
deployment of the trusted authority route after the Cloudflare staging storage
limit is remediated, and physical-device/assistive-technology acceptance.
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

Still open after this slice (unchanged residuals): Desktop full canonical
rich-draft UI adoption, real provider/model/account selectors, editor/diff
capture, attachment-bearing runtime delivery, chat-surface resume/retry/close
parity (the fleet cockpit carries those controls today), named Codex receipt,
deployed trusted authority, and physical assistive-technology acceptance.
