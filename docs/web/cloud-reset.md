# Cloud web reset

Owner direction (2026-10-08): nuke the `/cloud` pages and start over with a
few simple top-level pages. Target: ChatGPT parity. A left sidebar with New
chat, chats, and a few destinations; the account menu bottom-left holds
Settings, Billing, Docs, and Sign out.

## Deleted

Every `/cloud/app/*` page and the code that only rendered pages:

- the workspace shell (sidebar of twelve sections, workspace switcher,
  overview cards, "Unavailable" sections) and `/cloud/app/session`
- hosts, computers, projects, agents, Studio, workbench, Verse and worlds,
  tasks, plugins, team, recovery, billing statements, retail purchases,
  sales, sales floor, partners
- the host operation pages (computers, tasks, project jobs, the environment
  panel, request review) and the chat composer's hand-off to them: chat no
  longer offers a connected computer, and a chat that named one answers `410`
- their page tests, `cloud.css`, `cloud-start.js`, and the Wasm privacy
  guard the pages loaded

Old URLs answer `303`: `/cloud/app/settings/claude` to `/settings/claude`,
`/cloud/app/settings` to `/settings`, everything else under `/cloud/app` to
`/`. `/cloud/sign-in` redirects to `/sign-in`.

## New information architecture

Sidebar (top level):

| Entry | Path | When |
| --- | --- | --- |
| New chat | `/` | now |
| Environments | `/environments` | built separately: repo, live setup chat, saved environment, run Claude Code in it |
| Agents | `/agents` | only once there is a real flow behind it |

Account menu (bottom left):

| Entry | Path | Contents |
| --- | --- | --- |
| Settings | `/settings` | name, email, avatar; theme; link to Claude credential |
| Claude credential | `/settings/claude` | add or remove your own Anthropic API key or Bedrock, Vertex, or Foundry credential |
| Billing | `/billing` | omitted until a real payment flow exists |
| Docs | `/docs` | unchanged |
| Sign out | `POST /sign-out` | unchanged behavior, new path |

Sign-in moves to `/sign-in` (`GET` form, `POST` submit). Every page renders
through `UiPage` with `openagents-ui` components and plain copy.

## Kept as services

Non-page code the new pages and `/environments` need, under
`crates/openagents-web/src/cloud/`:

- `session.rs`: account sessions, cookies, CSRF
- `private.rs`: protected private files
- `custody.rs`: the credential vault
- `byo.rs`: Claude credential storage and release to the customer's own runs
  (its page moves to `/settings/claude`)
- `hosts.rs`, `effects.rs`: host bindings (the native operation client)
  and the request journal, for `/environments` to run on
- `account.rs` (crate root): who is signed in, for the account menu

## Order

1. This plan, and the tracking issue.
2. Teardown and the moves above in one change; old URLs redirect.
3. `/environments` lands (separate work) and fills the sidebar slot
   (`ui_page::Nav::Environments`).
4. Agents joins the sidebar only when a real flow exists.
