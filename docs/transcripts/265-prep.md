# Transcription: OpenAgents Episode 265 Prep

Media source: `OpenAgents on X_ _Episode 264_ …s https___t.co_g34yja90tp_ _ X - 2 August 2026.mp4`
Transcript runtime: 00:56:17.46
Timestamp basis: source video
Transcription model: gpt-4o-transcribe-diarize
Generated at: 2026-08-02

Machine-generated transcript, reviewed for recurring project names and technical
terms. Verify wording against the video before using it as quote-grade source
material.

**[00:00] Christopher David:** All right, let's continue. We are building this security invariant and evidence workbench. All right, let's continue. Hey, let's continue. So we're gonna build this workbench with our own vulnerability-scanning setup, kind of wrapping Loupe with some forensics and cool stuff built in. Part of why this is needed: today there was a bunch of advice like this floating around, basically like, hey, everybody, go use Kimi and find problems with repositories. You know, if you're a savvy engineer like Ben Carman here,

**[00:55]** you can probably get some signal from that and find some stuff that's good. If you are non-technical, like this gentleman, sorry to pick on you Scott, but Scott admits here that he's non-technical and he's just like pointing his Kimi credits at... These things and coming up with, first of all, a big ass list of supposed vulnerabilities.

**[01:24]** Uh foundation guy Zach says, this isn't a very responsible thing to publish. Kimi, for example, thinks that there's a critical vulnerability in Passport Core firmware, which is incorrect. I don't think Kimi is as good at tracking down vulnerabilities in embedded firmware compared to Fable or Sol. Yeah. Uh, Kimi, particularly by itself, if you're just pointing Kimi at a repo, it'll be like, find problems. No. If it does find vulnerabilities, dumping them on the timeline of x.com is not how you report them. That's not how you report vulnerabilities.

**[01:57]** Hey, everyone, here it is. You notify the project. Uh, if... If you're going to YOLO, at least run Project Loupe, which actually has proper system prompts and some verification built in. There's one pass it does where it has an adversarial review like, hey, double-check everything you just did. And it's still very naive and basic, but it at least with some prodding was able to discover the Coldcard vulnerability and...

**[02:27]** So we're going to be kind of building our own harness around Loupe, and then hopefully this can be something where the types of people who want to contribute have better tools than just pointing Kimi at stuff. I'll say one more thing on the cyber point. You know, a bunch of people also have been more or less correctly complaining at OpenAI because they have a bunch of these refusals for cyber things, and that is true. However, the refusals largely go away. I haven't had any after getting cyber access added to my account.

**[03:17]** I had to upload my ID. That sucks. But I got cyber access for OpenAgents, no refusals when I ran Loupe using GPT models. Now, however, I did say that to Alex Thorn, who's leading the Galaxy investigation. And he said, even with cyber, we are hitting safeguards that are unnecessary, unfortunately, even when extracting data from my own database.

**[03:46]** So that is crappy and OpenAI should loosen those. However, here is where harness engineering comes into play, because it's really not an either-or situation. There can and should be a workflow. that uses GPT models or other top tier models when it can if it hits a problem like fall back to Kimi or as I've been able to do with Fable multiple times when you hit a refusal you can just rework your prompt and like get it through anyways like people said oh Fable doesn't help with distributed training when you see earlier in this series we had like a bunch of Fable assisting and distributed training you sometimes you just got like get crafty with the wording um so ideally there's just like a harness that can kind of take care of these things so this is are some of the like principles that we're going to be putting into our version of this now the um suggestion from uh codex in the last video about what OpenAgents should build

**[04:57]** This thing here. We built a bunch of that and turned most of it into a bunch of issues. So issue 9300 on the OpenAgents repo is, like, you know, we built a whole forensic-analysis roadmap. Begin with configurable, measurable, Loupe-style analysis inside Omega, our IDE, on admitted OpenAgents Cloud Linux workers. So, for example, to run Loupe you have to have a Linux box. We have a bunch of Linux capacity, so let's make it so you can easily click a button and run the vulnerability scans using our infrastructure. Probably good to have some of that run in a way that we can monitor it. You have to assume that,

