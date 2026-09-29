# Jev CEO: I made ChatGPT, now I'm building what's next

Transcript of Diogo Almeida's AI Engineer talk on why RLHF-trained models are built for assistance rather than automation, and what TypeSafe is optimizing for instead.

| Field | Value |
| --- | --- |
| Source | [https://www.youtube.com/watch?v=cJ0EOzey--o](https://www.youtube.com/watch?v=cJ0EOzey--o) |
| Channel | AI Engineer |
| Published | 2026-07-31 |
| Duration | 18:04 |
| Transcribed | 2026-09-29, locally with whisper.cpp `large-v3` (beam size 5), then proofread for names and repeated segments |
| Speakers | Diogo Almeida (TypeSafe AI CEO, formerly OpenAI) |

Timestamps are `[mm:ss]` from the start of the video. Chapter headings follow the video's own chapter markers. Speakers are not labeled. This transcript is kept for research on TypeSafe's approach to putting typed, probabilistic judgment inside software; see the `typesafe-ai` skill and https://typesafe.ai/ for the product.

## Video description (from YouTube)

> Diogo Almeida, a GPT-4 co author formerly at OpenAI and now CEO of TypeSafeAI, creators of Jev, argues that RLHF made models that are extraordinary at pleasing the human in the loop, and that is exactly the problem. Optimizing for human preference optimizes for engagement and for overpromising, the same pressure that makes a model confidently agree that a fart audio file is a symphony. That produces two camps: one where models act as assistants with a human catching mistakes, where RLHF shines, and one where they operate autonomously with real stakes, where the same instinct to please quietly becomes a liability.
>
> So what comes next is not the Claude Code era but a shift in what you optimize. Almeida frames it through Sutton's bitter lesson: the task matters more than the data, and reinforcement learning with verifiable rewards points the model at real automation instead of human approval. He is careful that pre trained models are already incredibly capable and that the trap is bolting preference optimization on top, which teaches confidence and drops modes. The through line is that assistance and automation pull in different directions in optimization space, and the field is only starting to say plainly which one it is building.

## Chapters

- [00:00] Not the Claude Code era
- [01:40] The state of the field
- [03:14] Two camps: assistance and autonomy
- [04:31] Why models please the human in the loop
- [06:37] How RLHF actually works
- [07:31] Preference versus what's true
- [08:10] When the consequences get real
- [08:47] So what's next
- [09:35] Assistance is not automation
- [14:31] Is pre-training the problem?
- [15:43] RLVR and Sutton's bitter lesson

## Transcript

### [00:00] Not the Claude Code era

[00:12] Excellent. I will say that I might speed run through this.
[00:17] Feel free, if you don't agree with something, to yell out.
[00:21] It's way more fun for me if things get interactive.
[00:25] Otherwise, I will go through this.
[00:27] First, can I have like a vague show of hands
[00:29] of who knows what RLHF is?
[00:32] Oh, excellent.
[00:33] I might be able to skip through that part quickly
[00:35] and get into the interactive stuff.
[00:37] So, my name's Diogo Almeida.
[00:39] I'm talking about what's next after RLHF.
[00:41] More accurately, I think this should be called
[00:44] what's next after the ChatGPT era
[00:46] that I think we're all in.
[00:48] And my hint for you guys is it is not the Claude Code era.
[00:52] I will justify this later on,
[00:54] but I actually believe them to be part of the same era.
[00:57] Why should you listen to me?
[00:58] I was co-author to what was basically
[01:01] OpenAI's greatest hits, at least published hits.
[01:04] Co-author to GPT-4, ChatGPT, RLHF/InstructGPT.
[01:10] The team I was part of basically
[01:11] invented post-training as a concept.
[01:13] So very qualified on a lot of this stuff.
[01:17] But what makes me somewhat unique here
[01:20] is that I'm one of the few people at OpenAI
[01:23] who actually hates on ChatGPT.
[01:26] Thank you.
[01:27] I don't hate ChatGPT as a product, to be clear.
[01:30] I think ChatGPT is a world-changing product
[01:32] that will probably stay with us for the rest of time
[01:35] unless something better comes up.
[01:36] But I also acknowledge its limitations,
[01:38] and I think a lot of what's happened

### [01:40] The state of the field

[01:41] in this state of the field can be traced back
[01:43] to minor decisions we made
[01:45] in making the algorithms behind ChatGPT.
[01:50] I feel like the question that's relevant
[01:51] to everyone in AI right now is what's actually going on?
[01:56] There's a lot of like differing opinions,
[01:59] and I think it's really useful to like map out the spectrum
[02:02] and figure out how can smart people
[02:04] have like such different opinions.
[02:06] There's cult one, AI is not just going well,
[02:10] it's going insanely well.
[02:12] Every single benchmark, we surpass human level,
[02:15] and as far as we can measure,
[02:16] we are continuously surpassing you in performance.
[02:20] You know, like basically every new benchmark,
[02:22] and it's only getting faster and accelerating.
[02:25] You have every-- can I see my mouse?
[02:28] Excellent.
[02:28] Basically, every NLP benchmark is getting crushed.
[02:31] And not only that, allegedly, the time
[02:34] that LLMs can operate autonomously
[02:37] is growing exponentially.
[02:39] On the other hand, you have AI is not just going poorly.
[02:43] It's going insanely poorly.
[02:45] AI is a bubble.
[02:47] It's basically generating no value.
[02:48] It's just circular financing deals, et cetera, et cetera.
[02:52] And if AI is so great, why is everything
[02:55] just like a chat app right now, or like a Claude Code thing?
[02:59] And a lot of the people have actually
[03:01] kind of given up on what was the old guard's terminology
[03:04] of a transformative AI revolution.
[03:06] People aren't really talking about that anymore.
[03:09] They're talking about it being massively valuable,
[03:11] like B2B SaaS.
[03:12] So the only thing that everyone agrees on

### [03:14] Two camps: assistance and autonomy

[03:14] is there's just these extreme points of view and nothing
[03:17] in between.
[03:18] And everyone basically thinks AI is insane.
[03:21] but like for different reasons.
[03:22] And what I would want to talk about is,
[03:25] what is the sane view of AI?
[03:26] Let's take all the evidence of like cult one,
[03:29] it's going super well.
[03:30] Take all the evidence of cult two, it's going super poorly.
[03:33] Like, you know, map them out
[03:35] and try to explain what explains that divide.
[03:39] Like what is the simplest possible explanation
[03:42] of why some things are too good to be true
[03:44] and some things are not just bad,
[03:45] they are so bad that we would still employ human workers
[03:48] to do dumb tasks.
[03:52] No offense to any of them, a lot of these tasks on the right
[03:56] seem way, way, way easier than the stuff on the left.
[03:59] How can we be solving unsolved math problems,
[04:02] but still customer service requires humans in the loop
[04:06] in order to actually make decisions?
[04:08] This, I think, is a wild state of affairs.
[04:11] And in my opinion, anyone who works adjacent to AI
[04:15] should have an answer to this, because this
[04:17] like the evidence in the field right now. I would normally pause and ask people if they
[04:22] want to yell out their thoughts on this, but I don't think we have time for that, and I've been
[04:27] told to not take Q&A until after. But I'll just give you my answer to this, which is, in my

### [04:31] Why models please the human in the loop

[04:32] opinion, the simplest explanation. All the stuff in the left is not just a task that happens to
[04:39] have a human in the loop. In the left, the task, the goal of it is to please the human in the loop.
[04:44] These tasks are intrinsically human-in-the-loop tasks.
[04:48] Like, Claude Code's job is not to just make code work.
[04:51] The way it converses would be totally different.
[04:54] The goal is to please the human in it.
[04:57] And on the other side, all of these tasks
[04:59] that seem way more basic, the goal
[05:01] is to not have removed the human loop.
[05:03] Ideally, it would be running in the background in a server
[05:06] that you never even look at.
[05:07] And ideally, it eventually becomes like legacy software
[05:10] that you don't really worry about.
[05:12] And this is the divide between assistance and automation.
[05:17] Lesson one for my talk is that today's AI, everything
[05:20] inherited from RLHF, is incredible at the human
[05:23] in the loop stuff, but not for automation tasks.
[05:26] This is a longer aside, but the lesson basically
[05:29] every business has learned is do not
[05:31] use AI for decisions with stakes to your business.
[05:36] A common pattern is make sure that all of the costs
[05:39] are to the user and not to your business.
[05:41] So it's totally OK to throw the user at infinite docs
[05:45] and customer service.
[05:46] But it is not OK to make it make expensive decisions.
[05:49] Horrible pattern, but that is the state of AI right now.
[05:53] I can blitz through the what is RLHF part,
[05:55] because you all seem to know what it is.
[05:58] It's the algorithm behind not just ChatGPT,
[06:00] but basically every LLM today.
[06:02] As far as I can tell by usage, 100% roughly of LLMs
[06:07] are trained with RLHF.
[06:09] And we have this, we as in we the OpenAI team,
[06:12] had this great blog post on how it worked.
[06:15] I will not get into that because you all know it
[06:17] and this is super boring.
[06:20] The summary of this is it is just collect human preferences,
[06:23] optimize for human preferences.
[06:26] And if you want to see like an annotated version of this,
[06:28] you can see which parts are collecting human preferences,
[06:30] which ones are optimizing for them.
[06:32] And this, I think, provides a really clear answer

### [06:37] How RLHF actually works

[06:36] to everyone in the field asking,
[06:38] Why do all LLMs require a human in the loop?
[06:42] And the simple answer is we literally put them in the loop.
[06:45] The goal of the loop is to optimize for human preference.
[06:48] It is not to run software autonomously.
[06:50] It's kind of super obvious.
[06:53] Thank you, my man at the back.
[06:56] Yeah, I love that you're laughing at this.
[07:00] And because of that, overpromising is a feature.
[07:05] This is by design.
[07:06] This is an old meta study, and the numbers probably
[07:10] have changed, but by construction,
[07:14] every RLHF model will always have
[07:16] a big difference between human preference and results,
[07:19] even if the results are good, because the main objective
[07:22] you're optimizing for is for human preference.
[07:24] This is just natural to how LLMs work.
[07:28] I love this tweet of sending ChatGPT an audio file of fart

### [07:31] Preference versus what's true

[07:33] sound effects and asking what you think of the music I made.
[07:38] Here's a straight, honest reaction.
[07:40] It's a very eerie vibe atmosphere piece.
[07:44] And this is just how RLHF works.
[07:45] If it doesn't know, it will err on the side
[07:48] of doing what it thinks is best for human preference.
[07:51] And this makes total sense if you are a user in the loop,
[07:54] because the end game for all RLHF models
[07:57] is optimizing for engagement.
[07:58] But what you really want if you want automation
[08:01] is for it to just like not give a shit about the humans
[08:04] and just do the task correctly in a calibrated way.

### [08:10] When the consequences get real

[08:11] Lesson number two is that today's AI
[08:14] was designed for assistance
[08:15] through optimizing for human preference.
[08:17] This is like, it's like in the name.
[08:19] This is not like a controversial take
[08:22] and the consequences are maybe more controversial,
[08:25] but it's like very obvious if you think about
[08:27] what we really are optimizing for,
[08:29] which is no matter how wrong the models are,
[08:32] they will look right because of the asymmetry
[08:34] within the reward model in RLHF.
[08:37] And this is where a lot of the dilemma in the field
[08:41] stems from because people really want automation to happen.

### [08:47] So what's next

[08:47] Cool, so back to the original question.
[08:49] I am over halfway done with the talk
[08:51] and I haven't even answered it.
[08:52] I was just talking about what's RLHF.
[08:54] But this was a framing to talk about
[08:58] what RLHF is to talk about what's next.
[09:01] And I would say, the real question is,
[09:03] what's next after AI's assistance era,
[09:06] which I think that we are very firmly in right now.
[09:09] And back to the original clue of why it's not Claude Code,
[09:12] it's actually a super fun, nuanced discussion,
[09:15] but it's not Claude Code,
[09:16] because Claude Code is still part of that assistance era.
[09:18] Claude Code is still RLHF'd,
[09:20] and it would look very, very different
[09:22] if it was purely, this is a little advanced,
[09:24] but if it was purely RLVR'd,
[09:25] it would look very, very different.
[09:27] And this is why you get this dilemma with models,
[09:30] where sometimes it gets really good at agentic stuff,
[09:32] but it stops following what you actually want.

### [09:35] Assistance is not automation

[09:34] This is like the trade-off in optimization space
[09:36] that keeps dancing.
[09:38] But both of these trade-offs in optimization space
[09:40] do not add to the automation component.
[09:43] And that leads to what I think the logical answer of what's
[09:47] next after assistance is real automation.
[09:51] To talk a little bit about automation
[09:54] and how that would work, I want to talk about software.
[09:57] Maybe this is a little bit philosophical for you guys,
[10:00] but I think when it clicks, and hopefully it clicks
[10:03] if I do a good job, hopefully it'll be really clear,
[10:06] which is I'm a lover of software.
[10:08] I assume everyone here loves software.
[10:11] Software is like super valuable.
[10:13] See all the SaaS.
[10:14] And kind of like the craziest part of software,
[10:17] in my opinion, is that all of the SaaS
[10:20] basically has not changed since 2019.
[10:23] Like SaaS has not really changed in the LLM era,
[10:26] except sometimes a chatbot is like latched on,
[10:29] which is like kind of insane if you think about
[10:32] like the progress made in AI,
[10:33] but is actually very predictable
[10:36] when you think that AI is assistance native, right?
[10:39] Like AI is made for assistance.
[10:40] What can you do in SaaS?
[10:42] Just provide an assistant on the side.
[10:44] And this is not what early AI pioneers
[10:47] used to think would happen.
[10:49] Like when you see like the early wording
[10:51] in OpenAI's charter,
[10:53] It's about like doing like tons of work,
[10:56] not about like making profit or anything like that.
[10:58] And we used to think that software would get a lot smarter,
[11:01] not just cheaper to write,
[11:03] which is kind of the direction we're going down right now.
[11:06] And I actually really like this phrasing from Garry Tan.
[11:10] I think he means this as a compliment
[11:13] to what's going on right now.
[11:14] We're entering the golden age of just-in-time software,
[11:17] but I actually think that this is like a double-edged sword.
[11:21] Like, I don't just want just-in-time software,
[11:24] which is cool, I love Claude Code, to be clear,
[11:26] just like I love ChatGPT, I would keep using it.
[11:29] But like, what I want is smarter software.
[11:31] Why can't like, B2B, like why can't software
[11:34] just be more expressive?
[11:36] Like, why are the building blocks of software
[11:39] actually still the same?
[11:41] And I think this is a question that the whole AI industry
[11:45] should ask itself, and basically every time
[11:47] you're thinking about we want to do automation,
[11:50] It is not about an amalgamation of automating a person's work.
[11:54] It's about, hey, there's this extremely rote work.
[11:57] It's so simple that we can communicate to someone else
[12:00] that this thing should be done.
[12:02] And ideally, it's so basic that it could be done repeatedly
[12:05] for basically free, or it could be done by computers.
[12:08] And that's really not happening right now.
[12:10] What we're doing is we're just automating
[12:11] the writing of the software, but then its expressibility
[12:14] is the same.
[12:15] And that, to me, is tragic in the state of the world.
[12:20] Oh, lesson three. This is something that I believe strongly in. I believe that eventually
[12:31] the field will right -- I wouldn't say RLHF is wrong, but it was a weird detour and one
[12:37] that we didn't expect. Tomorrow's AI, I believe, will be for automation, and we will eventually
[12:43] have a world with smarter software. There will start to be actual work that is automated,
[12:48] which right now it's a rounding error,
[12:50] despite LLM's intelligence.
[12:52] And that is what we are working on at TypeSafe.
[12:57] We are still kind of stealthy,
[13:00] like I'm willing to give these talks,
[13:02] but these are like some of the early ones.
[13:04] Our core question is what if the AI stack
[13:08] was redesigned for reliability and automation?
[13:11] Like how would that all change?
[13:14] What would you do?
[13:15] And actually there's a lot,
[13:16] It's a very interesting fork in the road
[13:19] for what's, you know, like from basically
[13:22] every LLM that's built today.
[13:23] And I think it's one of the most satisfying things
[13:25] I've worked on and I've worked on some pretty cool stuff.
[13:28] We are releasing soon, so if you want to work with us
[13:31] or you want to like, you know, be the first,
[13:33] one of the first to build smart software,
[13:35] please sign up on either our mailing list or careers page.
[13:39] And I am trying to start a Twitter,
[13:41] so follow me and I will post really spicy things.
[13:44] I actually will post something later today
[13:46] that I guarantee will be very spicy.
[13:48] The hint is that the original scaling laws were incorrect.
[13:53] Cool, that's it for my prepared stuff.
[13:57] I would love, do I have time
[13:58] for people yelling out questions?
[14:00] I would love questions, feedback, disagreements,
[14:03] strong stuff, I can repeat the question,
[14:04] you don't have to worry about the mic.
[14:06] Hell yeah.
[14:08] Cool, the question was roughly,
[14:10] what if you trained a classifier head
[14:12] with pre-training as well, roughly,
[14:15] like Yoshua Bengio is suggesting.
[14:19] I will say that that's complicated,
[14:22] and I actually think I don't have the time
[14:24] to answer that particular question.
[14:26] I will give my simplified view on this,
[14:28] and the answer is I actually don't think

### [14:31] Is pre-training the problem?

[14:31] that pre-training is the problem.
[14:33] I think pre-training is fucking phenomenal.
[14:37] The fact that we compress the knowledge of the internet
[14:39] into this core of intelligence
[14:41] that then can be utilized is incredible,
[14:43] And the pre-trained models are incredibly intelligent.
[14:47] And I believe that the problem is how we unearth it.
[14:50] And hallucination, to me, is intrinsic to optimizing
[14:55] for human preference.
[14:57] There's an asymmetry in the reward model,
[14:59] kind of like GANs have.
[15:00] Oh, I really should not get-- this is a very advanced topic.
[15:02] But there's an asymmetry in the reward model,
[15:04] like what GANs have, that encourage the models to drop
[15:12] modes and be confident because it's very easy to see when the model is not confident and
[15:16] to punish that from a reward model perspective. It's very complicated but I'm happy to chat
[15:21] afterwards if you want to jam. Cool. Oops. I have other slides from other talks as well
[15:32] that I could go into more about that.
[15:34] I have a minute left, hell yeah.
[15:36] - Are you doing RLVR?
[15:37] - Say it again.
[15:38] - Is that a third thing?
[15:39] Are there just RLVR and new things or RLVR?

### [15:43] RLVR and Sutton's bitter lesson

[15:43] - It is definitely not RLVR.
[15:44] So it is a new thing.
[15:46] Every single optimization stack,
[15:48] I will actually go into an old presentation that I have
[15:51] because I think this is super important.
[15:54] In terms of like, to me, Sutton's bitter lesson
[15:58] is that algorithms matter more than compute.
[16:00] This is true in games,
[16:02] but not true in reality.
[16:03] I actually think that the full stack
[16:05] is that data matters more than compute,
[16:07] and doing the right task matters way more than data.
[16:11] And basically, every single branch of LLM post-training,
[16:16] if you want to call it, has its own north star
[16:18] of what it's optimizing for.
[16:19] So RLHF is optimizing for human preference.
[16:22] RLVR is optimizing for log error rates of pure correctness.
[16:27] But we are doing a third thing that is optimized
[16:30] for calibrated decision-making, and basically mainlining
[16:34] the intelligence of pre-trained models
[16:36] into being actually useful for software, which I think
[16:38] is quite different.
[16:41] Could you say that again?
[16:53] They're asking if the reward is injected
[16:55] through the whole process.
[16:56] I will actually say that even the shape of the API
[17:00] is different because the shape of the API of RLHF
[17:02] is different from RLVR,
[17:03] which is different from what we are doing.
[17:05] So we are like thinking about it from scratch,
[17:08] just like no one thought about instruction following
[17:10] before we made instruction following happen.
[17:12] Usually when there's a big branch in new ways to post-train,
[17:15] like it just looks like totally alien
[17:17] and then in hindsight becomes super obvious.
[17:22] Cool.
[17:23] I believe I'm over time 'cause this red thing is beeping,
[17:25] but please find me afterwards.
[17:28] I love questions, I love the interactivity.
[17:31] And follow me on Twitter for spicy stuff.
[17:35] Heck yeah.
[17:37] Oh yeah, it's over here.
[17:39] Complete skeptic.
[17:41] It's on brand for me.
[17:44] Cool, heck yeah, thank you.
