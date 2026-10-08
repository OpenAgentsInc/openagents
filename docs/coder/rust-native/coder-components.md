# Shared Coder components and the web catalog

Status: implementation specification, October 8, 2026. The first deliverable is
**`openagents.com/components`**, an interactive web component library with web
versions of **every presentation component in `crates/coder-new`**, including its
imported presentation dependencies, visible variants, and composed screens. The
library must let an application recreate the current Coder UI exactly on the web.
This document extends the [Coder Cloud specification](../../cloud/coder-cloud.md).
It does not claim the catalog or general web adapter is implemented.

## Shared library decision

The sibling `~/work/coder` uses shared components across its platform surfaces.
Carry that architecture forward through this repository's existing Rust Native
foundation: define a component's meaning, presentation values, states, and typed
interactions once, then render them through platform adapters. Reimplement this
design here in Rust. The sibling is reference material; its private code,
backend, prompts, endpoints, authentication, and credentials remain outside this
repository.

Rust Native is the reusable framework. Coder's application component library
extends `coder-ui`; a proposed `rust-native-web` adapter renders it on the web.
The catalog and Coder Cloud consume that library. Coder's terminal and other
platforms adopt the same definitions incrementally. A page-specific copy of the
terminal UI does not meet the shared-library requirement.

The current [view contract](../../../crates/rust-native/docs/spec.md) is
`rust-native.view.v2`. It includes Stack, List, Text, Button, Surface, Transcript,
Message, Markdown, Tool, Working, and Composer, plus shared editing, selection,
syntax, and transcript layout. `coder-ui` currently contains theme values;
`coder-new` still contains its own Ratatui presentation. There is no general
`rust-native-web` adapter. Older foundation-only inventories do not describe
the complete current core, and sharing a palette does not constitute shared
component delivery.

| Layer | Ownership and dependency direction |
| --- | --- |
| `rust-native` | Generic semantic elements, validated views, interaction identity, styles, editing/IME, selection, Markdown, and layout. No Coder palette, app state, transport, account, model, or execution dependency. |
| `coder-ui` | Pure Coder presentation types, component constructors, named product tokens, presentation-state helpers, and deterministic fixture definitions. Depends on Rust Native, not `coder-new`, Ratatui, a provider, or a host runtime. |
| `rust-native-web` (proposed) | Reusable escaped HTML/CSS mapping and Rust/Wasm DOM mounting, focus, input/IME, selection, scrolling, browser measurement, and capability reporting. No Coder-specific screens or domain effects. |
| `coder-new` and other application projections | Convert existing state/events into shared presentation values; resolve product intents through existing controllers and authority. Backend/session/provider state stays with its existing owner. |
| Terminal/native adapters | Render shared component meaning with existing terminal facilities or platform controls. Ratatui values stay on the terminal side of the boundary. |
| `openagents-web` | Own `/components`, catalog navigation, fixture selection, assets, and the page shell. Compose the same library later in the authenticated app. |

Application components are Rust functions/types that compose generic elements.
Keep transcript, agent rail, plugin manager, picker, and settings definitions in
the product layer. Add a core primitive only when a reusable semantic gap demands
it. Current gaps include standalone editable form fields, focus-scoped dialogs,
selected listbox semantics, and styled inline runs needed by source-equivalent
code/diff rows. Establish their accessibility, input, bounds, and schema
compatibility before extending the core. Existing Markdown tables and editing
must be reused rather than reimplemented.

Use generic elements only with their declared meaning. Current Message, Tool,
and Composer contracts include user-bubble placement, expansion, and send/stop
controls that differ from some terminal presentations. Compose primitives in
`coder-ui` or add an explicitly compatible appearance contract; do not silently
redefine generic elements to obtain terminal parity. Version new semantics
deliberately: closed v2 decoding rejects unknown fields.

