**[00:00]**
**[Visual: A dark screen displaying a Markdown file titled "Install Pylon" with instructions. In the bottom right circle overlay, the speaker addresses the camera.]**

All right folks, we're launching the first versions of two pieces of software: Pylon and Nexus. We'll step through that in a minute. Allow me for a moment though, to do a little, a little cinematic launch trailer. Okay? May contain content inappropriate for children. Yes, so courtesy of, of Blizzard.

**[00:19]**
**[Visual: A cinematic trailer begins playing, featuring the Blizzard Entertainment logo, followed by dramatic sci-fi imagery.]**

You know Blizzard is now owned by Microsoft, so best case scenario someday, we're gonna push Microsoft out of the Mag 7 and acquire Blizzard from them. We're gonna make Starcraft 3 and then World of Starcraft. Okay? All right, so let's mute... I think that's Tassadar.

**[00:46]**
**[Visual: The speaker minimizes the video slightly to show his desktop dock, then overlays a transparent terminal window on top of the playing video.]**

Okay, watch this. So I've got a little markdown thing here. Pay attention here, we're gonna show you what a Pylon looks like. You gotta get it. You gotta get it.

**[01:00]**
**[Visual: The video shows a Protoss probe unit in a battle scene.]**

So middle of a battle, you got the Protoss fighting the Zerg. The little guy there is a Probe. Oh yeah. Fighting, fighting, fighting. Okay, so he's gonna start summoning a Pylon. Okay?

**[01:25]**
**[Visual: The Probe unit initiates a summoning sequence. The speaker types `probe spawn pylon` into the terminal overlay.]**

Now of course, all these other guys are gonna try to stop that. Okay, so Pylon is the thing you run on your computer that lets you sell your compute for Bitcoin. It includes the Bitcoin wallet. Soon it's gonna let you sell other types of things, like maybe you're putting Claude Code to work on certain jobs. But Pylon is the node that you run on your computer. Ooh, here's an Ultralisk.

**[01:56]**
**[Visual: The video shows High Templar units merging. The speaker types `pylon - swarm compute node`.]**

Those guys are High Templar, they're gonna turn into an Archon. Nexus... so Pylon is the swarm compute node. And then you've got Nexus...

**[02:14]**
**[Visual: The video cuts to a massive structure. The speaker types `nexus - swarm relay`.]**

...is the swarm relay. So Nexus right now is basically just a glorified Nostr relay. We're gonna be putting a few optimizations in there, maybe specific to agents. We think these are gonna be relatively high throughput, way higher than average Nostr relays, so we might need to get creative on indexing and stuff. But Nexus is something... so we're gonna run our own Nexus, nexus.openagents.com, that things will connect to by default. And you can run your own Nexus.

**[02:53]**
**[Visual: In the video, a massive Pylon structure warps in, creating a blue energy field. Units begin warping in.]**

Now watch this. Pylon! Boom! Okay. You can teleport things in. So just imagine that's a bunch of agents and we're fighting... who are we fighting? A bunch of closed agents? Okay. Thank you, Blizzard.

**[03:15]**
**[Visual: The speaker closes the video and switches to a web browser showing the "Nexus Relay" dashboard, then navigates to a GitHub repository page.]**

Okay, so all of this is open source on our GitHub repo. In the crates—so all this is Rust. Pylon code is in Pylon. There's probably going to be a UI in a separate crate, Pylon Desktop. Right now this is all CLI, so your agents can drive it very easily.

**[03:41]**
**[Visual: The speaker switches back to the "Install Pylon" instruction file.]**

The instructions for all this, it's literally you're just pasting it to your agent. This says Silicon Mac is required. We actually went ahead also and added support for Ollama, which is not thoroughly tested. So this maybe should work. Whether it does now or it takes a day or two, we do want people regardless if you're on Linux or Mac... or Windows to be able to run this. It's just Rust, so we'll get all that ironed out in the next couple of days.

