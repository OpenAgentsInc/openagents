# Web chat: GitHub tools

Issue: [#11167](https://github.com/OpenAgentsInc/openagents/issues/11167)
(orchestrator parity row C2). Code: `crates/openagents-web/src/github_tools/`
(pages, sealing, the thread card) over `crates/github-actions/` (the typed
actions, their command-line form, and the REST calls), which the CLI's
`openagents issue|project` verbs (#11166) share.

A signed-in person with GitHub connected sees **GitHub tools** under a web
chat's composer. `/chat/{id}/github` lists five tools, each a typed form:

| Tool | GitHub REST calls |
| --- | --- |
| Open an issue (optionally on a board, with a status) | `POST /repos/{o}/{r}/issues`, then the board steps below |
| Comment on an issue or pull request | `POST /repos/{o}/{r}/issues/{n}/comments` |
| Close an issue (optional comment first) | comment, then `PATCH /repos/{o}/{r}/issues/{n}` `state=closed, state_reason=completed` |
| Move an issue on a board | `GET /orgs\|users/{o}/projectsV2/{board}` + `/fields`, find the item (`/items?q={n}`) or add it (`POST /items`), `PATCH /items/{item}` Status |
| Open a pull request | `GET /repos/{o}/{r}` for the default branch when no base is given, `POST /repos/{o}/{r}/pulls` |

Boards use REST only, as `scripts/dev/issue-board.sh` does, so they keep
working when the GraphQL limit is spent.

## Confirm before every write

Sending a form shows a confirm card: the repository, one sentence per
change in the order they happen, and the text to be posted. The card
carries the action sealed with the server's key (HMAC over the JSON, bound
to the person and the chat, good for 30 minutes, run once). Confirm runs
exactly what the card showed. Nothing reads message words to pick a tool;
the form names its tool with one bounded value.

## Access, asked for only when needed

Running first reads `X-OAuth-Scopes` from `GET /user`:

- no `repo`/`public_repo`: stop, nothing changed, consent page;
- a board change without `project`: stop, nothing changed, consent page
  that says GitHub will be asked to let OpenAgents change boards.

The consent page's one button goes to `/auth/github/board`
(`oa_auth::Purpose::Board`, scopes `read:user repo read:org project`). The
card is kept in a short-lived cookie, and the trip returns to
`/chat/{id}/github/confirm`, which shows the same card again. A GitHub App
user connection sends no scopes header and goes straight on.

The person's token is read from the account service for that request only
(`CloudSession::github_token`) and never stored by the web server. What ran
is noted in the chat as a tool message with the GitHub link.

## The chat proposes a change

A message like "open an issue for the login loop and put it on board 22 as
Todo" goes through the chat router's command route, the same central
typed selector every surface uses (`crates/coder/src/cli_route.rs`):

1. Jev's `route` reading is `cli` and its `cli_group` reading is `issue`,
   `project`, or `pr` (`WEB_GROUPS` in `crates/coder/src/router/policy.rs`).
   On the website every other command group still becomes the model.
2. The descent picks the leaf with one Jev Choice per level; on the
   website only `github_actions::COMMANDS` are offered
   (`cli_route::gate::WEB_COMMANDS`). The model fills the free text
   (title, body), and the command's own parser checks the words.
3. The worker sends the `cli` offer with `cli.offer.website` ("Here's that
   change on GitHub. Nothing changes until you confirm it."), and the
   reply's `Meta::command` keeps the words, at NIP-CJ's bounds for GitHub
   commands (32 words, 256 bytes each, one line).
4. The thread shows the reply's card: `/chat/{id}/github/proposed/{index}`
   reads the stored command into an `Action` (`Action::from_argv`, exact
   parsing of the chosen command's fields; `--repo` defaults to the chat's
   project) and seals it like a form's card. Confirm runs it once.

Nothing reads the message's words to pick a tool.

## Shared with the CLI

`github-actions` has no CLI or web types:

- `Action::from_argv(argv, default_repo)` and `Action::argv()`: the
  `issue create|comment|close`, `project move|add`, and `pr open` words.
- `rest::run(api, &action)` for a whole action, and the steps
  (`create_issue`, `comment`, `close_issue`, `reopen_issue`, `place`,
  `find_board`, `open_pull_request`) for a verb's own composition, over
  any `rest::Api`: `rest::Http::new(API_BASE, token)` (feature `http`) or a
  fake.

## Left for later

- #11166's `coder::github_rest` (not on `main` yet) moves onto this crate
  when it lands, so there is one REST implementation.
- A body longer than one 256-byte line can't come through a chat proposal
  (NIP-CJ's `cli` offer bounds); the card links to GitHub tools to write
  it there.
- `pr open` has no CLI verb yet, so the chat can't propose it; the tools
  page has it.
