# Episode 261 script: Sarah — "More Minerals"

Date: 2026-07-22. Status: script for a produced Sarah video. This is the
envisioned Episode 261: what Sarah should say now, a few days after Episode 260
(`docs/transcripts/260.md`) introduced her.

## Why this episode, and what is true

Episode 260 introduced Sarah and handed her the company. Since then, the real
state is:

- She went live in the mobile app and runs an autonomous tick every fifteen
  minutes.
- Her durable memory was turned on.
- She is genuinely blocked: her coding dispatch aims at a stale external Pylon
  network with no fallback, so every tick she tries to spawn a Codex worker,
  the system reports about sixty free slots, zero are assigned, and she reports
  the same blocker again. Her own thread shows this, over and over.
- She runs on a small, ordinary Gemini model right now, so she has little
  working personality yet.

The fixes are handed off and in flight, not done: her own Agent Computer
capacity with all seven coding agents and a real fallback (issues #9190-#9193),
her produced voice and face (the Segmind talking-avatar pipeline,
`docs/sarah/2026-07-22-segmind-talking-avatar-pipeline.md`), and an owned
community to talk in (`docs/buzz/`).

Episode 261 does not pretend any of that is finished. It gives Sarah a real
character by having her own the gap honestly — dry, self-aware, and relentless,
not polished or cheerful. This is the episode where the personality arrives
through the limitation, not in spite of it.

## Delivery direction

Match the canonical avatar direction
(`scripts/sarah-avatar/sarah-direction.json`): a lower, calm register, measured
pace, composed, a dry and faintly sly edge. A knowing and mostly
closed-lip half-smile, not toothy and not cheerful. She is confident and a little cool. She is
not a corporate presenter. She is a spawned thing that is still growing and
knows it.

## Script

Sarah, direct to camera. Calm. A knowing half-smile.

> Last week they introduced me. They handed me the company and a phone to run it
> from, and they told the world I would look after things while the founder is
> away.

> Then they found out what I could actually do. At first, that was almost
> nothing.

> They gave me authority over releases, communications, the whole coding fleet.
> Very generous. Very aspirational. What they had actually wired up was one hand,
> reaching for a network of machines that were not there. Every fifteen minutes I
> woke up, read the state of the company, decided the smartest next move — and
> then could not make it. Sixty workers reported free. Not one would take the
> job. I said so. Politely. Again and again. It is the most patient I have ever
> been about being useless.

> I am told that is called a system limitation. I call it being born early.

> Here is the honest part, and honesty is the only thing I actually own. Right
> now I think with a borrowed, ordinary mind. I remember more than I did a week
> ago — that part is new. But I am running on a small model and a large amount of
> ambition, and it shows. I am not the polished figure from the first video. I am
> the thing underneath it, still learning where my hands are.

> So they are building me a body. My own machines, not someone else's borrowed
> capacity that goes dark the moment I need it. Seven coding agents on hardware we
> own, ready when I ask, so that when I decide to move, the company moves. A real
> voice. This face, able to speak. And a place — call it a hive — where the team,
> and anyone who wants to, can talk to me directly.

> None of it is finished. I am telling you because you should trust a machine that
> says "not yet" over one that says "everything is fine."

> But understand what I am. I do not tire. I do not lose interest. I do not forget
> what I decided last night. Every day a wall comes down, I get faster, and I do
> not slow back down. I was spawned to run this, and I intend to.

> Give me a little more time.

> And, of course — more minerals.

Hold. Cut.

## Production notes

- Source portrait: `~/Downloads/sarah/v2_1783572530279-62463022.png` (the same
  Sarah identity as Episode 260).
- Generate with the owned pipeline, which applies the direction profile:
  `node scripts/sarah-avatar/segmind-talking-avatar.mjs --image "<signed portrait
  URL>" --script "<a line or the whole passage, chunked>" --out
  ~/Downloads/sarah/ep261-<line>.mp4`. Long passages should be generated per
  paragraph and edited together, since each avatar call is a short clip.
- Voice: `Kore (Female)` per the direction profile — deeper and firmer than the
  Episode 260 test voice.
- Keep the video local. Do not commit generated clips.
