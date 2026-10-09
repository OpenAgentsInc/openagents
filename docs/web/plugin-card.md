# Plugin cards in web chat

When an answer in the web chat at `/chat` is about which plugins to try,
the reply shows our real plugins as cards under a short, true sentence.
Before this, "Which plugin should I try?" got a model-written list of
plugins we don't have.

## Where the plugins come from

One list, of plugins that work today:

- `coder::builtin_plugins::BUILTIN_PLUGINS` names Coder's built-in
  plugins, each with its `slug`, `name`, one-line `summary`, and the code
  and tests that show it works. Its own test fails if a cited file is
  missing.
- `crates/coder/tests/plugin_catalog.rs` holds the plugin list note and
  `docs/plugins/README.md` to it, and fails if any product note shows a
  sample plugin.

Today that is Claude Code, Codex, Cursor, and Grok Build (the `openagents`
terminal's ACP delegation, `crates/coder-new/src/acp_discovery.rs` and
`bundled_runtime.rs`) and OpenRouter (bring your own key,
`crates/coder-new/src/plugin_definition.rs`).

The hosted runner's sample packages (`deploy/eval-runner/catalog`,
`crates/plugin-*`) are its test fixtures and are never shown.

## What a card shows

`openagents_ui::content::PluginCard`, in a `PluginCards` list:

| Part | Content |
| --- | --- |
| Icon | One per plugin: Claude Code the assistant mark, Codex code brackets, Cursor a cursor, Grok Build the agent mark, OpenRouter a key. An unknown slug gets the plugin icon. |
| Name | The plugin's `name`. |
| What it does | The plugin's `summary`, one line. |
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
   (`crates/coder/answers/chat-answers-v1.toml`). Elsewhere `eval.run`
   answers from the Gym's records; with no plugin there to test, it says
   so (`eval.run.none`) and names the built-in plugins.
3. `plugins.web` sets `plugins = true`. The worker's result for an entry
   like that carries the built-in plugins' slugs in a typed field:
   `"plugins": ["claude-code", "codex", ...]`
   (`coder::router::wire::Served::plugins`). The slugs come from the
   compiled-in list, never from a model.
4. `openagents_chat::router::Meta::resulted` keeps up to 12 short
   lowercase slugs, once each, and drops anything else.
   `suggestions::chip_meta` keeps them with the reply.
5. The web chat stores that record on the request (`Request::reply`;
   `chat_store` caps it at 16 slugs of 64 bytes).

## Rendering and streaming

- `pages/chat.rs` `turn()` draws an assistant message as its Markdown
  followed by `suggestions::plugin_cards(slugs)`. `message_plugins` takes
  the slugs from that message's answered request.
- Each slug is looked up in the compiled-in list, so the name and
  summary are its own words, escaped. An unknown slug, such as an old
  sample plugin's in a stored chat, draws nothing.
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
