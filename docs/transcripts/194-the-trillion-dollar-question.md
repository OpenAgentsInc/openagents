The End of Cloud Inference? Maybe.

People just remember, just remember that when people around T-Pot, AI Twitter, days or weeks from now, talk about how cool the Foundation Models API is from Apple, that you heard it here first, okay?

For some reason, all of you are sleeping on it—fine, good for me—but Apple has put out amazing new primitives for app devs. Rivaling, and in some cases exceeding, the Vercel AI SDK in my own experience and what I can tell of its potential. Massively, massively impressed by it.

And uh, it seems to be working fine. This is the first ever that I’ve seen... agentic search through a code base, grep, and all of that stuff using tool calls. You *can* run it on device. I have it hooked up because we're doing a desktop-mobile sync, I just run it on the M2 chip and then stream it over websockets to my mobile device for controlling Claude Code. But, you know, we're going to be supplementing Claude Code and Codex with the on-device model.

Which got me thinking.

Hey ChatGPT. What percentage of AI workloads in the next 5 years can or will run on Apple Silicon? Give me a range of possibilities. And explore the ramifications for OpenAI, NVIDIA, and other major players if certain percentages of AI workloads move from cloud to the edge.

So I fed this to ChatGPT Pro, which can search the internet for sources and stuff like that. I also fed in recent tweets and analysis, a little bit of this, a little bit of that, Chris Paik's essay... and I’m going to share this link in the tweet below this, you’ll see it. Summaries, blah blah blah... got a one-liner down here:

> "If the next five years play out as current vectors suggest, Apple Silicon could end up running 15–25% of the world’s AI inference by 2030, with a credible 7–31% total range... and that swing is large enough to reshape GPU demand, API business models, and who captures the economics of everyday AI."

And then I asked it to do a little bit of math about like, what is the actual market cap implications of all that. Is it correct to call this a trillion-dollar question? Yeah, yeah, easily a trillion-dollar swing. *[Speaker gives a thumbs up]*

And uh, what does it say up here for... being able to tell if it's on track to being on the high end of that 7 to 30%?

**What to watch for. What would have to be true for the high scenario?**

1.  Small capable models. It’s already got one of these; the one on there is, you know, like equivalent of 4B.
2.  *[Skipping]*
3.  PCC scale-out—the sort of central cloud that things can elevate to if you want a more powerful model.
4.  Hardware cadence.

Okay. Now I don’t have much control over 1, 3, and 4 there.

**Number 2, however: Developer adoption of Apple’s Foundation Models plus MLX is broad.**

None of you people have been talking about this *at all*. You didn't know—I guess you were waiting for me to make a video about it. Is that right?

Okay, um, well we’re going to continue doing what we’re doing, which is putting out open source releases on our GitHub. Going to continue building in public. We’re going to continue making videos.

And I’m going to chronicle for you over the next few videos, over hopefully the next week or so... what percentage of our *own* workload—as I’m focusing on coding agents, specifically letting you manage the top coding agents, currently Claude Code and Codex from your phone—it’s like, what percentage of our own coding agent workload is in the cloud?

Well, right now it’s 100%. But with version 0.3 of our Tricoder app in a few days, that’s going to drop from 100% to, I don't know, 95%. In the beginning, we’re just going to be using the local model for a little bit of orchestration, we’re generating title summaries... but like, 100 is going to drop to 95. And then it might keep dropping. What other things can we strip out and go local? Can we bring that down to 50%? 20%?

At what point does *that* start having broader ramifications?

So, stay tuned. Uh, if you’re a developer who wants to uh... leverage the best coding agents from your phone, go sign up for our TestFlight. This is going to be iOS only for probably the foreseeable future, next month or so at least. Go sign up.

If you’re an angel investor or an early-stage VC fund and you want exposure to this thesis... my DMs are open. Message me sooner than later, I recommend. See ya.