**[04:21]**
**[Visual: The speaker highlights the installation steps.]**

Okay, so Pylon, you run on your computer. You go online for jobs. It's literally, you start the thing and then you're online. Your computer will kind of compete for requests. It'll do the request, basically inference or some inference-related job. And then if you get the job and you do it, then you get paid Bitcoin.

For the first week, we're keeping this in Testnet, using what's called Regtest. It's like a simulated Bitcoin network.

**[04:55]**
**[Visual: The speaker navigates to the "Lightspark Regtest Faucet" website.]**

We're using the Spark network. It's like a Bitcoin L2 that combines Bitcoin Lightning and some other wizardry that they've got. So they maintain a Regtest faucet so you can get Bitcoin to there for free. Or fake Bitcoin to there for free.

**[05:19]**
**[Visual: The speaker opens a terminal window running an agent interface (labeled "Claude Code").]**

So let's give it a try. So just to give an example of how you can use this. So let's say, "Make a Pylon wallet, tell me the Bitcoin address." This is basically for now intended to be driven by Claude Code, or your other, you know, coding agent. We'll have this be more kind of human-friendly if you want to see docs and all that stuff yourself.

**[05:53]**
**[Visual: The terminal outputs a Bitcoin address starting with `spark`. The speaker instructs the agent to correct it.]**

So, yeah, the Bitcoin address is... that's a Spark address. No, I want the Bitcoin address starting in `bc`. There's enough like help functionalities with the CLI so if you could just nudge it, it'll figure it out. There we go. I think that's it.

**[06:58]**
**[Visual: The terminal displays a Bitcoin address starting with `bcrt`. The speaker highlights it.]**

Okay, the Bitcoin address is this. That starts with `bcrt` cause it's Regtest. Yeah, that's right. Okay, so you put that in there and you can put up to 50,000 sats. And you'll get them in about 60 seconds or less. Usually within 30 seconds.

**[07:20]**
**[Visual: The agent in the terminal asks if the user wants to add mainnet support because Pylon is "hardcoded to regtest". The speaker types a correction.]**

No, we are intentionally on Regtest. Whatever was confusing to you about not finding that, fix it. Okay, so, this is alpha quality software. It's very rough. Your agent should be able to navigate around. If there's anything that seems unclear or is a pain in the ass or is not working that should, please feel free to open a GitHub issue on our repo. Would really appreciate that. We're doing this to get that kind of feedback.

**[08:01]**
**[Visual: The terminal shows code diffs where the agent is modifying source code to fix the address display issue.]**

I generally prefer people not submit PRs, until we get like our hooks and contributing guidelines dialed in for agents. I want to be doing most of the coding, but it'll be done very fast if you give me a good GitHub issue.

**[08:23]**
**[Visual: The speaker briefly shows the "Nexus Relay" web dashboard again.]**

Okay. So Pylon can connect to one or more Nexuses, or Nostr relays. And you're welcome to try installing and running your own right now. Nexus runs really only on Cloudflare Workers. We'll support other backends over time.

Nexus has... every Nexus at the root of the domain—you know if you're connecting to it from the Pylon, you're connecting to the websocket root domain. If you visit it in a browser, you'll see some stats. And this is just a very basic UI. We'll kind of flesh this out into more relevant stats and throughput visualizations over time. But the different event kinds that are happening. How many completed jobs? We've done 377 jobs. Like what the different kinds are.

**[09:16]**
**[Visual: The speaker points to "Kind 5940" in the Job Marketplace list.]**

We defined a new event Kind 5940 for RLM events, as distinct from 5050 which is the usual like just basic text inference. And we're going to be really testing out this idea we discussed in the last episode about Recursive Language Models to see if this model of, you know, basically swarm compute, works well with RLMs. So a bunch of the stuff that you'll see from us next, and some of the beyond the kind of any bug fixing or smoothing this out on the way towards Mainnet going live with real Bitcoin next week, is we're going to be really flushing out this RLM section.

