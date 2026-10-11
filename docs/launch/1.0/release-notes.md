# OpenAgents 1.0 release notes (draft)

Draft for the owner (#11104). Nothing here is posted. Each line names the
issue it comes from so it can be checked against the build before posting.
Before publishing a platform's notes, open that platform's release build and
strike any line it doesn't do. Drop the issue numbers from the public copy if
you want it shorter.

The launch is web and terminal only. What's left out, and why, is in the
[README](README.md#left-out-on-purpose).

Every number and comparison here is checked in [claims.md](claims.md).

## Web (openagents.com)

Ships with the web deploy after the owner's local test (#11094).

- **Sign in with GitHub.** Click **Log in** at the top right. (#11039)
- **Your chats follow your account**, on any browser. Chats you started
  before signing in move to your account when you sign in. (#11039)
- **Organize chats.** Each chat's **…** menu has Pin, Rename, Archive, and
  Delete. Search is at the top of the sidebar. Archived chats are under
  **Archived chats** at the bottom of the sidebar. (#11036, #11038)
- **Delete everything.** Settings has **Delete all chats**. Deleted chats
  leave our servers right away; our storage provider can keep a copy we can
  recover for up to 7 days. (#11038)
- **Projects.** Open [/projects](https://openagents.com/projects) and click
  **Connect GitHub**. You pick which repositories, through a GitHub App. Chats
  are grouped by project in the sidebar. (#11034, #11056)
- **Your terminal chats on the web.** Sign in from Coder and turn on sync
  (see Terminal below), and those chats show in the sidebar with live status.
  While Coder is open on that computer, you can reply from the website.
  (#11046, #11047, #11048)
- **Signed-in computers.** Settings > Computers lists every computer signed
  in to your account. (#11045)
- **Status on chat rows.** A row shows a spinner while an answer is coming.
  (#11033, #11035)
- **Slow answers no longer fail.** When the model is slow to start, you see
  that it's still working instead of an error. (#11087)
- **Plain words.** We took the system talk out of the pages, and a check in
  the site's tests keeps it out. (#11031)
- **Privacy.** We ask the model providers not to train on your chats and not
  to keep them. Usage records are deleted after 30 days. The
  [privacy policy](https://openagents.com/privacy) was
  updated on October 9; read it. (#11040, #11041, #11042, #11044)

## Terminal (Coder 1.0.0)

Ships with the Coder 1.0.0 release on the stable channel (#11091). The new
lines below need a build from `main` at `35e41fd528` or later; the 1.0.0-rc.6
on `/download` doesn't have them.

Install or update, on macOS and Linux:

```sh
curl -fsSL https://openagents.com/cli/install.sh | bash
```

On Windows, in PowerShell:

```powershell
irm https://openagents.com/cli/install.ps1 | iex
```

- **Sign in to your OpenAgents account.** Run `coder login` (or `/login`
  inside Coder). It shows a code and opens
  [openagents.com/device](https://openagents.com/device); approve it there.
  `coder logout` (or `/logout`) signs out. (#11045)
- **Sync your chats.** `/sync on` saves new and changed chats to your
  account. `/sync all` also adds your 150 newest earlier chats. `/sync off`
  stops. `/sync delete` removes this computer's chats from your account.
  `/sync` shows where you are. Sync is off until you turn it on. (#11046)
- **Keys stay home.** Every message is checked for keys and passwords before
  it leaves your computer and again on our side. A message that looks like
  one isn't sent. (#11046)
- **Deletes go both ways.** Delete a chat on the website and Coder deletes it
  too. (#11046)
- **Plain error messages** instead of internal error text. (#11091)
- **`auto` picks the model.** The input bar says `auto`: OpenAgents picks
  the model for each message. A model you pick in `/models` shows by its own
  name. (`d14ef0d59f`)
- **Tab switches to `flash`.** Tab switches the input between `auto` and
  `flash`, a fast model on your own OpenRouter key. Without a key, Coder
  says to add one with F2. (`99eed810ce`)
- **Free every day when you're signed in.** `auto` is free up to about $1
  of model use a day. Coder tells you when it's used up. It starts again at
  midnight UTC. Goes live with the production deploy. (`881f380e0b`)
- **Coder says it is Coder.** Ask who it is and it says Coder, made by
  OpenAgents, never another product. Ask which model it runs on and it says
  OpenAgents picks it, or names the one you picked. (#11264)
- **Usage for every model call.** Each call's tokens, cost, and time are
  recorded when it ends. `/export` saves the chat to a file with them.
  (`6ab491f16a`)
- **Run boxes scroll on their left edge.** Scroll there to move through a
  command's output; scroll anywhere else to move through the chat.
  (`3a559ca2eb`)
- **Run keeps your changes.** It won't run git commands that throw away
  uncommitted work, such as `git stash` or `git reset --hard`, and suggests a
  fresh worktree instead. (`9dbcba33e6`)
- **Compaction reads plainly.** When Coder sums up older messages, a
  **Compacted** row in the chat says how many. It no longer shows a red
  error line. (`35e41fd528`)
