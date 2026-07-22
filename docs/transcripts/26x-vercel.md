# Transcription: OpenAgents Episode 26X - Raiding Vercel and Porting the AI SDK Harness

Media source: `Raiding Vercel and Porting AISDK Harness.mp4`
Transcript range: 00:01:34 to end
Transcript runtime: 00:05:25.66
Timestamp basis: source video
Transcription model: gpt-4o-transcribe-diarize
Generated at: 2026-07-21

Machine-generated transcript, reviewed against a prompted transcription pass and
the cited OpenAgents AI SDK harness analysis for product names and technical
terms. Verify wording against the video before using it as quote-grade source
material.

**[01:34] Christopher David:** Not contributing back to upstream whatsoever.

**[01:42]** Hey, let's start raiding stuff. You know, absorbing code that we want from wherever. Let's start with Vercel, who thought it was a good idea to ship a product called OpenAgents. Uh-huh. Uh-huh. And they haven't let me know where to send the lawsuits and the C&Ds. So we're just gonna, like, go take some of their code.

**[01:58]** Why? We're gonna take their code from AI SDK. A couple reasons. One, we need this harness glue code, okay?

**[02:05]** They're good at gluing things. Vercel is good at playing with glue. They probably played with glue as kids a lot, you know what I'm saying? And so they're good at gluing things.

**[02:09]** Now, we need it because we have in our app, you know, multiple of these agents. We got Codex, Claude Code, Grok, Apple FM. We'll add OpenCode. And then it's like, hey, you know, do a task, and then this will delegate to one of those subagents, okay?

**[02:32]** Codex, you know, you've been chosen to do this task. Session failed. Okay, so we need this glue code, which is, they have harnesses.

**[02:42]** Experimental harness, blah blah blah, for applying their Vercel glue to some harnesses. And they've got support for, what do they say? What do they say? Somewhere in here they say they support Claude Code, Codex, OpenCode, a few others. And, you know, maybe we would use their code.

**[03:08]** Because our stuff is in TypeScript primarily, but our stuff is in Effect. We are Effect maxis here, and AI SDK, you know, just in, like, old legacy ugly vanilla TypeScript, we just can't handle it. So we need all this in Effect. We're gonna do it ourselves.

**[03:29]** So we have an epic here for Effect ports of the AI SDK harness layer for provider lanes, blah blah blah. So we're starting with this harness thing. We may end up doing just a full port of the AI SDK to Effect, the parts that we want to preserve. And, hey, maybe we'll make it even, like, a standalone library. And maybe we'll call it, we'll call it Vercel. We'll call it Vercel.

**[03:53]** Like V-R space C-E-L. Yeah. Okay. That's cool.

**[03:59]** Let's find the audit we did.

**[04:02]** So I had Fable go around and do some analysis here. Let's take a look at what it said.

**[04:10]** All this is an analysis, open source, by the way. We love putting all of this stuff on our GitHub.

**[04:18]** Okay. AI SDK Harness Abstraction: Harvest Analysis for Full Auto, Managed Sandboxes, and Apple FM Routing.

**[04:28]** Okay. So the AI SDK repo now carries a complete, shipped harness abstraction, one typed surface that runs five established coding-agent runtimes: Claude Code, Codex, Deep Agents, OpenCode, and Pi, behind a pluggable sandbox-provider seam with durable turn suspension, yada yada yada. Goose, Amp, Mastra coming soon. Great.

**[04:46]** OpenAgents independently built the surrounding pieces: a managed-sandbox substrate with a Box v1 compatibility facade, orchestrator that rotates these lanes under durable leases and caps, and an Apple FM router.

**[04:56]** It won't just be Apple FM, but that's just what I'm using because they have that on my computer.

**[05:01]** But the desktop runtime integrations underneath those systems are bespoke per provider, split across two parallel execution stacks, and uneven on recovery.

**[05:10]** So what exactly does the AI SDK harness abstraction contain? Which parts are worth harvesting as Effect ports of the ideas? Where do those ideas land in the OpenAgents architecture? What is the concrete proposal-packet shape for adopting them?

**[05:29]** The headline finding is OpenAgents has already adopted the AI SDK's sandbox seam at the edges, blah blah blah, and we already own stronger durable authority than the AI SDK will ever ship, but it lacks the middle layer: a single versioned adapter contract with uniform turn suspend/continue semantics. That middle layer is the harness.

**[05:49]** Okay. Harvest. So yeah, we'll take their middle glue stuff. Isn't Vercel just a big pile of middle glue? That's just kind of, like, what their company is.

**[05:54]** They glue AWS and abstractions together and then, like, charge you a bajillion dollars for that? That's an interesting business model.

**[06:05]** We'll see how long that lasts. Let's see. The AI SDK harness layer is the missing middle of the OpenAgents agent stack.

**[06:12]** OpenAgents already has the better sandbox authority below it and the better orchestration, routing, and settlement above it. But every runtime in between is a bespoke integration with uneven recovery.

**[06:21]** The harvest is one Effect-native contract: session verbs with uniform suspend/continue; one stream vocabulary that `KhalaRuntimeEvent` already nearly is; capability by method presence with typed refusal; neutral skills/host tools; and durable-log replay.

**[06:35]** Implemented by the four existing lanes and consumed by both the Full Auto dispatcher and the Apple FM router. Nothing is vendored. The ideas are re-derived onto contracts the repo already owns. And the two places the repo touches AI SDK types stay the honest interop seams. Okay.

**[06:47]** So we have these issues, and I'll just presto change-o overnight to show you what it looks like after these are all implemented. Let's go.
