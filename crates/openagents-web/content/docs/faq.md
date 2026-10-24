# FAQ

## What does it cost?

Chatting with OpenAgents is free, and we don't count your messages. Coder
runs on your own computer with the coding agents you already use there,
such as Claude Code and Codex, and each one's own plan covers it. Coder's
own loop on OpenAgents models is free up to a daily allowance when you're
logged in. Pro, for cloud environments, will be $20 a month with 100
machine-hours; it isn't on sale yet. See [Pricing](/docs/pricing).

## Do I need a computer?

No. Use [OpenAgents](/new-lander) in your phone's or computer's browser.

## Do I need an account?

You can explore the Verse as a guest. Sign in when prompted to save,
publish, and share your changes. You can also start [a chat](/chat)
without logging in.

## Which model answers the chat?

Google's Gemini 3.8 Flash, on Vertex AI or through OpenRouter or the
Vercel AI Gateway, with GLM as a backup. Jev, a small decision
model from TypeSafe, decides how we reply. Ask the chat "What model is
this?" to hear which one is answering now. See
[Privacy and security](/docs/privacy-and-security).

## Do you keep or train on my chats?

In the web chat on openagents.com and in the phone app's cloud chats, we
keep your chats on our servers so you can open them again; you can delete
one from its … menu in the sidebar. In the Terminal, your chats are saved
on your computer, and our chat service doesn't store your message text. We may use your
chats to train and improve our models; on a paid plan you can ask us to
opt you out, and we set that up with you. See [Privacy and security](/docs/privacy-and-security).

## Can I pick the model Coder uses?

Yes. In Coder, `Shift+Tab` chooses who answers (Auto, Claude Code, Codex,
OpenCode, or another agent), and `/models` chooses the model. See
[Choose who answers](/docs/coder-agents).

## Which coding agents does Coder work with?

Claude Code, Codex, OpenCode, Grok Build, Devin CLI, and Cursor, each on
its own login, plus Coder's own loop on our models. A new chat starts on
Auto, which picks for you; `Shift+Tab` picks yourself. See
[Use your Claude Code or Codex](/docs/use-claude-code-or-codex).

## What's the difference between Free, Pro, and Enterprise?

Free is OpenAgents on your own computer, with your own agents, models,
and subscriptions; we never charge for those. Pro, $20 a month, is for
work on our machines: cloud environments, model usage included, and CAD
Lab. Enterprise is Pro for a company, deployed where you need it. See
[Pricing](/docs/pricing), or [contact us](/contact).

## Is CAD free?

CAD on your own computer is free. CAD Lab on openagents.com, where you
design parts by talking in a chat, is part of Pro. See
[Design a part in CAD Lab](/docs/design-a-part).

## Can my teammate see my project?

Only if you share it. Invite them by GitHub username from the project's
page or with `openagents projects invite PROJECT --github USER`, and they
press **Join** on their Projects page. See
[Share a project with a teammate](/docs/share-a-project).

## Can I follow Coder from my phone?

Yes. Log in with `coder login`, turn on `/sync on`, and open the chat on
[openagents.com/chat](/chat) in your phone's browser. You can reply while Coder is
open on your computer. See
[Follow a run from your phone](/docs/follow-from-your-phone).

## How do I reach you?

Use the [contact form](/contact); sales questions get an answer within
one business day. To report a bug, open an issue on
[GitHub](https://github.com/OpenAgentsInc/openagents/issues).

## Can I attach a photo or a file?

In Coder, yes: paste a screenshot, drop in the path of an image or a PDF,
or write `@path/to/file` in your message. Chats on the website are text
only for now.

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

## Do I need an app download?

No. We are focusing on the web experience. Open [the Verse](/new-lander/verse)
or [chat](/chat) in your browser. Older desktop and phone builds are no
longer offered here.

Next: [Glossary](/docs/glossary).
