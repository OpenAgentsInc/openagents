**[00:00]**
**[Visual: A black slide with white text reads "2025 was not the year of agents". A circular video overlay of the speaker is in the bottom right corner.]**

So no, 2025 was not the year of agents. It was the year of copilots. Agentic copilots, copilot agents.

**[00:09]**
**[Visual: The slide changes to "Copilots". Underneath, it lists categories: IDE (Cursor, Windsurf, Antigravity), Terminal (Claude Code, Codex, Warp), Browser (Dia, Atlas), and Enterprise (Copilot®).]**

You take an IDE, you take a terminal, you take a browser, you add some agentic workflows. You have an LLM in a loop calling tools, steered by you. It's powerful. Makes you feel cool. Not agents.

**[00:28]**
**[Visual: Text appears at the bottom of the slide: "Agentic – but not agents."]**

Real agents have not been tried. What do we mean by agents?

**[00:34]**
**[Visual: The slide changes to "Agents". It lists four bullet points: Autonomous, Long-lived, Learn & evolve, Autopilots not copilots.]**

Agents should be autonomous, long-lived, they learn and evolve. They are autopilots, not copilots.

**[00:46]**
**[Visual: The slide changes to "2025 was the year of copilots".]**

So 2025 was the year of copilots.

**[00:52]**
**[Visual: The text changes to "2026 is the year of autopilots".]**

2026 is the year of autopilots. You heard it here first.

**[00:57]**
**[Visual: The screen shows a Google search result for "Microsoft Copilot". The speaker points at the "Registered Trademark" symbol.]**

So, let's take a page out of Microsoft's playbook here. Look at this. Microsoft Copilot. R. Registered trademark. I don't want to do that.

**[01:08]**
**[Visual: A slide displays "Microsoft Copilot®". The text is crossed out.]**

But I do like jumping on the name. Not Copilot. Lame.

**[01:13]**
**[Visual: The text changes to "OpenAgents Autopilot".]**

How about OpenAgents Autopilot. Yes. Okay. So this is it, everybody. This is it.

**[01:21]**
**[Visual: A web browser shows a post on X (formerly Twitter) by OpenAgents titled "Episode 158: Quest for the Holy Grail".]**

This is the culmination of what we've been working towards since February of this year. Our quest for the holy grail of agentic software development. An agent that can reliably code, can convert issues to PRs, or push directly to main, in an automated loop. Everyone's moving in this direction. We are there.

**[01:43]**
**[Visual: The slide "OpenAgents Autopilot" returns. The speaker looks thoughtful.]**

We are there. I've had Autopilot running for almost 24 hours out of the last 24 hours. It's run overnight reliably twice.

**[01:57]**
**[Visual: The browser shows the `openagents/openagents` GitHub repository architecture diagram.]**

Just come take a look at the freaking commits on this repo. Look, it's coding while I'm talking.

**[02:04]**
**[Visual: The speaker navigates to the Commits page on GitHub. New commits are visible from minutes ago.]**

Yeah, added the APM storage and the trajectory collector. It's tomorrow already, wow. It keeps trying to add GitHub workflows, don't do that. Just these commits are so good. Now, you've probably in the past let your agents run wild, make commits, and the code degrades over time. The code is getting better over time. It's amazing.

**[02:35]**
**[Visual: A slide appears with four sections: "Open source", "Open brand", "Mech suit", and "Revenue sharing".]**

Okay. So the point is that this is working. Why will you like this? It's open source. Just use it. We were gonna make a landing page and email capture and tell you all this stuff. No, no, no.

**[02:45]**
**[Visual: The speaker shows the GitHub README for OpenAgents, highlighting the "Why You Should Care" section.]**

Point your agent at the repo, `openagents/openagents`, and ask it to tell you about Autopilot and how it worked for us and how it can work for you. Okay? We'll add more human docs soon, but here's the "all of the what we're building." Autopilot is one big piece of that.

**[03:03]**
**[Visual: Returns to the four-point slide. The cursor hovers over "Open brand".]**

Open brand. Make your own Autopilot, competing companies. I don't care. Like, I hope you do. Let's compare Autopilots. The more exciting thing is when we start really having our Autopilots learn from each other. So more on the infrastructure enabling that soon.

**[03:24]**
**[Visual: Cursor hovers over "Mech suit".]**

Mech suit. It works with any agent. We're focusing on Claude Code first. Claude Code's agent SDK is very, very, very good. They did not have a Rust version; our whole thing is in Rust. We wrote a Rust version just by porting it from TypeScript. It's powerful as heck. We don't want to reinvent Claude Code right now because it's the best in some ways. But just by putting it in this mech suit, was able to get it to code reliably overnight. So that's nice.

**[03:53]**
**[Visual: Cursor hovers over "Revenue sharing". The speaker then switches to a GitHub Wiki page titled "Video Series".]**

Revenue sharing. Okay, go back to Episode 1 of our video series. This is episode 199. It's all here on our Wiki. Episode 1 was all about, it was the day after DevDay 2023, Sam Altman was like, "Hey we're gonna do a GPT store and do revenue sharing." I was like, there's no way. They're definitely gonna half-ass that. And they did.

**[04:14]**
**[Visual: The speaker shows an X post from Ryan Carson: "I spent $200 on these Skills and it was worth 5x that."]**

But we want everyone to get paid to use this. Here's an example. Ryan Carson. "I spent $200 on these skills and it was worth 5x that. We're going to see agent skills marketplaces appear soon." Yes. If you're spending on things that you discover, you could choose to share and keep those private, or you should have a way of listing those on a marketplace and getting paid for them.

**[04:41]**
**[Visual: The speaker scrolls through X replies.]**

So it's really great that there's already a ton of really cool stuff being shared openly, open source. That's really great. But we believe that if there's another layer to it where people can optionally set prices on things that they're sharing... If you have a financial incentive, then maybe you're gonna go the extra mile and make your skills extra good. Maybe you're gonna be extra motivated to maintain it, and the usual decay in open source projects because people don't have a financial incentive to maintain them, maybe that starts to shift if you're getting paid a stream of micropayments whenever somebody uses your thing.

**[05:14]**
**[Visual: The speaker navigates to a post showing code and a diagram about "Sovereign Agents" and "NIP-54".]**

So one of the like, things that we've been coding here is a way to ensure through these key splits... because agents are able to hold their own private keys split across different guardians or different Frostr nodes, we can basically help ensure that paid agent skills can't be copy-pasted to a different agent. All sorts of cool stuff that we can do with cryptography. We'll do separate videos on that.

**[05:43]**
**[Visual: Returns to the four-point slide. The speaker looks directly at the camera.]**

So let's get you paid. Let's get Autopilots as the best, coolest way of using these agents. Over the next day or two you'll start seeing us rolling out our GUI for all of this. Their kind of like Starcraft-style HUD UI. This is all kind of CLI right now.

**[06:06]**
**[Visual: The speaker gestures with his hands.]**

So we're gonna start doing about daily videos rolling all this out, starting with our next episode, Episode 200. We're gonna zoom way out and we're gonna tell you the big picture of what we're building and why, and how you can help. See you soon.
