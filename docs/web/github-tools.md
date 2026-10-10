# Web chat: GitHub tools

Issue: [#11167](https://github.com/OpenAgentsInc/openagents/issues/11167)
(orchestrator parity row C2). Code: `crates/openagents-web/src/github_tools/`.

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

## Left for later

- Sharing these typed actions with the CLI verbs (the CLI issue).
- Letting the chat's model propose a filled card from a message (it would
  still go through the same confirm card).
