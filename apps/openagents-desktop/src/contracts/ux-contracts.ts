import {
  BehaviorContractSchemaVersion,
  type BehaviorContractRegistryDocument,
} from "@openagentsinc/behavior-contracts";

export const openAgentsDesktopUxContractRegistry: BehaviorContractRegistryDocument =
  {
    schemaVersion: BehaviorContractSchemaVersion,
    version: "2026-07-14.3",
    contracts: [
      {
        contractId: "openagents_desktop.mvp.visible_surface_allowlist.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "MVP visible workroom surface",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-screenshot-review", statedBy: "owner", statedOn: "2026-07-13" },
        statement:
          "remove from the interface everything not in the MVP spec.",
        authorityBoundary:
          "The ProductSpec Scope and User Experience sections, plus the owner's subsequent explicit AssuranceSpec document-format visualization direction and the owner-issued MAINT-1 harness-maintenance surface (#8785: per-harness version/channel truth with a one-click binary update in Settings), are the visible-surface allowlist. UX-4 (#8790) reconciles the rendered composition to it: the sidebar dock is exactly New chat, Chat, ProductSpec, AssuranceSpec, Project home, and Settings (the machine-checkable list with per-item spec citations lives in apps/openagents-desktop/src/renderer/mvp-visible-surfaces.ts). Bounded files and read-only Git review stay reachable through their closed CW-AC-12 command identities (palette, native Commands menu, ⌘K, deep link), not through dock icons, because the spec places file/Git review beside the conversation. The review surface renders no Git mutation affordance (no commit, push, stage/unstage, discard, branch switch/create, or issue/PR authoring), and the Files browser renders no file create/rename/delete/reveal affordance (CW-AC-14 forbids exposing filesystem or Git mutation authority). Fleet, provider/account selection, OpenAgents account linking, MCP/plugin configuration, Terminal/Inbox, model/reasoning selection, image attachment, and voice controls remain absent from dock, sidebar, composer, Settings, command palette, and native Commands menu. Internal post-MVP substrates do not authorize visible affordances.",
        evidenceRefs: [
          "docs/mvp/openagents-codex-workroom-mvp.product-spec.md",
          "apps/openagents-desktop/src/desktop-command-contract.ts",
          "apps/openagents-desktop/src/renderer/mvp-visible-surfaces.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/renderer/settings.ts",
          "apps/openagents-desktop/src/main.ts",
          "github:OpenAgentsInc/openagents#8756",
          "github:OpenAgentsInc/openagents#8790",
        ],
        oracles: [
          {
            id: "mvp_surface.shell_allowlist",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Proves the dock contains the MVP affordances plus the explicitly owner-directed AssuranceSpec document inspector, while the shell rejects Fleet, accounts, provider/model/reasoning selection, attachments, and voice.",
          },
          {
            id: "mvp_surface.rendered_composition",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/mvp-visible-surfaces.test.ts",
            description:
              "Walks the ACTUAL rendered shell tree for every reachable workspace state, requires the dock to equal the cited allowlist exactly (no additions, no silent losses), forbids every non-MVP surface key, and proves the oracle rejects a planted non-MVP surface.",
          },
          {
            id: "mvp_surface.settings_allowlist",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/settings.test.ts",
            description:
              "Proves Settings contains current-Codex-session truth but no OpenAgents account, Pylon, MCP, plugin, or extension-lifecycle controls.",
          },
          {
            id: "mvp_surface.command_allowlist",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/desktop-command-contract.test.ts",
            description:
              "Proves Fleet, Terminal, and Inbox are absent from schema-decoded palette, deep-link, shortcut, and native-menu command authority.",
          },
          {
            id: "mvp_surface.read_only_review_boundary",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/git-panel.test.ts",
            description:
              "Proves the review workspace renders no commit/push/stage/discard/branch/issue/PR affordance even when the substrate state carries them.",
          },
        ],
        verification:
          "Desktop typecheck, shell/settings/command/composition suites, build, and built-host smoke enforce the ProductSpec-visible allowlist against the actual rendered dock and screens.",
      },
      {
        contractId: "openagents_desktop.mvp.visible_surface_sweep.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "MVP visible workroom surface",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-screenshot-review", statedBy: "owner", statedOn: "2026-07-14" },
        statement:
          "This menu, when I click the settings button, looks horrible. This folder thing looks horrible. I thought we made a pass removing all screens that are not specifically called for in the MVP. You need to clean all this up and make a pass to remove everything from the sidebar and all UI that's not specifically called for in our MVP spec.",
        authorityBoundary:
          "UX-4 (#8790): the sidebar dock composition is mechanically enforced against the rendered view tree by the mvp-visible-surfaces oracle — Files and the command palette lose their dock icons (their CW-AC-12 command identities remain the entry points), the Git review panel and Files browser drop every mutation affordance to the CW-AC-14 read-only boundary, and the retained Settings, palette, and Files surfaces are design-passed on the shared tokens (one centered settings column on the raised-panel recipe, family-grouped palette rows with keycap chords, quiet grant-boundary presentation). No copy changed; styling, layout, and composition only. Removal is conservative: a swept surface returns only with an explicit spec citation.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/mvp-visible-surfaces.ts",
          "apps/openagents-desktop/src/renderer/mvp-visible-surfaces.test.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/renderer/git-panel.ts",
          "apps/openagents-desktop/src/renderer/workspace-browser.ts",
          "apps/openagents-desktop/src/renderer/app.css",
          "github:OpenAgentsInc/openagents#8790",
        ],
        oracles: [
          {
            id: "mvp_sweep.composition_oracle_with_falsifier",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/mvp-visible-surfaces.test.ts",
            description:
              "Renders every reachable workspace through desktopShellView, asserts zero visible-surface violations, and proves planted non-MVP dock items, removed affordances, forbidden screen keys, and silent allowlist shrink each FAIL the oracle.",
          },
          {
            id: "mvp_sweep.built_host_smoke",
            kind: "visual-smoke",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke journey asserts the exact rendered dock ids, the absence of the swept dock icons and Git mutation controls, and captures the pixel receipts for the cleaned sidebar and each retained screen.",
          },
        ],
        verification:
          "Desktop typecheck, the composition suite with falsifiers, design conformance, build, and built-host smoke.",
      },
      {
        contractId: "openagents_desktop.assurance_spec.document_visualization.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "AssuranceSpec document support",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-directive", statedBy: "owner", statedOn: "2026-07-13" },
        statement:
          "we'll want to be able to open different files and support different formats. And so just think about like we are adding support for the assurance spec format now. What do we want that file to look like? ... I want to see a beautiful visualization of Assurance Spec in the app",
        authorityBoundary:
          "The Desktop build parses the exact checked-in .assurance-spec.md artifact through the browser-safe package grammar and embeds only its bounded presentation snapshot; future editor-opened source uses the same app-owned projection boundary. Structural validity, criterion mapping, and repository candidates are presentation facts only: the view cannot admit work, execute checks, verify evidence, waive obligations, release software, or change public promises. Invalid bytes replace the document visualization with an explicit invalid state, and no filesystem or repository authority is added to the renderer.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/assurance-spec-workspace.ts",
          "apps/openagents-desktop/src/renderer/assurance-spec-workspace.test.ts",
          "packages/assurance-spec/src/browser.ts",
          "docs/mvp/openagents-codex-workroom-mvp.assurance-spec.md",
        ],
        oracles: [
          {
            id: "assurance_spec.document_is_source_driven_and_authority_honest",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/assurance-spec-workspace.test.ts",
            description:
              "Parses the checked-in MVP AssuranceSpec, proves all 18 obligations and their incomplete proof-design fields render, proves invalid bytes fail closed, and proves the view exposes no execution or verification actions.",
          },
        ],
        verification:
          "Desktop typecheck, renderer unit tests, design conformance, and production build enforce the source-driven Effect Native document visualization and proposal-only authority boundary.",
      },
      {
        contractId: "openagents_desktop.mvp.uses_logged_in_codex_session.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "MVP Codex runtime and Settings",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-directive", statedBy: "owner", statedOn: "2026-07-13" },
        statement:
          "this must work with the user's logged in Codex account. This whole idea of linking to pylons does not belong in this desktop application. The MVP needs to be, it uses your logged in Codex session, nothing else. Simplify this all right now.",
        authorityBoundary:
          "The local Desktop MVP admits exactly the ordinary current Codex session discovered at ~/.codex and launches the package-owned Codex app-server with inherited CODEX_HOME removed. Named Pylon accounts, account rotation, isolated device-auth, and Pylon account rows are not eligible for or rendered by the MVP workroom. The app-owned ProductSpec skill remains digest-pinned under the signed application resources and is registered as an explicit app-server extra root; it is not copied into ~/.codex. Fleet-only account custody remains outside this local-workroom contract. No credential bytes or home paths cross preload or renderer.",
        evidenceRefs: [
          "apps/openagents-desktop/src/codex-local-runtime.ts",
          "apps/openagents-desktop/src/codex-preflight.ts",
          "apps/openagents-desktop/src/main.ts",
          "apps/openagents-desktop/src/renderer/settings.ts",
          "apps/openagents-desktop/src/mvp-proof.ts",
          "github:OpenAgentsInc/openagents#8756",
        ],
        oracles: [
          {
            id: "mvp_codex_session.current_only_app_server",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/codex-local-runtime.test.ts",
            description:
              "Offers both the ordinary session and a Pylon account, then proves the production app-server selects only the ordinary session and completes the native interaction round trip.",
          },
          {
            id: "mvp_codex_session.no_linking_surface",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/settings.test.ts",
            description:
              "Proves Settings explains current-session reuse and renders no Codex/Claude Pylon account rows, connect action, reconnect action, or device-auth status.",
          },
          {
            id: "mvp_codex_session.app_owned_skill_root",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/builtin-productspec-skill.test.ts",
            description:
              "Pins ProductSpec skill registration to the signed app-owned extra root while allowing the current Codex session and forbidding default-home skill mutation.",
          },
        ],
        verification:
          "Desktop typecheck and the Codex local-runtime, preflight, Settings, built-in skill, MVP-proof, package, and built-host suites enforce the current-session-only MVP boundary.",
      },
      {
        contractId: "openagents_desktop.settings.reachable_workspace.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "Settings navigation and scrolling",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-screenshot-review", statedBy: "owner", statedOn: "2026-07-13" },
        statement:
          "when I toggle like that settings button or whatever that rightmost button in the sidebar is, it'll open and collapse the Command K bar. That doesn't make any fucking sense. I can't scroll down in the settings page.",
        authorityBoundary:
          "The sidebar dock wraps instead of clipping its final controls and keeps the typed Settings action last, so the visible rightmost gear dispatches only DesktopSettingsToggled; Command-K remains a separate typed command. The Settings workspace is the bounded vertical scroll owner and keeps every control reachable without changing body-level viewport policy. No renderer path, credential, provider payload, or broader runtime authority is introduced.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/renderer/app.css",
          "github:OpenAgentsInc/openagents#8756",
        ],
        oracles: [
          {
            id: "settings_navigation.rightmost_action_is_settings_only",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Proves Settings is the final dock item and its real typed intent opens and closes Settings without opening the Command-K palette.",
          },
          {
            id: "settings_navigation.workspace_is_scroll_owner",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/design-conformance.test.ts",
            description:
              "Proves the token-conformant Settings workspace owns bounded vertical overflow while the host body remains fixed.",
          },
        ],
        verification:
          "Desktop shell and design-conformance suites plus typecheck/build enforce the distinct navigation intents and scroll ownership.",
      },
      {
        contractId: "openagents_desktop.chat.launch_directory_is_default_cwd.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "local coding workspace",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-review", statedBy: "owner", statedOn: "2026-07-12" },
        statement:
          "the sessions are saved to this fucking app support shit ... needs to go into the current directory where the app was started from by default, and later configurable in a dir",
        authorityBoundary:
          "Electron main captures process.cwd() at module launch and supplies that exact directory as the top-level Claude and Codex coding cwd. The provider runtimes may not silently substitute an Application Support per-thread directory. The cwd crosses through an explicit host getter so a later persisted directory picker can replace the launch default without changing either provider runtime. Smoke/live-proof runs remain isolated under test userData; probes, account custody, and delegated child scratch work are unchanged. This default cwd does not grant the renderer path-selection authority.",
        evidenceRefs: [
          "apps/openagents-desktop/src/main.ts",
          "apps/openagents-desktop/src/fable-local-runtime.ts",
          "apps/openagents-desktop/src/codex-local-runtime.ts",
        ],
        oracles: [
          {
            id: "local_workspace.claude_exact_host_root",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/fable-local-runtime.test.ts",
            description: "Proves an explicit host workspace root becomes the exact Claude SDK cwd.",
          },
          {
            id: "local_workspace.codex_exact_host_root",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/codex-local-runtime.test.ts",
            description: "Proves the same explicit host workspace root becomes Codex's process cwd and -C argument.",
          },
        ],
        verification:
          "Desktop typecheck/build plus the Fable and Codex local-runtime suites enforce exact host-root propagation while retaining isolated fallback coverage.",
      },
      {
        contractId: "openagents_desktop.chat.codex_turns_do_not_time_out.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "Codex local turn lifecycle",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-12" },
        statement: "WHAT THE FUCK IS THIS TIMEOUT --- FIX IT",
        authorityBoundary:
          "A production top-level local Codex turn has no host wall-clock deadline: long or temporarily quiet work remains alive until the Codex process completes, fails, or the owner dispatches the existing typed Stop intent. Elapsed time alone never sends SIGTERM and never fabricates a timeout/provider-unavailable state. A deadline remains dependency-injectable only for deterministic unit coverage of the typed failure path; it is not wired by Electron main and grants no renderer or provider authority.",
        evidenceRefs: [
          "apps/openagents-desktop/src/codex-local-runtime.ts",
          "apps/openagents-desktop/src/main.ts",
        ],
        oracles: [
          {
            id: "codex_turn_lifecycle.no_production_deadline",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/codex-local-runtime.test.ts",
            description:
              "Runs a non-closing Codex process with production defaults, proves it remains pending, then proves the existing exact-turn interrupt still terminates it as interrupted rather than timed out.",
          },
        ],
        verification:
          "bun test apps/openagents-desktop/src/codex-local-runtime.test.ts plus Desktop typecheck and build enforce the no-default-deadline lifecycle and explicit Stop authority.",
      },
      {
        contractId: "openagents_desktop.chat.provider_event_interleaving.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "chat transcript event ordering",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-12" },
        statement:
          "the assistant message there at end is out of order like the tool calls appeared BEFORE that, i need everything interleaved sequentially in the order we get it. fix it and push",
        authorityBoundary:
          "The renderer and durable local thread store project display-bearing provider events in their exact arrival order. Consecutive text deltas coalesce into one assistant segment only until a non-text event arrives; that boundary closes the segment, the event is inserted next, and later text opens a new assistant segment after it. A tool result updates its matching invocation card in place at the invocation's original position. Final usage/model metadata may enrich the last assistant segment through a keyed in-place upsert but may never append or move that segment past intervening tool, model, reasoning, or lane events. No event gains new renderer, filesystem, provider, or persistence authority.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/local-harness.ts",
          "apps/openagents-desktop/src/main.ts",
          "apps/openagents-desktop/src/thread-store.ts",
        ],
        oracles: [
          {
            id: "provider_event_interleaving.live_projection",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/local-harness.test.ts",
            description:
              "Streams model, assistant text, tool use, tool result, then more assistant text and proves the transcript notes retain that exact relative sequence with two correctly attributed assistant segments.",
          },
          {
            id: "provider_event_interleaving.durable_upsert_position",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/thread-store.test.ts",
            description:
              "Proves final assistant metadata/text enrichment replaces the exact keyed durable note in place without moving it after a later tool note.",
          },
        ],
        verification:
          "bun test apps/openagents-desktop/src/renderer/local-harness.test.ts apps/openagents-desktop/src/renderer/shell.test.ts apps/openagents-desktop/src/thread-store.test.ts plus Desktop typecheck and build cover live projection, tool-card folding, durable keyed replacement, and host integration.",
      },
      {
        contractId: "openagents_desktop.chrome.command_notice_is_transient_toast.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "command notice chrome",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "what is that yellow command request shit at top, is that supposed to be there, fix if not",
        authorityBoundary:
          "The command notice (a duplicate/unavailable deferred-command rejection or a keybinding-save failure) presentation changes ONLY: from a permanent full-width top-edge caption banner that never cleared until the next successful command, to a compact, floated, token-styled warn TOAST that auto-dismisses on a bounded (~4.5s) Effect-scheduled clear and is dismissible immediately via a typed intent (× / click). The auto-clear is a forked Effect fiber (never a leaked raw setTimeout); a new notice cancels any prior pending clear; and the mount registers the controller's shutdown as a scope finalizer so a pending clear can never fire after unmount. The underlying rejection behavior is unchanged (CUT-15): the command is still rejected/ignored; only the notice is now transient, not permanent. No raw colors/px enter the renderer (apps-sdk chrome tokens + the design-conformance oracle), and the render stays commit-idempotent (the keyed toast is not re-parented on unrelated re-renders, so its enter animation never replays).",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/command-notice.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/renderer/boot.ts",
          "apps/openagents-desktop/src/renderer/app.css",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "command_notice.transient_controller_and_toast",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/command-notice.test.ts",
            description:
              "Drives the controller under Effect's TestClock (not wall-clock): a notice auto-clears exactly on the bounded delay boundary, a new notice cancels the prior pending clear (no early/double dismiss), the dismiss intent clears immediately and kills the pending timer, the view renders a warn Toast carrying the DesktopCommandNoticeDismissed intent (and nothing when clear), and the real intent registry clears the notice on that dismiss intent.",
          },
          {
            id: "command_notice.transient_rejection_smoke",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke's command-duplicate-visible-rejection step asserts the duplicate command's rejection notice APPEARS as a dismissible toast (data-en-role=toast with a dismiss control) AND then auto-dismisses on its own bounded timer, rather than persisting as a permanent banner.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the command-notice controller/toast suite, the shell view suite, and the Electron smoke command-duplicate-visible-rejection step asserting appear-then-auto-dismiss.",
      },
      {
        contractId: "openagents_desktop.chat.details_affordance_visibility_is_pointer_only.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "chat transcript chrome",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "the 'details' thing under message, when i hover it or its visible or whatever, it flashes back in every time i type something in the input, WHY??? WHY IS IT CONNECTED TO ANOTHER COMPONENT - fix it - and ensure that category of error wont happen anywhere else in codebase",
        authorityBoundary:
          "The per-message details affordance is a hover/focus reveal (opacity-0 at rest). Its visibility must be a pure function of pointer/focus ONLY — never of composer input, global state, or re-render timing. Root cause: the pure state->View re-render on every keystroke re-parented the persisted keyed affordance in the DOM renderer (Transcript wrappers were rebuilt from scratch and Stack children were unconditionally re-appended via replaceChildren); detaching + re-attaching a node restarts its CSS opacity transition, flashing it visible. Fix is structural, not cosmetic: (1) the resting opacity:0 is keyed on the affordance itself and no longer requires the [data-en-message] ancestor, so a momentary detach cannot expose it; (2) the shared render-dom commit is now idempotent — persisted keyed content is never re-parented on an unrelated re-render, killing the whole transition-replay category (also fixes the tool-title shimmer and disabled-reason popover).",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/app.css",
          "apps/openagents.com/packages/effect-native-render-dom/src/index.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
        ],
        oracles: [
          {
            id: "details_affordance.stable_on_composer_input.smoke",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke types into the composer while NOT hovering the message row and fails if the details affordance's computed opacity ever rises above 0, or if its DOM node is replaced or re-parented (sameNode/sameParent/restingOpacity=0/finalOpacity=0/maxOpacityDuringTyping=0).",
          },
          {
            id: "details_affordance.commit_idempotent_no_reparent",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents.com/packages/effect-native-render-dom/tests/index.test.ts",
            description:
              "Provider-agnostic render-dom guard: re-committing a Transcript whose only change is a sibling (the composer value) performs ZERO DOM moves of the persisted keyed hover-reveal affordance, so its CSS transition can never replay from an unrelated re-render.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the details-affordance-stable-on-composer-input Electron smoke step; bun test at apps/openagents.com/packages/effect-native-render-dom runs the commit-idempotency guard.",
      },
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
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-12" },
        statement: "when i load message details in right sidebar it doesnt scroll down, fix that.",
        authorityBoundary:
          "The inspector projects only bounded per-message metadata persisted by the host. When a new message selection appends details below a taller live-agent graph, the exact right rail becomes its own scroll owner and synchronously reveals a unique keyed marker immediately before the inspector. The reveal runs once per changed target after generic scroll restoration, never moves the transcript viewport, and does not keep pinning after the user scrolls manually. Selection remains a typed intent and grants no runtime, resume, or filesystem authority.",
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
            id: "chat_message_inspector.keyed_scroll_reveal",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents.com/packages/effect-native-render-dom/tests/index.test.ts",
            description:
              "Proves a changed Stack scroll target becomes the exact overflow owner, reveals the keyed details marker after generic position restoration, clears its one-shot marker, and leaves later manual scrolling untouched.",
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
        contractId: "openagents_desktop.chat.composer_stop_button.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "composer turn interruption",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "capability-audit", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "interrupt a running turn from the UI",
        authorityBoundary:
          "While a turn streams (pending), the composer's trailing icon-only Send is replaced by an icon-only Stop that dispatches the DesktopTurnInterrupted intent; the handler signals the active local lane's already-plumbed interrupt IPC path (FableLocal/CodexLocal interrupt channel) by the exact active turnRef and invents no terminal state — the runtime's typed `interrupted` failure is what finalizes the turn and reverts the control to Send. Stop grants no new authority: it cannot start a turn, route to another lane, or fabricate a completion, and a host without a local streaming lane simply no-ops.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/renderer/local-harness.ts",
          "apps/openagents-desktop/src/fable-local-contract.ts",
          "apps/openagents-desktop/src/capability-registry.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "composer_stop_button.render_and_intent_loop",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Proves the composer renders the icon-only Stop (and no Send) while pending and Send (no Stop) while idle, and that dispatching DesktopTurnInterrupted through the real intent registry calls the chat host's interruptActive exactly once when pending and never when idle.",
          },
          {
            id: "composer_stop_button.interrupt_path",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/capability-evals.test.ts",
            description:
              "Drives the local-harness interruptActive seam headlessly against a fake lane bridge: a streaming turn's exact turnRef is signalled on the frozen interrupt channel, and the runtime's typed `interrupted` FableLocalEvent maps to a turn_failed reason that finalizes the turn.",
          },
          {
            id: "composer_stop_button.live_proof_step",
            kind: "visual-smoke",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/live-proof.ts",
            description:
              "The interrupt-stop live-proof step clicks Stop mid-turn in the real Electron window and journals the interrupted transcript state with a PNG receipt (rung-4; executed by the live-proof driver, not the headless sweep).",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the shell Stop-button suite and the capability-evals interrupt-path oracle; the interrupt-stop live-proof step is exercised by the live-proof driver run.",
      },
      {
        contractId: "openagents_desktop.chat.durable_runtime_turn_controls.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "durable runtime turn controls",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "issue-completion-criteria", statedBy: "owner", statedOn: "2026-07-12" },
        statement:
          "Provider questions, permission/tool approvals, plan/review transitions, interrupt, resume, retry, and cancel are first-class typed timeline items.",
        authorityBoundary:
          "CUT-16 (#8696) Desktop slice for the durable Khala runtime path. (1) The composer Stop button now also interrupts a DURABLE turn: the runtime conversation host implements interruptActive over the exact confirmed thread/run this renderer has in flight, dispatching conversation.interrupt through the protocol-v10 gateway with the confirmed run's expectedVersion. The acknowledgement is admission truth only — the confirmed canceled terminal (never the Stop handler) finalizes the turn and reverts the composer; a host with no in-flight durable send returns false and sends nothing. (2) Queue-until-idle now works on the durable path: a mid-turn submit enqueues a text follow-up that is promoted only at the previous turn's CONFIRMED terminal, as a real conversation.append plus conversation.start on the same lane; a refused enqueue restores the cleared draft instead of dropping text. (3) Every control intent (chat Stop and fleet-cockpit pause/cancel/resume/retry/close) carries the EXACT confirmed run lane (claude_code→claude_pylon, codex/opencode_codex→codex_app_server, openagents_native→hosted_khala) as an additive optional gateway field threaded into the shared control-intent builders, because the durable authority's lane fence (runtime_target_lane_mismatch) rejects a mismatched target — the previous hard-coded Codex default made Claude/hosted turn controls unadmittable from Desktop. No schema, migration, server, or intent-contract change: openagents.khala_runtime_control_intent.v1 and protocol v10 are unchanged apart from the additive optional lane field.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/runtime-conversation.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/renderer/fleet-workspace.ts",
          "apps/openagents-desktop/src/runtime-gateway-contract.ts",
          "apps/openagents-desktop/src/main.ts",
          "github:OpenAgentsInc/openagents#8696",
        ],
        oracles: [
          {
            id: "durable_runtime_turn_controls.interrupt_lane_exact",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/runtime-conversation.test.ts",
            description:
              "Proves interruptActive during an in-flight durable send dispatches conversation.interrupt with the exact threadRef/runRef, the lane derived from the confirmed run runtime (claude_code→claude_pylon), and the confirmed run version; returns true only on an admitted outcome; and returns false with no command when no durable send is in flight or the confirmed run is already terminal.",
          },
          {
            id: "durable_runtime_turn_controls.queue_until_idle_confirmed_drain",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/runtime-conversation.test.ts",
            description:
              "Proves a follow-up queued mid-turn is promoted only after the first run's confirmed terminal as a real append + start on the same lane, that the final thread carries both confirmed user messages, and that queueFollowup without an in-flight durable send reports queued:false and sends nothing.",
          },
          {
            id: "durable_runtime_turn_controls.queue_refusal_restores_draft",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Proves the DesktopNoteSubmitted pending branch restores the cleared composer draft when the host reports queued:false, and leaves newer user input untouched.",
          },
          {
            id: "durable_runtime_turn_controls.gateway_lane_passthrough",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/tests/runtime-gateway.e2e.test.ts",
            description:
              "Proves the protocol-v10 gateway decodes the additive optional lane on conversation.interrupt/continue/retry/close, hands it to the runtime command service unchanged, rejects an unknown lane literal as invalid_request, and that main's control adapters thread input.lane into the shared control-intent context instead of the hard-coded Codex default (source oracle).",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the runtime-conversation control suite, the shell queue-refusal suite, and the gateway lane pass-through oracle in the normal sweep.",
      },
      {
        contractId: "openagents_desktop.chat.opencode_composer_shape.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "chat input composer layout",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-directive", statedBy: "owner", statedOn: "2026-07-12" },
        statement:
          "Between codex/claude and reasoning dropdowns we need model selector. Codex select between GPT-5.6 and GPT-5.5, Claude select between Fable, Opus 4.8, Sonnet 5. Also that plus button make it not a huge circle, must be icon only.",
        authorityBoundary:
          "The single OpenCode-shaped composer card keeps its multiline input above one compact action bar. That bar orders compact icon-only Attach, Provider, provider-scoped Model, Codex-only Reasoning, account/permission controls, a flexible spacer, and circular Send/Stop. Exact model IDs are closed typed values and reach the corresponding provider launch field; no model is inferred from its display label and Claude refuses provider substitution before content. Attach uses the shared Effect Native IconButton's `sm` size (32px) with a required accessible label rather than inheriting the generic 44px circular action treatment. No attach, queue, stop, availability, or submission behavior is removed.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/docs/design-ports.md",
          "projects/repos/opencode/packages/app/src/components/prompt-input.tsx",
        ],
        oracles: [
          {
            id: "opencode_composer_shape.structural_layout",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Proves the composer card holds a multiline shell-input above a bar ordered Attach, Provider, Model, Codex Reasoning, spacer, and Send/Stop; model options change with provider, exact selected IDs ride the next send, and Attach is the shared compact icon-only control.",
          },
          {
            id: "opencode_composer_shape.token_conformance",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/design-conformance.test.ts",
            description:
              "The design-conformance oracle holds on the composer: no raw color literals, spacing/radius values stay on shared scales, and Provider, Model, and Reasoning are compact native Select components.",
          },
          {
            id: "opencode_composer_shape.smoke",
            kind: "visual-smoke",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke renders the composer with input above the action bar and fails if the typed controls or trailing Send are missing.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the shell composer-layout suite, the design-conformance oracle, and the Electron smoke; pixel receipts of the empty/text/image/streaming composer states are captured under scratchpad ep250-composer-shots/.",
      },
      {
        contractId: "openagents_desktop.chat.composer_image_input.v1",
        state: "retired",
        surface: "openagents-desktop",
        productArea: "composer image input",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "capability-audit", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "attach a screenshot to a coding turn (image input)",
        authorityBoundary:
          "The composer carries a leading attach affordance plus drag-drop and paste-from-clipboard image attach. Accepted images are PNG/JPEG/WebP/GIF, bounded to at most 8 per message and 10 MB each; oversize or wrong-type files are rejected honestly with transient copy (no standing caption). The renderer holds each attachment as bounded base64 and NEVER reads an arbitrary filesystem path — bytes come only from an in-renderer drop/paste File or a main-mediated native file picker. Attachments thread through the additive fable-local start `images` field to BOTH lanes: Fable sends them as SDK base64 image content blocks in a streaming-input user message (a bare string prompt cannot carry an image), and Codex writes them into the turn workspace and passes `codex exec -i <path>`. Attaching grants no new authority: it starts no turn on its own, routes to no other lane, and reads no file the user did not hand the app.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/composer-images.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/renderer/boot.ts",
          "apps/openagents-desktop/src/fable-local-contract.ts",
          "apps/openagents-desktop/src/fable-local-runtime.ts",
          "apps/openagents-desktop/src/codex-local-runtime.ts",
          "apps/openagents-desktop/src/capability-registry.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "composer_image_input.decode_and_state",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/composer-images.test.ts",
            description:
              "Proves media-type/size classification with honest rejection copy, base64 decoding of an in-renderer File (drop/paste path), the ≤8 count bound, and the boundary projection that drops renderer-only fields.",
          },
          {
            id: "composer_image_input.render_and_intent_loop",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Proves the composer renders the attach affordance and thumbnails with remove, disables attach at the 8-image limit, surfaces the rejection notice, and that add/remove/submit through the real intent registry thread the base64 image into the chat host (including an images-only turn).",
          },
          {
            id: "composer_image_input.fable_sdk_block",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/fable-local-runtime.test.ts",
            description:
              "Drives the real Fable runtime prompt construction: a captured fake query proves an image turn lowers to an AsyncIterable user message whose content carries a { type:\"image\", source:{ type:\"base64\", media_type, data } } block (sdk.d.ts receipt), while a no-image turn keeps the plain string prompt.",
          },
          {
            id: "composer_image_input.codex_image_flag",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/codex-local-runtime.test.ts",
            description:
              "Proves the Codex lane writes each attachment into the turn workspace and passes it as `-i <path>`, terminated by `-C` before the positional prompt so the variadic --image never swallows the prompt.",
          },
          {
            id: "composer_image_input.smoke_step",
            kind: "visual-smoke",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The image-attach smoke step drops a fixture PNG onto the composer in the real Electron renderer, asserts the thumbnail renders, submits, and asserts the assistant reply carries the fixture's image-received marker — proving the image reached the SDK query payload end-to-end.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the composer-images, shell, fable-local-runtime, and codex-local-runtime suites plus the image-attach smoke step; a real live provider image turn is deferred to a live-proof run.",
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
        state: "retired",
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
          "In local (not-signed-in) mode, selecting Claude runs a real streaming turn on this machine with zero login: the default selection prefers the user's currently authenticated local Claude Code session, then falls back to ready isolated sibling Pylon Claude homes; explicitly selecting a named Pylon account pins it. Selecting a harness never routes to the cloud gateway or another provider; an unavailable lane renders a disabled chip with its reason and a Send that does not accept the action.",
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
              "Proves current local Claude session precedence, isolated Pylon fallback discovery, read-only headless SDK options, bounded/redacted event mapping, same-lane account rotation only before content, and that no ready account yields a typed unavailable result with the SDK never loaded.",
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
          "MODEL PIN (user-facing brand is \"Claude\" per owner 2026-07-12, because Fable may not always be the available model; the pinned model is unchanged): the local Claude lane requests model claude-fable-5 with skills removed from the lane (disallowed, never offered-then-denied); if the SDK init reports an effective model outside the claude-fable family the turn fails typed as model_substituted naming requested vs effective, no substituted output is ever streamed or persisted as Claude, and the lane never rotates accounts on a model mismatch; the effective model is emitted as a typed event and displayed with the reply (e.g. Claude · claude-fable-5), never asserted from the brand alone.",
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
              "Proves the renderer displays the SDK-reported effective model as the Claude · <model> transcript caption from the typed model_effective event, positioned with the trace lines the persisted thread also carries.",
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
          "The local Fable lane may delegate bounded tasks to Codex sub-agents only through the typed mcp__codex__delegate tool: every child is requested pinned to model gpt-5.6-sol at medium reasoning effort as spawn-config truth (labeled requested; the codex exec --json stream does not echo model or effort); children run with the owner-local danger-full-access profile in isolated per-child scratch workspaces, preferring the ordinary authenticated local Codex session and using registry-isolated Codex homes as fallback, and the tool description tells Fable the child STARTS in an empty scratch directory so absolute paths must be included for anything it should read; a failing session is never silently skipped — auth-class failures (broadened marker set including the live SHORT variant 'Your access token could not be refreshed. Please log out and sign in again.') rotate with a typed account_reconnect_required event and demote the session in the in-process health memory, any other pre-content failure rotates with a typed pre_content_failure_rotated event, post-content failures and timeouts fail the child without rotation; candidate ordering per call is last-known-good first, then untried, then auth-failed last (a success clears the mark); when every session is exhausted the delegation returns a typed failure naming the reconnect need (all-auth) or the failure mix; at most 3 children run concurrently and 6 per turn, with over-cap calls refused typed before any spawn; exact per-child token usage from turn.completed (total = input + output + reasoning) rolls into the session usage ledger and the Fleet view's evidence-labeled Session usage section; and a session-observed auth failure or failed usage probe supersedes registry presence with a typed reconnect-required readiness state.",
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
          "OpenAgents Desktop presents Runtime Gateway v8's confirmed canonical graph and schema-decoded desktop-local Claude/Fable and Codex graphs as one parent/subagent hierarchy in the chat's right context rail. Pointer activation, keyboard activation, and screen-reader buttons select the same typed agent ref; status, attention, current action, elapsed time, terminal reason, session, runtime, provider, and worktree facts remain inspectable; historical authority is labeled and never gains a live focus control; rapid replacement falls back deterministically and large graphs disclose their bound.",
        authorityBoundary:
          "The renderer accepts only graph post-images already schema-decoded by Runtime Gateway v8 or the frozen preload local-graph bridge and projects both through the shared client model. Agent selection is local inspection/focus state, not execution movement or provider/process authority. Historical projections remain inspectable with canControl=false. No provider payload, history heuristic, credential, path, store/session handle, or transport handle enters the view.",
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
        contractId: "openagents_desktop.chat.new_chat_always_exits_history.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "chat navigation",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-live-review", statedBy: "owner", statedOn: "2026-07-12" },
        statement:
          "New Chat and Command-N must always leave the old chat and open a fresh chat.",
        authorityBoundary:
          "Every New Chat entry point dispatches the same DesktopNewChat intent. Creation is local-first and must not enter live Sync's pending-reconciliation path: the converging chat host creates through the app-owned durable local thread store, pins that ref to local authority, exits any loaded history page, mounts an empty transcript, and focuses the composer. The typed runtime host is attempted only when the local durable bridge cannot create. No fake success is projected without a real thread from one of the two typed durable hosts.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/runtime-conversation.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/renderer/boot.ts",
        ],
        oracles: [
          {
            id: "new_chat_always.local_creation_bypasses_sync_reconciliation",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/runtime-conversation.test.ts",
            description:
              "Proves New Chat creates exactly once through the durable local store, never enters live Sync reconciliation, and pins the resulting thread ref to local authority.",
          },
          {
            id: "new_chat_always.button_and_command_n_exit_history",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke proves both the dock button and platform Command-N leave loaded history, mount a fresh empty transcript, and focus the composer.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the converging-host fallback unit test and both built-Electron New Chat paths.",
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
          "The Codex chip in local mode runs a REAL bounded `codex exec --json` turn, preferring the ordinary authenticated local Codex session and using the pylon registry's isolated Codex homes as fallback — never the cloud gateway. The lane reuses the frozen fable-local event envelope so codex turns render through the exact same transcript cards (reasoning lines, tool cards, markdown assistant body, usage/metadata inspector facts). The chat lane persists codex sessions (no --ephemeral) so threads resume via `codex exec resume <thread_id>` on the SAME account only; a rotated account falls back to bounded-history prepend. Delegate children keep --ephemeral. No renderer authority is widened: the bridge carries only bounded, redacted typed events.",
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
        source: { channel: "owner-video-review", statedBy: "owner", statedOn: "2026-07-12" },
        statement:
          "put a microphone button there left of the submit button. make submit button an up arrow like codex has there. clicking microphone button toggles a voice mode we havent implemented yet. just put a text thing there for like disabled or something",
        authorityBoundary:
          "The composer renders exactly one Submit control as the shared catalog IconButton with ArrowUp inside and no visible Send label. Immediately left is a compact shared Mic IconButton. Mic dispatches only DesktopVoiceModeToggled into renderer presentation state: selected state shows the literal `Voice unavailable` badge and starts no microphone, permission prompt, transcription, network, provider, or device action. Pending turns disable Mic and its handler also refuses programmatic toggles. Submit keeps its accessible name, disabled-reason wrapper, and DesktopNoteSubmitted intent.",
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
              "Proves the composer renders exactly one ArrowUp Submit IconButton, places the compact Mic control immediately before it, toggles the honest Voice unavailable badge through the typed intent, and refuses that toggle while pending.",
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
        state: "retired",
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
      {
        contractId: "openagents_desktop.history.claude_import.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "local coding-history import",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "capability-audit", statedBy: "agent", statedOn: "2026-07-11" },
        statement:
          "From the daily-coding capability audit (docs/fable/2026-07-11-daily-coding-capability-audit.md §4 H3): History import/browse must import ~/.claude alongside ~/.codex — the desktop imported ~/.codex history only, with no ~/.claude importer, even though the window held 2,243 Claude files. OpenAgents Desktop discovers Claude Code parent sessions and subagent/workflow sidechains, reconstructs the same loss-accounted parent/child agent graph (edge = the invoking Agent tool call's structured toolUseResult.agentId, NOT parentUuid), and surfaces Claude sessions in the SAME history catalog as Codex, tagged by source.",
        authorityBoundary:
          "Read-only, owner-local, additive. The Claude importer (claude-history.ts) and the merged surface (merged-history.ts) return only the same schema-bounded, credential-redacted CodexHistory catalog/page projections the Codex importer returns, namespaced `claude:` so page/search routing is unambiguous. Import NEVER changes what counts: each source keeps its whole-conversation completeness equation source = rendered + redactions + gaps, and the ~3% Claude orphan class (a child file with no recoverable structured edge) is shown and counted as an explicit topology gap, never silently hidden. Raw prompts, thinking, tool arguments, command output, credentials, and file contents never reach the renderer beyond the existing bounded/redacted item fields; no filesystem authority, resume, or provider runtime authority enters the renderer.",
        evidenceRefs: [
          "apps/openagents-desktop/src/claude-history.ts",
          "apps/openagents-desktop/src/merged-history.ts",
          "apps/openagents-desktop/src/codex-history-contract.ts",
          "docs/teardowns/2026-07-10-claude-subagents-rendering-analysis.md",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "claude_import.graph_and_loss_accounting",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/claude-history.test.ts",
            description:
              "Reconstructs the Agent-edge graph (source-tagged, namespaced refs), projects rich redacted items, links subagent previews, represents an unlinked child as an explicit orphan/topology gap, pages without overlap/omission, and holds source = rendered + redactions + gaps.",
          },
          {
            id: "claude_import.merged_catalog_and_scale",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/tests/history-search.test.ts",
            description:
              "A mixed Codex+Claude catalog shows both sources tagged and routes pages by ref; the 100MiB/100-child scale oracle (claude-history-performance.e2e.test.ts) keeps discovery bounded and the completeness equation intact; the capability-evals headless oracle drives the merged catalog/page.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the Claude importer unit suite, the merged-catalog + search suite, the scale oracle, and the capability-evals headless H3 oracle.",
      },
      {
        contractId: "openagents_desktop.history.free_text_search.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "local coding-history search",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "capability-audit", statedBy: "agent", statedOn: "2026-07-11" },
        statement:
          "From the daily-coding capability audit (docs/fable/2026-07-11-daily-coding-capability-audit.md §4 H4 / §6 item 11): Session search must go beyond the structured project:/repository:/state: catalog grammar — the daily reality is 'find that conversation where…' across 3,262 session files, and nothing searched transcript content. OpenAgents Desktop adds free-text matching over session TITLES and a bounded per-session CONTENT index across Codex AND Claude sessions, ranked by match then recency, with a search box in the history workspace whose results open the session at the matching item.",
        authorityBoundary:
          "The content index is a REBUILDABLE LOCAL CACHE, never authority (per the audit's History/Discovery/Memory split, indexes are caches). It ranks over titles (always available from the loss-accounted catalog) and a bounded content projection of the most-recent sessions (bounded item cap; first search never blocks on the whole archive; response reports indexedSessions/truncated honestly). Search NEVER mutates catalog/page truth or the completeness equation. A content result carries the exact matching item ref so opening it windows the session on that item, reusing the bottom-anchored restore-to-item flow; a title result opens at the end. Matching is deterministic substring filtering of an owner-local corpus — not ad-hoc intent routing — and surfaces only the same bounded/redacted item text the projector already exposes.",
        evidenceRefs: [
          "apps/openagents-desktop/src/history-search.ts",
          "apps/openagents-desktop/src/merged-history.ts",
          "apps/openagents-desktop/src/renderer/history-workspace.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "free_text_search.ranking_and_open_at_item",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/tests/history-search.test.ts",
            description:
              "Title match, content match with the exact open-at-item ref, cross-source recency ranking, empty/no-match returns nothing, and the index is rebuildable (a fresh build yields identical ranked results); the pure ranking core proves titles outrank content and recency breaks ties.",
          },
          {
            id: "free_text_search.ui",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/history-workspace.test.ts",
            description:
              "The search field renders a query-bound TextField; a non-blank query activates search; result rows carry the source badge and dispatch HistorySearchResultOpened with the threadRef; a content result opens at its matching item while a title result opens at the end.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the search ranking/open-at-item suite, the history-workspace search UI suite, and the capability-evals headless H4 oracle.",
      },
      {
        contractId: "openagents_desktop.chat.fable_local_runtime_capabilities.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "fable-local runtime capability substrate",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "capability-audit",
          statedBy: "agent",
          statedOn: "2026-07-11",
        },
        statement:
          "From the daily-coding capability audit (docs/fable/2026-07-11-daily-coding-capability-audit.md §4/§6): the local Fable lane must surface plan/task progress (J2 plan mode/plan review + J4 task/todo progress — both providers externalize plans constantly, 1,617 update_plan observations, yet the app rendered none of it and plan mode was disallowed); it must be able to steer or stop a running child (G4 steer/message running children — the app could spawn children but could not talk to or stop one); it must let a user queue a follow-up while a turn runs (A3 message queueing during a turn — steer-by-queueing is habitual in both CLIs); and it must load user-configured MCP servers (I2 — ~858 calls across Stripe/Expo/Apollo/docs/design servers with no config surface, only the internal delegate server).",
        authorityBoundary:
          "This contract binds the RUNTIME SUBSTRATE only (typed FableLocalEvent kinds + control channels + programmatic oracles); the renderer that draws these is a separate wave-2 lane. Everything is additive and default-off so current turn behavior is unchanged: plan mode is opt-in (default permissionMode 'default'); TodoWrite (never disallowed) emits plan_updated additionally to its tool_use trace; steerChild interrupts a running Codex delegate child (message is honestly 'unsupported' — codex exec is non-interactive and the SDK Agent tool exposes no per-subagent message API); queueFollowup is queue-until-idle (the single-string-prompt turn cannot inject mid-stream, so the queued message is promoted at the idle boundary, not steered mid-stream); user MCP servers are bounded/validated and merged next to the internal 'codex' server (reserved), and a failed or invalid server emits a typed mcp_server_unavailable without ever crashing the turn. The owner-local full-access posture (no allowedTools restriction, danger-full-access children) is preserved.",
        evidenceRefs: [
          "apps/openagents-desktop/src/fable-local-contract.ts",
          "apps/openagents-desktop/src/fable-local-runtime.ts",
          "docs/fable/2026-07-11-daily-coding-capability-audit.md",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "fable_local_runtime_capabilities.plan_todo",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/fable-local-runtime-caps.test.ts",
            description:
              "A TodoWrite tool call emits plan_updated with mapped {step,status} entries in addition to the raw tool_use; a non-TodoWrite tool never emits plan_updated and unknown todo status coerces to pending; the default turn uses permissionMode 'default' with ExitPlanMode disallowed, and opt-in planMode switches to permissionMode 'plan' and allows ExitPlanMode while Skill/EnterPlanMode stay disallowed.",
          },
          {
            id: "fable_local_runtime_capabilities.child_steer",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/fable-local-runtime-caps.test.ts",
            description:
              "steerChild reaches a running delegate child: interrupt emits child_steered(interrupted) and the turn completes; message is honestly unsupported and a later interrupt still ends the turn; an unknown child or turn mismatch returns not_found with no event; and a whole-turn interrupt also aborts the running child.",
          },
          {
            id: "fable_local_runtime_capabilities.queue_followup",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/fable-local-runtime-caps.test.ts",
            description:
              "queueFollowup during a streaming turn emits followup_queued (with position) and, at turn end, followup_promoted with the queued message, ordered after turn_completed; two queued follow-ups take positions 1 and 2 with only the first promoted per turn end (FIFO); and a queue with no live turn returns no_active_turn and emits nothing.",
          },
          {
            id: "fable_local_runtime_capabilities.user_mcp_servers",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/fable-local-runtime-caps.test.ts",
            description:
              "An enabled stdio/http server is merged into Options.mcpServers and its mcp__name__tool is allow-listed via canUseTool (no delegate auto-allow when no delegate); an SDK-reported failed server and an invalid config (bad name / reserved codex / missing transport field) each emit mcp_server_unavailable while the turn still completes; and the frozen FableLocalMcpServerConfig schema enforces its bounds (cap, transport enum, boolean enabled) with normalization rejecting bad/reserved/duplicate/missing-field entries.",
          },
          {
            id: "fable_local_runtime_capabilities.renderer_surface",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/runtime-cards.test.ts",
            description:
              "WAVE-2 landed for plan/todo, child steer/stop, and the queued-follow-up chip (see openagents_desktop.chat.runtime_capability_cards.v1). The MCP-server settings/status surface (I2) remains a separate wave-2 settings lane.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the fable-local runtime capability suite (src/fable-local-runtime-caps.test.ts) as programmatic oracles; the wave-2 renderer surfaces are proven by runtime-cards.test.ts + the smoke.",
      },
      {
        contractId: "openagents_desktop.chat.runtime_capability_cards.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "chat transcript runtime-capability cards",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "capability-audit",
          statedBy: "agent",
          statedOn: "2026-07-11",
        },
        statement:
          "WAVE-2 renderer for the wave-1 runtime substrate (audit §4/§6 J2/J4 plan-todo progress, G4 steer/stop a running child, A3 queue a follow-up while a turn runs): the desktop transcript must render a compact task-progress checklist that updates in place, let the user Interrupt a running delegate child, retain each delegated child's bounded exact prompt and answer, show that conversation when its card or agent-row is selected, and let the user queue a follow-up while a turn streams.",
        authorityBoundary:
          "Renderer/presentation only over the bounded additive FableLocalEvent stream + control channels — no new authority is granted. child_started carries the exact bounded instruction and child_completed carries the exact bounded answer; the renderer retains both on the keyed child card and projects them as the selected child's user/Codex transcript above secondary runtime metadata. Older persisted cards without transcript remain schema-valid. plan_updated renders ONE compact plan card per turn (a status glyph per entry from the exact pending/in_progress/completed enum, the in_progress row emphasized) that replace-renders in place as new plan_updated events arrive (latest wins). A running delegate child offers a single Interrupt control that dispatches DesktopChildInterruptRequested -> fableLocal.steerChild(action:'interrupt') by exact { turnRef, childRef }; MESSAGE-ing an in-flight child is NOT offered. The composer stays usable while a turn streams; a mid-turn submit calls fableLocal.queueFollowup instead of starting a new turn, renders a 'Queued follow-up (#N)' chip, and clears it on followup_promoted. Token styling only; a host without a local streaming lane simply no-ops.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/runtime-cards.ts",
          "apps/openagents-desktop/src/renderer/local-harness.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "docs/fable/2026-07-11-daily-coding-capability-audit.md",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "runtime_capability_cards.plan_todo",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/runtime-cards.test.ts",
            description:
              "Proves plan_updated projects ONE plan card (keyed) that updates in place as entries change (latest wins), each row carrying the status glyph for the exact pending/in_progress/completed enum with the in_progress row emphasized, and the progress summary counting done/in-progress.",
          },
          {
            id: "runtime_capability_cards.child_steer",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/runtime-cards.test.ts",
            description:
              "Proves a running child card renders an Interrupt control (and NO message control), the control dispatches DesktopChildInterruptRequested with the exact { turnRef, childRef }, the child_steered outcome renders as a compact line, and a completed/failed or already-interrupted child no longer offers Interrupt.",
          },
          {
            id: "runtime_capability_cards.queue_followup",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/runtime-cards.test.ts",
            description:
              "Proves followup_queued renders a 'Queued follow-up (#N)' chip, followup_promoted clears it, the composer stays usable while pending with a queue placeholder, a mid-turn submit routes to queueFollowup (not sendMessage), and the promoted follow-up starts as the next turn.",
          },
          {
            id: "runtime_capability_cards.smoke",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke fable fixture turn scripts a TodoWrite plan_updated and asserts the plan/todo card renders in the transcript with status glyphs (no raw JSON, no SYSTEM label).",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs runtime-cards.test.ts + the local-harness projection suite as the renderer oracles and the Electron smoke plan-card step.",
      },
      {
        contractId: "openagents_desktop.seam.typed_git_github_surface.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "Git/GitHub review surface",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "capability-audit", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "Commit, push, branch, and GitHub issue/PR flows work from OpenAgents Desktop through a TYPED Git/GitHub surface with receipts — not only via an agent's Bash tool. The review workspace has a Git panel: a status header (branch, ahead/behind, dirty state), a changed-files list with stage toggles, a commit box that shows the resulting SHA, a Push button with its pushed-ref receipt and typed failure classes, a branch switcher, and an issues/PRs section that can list and create (returning the item url). (EP250 capability audit §4E, §6.3, ranks E2–E5 commit/push/PR/issue as a daily habit with zero typed UI.)",
        authorityBoundary:
          "The renderer never supplies argv: the host (git-github-host.ts) runs a FIXED, closed operation set over the active canonicalized workspace root, with user strings only reaching git/gh as validated path/ref/message values that cannot be reinterpreted as flags. Every request and result is Effect-Schema decoded on both sides (git-github-contract.ts). Results are public-safe — a commit SHA, a pushed ref, an issue/PR number+url — never tokens, credentials, raw stderr, or absolute paths; failures are typed classes (no_upstream, non_fast_forward, auth_failed, blocked_by_hook, dirty_tree, gh_unavailable, gh_unauthenticated, …). Commit refuses an empty message or nothing-staged; checkout refuses a dirty tree; push applies a bounded fetch→rebase→push retry; gh operations never trigger an auth prompt. This is owner-local executor authority (this is the owner's machine); it adds no untrusted-labor or provider authority.",
        seam: {
          client: "apps/openagents-desktop/src/git-github-contract.ts",
          server: "apps/openagents-desktop/src/git-github-host.ts",
        },
        evidenceRefs: [
          "apps/openagents-desktop/src/git-github-host.ts",
          "apps/openagents-desktop/src/git-github-contract.ts",
          "apps/openagents-desktop/src/renderer/git-panel.ts",
          "apps/openagents-desktop/src/main.ts",
          "apps/openagents-desktop/src/preload.cts",
          "docs/fable/2026-07-11-daily-coding-capability-audit.md",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "git_github_surface.host_real_repo",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/git-github-host.test.ts",
            description:
              "Against a REAL temp git repo (and a REAL local bare remote): status structure, stage/unstage, commit returns the real HEAD SHA, empty-message and nothing-staged refusals, branch list/create/checkout, dirty-tree checkout refusal, push returns the pushed ref/sha and advances the remote, no-upstream refusal, and the non-fast-forward fetch→rebase→push retry (success on non-conflict, typed non_fast_forward after aborting a conflicting rebase). The gh path is boundary-unit-tested plus one guarded real gh --version/auth-status test that skips honestly when gh is unavailable.",
          },
          {
            id: "git_github_surface.contract_both_sides",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/git-github-contract.test.ts",
            description:
              "The closed operation set and every result variant (incl. the typed error) round-trip through decode on both sides; unknown ops and malformed params are rejected and excess keys stripped.",
          },
          {
            id: "git_github_surface.panel_intent_loop",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/git-panel.test.ts",
            description:
              "The panel renders the status header, changed-files stage toggles, commit box, Push control, branch switcher, and issues/PRs section; disabled controls (commit with nothing staged / empty message, Push without upstream, gh Create when gh is unavailable) carry the hover-only Tooltip reason; and the real intent loop stages, commits (showing the SHA receipt), pushes (ref receipt), checks out branches, marks gh unavailable with a reason, and creates an issue (url receipt) — never a fabricated success.",
          },
          {
            id: "git_github_surface.smoke_real_status",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke routes to the review workspace and asserts the typed Git panel rendered REAL read-only status of the app's own repo (status header, commit box, Push, branch switcher, issues/PRs section all present; branch label resolves to a real branch or detached HEAD) without committing or pushing.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the host real-repo suite, the contract both-sides suite, the panel intent-loop suite, and the Electron smoke git-review step.",
      },
      // =====================================================================
      // I2 (#8712 EP250 wave-2) — the MCP-config SETTINGS surface, landing the
      // renderer half that the runtime-capabilities contract left as a planned
      // wave-2 oracle. Realizes audit gap §6.5.
      // =====================================================================
      {
        contractId: "openagents_desktop.settings.mcp_servers.v1",
        state: "retired",
        surface: "openagents-desktop",
        productArea: "settings — user-configured MCP servers",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: {
          channel: "capability-audit",
          statedBy: "agent",
          statedOn: "2026-07-11",
        },
        statement:
          "User-configured MCP servers — ~858 calls across Stripe/Expo/Apollo/docs/design servers. No config surface; only the internal delegate server exists. (missing, I2)",
        authorityBoundary:
          "The Settings screen gains an 'MCP servers' section (apps-sdk chrome + shared token styles only — the design-conformance oracle) that lists configured servers (name, transport chip, enable/disable Toggle, Remove) and an Add form whose stdio/http fields switch on a transport RadioGroup. Client-side validation mirrors the FROZEN FableLocalMcpServerConfig bounds (name charset/length, reserved 'codex', duplicate names, transport-specific required field, arg/env/header value bounds) and shows a single inline error before anything crosses to main. Persistence is a main-process host writing a private JSON file under userData mode 0600; env/header/arg VALUES are user-provided and may be sensitive, so they are persisted and handed to the fable-local runtime's userMcpServers getter (main-only) but are NEVER logged and NEVER cross back to the renderer — the projection carries name/transport/enabled/command/url and arg/env/header COUNTS only. Stored rows are re-validated against the frozen schema on read; invalid rows are dropped and counted, never crashing. A runtime-reported mcp_server_unavailable status renders a warn chip when threaded to settings; until that thread lands the section shows config state. This surface adds no new primitive: it composes the shared catalog TextField/RadioGroup/Toggle/Button/Badge only.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/settings.ts",
          "apps/openagents-desktop/src/mcp-config-host.ts",
          "apps/openagents-desktop/src/mcp-config-contract.ts",
          "apps/openagents-desktop/src/fable-local-contract.ts",
          "docs/fable/2026-07-11-daily-coding-capability-audit.md",
          "github:OpenAgentsInc/openagents#8712",
        ],
        oracles: [
          {
            id: "mcp_servers.settings_render_and_intent_loop",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/settings.test.ts",
            description:
              "The MCP servers section lists configured servers with a transport badge, enable/disable Toggle, and Remove; the Add form shows stdio vs http fields by transport; client-side validation rejects empty/invalid/reserved/duplicate names and missing transport fields with an inline error; and the add/toggle/remove intent loop drives a fake bridge, resets the draft on success, and ignores names not displayed.",
          },
          {
            id: "mcp_servers.persistence_host",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/mcp-config-host.test.ts",
            description:
              "The persistence host round-trips configs to a mode-0600 file, drops-and-counts schema-invalid stored rows without crashing, enforces the reserved/duplicate/transport/list-cap rules on add, and returns a renderer projection that never contains secret env/header values.",
          },
          {
            id: "mcp_servers.smoke",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke opens Settings, asserts the MCP servers section renders, adds a fixture stdio server through the real Add form + typed IPC, and asserts it persists and lists — without spawning a real MCP server (the fable query is a fixture in smoke).",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the settings renderer suite and the persistence-host suite as programmatic/UI oracles, plus the Electron smoke MCP add-and-list step.",
      },
      {
        contractId: "openagents_desktop.terminal.workspace_bounded_pty.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "workspace terminal / execution",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "audit", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "Interactive terminal / stdin steering — the audit's #1 daily-coding gap (8,333 write_stdin observations, capability D3). Ordinary build/test/dev-server work must run in scoped Desktop terminals with an explicit local-preview lifecycle and no renderer ambient process authority. (CUT-20, #8700)",
        authorityBoundary:
          "The renderer holds no shell and no process: every terminal operation is a typed intent (create/input/resize/interrupt/restart/close/preview-open) schema-decoded on both sides of the sandbox. Main alone binds each session to the currently authorized workspace root + a bounded environment; the renderer sends a session ref and, for input/resize, bounded data / integer geometry — never a shell, argv, cwd, or env, so a compromised renderer can steer stdin but never chooses WHAT is spawned or WHERE. Output crossing to the renderer is BOUNDED (a byte-capped ring, loss-accounted with a gap flag) and REDACTED (secret-named/secret-shaped env VALUES and token-shaped literals are scrubbed in main before any chunk is sent). On project/workspace close the OWNED process tree is killed exactly once (SIGTERM then SIGKILL against the process group; a second close is a no-op). A bounded tail persists (mode 0600) and is reloaded as an explicitly recovered, gap-marked session after an app restart. Local preview discovers an EXPLICIT announced port parsed from the session's OWN output (never a port scan), shows readiness, and stops with its owning session; opening it is out-of-process (external browser) behind a confirmation — never arbitrary in-app navigation. The shipped backend is a child-process-group terminal (zero native deps, runs under bun test AND Electron); node-pty pseudo-TTY + xterm.js are a documented TerminalBackend swap deferred to the #8574 packaging lane. The terminal UI (bounded monospace output + a typed input line + interrupt/restart) composes only shared catalog primitives on the design-conformance token scales.",
        evidenceRefs: [
          "apps/openagents-desktop/src/terminal-host.ts",
          "apps/openagents-desktop/src/terminal-contract.ts",
          "apps/openagents-desktop/src/renderer/terminal-workspace.ts",
          "apps/openagents-desktop/src/main.ts",
          "apps/openagents-desktop/src/preload.cts",
          "docs/fable/2026-07-11-daily-coding-capability-audit.md",
          "github:OpenAgentsInc/openagents#8700",
        ],
        oracles: [
          {
            id: "terminal.adversarial_and_built_host_receipt",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/terminal-host.test.ts",
            description:
              "The adversarial PTY suite — shell injection (renderer input never becomes argv; the spawn argv stays fixed and workspace-bound), secret environment (bound secret values redacted in the streamed chunk AND the tail), runaway output (ring buffer holds the byte cap and marks gap), orphan children (closing a session reaps a REAL backgrounded grandchild via process-group kill — the disposal evidence), duplicate start, port collision (typed error on a second live session claiming the same announced port), and revoked grants — plus the built-host receipt (a real /bin/sh runs a stdin command, output captured, exit code observed, tree disposed) and the dev-preview receipt (a real server's announced port is detected + reachable, then freed when the session stops).",
          },
          {
            id: "terminal.renderer_intent_loop",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/terminal-workspace.test.ts",
            description:
              "The terminal workspace transitions (ready/output/exit/preview/closed/error + snapshot recovery) and the typed intent loop through the real registry with a fake bridge: create adds the returned session, submit writes the input line to stdin with a newline and clears the field, interrupt/restart target the active session, and a failed preview-open surfaces a typed notice — all on the design-conformance token scales.",
          },
          {
            id: "terminal.smoke_built_electron_pty_receipt",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke routes to the terminal workspace through the canonical workspace.terminal command, then runs a REAL bounded command through the real preload bridge + real main PTY host (bound to the app's own repo in smoke), asserts the ready + redacted output events, closes the session, and the lifecycle-teardown asserts zero live terminal sessions remain.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the adversarial PTY host suite and the renderer intent-loop suite as programmatic/UI oracles, plus the built-Electron smoke terminal PTY receipt step and its lifecycle-teardown disposal check.",
      },
      {
        contractId: "openagents_desktop.preferences.typed_durable_migratable_schemas.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "app preferences & operability",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "issue", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "Theme, density, font, reduced motion, keybindings, provider defaults, privacy, notifications, and update preferences have typed durable schemas/migrations.",
        authorityBoundary:
          "A single versioned, migratable preferences document (<userData>/preferences.json, mode 0600) owns density, font, reduced-motion, provider-defaults, privacy, notifications, and update preferences. Theme is intentionally NOT a mutable field (the app is the fixed Protoss-blue khalaTheme; recorded, not switchable) and keybindings keep their existing typed store (desktop-command-bindings). Density and font genuinely resize the app through a scaled theme applied at mount; reduced-motion resolves to a root attribute the CSS honors (explicit override wins over the OS). Provider-defaults/privacy/notifications/update-prefs are durable and IPC-round-tripped; each is consumed where a real effect already exists. The migrator is total: a missing, corrupt, partial, legacy, or future-versioned file always resolves to a valid current document and never throws.",
        evidenceRefs: [
          "https://github.com/OpenAgentsInc/openagents/issues/8704",
          "apps/openagents-desktop/src/desktop-preferences-contract.ts",
          "apps/openagents-desktop/src/desktop-preferences-host.ts",
          "apps/openagents-desktop/src/desktop-preferences-effects.ts",
          "apps/openagents-desktop/src/renderer/app.css",
          "apps/openagents-desktop/src/renderer/boot.ts",
        ],
        oracles: [
          {
            id: "preferences.migration_and_host_and_effects",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/desktop-preferences.test.ts",
            description:
              "The migration chain (current/defaults/legacy_v0/merged/downgraded) is total and value-preserving, the host round-trips to a mode-0600 file and self-heals legacy bytes, and the effects module scales the theme (font/density) and maps reduced-motion to the root attribute with defaults returning the identity theme.",
          },
          {
            id: "preferences.durable_ipc_round_trip.smoke",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke round-trips preferences over the real IPC (update density→compact, read back compact, reset→comfortable) in the diagnostics-and-preferences step.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the preferences migration/host/effects suite and the Electron smoke preferences round-trip.",
      },
      {
        contractId: "openagents_desktop.accessibility.core_flows_meet_wcag_aa.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "accessibility",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "issue", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "Desktop/mobile meet keyboard, focus, screen-reader, contrast, dynamic type, target-size, and reduced-motion acceptance for core coding flows.",
        authorityBoundary:
          "This contract binds the DESKTOP surface only. It guarantees: WCAG 2.1 AA text contrast on the theme's primary/secondary/status text roles across all four surfaces; a high-contrast, clearly-visible focus ring (focus token ≈ 7.9:1 on background); reduced-motion honored both via the OS prefers-reduced-motion media query AND an explicit in-app override; and accessible names on every interactive node in the diagnostics/preferences operability surfaces. Disabled text is treated per the WCAG 1.4.3 exemption. Mobile accessibility for core coding flows is a SEPARATE app (apps/openagents-mobile) and is NOT covered by this contract — it remains the named residual on #8704.",
        evidenceRefs: [
          "https://github.com/OpenAgentsInc/openagents/issues/8704",
          "apps/openagents-desktop/tests/accessibility.test.ts",
          "apps/openagents-desktop/src/renderer/app.css",
          "apps/openagents-desktop/docs/2026-07-11-cut24-accessibility-audit.md",
        ],
        oracles: [
          {
            id: "accessibility.contrast_and_reduced_motion",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/accessibility.test.ts",
            description:
              "Computes WCAG relative-luminance contrast for the theme text roles and asserts AA (primary/secondary/status ≥ 4.5:1, faint/accent ≥ 3:1, focus ring high-contrast); asserts app.css honors both the OS prefers-reduced-motion media query and the explicit data-en-reduce-motion override.",
          },
          {
            id: "accessibility.diagnostics_accessible_names",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/diagnostics.test.ts",
            description:
              "Every interactive control in the diagnostics panel carries a non-empty accessible name (Button label), each health row is a labelled group region, and no rendered text leaks a path/url/token.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the accessibility contrast/reduced-motion suite and the diagnostics accessible-name suite. Mobile a11y is tracked separately as the #8704 residual.",
      },
      {
        contractId: "openagents_desktop.notifications.refs_only_and_authoritative_clear.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "notifications & attention",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "issue", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "Notifications carry stable authorized refs and never prompt/code/secrets; attention clears only after authoritative acknowledgement.",
        authorityBoundary:
          "The desktop notification-analog is the confirmed live-agent-graph attention projection (attentionCount / attentionLabel) plus the typed notification preference payload. Attention surfaces only enum-derived labels and public-safe refs (never the underlying question/approval prompt text), and it reflects the newest confirmed graph cursor only — a stale lower-cursor snapshot can neither raise nor clear attention optimistically. The notification preference payload is boolean-only, with no content field a prompt/secret could ride in on. The in-transcript question/approval CARD (which does carry prompt text) is a separate detail surface and is never routed into a notification payload.",
        evidenceRefs: [
          "https://github.com/OpenAgentsInc/openagents/issues/8704",
          "apps/openagents-desktop/tests/notification-attention.test.ts",
          "apps/openagents-desktop/src/renderer/runtime-interactions.ts",
          "apps/openagents-desktop/src/agent-graph-presentation.ts",
        ],
        oracles: [
          {
            id: "notifications.refs_only_authoritative_clear",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/notification-attention.test.ts",
            description:
              "An approval/question attention projects an enum-derived label (not the prompt), the serialized projection carries no prompt/secret text, newestLiveAgentGraph picks the higher-cursor confirmed graph so attention clears only on the authoritative snapshot (and a stale snapshot never overrides it), and the notification preference payload rejects any content string.",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the notification/attention suite; the authoritative-clearing of interactive decisions is additionally exercised by runtime-interactions.test.ts.",
      },
      {
        contractId: "openagents_desktop.diagnostics.watchdog_redacted_export_and_recovery.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "diagnostics & recovery",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "issue", statedBy: "owner", statedOn: "2026-07-11" },
        statement:
          "Diagnostics/watchdog show provider, Runtime Gateway, Sync, workspace, PTY, and extension health with redacted export, restart, and recovery actions.",
        authorityBoundary:
          "The diagnostics panel projects public-safe health for all six domains (provider, Runtime Gateway, Sync, workspace, PTY, extensions). Health rows carry only a bounded domain/level enum, a short public-safe summary, and public-safe refs — never a path, email, prompt, token, or url (structural privacy). The export is ALWAYS redacted before it touches disk (a secret-pattern scrubber runs even if an upstream builder regresses), and the returned notice never carries the saved path. Recovery actions map only to safe typed paths: provider re-probe re-checks accounts, and refresh/refresh_workspace/reload_extensions re-gather fresh sources. PTY health is honestly 'unavailable' until the CUT-20 (#8700) PTY host merges, and restart_runtime/reconnect_sync report 'no recovery action available' until a safe typed restart exists — surfaced honestly, never faked.",
        evidenceRefs: [
          "https://github.com/OpenAgentsInc/openagents/issues/8704",
          "apps/openagents-desktop/src/diagnostics-contract.ts",
          "apps/openagents-desktop/src/diagnostics-report.ts",
          "apps/openagents-desktop/src/diagnostics-host.ts",
          "apps/openagents-desktop/src/renderer/diagnostics.ts",
        ],
        oracles: [
          {
            id: "diagnostics.builder_redaction_and_host",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/diagnostics.test.ts",
            description:
              "The report builder maps each domain to the right level under fault injection (provider outage, closed/unobserved sync, gateway lifecycle/capability degradation, git-unavailable workspace, dropped MCP rows) and never emits a path/token; redaction scrubs a leaked secret and keeps the report schema-valid + export-safe; and the host writes an owner-only redacted bundle whose notice carries no saved path.",
          },
          {
            id: "diagnostics.view_and_handler_loop",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/diagnostics.test.ts",
            description:
              "The panel renders a row + level badge per domain, refresh/export/recovery intents drive a fake bridge (a successful recovery re-gathers), a corrupt gather resolves to unavailable (never a throw), and no rendered text leaks a path/token.",
          },
          {
            id: "diagnostics.renders_and_exports.smoke",
            kind: "bun-test",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "The built-Electron smoke opens Settings, asserts all six diagnostics health rows render with level badges, asserts no rendered diagnostics text is secret-like, clicks Export and asserts a public-safe notice appears (no saved path).",
          },
        ],
        verification:
          "bun run --cwd apps/openagents-desktop verify runs the diagnostics builder/redaction/host suite, the diagnostics view/handler suite, and the Electron smoke diagnostics-and-preferences step.",
      },
      {
        contractId: "openagents_desktop.microinteraction.owner_review_register.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "micro-interaction do/don't register",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-directive", statedBy: "owner", statedOn: "2026-07-13" },
        statement:
          "I want the ones that we have for that for this app to include micro interactions and things that I do and don't want to see. There are some things I don't want to see, such as long streams of text where icons should be.",
        authorityBoundary:
          "This contract is the REGISTER for owner-stated micro-interaction and visual do/don't rules on this app: every future 'things I do and don't want to see' statement lands here as its own versioned contract with a real oracle in the same change (house law 2026-07-03), never conversation-only. The register grants no rendering authority and does not make any individual rule true — each concrete rule (starting with openagents_desktop.microinteraction.icon_slot_no_raw_text.v1 and openagents_desktop.typography.approved_fonts_only.v1) carries its own oracle and its own honest state. AssuranceSpec obligations may cite these contractIds via contract_refs for environment-bound pixel evidence later; this registry stays the single source of the rule text and AssuranceSpec never duplicates it (docs/assurance/UX_CONTRACTS_AND_ASSURANCE.md).",
        evidenceRefs: [
          "contract:openagents_desktop.microinteraction.icon_slot_no_raw_text.v1",
          "contract:openagents_desktop.typography.approved_fonts_only.v1",
          "docs/assurance/UX_CONTRACTS_AND_ASSURANCE.md",
          "apps/openagents-desktop/tests/owner-ux-rules.test.ts",
        ],
        oracles: [
          {
            id: "microinteraction_register.rules_present_and_documented",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/owner-ux-rules.test.ts",
            description:
              "Proves both concrete owner rules exist in this registry as enforced contracts whose oracle refs point at the real enforcing suite, that both owner statements are recorded verbatim, and that docs/assurance/UX_CONTRACTS_AND_ASSURANCE.md exists and names all three contractIds so the next owner rule has a documented home.",
          },
        ],
        verification:
          "bun test apps/openagents-desktop/tests/owner-ux-rules.test.ts runs in the normal desktop sweep; it fails if either concrete rule is removed, renamed, downgraded from enforced, detached from its oracle file, or undocumented in the assurance clarification doc.",
      },
      {
        contractId: "openagents_desktop.microinteraction.icon_slot_no_raw_text.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "icon-slot micro-interactions",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-directive", statedBy: "owner", statedOn: "2026-07-13" },
        statement:
          "I want the ones that we have for that for this app to include micro interactions and things that I do and don't want to see. There are some things I don't want to see, such as long streams of text where icons should be.",
        authorityBoundary:
          "Icon slots are the closed-catalog glyph positions: sidebar dock items, icon-only action controls (IconButton and any icon-carrying node), and status indicator glyphs. Each must resolve a glyph name in the closed @effect-native/core iconNames catalog — never a raw string rendered where the glyph belongs — and dock labels stay bounded single-line micro-copy (24-char bound). Accessible labels are announced, not painted, and stay full-length. ENFORCED SUBSET (honest): the oracle proves the structural rule on the real typed view trees across sampled shell states — every workspace dock item carries a catalog glyph plus bounded single-line label, every icon-carrying node's glyph resolves in the closed catalog, and every IconButton is glyph-plus-accessible-label with no rendered text content — and demonstrates sensitivity against known-bad fixtures. The fully general 'no long text ever appears where an icon was designed' claim over arbitrary rendered pixels is NOT mechanically expressible on typed trees today; it lands later as an AssuranceSpec obligation with technique visual citing this contractId (ASSURANCE_SPEC.md §5), and this contract does not claim that pixel evidence exists yet.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents.com/packages/effect-native-core/src/index.ts",
          "docs/assurance/UX_CONTRACTS_AND_ASSURANCE.md",
        ],
        oracles: [
          {
            id: "icon_slots.closed_catalog_glyphs_and_bounded_labels",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/owner-ux-rules.test.ts",
            description:
              "On the real desktopShellView trees across sampled states: every workspace dock item carries a closed-catalog glyph, a non-empty single-line label within the 24-char micro-copy bound, and an accessible label; every node carrying an icon prop and every Icon node resolves in the closed iconNames catalog; every IconButton carries a glyph and accessible label and no rendered text content. The falsifier test proves the validators reject an unknown glyph, an empty accessible label, rendered IconButton text, and a long-stream dock label.",
          },
        ],
        verification:
          "bun test apps/openagents-desktop/tests/owner-ux-rules.test.ts runs in the normal desktop sweep and enforces the structural subset on real view trees; the residual pixel-level generalization is deferred to an AssuranceSpec visual-technique obligation referencing this contractId, per docs/assurance/UX_CONTRACTS_AND_ASSURANCE.md.",
      },
      {
        contractId: "openagents_desktop.typography.approved_fonts_only.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "typography",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-directive", statedBy: "owner", statedOn: "2026-07-13" },
        statement:
          "I want all that enforced in the assurance pieces. I want to be able to specify rules there. For example, I don't want to see certain things like strings where icons should be, certain fonts, and that must be specified",
        authorityBoundary:
          "The app's approved rendered font truth is exactly the host system stack declared on html/body in src/renderer/app.css (-apple-system, BlinkMacSystemFont, SF Pro Text, Helvetica Neue, sans-serif) plus the generic monospace family for code surfaces (the shared @effect-native/render-dom CodeBlock lowering). The @effect-native/tokens type scale deliberately carries size/weight only — no family tokens — so no renderer module, stylesheet, or typed style object in this app may declare any other family, and the CSS font shorthand stays exactly the form-control 'font: inherit' reset so a family cannot ride past the family checks. This contract binds the desktop app's own sources; the shared renderer package's catalog-owned lowering is referenced truth, not a place this oracle scans.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/app.css",
          "apps/openagents.com/packages/effect-native-tokens/src/index.ts",
          "docs/assurance/UX_CONTRACTS_AND_ASSURANCE.md",
        ],
        oracles: [
          {
            id: "typography.approved_font_stack_only",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/owner-ux-rules.test.ts",
            description:
              "Scans every non-test .ts/.cts/.css file under apps/openagents-desktop/src recursively: every CSS font-family declaration and every TypeScript fontFamily value must resolve family-by-family to the approved allowlist, every CSS font shorthand must be exactly 'inherit', and the approved base stack must still be declared in app.css. The falsifier test proves a rogue family (Comic Sans MS / Papyrus / a font shorthand smuggle) is rejected while the approved stack passes.",
          },
        ],
        verification:
          "bun test apps/openagents-desktop/tests/owner-ux-rules.test.ts runs in the normal desktop sweep; adding any stray font family anywhere under apps/openagents-desktop/src fails it.",
      },
      {
        contractId: "openagents_desktop.startup.window_first_no_blank_frame.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "startup / boot process",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-incident", statedBy: "owner", statedOn: "2026-07-13" },
        statement:
          "Opening the openagents app, via our new oa command or in dev, shows a blank/brown screen for ~5 seconds before opening the UI. This is unacceptable. I thought we had a UX contract somewhere about the need to show initial codex chats in <50 ms. That should be timed from startup. Go look up our ways of testing load times. Startup and everything else. Write full analysis of current situation in openagents/docs/fable/ new doc. This is an incident. Very bad. Need good bootup process. No brown screen. If any loading, show beautiful starcraft version of it, or something. Time to seeing stuff and then interactable elements on bootup is extremely important. Analyze, fix, update analysis, push.",
        authorityBoundary:
          "The BrowserWindow is created before any local database open, OS-keychain custody, or session network verification on the production whenReady path, and the post-window network settle is fire-and-forget. The renderer paints a static branded boot frame (khalaTheme literals mechanically synced to @effect-native/tokens) with the first HTML parse, mounts the interactable shell BEFORE the local coding-history scan, and streams hydration in afterwards behind an explicit 'Scanning coding history…' sidebar state — the 'No local Codex history found.' claim renders only after the scan settles. This contract governs boot ordering and honest loading presentation; it does not change the separate post-selection thread_first_content_under_50ms.v1 projection budget, and it does not promise a wall-clock bound for full history hydration on arbitrary ~/.codex sizes (bounding the scan itself is follow-up work, now off the critical path).",
        evidenceRefs: [
          "apps/openagents-desktop/src/main.ts",
          "apps/openagents-desktop/src/renderer/boot.ts",
          "apps/openagents-desktop/index.html",
          "apps/openagents-desktop/tests/startup-contract.test.ts",
          "apps/openagents-desktop/scripts/startup-bench.ts",
          "apps/openagents-desktop/benchmarks/startup/2026-07-13-window-first-boot-frame.json",
          "docs/fable/2026-07-13-desktop-startup-incident.md",
        ],
        oracles: [
          {
            id: "startup.window_first_ordering",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/startup-contract.test.ts",
            description:
              "Proves the production whenReady path creates the window before SQLite/keychain/network work, never awaits the network session settle after the window, and keeps the network call confined to the settle helper; falsifier fixtures prove the pre-incident ordering is rejected.",
          },
          {
            id: "startup.shell_mounts_before_hydration",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/startup-contract.test.ts",
            description:
              "Proves the renderer mounts the shell before the coding-history hydration effect runs, that the history catalog fetch lives inside hydrateAfterMount, and that the boot frame is removed after mount.",
          },
          {
            id: "startup.boot_frame_token_sync",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/startup-contract.test.ts",
            description:
              "Proves the branded boot frame exists in index.html and every color literal in it is an exact khalaTheme token value — no off-palette (brown) frame can ever paint — and the BrowserWindow backgroundColor stays the token background.",
          },
          {
            id: "startup.sidebar_scanning_honesty",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/startup-contract.test.ts",
            description:
              "Proves the typed shell view renders the scanning row while history hydration is pending and the empty-history claim only after hydration settles.",
          },
          {
            id: "startup.bench_budgets",
            kind: "script",
            mode: "headless",
            ref: "apps/openagents-desktop/scripts/startup-bench.ts",
            description:
              "The fixture-mode startup bench asserts median windowReadyToShow < 1500 ms and median shellMounted < 2500 ms and writes a timings-only receipt to benchmarks/startup/; the real-wiring OPENAGENTS_DESKTOP_STARTUP_TRACE mode records the same milestone chain (plus historyHydrated) against a real profile.",
          },
        ],
        verification:
          "Desktop typecheck, tests/startup-contract.test.ts in the normal sweep, scripts/startup-bench.ts receipts (real-profile before: shellMounted 5.4–7.0 s; after: ~0.7 s), and smoke screenshot receipts of the boot frame and mounted shell.",
      },
      {
        contractId: "openagents_desktop.workbench.turn_checkpoints.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "Coding workbench turn checkpoints",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-directive", statedBy: "owner", statedOn: "2026-07-13" },
        statement:
          "Desktop coding turns mutate the workspace with no cheap per-turn restore point. Capture at turn start/completion via an isolated temp GIT_INDEX_FILE, write hidden refs (refs/openagents/checkpoints/<thread>/<turn>), bounded to tracked + non-ignored files with size exclusions; never touches user branches/index. Typed revert as an explicit command with an irreversible-effects statement: stage, inspect, commit/clear; refuse on dirty conflicting state.",
        authorityBoundary:
          "The host-side turn-checkpoint service owns hidden-ref capture, the typed turn-over-turn diff query, and the staged revert command. Every snapshot is built through an isolated temporary GIT_INDEX_FILE: user branches, HEAD, the user index, stashes, and (during capture) the worktree are never written. Capture is bounded to tracked plus non-ignored untracked files with a per-file size exclusion and a total-file refusal. Revert only ever runs as stage then inspect then explicit commit; every staged revert carries the irreversible-effects statement, refuses dirty conflicting state, and retains a pre-revert baseline snapshot. Checkpoint refs and snapshots stay in the local repository only — they can contain secrets and never enter Sync projections, renderer state, or push surfaces. Thread checkpoint deletion removes every hidden ref for that thread. This is an internal post-MVP substrate: it authorizes no visible renderer affordance under the MVP surface allowlist.",
        evidenceRefs: [
          "apps/openagents-desktop/src/turn-checkpoint-contract.ts",
          "apps/openagents-desktop/src/turn-checkpoint-host.ts",
          "apps/openagents-desktop/src/main.ts",
          "docs/teardowns/2026-07-13-t3-code-teardown.md",
          "docs/teardowns/2026-07-10-opencode-v2-architecture-teardown.md",
          "github:OpenAgentsInc/openagents#8781",
        ],
        oracles: [
          {
            id: "turn_checkpoints.capture_hidden_ref_untouched_user_state",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/turn-checkpoints.test.ts",
            description:
              "Against a real fixture repository: capture on a fixture turn creates the hidden ref (ignored and oversized files excluded, typed completion signal emitted) with byte-identical user branches, HEAD, index, status, and stashes before and after.",
          },
          {
            id: "turn_checkpoints.staged_revert_exact_bytes_and_transitions",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/turn-checkpoints.test.ts",
            description:
              "Proves the typed diff query reports real turn-over-turn changes; stage/inspect/commit restores exact text and binary bytes (deleting later-turn artifacts) while inspect carries the irreversible-effects statement; clear abandons without mutation; double-stage, commit-without-stage, dirty stage, and post-stage drift all refuse typed; thread deletion removes that thread's refs only.",
          },
        ],
        verification:
          "Desktop typecheck and tests/turn-checkpoints.test.ts in the normal sweep; the suite also proves main.ts wires capture at turn_start and turn_completed on both local lanes.",
      },
      {
        contractId: "openagents_desktop.settings.harness_maintenance_one_click.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "Settings harness maintenance",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-issue", statedBy: "owner", statedOn: "2026-07-13" },
        statement:
          "fleet(MAINT-1): one-click provider install/update with ledger pinning and provenance receipts — typed per-harness maintenance actions: detect installed version + channel, resolve latest per channel, execute update via the harness's native path, then RE-PROBE the harness before reporting success; version pinning (record expected version/hash before update; refuse silent channel jumps); a provenance receipt for the swapped binary; one click in Desktop Settings per connected harness driving the typed action through the existing command path; CLI parity via pylon command; NEVER touch the default ~/.codex login home during update flows.",
        authorityBoundary:
          "The maintenance engine lives in @openagentsinc/pylon-core (custody/harness-maintenance) and updates BINARIES only: every probe and update spawn scrubs CODEX_HOME/CLAUDE_CONFIG_DIR/GROK_HOME from its environment, login/auth-flow arguments are refused by a typed guard, and the default ~/.codex home is never read or written. Success is only reported after a post-update version RE-PROBE answers on the same channel (launch-receipt lesson 4); a failed re-probe, a channel change, or an unchanged version is a typed maintenance failure with the previous state recorded intact. A pre-update pin (expected version + binary sha256 + channel) and a provenance receipt (source, command, output excerpt, before/after states, re-probe result) persist append-only under the shared Pylon home; the renderer projection carries versions/channel/advisory only — never paths, tokens, or raw command output. Settings renders the per-harness rows and the update affordance driving the typed gateway command; Electron main wires the actions post-window and adds nothing to the pre-window startup path.",
        evidenceRefs: [
          "packages/pylon-core/src/custody/harness-maintenance.ts",
          "packages/pylon-core/src/custody/harness-maintenance.test.ts",
          "apps/openagents-desktop/src/runtime-gateway-contract.ts",
          "apps/openagents-desktop/src/runtime-gateway.ts",
          "apps/openagents-desktop/src/renderer/settings.ts",
          "apps/pylon/tests/accounts-maintenance-cli.test.ts",
          "docs/fable/2026-07-13-chatgpt-codex-launch-failure-analysis.md",
          "docs/receipts/2026-07-14-harness-maintenance/README.md",
          "github:OpenAgentsInc/openagents#8785",
        ],
        oracles: [
          {
            id: "harness_maintenance.engine_round_trip_and_guards",
            kind: "bun-test",
            mode: "unit",
            ref: "packages/pylon-core/src/custody/harness-maintenance.test.ts",
            description:
              "Fixture-harness round trip with REAL spawned fixture binaries: detect → pin → update → re-probe → receipt; failure paths (update fails, post-update probe fails, version unchanged) keep the previous state intact in the receipt; channel jumps are refused without execution; the fixture ~/.codex/auth.json is byte-identical after every flow and auth-flow arguments are refused.",
          },
          {
            id: "harness_maintenance.cli_parity",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/pylon/tests/accounts-maintenance-cli.test.ts",
            description:
              "`pylon accounts maintenance --json` projects version/channel/advisory and `--update --harness codex` runs the same engine end to end against fixture binaries and a local registry: provenance receipt persisted under the Pylon home, channel jump refused with non-zero exit, fixture ~/.codex untouched.",
          },
          {
            id: "harness_maintenance.desktop_gateway_and_settings",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/harness-maintenance.test.ts",
            description:
              "The maintenance query/command decode through the versioned gateway contract (unknown harnesses refused), dispatch to injected host actions with per-harness single-flight, and Settings renders version/channel truth with the one-click update affordance driving the typed intent; failure and channel-jump-refusal outcomes surface honestly.",
          },
        ],
        verification:
          "Desktop typecheck, the three oracle suites in the normal sweep, and the built-host smoke settings capture (docs/receipts/2026-07-14-harness-maintenance/) showing the rendered harness rows.",
      },
      {
        contractId: "openagents_desktop.composer.focused_on_open.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "chat composer / window open",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-incident", statedBy: "owner", statedOn: "2026-07-14" },
        statement:
          "the text input should be focused immediately on open. so i can start typing right away.",
        authorityBoundary:
          "On window open — fresh launch and macOS re-activate with an existing window — keyboard focus lands in the message composer at SHELL-INTERACTABLE (the moment the shell mounts under the branded boot frame, composing with window_first_no_blank_frame.v1's boot ordering), so the first keystroke enters the composer with zero clicks. Background history hydration must never steal that focus; conversely the automatic settle passes (post-hydration, window re-activate) claim only UNOWNED focus (document.activeElement at body/root) and never move focus the user placed elsewhere. When the restored workspace renders no composer (a loaded history page), nothing is force-focused.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/composer-focus.ts",
          "apps/openagents-desktop/src/renderer/boot.ts",
          "apps/openagents-desktop/src/main.ts",
          "github:OpenAgentsInc/openagents#8787",
        ],
        oracles: [
          {
            id: "composer_focus.dom_focus_and_no_steal",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/composer-focus.test.ts",
            description:
              "DOM-level: the focuser lands document.activeElement on the composer at mount (and across late commits), a keystroke at the active element routes to the composer, the settle pass claims unowned focus, and it NEVER steals focus the user placed in another input.",
          },
          {
            id: "composer_focus.built_electron_first_keystroke",
            kind: "script",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "Built-Electron smoke steps composer-focused-on-open and first-keystroke-lands-in-composer: before ANY pointer event, document.activeElement is the composer at shell-mount and still after hydration settles, then a real Chromium keyboard event sent from the main process appears as typed text in the composer.",
          },
        ],
        verification:
          "Desktop typecheck, src/renderer/composer-focus.test.ts in the normal sweep, and the built-host smoke's composer-focused-on-open + first-keystroke-lands-in-composer steps.",
      },
      {
        contractId: "openagents_desktop.history.session_search_filters.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "sidebar session search",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-incident", statedBy: "owner", statedOn: "2026-07-14" },
        statement:
          "The search doesn't seem to fucking work at all. One of the chats is titled Assurance, but when I start typing in the first few letters there, it does not show it.",
        authorityBoundary:
          "Typing in the sidebar session search filters the list with case-insensitive substring matching over session titles and workspace labels — bounded deterministic field matching over the owner-local corpus (the semantic-routing invariant's bounded-field exception), never keyword intent routing. The search operates over the FULL loss-accounted catalog store (every root, including beyond the sidebar's current page), not just rendered rows: instant title matches come straight from the hydrated catalog cache, and the host content index (itself now byte-bounded per session, so a multi-GB rollout can no longer crash or starve it) merges in when it settles. While the host response is in flight the empty state says 'Searching…'; 'No sessions match.' renders only once settled; clearing the query restores the full list. The index remains a rebuildable cache, never catalog/page authority.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/history-workspace.ts",
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/history-search.ts",
          "apps/openagents-desktop/src/merged-history.ts",
          "github:OpenAgentsInc/openagents#8788",
        ],
        oracles: [
          {
            id: "session_search.filters_full_catalog",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "Through the real intent registry on a 45-root fixture catalog: a title prefix ('Ass') filters to the matching session even though it sits beyond the 40-row first page; the no-match state is explicit; clearing restores the full list; a deferred host response shows 'Searching…' (never a false no-match) and merges content results when it settles.",
          },
          {
            id: "session_search.instant_and_merge_helpers",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/history-workspace.test.ts",
            description:
              "historyImmediateSearchResults prefix-matches the full catalog case-insensitively including beyond-page roots; mergeHistorySearchResults dedupes by threadRef, keeps host content matches, ranks by score, and degrades to instant matches on a null host response.",
          },
          {
            id: "session_search.workspace_label_and_bounded_index",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/history-catalog-scale.test.ts",
            description:
              "The host index carries the session's workspace label (cwd basename) as a searchable title-tier field, and its per-session content read is item- and byte-bounded rather than a whole-file read.",
          },
          {
            id: "session_search.built_electron_filter_journey",
            kind: "script",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "Built-Electron smoke types a title prefix into the sidebar search and asserts the fixture session row appears, then a no-match query shows the explicit empty state and clearing restores the full list — with a pixel receipt of the filtered sidebar.",
          },
        ],
        verification:
          "Desktop typecheck, the shell/history-workspace/history-catalog-scale suites, and the built-host smoke's session-search steps with screenshot receipts.",
      },
      {
        contractId: "openagents_desktop.history.sidebar_header_truthful_scope.v1",
        state: "enforced",
        surface: "openagents-desktop",
        productArea: "sidebar coding-history header / catalog scope",
        enforcementTier: "test-sweep",
        blockerRefs: [],
        source: { channel: "owner-incident", statedBy: "owner", statedOn: "2026-07-14" },
        statement:
          "That says coding history all time, but it only has five chats, so that's definitely not all time.",
        authorityBoundary:
          "The sidebar header's scope claim must match the projection's real semantics: 'Coding history · scanning…' before hydration settles, a counted disclosure 'Coding history · N of M' while the loss-accounted catalog is paged (M counts every catalogued session for this surface, deduplicated against local threads; explicit 'Load K more' paging reaches the remainder per the episode-248 loss-accounted v2 contract — recent-first bounded disclosure, no age ceiling, no silent truncation), and 'Coding history · all N' only when every catalogued session is shown. A label is never allowed to claim more than the projection delivers. The root cause this contract pins closed: the catalog graph build read whole rollout files to derive titles, ENOMEMed on a real 4.5 GB rollout, and silently collapsed the 'all time' surface to the 24-hour recent list; catalog title scans, page reads, and search-index content reads are now byte-bounded/streaming so an oversized session degrades to a fallback title instead of taking down the catalog. Scope wording only — no marketing copy changed.",
        evidenceRefs: [
          "apps/openagents-desktop/src/renderer/shell.ts",
          "apps/openagents-desktop/src/codex-history.ts",
          "apps/openagents-desktop/src/merged-history.ts",
          "apps/openagents-desktop/tests/history-catalog-scale.test.ts",
          "docs/teardowns/2026-07-10-openagents-product-adaptation-analysis.md",
          "github:OpenAgentsInc/openagents#8789",
          "github:OpenAgentsInc/openagents#8674",
        ],
        oracles: [
          {
            id: "history_header.counted_disclosure_truth",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/src/renderer/shell.test.ts",
            description:
              "On a >page-size multi-root fixture: the header reads 'scanning…' pre-hydration, 'N of M' with an explicit 'Load K more' row while paged, 'all N' only at full disclosure, never double-counts local threads that are also catalogued, and formats the owner's 1,543-scale counts with separators.",
          },
          {
            id: "history_header.catalog_survives_oversized_rollouts",
            kind: "bun-test",
            mode: "unit",
            ref: "apps/openagents-desktop/tests/history-catalog-scale.test.ts",
            description:
              "A >page-size multi-workspace store catalogues every root (children excluded, never lost) with no silent truncation; a session whose authored title lies beyond the bounded head scan degrades to the fallback title while the rest of the catalog survives; the streaming page read keeps whole-conversation totals and loss accounting while returning only the requested window.",
          },
          {
            id: "history_header.built_electron_header",
            kind: "script",
            mode: "e2e",
            ref: "apps/openagents-desktop/src/main.ts",
            description:
              "Built-Electron smoke asserts the sidebar header states the fixture catalog's true counted scope ('Coding history · all 1') — the untrue 'all time' claim fails the smoke.",
          },
        ],
        verification:
          "Desktop typecheck, shell + history-catalog-scale suites, the built-host smoke header assertion, and the real-store diagnosis receipt (1,289 roots from 1,582 sessions in ~1.9 s where the pre-fix build ENOMEMed).",
      },
    ],
  };
