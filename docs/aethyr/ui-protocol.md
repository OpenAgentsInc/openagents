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
| Design tokens | `crates/aethyr-tokens`, written to `apps/verse-ui/src/aethyr/tokens.css` |
| Controller runtime | `apps/verse-ui/src/aethyr/controller.ts` |
| React DOM renderer | `apps/verse-ui/src/aethyr/renderer.tsx`, joined to a controller by `host.tsx` |
| Demo | `apps/verse-ui/src/aethyr/entry.ts`, the `/new-lander/verse` entry dialog |

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
  | `chip` | A small toggle with explicit `selected` | `activate` |
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
- holds no state beyond DOM focus and hover. A chip, row, or field shows what
  the tree says until a new revision changes it. Escape on a dialog sends
  `dismiss`; the dialog stays open unless the controller closes it;
- names the revision it drew in every intent;
- keys children by node ID and keeps media and engine surfaces mounted
  across revisions through the host's `mount(kind, id, element)` callback;
- sends `activate` for a plain click on a link and lets modified clicks
  follow `href`, so links still work without the controller.

`ControllerView` (in `host.tsx`) reads the controller's `view` atom from the
shared registry and runs `dispatch` in the host's runtime. It drops `stale`
refusals, which are expected after a newer revision.

## Design tokens

`crates/aethyr-tokens` defines each token once as a Rust constant: the shadcn
colors and radius, the HUD values (`--hud-glass`, `--hud-line`, `--hud-tick`,
`--hud-glow`, `--hud-header` with its phone value), type, blur, motion, and the
spacing scale. `cargo run -p aethyr-tokens -- --write` writes `tokens.css`,
including Tailwind's `@theme inline` mapping, and `theme.css` imports it.
Native and GPU renderers read the constants (`aethyr_tokens::rgba` parses a
color). The view tree's `gap`, `padding`, and `tone` resolve to these tokens.

## Generated files and checks

| File | Regenerate | Check |
| --- | --- | --- |
| `aethyr.ui.v1.schema.json`, `protocol.ts` | `cargo run -p aethyr-ui -- --write` | `cargo test -p aethyr-ui`; `npm test` compares the schema digest in `protocol.ts` |
| `tokens.css` | `cargo run -p aethyr-tokens -- --write` | `cargo test -p aethyr-tokens` |
| `crates/aethyr-ui/fixtures/ts/*.json` | `AETHYR_WRITE_FIXTURES=1 npm test` | Both test suites |
| `tests/snapshots/aethyr-renderer.json` | `UPDATE_SNAPSHOTS=1 npm test` | `npm test` |

Both generators are listed in `.openagents/generated.json`. After changing
`theme.css` or the tokens, run `npm run build` in `apps/verse-ui` and commit
`ui.css` and `ui.js`.
