import {
  BehaviorContractSchemaVersion,
  type BehaviorContractRegistryDocument,
} from "@openagentsinc/behavior-contracts";

export const openAgentsDesktopUxContractRegistry: BehaviorContractRegistryDocument =
  {
    schemaVersion: BehaviorContractSchemaVersion,
    version: "2026-07-11.19",
    contracts: [
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
          "The signed Desktop renderer reaches host runtime state through one versioned closed query/command/event protocol. Protocol v6 includes bounded provider-native Codex history, canonical confirmed conversation/timeline queries, and exact-ref start/interrupt commands; unknown requests fail schema decoding, unavailable or pending commands never appear completed, lifecycle events are ordered and disposable, and the renderer never receives runtime credentials or a generic transport.",
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
          "Desktop opens the shared Khala Sync SQLite store inside Electron main, persists one installation identity across restart, and after native-session verification composes the shared HTTP/WebSocket session on exactly the server-derived owner's personal scope. Rotation is re-read host-side and the session closes before the store.",
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
              "Proves restart-stable identity, private permissions, personal-scope selection, dynamic token lookup, live/freshness transition, session-before-store close, and reuse of the shared SQLite store.",
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
          "The signed Desktop renderer can query confirmed canonical conversation catalogs, threads, and their current agent timeline and enqueue canonical create/append plus exact thread/message/run start or interrupt commands only through Runtime Gateway protocol v6. Enqueues return pending_reconcile or unknown_pending_reconcile with the durable mutation id, never optimistic completed.",
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
              "Round-trips confirmed catalog/thread/timeline projections and exact create/append/start/interrupt refs through protocol v6, proves pending-reconcile outcomes, unavailable fail-closed behavior, bounds, and schema rejection.",
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
          "Runtime Gateway protocol v6 accepts bounded agent.timeline by exact runRef and conversation.timeline by exact threadRef, returning only a live confirmed agent-run snapshot with its server-projected routeRef and at most 500 ordered bounded canonical timeline items; unavailable, not-found, and read failure remain typed and body-free.",
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
          "At boot, the Effect Native Desktop shell selects exactly one chat authority: Runtime Gateway v6 confirmed Sync when its catalog is live, otherwise the existing explicit local-only host. In Sync mode, visible threads/messages and bounded assistant lifecycle items come from confirmed projections; create/append/start remain pending until exact refs and terminal state reconcile.",
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
    ],
  };