The dependency test is practical: the web component library builds for Wasm
without linking terminal I/O, the full Coder app, local credentials, subprocess
execution, or a provider SDK. Shared components take bounded presentation values
and produce validated views. Browser and terminal code cannot independently
decide a tool's status, actual model, cost, or permission from display text.

## Initial deliverable at `/components`

Serve this catalog from the Rust web app in this repository. Explicitly own the
route instead of forwarding it to the legacy sidecar. Public examples contain
deterministic synthetic data and require no login, provider, enrolled computer,
cloud service, wallet, sales credential, or live session. A public catalog can
ship before Coder Cloud's accounts or commercial activation.

The catalog contains:

- A searchable index grouped by foundations, conversation, tools, agents,
  composer, pickers, settings, recovery, and full screens.
- A stable component URL, `/components/{component}`, with named fixture/state
  selection, description, typed property/intent contract, public source symbol,
  source revision, and adapter support.
- A working example of every variant. Controls select state, width/height,
  sample content, focus/selection, elapsed time, and animation phase. Inputs,
  pickers, disclosure, navigation, and scrolling operate on fixture state.
- A source-equivalent Coder preview beside the isolated component, with a
  fullscreen preview that removes the catalog chrome.
- Sanitized props, resolved token/style information, and a demo event inspector.
  Secret fields and private-value intents never enter the inspector or URLs.
- Searchable completeness: each inventoried render branch and fixture reports
  its web implementation and visual/interaction verification state.

Generate navigation and counts from the catalog manifest. Serve useful escaped
HTML while Wasm loads or is unavailable, then mount interactive examples through
the reusable adapter. Verify HTML output and functioning browser controls
separately. Admit links explicitly and display untrusted markup as literal text.

Test-key, send, delegate, save, resume, approve, stop, and export examples use
fixture controllers. A demo acknowledgment cannot invoke a provider, change
real settings, approve disclosure, create a task, or move money. The only actual
local export is a user-requested synthetic fixture download or clipboard copy.
No example reads the visitor's files or the operator's home directory. Label
these controls and outcomes as demonstrations.

Do not ship a screenshot gallery as the library, render the app from a Ratatui
buffer, or draw every control onto one canvas. Examples instantiate reusable
components with actual selectable text and accessible DOM controls. Source
snapshots are comparison evidence, not the production presentation contract.

## Required source inventory

“All components” includes every renderable building block, visible conditional
branch, state-dependent layout, imported Markdown/diff helper, modal, form,
picker, and full-screen composition reached by `coder-new`. It includes
production states absent from the existing demo images. Runtime-only functions
are outside the visual inventory; their observable UI states remain inside it.

Maintain a reviewed catalog manifest, proposed schema
`openagents.coder.component-catalog.v1`, in the shared product library. Each entry
binds a stable component ID, public source file/symbol and branch, typed props
and intents, named fixtures, source revision, viewport/theme/clock inputs,
reference artifacts, adapter implementation, and verification results.
One component can cover several source helpers, but every source render site
must map to an entry and every visible variant must map to a fixture.

The following inventory is the required starting scope, reviewed against
`coder-new` at commit `36d59f3806`. Function names locate the current source;
extraction can change their names without dropping their coverage.

