import {
  BehaviorContractSchemaVersion,
  type BehaviorContractRegistryDocument,
} from "@openagentsinc/behavior-contracts";

export const openAgentsDesktopUxContractRegistry: BehaviorContractRegistryDocument =
  {
    schemaVersion: BehaviorContractSchemaVersion,
    version: "2026-07-11.34",
    contracts: [
      {
        contractId: "openagents_desktop.chat.compact_message_details_affordance.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "chat transcript chrome",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "that metadata button needs to be way smaller and more like an icon button, not a huge ginormous circle.",
        authorityBoundary:
          "Only the details affordance presentation changes: it stays a real keyboard-focusable catalog Button dispatching the same typed DesktopMessageSelected intent with the same accessible label. The catalog IconButton's fixed 44px circle is simply no longer used for this affordance; no local UI primitive is introduced (ghost Button lowered via typed style tokens: zero padding, caption scale, muted color).",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/shell.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "compact_details_affordance.not_icon_circle",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Proves every message row's details affordance is a ghost catalog Button (NOT the IconButton circle variant) with zero padding, caption type scale, and muted color, still dispatching DesktopMessageSelected with the message key.",
          },
          {
            id: "compact_details_affordance.smoke",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke fails if the rendered details affordance carries the icon-button variant or exceeds a compact line-height bound before opening the inspector through it.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the shell view suite and the Electron smoke message-inspector step with the compact-affordance guards.",
      },
      {
        contractId: "openagents_desktop.chat.typed_tool_call_cards.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "chat transcript tool-call rendering",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "why don't you go improve the UI of those tool calls so it's not just JSON stuff? Like I thought we had some custom components that showed things properly, not these JSON blobs.",
        authorityBoundary:
          "Tool cards re-present the SAME bounded, redacted trace payloads the system notes already carried — no new data crosses the Electron boundary. One card per invocation updates in place from started to ok/failed (pairing is honest renderer-side toolName+order FIFO because the events carry no invocation id); the bounded raw args/result stay reachable behind a compact expand affordance and are never the default rendering; failure text renders as content. Tool cards drop the SYSTEM role label (the tool title is the header) but keep timestamps.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/tool-cards.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/fable-local-contract.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "tool_cards.pairing_humanization_fallback",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/tool-cards.test.ts",
            description:
              "Table-driven humanization per known tool (delegate/Agent/Bash/Read/Write/Edit/Glob/Grep/ToolSearch/WebSearch/WebFetch), started+ok folding into one card, unknown-tool bounded compact fallback with no raw JSON, failed-state result text, and the text-parse fallback for pre-typed persisted notes.",
          },
          {
            id: "tool_cards.transcript_rendering",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Proves the transcript renders one role=tool card per invocation with humanized title/detail, toned status chip, result line, no SYSTEM sender label, collapsed-by-default raw details behind the compact toggle, and the expand intent loop through the real registry.",
          },
          {
            id: "tool_cards.smoke",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron fixture journey asserts the delegate invocation renders ONE updating card carrying the humanized task text and the child's answer, with no raw JSON args rendered by default anywhere in the transcript.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the tool-card projector suite, the shell view suite, and the Electron smoke tool-card assertions.",
      },
      {
        contractId: "openagents_desktop.chat.interactive_question_cards.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "chat transcript agent-question cards",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "make the question UI too. Why not? proper effect native primitives and add some if needed.",
        authorityBoundary:
          "Question cards render only the bounded typed question_pending payload from the FROZEN additive FableLocalEvent contract and answer only through the typed fableLocal.answerQuestion bridge in the frozen shape (answers: one { question, labels } entry per question, labels an array even for single-select; single-select dispatches on click, multiSelect toggles behind an explicit confirm). A bridge without answerQuestion renders read-only pending — the card never invents answer authority. Outcomes (answered/timeout/denied) come from question_resolved and render as dim resolved states; never raw JSON, never a SYSTEM label. Option rows compose catalog primitives (Button label + caption Text description); no local one-off primitives.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/renderer/local-harness.ts",
          "apps/openagents-desktop/src/fable-local-contract.ts",
          "apps/openagents-desktop/src/chat-contract.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "question_cards.intent_loop_and_bridge",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Drives the real intent registry with a fake typed bridge: single-select option click dispatches the answer immediately in the frozen shape, multiSelect toggles then confirms, timeout/denied render dim resolved states, and an absent bridge renders disabled read-only options that dispatch nothing.",
          },
          {
            id: "question_cards.event_projection",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/local-harness.test.ts",
            description:
              "Proves question_pending projects an interactive question note into the streaming transcript and question_resolved updates the same note in place with the runtime-authoritative outcome.",
          },
          {
            id: "question_cards.smoke",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron journey renders the fixture question as an interactive card (header chip, option labels, dim description, no SYSTEM label, no raw JSON), clicks an option through the REAL typed answerQuestion IPC, and proves the runtime's typed rejection reverts the card to honest pending with the selection retained.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the shell question-card suite, the local-harness projection suite, and the Electron smoke question-card step.",
      },
      {
        contractId: "openagents_desktop.chat.opencode_card_design_language.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "chat transcript card design language",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "Make a design pass through the projects/repos/opencode desktop app. any of its tool/message card formatting, we should port its tailwind stuff to our Effect Native, i want our component slooking just like theirs but adapted to our starcraft blue etc, and using the openai apps sdk icons we are.",
        authorityBoundary:
          "This is a presentation port only: opencode's card anatomy and density translate into typed Effect Native style objects on the shared tokens vocabulary — never Tailwind class strings (owner decision 2026-07-08), never local one-off primitives, never a light theme, and always our existing catalog icon set (apps-sdk-ui lineage), our Protoss-blue tokens, and our typed intents. No opencode code is vendored; the port provenance is recorded in-repo (docs/design-ports.md) with spec receipts.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/renderer/tool-cards.ts",
          "apps/openagents-desktop/docs/design-ports.md",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "opencode_card_design.structural_anatomy",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Asserts the opencode-derived structural properties on our cards: tool cards render the header icon + title + status-chip anatomy in that order, detail/result lines stack beneath, raw output is collapsed by default (no raw JSON in the default rendering), and styling is typed token style objects (no class strings anywhere in the card views).",
          },
          {
            id: "opencode_card_design.smoke",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke asserts the rendered tool cards carry the ported anatomy (one card per invocation, humanized header, no raw JSON default) in the uniform Protoss-blue theme.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the shell card suites and the Electron smoke; the design-port provenance note records the opencode source receipts.",
      },
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
        state: "retired",
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
          "RETIRED 2026-07-11, superseded by v2 same day: (a) the owner full-access override replaced the read-only child sandbox with danger-full-access (owner sign-off, verbatim: 'disallowing bash is retarded, give them full tools full permissions etc'); (b) the live EP250 rotation miss (the SHORT auth variant 'Your access token could not be refreshed. Please log out and sign in again.' carried none of v1's markers, so no rotation happened) broadened the auth classifier, added typed pre-content rotation, and added the in-process account health ordering. Kept for history.",
        seam: {
          client: "apps/openagents-desktop/src/renderer/fleet-workspace.ts",
          server: "apps/openagents-desktop/src/codex-child-runtime.ts",
        },
        evidenceRefs: [
          "contract:openagents_desktop.seam.codex_delegation_no_substitution.v2",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "codex_delegation.child_runtime_rotation_and_usage",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/codex-child-runtime.test.ts",
            description:
              "HISTORICAL (retired state skips coverage): proved the v1 read-only spawn recipe and marker-set rotation. Replaced by the v2 oracles below.",
          },
        ],
        verification:
          "Retired — superseded by openagents_desktop.seam.codex_delegation_no_substitution.v2; see that contract's verification.",
      },
      {
        contractId: "openagents_desktop.seam.codex_delegation_no_substitution.v2",
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
          "The local Fable lane may delegate bounded tasks to Codex sub-agents only through the typed mcp__codex__delegate tool: every child is requested pinned to model gpt-5.6-sol at medium reasoning effort as spawn-config truth (labeled requested; the codex exec --json stream does not echo model or effort); children run with the owner-local danger-full-access profile in isolated per-child scratch workspaces on registry-isolated Codex account homes, never the default ~/.codex, and the tool description tells Fable the child STARTS in an empty scratch directory so absolute paths must be included for anything it should read; a failing account is never silently skipped — auth-class failures (broadened marker set including the live SHORT variant 'Your access token could not be refreshed. Please log out and sign in again.') rotate with a typed account_reconnect_required event and demote the account in the in-process health memory, any other pre-content failure rotates with a typed pre_content_failure_rotated event, post-content failures and timeouts fail the child without rotation; candidate ordering per call is last-known-good first, then untried, then auth-failed last (a success clears the mark); when every account is exhausted the delegation returns a typed failure naming the reconnect need (all-auth) or the failure mix; at most 3 children run concurrently and 6 per turn, with over-cap calls refused typed before any spawn; exact per-child token usage from turn.completed (total = input + output + reasoning) rolls into the session usage ledger and the Fleet view's evidence-labeled Session usage section; and a session-observed auth failure or failed usage probe supersedes the registry's presence-based ready with a typed reconnect-required readiness state.",
        authorityBoundary:
          "The renderer receives only bounded typed child lifecycle events (childRef, account ref, public-safe summaries with the child workspace redacted, exact token counts, typed failure reasons/rotation activities) and the typed session-ledger snapshot — never prompts, raw JSONL, credentials, auth paths, or local paths beyond the <child-workspace> label. danger-full-access is the owner-local executor invariant (never a public wire field, never for untrusted labor/provider work). The health memory is in-process only (main-process lifetime, never persisted), and the spawn-config model pin is not presented as a provider echo.",
        seam: {
          client: "apps/openagents-desktop/src/renderer/fleet-workspace.ts",
          server: "apps/openagents-desktop/src/codex-child-runtime.ts",
        },
        evidenceRefs: [
          "apps/openagents-desktop/src/codex-child-contract.ts",
          "apps/openagents-desktop/src/fable-local-runtime.ts",
          "apps/openagents-desktop/src/usage-ledger-contract.ts",
          "contract:openagents_desktop.chat.fable_local_owner_full_access.v1",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "codex_delegation.child_runtime_rotation_and_usage",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/codex-child-runtime.test.ts",
            description:
              "Drives the real JSONL parser: danger-full-access spawn recipe with pinned model/effort and isolated CODEX_HOME; exact usage totals; the verbatim SHORT auth variant classifying auth-class and rotating typed; generic pre-content failure rotating with pre_content_failure_rotated; post-content failure staying terminal; health ordering (last-good first, auth-failed demoted for the NEXT call, success clears); typed all-exhausted failures for the all-auth and mixed cases; host-side timeout; and concurrent isolated children.",
          },
          {
            id: "codex_delegation.fable_tool_caps_and_events",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/fable-local-runtime.test.ts",
            description:
              "Proves the delegate tool is auto-allowed under its fully-qualified name with the empty-scratch/absolute-paths guidance in its description, per-turn concurrency and total caps refuse typed without spawning, child lifecycle events (including both typed rotation activities) flow schema-valid through the FableLocalEvent envelope, and the tool result labels usage as requested spawn-config truth.",
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
      // -----------------------------------------------------------------------
      // EP250 local Fable lane permissions (#8712). HISTORY: the scoped-write
      // contract below bound the lane to workspace-contained Write/Edit with
      // no Bash/WebSearch. SUPERSEDED same-day by the owner full-access
      // override (next contract) — owner sign-off, verbatim: "disallowing
      // bash is retarded, give them full tools full permissions etc". Repo
      // law requires owner sign-off to weaken an oracle; that statement is
      // the sign-off, and the scoped-write oracles were REPLACED by the
      // full-access oracles rather than silently deleted.
      // -----------------------------------------------------------------------
      {
        contractId: "openagents_desktop.chat.fable_local_lane_scoped_write.v1",
        state: "retired",
        surface: "openagents-desktop",
        productArea: "chat local Fable lane permissions",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "Writes are permitted only inside the turn/thread workspace; interactive-only tools are never offered to the model in this headless lane; out-of-scope denials say so honestly. The lane must never surface permission copy implying a grant flow exists (live incident: Write greetings.md failed with 'Claude requested permissions to write to <workspace>/greetings.md, but you haven't granted it yet.' — a grant no UI could give).",
        authorityBoundary:
          "RETIRED 2026-07-11, superseded by contract:openagents_desktop.chat.fable_local_owner_full_access.v1 on the owner's explicit sign-off ('disallowing bash is retarded, give them full tools full permissions etc'). The workspace boundary guard, out-of-scope denial copy, and Bash/WebSearch removal no longer apply; the honest-copy law (never imply a grant flow that does not exist) survives trivially because nothing is denied anymore. Kept for history.",
        evidenceRefs: [
          "apps/openagents-desktop/src/fable-local-runtime.ts",
          "contract:openagents_desktop.chat.fable_local_owner_full_access.v1",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "fable_local_scoped_write.boundary_guard",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/fable-local-runtime.test.ts",
            description:
              "HISTORICAL (retired state skips coverage): proved the PreToolUse guard, out-of-scope denial copy, and restricted tool set. Replaced by fable_local_owner_full_access oracles asserting the inverse posture.",
          },
        ],
        verification:
          "Retired — superseded by openagents_desktop.chat.fable_local_owner_full_access.v1; see that contract's verification.",
      },
      {
        contractId: "openagents_desktop.chat.fable_local_owner_full_access.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "chat local Fable lane permissions",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-codex-session", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "disallowing bash is retarded, give them full tools full permissions etc (owner override after live Claude subagents failed with 'Bash is not available in this lane.'; supersedes openagents_desktop.chat.fable_local_lane_scoped_write.v1)",
        authorityBoundary:
          "OWNER-LOCAL danger profile only — the same owner-local executor invariant as the Khala->Pylon runbook's danger-full-access/approval-never posture, never a public wire field and never applied to untrusted labor/provider work. The local Fable lane and its Agent children get the full SDK toolset (no allowedTools restriction beyond the delegate auto-allow, no PreToolUse workspace guard, no out-of-scope denial copy) with permissionMode 'default' plus an allow-all canUseTool — deliberately NOT bypassPermissions, which per sdk.d.ts bypasses all permission checks including the canUseTool handler the AskUserQuestion flow parks on. Skill and EnterPlanMode/ExitPlanMode stay disallowed as separately-decided UX noise (not permission bounds). Codex delegate children spawn with -s danger-full-access. The per-thread scratch workspace remains the default cwd only.",
        evidenceRefs: [
          "apps/openagents-desktop/src/fable-local-runtime.ts",
          "apps/openagents-desktop/src/codex-child-runtime.ts",
          "contract:openagents_desktop.chat.fable_local_lane_scoped_write.v1",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "fable_local_owner_full_access.full_toolset_and_question_flow",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/fable-local-runtime.test.ts",
            description:
              "Proves session options carry NO allowedTools restriction (only the delegate auto-allow when offered), NO PreToolUse hook, permissionMode 'default'; canUseTool allows Bash/out-of-workspace Write/WebSearch/Agent with no denial or scope copy; NotebookEdit is offered while Skill/plan-mode stay disallowed; and the AskUserQuestion regression: with the allow-all handler the question still parks pending and resolves through answerQuestion.",
          },
          {
            id: "fable_local_owner_full_access.codex_child_danger_sandbox",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/codex-child-runtime.test.ts",
            description:
              "Proves the codex exec spawn recipe carries -s danger-full-access (owner-local profile) while keeping the isolated CODEX_HOME and per-child scratch cwd.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the fable-local runtime suite (full-toolset posture, allow-all canUseTool, question-flow regression) and the codex-child suite (danger-full-access spawn args).",
      },
      {
        contractId: "openagents_desktop.chat.fable_local_question_flow.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "chat local Fable lane questions",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "make the question UI too. Why not? proper effect native primitives and add some if needed. (AskUserQuestion must be a real affordance: on camera it surfaced as 'AskUserQuestion · failed · Answer questions?' with no way to answer.)",
        authorityBoundary:
          "This contract covers the runtime/IPC half: AskUserQuestion parks on the SDK canUseTool callback, emits a bounded path-redacted question_pending event (questionRef stable per invocation; question/header/options/multiSelect), and resolves with the user's answers via the answer-question invoke channel as canUseTool allow + updatedInput answers keyed by original question text (multi-select comma-separated — the SDK's documented mechanism). No answer within the window resolves a graceful typed deny with outcome timeout; turn interrupt/failure/dispose resolves outcome denied; unknown or settled questionRefs and unmatched answers are typed rejections that never throw and never burn a pending question. Multiple pending questions settle independently without deadlock. The renderer supplies only schema-checked answers; no tool execution, filesystem, or session authority crosses the channel.",
        evidenceRefs: [
          "apps/openagents-desktop/src/fable-local-runtime.ts",
          "apps/openagents-desktop/src/fable-local-contract.ts",
          "apps/openagents-desktop/src/main.ts",
          "apps/openagents-desktop/src/preload.cts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "fable_local_question_flow.answer_roundtrip",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/fable-local-runtime.test.ts",
            description:
              "Drives the real canUseTool path: question_pending with bounded questions, typed rejections for unknown/wrong-turn/unmatched answers, allow with updatedInput answers keyed by original question text, multiSelect comma-joined, timeout outcome with the turn continuing, interrupt outcome denied, and malformed input denied without parking a question.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the fable-local runtime suite covering the full question flow (answered, timeout, denied, typed rejections, multiSelect).",
      },
      {
        contractId: "openagents_desktop.coding_catalog.restart_safe_navigation.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "projects and coding sessions",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "khala-code-session",
          statedBy: "owner",
          statedOn: "2026-07-11",
        },
        statement:
          "OpenAgents Desktop persists stable project, repository, worktree, coding-session, tab, route, and typed focus refs across restart. Adding a workspace creates or resumes one canonical session; recent-first active, recovery, and archived filters use typed actions; duplicate opens collapse; missing worktrees and archived sessions recover explicitly; pointer and keyboard activation share the intent registry; and the project home never treats a local path or renderer tab as authority.",
        authorityBoundary:
          "Signed-out catalog rows live only under scope.device_local in the host-owned Sync SQLite local_entities table. Raw filesystem bindings live in a separate mode-0600 main-process file and never cross preload or enter canonical post-images. Hosted server projection and confirmed reads accept only user/team scopes. The renderer receives a bounded public-safe projection and can invoke only fixed schema-decoded choose/open/archive/recover actions.",
        evidenceRefs: [
          "packages/khala-sync/src/coding-session.ts",
          "apps/openagents-desktop/src/desktop-coding-catalog.ts",
          "apps/openagents-desktop/src/coding-catalog-contract.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "docs/sol/2026-07-11-cut-13-canonical-coding-session-catalog-receipt.md",
          "github:OpenAgentsInc/openagents#8693",
        ],
        oracles: [
          {
            id: "coding_catalog.host_restart_and_recovery",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/desktop-coding-catalog.test.ts",
            description:
              "Proves private path separation, stable refs across host-service reopen, duplicate-open collapse, recent sort, typed focus, archive selection, missing-worktree recovery, and owner-private file modes.",
          },
          {
            id: "coding_catalog.effect_native_action_equivalence",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Proves Project Home renders the bounded catalog without nested cards and choose/filter/open/archive dispatch through the same schema-checked Effect Native registry used by keyboard activation.",
          },
          {
            id: "coding_catalog.built_reload_restoration",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built Electron fixture writes through real local SQLite/private binding services, opens the Project Home with the stable current session, reloads the renderer, and observes the same restored catalog authority and selection.",
          },
        ],
        verification:
          "Focused catalog/shell/boundary tests, shared/server/client CUT-13 suites, Desktop typecheck and full test, production build, and OPENAGENTS_DESKTOP_SMOKE=1 Electron smoke.",
      },
      {
        contractId: "openagents_desktop.chrome.apps_sdk_chrome_design_language.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "application chrome design language",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "do a separate design pass of projects/repos/apps-sdk-ui and thats what i want to use for the rest of the app chrome, menus, etc, everything other than messages, but still harmonized to messages. we want that design language, ported to starcraft kinda, represented in EVERY other surface of the app",
        authorityBoundary:
          "Presentation only. The apps-sdk-ui chrome language (alpha-overlay state engine — hover/active/selected as translucent overlays of one base color, never new hues; elevation = lighter surface + hairline ring for floating overlays; 150/350/200ms motion; the trimmed 4-step control lattice; the three-level dim ladder) is expressed as new @effect-native/tokens roles/groups (upstream, public-safe), the vendored DOM renderer chrome base ruleset, typed token style objects in the renderer views, and a host stylesheet that resolves every color through --en-* custom properties. Our icon set stays; uniform Protoss-blue dark theme only; no light theme, no caution/discovery intents, no pink family, no 24px composer radius, no backdrop-blur popover variant, no 9-step lattice, none of their icons (deviations recorded in docs/design-ports.md). Message/tool cards keep the OpenCode geometry, harmonized onto the same shared scales.",
        evidenceRefs: [
          "apps/openagents.com/packages/effect-native-tokens/src/index.ts",
          "apps/openagents.com/packages/effect-native-render-dom/src/index.ts",
          "apps/openagents-desktop/src/renderer/theme.ts",
          "apps/openagents-desktop/src/renderer/app.css",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/docs/design-ports.md",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "chrome_design.token_closure_and_scale_membership",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/design-conformance.test.ts",
            description:
              "Mechanical conformance: (a) zero raw hex/rgb/hsl color literals in renderer modules and the host stylesheet outside the theme module; (b) spacing/radius/type style values are members of the shared token scales with a small documented numeric-dimension allowlist; (c) per-surface structural recipes — sidebar rail sections, palette on surfaceOverlay+borderSubtle+xl with chord captions, composer radius cap + recessed segmented harness track, settings panel padding/hairline, fleet chrome, inspector rail scale, tool-card shimmer keys, 240px raw wells, context-group anatomy.",
          },
          {
            id: "chrome_design.theme_is_khala_canonical",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "The desktop theme IS the tokens-package khalaTheme (radius 2/4/6/8 quantized scale, state-overlay + dim-ladder + overlay-surface roles, motion/control groups) — app-local palette drift deleted.",
          },
          {
            id: "chrome_design.smoke_pixels",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke drives every restyled surface (shell+sidebar, palette, settings, fleet, inspector, composer) and captures pixel receipts when OPENAGENTS_DESKTOP_SMOKE_SHOTS is set.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the design-conformance sweep, the shell/theme suites, and the Electron smoke over the restyled surfaces.",
      },
      {
        contractId: "openagents_desktop.chat.new_chat_autofocuses_composer.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "chat composer focus",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement: "when i do new chat, clicking button or command N, auto focus the input.",
        authorityBoundary:
          "Focus behavior only. Every DesktopNewChat entry point — the workspace-new-chat dock button, the command palette chat.new row, and the Cmd+N/Ctrl+N chord (the canonical chat.new default binding, newly wired as a window keydown following the existing platform-modifier shortcut pattern; no Electron menu or OS accelerator collision exists) — dispatches the SAME typed intent, and the composer input receives focus AFTER the chat view mounts (retry across render commits, because a New chat from a loaded history page swaps the center view and a re-parented input loses focus). No new dispatch authority; the chord is suppressed inside editables like the other global shortcuts.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/boot.ts",
          "apps/openagents-desktop/src/desktop-command-contract.ts",
          "apps/openagents-desktop/src/renderer/command-registry.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "new_chat_focus.palette_chord_caption",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/design-conformance.test.ts",
            description:
              "The palette chat.new row surfaces its canonical chord caption (⌘N on darwin) from the single command registry entry — one registered command, all input paths dispatch the same intent.",
          },
          {
            id: "new_chat_focus.smoke_button_and_chord",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke proves BOTH paths end focused: the dock New-chat click from a loaded history page yields a fresh empty transcript with document.activeElement === the composer input, and a synthesized platform-modifier+N keydown from the fleet workspace does the same.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the palette-registry assertions and the Electron smoke new-chat + cmd-n focus steps.",
      },
      {
        contractId: "openagents_desktop.chrome.disabled_control_reason_popover.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "disabled-control affordances",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "i can't tell why the Codex option is disabled in the composer. for things like that you need to put a popover on hover over the disabled button explaining why.",
        authorityBoundary:
          "Presentation only, hover/focus only. A disabled control that carries a reason string is wrapped in the catalog Tooltip: pointer hover or keyboard focus reveals the reason as a small overlay on the shared overlay recipe (surfaceOverlay fill, overlay shadow + hairline ring, caption text, fast fade); leave/blur dismisses it. The popover reads whatever reason string the control state carries — never hardcoded copy — so a future lane that lights the Codex chip changes nothing here. The accessible label keeps carrying the reason for screen readers, and NO standing caption returns (openagents_desktop.chat.no_composer_disabled_caption.v1 stays intact: the bubble is [hidden] at rest and excluded from visible-text checks). Applied to the composer harness chips, the unavailable-lane Send button, and the settings Reconnect control while another device-auth flow is live.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/renderer/settings.ts",
          "apps/openagents-desktop/src/renderer/app.css",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "disabled_reason_popover.typed_wrapping",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/design-conformance.test.ts",
            description:
              "A disabled harness chip with a reason is wrapped in a Tooltip whose content equals the exact lane reason; the unavailable-lane Send button gets the same wrap; available controls render bare (no popover, no caption).",
          },
          {
            id: "disabled_reason_popover.smoke_hover_reveal",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke proves the disabled Codex chip's popover is hidden at rest, matches the accessible reason exactly, reveals on pointerenter, and dismisses on pointerleave — while the standing-caption ban assertion keeps passing on visible text.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the popover unit assertions and the Electron smoke hover-reveal step.",
      },
      {
        contractId: "openagents_desktop.window.fullscreen_hotkey.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "window controls",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement: "add a hotkey for maximizing (command+something) to fullscreen like command f",
        authorityBoundary:
          "Window presentation only. Cmd+F/Ctrl+F is the canonical window.fullscreen_toggle default binding dispatching DesktopFullscreenToggled through the closed command registry; the shell handler calls the window host seam, and main toggles the sender BrowserWindow's fullscreen state. Deliberately no editable-guard (no find-in-page exists yet; rebind review when find lands). No renderer window-handle authority.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/boot.ts",
          "apps/openagents-desktop/src/desktop-command-contract.ts",
          "apps/openagents-desktop/src/window-contract.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "fullscreen_hotkey.contract_binding",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "The command contract carries window.fullscreen_toggle with Meta+F/Control+F defaults bound to DesktopFullscreenToggled, and dispatching the intent through the registry invokes the injected window host toggle exactly once.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the shell registry dispatch assertion.",
      },
      {
        contractId: "openagents_desktop.shell.no_sidebar_brand_row.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "sidebar chrome",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement: "remove the \"OpenAgents\" with icon top left ins idebar",
        authorityBoundary:
          "Presentation only: the sidebar renders no brand row (icon + product-name text) above the workspace dock; window identity remains in the native title bar and app metadata.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/shell.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "no_sidebar_brand.absence",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "shellSidebar's rendered view contains no sidebar-brand or sidebar-brand-icon nodes and no literal 'OpenAgents' text node in the sidebar tree.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the sidebar absence assertion.",
      },
      {
        contractId: "openagents_desktop.chat.codex_first_class_local_lane.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "composer Codex local chat lane",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement: "yeah i need codex and claude both first class",
        authorityBoundary:
          "The Codex chip in local mode runs a REAL bounded `codex exec --json` turn on the pylon registry's isolated Codex homes — never the default ~/.codex, never the cloud gateway. The lane reuses the frozen fable-local event envelope so codex turns render through the exact same transcript cards (reasoning lines, tool cards, markdown assistant body, usage/metadata inspector facts). The chat lane persists codex sessions (no --ephemeral) so threads resume via `codex exec resume <thread_id>` on the SAME account only; a rotated account falls back to bounded-history prepend. Delegate children keep --ephemeral. No renderer authority is widened: the bridge carries only bounded, redacted typed events.",
        evidenceRefs: [
          "apps/openagents-desktop/src/codex-local-runtime.ts",
          "apps/openagents-desktop/src/codex-local-contract.ts",
          "apps/openagents-desktop/src/renderer/local-harness.ts",
          "apps/openagents-desktop/src/main.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "codex_first_class.lane_runtime",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/codex-local-runtime.test.ts",
            description:
              "Drives the REAL JSONL parser with scripted codex exec streams: the receipted no-ephemeral spawn recipe, exec-resume continuation on the same account (sandbox via -c, prompt = new message only), bounded-history fallback after rotation, full event mapping (reasoning/Bash cards/deltas/exact usage), typed visible rotation, interrupt, and typed all-revoked/no-account failures.",
          },
          {
            id: "codex_first_class.smoke",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke selects the Codex chip (lit only after the fixture preflight verifies an account) and streams a scripted codex exec turn through the real parser, IPC bridge, thread persistence, and renderer: reasoning line, Bash tool card, markdown assistant body, the 'Codex · gpt-5.6-sol (requested)' caption, no ASSISTANT label, composer re-enabled.",
          },
          {
            id: "codex_first_class.live_proof",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/live-proof.ts",
            description:
              "The EP250 live-proof driver's codex-chip and codex-turn steps pass with a verified account: a real gpt-5.6-sol turn streamed in the transcript with mid-stream capture, journaled honestly when no account verifies.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the codex-local runtime suite and the Electron smoke codex-local step; OPENAGENTS_DESKTOP_LIVE_PROOF=1 exercises the real lane.",
      },
      {
        contractId: "openagents_desktop.reliability.codex_connection_signature_corpus.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "Codex connection reliability regression corpus",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "add thorough fucking tests or whatever to prevent this category of codex connection error PLEASE I HATE ALL THEF UCKING SPEEDBUMPS HERE",
        authorityBoundary:
          "The corpus is table-driven over checked-in verbatim failure fixtures (live-captured where available): LONG revoked, SHORT auth variant, 401 token_invalidated, refresh_token_invalidated, missing auth.json, malformed auth.json, quota/429, network-refused, timeout. Every row asserts classification (auth/rate-limit/generic), rotation behavior, health-map effect, the UI-facing reason string, and the fleet readiness projection — one new signature is ONE new row. The preflight prober (a real minimal read-only `codex exec` turn per account; `codex login status` is receipted presence-only and never a probe) runs on boot, fleet Refresh, reconnect completion, and lazily before first dispatch, and its session-scoped results supersede presence-based ready state.",
        evidenceRefs: [
          "apps/openagents-desktop/src/codex-connection-signatures.test.ts",
          "apps/openagents-desktop/src/codex-preflight.ts",
          "apps/openagents-desktop/src/codex-child-runtime.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "codex_signature_corpus.table",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/codex-connection-signatures.test.ts",
            description:
              "One row per failure signature through the REAL parser/classifier/rotation path, asserting all five row dimensions, plus the chip lifecycle state machine (boot-probe to verified to enabled; revoke-mid-session demotes while another verified account keeps the chip; a reconnect probe clears; none-verified disables with the reconnect reason).",
          },
          {
            id: "codex_signature_corpus.preflight",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/codex-preflight.test.ts",
            description:
              "Proves the receipted minimal probe recipe, verified/reconnect/rate-limit/missing/failed classification, the credentials_missing no-spawn fast path, the host-side timeout bound, health + ledger feeds, and ensureProbed session-cache semantics.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the signature corpus and preflight suites in the normal sweep; the live-proof journey journals the real per-account probe round as step 0.",
      },
      {
        contractId: "openagents_desktop.seam.codex_local_lane_no_substitution.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "Codex local lane substitution law",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "Selecting Codex routes to the local Codex lane only: the requested model and reasoning effort are spawn-config truth (gpt-5.6-sol, medium — the exec stream echoes nothing back), every projected model string is labeled (requested), account rotation is typed and visible in the transcript, and no send is ever silently rerouted to another account, lane, or model.",
        authorityBoundary:
          "When no PROBE-VERIFIED Codex account exists, Send refuses with the chip reason and the message goes nowhere. Rotation is bounded by the registry, announced per skip via typed lane_notice events, and post-content failures are terminal (a partial reply never double-runs). The ledger and message metadata record the requested model as spawn-config truth, never as a provider echo.",
        seam: {
          client: "apps/openagents-desktop/src/renderer/local-harness.ts",
          server: "apps/openagents-desktop/src/codex-local-runtime.ts",
        },
        evidenceRefs: [
          "apps/openagents-desktop/src/codex-local-runtime.ts",
          "apps/openagents-desktop/src/renderer/local-harness.ts",
          "apps/openagents-desktop/src/codex-local-contract.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "codex_local_no_substitution.runtime",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/codex-local-runtime.test.ts",
            description:
              "Asserts the pinned -m/-c spawn args, the (requested) model caption event, typed visible rotation with health demotion, terminal post-content failures, and typed refusals that name that no other lane was used.",
          },
          {
            id: "codex_local_no_substitution.renderer_refusal",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/local-harness.test.ts",
            description:
              "A codex send without verified availability is an explicit typed refusal that never reaches the legacy gateway or the fable bridge.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the codex-local runtime and local-harness suites in the normal sweep.",
      },
      {
        contractId: "openagents_desktop.chat.codex_chip_verified_evidence.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "composer Codex chip evidence gating",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "The composer Codex chip is enabled only when a PROBE-VERIFIED Codex account exists this session; registry auth.json presence is never validity. Unavailable reads Codex — no verified account · Reconnect in Settings; when the only obstacle is quota it reads Codex — accounts rate-limited · retry later or connect another account (reconnecting never restores quota); while the session probe is still running it reads Codex — verifying accounts…; the shell never blocks first mount on the probe round.",
        authorityBoundary:
          "Verification evidence is a real bounded minimal `codex exec` probe turn per account (read-only sandbox, ~30s host bound), session-scoped with observedAt, re-run on boot/fleet-Refresh/reconnect-completion and lazily before first dispatch. Probe results feed the shared account-health ordering (verified first), the fleet readiness projection via the ledger typed reconnectRequired flag (probe evidence supersedes presence-based ready; a fresh verified probe clears a stale flag), and the chip state. The reason string lives in the chip state for the chrome disabled-reason popover to render; enabling the chip grants no new authority beyond the existing typed start channel.",
        evidenceRefs: [
          "apps/openagents-desktop/src/codex-preflight.ts",
          "apps/openagents-desktop/src/codex-local-contract.ts",
          "apps/openagents-desktop/src/renderer/boot.ts",
          "apps/openagents-desktop/src/usage-ledger.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "codex_chip_verified_evidence.lifecycle",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/codex-connection-signatures.test.ts",
            description:
              "The chip lifecycle state machine: pending probe renders verifying disabled; verified enables; revoke-mid-session keeps the chip on the other verified account; a reconnect probe clears; none verified disables with the reconnect reason.",
          },
          {
            id: "codex_chip_verified_evidence.smoke",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke asserts the codex chip stays disabled (with its popover reason) until the gated fixture preflight verifies an account, then lights for the streamed codex-local turn.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the lifecycle suite and the Electron smoke chip assertions; the live-proof journal records the real per-account probe round.",
      },
      {
        contractId: "openagents_desktop.chat.composer_shift_tab_harness_toggle.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "composer keyboard harness toggle",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "i want shift+tab to togle between modes in composer (fable / codex) in this case",
        authorityBoundary:
          "The gesture exists only while the composer input has focus: Shift+Tab there toggles the selected harness both directions through the SAME typed DesktopHarnessSelected intent the chips dispatch, with preventDefault so focus never moves. Shift+Tab anywhere else keeps normal reverse focus navigation, and plain Tab is untouched. Toggling TO an unavailable lane is allowed — selection moves and the disabled-reason popover / evidence-gated Send explain why it cannot act; the gesture is never silently blocked and grants no send authority.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/composer-shortcuts.ts",
          "apps/openagents-desktop/src/renderer/boot.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "composer_shift_tab_toggle.handler",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/composer-shortcuts.test.ts",
            description:
              "Proves focused-composer Shift+Tab toggles both directions and preventDefaults, focus-elsewhere and plain Tab are untouched, already-consumed events are left alone, and availability is never consulted (unavailable-lane toggles allowed).",
          },
          {
            id: "composer_shift_tab_toggle.smoke",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke synthesizes Shift+Tab on the focused composer input, asserts the segmented selection flips to codex (a disabled lane) and back with dispatchEvent reporting preventDefault, and that Shift+Tab on a non-composer target neither toggles nor gets consumed.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the composer-shortcuts suite and the Electron smoke composer-gestures step.",
      },
      {
        contractId: "openagents_desktop.chat.composer_icon_only_send.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "composer send control",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "airplane icon in composer OUTSIDE of the button is stupid. put it in , remove text 'send'",
        authorityBoundary:
          "The composer renders exactly ONE send control: the catalog IconButton with the paper-plane glyph inside (width = height per the icon-only rule), solid accent intent and control-lattice radius via typed style tokens, and no Send text label anywhere; the freestanding icon node is removed. The accessible name stays (Send message, or the disabled reason), the disabled state keeps the disabled-reason popover wrapper, and the control dispatches the same DesktopNoteSubmitted intent — no authority change.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/shell.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "composer_icon_only_send.view",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Proves the composer renders exactly one send control as an IconButton (icon Plane, accessibilityLabel Send message, no label text), the freestanding shell-send-icon node is gone, disabled/pending states hold, and the intent ref stays DesktopNoteSubmitted.",
          },
          {
            id: "composer_icon_only_send.smoke",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke asserts the rendered send control carries the icon variant with an inline glyph and aria-label, exactly one send control exists, and no visible Send text or freestanding icon remains in the composer.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the shell composer suite and the Electron smoke composer-gestures step.",
      },
      {
        contractId: "openagents_desktop.sidebar.connected_accounts_usage_box.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "sidebar connected-accounts usage box",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "in the left sidebar, in a bottom box, like letting the chats flex up but show up to 5 connected accounts with a progress bar showing remaining weekly/hourly usage (grayed out if we dont have that data).",
        authorityBoundary:
          "Read-only presentation over the existing fleet accounts projection and its per-account usage entries: the box never probes providers, adds no polling loop, and mutates nothing. The bar renders a MEASURED remaining value only when a decoded usage entry carries real provider rate-limit windows (pylon truth.provider.snapshots, codex-rs RateLimitSnapshot 5h/weekly lineage); every other account renders the grayed borderSubtle track with zero fill and the honest reason ('no usage-window data for this provider') in the tooltip and accessible label — never a fake bar. At most five accounts render (ready first, then provider, then ref); overflow is a dim '+N more' row deep-linking to the Fleet workspace through the existing DesktopWorkspaceSelected intent. Zero connected accounts render no box.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/sidebar-accounts.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/provider-accounts.ts",
          "apps/openagents-desktop/src/provider-accounts-contract.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "sidebar_accounts_box.view_projection",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/sidebar-accounts.test.ts",
            description:
              "Proves the five-account cap with the '+N more' Fleet deep-link row, ready-first ordering, measured bar math (Meter value = tightest window remainingPercent / 100 with all windows in the accessible label), the grayed no-data bar (zero fill, reduced opacity, borderSubtle track, honest tooltip/aria reason), and the absent box at zero accounts.",
          },
          {
            id: "sidebar_accounts_box.pinned_structure",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Proves the sidebar column keeps the chats list flexing (NavRail flex:1/minHeight:0) while the accounts box renders as the LAST sidebar child (pinned bottom, hairline-topped) when accounts exist, and does not render at all when none do.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the sidebar-accounts view suite, the shell pinning assertions, and the design-conformance token oracle over the new module.",
      },
      {
        contractId: "openagents_desktop.history.markdown_prose_rendering.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "codex history workspace message prose",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "i see assistant messages showing raw markdown what the fuck. need to use the same markdown renderer we use elsewhere",
        authorityBoundary:
          "Presentation only. Assistant, user, and agent-message prose in the history workspace (center transcript AND the right-rail item inspector) renders through the SAME bounded markdown projector chat assistant bodies use (renderer markdown.ts -> catalog Markdown/CodeBlock/Divider) — no second parser, links stay safe text with the path visible, no navigation from historical content. Loss-accounted completeness (#8674/#8675) is untouched: every retained source item still projects exactly once (prose OR event row), gap/redaction notices stay plain styled text outside the markdown projection, and the completeness equation counts exactly what it counted before.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/history-workspace.ts",
          "apps/openagents-desktop/src/renderer/markdown.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "history_markdown.prose_projection",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/history-workspace.test.ts",
            description:
              "An assistant item carrying headings/bold/links/inline+fenced code renders typed Markdown/CodeBlock views (h3 + strong + code present, no literal ### or ** in text nodes, link label plus visible path as safe text); user and agent-message prose route through the same projector; gap/redaction notices stay plain event rows; the inspector body uses the projector for prose and plain text for notices; rendered-once accounting holds.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the history workspace suite plus the codex-history completeness suites.",
      },
      {
        contractId: "openagents_desktop.history.agent_roster_shortcut_traversal.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "codex history workspace keyboard navigation",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "just like command up and down scrolsl thru chats, have command shift up and down go up and down the agents of a convo.",
        authorityBoundary:
          "Cmd+Shift+Up/Down (Ctrl+Shift off-macOS) moves the selection through the SAME visible agent roster the right-rail Agents tree renders, dispatching the SAME typed HistoryAgentSelected intent the tree rows dispatch — no parallel selection path. Ends clamp exactly like the unshifted conversation shortcut; the unshifted chord keeps traversing conversations; no-op when no history conversation is open or the roster is empty; editable targets are never intercepted.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/history-workspace.ts",
          "apps/openagents-desktop/src/renderer/boot.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "history_agent_traversal.roster_walk",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/history-workspace.test.ts",
            description:
              "Traversal walks the exact visible roster (collapsed subtrees skipped) both directions, clamps at both ends, no-ops without an open page or roster, targets resolve to the same HistoryAgentSelected intent the tree rows carry, and the shifted chord discriminates from the unshifted conversation chord per platform.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the history workspace traversal suite.",
      },
      {
        contractId: "openagents_desktop.history.humanized_tool_cards.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "codex history workspace tool cards",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "spawn agent card is still showing a fucking json object in the card tool thing, not good",
        authorityBoundary:
          "Display only. Historical tool_call rows humanize through the SAME table chat tool cards use (renderer tool-cards.ts humanizeToolInvocation — never a second table): spawn_agent shows 'Spawn agent' with the task name and a dim fork-turns meta; terminal/file/search/web items show their command, path, or query; unknown items show a prettified name plus a bounded key:value summary. Raw JSON never renders as the default card body, and opaque base64-class continuation/message blobs NEVER appear in any card body — the raw input (blob included) stays reachable only through the item inspector, bounded as before. Loss-accounting completeness is untouched.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/tool-cards.ts",
          "apps/openagents-desktop/src/renderer/history-workspace.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "history_tool_cards.humanized_no_blob",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/history-workspace.test.ts",
            description:
              "A spawn_agent fixture with a base64-class continuation blob renders 'Spawn agent — issue_audit · fork turns: 3' with no blob and no braces in the card detail while the inspector still exposes the raw input; table-driven cases cover exec/shell/web_search/read_file/grep/apply_patch/mcp and the prettified unknown fallback.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the history workspace and tool-card humanization suites.",
      },
      {
        contractId: "openagents_desktop.history.bottom_anchored_autoload.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "codex history workspace pagination",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "you need to show the most recent messages, starting at bottom, and auto load them as i scroll up, smartly loading before the cursor",
        authorityBoundary:
          "Presentation and fetch-order only. A history conversation opens at its END (tail window, newest items visible, scrolled to bottom); the Previous/Next pager is gone. Scrolling up auto-loads the previous page ~1.5 viewports before the top edge and preserves the reader's scroll anchor by the exact prepended height; scrolling down auto-loads newer pages. A thin textFaint loading row / position caption marks the fetching edge. Overlapping fetches never double-count an item. Loss-accounted completeness (#8674/#8675) is untouched: source/rendered/redactions/gaps and totalItems stay whole-conversation truth as the loaded WINDOW changes — only fetch order changed. Restoring a saved item selection reopens the window around that item and scrolls to it; otherwise restore opens at the end.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/history-workspace.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/renderer/boot.ts",
          "apps/openagents-desktop/src/renderer/history-restore.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "history_autoload.window_math",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/history-workspace.test.ts",
            description:
              "Tail offset opens at the last page; scroll-up merge prepends older items and moves the offset back with no dupes; scroll-down appends newer items; prefetch predicates fire ~1.5 viewports before each edge and only while idle; prepend preserves the scroll anchor by the growth; no pager renders and the loading edge shows a thin textFaint row/caption; the restore plan reopens a saved item's window or opens at the end; completeness stays whole-conversation as the window grows.",
          },
          {
            id: "history_autoload.smoke_scroll",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "WRITTEN, pending owner (movie): the built-Electron trace acceptance opens a conversation at its tail (bottom-anchored, no pager), asserts scrolling to the top auto-prepends the previous window with the scroll anchor preserved and the position caption advancing, discovers tool/handoff items against the same tail window the UI renders, and the reload driver restores the saved window. NOT executed this session.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the windowed-loading unit suite; the Electron smoke bottom-anchored + prefetch steps are written but were not executed this session (owner watching a movie) — the coordinator runs the visual/smoke gate on integration.",
      },
    ],
  };
