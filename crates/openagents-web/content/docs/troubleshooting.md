# Troubleshooting

## Coder isn't answering on this Mac

The Mac app says "Coder isn't answering on this Mac" when its background
helper (the host) doesn't answer. The app keeps trying on its own.

1. Click **Try again** in [OpenAgents for Mac](/docs/mac).
2. If the app says "OpenAgents needs your OK to run in the background",
   click **Open Login Items** and turn on OpenAgents under **Allow in the
   Background**, then come back.
3. If it says "Coder couldn't start on this Mac", or the message lasts,
   quit OpenAgents and open it again.
4. Still stuck? Report a problem (below) with what the screen says.

## `coder` isn't found after installing

Open a new terminal: the installer added `~/.openagents/bin` to your
`PATH`, and only new terminals see it. Or run it by its full path,
`~/.openagents/bin/coder`. If you installed with
`CODER_NO_PATH_UPDATE=1`, add this line to your shell's startup file:

```sh
export PATH="$HOME/.openagents/bin:$PATH"
```

## Logging in

- **The browser didn't open.** Over SSH, or with no browser, `coder login`
  prints the address and the code instead. Open
  [openagents.com/device](https://openagents.com/device) on any device and
  enter the code.
- **The codes don't match.** Don't approve. Close the page and run
  `coder login` again; approve only when the page shows the same code as
  your terminal.
- **The code ran out.** A code works for 10 minutes. Run `coder login`
  again for a new one.
- **"Login is already waiting for you on the website."** You started a
  login inside Coder that you haven't approved yet. Approve it at
  [openagents.com/device](https://openagents.com/device).
- **No browser at all.** Make a login key in
  [Settings](https://openagents.com/settings/sign-in-keys), then run
  `coder login --key` and paste it.
- **"Coder couldn't find your user folder to keep your login in."** Set
  `HOME` (on Windows, `USERPROFILE`) to your user folder, or start Coder
  with `--state DIR`.

To log this computer out, run `coder logout`.

## Coder has nobody to answer

Coder needs at least one of these on the computer: Claude Code or Codex
logged in (run `claude` or `codex` once and log in), OpenCode installed,
your openagents.com login (`coder login`), or an OpenRouter key
([how](/docs/coder-plugins)). Then press `Shift+Tab` in Coder to see who
can answer.

## "Today's free allowance on OpenAgents is used up"

Coder's own loop on OpenAgents models is free up to a daily allowance.
Until it resets, press `Shift+Tab` to answer with Claude Code or Codex
on your own plan, or turn on OpenRouter BYOK and pick a model on your own
key in `/models`. The allowance starts again
at midnight UTC.

## OpenCode's first chat is slow, or Auto answered instead

The first time a chat runs on OpenCode, Coder sets up its checks inside
OpenCode, and the working line says "Setting up OpenCode (first time
only)". This can take a few minutes, since OpenCode may download what it
needs; later chats start at once.

If the setup doesn't finish, the chat still gets an answer: a grey line
says "OpenCode didn't start, so Auto answered", with OpenCode's own
reason, such as a network timeout. To fix it:

1. Check that OpenCode runs on its own: run `opencode` in a terminal, and
   quit it.
2. Check your network, then start a new chat (`Ctrl+N`) and pick OpenCode
   with `Shift+Tab`. The setup tries again.
3. To start every new chat on OpenCode, run
   `openagents coder models set opencode`.

## `Ctrl+Enter` sends instead of queueing

Many terminals send `Ctrl+Enter` as a plain `Enter`. Press `Ctrl+Q` (or
`Ctrl+J`) to queue instead; the hint under Coder's input names the key
that works in your terminal. Kitty, Ghostty, foot, and Windows Terminal
tell `Ctrl+Enter` apart.

## Windows

- **`coder` isn't found.** Close PowerShell and open a new window: the
  installer added `%USERPROFILE%\.openagents\bin` to your user `PATH`, and
  only new windows see it.
- **An agent won't start.** Run it once on its own in PowerShell, such as
  `claude` or `codex`, and log in. Agents installed with `npm install -g`
  start from Coder on Windows.
- **Background automation isn't there.** Background automation and local
  task services need macOS or Linux for now. Coder, its chats, and the
  `openagents` command work on Windows.

More in [Use Coder on Windows](/docs/coder-on-windows).

## My computer's chats don't show on the website

1. Run `coder login` on that computer and approve the code at
   [openagents.com/device](https://openagents.com/device).
2. In Coder, type `/sync on`.
3. Check [Settings](https://openagents.com/settings): the computer should
   be listed. A reply from the site reaches Coder only while Coder is open on that computer.

See [Connect a computer](/docs/connect-a-computer).

## My phone won't connect

- Check that the computer's host is running: `openagents connect status`.
  If nothing answers, start it with `openagents host serve --iroh --control`
  and leave it running.
- Run `openagents connect invite` again and scan the new code in
  [OpenAgents for iPhone](/docs/iphone). Each code works once.
- Can't scan? Run `openagents connect invite --text` and paste the code it
  prints in the phone's **Paste a code**.

## A coding agent says it isn't logged in

Coder uses only agents logged in on the computer it runs on, as the same
user.

1. Run the agent's own program in a terminal on that computer (for
   example `codex` or `claude`) and log in.
2. Check the Mac app's sidebar or **Settings → Coder**, or OpenAgents
   Terminal's **Agents** row: it should say **Logged in**.
3. Nothing needs enabling: Coder uses every logged-in agent. Only one you
   turned off is skipped (**Agents Coder may run**, or
   `openagents settings enable AGENT`).

If the agent you asked for isn't available, Coder runs another one and the
start says why. See [Coding agents](/docs/coding-agents).

## Coder won't start

- **Not a project.** Coder needs a Git checkout with at least one commit.
  In the Mac app, **Choose folder…**; in the Terminal, start in your
  project's folder. If you set `coder.projects`, the checkout must be
  inside one of those folders.
- **No agent available.** No allowed agent is logged in with room right
  now. Sign one in, or turn another on.
- **Settings file can't be read.** Fix or remove
  `~/.openagents/settings.json`; the message names the problem.
- **From the phone:** on the Mac, check that it's online, a project is
  picked, and **Let my phone start Coder here** is on. With **Ask first**,
  tap **Run Coder** under the reply.
- **Offered but not started.** With **Ask first**, press Enter on an empty
  line in the Terminal, or choose **Run Coder** in the app.

## The usage meters look out of date

The Mac app's usage meters come from the last reading your computer took;
it reads again in the background when a reading is due.

- **Reading usage…** means no reading yet. Wait a little.
- **The login has expired** or **No login to read**: log in to that
  agent again.
- **Usage can't be read right now**: the provider didn't answer; it tries
  again later.
- Grok Build has no meter: it doesn't report usage.

An out-of-date meter never stops Coder by itself. Coder moves on from an
agent only on a fresh reading, or when the agent turns it away.

## "Couldn't reach OpenAgents; try again."

Send the message again. If a very long chat asks you to start a new one,
start a new chat.

## Remove a phone

Click **Remove** next to the phone in the Mac app's **Phones and
computers**, or run `openagents connect remove DEVICE`.

## Report a problem

In the iPhone app, open **Account → Report a problem**, or long-press the
tab bar on any screen so the report knows where you were. Pick a kind,
say what happened and what you expected, and add a screenshot if you want;
you see it first, and the Wallet and key screens never attach one.
**My reports** lists what you sent. You can also open an issue on
[GitHub](https://github.com/OpenAgentsInc/openagents/issues).

Next: [Run your own OpenAgents](/docs/self-host).