| Family | Required components and variants | Current presentation source |
| --- | --- | --- |
| Theme and text | Every neutral surface/text/border token and semantic accent; span modifiers; Unicode/cell measurement; wrapping; ellipsis; source color remapping; fixed-cell and selectable rich text. | [`theme.rs`](../../../crates/coder-new/src/theme.rs), `usgc_lines`; [`ui.rs`](../../../crates/coder-new/src/ui.rs), `span`, `truncate`, `wrap_display`; [`terminal wrap`](../../../crates/coder-terminal/src/wrap.rs). |
| Screen shell | Background, outer margins, header/body/composer/context/rail layout, empty live and populated demo, selected child, fullscreen replacement screens, modal overlay, minimum-supported and too-small viewport. | `ui::render`. |
| Header and context | Blank main header; selected child's styled name; repository directory and branch; default/absent context; long directory with preserved branch suffix; too-wide suffix fallback. | `ui::header_view`, `context_view`. |
| User prompt | Chevron, continuation indentation, full-width light band, Markdown content, empty/multiline/nested content, and wrapping. | `ui::prompt`, `message_body`. |
| Assistant reply | Completed and streaming Markdown, actual served-model attribution, elapsed-only/model-only/both/absent footer, right alignment, long attribution, and partial/stopped replies. | `ui::entry_lines`, `reply_lines`, `live_lines`, `live_conversation`. |
| Markdown | Paragraphs, headings 1–6, ordered/unordered/nested/task lists, quotes and blank rows, rules, fenced/indented code, known/unknown language, unfinished streaming syntax, inline marks and destinations, image-alt projection, and literal raw HTML. | [`turn::markdown_body`](../../../crates/coder-terminal/src/components/turn.rs); [`terminal Markdown`](../../../crates/coder-terminal/src/markdown.rs), `wrapped`, `block_lines`, `inlines`. |
| Markdown tables | Boxed wide tables, source column geometry, marked headers, wrapped cells, long values, and narrow stacked fallback. The terminal discards alignment markers; additional alignment support is a separate extension. | Terminal Markdown `table_lines`, `stacked_table`. |
| Demo native tools | Read, Search, Edit, and Run in running/done/failed states; headings, input/detail, output branch, multiline results, spinner/failure label, and completed Edit counts/diff. | [`tools.rs`](../../../crates/coder-new/src/tools.rs), `ToolKind`, `ToolState`, `tool_lines`; [`agents.rs`](../../../crates/coder-new/src/agents.rs), `MAIN_TOOLS`. |
| Diff and code rows | Equal/insert/delete lines, old/new line numbers and separate syntax state, gutters, full-width change bands, multiple hunks/separators, wrapped continuation, long words, narrow width, unknown language, and addition/deletion counts. | [`terminal diff`](../../../crates/coder-terminal/src/components/diff.rs), `hunks`, `lines`, `assemble_rows`, `render_gutter`, `render_content_spans`. |
| Demo plugin calls | Qualified operation label, optional input, result branch, running/done/failed, and adjacent tool/plugin grouping without added gaps. | `tools::plugin_lines`, `ui::conversation`, `agents::MAIN_PLUGINS`. |
| Live tools and plugins | Native Run/Read/Edit/Search labels versus qualified Plugin label, status glyphs, separate argument/result bands, running result, and human-readable Brainstorm summary. Preserve differences from demo rows. | `ui::entry_lines`; [`brainstorm.rs`](../../../crates/coder-new/src/brainstorm.rs), `summary`. |
| Parameters and results | Null/empty/scalar/string inputs, newline markers, nested dotted fields, arrays/deeper serialized values, five-field limit and omitted-field row, long keys/values, dark band and branch gutter. | `tools::parameter_lines`. |
| Delegation rows | Demo pulsing delegate row versus live running/done/failed row, agent/task/state/tokens, narrow label omission, and nested delegate linkage. | `tools::delegation_lines`, `pulse`, `ui::entry_lines`. |
| Agent rail | Empty/one/many/four-demo rows, selected/unselected, scroll window, long names/task truncation, aligned tokens, elapsed time, narrow token-only fallback, retained finished time, and full-row selection background. | `ui::agent_rail`, `token_count`; `agents::elapsed_time`. |
| Composer | Top/bottom rules with no side walls, prompt, empty/single/multiline draft, one to six visible rows, overflow/caret, selected-child intensity, picker-hidden caret, grapheme editing, paste, and independent parent/child drafts/cursors. | `ui::composer_view`; [`lib.rs`](../../../crates/coder-new/src/lib.rs), `Draft`, `select_agent`. |
| Composer contributions | Top/bottom rail slots, enabled-provider selected model/options, observed/current fallback model, absent/disabled contribution, priority/conflict handling, and suffix-preserving truncation. | [`plugin_definition.rs`](../../../crates/coder-new/src/plugin_definition.rs), `RailSlot`, `resolve_composer_rails`; `ui::composer_rail_text`; [`terminal rail`](../../../crates/coder-terminal/src/hairline.rs). |
| Slash picker | All build-available commands, prefix filtering, selected/unselected rows, scroll window, no matches, demo availability descriptions, aligned usage/description, and narrow/short popup above composer. | [`slash.rs`](../../../crates/coder-new/src/slash.rs), `Command`, `matches`, `render`. |
| Status and animation | Eight spinner frames, delegate pulse phases, caret on/off, running/retained elapsed time, token formatting, Working tail, global notices, chat errors, stopped/unavailable states, export/copied/clipboard-error notices, and demo acknowledgment. | `tools::spinner`, `pulse`; `ui::conversation`, `live_conversation`; `lib::tick`, command/export handling. |
| Exact disclosure review | Fullscreen recipient and pretty-printed exact Brainstorm input, scrolling paragraph, confirm enabled only after full review, reject/cancel, and return to conversation on closure/timeout. Review completion and timeout are lifecycle fixtures, not additional source widgets. | `ui::render` disclosure branch; [`approval.rs`](../../../crates/coder-new/src/approval.rs), `Desk::disclose`; `lib.rs` disclosure events. |
| Plugin manager and details | All eight built-ins; enabled/disabled/selected rows; Disabled, Setup required, Configured, Checking, Verified, Unavailable, Enabled, No agents detected, All agents off, Demo fixture, and Expired statuses; descriptions, configuration/error details, builtin info screens, wide columns/narrow rows, footers, and scrolling. | [`ui/plugins.rs`](../../../crates/coder-new/src/ui/plugins.rs), `manager`, `plugin_details`, `plugin_info`, `status_color`. |
| Settings primitives | Labeled fields, masked secret field, field/caret focus, checkbox/toggle, choice, descriptions, action control, separator, keyboard hints, status/error bands, save/cancel, and narrow/scrolling layouts. | `ui::plugins::{field, action, detail, render_fields}` and specialized forms below. |
| OpenRouter settings | Fixed endpoint, masked key, Model ID/default help, Test/Save/Remove key/Cancel; absent/saved/replacement/pending-removal keys; unchecked/checking/verified/failed/demo; private-file versus memory-only storage and validation/write errors. Manager enablement and staged model options remain separate components. | `ui::plugins::router_settings`; [`plugins.rs`](../../../crates/coder-new/src/plugins.rs). |
| Jev settings | TypeSafe direct/Vercel AI Gateway/Custom gateway choices; editable API base and gateway-specific model default; masked origin-bound key, replacement/removal; changed-origin rekey refusal; Test/Save/Remove key/Cancel; unchecked/checking/verified/failed/demo and validation/storage errors. | `ui::plugins::jev_settings`; [`bundled_settings.rs`](../../../crates/coder-new/src/bundled_settings.rs). |
| ACP settings | Detected-agent checklist, selected/unselected and enabled/disabled rows, names and wide-view program filenames, detected/on counts, empty/all-off/refresh/back, long-list scrolling, and persistence errors. Undetected executables are omitted; discovery fixtures perform no local probing. | `ui::plugins::acp_settings`; [`acp_discovery.rs`](../../../crates/coder-new/src/acp_discovery.rs). |
| Brainstorm settings | Fixed House perspective; saved versus edited HTTPS recipient; disclosure explanation; explicit public-discovery test; optional house key/time and non-atomic identity warning; disabled/configured/checking/verified/expired/unavailable/demo; Save/Cancel and validation/connection/storage errors. Opening, enabling, and saving cause no read. | `ui::plugins::brainstorm_settings`; [`brainstorm.rs`](../../../crates/coder-new/src/brainstorm.rs). |
| Boat and GCE settings | Distinct placement forms, Coder/integrated mode where supported, machine shape, template, credential variable names, workspace paths, focused edits, save/cancel/errors, and fixed GCE restrictions. | `ui::plugins::cloud_settings`; [`cloud_settings.rs`](../../../crates/coder-new/src/cloud_settings.rs). |
| Model picker | Centered rounded overlay on conversation or settings; Models/Reasoning/Output stages; search, selected/current options, loading/refresh-failure fallback/no matches; optional context/output details; capability-driven options and stage skipping; changed pending capabilities returning selection to Models; cancel/back and narrow sizing. | [`ui/models.rs`](../../../crates/coder-new/src/ui/models.rs), `render`, `choices`, `render_choice`, `search`, `details`; [`models.rs`](../../../crates/coder-new/src/models.rs), [`model_catalog.rs`](../../../crates/coder-new/src/model_catalog.rs). |
| Resume and session state | Fullscreen newest-first recent-session list; numbered title/age/entries/ID/cwd rows, selected window/paging, empty/error, resume/back hints; busy/store/snapshot/active-writer refusal notices; explicit passive follow, takeover pending/acquired/reclaimed, save failure, restored cwd notice, and restored parent/child state. | [`ui/resume.rs`](../../../crates/coder-new/src/ui/resume.rs); [`resume.rs`](../../../crates/coder-new/src/resume.rs), [`sessions.rs`](../../../crates/coder-new/src/sessions.rs), `lib.rs`. |
| Export and restored evidence | Parent and selected-child state, child trajectory linkage, partial/pending/completed/failed records, attribution/tokens, redaction, validation failure, and restored conversation renderings. | [`trajectory.rs`](../../../crates/coder-new/src/trajectory.rs), `document`, `main_document`, `export_app`, `read`, `restore_app`; existing notices rather than an invented trace screen. |

