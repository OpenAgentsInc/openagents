# OpenAgents 1.0 launch post (draft)

Draft for the owner (#11104). Nothing here is posted. Post only after every
platform named in it is live; if one slips, take it out of the post rather
than saying "soon".

## Posting checklist

Check every launch post, clip, and thread against this before it goes out
(#11768). It applies to the posts below, the Episode 290 upload and its
show notes, and any later launch clip.

- [ ] **AI label.** Every AI-made clip carries the platform's AI label when
  posted (X, YouTube, TikTok, Instagram and LinkedIn each have one), and
  its caption or description says what is AI-made. That includes Verse and
  Draconia renders, the launch film, and any AI voice (Episode 290's Bella
  narration).
- [ ] **Voice.** Captions and post text say "we" for OpenAgents and "I" for
  an agent speaking in its own clip (Coder, Microcoder, Solver). Never "I"
  for OpenAgents, never "we" for an agent.
- [ ] **No fake personas.** No AI character or voice is presented as a real
  person, no testimonial that didn't happen, no account but our official
  one. Draconia and our agents appear as what they are: an animated
  character and agents.
- [ ] **Claims.** Every number and comparison is in [claims.md](claims.md)
  with its evidence; no competitor claims.
- [ ] **Live.** Every platform and feature a post names is live today.

Checked 2026-10-17: the X post and thread below have no clips and no
agent speaking; they say "we" for OpenAgents and name no persona. Shots 1
and 2 are screenshots of the real app on a dedicated test account, not AI
images, and never the offline demo shown as live. The
Episode 290 script (`docs/transcripts/290.md`) puts this checklist on its
recording-day list, since its cut uses an AI voice.

## X (one post, 267 characters)

Attach shot 1 from the [plan](../../mobile/2026-10-17-mobile-first-launch.md#the-screenshots)
(first launch, a question typed, the answer streaming), from the build
people install.

```text
OpenAgents 1.0 is out, and it starts on your iPhone.

Ask a question with no account and no computer. Sign in and start a coding task on our cloud.

Your computer and terminal do the heavy work. Your phone is where you watch it. Sync is free.

openagents.com/download
```

## Thread or blog version

**1.** OpenAgents 1.0 is out, and it starts on your iPhone. Install it, ask a
question, and get an answer in the first minute, with no account and no
computer. [Shot 1]

**2.** After a few free messages, sign in with one tap. Ask for a coding
task and it runs on our cloud, with a few dollars of free cloud credit to
start. [Shot 2]

**3.** Your computer is where the power is. Run Coder in your terminal or
the desktop app, and the phone drives it: send, queue, steer, and stop a
live session from anywhere.

**4.** One conversation everywhere. The same chats are on the phone, the
web, the desktop, and the terminal, and sync is free.

**5.** In the terminal, install Coder:

`curl -fsSL https://openagents.com/cli/install.sh | bash`

Then `coder login` and `/sync on`. Your terminal chats show up on your
phone and on the web, live.

**6.** Coder's input says `auto`: OpenAgents picks the model for each
message. Press Tab for `flash`, a fast model on your own key. Every model
call's tokens, cost, and time are recorded, and `/export` saves them with
the chat.

**7.** Sync is off until you turn it on. Before a message leaves your
computer, Coder checks it for keys and passwords and leaves out anything that
looks like one.

**8.** We ask our model providers not to train on your chats and not to keep
them, and we delete usage records after 30 days.

**9.** The code for the apps, the website, and Coder is open source:
github.com/OpenAgentsInc/openagents. Tell us what breaks.

## Notes for the owner

- This is a draft waiting on your approval (#11985).
- The X post above is 267 characters, under 280. X counts the link as 23
  characters, which is also its real length.
- The post leads with the phone (owner, 2026-10-17). Post only after the
  external TestFlight build passes Beta App Review (#11986) and
  `/download` links to it. If the phone slips, go back to the web and
  terminal post in Git history rather than saying "soon".
- Parts 1 and 2 depend on work still in progress (#11978–#11981). Cut a
  part if the installed build doesn't do it. "In the first minute" and
  "a few dollars" stay unmeasured until [claims.md](claims.md#phone-claims)
  has the evidence; no number goes in until then.
- iPhone only. If asked about Android, say "iPhone first".
- The thread says nothing about the Pro plan, environments, agent payments,
  the wallet, or the inference API.
- Every number and comparison here is checked in [claims.md](claims.md).