**[10:07]**
**[Visual: The speaker continues looking at the Nexus dashboard.]**

Because I think that we may be positioned to uniquely push that forward and explore the frontier of what it's like to coordinate multiple devices into a massive network for test-time compute. And to have that monetized from day one is pretty cool.

**[10:31]**
**[Visual: The speaker opens a new terminal tab and types instructions for the agent.]**

So there's also an RLM CLI command over here. Let's start a new tab. And tell it to... First let's just say like, "Read the CLI about Pylon RLM, tell me what that does." No, just run the help commands.

**[11:18]**
**[Visual: The terminal displays the help text for `pylon rlm`.]**

So `pylon rlm` runs recursive language model queries on the swarm compute network. It submits your query as a NIP-90 job—that's the Data Vending Machine job—to Nostr relays. Providers on the network pick up and process the job, results are returned to you. You can analyze a file, max concurrent sub-queries, a budget, local only just test it yourself.

**[11:45]**
**[Visual: The speaker types `test the rlm thing on yeah summarize SYNTHESIS.md in our root of this folder`.]**

And then let's try... Okay, yeah, let's test the RLM thing on, yeah, summarize `SYNTHESIS.md` in our root of this folder. So `SYNTHESIS.md` is our massive like 18-page super strategy doc all about OpenAgents and that would be probably a good doc to test. Let's see what it does.

**[12:12]**
**[Visual: The terminal begins processing. Text indicates "Chunked file into 103 fragments" and starts processing chunks.]**

So the RLM integration that's in here is like basic and untested, largely. I mean like basic functionality works and like jobs proceed. None of this is probably like useful yet. There's some tuning that we'll need to do to our integration. But mainly we're putting this out now to start getting kind of a stream of data from people running these. And like let's start to model out what actually happens when you've got five different providers online. Is there different algorithms that we need to explore for like matchmaking or how are things being routed? There's a lot of kind of unique considerations we'll just need to explore and bump into and such.

**[12:59]**
**[Visual: The speaker addresses the camera while the terminal continues processing in the background.]**

But I'm, I'm very bullish and excited about RLMs. I think it's a perfect fit that answers the question that we had two years ago when we phased out our earlier swarm compute network. We did not have any really compelling buy-side use case to kind of have demand for the compute. I think that the type of compute that RLM wants for the kind of async fan-outs is perfect for this.

**[13:30]**
**[Visual: The terminal shows "Task Output" appearing.]**

So we'll just need to do a little bit of tuning of that to make sure that it's actually useful in the context of a coding agent workflow. So these are sort of primitives that enable anybody to come in and use this. Some of you may even figure out good ways of applying this model to RLMs or anything else even before we do.

**[13:58]**
**[Visual: The speaker gestures to the screen.]**

But once we get these basic pieces in place, we're going to be using this swarm compute hopefully heavily via our AutoPilot product. You know, so just like we've got Pylon and Nexus in this repo, we also have AutoPilot with a nice UI. We'll do some videos on that in a few days. But we envision AutoPilot using this compute heavily via RLM, via other things maybe.

**[14:29]**
**[Visual: The processing completes in the terminal.]**

And then people are going to be hopefully paying us to use AutoPilot cause it's going to be an amazing coding agent that combines Opus and all the best things from Claude Code with RLM. Because of the way that we're structuring this, it allows people like *you* to provide some of the compute used by those coding agents. So it's like we've got revenue share built in so that we all get paid. We all have a financial incentive to improve really everything here. And hopefully this is also the path to creating an industry-leading coding agent.

**[15:12]**
**[Visual: The speaker waves goodbye.]**

So lots here. Lots of work still to do, but we'll be updating this rapidly. And if nothing else, check back in a week for when we go live on Bitcoin's Mainnet and start getting some real funds flying around. See you soon.