The manager includes OpenRouter BYOK, Microcoder, Jev, OpenAgents CLI, ACP
Subagents, Brainstorm, Boat Cloud, and GCE Cloud. Microcoder and OpenAgents CLI
have information views instead of editable settings forms. ACP settings list
detected executables; they do not define arbitrary executable commands. Cloud
forms select credential variable names, not secret values; GCE fixes its pool
shape and Coder mode. The catalog is not a generic JSON settings editor.

Model fixtures include the seeded source catalog and its declared capabilities.
Reasoning shows only supported efforts; mandatory reasoning omits **None**.
Output choices use **Model default** and supported token limits, filtered by the
effective maximum. Unknown context/output limits stay unknown. Public metadata
changes cannot replace host-owned labels; refresh failures preserve known
choices. The fallback OpenAgents AI Gateway contributes to the composer rail
but is not a ninth plugin-manager settings screen.

`approval.rs` also owns exact command approval events, but `ui.rs` does not have
a standalone generic command-approval card. Preserve its observable existing
states in fixtures. A new web approval presentation is an explicitly designed
adapter over that owner contract, not a component falsely claimed to exist in
the terminal. The same distinction applies to a future trace-detail view.

Every table row needs all its named variants. Add fixtures for data and error
branches discovered during extraction. An absent fixture, unsupported essential
web primitive, or static-only substitute counts as incomplete, even if other
components have shipped. Source changes to a render path require updating the
inventory and evidence; deleting an entry cannot make missing parity pass.

