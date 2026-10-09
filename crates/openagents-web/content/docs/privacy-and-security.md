# Privacy and security

## Your chats

- **No account.** There's no account or password. Your phone or computer
  makes its own key the first time it runs and signs every message with
  it. On the website, a random cookie tells your chats apart, and the
  site signs your messages for you.
- **In the web chat** on openagents.com, we save your chats on our servers
  so you can open them again in the same browser. Only that browser's
  cookie opens them, and our team can read them. There's no time limit
  yet, and you can't delete a web chat yourself.
- **In the Mac app, Terminal, and phone app,** your chats are saved on your
  device, encrypted with its key (on a computer, by its host). Your
  messages are encrypted before they leave your device, and the OpenAgents
  relay that carries them to our chat service sees only the encrypted form
  and keeps nothing. When your phone reaches your computer through the
  relay, the relay holds those messages, encrypted, for that phone and
  computer only.
- **Our chat service doesn't keep your message text.** It records each
  request's time, key, surface, route, model, token counts, and timings for
  usage statistics.
- **Training.** We don't use your chats to train models.
- **Reports.** If you send a problem report and choose to share the chat,
  that chat goes to our team with it.

## Who sees what

To answer, our chat service sends your messages to these services:

- **Space Bunny Alpha**, an anonymous preview model reached through
  OpenRouter, which every turn asks first. Its provider may keep prompts
  and replies, though not to train on them.
- **Google's Gemini 3.8 Flash**, through the Vercel AI Gateway, when Space
  Bunny Alpha doesn't answer (and Gemini 2.5 Flash, through the same
  gateway, for some Gym news replies).
- **TypeSafe's Jev**, which decides how we reply. Jev is reached through
  the Vercel AI Gateway first, then OpenRouter, then TypeSafe directly.
- **OpenRouter**, for a short personalized line in some replies that hand
  work to Coder.
- **OpenAI embeddings, through the Vercel AI Gateway**, when a question is
  answered from our product notes, our code, or the Gym's records.

When you chat on a computer, or from a phone connected to one, each
message also names that computer and the chat's project folder; after
Coder runs in a chat, the next messages carry what it reported, the files
it changed, and its commands. Only our chat service and the chat model read
those.

Ask the chat "What happens to my messages?" and it answers with the
services it uses right now.

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
