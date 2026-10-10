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

## A coding agent says it isn't signed in

Coder uses only agents signed in on the computer it runs on, as the same
user.

1. Run the agent's own program in a terminal on that computer (for
   example `codex` or `claude`) and sign in.
2. Check the Mac app's sidebar or **Settings → Coder**, or OpenAgents
   Terminal's **Agents** row: it should say **Signed in**.
3. Nothing needs enabling: Coder uses every signed-in agent. Only one you
   turned off is skipped (**Agents Coder may run**, or
   `openagents settings enable AGENT`).

If the agent you asked for isn't available, Coder runs another one and the
start says why. See [Coding agents](/docs/coding-agents).

## Coder won't start

- **Not a project.** Coder needs a Git checkout with at least one commit.
  In the Mac app, **Choose folder…**; in the Terminal, start in your
  project's folder. If you set `coder.projects`, the checkout must be
  inside one of those folders.
- **No agent available.** No allowed agent is signed in with room right
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
- **The sign-in has expired** or **No sign-in to read**: sign in to that
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
