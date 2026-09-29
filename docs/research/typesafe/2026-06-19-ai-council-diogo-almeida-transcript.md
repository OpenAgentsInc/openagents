# Jev: ChatGPT Co-Creator’s Answer to RLHF

Transcript of Diogo Almeida's AI Council 2026 talk, given four months before Jev launched, on why RLHF-trained models excel at assistance and fail at automation, and what TypeSafe set out to build instead.

| Field | Value |
| --- | --- |
| Source | [https://www.youtube.com/watch?v=o-y1HJ6buGQ](https://www.youtube.com/watch?v=o-y1HJ6buGQ) |
| Channel | AI Council |
| Published | 2026-06-19 (recorded 2026-05-12 to 14 at AI Council 2026, San Francisco) |
| Duration | 36:38 |
| Transcribed | 2026-09-29, locally with whisper.cpp `large-v3` (beam size 5), then proofread for names and repeated segments |
| Speakers | Diogo Almeida (TypeSafe AI co-founder and CEO, InstructGPT and GPT-4 co-author) |

Timestamps are `[mm:ss]` from the start of the video. Chapter headings follow the video's own chapter markers. Speakers are not labeled. This transcript is kept for research on TypeSafe's approach to putting typed, probabilistic judgment inside software; see the `typesafe-ai` skill and https://typesafe.ai/ for the product.

## Video description (from YouTube)

> 4 months before TypeSafe AI launched Jev, founder Diogo Almeida stood up at AI Council and laid out the problem Jev was built to solve.
>
> He never names the product in this talk. He says only that TypeSafe is going all in on automation and getting ready to ship in a couple of months, then spends 36 minutes on why he thinks the rest of the field is pointed the wrong way. TypeSafe came out of stealth on September 15, 2026 with $40M led by DCVC and launched Jev, a model that outputs typed decisions with probabilities for software to act on.
>
> Almeida spent four and a half years at OpenAI, where he co-authored the InstructGPT paper and the GPT-4 Technical Report and worked on the RLHF research behind ChatGPT. His argument here starts from that work. His read is that roughly all production LLMs are trained with RLHF or some variation of it, and RLHF optimizes for human preference. Human preference is a different target from doing the task correctly. That gap is why AI looks superhuman on assistance work and falls apart on automation work that looks easier.
>
> He walks through mode collapse, Yann LeCun's doomed-LLM argument, why a 1.3B-parameter InstructGPT model was preferred to 175B GPT-3 at following instructions, and where he thinks Sutton's bitter lesson is incomplete. He closes on the question that started TypeSafe, which is what it would take to treat language models as a primitive that software calls, the way it calls a database or an API.
>
> Recorded live at AI Council 2026 in San Francisco, May 12-14.

## Chapters

- [00:00] AI is too good to be true and too bad to be useful
- [00:51] Four and a half years at OpenAI
- [02:11] Where is the economic revolution?
- [05:23] Assistance work versus automation work
- [06:09] But aren't coding agents automation?
- [09:43] When is a result too good to be true?
- [11:18] Sutton's bitter lesson, and the bitterest lesson
- [13:49] What RLHF is and what it costs you
- [14:44] Yann LeCun's doomed-LLM argument
- [15:49] Mode collapse, RLHF's deal with the devil
- [18:18] Is software engineering easier than drive-thrus?
- [19:49] Why a model can't be trusted with stakes
- [20:42] Is ChatGPT cooked?
- [25:33] Type-safe language models, going beyond strings
- [27:13] The FLAN lesson, when the whole field is wrong
- [29:40] Can one model do assistance and automation?
- [31:08] What TypeSafe is building
- [33:42] Summary
- [35:00] Was RLHF AGI, and what was missing

## Transcript

### [00:00] AI is too good to be true and too bad to be useful

[00:00] - Technically, I am CEO of a company called TypeSafe.
[00:04] Vast majority of this talk will be as technology lover Diogo,
[00:08] not CEO Diogo.
[00:10] The title of this is AI too good to be true,
[00:13] too bad to be useful.
[00:14] Another topic that I,
[00:16] pretending title that I was considering was
[00:19] what's the deal with AI over-promise, under-deliver?
[00:22] This is a, we'll actually come back to this.
[00:25] This is a surprisingly deep metaphor for optimization,
[00:28] but I believe that we have a major problem in AI right now,
[00:32] which is the massive divide between LLM over-promise
[00:36] and LLM under-deliver.
[00:37] Very happy to debate that.
[00:39] Please save it for questions later.
[00:41] This is my favorite topic ever to talk about.
[00:43] And if it doesn't fit, I will be in that room
[00:45] and debate people until the cows come home or yeah, yeah.

### [00:51] Four and a half years at OpenAI

[00:51] Who am I?
[00:52] You guys are here, so you probably know that already.
[00:55] I was at OpenAI for about four and a half years.
[00:58] I was co-author to all of its greatest hits.
[01:02] This is, well, this is my first talk since, you know,
[01:04] the Anthropic takeover.
[01:05] So you could say I was there for its golden era
[01:07] 'cause it's kind of like silver right now,
[01:09] but co-author of GPT-4, InstructGPT, ChatGPT,
[01:14] and most relevantly of all, RLHF.
[01:17] I'm not gonna get into that right now.
[01:19] This will be a very technical talk.
[01:21] I'm sorry.
[01:23] And what's kind of unique about me,
[01:25] especially like from like an OpenAI perspective,
[01:28] very rare, it's very rare that people at OpenAI
[01:32] hate on ChatGPT.
[01:33] Culturally, it's actually just very weird.
[01:36] And I don't hate ChatGPT.
[01:37] I use it basically every day.
[01:39] And I just think that it has caused a lot of problems
[01:43] that I think it's just really worth talking about
[01:44] despite being a phenomenal product, excellent product,
[01:48] not on the path of, it is not our AI dream.
[01:51] And, you know, it's not leading
[01:53] to an AI-based economic revolution,
[01:55] which is I think what the early people really wanted.
[01:58] So I'm gonna talk, broken down sections.
[02:01] They're gonna be kind of random,
[02:02] but this to me is the most important part.
[02:04] Please stay for this section and try to learn from this one.
[02:08] This is the thing that I think everyone should know,

### [02:11] Where is the economic revolution?

[02:10] which is there are like such a wide divide in LLM land
[02:14] between like two narratives, you know,
[02:16] like what the hell is going on?
[02:17] Like, are we in like the path of the exponential machine god,
[02:21] like the EAs think, or are we like all hype,
[02:23] like the economists think, and like,
[02:25] there's like weird ass circular financing, whatever else.
[02:28] And I believe that everyone in AI
[02:30] should know an answer to this.
[02:32] And if you guys want,
[02:34] I can even give you times to try to answer things out loud.
[02:37] Let's improvise this talk.
[02:40] But my question has always been,
[02:41] where's the economic revolution?
[02:43] If you look at benchmarks,
[02:44] we've just been like saturating each benchmark nonstop.
[02:47] The models have actually been getting better
[02:50] and it kind of looks like we're on an exponential.
[02:52] This doesn't have the METR plots yet,
[02:53] but I like this one because it shows GPQA.
[02:58] Passing human level performance at GPQA,
[03:00] Google proof question answering,
[03:01] is what caused the OpenAI coup.
[03:03] People were like, holy shit,
[03:05] did we just like solve everything?
[03:07] This is too dangerous.
[03:08] Let's kick out Sam.
[03:09] And like, whether, you know,
[03:12] it doesn't matter like which side of that you want to,
[03:14] you want to stay in that argument on Sam,
[03:17] it's clear that that model was not dangerous.
[03:19] You know, that was worse than o1,
[03:21] which people today do not give a shit about.
[03:23] You know, people don't give a shit about
[03:24] way better models than that.
[03:26] Then if you look on the other flip side,
[03:27] it's people saying like, you know,
[03:27] I'm going to do this, I'm going to do that,
[03:28] like AI is a bubble.
[03:29] There's basically no economic impact.
[03:31] And like that is actually also technically true.
[03:34] So what is actually happening in the field of AI?
[03:39] Oh, oh yeah.
[03:39] And like one other thing is that the goalposts
[03:41] have really shifted as well.
[03:43] Like even the people who used to be like the true believers
[03:46] of like, this is a transformative technology
[03:48] are now like, eh, it's going to be a little different.
[03:50] It's going to be more renaissance than revolution.
[03:53] And they've moved the goalposts
[03:55] from automating most economically valuable work
[03:57] to either making or having a revenue
[04:00] or profit of a hundred billion dollars.
[04:01] And like, what the hell?
[04:03] That like AI is fucking awesome.
[04:05] And actually believing the mainstream narrative
[04:08] of it's only so good at so many things
[04:10] is actually extremely underwhelming
[04:12] to the potential of AI.
[04:13] I will convince you of this later.
[04:15] So, but the question we should ask,
[04:18] and this is a question
[04:19] that everyone should know an answer to,
[04:20] please think of your answer,
[04:22] is how can you have the two stories
[04:24] of it being too good to be true at,
[04:27] a whole bunch of stuff, everything in the news,
[04:29] but like too bad to be useful
[04:31] for like basically everything else.
[04:33] And these tasks, you know,
[04:34] it doesn't look like the stuff on the right
[04:37] is any harder than the stuff on the left.
[04:40] In fact, it looks like the opposite.
[04:42] And if you want to understand AI,
[04:44] you need at least an Occam's razor explanation
[04:46] of like, why is this the case?
[04:48] If you're wanting to like build a product,
[04:50] you should know where you fall.
[04:52] If you're investing in a company,
[04:53] nowhere to fall.
[04:54] If you want to work at a company,
[04:55] like, are you in like vaporware land?
[04:57] Or are you in like too good to be true?
[04:58] This is going to be incredible land.
[05:00] And, you know, like we should have
[05:02] like a simple explanation for all of this.
[05:04] I'm going to oversimplify
[05:06] and just jump straight to my answer.
[05:08] Actually, I'm just going to speed run this whole talk
[05:09] because I want to get to Q&A.
[05:10] It looked really fun last time.
[05:13] It looked super fun.
[05:14] So I'm just going to blast through a lot of the stuff
[05:16] I would go slower on.
[05:18] I believe that the answer to this
[05:20] is simply that all of the stuff in the left

### [05:23] Assistance work versus automation work

[05:23] were human in the loop assistance tasks.
[05:25] And the things on the right were like,
[05:26] machine in the loop automation tasks.
[05:29] And that is why a lot of them look simpler
[05:31] because like, if you want to automate stuff,
[05:32] you want to automate like simple little bits of work
[05:35] that just need to be done reliably.
[05:36] And you can see the critical difference
[05:38] between customer service without decisions
[05:41] and customer service with decisions.
[05:43] There's a lot of AI chatbots out there
[05:45] who make customer service agents.
[05:47] They do not allow you to take any actions.
[05:50] They like give you the kid gloves.
[05:51] I'll go get back to this later on.
[05:53] And they throw you into like the labyrinth of documentation.
[05:56] But they are not allowed to take actions for good reasons.
[05:59] The models are not reliable enough.
[06:02] But they are really, really good
[06:04] at the assistance side of things.
[06:06] Super common FAQ here is what about coding agents?

### [06:09] But aren't coding agents automation?

[06:10] Aren't coding agents kind of like automation?
[06:12] It speaks the language of computers.
[06:14] You know, it's code.
[06:15] What more is there to automation?
[06:17] And believe it or not, that is also assistance.
[06:21] They're like searching smells
[06:23] to when a task is optimized for assistance.
[06:25] Like, you can like kind of see this
[06:27] when you're like seeing outputs of the models.
[06:29] Like, especially when you get plausible,
[06:31] good-looking but incorrect answers.
[06:33] We'll talk about this later when we talk about optimization.
[06:36] But the simplest, you know,
[06:38] razor for when it's optimized
[06:40] for assistance versus automation is,
[06:41] does the workflow literally involve humans in the loop?
[06:44] And debatably, code is a language for humans,
[06:46] not for machines.
[06:49] And it's quite surprising to me
[06:52] that people are surprised by this.
[06:54] These were results from last year.
[06:55] This is on an older generation of models.
[06:58] So it may not apply,
[06:59] but this is basically the effect
[07:01] that we've kept observing in the economy right now.
[07:04] You see a lot of perceived value of AI speed up,
[07:08] but in terms of actual results,
[07:10] it is not been, it's not really been paying off.
[07:13] And this is exactly what you'd expect
[07:15] if you're optimizing for the human preference,
[07:17] that is the HF in RLHF,
[07:19] instead of actually the root of the thing you care about,
[07:22] which is logic, decision-making.
[07:24] Like just do the fucking task, right?
[07:26] Which LLMs are just not very good at,
[07:28] or at least they're not optimized for right now.
[07:30] And the result is a Stockholm syndrome of the field
[07:33] where people are like,
[07:34] LLMs are more like humans than software.
[07:36] They're unpredictable.
[07:38] As the need for precision goes up,
[07:40] the utility of AI goes down,
[07:42] which is completely correct for today's LLMs,
[07:45] but completely novel for machine learning.
[07:47] This is not how machine learning works.
[07:49] Machine learning is highly structured.
[07:51] It is meant to integrate with automation.
[07:54] And actually, as the precision goes up,
[07:56] the utility of ML super skyrockets.
[07:58] Would you trust a human being
[08:00] to like balance the recommender algorithm
[08:02] of like the Meta feed flow thing?
[08:05] That's insane, that's so fucking hard, right?
[08:07] But we, for some reason we think, no, no,
[08:10] the LLMs, they're like the opposite of ML,
[08:12] kid gloves on them.
[08:13] They are just good for like the perception of utility
[08:16] and not true utility.
[08:17] That is kind of nuts to me.
[08:20] Perfect, summary for this section.
[08:22] This is most important part
[08:23] of the talk, the rest of my talk is basically a rambling
[08:26] of less important details,
[08:28] but almost all of today's LLMs are optimized for assistance.
[08:32] This causes the massive divide
[08:34] between over-promise and under-deliver.
[08:36] I would actually argue there wasn't LLM history pre-RLHF.
[08:39] I wasn't part of it.
[08:40] So I'm like even more biased
[08:42] to think the RLHF part is better,
[08:44] but I actually think that there was not that divide then.
[08:46] Like GPT-3 did not have the massive divide
[08:49] that ChatGPT has,
[08:50] even though GPT-3 was a fucking awesome model.
[08:53] And if you want today's tasks,
[08:54] today's LLM tasks to work,
[08:56] make sure it's on assistance tasks.
[08:58] People have tried on really basic automation tasks.
[09:00] Like to my knowledge, I don't really go out much,
[09:03] but I don't believe drive-thrus are automated yet.
[09:05] Someone correct me if I'm wrong,
[09:06] but like there's lots of economic incentive to do so.
[09:09] And you know, like also we had during this section,
[09:12] we had some vague foreshadowing
[09:14] that we can do better as a field,
[09:15] which eventually we'll get into,
[09:17] but we need to like build up our technical fundamentals
[09:19] to get there.
[09:21] Perfect.
[09:22] Optimization.
[09:23] This is the thing I am nerdiest for.
[09:26] And I love talking about optimization.
[09:28] It's a short section that I think explains,
[09:33] you know, gives you like the technical backing
[09:34] of how it could be possible
[09:36] that what LLMs are optimized for causes all of these effects.
[09:40] So my favorite question optimization

### [09:43] When is a result too good to be true?

[09:43] is when is a result too good to be true?
[09:46] This is a super fake looking result.
[09:49] I love this result
[09:50] 'cause it's like the fakest result I've ever seen.
[09:52] You see like OpenAI,
[09:54] like flat lining in the bottom with number of parameters.
[09:56] This is not sandbag.
[09:58] This is actually on the efficient frontier.
[10:00] And you see like a tiny little us model
[10:03] completely dominating what was like
[10:05] the best model of the time.
[10:06] And this was the best model of the time to be clear.
[10:09] Then this is not TypeSafe.
[10:10] This is actually RLHF.
[10:13] That is the same plot.
[10:13] I just masked out some points so you can see the difference,
[10:17] but like there's something visceral about this plot
[10:19] that this plot doesn't quite show.
[10:21] And it comes down to like people's intuitions
[10:24] about optimization.
[10:25] This line at the, you can see my, yeah, you can see my mouse.
[10:29] This line at the bottom is actually the real scaling law.
[10:33] This is GPT-2.
[10:35] This is a small version in the GPT-3 family.
[10:37] And this is full on 175B GPT-3
[10:41] at the task of instruction following.
[10:43] And what you could see is even GPT-2 sized models,
[10:46] like completely dominated GPT-3, best model of its time.
[10:50] That warrants a very, you know,
[10:52] like that warrants an explanation
[10:54] and people's mental models of the field
[10:55] should be updated accordingly.
[10:57] One of the very interesting things here
[10:59] is that even if you prompt GPT
[11:01] in an extremely well optimized way,
[11:03] it doesn't even approach the most basic version
[11:06] of optimization here,
[11:07] which doesn't really square with the scaling laws
[11:11] that people talk about.
[11:13] And this will come up a bunch later.
[11:15] So hopefully it's informative.
[11:16] People like to talk about Rich Sutton's bitter lesson.

### [11:18] Sutton's bitter lesson, and the bitterest lesson

[11:20] I'm paraphrasing.
[11:21] It is vaguely compute matters more than algorithms.
[11:24] This is when people say their bitter lesson pill.
[11:26] They're just like throw a compute at it.
[11:27] Like, you know, let's fucking go.
[11:29] And algorithms don't matter as much.
[11:31] We just want to like, just get as much compute as possible.
[11:33] And this does not explain almost anything in the field.
[11:37] Actually, there's many results
[11:38] that show this simple explanation to not,
[11:40] we'll get to it, we'll get to it.
[11:44] But to me, there is a bitterest lesson
[11:46] that can be simply explained, which is data matters.
[11:49] Data matters way more than compute
[11:52] and doing the right task matters way more than data.
[11:54] And the reason why the scaling laws work
[11:56] and you get compute matters more than algorithms
[11:59] is in some domains, the data is already available,
[12:02] like on the internet or in RL, even better,
[12:06] doing the right task is kind of like a toy experiment
[12:09] that's simply defined and data is a function of compute.
[12:11] So kudos to Sutton,
[12:13] but I don't believe that is like a fully explainable thing.
[12:17] And this will come up a bunch.
[12:19] I believe that this explains
[12:21] the rise of Anthropic quite easily,
[12:23] which OpenAI in terms of what their alignment is,
[12:27] what is their North Star?
[12:28] What right task are they doing?
[12:30] Kind of like flops in between like some mixture
[12:33] of making it good for chatbots,
[12:35] making it good for like PhD level,
[12:37] like sciencey reasoning stuff,
[12:39] and also like playing catch up on code.
[12:41] And Anthropic clearly has a more clear North Star for this.
[12:45] And that is how they're catching up.
[12:47] And notably, if you look at this curve
[12:50] and you try to explain it with any of the things
[12:53] other than doing the right task, you are kind of cooked.
[12:56] You know, OpenAI spends more in data than Anthropic.
[12:58] They have way more compute than Anthropic.
[13:00] And I think that you could maybe make a case
[13:03] that Anthropic is way better than algorithms,
[13:05] but I wouldn't personally do that.
[13:07] I think the answer is simple.
[13:09] Like you just get what you optimize for.
[13:11] And that's the whole lesson of this part.
[13:12] Oh, hell yeah.
[13:15] You get what you optimize for.
[13:17] This is like the lesson in all of ML, AI, LLMs, whatever.
[13:20] It is just optimization with style.
[13:22] If you forget that you're optimizing stuff,
[13:24] then you are also not understanding the space.
[13:27] And we have optimized a lot for strings in RLHF.
[13:35] Cool.
[13:37] Can I get a vague show of hands of who knows what RLHF is?
[13:42] Oh, about half.
[13:44] Oh.
[13:45] Huh.
[13:47] We'll improvise this.

### [13:49] What RLHF is and what it costs you

[13:48] We'll see if it comes up.
[13:50] So RLHF stands for Reinforcement Learning from Human Feedback.
[13:54] It is the algorithm behind ChatGPT.
[13:58] And if you see plots like this, RLHF is what made that happen.
[14:02] As far as I can tell, roughly 100% of actual production LLMs
[14:06] are trained with RLHF or some variation of it.
[14:10] And this is what is used to just make models work.
[14:13] There's a lot of trade-offs to that.
[14:15] I might have gone a little bit too deep in this presentation.
[14:19] So sorry.
[14:22] But hopefully, I'll be recording.
[14:25] But RLHF results in a bunch of very odd properties
[14:28] that are worth talking about, because they're very centric
[14:31] on this string-in, string-out paradigm that
[14:34] is very good for human consumption,
[14:36] but very bad for automation.
[14:38] The first thing I'm going to talk about with RLHF--
[14:40] and this is kind of like a fun fact--
[14:43] is that there is this famous slide

### [14:44] Yann LeCun's doomed-LLM argument

[14:45] of Yann LeCun-- if you don't know who he is,
[14:47] he is like one of the godfathers of modern AI,
[14:51] definitely a godfather of deep learning.
[14:53] And he has this slide that people
[14:54] like to dunk on him about, which is that LLMs are doomed.
[14:59] He keeps saying LLMs are doomed.
[15:01] He thinks that they're a bad direction.
[15:02] And he makes this really simple mathematical argument,
[15:05] which is that if you're making a sequence of tokens, which
[15:09] is what an LLM does, if each sequence is a probability
[15:12] of being incorrect, and you make a lot of them in the row,
[15:15] the probability of the whole sequence
[15:16] correct goes down exponentially.
[15:19] And this is a very interesting, intuitive argument,
[15:23] except that common sense shows it doesn't work.
[15:27] You ask ChatGPT for a Wikipedia article,
[15:28] it's going to Wikipedia article that thing really well.
[15:31] It's not going to degrade and diverge exponentially.
[15:35] It is just going to be right or wrong,
[15:37] just like a short answer is going to be.
[15:39] So that's kind of weird.
[15:40] You have these two different takes on what's going on in string
[15:44] models, and the reason this occurs

### [15:49] Mode collapse, RLHF's deal with the devil

[15:48] is something called mode dropping or mode collapse.
[15:51] This is RLHF's unfixable deal with the devil,
[15:55] because it really is a feature and not a bug.
[15:58] When you have an optimization surface-- oh, man,
[16:00] I'm going to go way too technical, aren't I?
[16:03] Do it?
[16:03] Oh, boy.
[16:05] When you have an optimization surface that
[16:07] is way too complicated for your underlying function to fit,
[16:10] or it's very hard to fit it, you end up
[16:12] having to choose what the shape of your string
[16:14] or the shape of your errors are like.
[16:16] And this tends to occur from your optimization function.
[16:19] And two rough shapes of errors that can occur
[16:23] are mode collapse and mode covering.
[16:27] So sometimes, in between two modes,
[16:30] you can get stuff in the middle.
[16:32] And pre-trained language models are really good at this,
[16:35] because their loss does that.
[16:36] That is why they are so creative,
[16:38] and they can write about absolute crazy stuff.
[16:41] On the flip side, RLHF is extremely mode dropping, which
[16:44] means that if you have two possibilities,
[16:47] it's highly incentivized to just go with the safe option.
[16:50] And this makes things look very plausibly correct pretty much
[16:54] all the time.
[16:55] And that should give you a pretty big huh
[16:58] about the state of AI.
[16:59] Why is it always so plausibly correct?
[17:02] And this is not true of all LLMs.
[17:04] This is a property of RLHF.
[17:07] I promised myself when making this,
[17:09] do not nerd out too much here.
[17:10] So I will not do that.
[17:12] But this is a very important property
[17:14] of the whole of what RLHF is doing
[17:17] and why there is no free lunch in the optimization,
[17:19] because this is an extremely valuable property.
[17:22] If you did try to mode cover, that
[17:25] is OK when you're doing one decision.
[17:27] But if now you're doing another decision based on that decision,
[17:30] and you do that like 10,000 times,
[17:32] you now have zero chance of a plausibly correct answer
[17:35] with 10,000 tokens.
[17:36] And Yann LeCun's answer gets right.
[17:38] So the TL;DR here would be that RLHF works a lot more
[17:44] like a GAN.
[17:45] So if you have a minority class that is being sampled,
[17:48] it's OK for it to just drop that space
[17:51] to make plausible looking answers,
[17:52] while pre-training is more like doing a blurry image
[17:57] in between instead of just taking stuff that
[18:00] is from the existing training set.
[18:01] I'm not making a claim about what is better for automation
[18:04] just yet, but I am making a claim
[18:05] that this is an undeniable property of RLHF that explains
[18:09] a whole bunch of things.
[18:11] Perfect.
[18:12] And this is a little bit--
[18:13] this is a little bit related to this aside of,

### [18:18] Is software engineering easier than drive-thrus?

[18:18] is software engineering easier than drive-throughs?
[18:22] That is, we should be asking ourselves this question.
[18:24] Anyone who knows what's going on in AI right now,
[18:27] there's a meta-narrative going on that software engineering
[18:30] is getting solved.
[18:31] How could that be easier than drive-throughs?
[18:33] I like drive-throughs as an example,
[18:35] not that I care a lot about drive-throughs, obviously.
[18:37] But because the failures are so public and entertaining
[18:41] that it's kind of like a-- it's like a litmus
[18:43] of what's actually happening in an industry.
[18:45] And the narrative that the really AI people are trying
[18:49] to say is, those people suck.
[18:50] They don't know how to implement it.
[18:52] They're bad at it.
[18:53] And if you just pay us, we'll implement it for you,
[18:55] which is, as far as I can tell, not true,
[18:59] and really hasn't moved the needle very much.
[19:03] So the similar question to me is, if I don't believe software
[19:08] engineering is easier than drive-throughs,
[19:10] I actually think that the way to think about it
[19:13] is that, would you trust a coding agent without--
[19:16] I should have put version control here--
[19:17] but without version control?
[19:18] And that's kind of crazy, right?
[19:20] Like, a coding agent without version control?
[19:23] That's like absolute nuts.
[19:24] But you could trust humans with that, right?
[19:27] Or at least, that was back before my era.
[19:29] My understanding is that software engineering happened
[19:31] before version control.
[19:33] Anyone?
[19:33] I don't know.
[19:34] Yes?
[19:35] Did it happen?
[19:38] Oh.
[19:39] I was at Google, so we did some ancient stuff.
[19:43] But to me, the lesson here is that the models just
[19:47] cannot be trusted to make decisions with stakes

### [19:49] Why a model can't be trusted with stakes

[19:50] that you pay, because they're so encouraged
[19:53] to make plausible-looking answers, which is not
[19:56] what you want if you want to make calibrated decisions.
[19:59] If others pay, it's all good.
[20:00] That is why you see a rise of AI customer support.
[20:03] But the AI customer support never
[20:05] takes actions, because the cost is always to the users.
[20:08] And we'll get back to this later on.
[20:11] Maybe I'll just get into it now.
[20:12] I believe.
[20:13] This is why there's this subtle hatred for AI out in the world.
[20:17] I don't think people hate technology.
[20:19] I think they hate when technology is bad for them.
[20:21] And right now, everyone's extremely
[20:23] incentivized to never trust the AI with stuff,
[20:27] but you need to use AI for stuff.
[20:28] And all of the externalized costs
[20:30] is put on the users, which makes them be like, fuck it.
[20:33] This sucks.
[20:34] And I think this is just the realistic take
[20:37] of the whole thing.
[20:39] Cool.
[20:40] Oh, another aside on this.

### [20:42] Is ChatGPT cooked?

[20:42] What's going to happen?
[20:42] What's going to happen to ChatGPT?
[20:44] Are they cooked?
[20:46] Are they just going to plateau and just be
[20:48] at sub-billion users, or whatever this is?
[20:53] I believe that the answer to that--
[20:55] people can have random ass takes to the future.
[20:57] I'm an optimization guy.
[20:58] There's a very simple optimization take to this.
[21:01] ChatGPT is extremely not cooked.
[21:03] The world is cooked.
[21:06] OpenAI has not yet--
[21:08] and even if OpenAI doesn't do it,
[21:10] and they try to be the goodies, someone else
[21:12] will be the baddie here.
[21:14] This is inevitable.
[21:15] I don't blame TikTok for being brain rot.
[21:17] No offense.
[21:18] But brain rot will emerge.
[21:20] There's just a lot of incentive for that.
[21:22] And if we don't protect ourselves as a society,
[21:25] it will happen.
[21:26] And right now, we're in ChatGPT's MySpace era.
[21:29] It's friendly.
[21:30] It's fun.
[21:31] And honestly, I think that they're just confused
[21:33] about what they're optimizing for,
[21:34] which is why they whiplash back and forth so much.
[21:37] It's kind of obvious that their optimization team
[21:40] lacks horse blinders here.
[21:42] And we, as a field, have not even begun to optimize.
[21:46] Even technical people-- this has nothing
[21:48] to do with my talk, just optimization.
[21:50] I'm just ranting.
[21:51] Even very technical people are surprised by this.
[21:53] I think it's going to be really bad for the world.
[21:55] They make the counterargument that, what
[21:57] if you have the script kiddies tuning your prompts
[22:00] to try to get slightly better at getting more addictive
[22:04] if you're making sex bots or whatever else?
[22:06] And that is absolutely not the same as a company
[22:09] doing production scale optimization on an objective.
[22:12] That you get such rich feedback on that ChatGPT does.
[22:15] So this is going to be bad.
[22:17] Prepare for it.
[22:19] And ideally, governments are involved,
[22:21] but I'm not too hopeful of that.
[22:23] But the lens of optimization just
[22:26] shows that we have not yet begun to scratch that surface.
[22:29] And that is super spooky.
[22:33] Perfect.
[22:33] And the results of all of this string wisdom
[22:37] is the correct thing to do in the short term is to go all in.
[22:42] Actually, you should give up on the other approaches.
[22:44] Truly, unless you're a researcher,
[22:46] you should give up on that because it doesn't work.
[22:49] You see, there's a post from Thinking Machines
[22:51] yesterday, which is they're going all in on human AI
[22:55] bandwidth.
[22:57] I think that that's a very good direction.
[22:58] I don't know how to say this in a way that's not
[23:00] passive aggressive, but that makes sense for most people
[23:03] if you can't solve the automation problem.
[23:07] That is what you should be doing right now,
[23:09] because every use case of AI is human in the loop.
[23:11] And I love this summary here, LLMs are incredibly resistant
[23:18] to layering.
[23:20] Generally, the consumer is a human, not another layer
[23:22] of software, blah, blah, blah.
[23:24] And that is the state of the field right now.
[23:28] So if you are betting on composability or layering
[23:31] or other cool stuff like that, I have bad news for you.
[23:34] You are cooked if you're not doing the optimization
[23:37] yourself, because the optimization
[23:39] is done behind the scenes with ways that you are not
[23:41] privy to.
[23:44] Perfect.
[23:45] Summary for this section is, if you use today's LLMs,
[23:49] please understand their properties.
[23:51] Don't make decisions with stakes.
[23:53] People have tried.
[23:54] I don't know of anyone who's really succeeded.
[23:57] The most I've seen for stakes is having
[23:59] control of a calendar.
[24:01] The open-- you guys tell me.
[24:04] You guys, come on.
[24:05] Let's have a real debate here.
[24:06] But what's happening with OpenClaw makes me--
[24:11] I don't know of another analogy for scared.
[24:13] I'm very, very afraid of what's going on with OpenClaw.
[24:18] It seems to be dying out, but this
[24:20] seems really bad for the world.
[24:22] AI can't do that.
[24:24] Common sense, this is not working out.
[24:26] No matter how crazed people are about it,
[24:28] it's not going to work.
[24:29] Have humans in the loop.
[24:30] That is the number one lesson that OpenClaw did not follow.
[24:33] And I don't know how businessy people are here,
[24:37] but FDEs will not solve this problem.
[24:39] This is the common problem that everyone else
[24:41] is trying to solve with, because the meta-narrative
[24:45] is clearly AI is great, but all of my internal evidence
[24:48] is that AI sucks.
[24:50] It must be a me problem.
[24:52] Let me pay experts to try to do that.
[24:54] But as someone who's worked with those experts
[24:56] and seen how the sausage is made,
[24:57] I don't believe that is the case.
[24:59] Skill issue.
[25:01] Skill issue.
[25:02] Yeah.
[25:02] Well, I don't think it's a skill issue.
[25:04] There are companies in the OpenAI fund
[25:06] that are automating just as little as everything else.
[25:09] I probably shouldn't name names, because--
[25:11] I don't want to start fights right now.
[25:13] I will start fights before we release a product, but not
[25:16] today.
[25:18] But that is the narrative right now,
[25:20] and I don't think it makes sense.
[25:22] And I think it's actually bad for the world
[25:24] to be this misleading, even though there's obviously
[25:26] so much economic incentive to do so.
[25:31] It's like therapy for me.

### [25:33] Type-safe language models, going beyond strings

[25:33] Perfect.
[25:34] Type-safe language models.
[25:35] This is type-safe, the adjective, not the company.
[25:38] Type-safety is a concept in computer science.
[25:40] It's about going deep.
[25:40] It's about going beyond strings and actually integrating
[25:43] into programming.
[25:44] I want to talk about these in general.
[25:46] It'll be clear when we're in the ad section.
[25:49] My claim is we can do better.
[25:52] This is a very spicy claim.
[25:55] And actually, this is probably the biggest question people
[25:58] ask when I say how--
[26:01] people are like, this whole narrative makes sense.
[26:03] But surely they can't be that bad.
[26:06] And I will tell you, when I first got this direction,
[26:09] I was at OpenAI.
[26:10] At the time.
[26:11] And when I had it, I was like, shit, this is too obvious.
[26:14] Anthropic is like a year ahead.
[26:16] We are kind of cooked as a field.
[26:17] And OpenAI is cooked in this direction.
[26:19] And it still hasn't happened yet.
[26:20] We still don't have layerable AI that
[26:24] is kind of like the intelligence too cheap to meter.
[26:26] And I probably shouldn't talk too much about what we do.
[26:29] But I believe this is possible.
[26:32] There's two questions that arise from how that can be.
[26:36] And this is still with technologist hat Diogo.
[26:38] So I want to be inspirational and say, you can do,
[26:40] you can do better.
[26:41] We can do better.
[26:42] The field doesn't end here.
[26:43] You should give up on this in the short term
[26:45] if you're trying to use LLMs today.
[26:46] But this is the inevitable future of the field
[26:49] because this is just obviously how you get to automation.
[26:53] So question one that comes up from the claim of how can--
[26:57] we can do better is, how can that be possible?
[27:00] Is the mainstream-- oh, god, I sound like a conspiracy
[27:02] theorist.
[27:03] Are the mainstream labs so bad that they just don't know this
[27:09] and they're just like--
[27:10] you know, clowning around?
[27:12] And I call this the FLAN lesson.

### [27:13] The FLAN lesson, when the whole field is wrong

[27:15] There is a paper called FLAN, fine-tuned language something.
[27:18] I don't really know.
[27:19] It's a Google paper.
[27:20] It had 6,000 citations.
[27:22] It was a-- I'm not bitter, I swear.
[27:25] I actually am a fan of this work.
[27:27] They copied the OpenAI thing.
[27:29] They coined instruction tuning.
[27:31] And then they tried to like front run the model, whatever.
[27:35] I thought it was cool because they're like, wow,
[27:37] they open sourced their data.
[27:38] That is so cool.
[27:39] I love data.
[27:40] I love data.
[27:40] This is before I realized that doing the right task
[27:42] is important.
[27:43] It's actually surprisingly hard.
[27:44] And we found that the optimal amount of this is absolute zero.
[27:49] And that's kind of what the state of the art
[27:52] can be at the time.
[27:53] The state of the art can be straight up wrong
[27:56] because benchmarks are super misleading.
[27:58] And their data was worthless.
[28:00] And the sad part is that people continue
[28:02] to try their data sets.
[28:03] And I still see papers every now and then that are like,
[28:06] we tried this data and we found the optimal amount was zero.
[28:09] And I'm like, is this even possible?
[28:14] And I'm like, no, this is just like, the mainstream--
[28:17] the field can be wrong.
[28:19] This is just how the field works.
[28:22] And a flip side of this question is, if RLHF was so obvious,
[28:27] which now it's extremely obvious.
[28:29] It wasn't obvious at the time.
[28:31] It wasn't even in-- there were like three post-training methods
[28:34] at OpenAI at the time.
[28:36] It was everyone's least favorite one.
[28:37] No one liked it because it was an alignment project, and it was
[28:39] because we did the right task.
[28:41] The other efforts kind of failed.
[28:43] But like this end up becoming like unbelievably successful,
[28:46] took over the whole world, giant commercial success.
[28:48] And it seems obvious in hindsight.
[28:50] But why didn't we do it with GPT-2?
[28:52] Like I can't jump to that slide right now.
[28:54] But like a GPT-2 sized model clearly could completely
[28:58] trounce GPT-3 at this thing, right?
[29:00] And the thing is that doing this is actually super duper hard.
[29:03] And like I would say that it's happened maybe 1 and 1/2 times.
[29:09] And if you count LLMs, it's starting like GPT-3-ish.
[29:13] They've been around for like, what, 6 and 1/2 years?
[29:16] And there was RLHF, which I believe
[29:19] is like the biggest curve in the road.
[29:21] And then RLVR, which I'll give like half credit for because people
[29:25] kind of use it, but it's still RLHF because the RLVR doesn't
[29:27] really work very well.
[29:28] It's the reasoning stuff.
[29:30] And that's a very small amount of time
[29:33] for like six years of innovation.
[29:34] And it's not an easy thing to do.
[29:39] The second question that people love to ask

### [29:40] Can one model do assistance and automation?

[29:41] is, can we have the best of all worlds?
[29:43] Can we just get like a model that is super good at assistance
[29:47] and then just find--
[29:48] maybe like ChatGPT-10 is going to be good enough at automation
[29:52] that we finally solve it, but we provide
[29:54] a lot of value of automation from assistance before that.
[29:58] And if you look at the old curve,
[30:00] you'd have to extrapolate so fucking far to hope for it
[30:03] to generalize to the wrong task.
[30:05] How GPT-3 was beat is its task was understanding-- modeling
[30:09] and pre-training distribution.
[30:10] And how what ChatGPT could be beat at automation
[30:14] is that they're optimizing for human preference.
[30:16] That is different.
[30:18] And my response to this, if I were to give another one,
[30:21] is actually the same image.
[30:24] Can you actually like max out on bread and poppy seed
[30:29] or whatever that is?
[30:31] And like you just can't have both, right?
[30:32] Like you can have like an optimal mixture of them,
[30:35] but it's just very hard because you get two extremely
[30:38] different properties you're optimizing for.
[30:40] And if they're pulling in super different directions,
[30:43] you end up like shattering the optimization space
[30:45] and ending up with like what looks like spiky intelligence
[30:48] or the jagged frontier.
[30:50] And this, I believe, is not great
[30:52] because if you want actual reliability,
[30:54] you need like nines of reliability to work.
[30:57] And this is like trying to go for both
[30:59] is kind of like poisoning your models.
[31:01] Like maybe in the limit, but I don't
[31:03] think that you can get the best of both worlds here.

### [31:08] What TypeSafe is building

[31:07] Perfect.
[31:08] Now for my ad component.
[31:10] Oh, I have not too much time.
[31:13] I'm going to remove my passionate researcher hat,
[31:15] talk about TypeSafe hat.
[31:18] I can't tell you what we're doing.
[31:20] Bryan Bischof told me so.
[31:22] He says-- but like what we're doing
[31:24] is we're giving up on all the assistance,
[31:26] going all in on automation.
[31:28] I would tell you more, but this is a real email.
[31:30] This is not AI.
[31:31] This is not-- this is a fake picture.
[31:33] But yes, we are just going all in on that automation.
[31:37] I want to talk about the why.
[31:38] We can talk about what to happen afterwards.
[31:40] But for me, I believe AI truly is this world-changing thing.
[31:45] Like pre-trained-- like the fact that ChatGPT released,
[31:49] what, three, four years ago?
[31:51] I don't really know.
[31:52] And like right now, all we have or all what most people have
[31:56] is like ChatGPT and now Claude Code.
[31:58] That's kind of insane.
[31:59] Like we thought that there would be
[32:01] like this foundational new technology
[32:03] that we're building up on.
[32:04] And now we just have like this additional tab
[32:06] we sometimes open that's kind of
[32:08] like Google.
[32:09] This is such a waste of a technology.
[32:12] And I think that that economic revolution still
[32:14] can be happening.
[32:16] I will say that what made me decide to do this startup was
[32:19] that if the AI bubble bursts--
[32:22] I'm hoping it doesn't burst at all,
[32:24] so I will look really dumb if we manage to avert it.
[32:27] But this is what I care about.
[32:29] If I did not go all in to avert this,
[32:31] I will feel endless regret for the part
[32:33] I played in making this happen.
[32:34] I really think RLHF was like a curve in the road
[32:38] that was not necessary to happen.
[32:40] I think it's great, but there are other curves in the road
[32:43] that I think people don't realize we could have taken.
[32:45] And I will give like one more question
[32:48] that I'll talk about later on.
[32:50] But I have two minutes.
[32:52] If you want to be involved, people
[32:54] are making me say this at the company.
[32:56] If you want to do real automation,
[32:57] we have a signup form.
[32:58] We have like a--
[33:01] if you want to skip the wait list,
[33:02] you should tell us more about what you're doing.
[33:04] We are not a perfect fit for all automation.
[33:06] If you are trying to make--
[33:07] like an assistant-y, human-in-the-loop-y thing,
[33:10] we are the wrong model for you.
[33:12] We want hardcore, super boring-ass automation
[33:15] of finally automating jobs.
[33:18] Please fill that out.
[33:19] If you want to build it with us, we have a careers page.
[33:22] I swear we are very, very cool.
[33:24] My co-founder's here, and we will answer questions later on.
[33:27] But we are hiring all sorts of roles,
[33:30] and we are growing super fast.
[33:32] We are getting ready to release this in a couple of months.
[33:34] And for everything else, just email us.
[33:37] Oh, I have another slide.
[33:40] Summary-- my summary is we optimize for assistance.

### [33:42] Summary

[33:44] That is the answer to the question of over-promise,
[33:46] under-deliver.
[33:46] It's super-duper clear to me.
[33:48] Hopefully, it's clear to you guys.
[33:49] I'm down to test hypotheses together in that room later on.
[33:54] Don't have AI make decisions with stakes.
[33:57] I think it just gives AI a bad name.
[34:00] Hopefully, everyone has internalized this.
[34:01] But maybe if you say it this clearly,
[34:03] it becomes a little bit easier.
[34:04] Should we install OpenClaw to, like, you know,
[34:07] onboard users into whatever organization?
[34:10] Like, it depends.
[34:11] Are there stakes or not?
[34:12] Do you have the security around it
[34:14] to make sure it can't do harm?
[34:16] If not, don't do it.
[34:18] Or if you want people to laugh at you, go for it.
[34:22] And if you are jaded with LLMs, just
[34:24] know that we as a field can do better.
[34:26] I know that there are some people who
[34:27] are, like, super passionate about actually having
[34:30] this new layer of technology.
[34:32] And there are other paths other than RLHF,
[34:34] which I am personally excited about working on.
[34:37] And there might be more.
[34:38] But I am super jazzed about this whole path.
[34:41] And I have 11 seconds, which is perfectly timed.
[34:46] This was the question that made me re-- actually,
[34:48] I'm going to go over time on this one.
[34:49] I just realized I have, like, background.
[34:52] When we released RLHF--
[34:54] actually, this was around the GPT 3.5 era--
[34:57] we got the results that the models were super human

### [35:00] Was RLHF AGI, and what was missing

[35:00] at following instructions.
[35:02] This whole string in, string out thing,
[35:04] we were surpassing human level at this.
[35:06] And at the time--
[35:07] I mean, we asked ourselves, is this AGI?
[35:10] And then the whole team was like, let's not release this,
[35:12] because they were safety researchers.
[35:14] But I'm, like, a capabilities researcher.
[35:16] So I said, fuck it, we ball.
[35:17] Like, release it.
[35:18] And that was-- it was huge, you know?
[35:20] Like, it took over the whole world.
[35:22] But for some reason, it was used for extremely little
[35:24] automation.
[35:25] It was only used basically for copywriting.
[35:28] I believe, like, 90% of the usage was.
[35:30] Copywriting is like writing these giant, like, salesy fake
[35:33] emails.
[35:33] And like, that's, like, so bad for the world.
[35:36] Like, I don't want to judge--
[35:36] I'm not providing value or whatever else.
[35:38] But, like, it's a hard thing to defend.
[35:40] And I used to ask myself, like, what was missing?
[35:43] Like, how can AI be so smart on average?
[35:46] Because, like, you don't even need
[35:47] to get to average double human performance.
[35:49] Like, half the humans are dumber than that.
[35:51] But they can still do a lot of work.
[35:53] Not an insult to them.
[35:54] It should, like, ask us about, like, our technology.
[35:57] We should think about it.
[35:58] And what was the missing thing?
[35:59] And I don't think it's about making the model smarter,
[36:02] per se.
[36:02] I think we were just doing the task wrong.
[36:04] And the thing that really clicked for me
[36:06] is, if we had an AI-based economic revolution,
[36:09] the kind of thing a lot of people have given up on now,
[36:11] and they're just thinking, like, the future is,
[36:13] like, Claude Codes for days, what percent of calls
[36:16] would be simple stuff to computers?
[36:18] You know, like, the kind of thing
[36:20] that's optimized for, like, computer understanding,
[36:22] having LLMs be a new primitive, more like databases and APIs,
[36:27] rather than, like, facsimiles of co-workers.
[36:29] And should we optimize for that thing?