## Full-screen compositions and visual fidelity

The catalog must compose the same components into these complete examples:

1. Empty live conversation and populated main demo with all four native tools,
   plugin calls, four delegates, composer, context, and agent rail.
2. Each selected demo child, plus live parent/child screens with streaming,
   nested delegates, actual tools, completion attribution, errors, and stop.
3. Parent/child drafts and scroll positions, long transcripts, long commands,
   Markdown tables/code/diffs, notices, and restored/pending evidence.
4. Slash suggestions over empty and multiline composers.
5. Plugin manager, builtin info views, and every specialized settings form.
6. Model, reasoning, and output pickers over conversation and settings.
7. Recent-session picker, empty/error state, follow-only session, and takeover.
8. Exact Brainstorm disclosure before and after full input review.
9. Wide, narrow, minimum-supported, and too-small compositions for each layout.

Use a source-equivalent **Coder terminal** layout profile as the default Coder
example. Preserve the blank main header, selected-child header, transcript
spacing, composer rules without side walls, context below the composer, agent
rail below context, floating slash picker, parameter bands, and modal layering.
Native browser interaction adds accessibility without changing this profile's
visual arrangement. Alternative responsive layouts can be named separately;
they do not replace the source-equivalent examples.

Promote the current product roles from
[`coder-new::theme`](../../../crates/coder-new/src/theme.rs) into `coder-ui`
without changing their resolved values. They include near-black, light/dark
bands, text/grays/border, cyan model/skill/code, magenta delegates, amber
commands, orange paths, green success/insertion, and red failure/deletion, with
the current dim change-band backgrounds and imported syntax/Markdown remapping.
Keep generic Rust Native color values product-neutral. The website's white
palette and the sibling's historical amber palette cannot overwrite this
Coder profile. Show every token and rich-text modifier in the catalog.

