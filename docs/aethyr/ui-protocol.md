# The Aethyr UI protocol

[ADR 0005](../adr/0005-effect-controls-the-ui.md) splits each Aethyr interface
into a controller and a renderer. This page describes the protocol between
them, `aethyr.ui.v1`, and the design tokens both sides use
([ADR 0006](../adr/0006-one-design-system.md)).

| Piece | Where |
| --- | --- |
| View tree and intent schema | `crates/aethyr-ui` (Rust, the source) |
| JSON Schema | `crates/aethyr-ui/schema/aethyr.ui.v1.schema.json` (generated) |
| TypeScript types | `apps/verse-ui/src/aethyr/protocol.ts` (generated) |
| Design tokens | `crates/oa-tokens` ([ADR 0012](../adr/0012-one-token-crate.md)), written to `apps/verse-ui/src/aethyr/tokens.css` |
| Controller runtime | `apps/verse-ui/src/aethyr/controller.ts` |
| React DOM renderer | `apps/verse-ui/src/aethyr/renderer.tsx`, joined to a controller by `host.tsx` |
| Entry dialog | `apps/verse-ui/src/aethyr/entry.ts` |
| `/new-lander/verse` | `verse.ts` (controller) and `verse-host.tsx` (web host and engine bridge); see [Surfaces](#surfaces) |
| Planet setup, invitations, player bar | `house.ts` (controllers) and `house-host.tsx` (web host and bridge); see [Surfaces](#surfaces) |
| Studio HUD (`/new-lander/house`, courtyards) | `studio.ts` (controller) and `house-host.tsx`; see [Surfaces](#surfaces) |

The web files move to `apps/aethyr` when that package exists.

## The view tree

A controller publishes a `View`:

```json
{ "schema": "aethyr.ui.v1", "instance": "verse-entry-1", "revision": 2,
  "root": { "id": "verse-entry", "element": { "kind": "dialog", ... } } }
```

- `instance` names one controller lifetime. `revision` is positive and
  increases. One revision always names the same tree.
- Each `Node` has an `id`, unique in the view and stable across revisions for
  the same thing, so the renderer keeps focus, scroll position, and mounted
  media on it. IDs use letters, digits, `-`, `_`, `.`, and `:`, up to 128
  bytes.
- `element` is one of a closed set of kinds, tagged by `kind`:

  | Kind | What it is | Intents it takes |
  | --- | --- | --- |
  | `panel` | HUD glass panel with corner ticks, optional title | none |
  | `stack` | Children in a column, a row, or a wrapping row | none |
  | `text` | Text with a role: body, title, heading, caption, code, status | none |
  | `label` | Mono uppercase HUD readout, optional signal dot | none |
  | `button` | A control; with `href` (a same-site path) a link | `activate` |
  | `chip` | A small toggle with explicit `selected`, and an optional `detail` line | `activate` |
  | `list` | Rows; `selectable` lists choose a row | `select` (value: row ID) |
  | `field` | Single or multiline text, bounded by `max_bytes` | `input` (value), `submit` |
  | `sheet` | Non-modal panel; bottom sheet on phones | `dismiss` when `dismissible` and open |
  | `dialog` | Modal focus scope | `dismiss` when `dismissible` and open |
  | `toast` | Short status message | `dismiss` when `dismissible` |
  | `media_slot` | A place the host fills, named by `slot` (never a URL) | none |
  | `engine_surface` | The engine's drawing surface, named by `surface` | none (the engine takes input) |

- `style` refers to tokens only: `gap` and `padding` (`none` to `xl` on the
  spacing scale), `align`, `tone` (`default`, `muted`, `primary`, `danger`),
  and `hide_on_phone` for secondary nodes. No field can hold a color or a
  length.
- `a11y` overrides the kind's default role (`group`, `region`, `navigation`,
  `status`, `alert`, `presentation`), its accessible name and description,
  live-region politeness, or hides a decorative node. Controls and focus
  scopes need a nonempty label.

Limits: 1,024 nodes, depth 16 (the root is depth 1), 512 KiB encoded, 16 KiB
per string, and 64 KiB for a field's `max_bytes`. Unknown kinds, properties,
and token names fail to decode.

## Intents

The renderer sends an `Intent`:

```json
{ "instance": "verse-entry-1", "revision": 2, "node": "continue-guest",
  "kind": "activate" }
```

`select` and `input` carry a `value`; the others carry none. An intent carries
identity, not a payload to run: the controller looks the node up in the
revision it published and refuses the intent when:

| Code | Meaning |
| --- | --- |
| `stale` | Another instance, or a revision the controller has since replaced |
| `unknown_node` | No such node in that revision |
| `not_accepted` | The node does not take this kind (see the table above) |
| `disabled` | The control is disabled |
| `bad_value` | A value is missing, unexpected, not a row of the list, or over `max_bytes` (UTF-8) |

Input values are personal data. Rust redacts them from `Debug`; the web host
never logs them.

View checks return `schema`, `identity`, `bad_id`, `duplicate_id`,
`node_limit`, `depth_limit`, `text_limit`, `view_limit`, `missing_label`,
`bad_reference`, or `encoding`. Rust (`View::validate`, `ValidatedView::accept`)
and TypeScript (`validate.ts`) return the same codes for the shared fixtures in
`crates/aethyr-ui/fixtures`.

## Controllers

A controller is a `Definition`:

- `instance` and `initial` state;
- `render(state)`, a pure function from state to a root node;
- `handle(state, intent, node)`, an Effect that returns the next state. It
  may call services and may fail; on failure the state and revision stay as
  they were.

`Controller.make(definition)` needs the `AtomRegistry` service. State and the
published view live in one atom; `view` and `state` atoms derive from it, so a
renderer subscribes to `view`. `dispatch(intent)` checks the intent against
the current revision, runs `handle`, renders, validates, and publishes a new
revision only when the tree changed. Intents run one at a time in arrival
order. `update(f)` publishes a change that did not come from an intent, such
as a service result.

Services come from Effect layers. The entry demo uses `EntryOptions`,
`Navigator`, and `GuestMemory`; its tests provide recording layers and run
with no DOM. Domain authority stays in Rust services; a controller calls them
through services and never reimplements them.

## Renderers

A renderer draws any valid view and sends intents back. The React DOM
renderer (`AethyrView`):

- uses the design system: `hud-panel`, `hud-label`, `hud-button`,
  `hud-toast`, the `aui-*` layout classes and `hud-dialog` in `theme.css`, and
  the shadcn `Input` and `Textarea`;
- holds no state beyond DOM focus, hover, and a field's draft. A chip or row
  shows what the tree says until a new revision changes it. Escape on a
  dialog sends `dismiss`; the dialog stays open unless the controller closes
  it;
- keeps a field's draft while its `input` intents are handled, so a slow
  handler never drops keystrokes. One input is in flight at a time; when it
  settles, the field sends its newest draft against the newest revision. The
  draft ends when the tree shows it, or when the controller sets a value the
  field never sent (a reset). Hosts return the dispatch promise from
  `onIntent` so fields know when an input settled;
- names the revision it drew in every intent;
- keys children by node ID and keeps media and engine surfaces mounted
  across revisions through the host's `mount(kind, id, element)` callback;
- sends `activate` for a plain click on a link and lets modified clicks
  follow `href`, so links still work without the controller.

`ControllerView` (in `host.tsx`) reads the controller's `view` atom from the
shared registry and runs `dispatch` in the host's runtime. It drops `stale`
refusals, which are expected after a newer revision.

## Surfaces

### `/new-lander/verse` (#12588)

The server page (`crates/openagents-web/src/pages/new_lander.rs`) is a
shell: the planets' stage (`#engine`, `#world-notice`) inside
`#universe-exploration`, the `#verse-hud` mount point, the
`#verse-config` JSON the controller starts from, and two `.verse-bridge`
blocks. The Verse controller draws the entry dialog, the destination sheet
with its world chips, the map controls, and the fallback links into
`#verse-hud`.

The `.verse-bridge` blocks are the server's own copy of those controls. They
are the page for visitors without JavaScript (the fallback links work), and
they carry the element IDs the pinned engine release (`aethyrforge-host`'s
`mount_universe`) reads and writes: `#continue-guest`, `#verse-explore`,
`#option-*`, `#destination-*`, `#planet-*`, `#world-title`, and the rest.
Once the web host mounts, it hides them (`data-aethyr-verse` on the root),
keeps the release's own entry dialog from opening as a modal, turns intents
into clicks on the bridge, and folds what the release writes there, and its
`universeDemo.visit()`, back into the controller (`engineReport`). The
engine never reopens a dialog the visitor closed. When a release built from
this repository (#12590) stops writing to the page, the bridge goes.

Browser checks address the visible controls as `[data-aethyr-id=ID]`, with
the same IDs the bridge uses (`[data-aethyr-id=continue-guest]`,
`[data-aethyr-id=option-haunted-house]`, `[data-aethyr-id=planet-setup]`).

### Planet setup, invitations, and the player bar (#12588)

The same pattern serves three house pages. Each server page keeps its own
markup in `.house-bridge` and adds an empty `#house-hud`; once
`house-host.tsx` mounts, it sets `data-aethyr-house` on the root, which hides
the bridge, and draws the controller's tree into `#house-hud`.

| Page | Controller | What the bridge keeps |
| --- | --- | --- |
| `/new-lander/planet/{id}` | `makeSetupController`: the name field (a draft until saved), Save, and the courtyard link | The name form and its CSRF token. Save fills `#planet-name` and submits the form. |
| `/new-lander/invite/{world}/{release}` | `makeInviteController`: access and expiry as chips, Create, the invitation links, and End invitation on open ones | The invitation form, and one `.invitation-end` form per open row. Create sets both selects and submits the first; End submits the row's form. Rows carry `data-state`, `data-access`, and `data-expiry`. |
| `/new-lander/play/...`, `/new-lander/visit/{id}` | `makePlayerController`: back, title, version, Retry, and Full screen | The header with `#world-retry` and `#world-fullscreen`, which the release listens on and hides when it fails. The bar follows `hidden` on them. |

Visits are play only; they have no chat (owner, 2026-10-24). On a visit the
header carries `data-visit`, and `watchVisit` asks
`GET /v1/verse/studio/visits/{id}` every 15 seconds. A 410 (the host ended
the invitation, or it expired) or a 401 (a sign-in visit after sign-out)
closes the player (`versePlayer.close()`) and shows why in the bar.

The engine frame, `#world-notice`, and `#player-controls` stay outside the
bridge, as the release draws them. Browser checks
use `[data-aethyr-id=planet-name] input`, `[data-aethyr-id=planet-save]`,
`[data-aethyr-id=access-signed-in]`, `[data-aethyr-id=invite-create]`,
`[data-aethyr-id$=-end]` on an invitation row, and
`[data-aethyr-id=world-retry]`.

### The studio HUD (#12588)

On `/new-lander/house` and a courtyard (`/new-lander/planet/{id}/world`),
`makeStudioController` draws the HUD into `#house-hud`:

- the world notice: opening, or an error with Reload world and Back to
  planets. Without WebGPU the error says the browser can't show 3D worlds;
- the dock: Planets, Reload, Full screen (wide screens), Mesh, Edit, and
  Save, disabled while the editor is open;
- the voice bar (Listen, Stop / mute, status, what it heard, its answer), or
  the controls help line when voice is off;
- the world actions (Publish, Play, Share, and More), shown when the release
  offers them and the editor is closed or on Request.

The release's copies (`#world-notice`, `#world-controls`, `#world-voice`,
`#request-choices`) carry `.studio-bridge` and are hidden once the host
mounts (`data-aethyr-house="studio"`). The host clicks them by ID and reads
their `hidden`, `disabled`, `aria-pressed`, and text back through
`studioReport`. Visible nodes use the same IDs, so checks address them as
`[data-aethyr-id=world-edit]`, `[data-aethyr-id=edit-voice-status]`, and so
on. While the world shows an error the root carries
`data-aethyr-world="error"` and the frame is hidden, so the engine's own
message never shows beside ours. When the editor closes, focus returns to the
visible Edit button.

The editor (Jarvis: its tool windows and `#edit-panel`), the sign-in and
confirmation dialog (`#edit-auth`), publish review (`#publish-review`), the
share card, and courtyard settings (`#planet-settings`) are still the
release's own DOM. They use controls the protocol has no element for yet
(selects, checkboxes, ranges, color, file pickers, and a paint canvas).

## Design tokens

`crates/oa-tokens` defines each token once as a Rust constant
([ADR 0012](../adr/0012-one-token-crate.md)). Its `stylesheet` module writes
`tokens.css`: the shadcn colors on Coder Noir's roles (`noir`), with the hover
fill as a product role (`product::HOVER_FILL`); the HUD values in `hud`
(`--hud-glass`, `--hud-line`, `--hud-tick`, `--hud-glow`, `--hud-header` with
its phone value), the radii, and the label type; the Geist stacks
(`typography::geist`); and the `blur`, `motion`, and `space` scales.
`cargo run -p oa-tokens -- --write` writes `tokens.css`, including Tailwind's
`@theme inline` mapping, and `theme.css` imports it. Native and GPU renderers
read the constants (`Rgba8::from_hex` parses a color). The view tree's `gap`,
`padding`, and `tone` resolve to these tokens.

## Generated files and checks

| File | Regenerate | Check |
| --- | --- | --- |
| `aethyr.ui.v1.schema.json`, `protocol.ts` | `cargo run -p aethyr-ui -- --write` | `cargo test -p aethyr-ui`; `npm test` compares the schema digest in `protocol.ts` |
| `tokens.css` | `cargo run -p oa-tokens -- --write` | `cargo test -p oa-tokens` |
| `crates/aethyr-ui/fixtures/ts/*.json` | `AETHYR_WRITE_FIXTURES=1 npm test` | Both test suites |
| `tests/snapshots/aethyr-renderer.json` | `UPDATE_SNAPSHOTS=1 npm test` | `npm test` |

Both generators are listed in `.openagents/generated.json`. After changing
`theme.css` or the tokens, run `npm run build` in `apps/verse-ui` and commit
`ui.css` and `ui.js`.
