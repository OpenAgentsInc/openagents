# Web sidebar: chats, projects, and live status

Status: design, 2026-10-08. Phase 1 is being built; later phases are design.

The web app's left panel today has **New chat** (⌃N), the recent chats list
(`openagents_ui::shell::ChatList`, filled by `pages::chat::chat_list`),
Docs, and the account menu. This doc is the plan for turning it into the one
place where chatting and coding live together: a chat can be connected to a
GitHub repository, run work on an environment or a computer, and the sidebar
shows what each chat is doing without opening it.

Terms follow the [glossary](../glossary.md): a **chat** is one conversation;
a **project** scopes repository work; an **environment** is a saved setup a
task runs in; a **computer** executes; a **task** is one unit of work started
from a chat (a Cloud job or a Coder task on a connected computer).

## Information architecture

Top to bottom:

1. **New chat** (⌃N), then **Search** (⌘K / ⌃K, phase 4).
2. **Pinned**: chats the person pinned, in the order they pinned them.
   Hidden when empty.
3. **Projects** (phase 2): one collapsible group per project. A project is
   a connected GitHub repository (later: several repositories, or none).
   Each group lists its chats, newest activity first, five at a time with
   **Show more**. The group heading shows the project name and, when any of
   its chats is running or needs the person, one status dot (the most urgent
   of its chats). Groups remember their open or closed state per browser.
4. **Chats**: every chat with no project, newest first. Before projects
   exist this is the only list, as today.
5. Docs and the account menu at the bottom, unchanged.

Archived chats leave the sidebar; **Archived** in Settings lists them with
**Restore** (phase 4). The open chat always stays visible, even inside a
closed group.

Order inside a list: activity time, which is the latest of the last message
sent, the last answer, and the last task change. Pinned order is the
person's own. Rows never jump while the pointer is over the list; a pending
reorder applies when the pointer leaves.

## A chat row

```
Fix the login redirect                      ● Working
acme/storefront · fix-login · Node 20 v3
```

- **Line 1**: the title, cut with an ellipsis, and at the end a status
  (`openagents_ui::shell::ChatStatus`) or, with no status, nothing.
- **Line 2** (only when there is something to say): repository · branch ·
  environment and its version, joined with " · ", cut with an ellipsis.
  Inside a project group the repository is dropped (the heading says it).
  A chat with no repository has no second line.
- Long-running tasks (phase 3) add a thin progress line under the row only
  when the task reports steps ("3 of 7"); never a fake percentage.
- Collapsed to the rail (or the closed narrow panel), rows hide like other
  nav labels; the open chat's status dot stays on its rail icon when the
  rail shows chat icons.

### Status words

One small vocabulary, in plain words, most urgent first:

| Status | Shown when | Color |
| --- | --- | --- |
| Waiting for you | a task asks a question, needs an approval, or needs the person to sign in to Claude or GitHub | warning |
| Failed | the last answer or task did not finish | danger |
| Working | an answer is being written or a task is running (queued, starting, running) | info, pulsing dot |
| Paused until 3:40 PM | a task hit a usage limit and continues by itself at that time | quiet |
| Done | a task that ran for more than a minute finished and the person has not opened the chat since | success |
| (nothing) | everything else | |

Done clears when the chat is opened. Failed stays until a new message is
sent. Answers that take a few seconds never show Done; the chat simply
updates.

Where each status comes from:

| Source | Working | Waiting for you | Paused until | Done / Failed |
| --- | --- | --- | --- | --- |
| Chat answer (`chat_store` `pending`, last request outcome) | `pending` set | | | Failed: outcome `failed` |
| Cloud job (coder-cloud `State`) | created, provisioning, resuming, ready, dispatching, running | needs Claude sign-in | `paused` with the reported reset time | completed / failed (cancelled shows nothing) |
| Coder task on a connected computer (task reads) | running | a question or approval in the task | | finished / failed |
| Environment setup chat | building, verifying | asks for a secret or a choice | | saved / failed |

## Interactions

- **Row menu** (the `…` that shows on hover or focus, and on long press):
  Pin / Unpin, Rename, Move to project, Archive. Delete stays out until
  deletion is a real, complete flow.
- **Rename** edits the title in place: Enter saves, Escape cancels.
- **Archive** is undoable from a short notice ("Chat archived · Undo").
  Archiving a chat whose task is still working asks first.
- **Search** filters titles as you type, then matches message text (two
  characters or more). Arrow keys move, Enter opens, Escape clears.
- **Keyboard**: ⌃N new chat; ⌘K / ⌃K search; ⌃⇧[ and ⌃⇧] previous and
  next chat; rows are links, so Tab and Enter work everywhere.
- **Move to project** offers the person's projects and **No project**.

## Projects and repositories

- A project is created by connecting a GitHub repository after GitHub
  sign-in ([auth](../auth/README.md)). Sign-in asks only for identity. Seeing
  private repositories needs the `repo` scope (and `read:org` for
  organization repositories); we ask for it the first time the person
  connects a private repository, with a plain sentence saying why, and the
  connect screen works for public repositories without it. A GitHub App
  install with per-repository access is the better long-term answer and
  replaces the broad scope when it lands.
- A chat joins a project when it is started from that project (the project
  menu's **New chat**), when its repository picker selects that repository,
  or by **Move to project**. The chat stores the project id; the repository
  and branch it worked on stay on the chat as today
  (`Selection::repository`).
- An environment belongs to a repository; a chat that runs a task in an
  environment shows the environment name and version on line 2.

## Live updates

- One server-sent events stream per signed-in tab for the sidebar
  (`/chats/events`), next to each chat's own stream. It sends a replacement
  row (or group heading) when a chat's title, status, or line 2 changes,
  swapped by id with HTMX; it never polls per row.
- The stream reads the same status source the chat page uses: the chat
  store's revision for answers, and the task record changes the Cloud
  operator and the Coder task reads already produce. A change wakes the
  stream; there is no timer per chat.
- The stream sends only the owner's own rows and closes on sign-out. A tab
  in the background keeps the connection; a reconnect asks for rows changed
  after its last revision.

## Narrow screens

The sidebar is a sheet over the page. Rows keep both lines; the status
shrinks to its dot under 360 px wide with the words still read aloud.
Opening a chat or starting one closes the sheet. Group headings and the row
menu need no hover: the `…` is always visible on touch.

## Privacy

- Repository, branch, environment, and project names are shown only to the
  chat's owner (and, later, project members). The visitor-scoped list for
  someone not signed in shows only that visitor's own chats, as today.
- Shared or public chat pages never render the sidebar of the person who
  shared them.
- Nothing in a row is sent to analytics.

## Phases

1. **Row line and status** (now): `NavItem::detail` and `ChatStatus` in
   `openagents-ui`; the chat list shows repository · branch from the chat's
   selection, Working while an answer runs, Failed when the last one failed.
2. **Projects**: connect a GitHub repository, a project record, chats join a
   project, sidebar groups.
3. **Live status**: the sidebar event stream; Cloud job and Coder task
   statuses (Waiting for you, Paused until, Done) on rows.
4. **Organize**: pin, rename, archive, search, keyboard.
5. **Environment and task links**: a chat records the environment and the
   tasks it started; line 2 shows the environment and version; a long task
   shows its steps.