Preserve Rust Native's resolved style semantics: ordered leaf composition,
`Unset`/`Reset`, explicit defaults, and no implicit parent inheritance. CSS
inheritance or class order cannot change a resolved Rust style. Verify these
rules through nested and conflicting-style fixtures.

The current [`code-highlight`](../../../crates/code-highlight/src/lib.rs)
Wasm build has no grammars and returns plain text. That fallback cannot satisfy
syntax-color parity. For catalog fixtures, the Rust server can derive bounded
typed syntax spans using the same public highlighting path as the terminal;
the browser renders those spans. Validate text ranges and style roles rather
than accepting arbitrary HTML. A portable highlighter is an alternative only
after its source-equivalent output passes the same checks.

[`snapshot::svg`](../../../crates/coder-new/src/snapshot.rs) invokes the same
`ui::render` as the terminal. Use it as the source reference: the main fixture
is 110 columns by 36 rows, with 9 by 20 pixel reference cells, Paper Mono at
the exporter's font metrics, and controlled cursor/spinner/pulse/clock inputs.
Retain additional source sizes, including 80 by 24, 40 by 12, 24 by 12, and
too-small fallback. Browser CSS pixels and terminal cells are distinct units;
the Coder profile records their explicit mapping.

Compare exact logical text, color roles, modifiers, line-number/gutter geometry,
ordering, wrapping/truncation, row widths, selection, actual attribution, and
animation phases. Browser captures compare the component region at the same
dimensions with bundled fonts ready. Pin the browser, device scale, phase, and
visual comparison policy; explain measured rasterization differences without
waiving structural or interaction differences. A one-off mockup or acceptable
single screenshot does not establish all-component fidelity.

## Interaction and lifetime contract

Component props are immutable presentation snapshots with stable keys. Typed
callbacks resolve against the displayed instance and revision; disabled,
removed, or stale controls refuse. The application separately checks authority
and durable idempotency. Generic validation never proves a user approved a
command, disclosure, merge, payment, or message.

Keep local editing and disclosure/selection/focus presentation state in Rust.
The browser supplies DOM input/composition, pointer, keyboard, and clipboard
events through the web adapter. Preserve graphemes, selection, paste without
implicit send, IME commit once, draft/cursor ownership, and exact submitted-text
acknowledgment. Editing while a send is pending must not lose the newer draft.
Masked fields never expose their values through inspector, diagnostics, copy,
fixtures, or persisted browser state.

Mounts acknowledge the revision actually applied. A late mount cannot replace a
newer one; recycled or disposed instances invalidate old callbacks. Verify this
ordering with asynchronous updates rather than relying only on event checks.

