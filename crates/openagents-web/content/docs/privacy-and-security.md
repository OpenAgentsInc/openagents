# Privacy and security

## Your chats

- **No account needed.** The apps have no account or password. Your phone
  or computer makes its own key the first time it runs and signs every
  message with it. On the website, the site signs your messages for you.
- **In the web chat** on openagents.com, we save your chats on our servers.
  Our team can read them.
  - **Signed in** (with GitHub), your chats belong to your account: you see
    them in any browser where you sign in, and after you sign out, someone
    else using that browser doesn't. Chats you started in that browser
    before signing in move to your account when you sign in.
  - **Not signed in,** a random cookie tells this browser's chats apart.
    Only this browser opens them, and anyone using it can. If you clear
    your cookies, you can't open them again.
  - **Deleting.** To delete a chat, open it and choose **Delete chat**. To
    delete every chat, choose **Delete all chats**: in Settings when signed
    in, or on a chat's delete step when not. Chats are removed from our
    servers right away, though our storage provider may keep a copy we can
    recover for up to 7 days. Chats you don't delete stay; there's no time
    limit yet.
  - **Exporting.** Signed in, open
    [Settings](https://openagents.com/settings) and choose **Download**
    under **Your data** (or go straight to
    [openagents.com/settings/export](https://openagents.com/settings/export)).
    You get one file with every chat (each with a Markdown copy to read),
    your projects, your uploaded traces, your signed-in computers, and your
    settings. It opens in any text editor, without OpenAgents. It holds only
    your own account's things, and never a key, token, or password: an API
    key shows by name only, and a saved Claude credential by its kind only.
- **In the Mac app, Terminal, and phone app,** your chats are saved on your
  device, encrypted with its key (on a computer, by its host). Your
  messages are encrypted before they leave your device, and the OpenAgents
  relay that carries them to our chat service sees only the encrypted form
  and keeps nothing. When your phone reaches your computer through the
  relay, the relay holds those messages, encrypted, for that phone and
  computer only.
- **Our chat service doesn't keep your message text.** It records each
  request's time, key, surface, route, model, token counts, and timings for
  usage statistics, and deletes those records after 30 days.
- **Training.** We may use your chats to train and improve our models. On a
  paid plan you can ask us to opt you out; we set that up with you.
- **Reports.** If you send a problem report and choose to share the chat,
  that chat goes to our team with it.

## Who sees what

To answer, our chat service sends your messages to these services:

- **Space Bunny Alpha**, an anonymous preview model reached through
  OpenRouter, which every turn asks first.
- **Google's Gemini 3.8 Flash**, through the Vercel AI Gateway, when Space
  Bunny Alpha doesn't answer (and Gemini 2.5 Flash, through the same
  gateway, for some Gym news replies).
- **TypeSafe's Jev**, which decides how we reply. Jev is reached through
  the Vercel AI Gateway first, then OpenRouter, then TypeSafe directly.
- **OpenRouter**, for a short personalized line in some replies that hand
  work to Coder.
- **OpenAI embeddings, through the Vercel AI Gateway**, when a question is
  answered from our product notes, our code, or the Gym's records.

When we send your messages to Space Bunny Alpha, Gemini, or the
OpenRouter model that writes personalized lines, we ask the model provider
not to keep them or train on them. If a model's provider can't agree, we
don't use that model. We don't ask this of Jev or the embeddings yet.

When you chat on a computer, or from a phone connected to one, each
message also names that computer and the chat's project folder; after
Coder runs in a chat, the next messages carry what it reported, the files
it changed, and its commands. Only our chat service and the chat model read
those.

Ask the chat "What happens to my messages?" and it answers with the
services it uses right now.

## What we count on the website

We count how openagents.com is used ourselves. There's no outside
analytics service, no tracking script from another company, and no cookie
for it.

- **What we count:** which page was opened (the kind of page, not its full
  address: every chat counts as "a chat"), the name of the site that sent
  you (like `ycombinator.com`), whether you're on a phone, tablet, or
  computer, whether the visit came from a person, an AI agent, or a
  crawler, how fast the page loaded, and a few actions: sending a chat,
  getting an answer, clicking a suggestion or a card, clicking a download,
  copying the install command, and signing in.
- **What we don't:** your IP address, cookies, your account, what you
  write, or anything that tells one visitor from another. We keep hourly
  totals, so we can't tell whether two visits came from one person.
- **Do Not Track and Global Privacy Control:** with either one on in your
  browser, we count only that the page was opened, and nothing else.
- **How long:** the hourly totals are kept for 13 months.

Our hosting provider's request logs, which keep the site running and
secure, do see IP addresses; they're deleted after 30 days.

## Your computer

- A phone can reach your computer only after you scanned that computer's
  QR code (or pasted its code, or approved it on the computer's screen).
- Every connected phone gets full access to that computer, a terminal
  included. The computer checks each message against that access.
  **Remove** in the Mac app cuts a phone off at once.
- The Mac app keeps its keys in the Mac's keychain.
- Coder works on your computer, in your projects. Like Codex or Claude Code
  on their own, the coding agent it runs sends what it reads to that
  agent's model provider, under your sign-in there.
- By default Coder approves its own steps, including a push, so it can
  finish without stopping to ask. To run it inside a stricter boundary, see
  `coder.access` in the [settings reference](/docs/settings).

## Your phone's keys

- **Device key:** made on your phone, kept in its Keychain on this device
  only, and never synced. It holds access to your computers.
- **World key:** a separate key for the Verse and your XP, so other players
  can't link you to your device key.
- **Wallet:** its own recovery words, separate from both.

The full details are in the [Privacy Policy](/privacy) and the
[Terms of Service](/terms).

Next: [Coder](/docs/coder).
