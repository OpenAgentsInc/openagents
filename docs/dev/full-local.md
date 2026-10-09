# Test the whole web stack locally

One command runs the 1.0 web stack on this Mac the way production runs it:
the gateway as the account service and the inference gateway, a chat worker
whose model calls go through that gateway, the website, and a Coder that
signs in to it. Nothing here touches staging or production services.

```sh
scripts/dev/full-local.sh start     # build, start, return (about 2 minutes warm)
scripts/dev/full-local.sh status    # what runs, where
scripts/dev/full-local.sh stop
```

- Website: <http://127.0.0.1:4301> (the local GitHub OAuth App returns to
  this address, so the site must be on 4301; if something else holds 4301,
  `start` says what and stops without touching it).
- Gateway: `127.0.0.1:8791`. Inference dashboard at `/admin/inference`
  with the token in `~/.openagents/full-local/admin.token`.
- Coder: `~/.openagents/full-local/bin/coder`, this checkout's `coder-new`
  pointed at the local site with its own sign-in and chats, so your real
  Coder is untouched.
- Everything (accounts, chats, saved keys, logs) persists in
  `~/.openagents/full-local/` across restarts. Logs:
  `~/.openagents/full-local/logs/{gateway,worker,web}.log`.
- Binaries build into `~/work/openagents-target-fulllocal`. `start
  --no-build` skips the build.
- `/environments` is on when `~/work/.secrets/boat.env` and a Codex login
  exist. Its setup machines are real Boat machines and cost money;
  `FULL_LOCAL_ENVIRONMENTS=0 scripts/dev/full-local.sh start` turns it off.

## Click through

Each step says what you should see. Start signed out.

1. **Sign in with GitHub.** Open <http://127.0.0.1:4301>, click **Log in**,
   then **Continue with GitHub**, approve on github.com. You come back
   signed in: your GitHub name and picture in the top right.
2. **Connect repositories.** Open **Projects** (`/projects`). Click
   **Connect GitHub** (leave **Public repositories only** checked, or
   uncheck it to include private ones), approve on GitHub. Your
   repositories list, newest first; **Add** one. It shows under Projects
   and in the sidebar.
3. **Start a chat in a project.** From the project, ask something (for
   example "What does this repository do?"). The answer streams in under
   "Working"; the chat is listed under the project in the sidebar.
4. **Pin, archive, delete.** In the sidebar, open a chat's **⋯** menu:
   **Pin** moves it under Pinned; **Archive** moves it to Archived chats;
   **Delete** asks "Delete this chat? This can't be undone." and removes it.
5. **Claude key.** Account menu → **Settings** → Claude → **Manage**. Pick
   Anthropic API key, paste a key, check **Save this key for my account**,
   **Save**: it shows "Saved: Anthropic API key" with dots, never the key.
   **Remove**: it shows "Nothing saved." again. Settings also shows Plan:
   Pro with "Subscribing isn't open on this server yet." (expected here).
6. **Coder sign-in.** In a terminal:
   `~/.openagents/full-local/bin/coder login`. Your browser opens
   `/device` with "Sign in to Coder on <your Mac>?" and the code the
   terminal shows. **Approve**: the page says it's signed in, and the
   terminal says "Signed in to OpenAgents as <you>." Settings → Computers
   lists the Mac.
7. **Sync a Coder chat.** In a project folder run
   `~/.openagents/full-local/bin/coder`, type `/sync on`, ask Coder
   something. Within seconds the chat shows in the web sidebar with
   "Terminal · <your Mac>" under its title, with the same messages.
8. **Reply from the web.** Open that chat on the website, type a reply,
   send. It shows "Waiting for Coder on <your Mac>." Coder picks it up,
   answers in the terminal, and the answer appears on the website.
9. **Plugin cards.** New chat → **Test a plugin** (or ask "Which plugin
   should I try?"). The answer is a set of plugin cards (Project map, Code
   finder, Test reader, ...) each with **Get Coder**.
10. **Public pages.** Footer **Privacy** and **Terms** open the policies;
    **Download** shows the install commands for macOS/Linux and Windows;
    `/ui` shows the component catalog in light and dark.

## Checks without a browser

- `scripts/chat-goldens.sh http --base http://127.0.0.1:4301` runs the
  web chat goldens against this site.
- The account service's own sign-up makes a test account without GitHub:
  `curl -s -X POST http://127.0.0.1:8791/v1/accounts -H 'content-type:
  application/json' -d '{"label":"Test"}'` returns a `sess_` token; send it
  as the `oa_cloud_session` cookie to act signed in.
