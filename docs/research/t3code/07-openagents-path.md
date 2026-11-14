# What to adapt for OpenAgents and Coax Native

This chapter is a recommendation derived from the audit. It does not claim
that the following architecture is already wired into the OpenAgents phone app,
nor authorize/implement a migration. The only changes in this research task are
these documents.

## Carry the execution boundary across, not the TypeScript stack

The most useful T3 pattern is a durable environment service that owns coding
agents beside their workspaces, with one application protocol for mobile, web
and desktop. Coax Native should consume that protocol and present the shared
chat components. It does not need Electron, React, Effect or a WebView to adopt
that boundary. The earlier [Exact research](../exact/README.md) addresses the
cross-platform app/host boundary; this audit adds the execution/control-plane
boundary behind an agent UI.

A Coax implementation should keep product decisions and protocol state machines
in Coax, with host effects limited to processes, sockets/TLS, filesystem,
Keychain and platform UI. This fits the repository's existing rules in
[AGENTS.md](../../../AGENTS.md), [reuse guide](../../reuse.md) and
[Coax Native proposal](../../../coax/native.md).

## Existing Coax work comes first

The OpenAgents tree already contains provider protocols, agent engines and a
shared component system. Starting a new “T3-like SDK” without examining them
would duplicate the hard parts and violate the repository's reuse rule.
The inventory below is checked against the audit worktree's base
`6fe29cc703`; it is an implementation starting point, not a new parity claim.

| Need | Existing source to extend/qualify |
| --- | --- |
| Claude CLI stream-json/control protocol | [`coax/lib/agent/claude/`](../../../coax/lib/agent/claude/PARITY.md), query/control/messages/transport modules |
| Codex app-server protocol and process host | [`coax/lib/agent/codex/`](../../../coax/lib/agent/codex/PARITY.md), client/transport/fixtures |
| Engine/session/transcript and approval integration | [`coax/lib/agent/engine/engine.coax`](../../../coax/lib/agent/engine/engine.coax), Claude/Codex/Cursor engine modules and `coax/lib/agent/session/` |
| Existing hosted runner wire and account-chat shapes | [`proto/registry.json`](../../../proto/registry.json), `proto/coder/agent-managed-v1alpha1.json`, `account-chat-v1.json`, `chat-history-v1.json`, `coax/web/agent/runner-wire.coax` |
| Shared Markdown, edit buffer, transcript/tool components | [`components/spec.md`](../../../components/spec.md), `components/core/markdown.coax`, `components/zeron/` |
| Native account/services and optional signed OTA | [`coax/native/README.md`](../../../coax/native/README.md), `apps/shared/native/`, `apps/openagents-ios/` |

The current native starter README describes UIKit controls plus compiler-generated
Coax in JavaScriptCore, with no WebView/DOM; the reuse inventory still contains
historical WebKit wording. Use concrete installed-profile evidence when making
runtime claims. A shared protocol can span those delivery profiles without
putting provider execution on the phone.

The existing parity documents also identify real gaps:

- The Coax Claude library ports the **CLI wire**, not the npm SDK. Its declared
  baseline is SDK 0.3.296 / Claude Code 2.1.312, compared with T3's locked SDK
  0.3.321. SDK callback examples need translation into Coax's pull/answer state
  machine, not direct copying.
- The Coax Codex client tracks CLI 0.163.0 and records specific request/reply
  behavior. Its documented gaps include steering, native subagent handling,
  input questions/MCP elicitation and catalog paging. The lower-level existence
  of a generic request method is not finished product support for those cases.
- Both libraries have fail-closed immediate-response behavior for some unanswered
  host requests. A remote approval UI can take seconds or minutes; it requires
  a deliberate pending-request owner and timeout/cancellation policy. Simply
  delaying the next read or returning the default decline is not remote UI parity.

Sources: [Claude parity](../../../coax/lib/agent/claude/PARITY.md),
[Codex parity](../../../coax/lib/agent/codex/PARITY.md). These documents' recorded
smoke results are historical evidence, not checks rerun during this audit.

