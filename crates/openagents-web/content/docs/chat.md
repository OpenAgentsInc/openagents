# Chat

Chat is where you talk with OpenAgents. It's the same chat on the website,
the Mac app, the iPhone app, and the Terminal. OpenAgents answers as "we".

## What you can ask

- **About OpenAgents.** How to connect a phone, what a plugin is, what's
  new in the Gym, how the wallet works. These are answered from our
  product notes.
- **About your work.** Plan a change, ask how something works, or ask what
  Coder just did.
- **Anything else.** A general question gets a general answer.
- **Coding work.** "Fix the failing test", "add a unit test for slugify",
  "work on #123". These go to Coder on your computer, as described
  below.
- **To open something.** "Open my wallet", "show me how you route things",
  "show the test-time capabilities deck" open that screen in the app that
  has it.

To start fresh, choose **New chat** (Cmd+N on the Mac, `/new` in the
Terminal). Every chat goes to OpenAgents; there's nothing to pick.

## Why some answers appear at once

A small, fast model, Jev from TypeSafe, reads each message first and
decides how we reply. When one of our prepared answers, or an entry in our
product notes, fully answers the question, it shows in about half a second.
Otherwise a larger model writes the reply, which takes a few seconds. A
prepared answer often has follow-up questions under it to tap.

## When Coder starts

When a message is coding work, the reply says so and Coder starts on a
computer:

- **On a computer** (the Mac app, the Terminal, `openagents chat`), Coder
  starts right there at once, in your project. If you set Coder to **ask
  first**, the reply offers **Run Coder** instead.
- **On the phone**, Coder starts on your connected computer at once if
  that computer allows it; otherwise the reply shows **Run Coder on** your
  computer. Without a computer, it offers **Connect a computer**.

A reply that already answered your question never starts Coder. See
[Coder](/docs/coder).

## Use it as much as you like

We don't count your messages per minute or per day. If you ever see
"Couldn't reach OpenAgents; try again.", send the message again. A very
long conversation may ask you to start a new chat.

## What the chat can't do

From the chat itself we can't read your files, run code, or reach your
computer. That's Coder's job, and it happens only on a computer you
connected or are using.

Next: [Privacy and security](/docs/privacy-and-security).
