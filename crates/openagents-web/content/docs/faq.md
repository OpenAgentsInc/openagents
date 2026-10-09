# FAQ

## What does it cost?

We haven't published pricing. Chatting with OpenAgents is free right now,
and we don't count your messages. Coder runs on your own computer with the
coding agents you already use there, so it doesn't bill you through us;
each agent's own provider bills you as it normally does.

## Do I need a computer?

Not to chat. You need your own computer to run Coder: a Mac with
OpenAgents for Mac, or a computer with OpenAgents Terminal. We don't offer
a hosted computer yet.

## Do I need an account?

No. Each device makes its own key the first time it runs. There's no
sign-in and no password.

## Which model answers the chat?

Space Bunny Alpha, an anonymous preview model through OpenRouter, first;
Google's Gemini 3.8 Flash when it doesn't answer. Jev, a small decision
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

Not from the apps yet. Name a model for an agent in the settings, such as
`codex:MODEL`; see the [settings reference](/docs/settings).

## Can I attach a photo or a file?

Not yet. Chats are text only on every surface for now.

## Do I need Tailscale?

No. Your phone reaches your computer directly or through our relay.
Tailscale is an optional extra route if you already use it.

## Does Coder change my files?

Not in your checkout. Coder works in its own worktree, a separate copy of
your project, and you choose whether to publish the change. See
[Worktrees and changes](/docs/worktrees-and-changes).

## Do plugin authors get paid?

Not today. Credit is XP and your name, and XP isn't money. Paying plugin
authors when their plugins are used is something we've talked about, not
something we've built.

## Is it open source?

Yes. The apps, the chat service, Coder, and the protocols are at
[github.com/OpenAgentsInc/openagents](https://github.com/OpenAgentsInc/openagents),
under the Apache License 2.0.

## When is version 1.0 out?

The Mac app and OpenAgents Terminal are at release candidate 1.0.0-rc.2 on
the [download page](/download). Stable releases come after testing.

## How do I see what changed in a build?

On the phone, **Account → Changelog** lists what each build brought, and
**Account → About this device** shows the version and build.

Next: [Glossary](/docs/glossary).
