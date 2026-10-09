# Plugin cards in web chat

When an answer in the web chat at `/chat` is about which plugins to try,
the reply shows our real plugins as cards under a short, true sentence.
Before this, "Which plugin should I try?" (the **Test a plugin** starter)
got a model-written list of plugins we don't have.

## Where the plugins come from

One catalog, nothing hand-kept:

- `deploy/eval-runner/catalog` lists the plugin directories the Gym tests,
  in order.
- Each directory's `package.json` gives the plugin's `slug`, `name`, and
  one-line `summary`.
- `coder::gym_kb::CATALOG_PACKAGES` compiles those package records in, and
  `coder::gym_kb::catalog_plugins()` reads them.
  `crates/coder/tests/plugin_catalog.rs` fails if the two lists drift.

Today that is Project map, Code finder, Test reader, Explain this error,
Release notes, and Dependency check.

## What a card shows

`openagents_ui::content::PluginCard`, in a `PluginCards` list:

| Part | Content |
| --- | --- |
| Icon | One per plugin: Project map a map, Code finder a search glass, Test reader a flask, Explain this error a bug, Release notes a notepad, Dependency check a cube. An unknown slug gets the plugin icon. |
| Name | The package's `name`. |
| What it does | The package's `summary`, one line. |
| Where it runs | "With Coder on your computer." |
| Action | **Get Coder**, a link to `/download`. |

The website can't run a plugin or start a test, so the only action that
works here is getting Coder, which can. A card never shows a disabled or
placeholder button. If a surface someday starts a plugin's test itself,
its card's action becomes that ("Test it"), and only there.

The sentence above the cards comes from the answer bank and doesn't list
the plugins. The cards do that.

## How the server decides to show cards

Routing is semantic. Nothing matches words in the message.

1. Jev reads the message and picks the typed `eval.run` route ("test or
   try one of our plugins, or which plugin to try").
2. On the website (`Surface::Web`), `coder::router::policy::decide` turns
   `eval.run` into the bank entry `plugins.web`
   (`crates/coder/answers/chat-answers-v1.toml`). Elsewhere `eval.run` is
   unchanged: the apps offer to start a test.
3. `plugins.web` sets `plugins = true`. The worker's result for an entry
   like that carries the catalog's slugs in a typed field:
   `"plugins": ["project-map", "code-finder", ...]`
   (`coder::router::wire::Served::plugins`). The slugs come from the
   compiled-in catalog, never from a model.
4. `openagents_chat::router::Meta::resulted` keeps up to 12 short
   lowercase slugs, once each, and drops anything else.
   `suggestions::chip_meta` keeps them with the reply.
5. The web chat stores that record on the request (`Request::reply`;
   `chat_store` caps it at 16 slugs of 64 bytes).

## Rendering and streaming

- `pages/chat.rs` `turn()` draws an assistant message as its Markdown
  followed by `suggestions::plugin_cards(slugs)`. `message_plugins` takes
  the slugs from that message's answered request.
- Each slug is looked up in the compiled-in catalog, so the name and
  summary are the package's own words, escaped. An unknown slug draws
  nothing.
- While an answer streams, its request has no reply yet, so the text
  arrives as before. When the answer settles, the `transcript` SSE event
  re-renders the messages, and the cards show with the follow-up chips.
  A reload shows the same thing, because the slugs live in the stored
  chat.
- The phone and desktop apps ignore `plugins` for now. They keep their own
  Gym cards.

## Look

- `static/components/plugin-card.css`: a grid of
  `minmax(min(100%, 15rem), 1fr)` columns. That is one column at phone
  width and two or three as the thread widens, with no horizontal scroll.
- Colors use the semantic tokens, which resolve through `light-dark()`.
  The icon tile is `light-dark(var(--gray-100), var(--gray-200))`, so
  cards follow Coder Light and Coder Noir and the theme toggle, with no
  `data-theme` selectors.
- Strict CSP: no inline style or script. The action is a plain link.
- The catalog at `/ui` shows a **Plugin cards** section in both themes.

## Not yet

- A knowledge answer to "which plugins are there?" (the
  `openagents.plugin-list` note) is true text with no cards. Showing cards
  there would need the grounded tier to mark the note it answered from.
