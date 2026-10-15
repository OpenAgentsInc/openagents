# FAQ

## What does it cost?

Chatting with OpenAgents is free, and we don't count your messages. Coder
runs on your own computer with the coding agents you already use there,
such as Claude Code and Codex, and each one's own plan covers it. Coder's
own loop on OpenAgents models is free up to a daily allowance when you're
logged in. Pro, for cloud environments, will be $20 a month with 100
machine-hours; it isn't on sale yet. See [Pricing](/docs/pricing).

## Do I need a computer?

Not to chat. You need your own computer to run Coder: a Mac, Linux, or
Windows computer with Coder installed ([Download and install](/docs/download)).
We don't offer a hosted computer yet.

## Do I need an account?

Not for the apps or Coder: each device makes its own key the first time it
runs. An account ([log in](https://openagents.com/login)) keeps your
website chats, and with `coder login` and `/sync on` it shows Coder's
chats from your computer. See [Sync, share, and export](/docs/coder-sync).

## Which model answers the chat?

Google's Gemini 3.8 Flash, on Vertex AI or through OpenRouter or the
Vercel AI Gateway, with GLM as a backup. Jev, a small decision
model from TypeSafe, decides how we reply. Ask the chat "What model is
this?" to hear which one is answering now. See
[Privacy and security](/docs/privacy-and-security).

## Do you keep or train on my chats?

In the web chat on openagents.com, we keep your chats on our servers so
you can open them again; you can delete one from its … menu in the sidebar. In
the Mac app, Terminal, and phone app, your chats are saved on your device,
and our chat service doesn't store your message text. We may use your
chats to train and improve our models; on a paid plan you can ask us to
opt you out, and we set that up with you. See [Privacy and security](/docs/privacy-and-security).

## Can I pick the model Coder uses?

Yes. In Coder, `Shift+Tab` chooses who answers (Auto, Claude Code, Codex,
OpenCode, or another agent), and `/models` chooses the model. See
[Choose who answers](/docs/coder-agents).

## Can I attach a photo or a file?

In Coder, yes: paste a screenshot, drop in the path of an image or a PDF,
or write `@path/to/file` in your message. Chats on the website and in the
apps are text only for now.

## Do I need Tailscale?

No. Your phone reaches your computer directly or through our relay.
Tailscale is an optional extra route if you already use it.

## Does Coder change my files?

In the `coder` terminal app, yes: a chat works in the folder you open it
in, like any coding agent, and `/rewind` puts back the files a turn
changed. Background agents, Solver, and chats run from scripts work in
their own copy of your project instead. Commands that would throw away
uncommitted work are refused everywhere. See [Safety](/docs/coder-safety).

## Do plugin authors get paid?

Not today. Credit is XP and your name, and XP isn't money. Paying plugin
authors when their plugins are used is something we've talked about, not
something we've built.

## Is it open source?

Yes. The apps, the chat service, Coder, and the protocols are at
[github.com/OpenAgentsInc/openagents](https://github.com/OpenAgentsInc/openagents),
under the Apache License 2.0.

## When is version 1.0 out?

Coder 1.0 is in release candidates now; the [download page](/download)
shows the latest. Stable releases come after testing, and Coder updates
itself when they're out.

## How do I see what changed in a build?

On the phone, **Account → Changelog** lists what each build brought, and
**Account → About this device** shows the version and build.

Next: [Glossary](/docs/glossary).