**[05:51]** Adversaries are tracking all this stuff. So even though we're gonna be building in the open we still got to be Using our brain a little bit about who's gonna be using this for what? Okay So independently reproduce the Coldcard incidents evidence chain that generalized through governed prompt optimization Artifact witnesses executable evidence variants and coordinated remediation The near-term success condition is not merely Omega as a scanner. Omega must run a reproducible experiment, improve the analysis program without moving the scoring goal posts, and show why a candidate is better on evidence you couldn't understand.

**[06:22]** Yep. Sure, this still just can't explain freaking simply. Let's pop open our... Let's pop open our dev build here of... Omega and just try to make some sense of this. To do the UI overhaul here this is still heavily work in progress but let's see if we can do Omega.

**[06:54]** Is Omega what I want here? I actually do want OpenAgents for the analysis. Okay, so let's say: read everything in `docs/loupe` and `docs/coldcard`, then summarize issue, what did I say, 9300?

**[07:40]** Alright, so let's just get an agent kind of up to speed on where we're at. In this interface we have the kind of basic chat powered by GPT Luna. And then it can delegate out to Codex or other powerful agents, Claude Code.

**[08:13]** Omega here: I read all 22 files under Loupe and Coldcard, then reviewed the issue. Okay, so issue summary. Objective: build Omega into a governed forensic security workbench, beginning with Loupe-style source analysis running on admitted OpenAgents Cloud Linux workers. The program must reproduce the Coldcard incident evidence chain independently; measure analysis quality, reliability, efficiency, and uncertainty; improve prompts and analysis programs without changing scoring criteria; demonstrate improvement on untouched holdout evidence; and expand from detection into artifact witnesses, executable proof, variant discovery, and coordinated remediation.

**[08:52]** Yeah. So let's just try this also. Uh, put in two sentences what the completed forensic workbench will be able to do and why we're building this. Pretend you're talking to a fifth grader. Just speak plainly real quick. The forensic workbench will help Omega carefully inspect computer projects, find suspicious problems.

**[09:21]** And show clear proof of what happened without exposing private secrets. We are building it so people can trust the results, fix problems safely, and stop similar attacks from happening again. Okay, let's also go to the counter example of what I said we don't want the Kimi Slop. What is this? Alright, let's go down here.

**[09:56]** Let's see if I can screenshot this and save this and let's see if my omega can handle image uploads. Yeah.

**[10:31]** Summarize what you see in these images. Let's just test this first. The images show a public post explaining a security review of several Bitcoin wallet products. Okay. Well, that works. So let's also grab the commentary here. So I can say, summarize Oops.

**[11:17]** also what you see here. So we need to build into our workbench plans. Um, like the proper product shape that can provide an answer to this, like we're building in the proper verifications, we're not just using one Chinese model, we're using the best models with appropriate fallbacks if there's a security, you know, error preventing us from getting answer to it.

**[11:53]** from a particular model. Give me some quick thoughts on how our forensic workbench should reflect these concerns. Okay. The workbench should make verification not model confidence the product.

**[12:20]** Hey, verification, our favorite word. Shout out to Christian Catalini and our episode 259 about verifiable software. And the core thesis of OpenAgents will leave that as an exercise for the reader to do their homework. Okay, so. The workbench should use multiple independent models with different strengths, including embedded firmware and cryptography, instead of relying on one model or one provider.

**[12:46]** Require agreement and evidence uncertainty clearly, handle model failure explicitly, use self-fallbacks, keep the scoring fixed, separate claims, add an exception path, protect sensitive material, publish a better gap. In short, the workbench should behave like a forensic panel and audit trail, not a chat bot. Models propose independent check challenge controlled experiment test and only evidence-based conclusions graduate to report. Cool, yeah, so we want all that. Okay, let's try this.

**[13:12]** Now I want to involve the smarter Codex models. Let's try this. I want you to delegate to Codex. Uh, updating our plans to reflect this, like acquaint them with the security posts that I gave to you just now and these new considerations. Um, write me a new sort of updated audit, put it in the Coldcard DOCS folder based on this.

**[13:50]** With updated considerations. Don't update any other docs; for right now, just give me some analysis based on all this. So do that. I don't know if my subagents yet can choose which variant of GPT-5.6. Let's see. No, that went to Terra.

**[14:21]** Darn. Let me check on my updated, um... This is all a work in progress, our IDE here. So we're gonna be extending the IDE.

**[14:54]** See Omega. Then out of curiosity on this one. Oh, I've got to pick this first.

