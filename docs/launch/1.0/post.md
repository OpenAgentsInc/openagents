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

Checked 2026-10-15: the X post and thread below have no clips and no
agent speaking; they say "we" for OpenAgents and name no persona. The
Episode 290 script (`docs/transcripts/290.md`) puts this checklist on its
recording-day list, since its cut uses an AI voice.

## X (one post, 245 characters)

```text
OpenAgents 1.0 is out.

An open network of agents you work with in one conversation, on the web and in your terminal.

Sign in on the web and your chats follow you. Turn on sync and your terminal chats show up there too.

openagents.com/download
```

## Thread or blog version

**1.** OpenAgents 1.0 is out, on the web and in the terminal. Get it at
openagents.com/download.

**2.** OpenAgents is an open network of agents you work with through one
conversation. Each message goes to whichever agent, model, or computer in
the network serves it best; questions about us are answered from our
knowledge base. In the terminal, ask for work on your code and Coder starts
in your project.

**3.** On the web, sign in with GitHub. Your chats belong to your account now,
not a browser cookie. Pin, rename, archive, search, and delete them.

**4.** Connect your GitHub repositories at openagents.com/projects. You pick
which ones. Chats group by project in the sidebar.

**5.** In the terminal, install Coder:

`curl -fsSL https://openagents.com/cli/install.sh | bash`

Then `coder login` and `/sync on`. Your terminal chats show up in the web
sidebar, live. While Coder is open, you can reply to them from the website.

**6.** Coder's input says `auto`: OpenAgents picks the model for each
message. Press Tab for `flash`, a fast model on your own key. Every model
call's tokens, cost, and time are recorded, and `/export` saves them with
the chat.

**7.** Sync is off until you turn it on. Before a message leaves your
computer, Coder checks it for keys and passwords and leaves out anything that
looks like one.

**8.** We ask our model providers not to train on your chats and not to keep
them, and we delete usage records after 30 days.

**9.** The code for the website and Coder is open source:
github.com/OpenAgentsInc/openagents. Tell us what breaks.

## Notes for the owner

- The X post above is 245 characters, under 280. X counts the link as 23
  characters, which is also its real length.
- Web and terminal only (owner, 2026-10-09). No desktop, iPhone, or Android
  lines; add the iPhone back once TestFlight review passes.
- The thread says nothing about the Pro plan, environments, agent payments,
  the wallet, or the inference API.
- Part 6 needs Coder 1.0.0 built from `main` at `35e41fd528` or later
  (see the [README](README.md#what-has-to-be-true-first)); cut it if the
  stable build is older.
- The free daily allowance is left out of the post until it is live in
  production; the release notes carry it.
- Every number and comparison here is checked in [claims.md](claims.md).
