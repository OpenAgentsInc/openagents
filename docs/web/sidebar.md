# Web sidebar: chats, projects, and live status

Status: design, 2026-10-08. Phases 1, 2, and 4 are built; 3 is built for
answers and Claude Code runs; 5 is built for Claude Code runs.

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

Archived chats leave the sidebar; an **Archived chats** link under the list
(shown only when there are some) opens `/chat/archived`, which lists them
with **Restore** (phase 4; signed out, chats belong to the browser's
visitor cookie, so the page is not under Settings, which needs sign-in).
The open chat always stays visible, even inside a closed group.

Whose chats the list shows (#11039, `crates/openagents-web/src/chat_owner.rs`):
signed in, the account's, on any browser; signed out, the browser's
`oa_visitor` cookie's. Signing in moves the browser's signed-out chats to
the account once (a chat still being answered stays). **Delete all chats**
(`/chat/delete-all`, one confirm step) is in Settings when signed in, and
on a chat's delete step and the Archived chats page when signed out.

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
- A Coder chat synced from a terminal (#11046, #11047,
  `crates/openagents-web/src/coder_sync.rs`) says "Terminal · <computer>"
  on line 2, shows Working while Coder's heartbeat says it is replying,
  and opens at `/chat/{id}`. While Coder on that computer has checked in
  within the last minute (sync on), the page has a composer ("Reply to
  Coder on <computer>"): the reply waits on the chat ("Waiting for Coder
  on <computer>.") until Coder takes it, runs the turn there, and syncs the
  answer back (#11048). Otherwise "This chat runs in Coder on <computer>.
  To reply here, open Coder there with /sync on." stands in place of the
  composer. When the chat is in a project whose repository has a saved
  environment (local address, environments set up, and a Claude key saved
  in Settings or set on the server, like #11037), the offline note adds
  **Continue on a Cloud computer**
  (`/chat/{id}/continue`, #11050, `pages/chat_continued.rs`): the person's
  message joins the chat, Claude Code runs in that environment's saved
  version with the chat so far in its prompt, the thread shows a "Cloud
  computer" task row, and the run's answer joins the chat when it is done.
  Coder's uploads keep those messages after its own transcript until
  Coder, back online, takes them into its own copy with its next take
  (#11052); each is then shown once, where Coder put it.
  Deleting it on the web deletes it in Coder the next time Coder checks.
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
  Pin / Unpin, Rename, Move to project, Archive, Delete.
- **Delete** opens the same confirm step as the chat page's **Delete chat**
  (`/chat/{id}/delete`: "Delete this chat? This can't be undone." with
  Delete and Cancel). It removes the chat from the store for good, and waits
  while an answer is still being written. There is no undo.
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
- As built (#11035): `GET /chats/events?after=<unix time>&working=<ids>`,
  held by a hidden `#chat-sidebar-live` element that a whole page draws
  beside the list (an out-of-band list replacement leaves it alone). Each
  event (`status`) carries row status slots (`#chat-row-status-{id}`) out of
  band. The chat store announces this process's writes
  (`chat_store::Store::changes`); the stream waits a second for a burst to
  settle and reads only the chats that changed. Chats last seen Working are
  read again every 5 seconds and the whole list every 2 minutes, for writes
  on another replica. A connection lasts 10 minutes; the browser then
  reconnects with the last event id (a unix time) and gets the rows changed
  since.

## Narrow screens

The sidebar is a sheet over the page. Rows keep both lines; the status
shrinks to its dot under 360 px wide with the words still read aloud.
Opening a chat or starting one closes the sheet. Group headings and the row
menu need no hover: the `…` is always visible on touch.

## Privacy

- Repository, branch, environment, and project names are shown only to the
  chat's owner (and, later, project members). Signed in, the list is the
  account's chats; signed out, only that browser's own chats.
- Shared or public chat pages never render the sidebar of the person who
  shared them.
- Nothing in a row is sent to analytics.

## Phases

1. **Row line and status** (now): `NavItem::detail` and `ChatStatus` in
   `openagents-ui`; the chat list shows repository · branch from the chat's
   selection, Working while an answer runs, Failed when the last one failed.
2. **Projects** (built, #11034): `/projects` connects GitHub
   (`docs/auth/github.md`, "Repository access") and adds repositories as
   projects, kept by the account service. `chat_store` keeps `project`;
   the composer's project picker (`/?project=` preselects it), the row
   menu's Move to and Remove from entries (`POST /chat/{id}/project`, or
   the `/chat/{id}/project` page with many projects), and
   `openagents_ui::shell::ChatGroup` groups (`oa_project_groups` cookie for
   closed ones; Reconnect GitHub when access ended). Status dots on group
   headings wait for phase 3.
3. **Live status**: the sidebar event stream; Cloud job and Coder task
   statuses (Waiting for you, Paused until, Done) on rows. The stream is
   built for answers (#11035, `pages/chat_live.rs`): Working and Failed
   change on their own. Claude Code runs started from a chat (phase 5)
   make its row Working and Failed too, and Done when a run that took over
   a minute finished and the chat wasn't opened since (the chat keeps the
   task's `finished_unix` and, only while that Done waits, `opened_unix`).
   Synced Coder chats are Working while Coder says it is replying. Still
   without a source: no chat names a Coder task on a connected computer
   (`Pending::job_id` is always empty and running a chat on a computer is
   gone), and chat runs neither ask anything nor pause for a usage limit
   (they run on the person's API key, without the operator that records
   sign-in prompts and limit pauses), so no Waiting for you or Paused until.
4. **Organize** (built, #11036): pin, rename, archive, search, keyboard.
   `chat_store` keeps `pinned_unix` and `archived_unix`; the routes are
   `POST /chat/{id}/pin|archive|rename`, `GET /chat/{id}/rename` (the
   field), `GET /chat/list?q=` (search), and `GET /chat/archived`
   (`pages/chat_sidebar.rs`). The parts are `openagents_ui::shell`'s
   `RowMenu`, `RowRename`, and `ChatSearch`.
5. **Environment and task links** (built for Claude Code runs, #11037,
   `pages/chat_work.rs`): a chat about a repository with a saved
   environment (its project's repository, else the composer's) offers
   **Run Claude Code** in the header, on the local address only (like
   `/environments`; the site guard keeps `/chat/{id}/claude` local and
   sets `x-openagents-local` for pages that link it). Starting a run
   records `environment` (id, repository, version) and a `tasks` entry
   (run id, title, state, the message count it followed) on the chat.
   The header's breadcrumb names the environment and version and links
   it; the thread shows each task as a compact `openagents_ui::shell::TaskRow`
   after the message it followed, with Working / Paused / Done / Failed /
   Stopped and a link to the run; line 2 ends with "Environment v3"; the
   row is Working while a task runs and Failed when the newest task failed
   until a new message. The run's own record holds its state; a watcher
   while it runs, every chat load, and the sidebar stream's Working checks
   write a change into the chat (`work::sync`), and the chat store's change
   announcement carries it to open pages. An environment that no longer
   exists is marked removed when the chat opens ("Environment removed";
   the chat stays readable). Runs also start from the composer's Where it
   runs selector ("Composer selector row" below), and their answer joins
   the chat. Not built, for lack of a source: steps ("3 of
   7"; runs report none), Waiting for you and Paused until (runs ask
   nothing and don't pause), and an environment's setup conversation as a chat
   (it lives in the environments studio, not the chat store).

## Composer selector row

`crate::composer_row` puts compact selectors above the home and chat
composers. Each shows only when it can work for this person on this server;
a signed-out visitor sees the plain composer.

| Selector | Shown when | Choices | Sending with it |
| --- | --- | --- | --- |
| **Project** | signed in | the person's projects and No project; with none, one item: Connect a GitHub repository (`/projects`) | the chat records `project` |
| **Branch** | a project is picked | that repository's branches, read from GitHub as the person and kept (`composer::branch_names`); default branch first and preselected | the chat records `branch` (line 2 shows it) |
| **Where it runs** | more than Chat is possible | **Chat** (answers here); **Claude Code in {repository name} vN** when the project's repository has a saved environment, a Claude key is available, and the request came to the local address; **Coder on {computer}** for a new chat while Coder with sync on checked in from that computer lately | Chat: answered as before. Claude Code: the message starts a run in that environment (`chat_work::begin`; the branch is named in the prompt when it isn't the environment's, and the chat so far is carried in), recorded as the chat's environment and a task; the run's answer joins the chat when it is done. Coder: a new Coder chat (session `web-{request id}`) whose first message waits for Coder like a reply (`coder_sync::start_from_web`); Coder takes it at its next check-in, opens a new conversation under that id, answers, and syncs it back |

The picks travel as three `#chat-form` fields (`project`, `branch`,
`target`). Dropdowns load their choices into `#composer-panel`
(`GET /composer/row/{kind}`); a choice reloads the row (`GET /composer/row`)
with focus back on its dropdown, and the current choice takes focus when a
panel opens. Sending checks every pick again and refuses one that no longer
works (a project that isn't the person's, a branch the repository doesn't
have, an environment or computer that isn't offered) instead of answering
some other way. A chat's row starts from its project and branch, and from
Claude Code when its newest message started a run there.
