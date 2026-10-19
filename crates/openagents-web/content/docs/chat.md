# Chat

Chat is where you talk with OpenAgents. It's the same chat on the website,
the iPhone app, and the Terminal. OpenAgents answers as "we".

## What you can ask

- **About OpenAgents.** How to connect a phone, what a plugin is, what's
  new in the Gym, how the wallet works. These are answered from our
  product notes.
- **About your work.** Plan a change, ask how something works, or ask what
  Coder just did.
- **Anything else.** A general question gets a general answer.
- **Coding work.** "Fix the failing test", "add a unit test for slugify",
  "work on #123". These go to Coder, as described below.

To start fresh, choose **New chat** on the website or the phone, or type
`/new` in the Terminal. A chat goes to OpenAgents unless you pick one of your
[custom agents](/docs/custom-agents) on the phone.

## Why some answers appear at once

A small, fast model, Jev from TypeSafe, reads each message first and
decides how we reply. When one of our prepared answers, or an entry in our
product notes, fully answers the question, it shows in about half a second.
Otherwise a larger model writes the reply, which takes a few seconds. A
prepared answer often has follow-up questions under it to tap.

## When Coder starts

When a message is coding work, the reply says so and Coder starts on a
computer:

- **On a computer** (the Terminal, `openagents chat`), Coder
  starts right there at once, in your project. If you set Coder to **ask
  first**, the reply offers **Run Coder** instead.
- **On the phone**, coding work runs where **Run on** says: on
  OpenAgents Cloud, or on a computer you connected. See
  [OpenAgents for iPhone](/docs/iphone).

A reply that already answered your question never starts Coder. See
[Coder](/docs/coder).

## Use it as much as you like

We don't count your messages per minute or per day. If you ever see
"Couldn't reach OpenAgents; try again.", send the message again. A very
long conversation may ask you to start a new chat.

## What the chat can't do

From the chat itself we can't read your files, run code, or reach your
computer. That's Coder's job, and it happens only on a computer you
connected or are using, or on OpenAgents Cloud.

Next: [Privacy and security](/docs/privacy-and-security).