**[15:23]** All right, so I'm picking my repo. Let's do OpenAgents. Read everything in `docs/loupe` and `docs/coldcard` and summarize here.

**[15:57]** Okay, so aside from the delegation, I also added that you can just talk to the bare Codex agent. Alright, this is the previous build that I have. docs/loupe Codex analysis. Think that's old Bear with me here Okay, so here.

**[16:35]** I actually just have Sol in my Thing this will be fine. I Love these tool calls I can like actually see what the heck's going on and then pretty soon I'll have it like if you want to read the file I'll have this open up a right sidebar so you get like the editor view of it never mind it's already there it's so beautiful it's also very rough we'll put out an alpha build in a day or so and you can build this from source from our omega repo anytime

**[17:29]** Alright, so right now this is summarizing. Let me just check what my reasoning level is. Extra high, fantastic. Fast mode off. Well, let's flip it on just for you fine people here. And then I want to make sure that it had access to the latest. Okay, I've completed the Loupe material except for the road map lane.

**[17:58]** The central correction is already clear. The documents first predicted that Loupe would miss Coldcard for architectural reasons, but the pre-registered two-arm experiment refuted that. Blah, blah, blah. I'm now reconciling that chronology with the later implemented system and the independent Coldcard evidence. Yeah, this is how I want to consume Codex, by the way. I can do Command-N for a new tab, new thread, Command-W to close it. This is going to be like freaking StarCraft. But, yeah, when I manage my Codex, my Claude Code, Grok, my custom agents, other agents, all orchestratable from one interface, and then add the social layer where we're able to have our agents interact with your agents in Nostr chat channels and stuff. Got a lot of this stuff built, and I'm excited to roll it out in this context of some sort of crowdsourced forensic-analysis fun. Let's

**[19:11]** see what spinning up sub agents Omega wants to use Codex's computer use. I have never tried that before. Let's see if that actually works. And then I definitely want to give good UI for seeing what the subagents are doing. Okay.

**[20:07]** Yeah, so the first, like, summary, you could note that it was, like, super quick. That was Luna. OpenAI's cheap model. And then this is, like, Sol, extremely high-powered reasoner. So this will be good to use as the base for what we do next. Usually I like to use Sol extra high for the planning and then flip.

**[20:33]** switch it down to medium for the implementation. Okay, I read all 23 Markdown files in `docs/loupe` and `docs/coldcard`, 8,000 lines. The corpus tells one connected story: a catastrophic Coldcard entropy failure, an initially mistaken theory about Loupe's limitation, an experiment that corrected it, and the resulting implementation of a much stricter forensic-evidence workbench. The implementation guide says our issues are implemented and tested. The system now includes strict forensic contracts and canonical evidence identities,

**[21:31]** a Loupe-style discovery adapter with separate findings and hypotheses, an independent verifier. Yeah, so a lot of the stuff we just built and it just we haven't connected it into the UI or like it made a first pass at connecting it to the old UI but it looked like shit. So I'm just going to like pull some of it into this current UI and like make sure that we... It all is all logical. Okay, so first of all before I really read this I need to ensure that it read The other So this was from the previous thing What did this do?

**[22:22]** Okay. Confirm if you read this document that it's talking about. There was another thread of Omega that we did some analysis of. I want to see if that artifact is available in the remote main branch or any other local area and make sure that you can read it. If not, go read this thread and ensure that you are familiar with everything that was said there and the images that are there and then summarize that here also.

**[23:29]** What is this? Ooh, the plan! I've never seen this component before. I got a little bit of UI to improve, but this is a combination of like Zed as a base, our own theme, plus this other GUI chat product called Comet, and we're just like folding it all into this super IDE. Okay, the artifact is available.

**[24:17]** Yeah, I just like this UI way better than the Codex desktop app. It just feels calmer. Even though it's earlier in its dev cycle, it still feels less laggy and janky than the Codex app does. This uses the GPUI library from Zed that builds components in Rust immediate mode.

**[24:52]** So everything that you see is rendered by rust in real time, like writing to the screen pixels directly something like that. But it feels snappy and smooth for sure. I've read the entire 345 line artifact.

**[25:58]** Could be cool to add some interesting 2D or eventually 3D visualization for the different sub-agents interacting, like some kind of little factorio style units maybe. I'm getting some beach balls now I appreciate that good analysis,