## Proposed shared model

Preserve the distinctions T3 needed: environment, provider instance, native
conversation binding, application conversation, run/attempt, streamed item and
human request. Reuse our glossary/protocol vocabulary instead of importing
T3 names as a second incompatible registry. New/changed published contracts
must begin in `proto/` and the protocol specification with a versioned identity,
registry entry and one codec.

Capability metadata should say what is actually usable: native resume/fork/
rollback, active steering versus interrupt/restart, tool output streaming,
background work, native subagent identity, and approval/question response
lifetime. Unsupported operations need a clear refusal. Do not convert every
provider-specific feature into a client-side provider-name switch.

The application transcript should retain structured tools/plans/questions and
native references; Markdown is one content type within it. This lets the shared
components render the same state on UIKit, web, GPU and terminal while the host
does only platform effects. One text field containing concatenated agent output
cannot preserve tool/request ownership or a reliable Stop button.

## Proposed implementation sequence

1. **Inventory the existing end-to-end path.** Trace a current native account
   chat through its API/runner/protocol entries to the existing agent engines.
   Separate account-saved hosted chats from threads on a paired local execution
   environment. Record which runtime each conversation uses and which machine
   owns its files/credentials. This audit does not assume T3's environment
   catalog is already the OpenAgents account-chat model.
2. **Specify durable command and event semantics.** Extend the existing protocol
   for stable command identity/receipts, run state, structured items, replay
   cursors/snapshots and request response capabilities. Decide the bounded event
   retention policy explicitly. Keep projections, native transcripts and client
   caches separate. Policy/admission files need `#authority` and mutation gates.
3. **Ship one real provider vertical slice on the execution host.** Reuse our
   Codex or Claude protocol module; cover start, streamed text, one tool,
   one approval, interrupt, reconnect and native resume before widening the
   provider matrix. Qualify the exact installed binary/protocol combination.
4. **Make mobile delivery durable.** Use one connection owner per environment;
   distinguish transport readiness from data freshness. Stable message/command
   identity and an operation-owned outbox should survive lost acknowledgments.
   A resumed phone reads committed server state instead of guessing whether its
   last send ran. Persist state/cursor together.
5. **Add human requests and background ownership.** Keep live native callback
   state on the execution host, bind answers to the owning instance/thread/run,
   reject stale responses and represent async message-answer questions
   separately. Add child/background lifetimes before claiming subagent parity.
6. **Add provider switching and conversation operations.** Native continuation
   requires compatible home/account/native history. Portable handoff must state
   its actual truncation/coverage; it is not native context parity. Revert must
   preflight native conversation support before changing files.
7. **Widen routes/providers after the first slice is qualified.** LAN/HTTPS or
   a single hosted execution service can precede SSH, discovery and relay
   brokerage. Each route preserves environment auth and execution ownership.
   Implement native runtime modes only when the provider can enforce them.

This is sequencing for a later implementation, not a request to install T3 or
introduce its dependency graph. An app-side update can add protocol consumers
within existing host capabilities; server/provider changes and newly required
native host services have their own release requirements. Signed OTA does not
make a missing native bridge or provider capability appear.

## Qualification cases worth borrowing

Adopt the intent of T3's replay-through-real-orchestration tests using Coax
checks, deterministic fixtures and existing process/network host boundaries.
Do not replace the policy under test with a fake successful adapter.

The first qualification set should include lost send acknowledgment with a
repeated command, phone suspension during a turn, server/process loss during
approval, a declined write that makes no file, interleaved threads on a shared
runtime, async input after turn completion, background child completion after
the root turn, expired replay cursor and unsupported revert. Check source
versions and preserve scrubbed native transcripts as protocol evidence.

Keep T3's specific tradeoffs explicit when deciding what to adapt: its compact
handoffs omit context; its event log is not the full source of truth; scopes
that allow arbitrary execution confer broad OS-user authority; and native
agent behavior can change outside an app release. These constraints are as
useful as the reusable components.
