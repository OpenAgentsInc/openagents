import {
  BehaviorContractSchemaVersion,
  type BehaviorContractRegistryDocument,
} from "@openagentsinc/behavior-contracts";

export const openAgentsDesktopUxContractRegistry: BehaviorContractRegistryDocument =
  {
    schemaVersion: BehaviorContractSchemaVersion,
    version: "2026-07-11.25",
    contracts: [
      {
        contractId: "openagents_desktop.chat.no_assistant_role_label.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "chat transcript chrome",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement: "Remove where it says assistant. I don't care about that.",
        authorityBoundary:
          "Only the visible ASSISTANT role header is removed from assistant transcript rows. Timestamps stay; the user YOU label and system SYSTEM label stay; the effective-model caption stays as its existing compact system trace line; typed role data on the message contract is unchanged and grants no new rendering authority.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/shell.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "chat_no_assistant_label.note_projection",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Proves noteMessage emits no senderLabel for assistant rows while keeping the timestamp, YOU on user rows, and SYSTEM on system rows.",
          },
          {
            id: "chat_no_assistant_label.smoke",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron fable streaming smoke asserts the finalized assistant row renders no sender chip.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the shell view suite and the Electron smoke journey asserting the label-free assistant row.",
      },
      {
        contractId: "openagents_desktop.chat.message_metadata_inspector.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "chat message metadata inspector",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement: "if I click on the message, I see the metadata of the message in the right sidebar",
        authorityBoundary:
          "The inspector projects only the bounded per-message metadata the host persisted on the local thread store (role, timestamp, lane, SDK-reported effective model, account ref, turn ref, exact token total, duration) — never prompts, paths, tokens, credentials, or provider payloads. Selection is a typed intent (click dispatches; Escape and Close deselect) and grants no runtime, resume, or filesystem authority.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/chat-contract.ts",
          "apps/openagents-desktop/src/main.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "chat_message_inspector.intent_loop_and_fields",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Drives the real intent registry: details click selects the message, the right-side rail renders role/time/lane/model/account/turn/tokens/duration, Close and Escape deselect, and stale selections drop on thread switches.",
          },
          {
            id: "chat_message_inspector.smoke",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke clicks the streamed assistant message's details affordance and asserts the inspector shows the fixture's effective model, lane, account ref, and exact token total, then closes it.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the shell inspector suite and the Electron smoke message-metadata-inspector step.",
      },
      {
        contractId: "openagents_desktop.chat.no_composer_disabled_caption.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "composer harness lane affordances",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "I have no idea why the bottom says Codex requires Open Agent session. Don't put that shit in the UI ever. Remove that.",
        authorityBoundary:
          "Removing the caption never enables a dead lane: an unavailable chip stays visually disabled and refuses the action, and the reason string survives only in the chip's accessible label and host logs/journal. This does not weaken the evidence-gated composer or no-silent-substitution contracts.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/live-proof.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "chat_no_disabled_caption.composer_render",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Proves no caption node and no reason-bearing Text renders anywhere in the composer for any lane state, while the disabled chip keeps the reason as its accessible label and Send stays evidence-gated.",
          },
          {
            id: "chat_no_disabled_caption.smoke",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke asserts the disabled Codex chip carries its reason via aria-label only and that no standing caption text exists in the composer.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the composer suite and the Electron smoke; the live-proof driver journals the chip's disabled state + aria-label instead of any visible caption.",
      },
      {
        contractId: "openagents_desktop.chat.markdown_rendering.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "assistant message markdown rendering",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "The markdown isn't rendered as markdown, so fix our fucking markdown rendering. I thought we had built a component for that.",
        authorityBoundary:
          "Assistant bodies parse a bounded markdown subset (headings, bold, italics, inline code, fenced code, lists, blockquotes, rules) into the typed catalog Markdown/CodeBlock/Divider views — text nodes only, no raw HTML is constructible, and links render as safe text, never navigation. User input stays literal. Mid-stream unterminated markers render as plain text until closed; re-parsing per append never throws.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/markdown.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "chat_markdown.projector_unit",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/markdown.test.ts",
            description:
              "Proves the bounded subset parses to typed blocks, hostile link schemes stay inert text, unterminated **/`/``` render gracefully mid-stream, and segments lower to Markdown/CodeBlock/Divider catalog views with stable keys.",
          },
          {
            id: "chat_markdown.assistant_body",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Proves assistant note bodies project through the markdown views while user text stays literal.",
          },
          {
            id: "chat_markdown.smoke",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron fable fixture journey streams a mid-marker-split **streaming** reply and asserts the final assistant body renders a real <strong> with no literal ** text.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the markdown projector suite, the shell view suite, and the Electron smoke markdown assertion.",
      },
      {
        contractId: "openagents_desktop.seam.replaceable_owned_correlated_services.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "replaceable service lifecycle and operation correlation",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "github-issue", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "Desktop runtime, workspace, Sync, account, and history services are replaceable through the production host lifecycle; project, session, window, and app teardown closes exactly the resources it owns once, while one public-safe operation/session/run/correlation context survives the renderer-to-Sync path.",
        authorityBoundary:
          "Correlation carries bounded refs only and maps to private Sync causality refs; it never carries a path, URL, prompt, body, owner, token, credential, raw error, native handle, or provider payload. Replacement and disposal do not widen renderer or test-fixture authority.",
        seam: {
          client: "apps/openagents-desktop/src/runtime-gateway-contract.ts",
          server: "apps/openagents-desktop/src/desktop-host-lifecycle.ts",
        },
        evidenceRefs: [
          "apps/openagents-desktop/src/desktop-host-lifecycle.test.ts",
          "apps/openagents-desktop/src/desktop-operation-context.test.ts",
          "apps/openagents-desktop/tests/runtime-gateway.e2e.test.ts",
          "github:OpenAgentsInc/openagents#8684",
        ],
        oracles: [
          {
            id: "desktop_architecture.replaceable_owned_lifecycle",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/desktop-host-lifecycle.test.ts",
            description:
              "Uses the production lifecycle constructor with substitute runtime/workspace/Sync/account/history services and proves replacement, window close, app close, late-resource refusal, exact finalizer counts, and zero active slots.",
          },
          {
            id: "desktop_architecture.operation_correlation",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/tests/runtime-gateway.e2e.test.ts",
            description:
              "Preserves the same bounded operation/session/run/correlation refs through gateway observation, runtime command admission, Sync causality, response decoding, and the public-safe journal.",
          },
        ],
        verification:
          "The canonical Desktop verify gate runs lifecycle/correlation mutation and leak tests, builds Electron, executes the structured correlation path with substitute backing services, reloads the renderer, explicitly disposes the host, and requires active=0.",
      },
      {
        contractId: "openagents_desktop.seam.codex_trace_electron_acceptance.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "real Electron Codex trace acceptance",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-codex-session", statedBy: "owner", statedOn: "2026-07-10" },
        statement:
          "The built Electron app must render real owner-local Codex history as a stable named top-level catalog with nested agents, loss-accounted trace items, keyboard-operable topology, a reachable structured tool inspector, and ref-only selection restoration across a real renderer reload.",
        authorityBoundary:
          "The acceptance journey reads only the existing schema-bounded Runtime Gateway projection. Its receipt contains aggregate counts and timings only—never titles, transcript text, paths, raw JSONL, credentials, or stable private refs—and it grants no resume, write, sync, or provider execution authority.",
        seam: { client: "apps/openagents-desktop/src/electron-trace-acceptance.ts", server: "apps/openagents-desktop/src/main.ts" },
        evidenceRefs: [
          "apps/openagents-desktop/tests/electron-trace-acceptance.test.ts",
          "docs/sol/issues/desktop-codex-trace-acceptance.md",
          "github:OpenAgentsInc/openagents#8675",
        ],
        oracles: [{
          id: "codex_trace_real_electron_acceptance",
          kind: "bun-test",
          mode: "e2e",
          ref: "apps/openagents-desktop/src/electron-trace-acceptance.ts",
          description:
            "Runs inside built Electron against the real local Codex catalog, checks named/order-stable roots, child containment, nested topology, completeness, keyboard bindings, tool inspection, public-safe timings, and reload restoration.",
        }],
        verification:
          "bun run --cwd apps/openagents-desktop verify builds Electron, runs the normal contract suite, executes the real-history journey, reloads the renderer, and fails on every named video-blocking regression.",
      },
      {
        contractId: "openagents_desktop.seam.identity.local_first_account_link.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "two-tier native identity",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-10",
        },
        statement:
          "Desktop creates one immutable device-local identity before OpenAuth and remains usable in local-only mode. A verified OpenAgents account link is additive and reversible; unlink, denial, failed link, and restart retain local-authority rows and never relabel them server-confirmed.",
        authorityBoundary:
          "Local identity and LocalRevision rows use separate host-owned tables and device-local scopes. Only server-verified owner input may create a link. The renderer receives identity tier only; local refs, owner refs, tokens, storage, rows, and transport remain host-only, and hosted Sync refuses device-local scopes.",
        seam: {
          client: "apps/openagents-desktop/src/runtime-gateway-contract.ts",
          server: "apps/openagents-desktop/src/desktop-sync-host.ts",
        },
        evidenceRefs: [
          "packages/khala-sync/src/local-authority.ts",
          "packages/khala-sync-client/src/store-core.ts",
          "apps/openagents-desktop/src/renderer/settings.ts",
          "github:OpenAgentsInc/openagents#8666",
        ],
        oracles: [
          {
            id: "desktop_local_first_identity",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/tests/local-first-identity.e2e.test.ts",
            description:
              "Proves local identity restart stability, verified account link, reversible unlink, local retention, and tokenless projection.",
          },
        ],
        verification:
          "Desktop host, Runtime Gateway, Settings, boundary, build, and Electron smoke run in the normal Desktop verify sweep.",
      },
      {
        contractId: "openagents_desktop.seam.codex_loss_accounted_history.v2",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "local Codex history",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-10",
        },
        statement:
          "OpenAgents Desktop discovers active and archived Codex history without an age ceiling, preserves the parent/child agent graph, and shows a bounded source-order page for the selected agent with explicit redaction and gap accounting. The source equation is visible and unknown/corrupt records are never silently dropped.",
        authorityBoundary:
          "The worker is read-only and returns only schema-bounded, credential-redacted catalog/page projections through Runtime Gateway v4. Source files, raw JSONL, encrypted reasoning, credentials, filesystem authority, session resume, cloud sync, and provider runtime authority never enter the renderer.",
        seam: {
          client: "apps/openagents-desktop/src/renderer/boot.ts",
          server: "apps/openagents-desktop/src/runtime-gateway.ts",
        },
        evidenceRefs: [
          "apps/openagents-desktop/src/codex-history-contract.ts",
          "apps/openagents-desktop/src/codex-history.ts",
          "apps/openagents-desktop/src/renderer/history-workspace.ts",
        ],
        oracles: [
          {
            id: "codex_loss_accounted_history",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/tests/codex-subagent-history.test.ts",
            description:
              "Proves nested history graph, source order, rich items, paging, archive compression, redaction, and visible gaps.",
          },
          {
            id: "codex_history_scale",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/tests/codex-history-performance.e2e.test.ts",
            description:
              "Generates valid 100MiB/100-child/100k-item history and proves metadata-first discovery plus bounded pages.",
          },
        ],
        verification:
          "The Desktop verify sweep runs projection, gateway, Effect Native view, scale, boundary, build, and Electron smoke evidence.",
      },
      {
        contractId:
          "openagents_desktop.chat.thread_first_content_under_50ms.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "thread loading performance",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-10",
        },
        statement:
          "Threads must always show their first bounded message content in less than 50 milliseconds, regardless of total rollout size. Large threads must be chunked; full-rollout parsing is forbidden on the selection path.",
        authorityBoundary:
          "The 50-millisecond budget covers local first-content projection after selection. It does not authorize loading unbounded history, exposing raw events, or moving filesystem work onto Electron's main process.",
        evidenceRefs: [
          "apps/openagents-desktop/src/codex-history.ts",
          "apps/openagents-desktop/src/codex-history-worker.ts",
        ],
        oracles: [
          {
            id: "oversized_rollout_first_content.performance",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/tests/codex-history.e2e.test.ts",
            description:
              "Creates a 256 MiB sparse rollout and requires bounded first-content projection to finish under 50 ms.",
          },
        ],
        verification:
          "bun test apps/openagents-desktop/tests/codex-history.e2e.test.ts enforces the 50 ms wall-clock budget in the normal desktop test sweep.",
      },
      {
        contractId:
          "openagents_desktop.seam.runtime_gateway_closed_protocol.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "Desktop Runtime Gateway",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-10",
        },
        statement:
          "The signed Desktop renderer reaches host runtime state through one versioned closed query/command/event protocol. Protocol v8 includes bounded provider-native Codex history, canonical confirmed conversation/timeline/graph/command-outcome projections, and exact-ref start/interrupt commands; unknown requests fail schema decoding, unavailable or pending commands never appear completed, expired commands never dispatch, lifecycle events are ordered and disposable, and the renderer never receives runtime credentials or a generic transport.",
        authorityBoundary:
          "Electron main owns the Runtime Gateway and validates the invoking top-level bundled renderer. The renderer may request bounded OpenAgents session entry/exit and canonical conversation operations but receives only typed projections/outcomes; it gets no credential, callback/authorize URL, raw Khala Sync/store/session/transport authority, provider credential, raw IPC channel, MessagePort, filesystem handle, process handle, or raw runtime event.",
        seam: {
          client: "apps/openagents-desktop/src/preload.cts",
          server: "apps/openagents-desktop/src/runtime-gateway.ts",
        },
        evidenceRefs: [
          "apps/openagents-desktop/src/runtime-gateway-contract.ts",
          "apps/openagents-desktop/tests/electron-boundary.test.ts",
        ],
        oracles: [
          {
            id: "runtime_gateway_closed_protocol.e2e",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/tests/runtime-gateway.e2e.test.ts",
            description:
              "Round-trips schema-decoded renderer requests and proves truthful capability, unavailable command, lifecycle ordering, disposal, and rejection behavior.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the seam suite, mechanical boundary oracle, bundle, and real Electron bootstrap smoke.",
      },
      {
        contractId: "openagents_desktop.sync.host_owned_sqlite.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "Khala Sync local persistence",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-10",
        },
        statement:
          "Desktop opens the shared Khala Sync SQLite store inside Electron main, persists one installation identity across restart, migrates the supported legacy store without data loss, refuses a newer store before mutation with recovery guidance, and after native-session verification composes the shared HTTP/WebSocket session on exactly the server-derived owner's personal scope. Sparse event batches replay from the durable cursor; rotation is re-read host-side and the session closes before the store.",
        authorityBoundary:
          "The renderer receives only bounded phase and freshness. Owner refs, credentials, database path and handle, installation identity refs, rows, mutation queue, transport, and session remain host-only. The local database is a reconstructible cache/offline queue and never server authority; authenticated substrate is not an authoritative conversation projection.",
        evidenceRefs: [
          "apps/openagents-desktop/src/desktop-sync-host.ts",
          "apps/openagents-desktop/src/desktop-sync-store.ts",
          "packages/khala-sync-client/src/store-core.ts",
        ],
        oracles: [
          {
            id: "desktop_sync_host.lifecycle",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/desktop-sync-host.test.ts",
            description:
              "Proves restart-stable identity, private permissions, supported legacy migration, newer-version refusal, personal-scope selection, dynamic token lookup, live/freshness transition, session-before-store close, and reuse of the shared SQLite store.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the host lifecycle suite and real Electron gateway bootstrap.",
      },
      {
        contractId: "openagents_desktop.sync.native_conversation_continuity.v1",
        state: "enforced",
        surface: "openagents-desktop-and-mobile-hosts",
        productArea: "authoritative cross-device conversation continuity",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-10",
        },
        statement:
          "A Desktop-created canonical chat_thread and first chat_message can be confirmed on mobile, mobile can append one canonical follow-up, and both native hosts converge on identical public-safe refs, confirmed entity versions, thread cursor, and live phase, then reconstruct the same state after restart without duplicates.",
        authorityBoundary:
          "Only server-confirmed chat_thread/chat_message rows enter the bounded conversation projection. Owner identity, credentials, store/session/overlay/transport objects, optimistic bodies, provider runtime events, and assistant-role inference remain host-only or explicitly outside this contract; denial and sign-out remove the conversation capability.",
        evidenceRefs: [
          "packages/khala-sync-client/src/chat.ts",
          "packages/khala-sync-client/src/conversation.ts",
          "apps/openagents-desktop/src/desktop-sync-host.ts",
          "apps/openagents-mobile/src/sync/mobile-sync-host-core.ts",
          "docs/sol/issues/native-conversation-continuation.md",
          "github:OpenAgentsInc/openagents#8668",
        ],
        oracles: [
          {
            id: "native_timeline_fault_convergence.e2e",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/tests/native-timeline-fault-convergence.e2e.test.ts",
            description:
              "Feeds reordered/duplicate timeline rows and an authoritative gap snapshot through the Desktop and Expo/mobile SQLite adapters, proving byte-equivalent refs, versions, cursors, and retractions.",
          },
          {
            id: "native_conversation_continuation.e2e",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/tests/native-conversation-continuation.e2e.test.ts",
            description:
              "Runs the real Desktop node:sqlite host and mobile Expo-SQLite host over the shared session/overlay protocol against a server-authoritative chat fake, then proves Desktop-to-mobile-to-Desktop convergence, exact versions/cursor, and restart reconstruction.",
          },
        ],
        verification:
          "The native conversation continuation e2e runs in the normal Desktop sweep; shared chat mutator/projection tests run in the khala-sync-client sweep and the collection package regression proves existing consumers use the same centralized mutators.",
      },
      {
        contractId: "openagents_desktop.seam.runtime_gateway_conversation.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "authoritative conversation Runtime Gateway",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-10",
        },
        statement:
          "The signed Desktop renderer can query confirmed canonical conversation catalogs, threads, their current agent timeline, and durable command outcomes, and enqueue canonical create/append plus exact thread/message/run start or interrupt commands only through Runtime Gateway protocol v8. Enqueues return pending_reconcile or unknown_pending_reconcile with the durable mutation id, never optimistic completed; expired commands are terminal and never execute after reconnect.",
        authorityBoundary:
          "The seam carries public-safe thread/message/run/WorkContext refs, bounded canonical timeline items, timestamps, confirmed entity versions, exact scope phase/cursor, and pending count only. It carries no owner identity, credential, store/session/overlay/transport, generic IPC, raw provider stream, or process authority; not-live/read failure is typed and body-free.",
        seam: {
          client: "apps/openagents-desktop/src/preload.cts",
          server: "apps/openagents-desktop/src/runtime-gateway.ts",
        },
        evidenceRefs: [
          "apps/openagents-desktop/src/runtime-gateway-contract.ts",
          "apps/openagents-desktop/src/main.ts",
          "packages/khala-sync-server/src/runtime-mutators.ts",
          "apps/pylon/src/orchestration/runtime-intent-enforcement.ts",
          "apps/openagents-mobile/src/conversation/mobile-conversation.ts",
          "docs/sol/issues/native-streamed-conversation-handoff.md",
          "docs/sol/issues/desktop-runtime-conversation.md",
          "github:OpenAgentsInc/openagents#8669",
        ],
        oracles: [
          {
            id: "runtime_gateway_conversation.e2e",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/tests/runtime-gateway.e2e.test.ts",
            description:
              "Round-trips confirmed catalog/thread/timeline/command-outcome projections and exact create/append/start/interrupt refs through protocol v8, proves pending-reconcile and terminal-expiry outcomes, unavailable fail-closed behavior, bounds, and schema rejection.",
          },
          {
            id: "runtime_agent_run_transactional_binding",
            kind: "bun-test",
            mode: "e2e",
            ref: "packages/khala-sync-server/src/runtime-mutators.test.ts",
            description:
              "Against real local Postgres, proves durable start admission transactionally creates the canonical agent run, exact semantic retry reconciles, conflicting reuse fails closed, WorkContext/repository snapshot stays immutable, and runtime events preserve exact thread/run refs.",
          },
          {
            id: "runtime_provider_single_generation_claim",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/pylon/src/orchestration/runtime-intent-enforcement.test.ts",
            description:
              "Races two independent consumers against one admitted turn and proves only the winner of the durable sequence-one event claim invokes Codex.",
          },
          {
            id: "mobile_same_thread_runtime_control",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-mobile/tests/mobile-conversation.test.ts",
            description:
              "Proves mobile submits the same confirmed thread/message/run refs through the shared runtime builders, observes a later confirmed terminal projection, and interrupts only the exact confirmed run.",
          },
        ],
        verification:
          "Runtime Gateway e2e, Electron mechanical boundary, Desktop typecheck/build, and behavior-contract validation run in the normal Desktop sweep.",
      },
      {
        contractId: "openagents_desktop.seam.runtime_gateway_agent_timeline.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "confirmed provider-neutral agent timeline gateway",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-10",
        },
        statement:
          "Runtime Gateway protocol v8 accepts bounded agent.timeline by exact runRef, conversation.timeline by exact threadRef, and conversation.commandOutcome by exact stable intentId/threadRef, returning only confirmed server projections with bounded canonical timeline items; unavailable, not-found, and read failure remain typed and body-free.",
        authorityBoundary:
          "Electron main composes the shared confirmed timeline reader only behind authenticated live Sync. The server-projected agent_run.routeId is the sole route/thread binding; renderer code cannot derive it from runRef. The seam may expose bounded runtime/backend/WorkContext classification but never owner/objective/repository contents, provider source, raw payload, external callback, auth/store/session/transport, generic IPC, or process authority.",
        seam: {
          client: "apps/openagents-desktop/src/preload.cts",
          server: "apps/openagents-desktop/src/runtime-gateway.ts",
        },
        evidenceRefs: [
          "packages/khala-sync-client/src/agent-timeline.ts",
          "apps/openagents-desktop/src/runtime-gateway-contract.ts",
          "apps/openagents-desktop/src/main.ts",
          "docs/sol/issues/desktop-runtime-agent-timeline.md",
          "github:OpenAgentsInc/openagents#8673",
        ],
        oracles: [
          {
            id: "runtime_gateway_agent_timeline.e2e",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/tests/runtime-gateway.e2e.test.ts",
            description:
              "Round-trips the exact agent.timeline query and confirmed run/route/event projection through both schema boundaries, proves server routeRef preservation, body-free non-live failure, public-field bounds, and invalid-ref rejection.",
          },
        ],
        verification:
          "Runtime Gateway e2e, Electron boundary, Desktop host/typecheck/build, shared timeline, and behavior-contract validation run in the normal sweeps.",
      },
      {
        contractId: "openagents_desktop.seam.runtime_gateway_live_agent_graph.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "confirmed live-agent graph gateway",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-11",
        },
        statement:
          "Runtime Gateway protocol v8 emits server-confirmed openagents.live_agent_graph.v1 post-images inside the existing cursor-aware thread subscription. Exact resume emits the current bounded graph set, a proven cursor gap emits one authoritative-refetch snapshot, and interrupted or non-live scopes emit no cached graph authority.",
        authorityBoundary:
          "Electron main reads graph entities only from the authenticated canonical thread scope. Each event carries matching graphRefs plus at most eight validated post-images, with at most 2,000 nodes and 4,000 edges in aggregate. Provider-native history, raw callbacks, credentials, store/session/transport handles, and process authority never cross the renderer seam; unsupported graph facts remain explicit unknowns.",
        seam: {
          client: "apps/openagents-desktop/src/preload.cts",
          server: "apps/openagents-desktop/src/runtime-gateway.ts",
        },
        evidenceRefs: [
          "packages/khala-sync-client/src/live-agent-graph.ts",
          "packages/khala-sync-client/src/live-conversation.ts",
          "apps/openagents-desktop/src/runtime-gateway-contract.ts",
          "apps/openagents-desktop/src/runtime-live-subscriptions.ts",
          "github:OpenAgentsInc/openagents#8691",
        ],
        oracles: [
          {
            id: "confirmed_live_agent_graph_read_model",
            kind: "bun-test",
            mode: "unit",
            ref: "packages/khala-sync-client/src/live-agent-graph.test.ts",
            description:
              "Reads only graph-valid post-images from the exact live thread scope, hides cached rows while non-live, and bounds a busy thread to the newest aggregate-safe graph set.",
          },
          {
            id: "runtime_gateway_live_agent_graph_reconnect",
            kind: "bun-test",
            mode: "e2e",
            ref: "packages/khala-sync-client/src/live-conversation.test.ts",
            description:
              "Proves graph refs and post-images ride confirmed resume and authoritative-refetch snapshots through the same durable cursor and bounded subscription path without polling.",
          },
          {
            id: "runtime_gateway_live_agent_graph_boundary",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/tests/runtime-gateway.e2e.test.ts",
            description:
              "Round-trips a bounded canonical graph post-image through both protocol-v8 schema boundaries and advertises agent-graph capability only with live Sync.",
          },
        ],
        verification:
          "Khala Sync client graph/live-conversation tests and typecheck plus Runtime Gateway e2e, Desktop typecheck/build, and behavior-contract validation run in the normal sweeps.",
      },
      {
        contractId: "openagents_desktop.chat.authoritative_sync_mode.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "visible authoritative conversation",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-10",
        },
        statement:
          "At boot, the Effect Native Desktop shell selects exactly one chat authority: Runtime Gateway v8 confirmed Sync when its catalog is live, otherwise the existing explicit local-only host. In Sync mode, visible threads/messages, durable command outcomes, and bounded assistant lifecycle items come from confirmed projections; create/append/start remain pending until exact refs and terminal state reconcile.",
        authorityBoundary:
          "Mode is selected once per renderer lifetime so local and account-linked conversations never mix. The adapter uses only the generic decoded Runtime Gateway call; it gets no owner/credential/native authority, does not add preload IPC, does not infer assistant roles, and reports an unconfirmed append as still pending rather than completed.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/runtime-conversation.ts",
          "apps/openagents-desktop/src/renderer/boot.ts",
          "docs/sol/issues/desktop-visible-sync-conversation.md",
          "github:OpenAgentsInc/openagents#8670",
        ],
        oracles: [
          {
            id: "desktop_authoritative_sync_mode.adapter",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/runtime-conversation.test.ts",
            description:
              "Proves one-time live-vs-local selection, confirmed catalog/transcript mapping, stable client refs, create/append exact-ref confirmation, and pending timeout honesty.",
          },
        ],
        verification:
          "The adapter, shell, Runtime Gateway, Electron boundary, typecheck, bundle, and behavior-contract validation run in the normal Desktop sweep.",
      },
      {
        contractId: "openagents_desktop.session.os_encrypted_custody.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "native OpenAgents session custody",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-10",
        },
        statement:
          "Desktop keeps the native OpenAgents access token, refresh token, and server-derived owner ref in one versioned Electron safeStorage-encrypted record under its private userData root; recovered credentials remain unverified until the server accepts them.",
        authorityBoundary:
          "OS encryption and private disk custody do not verify the credential, authorize Khala Sync rows or commands, create a device_session, or expose any credential field to preload, renderer, Runtime Gateway, logs, receipts, or public errors.",
        evidenceRefs: [
          "apps/openagents-desktop/src/desktop-session-vault.ts",
          "docs/sol/issues/desktop-session-vault.md",
          "github:OpenAgentsInc/openagents#8661",
        ],
        oracles: [
          {
            id: "desktop_session_vault.os_encrypted_custody",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/desktop-session-vault.test.ts",
            description:
              "Proves safeStorage encryption, private atomic persistence, encryption/backend refusal, malformed-record purge, bounded recovery, idempotent clear, and public-safe failures.",
          },
        ],
        verification:
          "The desktop-session-vault and Electron-boundary suites prove custody and renderer isolation; Desktop typecheck/build and behavior-contract validation gate the integration.",
      },
      {
        contractId:
          "openagents_desktop.session.recovered_validation_rotation.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "native OpenAgents session recovery",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-10",
        },
        statement:
          "Desktop validates recovered native credentials through the existing OpenAgents native-session boundary, persists bounded OpenAuth rotation before readiness, purges denial or owner mismatch, and never equates verified session readiness with live Sync.",
        authorityBoundary:
          "Server verification establishes only a native OpenAgents session. It does not make Khala Sync live, authorize cached rows or commands, create a device_session, or expose owner or replacement token fields through the Runtime Gateway.",
        evidenceRefs: [
          "apps/openagents-desktop/src/desktop-session-recovery.ts",
          "docs/sol/issues/desktop-session-recovery.md",
          "github:OpenAgentsInc/openagents#8662",
        ],
        oracles: [
          {
            id: "desktop_session_recovery.validation_rotation",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/desktop-session-recovery.test.ts",
            description:
              "Proves exact native-session verification, rotation-before-ready, denial/owner-mismatch purge, transient retention, and tokenless results.",
          },
          {
            id: "desktop_session_recovery.gateway_projection",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/tests/runtime-gateway.e2e.test.ts",
            description:
              "Proves the renderer-facing Runtime Gateway projects only bounded verified readiness without owner or credential fields.",
          },
        ],
        verification:
          "Desktop recovery, Runtime Gateway, and Electron-boundary suites plus typecheck/build enforce both sides of the host-only validation seam.",
      },
      {
        contractId: "openagents_desktop.session.loopback_pkce_entry_exit.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "native OpenAgents session entry and exit",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-10",
        },
        statement:
          "Desktop main binds a temporary literal-loopback listener, launches the exact public-client GitHub code + S256 request, validates callback state, verifies the server owner before encrypted custody, and revokes both credential classes before local sign-out.",
        authorityBoundary:
          "The Runtime Gateway accepts only argument-free session entry/exit commands and returns bounded completed/cancelled/unavailable phase. No callback, authorize URL, state, code, verifier, owner, access token, or refresh token enters preload, renderer, logs, receipts, or public errors; verified session is not live Sync.",
        evidenceRefs: [
          "apps/openagents-desktop/src/desktop-session-pkce.ts",
          "docs/sol/issues/desktop-session-pkce.md",
          "github:OpenAgentsInc/openagents#8664",
        ],
        oracles: [
          {
            id: "desktop_session_pkce.loopback_entry_exit",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/desktop-session-pkce.test.ts",
            description:
              "Proves literal-loopback lifecycle, callback state, public-safe response, exact authorize/exchange tuples, server owner verification, immediate rotation, timeout/cancel, and dual revocation before clear.",
          },
          {
            id: "desktop_session_pkce.gateway_commands",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/tests/runtime-gateway.e2e.test.ts",
            description:
              "Proves argument-free session commands round-trip through the closed Runtime Gateway and return only bounded phase outcomes.",
          },
        ],
        verification:
          "Desktop PKCE, Runtime Gateway, and Electron-boundary suites plus typecheck/build enforce host composition without a live browser or GUI in tests.",
      },
      {
        contractId: "openagents_desktop.session.effect_native_controls.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "native OpenAgents session UI",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-10",
        },
        statement:
          "Desktop Settings renders the exact bounded OpenAgents session phase and routes visible sign-in/sign-out controls through typed Effect Native intents to argument-free Runtime Gateway commands, with honest in-flight and unavailable states.",
        authorityBoundary:
          "The renderer sees only signed-out, unverified, session-ready, denied, unavailable, or local authenticating presentation state. It receives no callback/authorize URL, state, code, verifier, owner, token, storage handle, or live Sync authority.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/settings.ts",
          "docs/sol/issues/desktop-session-controls.md",
          "github:OpenAgentsInc/openagents#8665",
        ],
        oracles: [
          {
            id: "desktop_session_controls.effect_native_intents",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/settings.test.ts",
            description:
              "Drives the real Effect Native Settings view and intent registry through signed-out, authenticating, session-ready, sign-in, and sign-out states with a tokenless fake bridge.",
          },
          {
            id: "desktop_session_controls.gateway_phase",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/tests/runtime-gateway.e2e.test.ts",
            description:
              "Proves bootstrap carries an explicit bounded session phase and session commands remain argument-free and single-flight.",
          },
        ],
        verification:
          "Settings view/intent, Runtime Gateway, and Electron-boundary suites plus Desktop typecheck/build enforce the visible tokenless path without GUI automation.",
      },
      {
        contractId: "openagents_desktop.chat.fable_local_lane_no_substitution.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "local-mode composer harness lanes",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-11",
        },
        statement:
          "In local (not-signed-in) mode, selecting Fable runs a real streaming Claude turn on this machine with zero login on an isolated sibling Claude account home (never the default ~/.claude); selecting a harness never routes to the cloud gateway or another provider; an unavailable lane renders a disabled chip with its reason and a Send that does not accept the action.",
        authorityBoundary:
          "The renderer receives only bounded, path-redacted typed stream events and typed availability/failure reasons — never tokens, account homes, credentials, raw SDK payloads, or provider error bodies. Main owns thread history; the renderer supplies only the new message.",
        evidenceRefs: [
          "apps/openagents-desktop/src/fable-local-contract.ts",
          "apps/openagents-desktop/src/renderer/local-harness.ts",
          "apps/openagents-desktop/src/fable-local-runtime.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "fable_local.runtime_isolation_and_rotation",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/fable-local-runtime.test.ts",
            description:
              "Proves sibling-home discovery excludes the default ~/.claude, read-only headless SDK options, bounded/redacted event mapping, same-lane account rotation only before content, and that no ready account yields a typed unavailable result with the SDK never loaded.",
          },
          {
            id: "fable_local.renderer_no_silent_substitution",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/local-harness.test.ts",
            description:
              "Proves fable sends stream progressively with trace lines, codex and unavailable-fable sends are typed refusals, and the legacy gateway sendMessage is reachable only by a laneless send.",
          },
          {
            id: "fable_local.evidence_gated_composer",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Proves unavailable lanes render disabled chips with visible captions, Send disables for the selected dead lane, submit refuses while keeping the draft, and selection auto-moves off a dead default.",
          },
        ],
        verification:
          "The Desktop verify gate runs the runtime/renderer/shell suites and the fixture-driven smoke journey (select Fable, send, observe progressive text, tool trace, finalize) inside built Electron.",
      },
      {
        contractId: "openagents_desktop.chat.fable_local_lane_no_substitution.v2",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "local-mode composer harness lanes",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-11",
        },
        statement:
          "IT HAS TO BE FABLE: the local Fable lane requests model claude-fable-5 with skills removed from the lane (disallowed, never offered-then-denied); if the SDK init reports an effective model outside the claude-fable family the turn fails typed as model_substituted naming requested vs effective, no substituted output is ever streamed or persisted as Fable, and the lane never rotates accounts on a model mismatch; the effective model is emitted as a typed event and displayed with the reply (e.g. Fable · claude-fable-5), never asserted from the brand alone.",
        authorityBoundary:
          "Model identity crossing the Electron boundary is only the bounded SDK-reported effective-model string and the typed model_substituted reason with a bounded requested-vs-effective detail — never raw SDK payloads, account homes, or provider error bodies. Model-level refusal grants no rotation, retry, or gateway-fallback authority.",
        evidenceRefs: [
          "apps/openagents-desktop/src/fable-local-contract.ts",
          "apps/openagents-desktop/src/fable-local-runtime.ts",
          "apps/openagents-desktop/src/renderer/local-harness.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "fable_local.model_no_substitution_runtime",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/fable-local-runtime.test.ts",
            description:
              "Proves SDK options carry model claude-fable-5 plus disallowedTools Skill and skills off; an init reporting a non-Fable model yields a typed model_substituted failure naming requested vs effective with zero assistant deltas surfaced and no account rotation; a claude-fable init (including versioned IDs) streams normally and emits the effective model.",
          },
          {
            id: "fable_local.model_effective_caption",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/local-harness.test.ts",
            description:
              "Proves the renderer displays the SDK-reported effective model as the Fable · <model> transcript caption from the typed model_effective event, positioned with the trace lines the persisted thread also carries.",
          },
        ],
        verification:
          "The Desktop verify gate runs the runtime and renderer suites covering model-level substitution refusal and the effective-model caption; the fixture smoke journey streams with the fixture init reporting claude-fable-5.",
      },
      // =====================================================================
      // Lane C (#8712) — Codex delegation. Kept as a clearly separate block:
      // a parallel chat-UI lane also edits this registry; the coordinator
      // owns resolving the registry-version bump on merge.
      // =====================================================================
      {
        contractId: "openagents_desktop.seam.codex_delegation_no_substitution.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "Fable-to-Codex sub-agent delegation",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-codex-session",
          statedBy: "owner",
          statedOn: "2026-07-11",
        },
        statement:
          "The local Fable lane may delegate bounded tasks to Codex sub-agents only through the typed mcp__codex__delegate tool: every child is requested pinned to model gpt-5.6-sol at medium reasoning effort as spawn-config truth (the codex exec --json stream does not echo model or effort, and every result and ledger row is labeled requested accordingly); children run read-only in isolated scratch workspaces on registry-isolated Codex account homes, never the default ~/.codex; a revoked-credential account is never silently skipped — rotation emits a typed account_reconnect_required event per skipped account, and when every registered account is revoked the delegation returns a typed unavailable result naming the reconnect need; at most 3 children run concurrently and 6 per turn, with over-cap calls refused typed before any spawn; exact per-child token usage from turn.completed (total = input + output + reasoning) rolls into the session usage ledger and the Fleet view's evidence-labeled Session usage section; and a session-observed revoked credential or failed usage probe supersedes the registry's presence-based ready with a typed reconnect-required readiness state.",
        authorityBoundary:
          "The renderer receives only bounded typed child lifecycle events (childRef, account ref, public-safe summaries with the child workspace redacted, exact token counts, typed failure reasons) and the typed session-ledger snapshot — never prompts, raw JSONL, credentials, auth paths, or local paths beyond the <child-workspace> label. Delegation grants no write, network-spend, or default-home authority, and the spawn-config model pin is not presented as a provider echo.",
        seam: {
          client: "apps/openagents-desktop/src/renderer/fleet-workspace.ts",
          server: "apps/openagents-desktop/src/codex-child-runtime.ts",
        },
        evidenceRefs: [
          "apps/openagents-desktop/src/codex-child-contract.ts",
          "apps/openagents-desktop/src/fable-local-runtime.ts",
          "apps/openagents-desktop/src/usage-ledger-contract.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "codex_delegation.child_runtime_rotation_and_usage",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/codex-child-runtime.test.ts",
            description:
              "Drives the real JSONL parser with the receipted spawn recipe and revoked-token shapes: pinned model/effort args, isolated CODEX_HOME, exact usage totals, typed visible rotation, typed all-accounts-unavailable, host-side timeout, and concurrent isolated children.",
          },
          {
            id: "codex_delegation.fable_tool_caps_and_events",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/fable-local-runtime.test.ts",
            description:
              "Proves the delegate tool is offered only with the fully-qualified allowed name, per-turn concurrency and total caps refuse typed without spawning, child lifecycle events flow schema-valid through the FableLocalEvent envelope, and the tool result labels usage as requested spawn-config truth.",
          },
          {
            id: "codex_delegation.session_ledger_and_readiness_override",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/fleet-workspace.test.ts",
            description:
              "Proves the evidence-labeled Session usage section renders exact per-account rows with the requested codex model, and that ledger reconnect rows or failed probes supersede presence-based ready with the typed reconnect-required state.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the child-runtime, delegate, ledger, and fleet suites plus the fixture smoke journey where a fable fixture turn calls the delegate once (scripted child) and the transcript shows the tool_use/tool_result pair with the ledger row rendered in the Fleet view.",
      },
      {
        contractId: "openagents_desktop.agent_graph.pointer_keyboard_focus_equivalence.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "live agent supervision",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "khala-code-session",
          statedBy: "owner",
          statedOn: "2026-07-11",
        },
        statement:
          "OpenAgents Desktop presents Runtime Gateway v8's confirmed canonical agent graph as one parent/subagent hierarchy. Pointer activation, keyboard activation, and screen-reader buttons select the same typed agent ref; status, attention, current action, elapsed time, terminal reason, session, runtime, provider, and worktree facts remain inspectable; historical authority is labeled and never gains a live focus control; rapid replacement falls back deterministically and large graphs disclose their bound.",
        authorityBoundary:
          "The renderer accepts only graph post-images already schema-decoded by Runtime Gateway v8 and projected by the shared confirmed client model. Agent selection is local inspection/focus state, not execution movement or provider/process authority. Historical projections remain inspectable with canControl=false. No provider payload, history heuristic, credential, path, store/session handle, or transport handle enters the view.",
        evidenceRefs: [
          "packages/khala-sync-client/src/live-agent-graph-presentation.ts",
          "apps/openagents-desktop/src/renderer/runtime-conversation.ts",
          "apps/openagents-desktop/src/renderer/runtime-agent-graph.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "docs/sol/2026-07-11-cut-12-live-agent-supervision-ui-receipt.md",
          "github:OpenAgentsInc/openagents#8692",
        ],
        oracles: [
          {
            id: "agent_graph.gateway_projection_and_no_poll",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/runtime-conversation.test.ts",
            description:
              "Drives a real Runtime Gateway live update carrying a validated root+child graph through the existing fenced subscription, asserts the thread receives the shared hierarchy projection, and retains the no-recurring-timeline-poll oracle.",
          },
          {
            id: "agent_graph.pointer_keyboard_screen_reader_view",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/runtime-agent-graph.test.ts",
            description:
              "Proves expand/select/focus/Escape all use typed Effect Native intent refs, selection exposes every required fact through ordinary accessible buttons and a region/table inspector, hierarchy depth is preserved, and historical authority removes live focus.",
          },
          {
            id: "agent_graph.shared_fault_and_scale_model",
            kind: "bun-test",
            mode: "unit",
            ref: "packages/khala-sync-client/src/live-agent-graph-presentation.test.ts",
            description:
              "Enforces deterministic hierarchy ordering, explicit missing facts, rapid-selection fallback, terminal/historical refusal, newest attachment/cursor selection, and exact large-graph remainder.",
          },
        ],
        verification:
          "bun test src/renderer/runtime-agent-graph.test.ts src/renderer/runtime-conversation.test.ts src/renderer/shell.test.ts plus typecheck, full test, build, and Electron smoke in apps/openagents-desktop; shared projection tests/typecheck run in packages/khala-sync-client.",
      },
      // =====================================================================
      // EP250 UI-owned reconnect lane — separate block: parallel lanes also
      // edit this registry; the coordinator owns the version bump on merge.
      // =====================================================================
      {
        contractId: "openagents_desktop.settings.ui_owned_codex_reconnect.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "Settings Codex account reconnect",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "owner-video-review",
          statedBy: "owner",
          statedOn: "2026-07-11",
        },
        statement:
          "don't recommend me the CLI command for Fleet Connect. We're doing stuff with the UI now. Like, CLI stuff is nice, but the UI controls need to be working.",
        authorityBoundary:
          "The Settings Codex section owns connect AND per-account reconnect through the hardened device-auth bridge: the reconnect action carries exactly one grammar-validated account ref that main re-validates against its own registry listing before spawning the receipted per-ref re-auth (auth codex --account <ref> --force-device-login into the SAME isolated home; never ~/.codex). Fleet stays overview-only — its broken-credential rows navigate to Settings via the existing DesktopSettingsToggled intent and mutate nothing. No settings state renders copy instructing the user to run a CLI command, and no tokens, emails, or raw child output cross the bridge.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/settings.ts",
          "apps/openagents-desktop/src/codex-connect.ts",
          "apps/openagents-desktop/src/codex-connect-contract.ts",
          "apps/openagents-desktop/src/renderer/fleet-workspace.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "ui_owned_reconnect.settings_rows_and_no_cli_copy",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/settings.test.ts",
            description:
              "Proves credential-failed rows render a working per-account Reconnect button (ready rows none), the full intent loop drives the ref-targeted bridge through awaiting_browser to connected with the accounts projection re-listed so readiness flips without restart (including after a FAILED exit), and that NO settings state renders copy matching a CLI-command pattern.",
          },
          {
            id: "ui_owned_reconnect.per_ref_spawn_receipt",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/codex-connect.test.ts",
            description:
              "Proves startReconnect spawns exactly auth codex --account <ref> --force-device-login for a ref main itself listed, refuses unknown or malformed refs typed, holds single-flight across connect and reconnect, and surfaces the bounded public-safe pylon-auth failure detail.",
          },
          {
            id: "ui_owned_reconnect.fleet_fix_in_settings",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/fleet-workspace.test.ts",
            description:
              "Proves broken-credential fleet rows carry the Fix in Settings navigation over the existing DesktopSettingsToggled intent with no account mutation from Fleet, and that a successful probe this session clears a stale reconnect override (probe evidence rules).",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the settings, codex-connect, and fleet suites plus the Electron smoke journey asserting the revoked fixture account renders its Reconnect button in Settings.",
      },
    ],
  };