**[27:12]** But I do want you to go find the images from the thread because you may come to different conclusions than that agent did. So, yes, go and find that and summarize here. Yeah, so, like, five seconds after I say how smooth GPUI is, I'm getting beach balls. But that's only because I've put in some horrible rendering-loop stuff. Stuff's re-rendering when it doesn't need to.

**[27:43]** Uh skill issue, but that's okay. Um so let's take a look through this a little bit. So this is the analysis that the first chat created. And what happens if I click on this? I do need to add like a preview mode, so instead of just seeing the raw markdown, You see the pretty version.

**[28:19]** Okay, the document is an analysis and planning proposal for adding a model panel and publication gauge to the Omega forensic workbench. It does not claim that the panel exists, authorizes scan, reproduce, record, or authorize publication. Uh, its central rule is a model panel expands coverage and exposes disagreement. It is not a truth oracle. Yeah, i i we can't be saying like, oh, Kimi said this, or Saul said this. It's like, no, give me the full run. of what was said what happened that one thread the guy was like oh i'm not going to share my system prompts and stuff because you'll just see what i'm telling the agent to just keep going like no we need to see all of that we need to have more eyeballs on what everyone is telling their agents um because it's possible to lead these agents down rabbit holes or like

**[29:10]** Give them leading questions that skews the analysis. So we gotta correct for a bunch of that. Form items that remain visible. Roles must remain separate. Yes.

**[29:49]** Lives inside the local omega thread UI. The thread info is on the fucking computer. You shouldn't need that. Look where threads are stored. Does our queuing work? Now it's opening up the wrong fucking UI. It's my queuing not working.

**[30:44]** Alright, at least it opened the right thing. So bottom left here, pretty soon this will be like your Nostr identity. And I like that Buzz figured out how to basically have a bunch of application logic use a ton of custom NIPs so that you can have everything run through a Nostr relay.

**[31:16]** So we're going to do that same thing. You're right, I overcomplicated it. So part of why we're pulling Codex into our own harness is to prevent it from overcomplicating things through customizing the delegation instructions and like more limited tool sets given to Codex when it's tasked to do certain things.

**[31:58]** Because sometimes it can, yeah, like pulling computer use when... a much easier programmatic solution available. Planning Thread Blob D-Com.

**[32:49]** I'm guessing the images are stored as blobs or some sort of interesting format. And then here's where I'd go check X. Let's see what X is saying while I'm waiting for this. Oh, I have notifications. Oh yeah, I just added this. On the left it'll show you the icon if it's a Codex chat or a...

**[33:35]** omega chat or Claude or whatever. I found the local stores. The likeliest thread is this. I'm extracting its compressed transcripts. I'm doing all this because I want it to be, I want this thread with the smartest Sol model to be um Just up to speed with everything we've talked about so far, including the raw images that I pasted.

**[34:08]** Appreciate your patience. And you can always skip ahead. So, I can't skip ahead. I'm also able to build in UI features that codex doesn't have that always piss me off.

**[35:12]** Like, with codex you can't see historically what the reasoning is, cuz it only like writes over the one reasoning area. So you can't like go back and see what the thought was as it's going. Like I like being able to see that all in line. Like don't get too cute with hiding stuff from me. And Claude Code is the worst at that, they hide all sorts of crap. like let me see what it's doing Claude Code will be like, oh I've just run 45 shell commands.

**[35:40]** What? What the hell 45 shell commands did you just run? And then you can't click on it because it's not click, it's a TUI. You gotta like figure out what the hotkey combination is that gets you there and it's still truncated. Ugh. Definitely liking the desktop app approach here. View image. Hey, we got an image. Can I look at the image? Can I look at the image?

**[36:08]** Binary files are not supported. Okay, they will be at some point. I'm going to screenshot and add that later. What about opening default app? Oh, yeah. That. an older image. Let's see if it actually has the right thread.

**[37:07]** Yeah, it was looking at Omega RC, which I think was the old build, and Omega Dev is the newer one. I see that you're looking through the Codex threads, but the thread I'm talking about was an omega thread,

**[38:00]** and I don't think it's persisted along with the Codex threads. It was just using the OpenAI API directly. So if you're only looking in Codex, you'll need to look instead where omega threads are stored. It was a chat that I had directly before our current one with Luna, and then the queuing seems to not be working, but if I just stop and send now it'll go.

