https://x.com/OpenAgentsInc/status/1948617421654245436

July 25, 2025

Episode 186: Actions Per Minute

Flawed ground-truth data in agent benchmark "Humanity's Last Exam" underscores the need for better ways to measure agent effectiveness.

Primarily we want to measure our agents' velocity of output. We'll adapt from StarCraft 2 the idea of actions per minute (APM).

We demo the stats pane we added to OpenAgents which analyzes historical Claude Code conversations and shows your APM over the last hour, 6 hours, day, week, etc.

From this author's 30-day Claude Code usage across 277 sessions, we establish our baseline of 2.3 APM.

Rookie numbers! Now to pump them up...

---

Here is the full transcription of the video.

**00:00**
**Speaker:** So how do we measure the effectiveness of our agents? Well, it came out yesterday that one of the big benchmarks for measuring the effectiveness of frontier agents had a bunch of bogus data in it. Some non-profit did a nice analysis of it and smart people are like, "Ooh, this is actually pretty bad." Humanity's Last Exam had a bunch of bogus data, people are measuring and optimizing for the entirely wrong thing, and agents are getting all messed up as a result of it.

**00:28**
**Speaker:** Now, who made Humanity's Last Exam? Hey Grok. Oh, it was designed by uh, some AI Safety Institute in collaboration with Scale AI. Oh, oh, oh, the dude who’s helping uh, Meta now become a closed source lab now. Uh, throw enough data slop at the wall from your overseas data farms to get hired, but you know, you're screwing with the rest of the ecosystem. Okay. Why are these people designing benchmarks that people care about? Screw that.

**00:58**
**Speaker:** Um, who should we trust instead to make benchmarks? You know who I think would be really good? Is developers themselves, who may also be gamers. Gamers know what to measure. Min-maxing? Oh my goodness.

**01:13**
**Speaker:** We're playing StarCraft here, people. We are inspired by StarCraft. And you know what I want to measure? Is what is the amount of effective actions per minute that my agents take. I want to maximize that. So, let's just take a page from StarCraft.

**01:34**
**Speaker:** *[Reading from screen]* In StarCraft, Actions Per Minute measures how rapidly players interact with StarCraft through mouse clicks and keyboard commands, serving as the primary metric of mechanical skill in competitive play.

**01:46**
**Speaker:** What's the equivalent of that for, um, agents? So I think for now, we're going to uh, draw a little parallel between StarCraft's APM and the APM of our agents. And that APM would be what? Messages to and from the agent, and tool calls. We'll start with that. We can sophisticate it over time, blah, blah, blah. But, um, Claude Code also has some built-in telemetry support. So we're going to start sophisticating kind of the analytics here.

**02:24**
**Speaker:** And so, um, anyway, just took a very quick and dirty, fast pass at this. Just give me the Actions Per Minute of me analyzing all of the Claude Code conversations—because it's all on my desktop, uh, saved, all of the two months of conversations that I've had. Just go in and analyze it. What's my APM over the last hour, six hours, day, week, month, lifetime? I have a lifetime APM of 2.298. Any of you are welcome to download the Tauri app and run it yourself, let's see.

**03:04**
**Speaker:** Um, so here's our baseline, and we're going to try to improve this. By the way, check this out. Uh, let's open up a chat window. Ooh. Ooh. Yeah. Oh, another chat window? Oh yeah. Oh yeah. Oh Markey, eat your heart out.

**03:26**
**Speaker:** All right, uh, any feedback on uh, APM measurement, benchmark stuff, let us know. We are going to keep a doc of the APM on our GitHub repo. Uh, that'll be at `openagents-inc/openagents`. `docs`. Uh, there'll be a thing, something in here that says `apm.md` and we'll keep that as sort of a running spec of how we're measuring A— uh, APM. There could be different versions of that, effective... different sorts of vetting. But uh, yeah, let's give it a try. See you soon.