Preserve source picker stage/back behavior, selection and filter windows,
settings draft/save/cancel/error behavior, model options and actual-response
attribution, and independent parent/child conversation state. New pointer
controls can complement keyboard controls; browser-reserved keys need an
accessible equivalent. Focus returns to its originating control after a modal
closes. Do not reset field focus or scroll because a streaming row changes.

Follow a transcript while its reader is at the bottom; pause follow while
reading history; offer **Jump to latest**; preserve the anchor when rows prepend.
Virtualization retains selection and access to original text. Fixture clocks are
explicit, and animation stops on inactive surfaces, with reduced-motion
alternatives retaining status. Disposal releases subscriptions, timers, input,
and callbacks; it does not cancel an application's durable work.

`Transcript.source` references an in-process Rust source registry. A source ID
rendered by the server does not transfer its rows into Wasm. Hydrate bounded
fixture pages into the browser's own registry and preserve source-bound cursors.
Include a long transcript exceeding the current 512 KiB/1,024-node view limits;
paging and virtualization must retain original text, selection, and prepend
anchors without constructing an oversized semantic view.

## Delivery and completion evidence

Implement this as the first Coder Cloud deliverable, before authenticated app
screens. Keep existing terminals and other platforms shipping while extracting
presentation. First inventory source branches and fixtures; then extract shared
values/components and missing generic semantics; implement the web adapter;
mount `/components`; finally assemble and compare complete screens. Incremental
commits can deliver families, but the first deliverable is complete only when
**all** inventoried components, variants, and compositions pass on the web.

Acceptance requires:

- Every inventoried render site and named variant has a manifest mapping,
  shared component, interactive web implementation, fixture, and recorded result.
  No unimplemented, unsupported, screenshot-only, or undocumented-exclusion rows
  remain in the required scope.
- Coverage checks run in both directions: every registered variant renders and
  has evidence, and every retained fixture/reference artifact belongs to a
  registered variant. Missing and obsolete fixtures or snapshots fail. Declared
  actions have meaningful interaction scripts, including reset to the fixture.
- The same component definitions build isolated examples and complete screens.
  A full Coder conversation can be assembled without reimplementing component
  markup, styling, formatting, or state behavior in `openagents-web`.
- The source-equivalent profile matches terminal fixtures at declared sizes,
  colors, layout, and deterministic phases, including demo/live differences.
- Keyboard, pointer, focus, Unicode/IME, clipboard denial, narrow layouts,
  scrolling, selection, stale callbacks, reduced motion, and tab suspension
  work with recorded limits on the supported browser.
- Escaped HTML remains useful before Wasm mounts. Interactive results separately
  prove revision acknowledgment, modal focus return, long-transcript hydration,
  zoom/overflow, registered-surface lifetime, and resolved-style parity.
- Generic Rust Native remains independent of Coder, and the product/web
  presentation dependency graph excludes runtime and terminal libraries.
- Catalog requests and fixture actions make no live provider/cloud/host/payment
  calls and read no real private stores. Displaying a future paid, approved, or
  running state creates no corresponding domain effect.

Reuse the existing source tests and previews as evidence inputs:
[`preview.rs`](../../../crates/coder-new/tests/preview.rs),
[`markdown.rs`](../../../crates/coder-new/tests/markdown.rs),
[`attribution.rs`](../../../crates/coder-new/tests/attribution.rs),
[`transcript_runtime.rs`](../../../crates/coder-new/tests/transcript_runtime.rs),
and settings/model/resume tests. Focus new tests on cross-adapter parity,
coverage, meaningful interactions, and lifecycle; avoid tests that repeat the
renderer implementation. Preserve existing notices and third-party licenses
when extracting public presentation code.

At implementation time, retain the manifest, source pin, fixture inputs,
terminal references, offscreen browser captures, interaction receipts, and
remaining platform limitations under the existing verification conventions.
Use the repository's build/browser leases and scratch hosts, not the owner's
screen or live sessions. Do not require all-platform migration, a funded sale,
or a full Rust release gate to ship the public fixture catalog.