**[39:04]** Context compacting. Let's scroll X together everybody. Coinkite said they don't keep user information, but they just sent an email advisory to me about the security notes, to an email that hasn't interacted with them in years.

**[40:03]** This is a great clip, by the way. Right now my feed is like 80% Bitcoin, 20% Anthropic hate.

**[40:55]** It's a pretty good reflection of my focus. Well, here's where I would want to be able to just right click on a thread,

**[42:30]** copy the session ID, and then drop it in and have it use a `read_thread` tool, like Grok has a `read_thread` tool. So I'll add that. Yeah, when it runs a command where it does a broad search across your computer, Mac will give you the alert, like, “Omega wants to access your Apple Music.” No, we do not want your Apple Music. But, found it. The screenshot-era Omega process was launched with custom user data,

**[43:49]** blah blah blah, so the relevant thread was never in the standard Omega dev. I'm reading that profile now. This directly matches the app instance and timestamp in your screenshot. Great. Yeah, I'm launching comment via... I'm launching, Omega VL interesting shell script with a separate user directory yeah There we go yeah

**[44:41]** Found them. Found them. It's a little rough, guys, but you can see this is going to be one powerful freaking desktop app. Okay, planning visual analysis and transcription, planning OCR and digital and detailed table parsing. Clarifying blah blah blah. Okay. I have all four original PNGs now and have read them at full resolution.

**[45:07]** I may have skipped the video ahead to this. It just took multiple minutes to find these images. Okay. One material correction is already clear. The first Omega summary missed that the table marks both Coldcard MK4 and Q in red for different reasons. Okay, so this will give us a nice, solid briefing to start from.

**[46:49]** Okay, I found and read the exact Omega thread. What happened in the thread? You asked Luna to read the Loupe and Coldcard documents and summarize issue 9300. Luna described 9300 as the roadmap for a governed forensic workbench, blah, blah, blah. You asked for a fifth-grade explanation. You supplied two screenshots containing a wall of security comparisons and company X posts. You supplied two more screenshots showing criticism from OpenAgents and Zach Herbert. You asked how the workbench would address those concerns. Luna proposed multiple-model failover, blah, blah, blah. You instructed Luna to delegate an updated Coldcard audit to Codex. Codex created this.

**[47:20]** Luna began reading that audit back. Its next hosted completion failed with a 400 error. Blah, blah. Okay. Good, this is what I wanted. Conclusions I would change or sharpen: model agreement is neither necessary nor sufficient. The delegated Codex session did not receive the four images. Well, that's not good.

**[47:58]** Note that for later. Okay, before we read this, let's do this. Please update all relevant docs based on your now combined understanding, put audits up into the GitHub repo on main branch, thoroughly update any relevant documentation and suggestion roadmap items, including what we would want to have added.

**[48:34]** added to our simplified Comet-based UI in the short term, like the very first things we want to see in our forensics bench moving in this direction. Factoring in also all the work that we did with issue 9300, we implemented a lot of that, and we want to start seeing some of it in clear, usable UI.

**[49:02]** Update everything accordingly and push to main. You hit your usage limit. Okay, now I did build an API key fallback that I haven't set up the API key for.

**[49:31]** In this version of it. But turn your eyes away real quick. I'm gonna try dropping my key in here. Okay, API key configured. Now, let's see if this works.

**[50:03]** Try again. Please work, please work, please work. Okay, I'm gonna switch accounts. Quit and come again.

**[52:35]** I see it's not loading the Codex chats at all. All right, I had to switch Codex accounts and hit a bug where we couldn't open a Codex chat.

**[54:05]** So, switching over to Codex, sorry. Whatever. So now we are turning this whole summary into updated roadmaps and docs. And while that cranks, I'll see if I can have Omega fix itself. Omega threads are not loading. I can't load Codex threads.

**[54:33]** Clicking on the thing does nothing. I can load Omega threads fine, but not Codex threads. Delegate this to Codex. Work in a worktree, then merge to main and rebuild the release fast. I suppose I should change this to: clicking on an OpenAI Codex thread in Sessions does not open it. It doesn't do anything.

**[55:38]** Only Omega threads load. Fix that. Delegate it to Codex. Solve it in a worktree, then merge to main. Merge to main. Please work. Let's pause here and reboot.
