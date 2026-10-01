# The route map

Status: built on `main` for the desktop app
([#10085](https://github.com/OpenAgentsInc/openagents/issues/10085)). The
shared model is ready for the phone; the phone has no Map page yet.

The Map page shows OpenAgents' composition as one zoomable graph: the
front (the chat router), its typed routes, what serves each route, and the
gaps between them. It is the visible form of the "agent of agents" in
[The Return of the General Agent](../essays/2026-10-01-the-return-of-the-general-agent.md):
a general front, specialist members, and the places where OpenAgents is
thin. Open it from **Map** in the sidebar's footer (beside Verse), from
the command palette (**Open the map**), from the Mac's **Window** menu, or
from chat on the desktop: ask "show me how you route things", and the Map
page opens when the reply arrives, with no tap. Only the router's typed
`open_screen` offer for `routes.map` (bank entry `meta.map.desktop`) opens
it, and only for a reply to a message sent from this window; the reply's
words never do. The reply's **Open the map** button stays, to open the page
again. The phone and the terminal answer with a sentence and never open it.

## One graph, colored by kind

Everything is on one graph; each node kind has its own color and a row in
the legend (`openagents_chat_app::visual::map`):

| Kind | What it is | Where it comes from |
| --- | --- | --- |
| Router | The front every message reaches first | `chat-router-v4` and the bank's identity |
| Route family | Answers, Work, The Gym, Screens and actions, Boundaries | `RouteId::family` in `crates/coder/src/router.rs` |
| Route | One of the router's typed routes | `RouteId::ALL` and the rubric (`router/rubric.rs`) |
| Prepared answer | An answer-bank entry, under the first route it answers | `crates/coder/answers/chat-answers-v1.toml` |
| Knowledge | Product knowledge under `product.kb`, the repository's code under `codebase.kb`, the coding knowledge under Coder | `knowledge/openagents/`, `codebase-questions-v1.json`, `knowledge/` |
| Chat model | The general model that writes a reply when nothing prepared fits | the bank's `general` route and the router's fallback |
| Coder | The coding agent work is handed to | the `chat.coder` capability |
| Engine | Codex, Claude Code, Grok Build, OpenCode, Devin | NIP-CJ's `Engine::ALL`, with this computer's readiness |
| Plugin | What a person adds: Wasm, workflows (programs), skills, knowledge, and tests in one package | every `crates/plugin-*` and `packages/*` directory |
| Screen or action | A screen an answer's offer opens, a deck, the command offers | the bank's `open_screen` offers, `openagents_deck::decks()` |

Edges follow a request: router → family → route → what serves it → engine
or plugin. Coder *admits* plugins; `eval.run` *tests* them (dashed);
answers *open* screens. Health is the ring, never the fill: no ring is
measured and good, a red ring is measured and weak (held-out precision
below 85 % or recall below 80 % on at least 5 rows), and a dashed ring is
not measured. A red dot marks a gap.

## Where the facts come from

The phone and the desktop don't link the router, so its facts reach them
as one committed snapshot, `crates/openagents-chat-app/src/route_map/sources.json`.
`crates/coder/tests/route_map_sources.rs` writes it from the live code and
data and fails when the committed file differs, so the map never shows a
hand-copied list:

```sh
cargo test -p coder --test route_map_sources                      # check
ROUTE_MAP_WRITE=1 cargo test -p coder --test route_map_sources    # rewrite
```

A plugin's evaluation status comes from published records only:
`records.json` holds the relay's verified NIP-EVAL results, checks, and
validations (`3189`), the NIP-EXT releases they name (`3184`), and the
`coder-defaults` adoption, admitted by `nostr::eval_ext::parse_publication`
and `linkage`. A result belongs to a plugin when its subject is that
plugin's package (publisher and slug from its `package.json`). Rewrite it
with the ignored live test:

```sh
ROUTE_MAP_WRITE=1 cargo test -p coder --test route_map_sources live_route_map_records -- --ignored
```

The ladder a plugin climbs: **Not packaged** (code without a package record
or tests), **Candidate** (packaged, no published result), **Better**, **No
clear change**, or **Worse** (a result), **Reproduced** (a Better result
another trainer's check confirmed), **Validated** (also Better on a second
test set), **Adopted** (in Coder's defaults for everyone). As of
2026-10-01: Project map is Adopted; Code finder and Test reader are
Reproduced; Explain this error, Release notes, and Dependency check
(#10086) are Better and wait for a check; Outline is Not packaged; and
Jev-probe is a Candidate. Explain this error is marked as the example to
copy (`SHOWCASE` in the sources' builder): its details offer **Make one
like this**, a new chat with "Help me make a plugin like Explain this error
that " in the composer.

What only this computer knows comes in at build time and never leaves it:
how often this person's own chats took each route (`Command::Routes`,
counted by the host from the saved chats' typed judgments, read-only) and
the engines' readiness from this computer's Coder (#10018).

## Gaps

Each gap is a typed fact (`openagents_chat_app::route_map::GapKind`), with
its evidence and one next step that runs an existing path after a tap:

| Gap | When | Next step |
| --- | --- | --- |
| Requests nothing serves | `capability.missing` has labeled examples (and, locally, your own turns) | **Draft a plugin in chat**: a new chat with "Help me make a plugin that " in the composer (the Gym's `eval.author` path) |
| Answered by the chat model alone | an answers route with no prepared answer or knowledge | **Write a knowledge entry**: copies `microcoder kb add …` |
| Product questions with no entry | the product KB question set's unanswerable questions | **Write a knowledge entry** |
| Weak held out; no held-out numbers; few labeled examples | below the floors, missing from the latest per-route record, or under 25 rows | **Add labeled examples**: opens a prefilled GitHub issue (rows go in `routes-v4.json` with a recalibration) |
| We ask you back often | over 15 % of at least 20 of your replies went to `clarify` | **Add labeled examples** |
| No package or tests; no published result | a plugin at Not packaged or Candidate | copies `openagents ext eval init` or `run`, or for a catalog plugin **Test it in chat** |
| Check, validate, one step from adoption, didn't help yet | the ladder's next rung | **Check it in chat**, **Write a second test set**, the operator's adoption command, or a rerun |
| Engine not signed in or at its limit | this computer's readings | **Sign in** / **See usage**: Settings → Coder |

The map never runs a step by itself: a chat step leaves its message in the
composer unsent, a command step copies the command and names its page, an
issue step opens the browser.

## Interaction

- Drag or two-finger scroll pans; pinch, Cmd or Ctrl with the wheel, Cmd
  `+` and `−`, or the toolbar zoom; Cmd `0` or **Fit** fits; a
  double-click zooms into a node and its members.
- A click selects a node and opens **Details**: what it is, why the router
  sends things there, its numbers each linked to its record, its members,
  its gaps, and next steps. A plugin's details list its parts and every
  result, check, validation, and adoption.
- Arrows move to the nearest node that way, Tab and Shift-Tab step through
  the outline's order, Enter zooms into the selection, and Esc steps out to
  the parent; Esc with nothing selected gives the keys back to the window.
- Filters: family, kind (prepared answer, knowledge, plugin, engine,
  screen), gaps only, not measured only. Filtered-out nodes stay in place,
  dimmed.
- Labels thin with zoom (`layout::detail`): families always, routes from
  mid zoom, members closer, evidence lines zoomed in; a selection's
  neighbors are always named, and a label that would overlap one already
  placed waits for a closer zoom.
- **Outline** lists every node in the tree's order as a button named for
  screen readers, and the surface describes every node to AccessKit with
  its bounds, so VoiceOver can select one.
- The camera eases over 260 ms on the frame clock; with Reduce motion
  (system or Settings) it moves at once.

## Architecture

- `openagents_chat_app::route_map`: the graph, gaps, inspector content,
  accessible names, and filters, deterministic and with no network.
  `route_map::layout`: the radial tree (positions depend only on the tree,
  so they hold across refreshes), the camera, hit-testing,
  level-of-detail, and arrow-key stepping.
- `openagents_desktop::route_map`: the page. The graph paints into one
  surface, `route-map`, with its own line rasterizer (cost in proportion to
  length) and clipped drawing; the toolbar and the side panel are normal
  Rust Native views. The side panel scrolls with the wheel and pages the
  Gaps list.
- The window builds the page when the Map opens and drops it when the
  person leaves (`sync_map` in `shell.rs`), so no other page pays for it.

## Not done

- No phone page yet; the model is shared and ready.
- `records.json` is a committed snapshot; the window doesn't refresh it
  from the relay while it runs.
- The 60 fps target is measured only as a debug-build headless paint; a
  person's pass at default and minimum sizes is in the verification
  record's owner checks.

Verification: [captures, tests, the release gate, and timing](verification/2026-10-01-route-map/verification.md).
