# Privacy and security

## Your computer

- A phone can reach your computer only after you scanned that computer's
  QR code, or approved it on the computer's own screen.
- The computer decides what a phone may do, and checks it on every
  message. **Remove** in OpenAgents for Mac cuts a phone off at once.
- OpenAgents for Mac keeps its keys in the Mac's keychain.
- Coder works in the projects you picked, on your computer. Like Codex
  and Claude Code on their own, it sends what it reads to the model
  provider you signed in to.

## Your chats

- There is no account or password. Your phone makes its own key the first
  time it opens and signs every message with it.
- Chat messages are encrypted before they leave your phone. The
  OpenAgents relay that carries them sees only the encrypted form and
  keeps nothing.
- To answer, our chat service sends the conversation to the AI models it
  uses: first Space Bunny Alpha, an anonymous preview model reached through
  OpenRouter, whose provider may keep what it is sent and its replies,
  though not to train on them; and Google's Gemini 3.8 Flash through the
  Vercel AI Gateway when Space Bunny Alpha can't answer. Our chat service
  itself stores no message text.

The full details are in the [Privacy Policy](/privacy) and the
[Terms of Service](/terms).
