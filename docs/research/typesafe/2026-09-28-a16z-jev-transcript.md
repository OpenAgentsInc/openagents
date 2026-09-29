# How Jev Turns AI Into Software That Gets Things Done

Transcript of the a16z Show episode with Ben Horowitz, Martin Casado, and TypeSafe AI founder Diogo Almeida.

| Field | Value |
| --- | --- |
| Source | [https://www.youtube.com/watch?v=Ut3LOjKNJaE](https://www.youtube.com/watch?v=Ut3LOjKNJaE) |
| Channel | a16z (The a16z Show) |
| Published | 2026-09-28 |
| Duration | 42:24 |
| Transcribed | 2026-09-29, locally with whisper.cpp `large-v3` (beam size 5), then proofread for names and repeated segments |
| Speakers | Ben Horowitz (a16z), Martin Casado (a16z), Diogo Almeida (TypeSafe AI) |

Timestamps are `[mm:ss]` from the start of the video. Chapter headings follow the episode's own chapter markers. Speakers are not labeled; the three voices are Ben Horowitz, Martin Casado, and Diogo Almeida. This transcript is kept for research on TypeSafe's approach to putting typed, probabilistic judgment inside software; see the `typesafe-ai` skill and https://typesafe.ai/ for the product.

## Episode description (from YouTube)

> a16z’s Ben Horowitz and Martin Casado sit down with TypeSafe AI founder Diogo Almeida to ask a simple question: AI has become remarkably capable, so where is all the automation?
>
> Diogo argues that coding agents may help us write software faster, but the software they produce still largely works the way software always has. TypeSafe is taking a different approach with Jev: putting intelligence inside software itself, so developers can build programs that reason about intent and make probabilistic decisions rather than simply generate text for a human to interpret.
>
> They discuss why reliability is the key to making AI genuinely programmable, how this could open a new era of probabilistic software, and why established SaaS companies may be particularly well positioned to benefit. Ultimately, Diogo’s goal is straightforward: technology that can reliably “do what I mean.”

## Chapters

- [00:00] Intro
- [00:50] Meet Diogo and Type Safe
- [04:07] Smart software, not just faster code
- [15:46] Where's all the automation?
- [22:00] Is it just a data problem?
- [30:21] SaaS apocalypse, reversed
- [34:58] New capabilities, not more code
- [38:27] Apps vs the guts of systems
- [41:08] Reliability and "do what I mean"

## Transcript

### [00:00] Intro

[00:00] Where the fuck is all the automation? AI is so unbelievably smart, and yet it's so useless at all other stuff.
[00:06] It doesn't matter how much AI coding agents you use, the software actually isn't getting better.
[00:10] Maybe you're running it faster. It's like, aren't you really getting worse?
[00:13] OpenAI has been trying to automate customer service since 2020. What I want instead is smart software.
[00:19] I want to expand what software itself can do, such that things that should be automatable can then be automatable.
[00:26] My favorite thing that you guys say is we build prod, not God.
[00:30] That's so good.
[00:30] Because if we had any other kind of like big lab leader, even if they had joy, they would cover it up.
[00:37] And then your view is so different. You're like, no, we're going to create a way better world.
[00:43] For nuanced reasons, I don't think we are on the path of RSI. In the SaaSpocalypse story...

### [00:50] Meet Diogo and Type Safe

[00:50] Today, we have the founder and leader of...
[00:56] ...TypeSafe, Diogo, with us, who is a bit of a hero to both Martin and me.
[01:05] He is not only building like a really interesting product, but creating what we think is a very important movement.
[01:13] So we're super excited about today.
[01:15] Thanks for coming.
[01:16] Yeah, thank you.
[01:17] And maybe you can give us kind of a brief on just, you know, what is Jev, what is TypeSafe?
[01:26] Why is it important?
[01:29] Is this a curse friendly or not?
[01:31] Yeah, yeah, yeah.
[01:32] Oh, okay, cool.
[01:33] What the fuck are you talking about?
[01:34] Oh, okay, okay, okay, cool.
[01:35] So I was actually asked for like an elevator pitch, which I tend to ramble on and I don't do well.
[01:41] But like, I realized my favorite elevator pitch for Jev is where the fuck is all the automation?
[01:46] Like, this is like so unbelievably tragic, you know, so much intelligence, AI is so unbelievably smart and yet so...
[01:55] Not that I hate on chatbots.
[01:56] Or coding agents.
[01:57] I love them myself.
[01:58] But it's like, it's so useless at all other stuff.
[02:02] And it's tragic.
[02:05] It's tragic that we have so much like diamond in the rough, but not polished for work.
[02:10] That's, but TypeSafe is making AI for software.
[02:15] You know, we want to make AI powerful, not just for humans in the loop, but to actually build real software.
[02:20] And Jev, to us, is our first model in this whole space to make it way, way better.
[02:26] Yeah.
[02:26] To like...
[02:26] To make automation.
[02:28] Yeah.
[02:28] And so it's been interesting because it's kind of caught fire in software world.
[02:33] So though, you know, one of the things that made us go, what the hell is going on here is like every developer we know is calling us and going, oh, this is fricking awesome.
[02:42] It's great.
[02:43] It's fast.
[02:44] It's, it's great.
[02:45] Everything's better.
[02:46] Um, and then how does that, uh, cause, cause everybody thinks of, well, we've got Claude Code, you know, we've got Codex.
[02:54] Don't we already have that?
[02:56] Like what's the difference?
[02:57] And then how does that lead to real automation?
[03:00] Ooh, I wish I had like a, some slopped visuals because I have like a favorite slot visual for this.
[03:07] So I like Claude Code and Codex.
[03:10] Um, I love the description from Garry Tan on them.
[03:13] It's just in time software, you know, incredible way to describe what they're doing.
[03:17] It's like, it makes software on the fly and you can like program software in natural language, but it has the same expressive power as software.
[03:24] Um, what I want.
[03:25] Instead is smart software.
[03:27] Like, instead of like automating software engineering, I want to expand what software itself can do such that things that should be automatable can then be automatable.
[03:37] And like in a more, um, flowery language, like I want to express things like intent.
[03:43] I want to like expand the vocabulary of what we can do.
[03:46] And I can talk about like all sorts of like weird sci-fi things I want, but like programming is like hyper specifying, like valuable things and then infinitely replicating them.
[03:55] It's so freaking cool.
[03:57] And I want to just make that more, you know?
[04:00] Oh, interesting.
[04:01] So, so one way to think about it is instead of kind of a tool that, uh, somewhat replaces a software or engineer with a faster, maybe not even as good software engineer.

### [04:07] Smart software, not just faster code

[04:14] What you're saying is no, no, no.
[04:16] We're going to super empower the software engineers.
[04:19] We have to write way, way better, more interesting things.
[04:25] Yeah, I, I actually, I mean, yeah, yeah.
[04:27] So this is, by the way, I just think so many people miss this point and it's such a subtle point and it's so important to actually tease it out, which is, um, if you use something like Claude Code or Codex, which is great or Cursor, which is great, they write code, but that code is the same thing a human being would have, right?
[04:42] Maybe it's better, maybe it's worse, but it's basically still code just like code looked 10 years ago.
[04:47] And the thing with Jev is whether or not your Claude Code or a human, you have this new primitive, this new thing that you stick.
[04:54] In your code that actually expands, like the power of software.
[04:58] So instead of like writing code, it is something that you include in your code, which go ahead, go ahead, which by the way, is interesting because it's this very powerful primitive, which would be great if you explain.
[05:10] But it's also a little bit different than like, you know, how programmers think, for example, like it has this notion of like, you know, probabilities or, you know, and like, so, you know, so, so an intelligent layer inside the software, think of like, like a library, like a library.
[05:24] It's a library that you can like, use natural language to describe what you want, and you give it kind of a state machine, and then it will will choose what to do with some confidence levels, which we kind of haven't really had before, like, so ubiquitous, so maybe, ooh.
[05:39] There's a lot of tricks there.
[05:41] I will jump into one thing first, which is I love the first thing you said, like, you know, in the direction of where the fuck is all the automation.
[05:49] I love software so much.
[05:51] I wish I could be writing it all day.
[05:53] Yes.
[05:54] I would not recommend being a CEO to people, but whatever.
[05:57] And also, like, it's wild that AI is so cool, and software has been unchanged in 10 years, you know, like, like, like that, to me, like, no one can like square this together.
[06:07] And the most we can do is add like a little chat bot in the side sometimes that can take actions, but not all actions, because some of the actions are not reliable enough.
[06:15] I just want to give like that tiny aside, I love the point, I'm gonna jump back to the point about like, this is a little bit of a different way to think about it.
[06:23] I think that, yes, I think that machine native doesn't exactly match bits perfectly.
[06:31] And like, that's actually the art form that we are trying to do.
[06:34] Like I, in our onboarding on day one, I draw like the Venn diagram of like, what AI is good at, what is valuable in code, we're in the middle.
[06:42] So, you know, we don't output like, you know, extrapolated floats, for example, because like, AI is just bad at that.
[06:48] Sure. You know, but things like probabilities are not exactly novel.
[06:51] And it's similar to the, is Jev just a classifier argument? Jev is absolutely
[06:58] a classifier. You know, like classifiers are sick. Classifiers were designed to be useful. And
[07:05] actually it's the same interface as like some of those ML concepts, because these came from like
[07:11] practical people who are trying to make systems work. And what I'm seeing is happening now is that
[07:16] Jev actually, I, my guess is that Jev probably is better than having like an MLE team from 2019
[07:24] making the stuff for you and you can just program it on the fly. Who knows what could be built?
[07:29] Because like there were not that many good MLE teams in 2019 to build like narrow things and to
[07:34] be able to like collect data sets and measure it and all of that. And it is just the beginning.
[07:39] There's, I feel like there's way like, like. By the way, to this point, do you think
[07:42] there's a slider bar here where like on one end it's like language in, language out,
[07:46] like we have today on the other end is like a, like an existing imperative program. And then
[07:50] you can kind of move between the two, or do you think like, this is like the point in the design
[07:53] space, which is language in kind of state machine out, which is gonna like solidify as the general
[07:58] purpose thing for programmers. Ooh, that's a tricky one. So I will say the answer in my heart.
[08:04] Yeah. The answer in my heart is that it is a slider. So in, and actually when I design for
[08:10] the properties we have, I might've made mistakes due to my personal preferences. Yeah. But like
[08:16] intelligence per dollar is my North Star right now. And it could be wrong just to be clear
[08:19] intelligence per second might be more valuable in the short term, but like, even like our interface,
[08:24] like calling the input state, this was intentional. Like it's to say, oh, that's great. I didn't catch
[08:29] that. It's meant to be the inside. Oh, I love that. So, so in my heart, cause like, so we are
[08:35] really optimizing a lot of the work I do is for even more complicated arrangements of the internals
[08:40] of program state. Can you put intelligence in there? I think that this is going to be an ever
[08:44] present battle to have. I, you know, we're very intentional about our design and also pragmatically,
[08:50] I think certain things happen. Like it's easier to make an AI at these milliseconds. So it'll be
[08:55] more like a database for a while than like a standard library thing, but I would love it to
[09:00] be a standard library thing too. Can I, can I just pull back? Like, what is the alchemy that creates
[09:04] a Diogo? I mean, like you speak when a man and a woman love each other, you speak like an AI researcher, you still speak like
[09:13] a systems person and you speak like a programmer. And normally these things have been like, not super
[09:18] overlapping and like you're, you're taking, you know, AI, which we've been pushing towards, you
[09:23] know, being a being and you're making it a programmer's tool. So maybe a little bit about
[09:26] your personal journey that. My history into AI is somewhat unorthodox. Um, I was a mathlete. Um,
[09:33] I was a award-winning mathlete. Um, the way I describe it is I was good enough at, this is a
[09:39] cringe. I was good enough at math to get girls. So that's quite good. No, that was a thing.
[09:43] And what kind of girls do you get when you're that good at math? That's our audience needs to know.
[09:53] Oh no. We have to inspire the youth here. Don't do it. Don't do it. It's not worth it.
[09:59] Just be cool and chill and interesting and don't overcompensate. Wow. I can't believe I said that.
[10:07] Um, so I was a mathlete, but I actually never, oh man, this also is a little cringe. I never really
[10:13] did math. I never really tried. I was just like big fish in little pond. And to me, math was,
[10:20] I math was always the path I was set on, but I hated it because it was always about like winning
[10:25] competitions. But then computer science is actually a lot like math. It's basically like
[10:29] math, but cool and useful and fun and interesting. And I still love giving algorithms interviews.
[10:36] It's not, is it the best thing for me to do? I don't know, but do I love it? Yes. And does it
[10:40] like allow me to like suss people out, out really well? Yes, it does. So,
[10:43] I love computer science. I consider myself to be computer scientist much more before AI researcher,
[10:49] despite my history. And, um, like what actually got me into it was I also won a Kaggle competition,
[10:55] not from sophisticated math, but from like, just automating like the fuck out of it. Um,
[11:01] you know, like, just like more nested loops, more, you know, like it, it, it solved like a systems
[11:06] problem, you know? So that event eventually got me, um, like I was forced to speak at NeurIPS normally
[11:13] as an honor, but I hated it because I just wanted to be in the mines. Was that from the Kaggle thing?
[11:17] Yes. Oh, wow. Yeah. Actually the Kaggle host of it was Isabelle Guyon, who was the co-inventor of the
[11:24] SVM. Actually, I think first author of SVM. I'm not a hundred percent sure I'm first author. Um,
[11:29] and she just basically saw that I was like this person who really didn't fit into the research
[11:34] community and then adopted me and showed me like, it got me to meet all the AI people. And that,
[11:41] you know, my career was just pushed.
[11:43] Into that direction from there. OpenAI? No, it was like a startup with Jeremy Howard. Um,
[11:52] no kidding. Yes. I love Jeremy. Yeah. Fantastic. Cool. Um, and then Google Brain for a while. Wow.
[11:59] And then, uh, retire for a while. Yeah. And then eventually I was like, just kind of tired of not
[12:07] doing anything. And I was like, you know what, actually AI is pretty damn fun. And I joined OpenAI
[12:13] uh, and it worked out really well. Amazing. Really, really well. Yeah. Yeah. Incredible. So
[12:19] you, you said something there that is so, um, unusual in today's world, which is AI is really,
[12:28] really fun. And then the company has such a different demeanor and view of AI than every,
[12:36] everybody else. And my favorite thing that you guys say is we build prod, not God, uh, because if we had
[12:43] any other kind of like big lab leader, they'd be like trying to, even if they had joy, they would
[12:49] cover that. And then your view is so different. You're like, no, we're gonna create a way better
[12:57] world and it's gonna be awesome. And there's gonna be not only are there not gonna be less jobs,
[13:02] there'll be more jobs and there'll be way better jobs and everybody's gonna have a great time. And
[13:06] like, just be being around you. Like you clearly believe that. So, so tell us about that and like,
[13:12] what this, cuz,
[13:13] for us, you know, TypeSafe, Jev, it's, it's more than a company. It's a, it's a whole movement
[13:18] towards a positive future that most people in the AI world kind of don't like. Yes. Or, or, or they're
[13:26] not with it. I think they don't get it. Yes. You know, like the, it's just a classifier complaint.
[13:31] It's like an ML level concern while everyone else is having like a Jev party, because it's like,
[13:38] holy shit. Like we can do all the things that we wanted to do. And I don't, I think if you don't
[13:42] get developers, it'll be hard to understand what's really going on. Yeah. So a hundred percent. I agree
[13:48] with that. I do think that there's like a pretty negative world painted that I obviously disagree
[13:53] with. I think it's really comes from this, like, um, you know, mono model Kool-Aid that everyone
[14:00] believes. Right. Right. One big brain to rule them all. That's one, that's one way that sounds,
[14:07] sounds much more ominous. Yes. Yes. But that's, that's what people hear. Yeah. For sure. That's
[14:12] what people hear. Yeah. But you know, like, will that one, is that one brain really on the path to
[14:17] rule us all? Like we have not automated really basic things that I don't think we want people
[14:22] to be doing, you know, like it there's lots of really, really basic stuff. And I think that,
[14:28] oh man, it, it, it pains me when the world is discordant with the reality and like part of the
[14:35] pain is, you know, on the, where the fuck is all the automation? Like how can we have AI be so smart?
[14:42] Mm-hmm . And,
[14:42] and, like, there's so much, so much financial incentive to automate stuff. Like, yeah,
[14:46] you could make an excuse for diffusion. I don't buy it at all. Yeah. I shouldn't name names,
[14:50] but like that obviously is not true. Part of the problem is like the discordance with the reality.
[14:55] And the fact that AI has like so much potential is what made it really tragic for me that we had
[15:01] not released this. So now it's a, now it's like a little bit of a party for me, but like, I was
[15:06] afraid of AI. And all Jev users are like, there's the happy AI, the people on Jev.
[15:12] Yeah. And then there's the morose AI, the people who are not.
[15:15] Yeah. Yeah. Yeah.
[15:16] It's, it's really, uh, it, it's quite a kind of fascinating dichotomy. It's,
[15:22] it is really, well, I'll give you an, and to your automation point, I had a funny conversation this
[15:28] morning with, uh, David George who runs our growth fund. Cause we're talking about the new tools. I
[15:32] was like, oh, have you tried the Muse thing? He's like, oh, it's awesome. I was like, what'd you do
[15:35] with it? He said, I finally canceled my New York Times subscription. And I was like,
[15:41] that, that is hard to do. But you know, it's a kind of a, it's a very tip of the iceberg of

### [15:46] Where's all the automation?

[15:51] the things that are horrible things to do that, that we need to automate.
[15:55] I think that if we were going to be really intellectually honest and we are really aiming
[16:00] for the North Star of automation, we cannot fall into the same anti-patterns that AI has fallen
[16:05] into, which is really, um, focusing on outliers and demos, right? Like a lot of people ask me,
[16:11] like, what are your favorite use cases? And I'm like, I'm not sure if they work. I want them to
[16:14] work in the background such that like someone would trust that to run and not page them. And
[16:19] like people can build on top of that too. And like, you know, composable, composable, but like other
[16:24] things like safe, right? Like it's a different type of safety where like, if you want it to
[16:29] actually like run with resources associated with it, with access to things, you need guarantees
[16:34] for that, or like at least statistical guarantees. And, um, like, so it doesn't go rogue breaking.
[16:39] I can face that type of thing.
[16:41] So I, I don't think our models will be doing that anytime soon, unless someone like does the
[16:46] software to do that, which would be very cool flex, very cool flex. I forgot how to give credits
[16:52] for that, but, um, but. In a way that we're not responsible. Right. Right. Right. I'm just curious,
[16:59] like how long has this intuition been percolating? Cause I remember talking to you maybe in, was it
[17:04] 2017 or so long? We did talk about that. Yeah. Yeah. And like, and then like a lot of these ideas were
[17:11] like, you know, you were talking about data being important and you're talking about like, you want
[17:14] to focus on the task and like, but like, so I just like, you know, was this like, did you know that
[17:20] this was gonna end up being a classifier or was this just an intuition that like, there's just
[17:24] kind of another way to view this entire kind of AI movement, you know? So actually a fun story
[17:30] about that chat in the talk from 2017, I think my talk was actually in a very similar theme.
[17:37] I think it was called something like AI modular in theory and flexible in practice. Yeah. Yeah. Yeah.
[17:41] Which is very software system. Yeah. Totally. Yeah. So I'm a little bit consistent in that. I,
[17:46] I think that this really started right before ChatGPT. Um, like right when we released these things,
[17:52] I did not have intuition about this. And honestly, I was not even, I was very,
[17:57] very pleasantly surprised by the generalization capabilities of RLHF. When is this? Must be end
[18:03] of 2021, like a fourth quarter of 2021. Um, like we were, it was really, really general. Like if you
[18:10] read the paper, it's unlike other papers that are like trying to prove their point. It is us
[18:16] actually, you know, scientific method ish, trying to disprove, like, is it cheating? And, uh, you
[18:22] know, my favorite query was why is it important to eat socks before meditating? We'd made sure
[18:28] that was not on the internet beforehand. And like the models were able to like make plausible you
[18:32] human looking answers for this. And that to us in the team was the thing that click, like,
[18:36] this is not cheating, which you should always be afraid of cheating in ML.
[18:40] Right. And then what really got me burnt was like, we released it. Um, you know, we did a,
[18:45] you know, I'm obviously a big capabilities guy. I did a lot to release that model. I,
[18:53] I really thought that that model had like a decent chance of being AGI. And when it didn't,
[18:58] that was like when my whole world came crashing down and I was like, why?
[19:02] Oh, so you were kind of on the other train for a bit, which like the crazy train or, well, no,
[19:07] just like RLHF generalizes, like maybe we have AGI, like.
[19:12] RLHF generalizes pretty well. RLVR is the thing that doesn't generalize as well from what I've seen.
[19:18] Um, and AGI in, well, I was just saying more, I mean, like, you know, you were in ChatGPT,
[19:26] you were behind these early GPTs. That was a very different goal, which is like creating a chat bot
[19:31] that will talk to the human being. It's not a programmer's tool, et cetera. So I'm just wondering,
[19:34] like, oh, well actually early, early, like 2020, um, OpenAI, when we talked about AGI,
[19:41] people used to describe it as Ilya in every if statement. Um, so it's not, it's kind of like,
[19:46] but like, is it like part, we were talking about OpenAI culture. Part of it is that
[19:52] it's like intentionally vague. So it's a wide like tent so that everyone can be inside of it.
[19:57] But like, I am not for nuanced reasons. I don't think we are on the path of RSI and I still don't.
[20:04] I don't think we're in the path of RSI. And I did then, I do think that what OpenAI defined as AGI
[20:09] is extremely doable. Automating most of the world's economically valuable work actually sounds like,
[20:16] uh, oh man, I don't like there's a, there's a lot of work out there. A lot of it is very rote and
[20:22] simple and like by volume in order to be able to like outsource work, you need like simple
[20:28] instructions that like basic people can do. Yeah.
[20:30] And as far as I can tell, the intelligence of that has been available in the models
[20:34] for like quite a while now. And like my, oh man, you know, chip on my shoulder is like,
[20:41] why is this not available? And, and then they, and since RLHF, the industry just like kind of
[20:47] bifurcated into gigantic over-promise under deliver. I think GPT-3 was actually quite
[20:52] calibrated back in that day, but because humans evaluate how good the models are,
[20:57] it looks really good because they are the judge, but we've been optimizing that judge
[21:00] instead of the automation part. And that has been the missing thing. So I would
[21:04] say that it was really, really then that it like hit me, you know, like,
[21:08] why is this thing not more useful? And so you think that the measure
[21:11] that we should have is to what extent can you automate actual productive tasks that, uh,
[21:15] when you say over promise and under deliver, that's the dimension in particular, you're
[21:21] talking to the ability to automate tasks. I like, I think in my heart, it's like cool
[21:27] sci-fi, um, you know, and I think that I think that that is the canary in the coal mine for cool. Like, are you really
[21:34] telling me that we like math is solved or like even like two years ago, GPQA, the Google-proof
[21:39] question answering is solved, but we still can't handle a drive-through, right? Like it's, it's,
[21:45] it's, it's a very hard thing to hold in your head at once. And I think a lot of people don't have
[21:51] good answers to that. Yeah. Can I just test one thing, which, which, which may, which may not make
[21:55] sense, but I mean, I mean, isn't there an argument though, that like the, the, the real, the

### [22:00] Is it just a data problem?

[22:01] distribution of the real world is. Yeah.
[22:04] Is, is different than the digital world, right? It's heavy tail. There's a lot of exceptions. We
[22:08] don't have all the data. And I mean, it, couldn't it be the case that the reason we're not doing
[22:13] productive stuff with the real world is just like, we're not, we don't have the data for
[22:17] that distribution. We're not training on that distribution. And this is why it's just been
[22:20] basically relegated to like these lower dimensional manifolds, like whatever math or code or.
[22:26] Um, I don't entirely buy the data argument in my opinion. Um, I do believe
[22:34] that there's a, a long tail for sure. Like that would be kind of crazy to deny. And I don't think
[22:40] that in my like canary in the coal mine situation, we need to automate that long tail. Like I think
[22:45] that, I think we need to be incredibly pragmatic on everything and like building reliable software
[22:51] is always an investment, right? Like it, like, you know, what were the three great virtues of
[22:56] a programmer laziness to not to do it again, hubris. And there was a third one.
[23:02] Yeah. No, I remember this is from Perl.
[23:04] Yeah. Yeah. There's a third one. I wish I could remember it, but like, it's about like the
[23:10] laziness to like spend, you know, like 10 hours to do like the five minute task instantly and to
[23:16] never have to do it again. Like it only make like, it should be an ROI decision for people who like
[23:21] automate stuff. Like I would just like it to be automatable. And I think that people will
[23:26] just make like new kinds of work out. Hence the Jev in Jevons new kinds of work once that stuff is
[23:32] doable. But, uh, like.
[23:34] As like a, a benchmark, I feel like it's useful to see, can we actually automate the stuff that
[23:39] it really, really looks like AI should be able to automate nice. OpenAI has been trying to
[23:43] automate customer service since 2020. Yeah. You know, like it, it, it's, you know, like it's not.
[23:51] It's just pretty amazing. It's it's it's wild. You know, it's wild. Well, I mean inside,
[23:57] I mean inside companies, um, there's very little that's automated right now. Like, and the projects,
[24:04] um, the projects haven't worked, um, other than programming has worked amazing.
[24:07] Can you, can you maybe classify the types of problems you think that are easier to automate
[24:13] now? Cause it, it was kind of interesting. So we've actually looked at support before the current
[24:17] generative wave. And it was interesting. You'd meet a company and the company would say, um,
[24:20] we, uh, we answered 95% of all, like, you know, like help desk calls. I'm like, that is so many,
[24:28] but then you actually look at the data all the same and you realize it's all password resets.
[24:32] And then like, but if you did it by like uniqueness, there was only something like 50% or
[24:37] something. So it just feels like when you're dealing with humans and natural systems, like,
[24:41] there's just kind of this very kind of, you know, like a long tail of exceptions. And so to what
[24:47] extent did like I, every, probably every hour I have somebody ping me, I'm like, I'm using Jev
[24:52] for this new use case. I'm like, I had no idea, you know, like, you know, and so like, to what
[24:56] extent did you even predict like the broad range of use case for it? Like, did you assume that was
[25:00] gonna happen? And have you been surprised by that or extremely surprised? Did not assume it would
[25:05] happen. Uh, this launch was like, not something like if anyone expected this, they are probably
[25:12] insane. Um, right. Like it is, I, I don't think someone could expect a ChatGPT for developers.
[25:21] Cause ChatGPT was for, you know, like, you know, the normal users and it's weird. I actually don't
[25:28] even know what percentage of the people who are part of the Jev party.
[25:30] Are developers themselves. I can't imagine non-developers using it. I don't know how they
[25:35] would use it. Um, but even my non-developer friends are just like part of the party and
[25:39] Twitter and memeing and everything like that. So number one, phenomenal. Number two, um,
[25:45] this will be hard to convey in this short message because like, it's been like blood, sweat and
[25:52] tears for years now. Like the amount I care about reliability is, um, it's, it's a lot like,
[26:00] reliability is what this thing is. If you don't understand that it'll be very hard to make like a
[26:06] copycat that's benchmarked. Like it's, I feel like every nine of reliability is going to be so
[26:13] valuable for everyone, even if it's not the most valuable thing, market cap wise, because it will
[26:18] just enable new applications. And like, we are fighting for like all sorts of like weird nines
[26:24] of reliability that like, we don't even fully understand because we are just like, you know,
[26:27] like really getting the, this like.
[26:30] motor of AI of intelligence, like into people's like workstations and they can figure out what to
[26:35] do with it. What does reliability mean in this context? Is this, is this like availability of
[26:40] the model or is it like I call the model and it returns the same thing or like, how, how do I
[26:45] think about reliability? Yeah. So for something that's inherently kind of stuck. So, uh, not so
[26:50] much the former thing. Okay. And the second thing is closer. Like I would describe the first thing
[26:55] as kind of like uptime or SLAs. Okay. The second thing I would maybe call closer to determinism.
[26:59] Yeah.
[27:00] Something thirdly, I would consider more like robustness. So robustness, I would kind of
[27:06] describe as, um, similar intelligence every time. Oh, interesting. Yeah. So like not exactly
[27:11] determinism because I think determinism it's useful for unit tests, but not real systems.
[27:15] Sure. Think about like, if you add a UUID to a prompt, it should be the same because it's the
[27:19] same functionally, but it's not exactly deterministic. Right. Right. Right. I think
[27:22] that there's a, another layer of it that I don't really know what it's called yet. Like maybe this
[27:28] is what I would call like some form of intelligence.
[27:30] Which is it doesn't have to be the similar function every time, but it needs to be smart
[27:35] every time, you know, like w if you were in that situation, would this be an understandable thing
[27:40] for a human to think because a developer can program around that. And actually to me, the
[27:45] highest honor of reliability will be to get to the point when people can program against Jev without
[27:52] making example queries. Like when you just trust it, you'll be in like perma-flow state, just crazy
[27:58] software. And like, that's where a lot of software is today. Right? Like I, I don't think it's totally
[28:05] unrealistic, but I'm gonna gonna be fine by the way, this is kind of a weird question. So like,
[28:09] I mean, feel, feel free. Like if it's too weird, just feel free. But, um, it occurs to me that
[28:14] actually the value of things like coding agents goes down. If you have a primitive like this in
[28:19] a way, which is like, you could be like, you know, whatever, some, you know, Codex builds all the
[28:24] software for me, but it doesn't actually use Jev. And so like the software itself, it creates a
[28:28] limited, or you can be like, okay, I, as a human being, I will write the software without using a
[28:32] coding agent, but I've got this very generalized primitive that makes running software easier. So
[28:36] like, do you feel like see a future where it's like the coding agents using Jev, and then you're
[28:41] telling the coding agents and then do you have like redundancy or do you feel it's like humans?
[28:46] This is more of like a coding agent question than it is a Jev question. Oh yeah, for sure. Yeah. My
[28:50] vibe is that, um, I'm not in the coding mines as much as I'd like to be. So you are, you two might
[28:57] be in there more than I am. Which is sad, but my experience is that they
[29:03] are really good at syntax and really the better semantics. Um, I would say incredibly bad at
[29:11] architecture. Yeah. Um, so like to me, architecture is like the most human creative part of software.
[29:16] So I love using coding agents. I think that, uh, Jev is almost certainly not in distribution.
[29:22] That would be spooky if they trained in our user data. Um, so it's probably not. Um, but,
[29:28] uh, I think that when it is in distribution, I, I see no problem with like having it to do the syntax.
[29:33] And the thing with architecture is that maybe the models are actually like not just crap at
[29:39] architecture, but maybe they're 50th percentile architecture. And if you don't know anything about
[29:42] architecture, it would be fine. So these are all like gray area trade-offs in order for you to
[29:47] navigate. And sometimes speed is the, the knob for your company or project to turn. Like you're
[29:52] willing to do a 50th, like a 50th percentile architecture instead of a 60th, because you want
[29:57] to move faster and have like Codex work overnight or something like that.
[30:00] Right. Right. Yeah. Actually kind of along those lines, one of the interesting things
[30:04] or phenomenons in the market already is that, you know, when the coding agents came out,
[30:09] it was the SaaS apocalypse and all their values dropped through the floor. And then when Jev came
[30:14] out, every SaaS company is like, this is the greatest thing ever. So explain that.

### [30:21] SaaS apocalypse, reversed

[30:21] I, I don't know what else to say, right? Like, I, I think it's like quite natural, like in
[30:27] the SaaS apocalypse story. The, the story that I feel like has panned out really poorly is that
[30:34] software is very cheap and perhaps easy to replicate, which I think I could, I could
[30:40] believe the former. I could not believe the latter because a lot of the stuff happens beneath the
[30:44] hood. I'm maybe overly a software fan boy here. Yeah. All of us. Yeah. Okay. Okay. Okay. I didn't
[30:51] know where you might be the coding agent. We have a lot of legacy around that. Yeah. Yeah. Yeah. Yeah.
[30:56] So I don't think that really panned out. So SaaS seems like maybe the markets don't agree,
[31:02] but like, I think SaaS is providing the same value it used to. Maybe the markets are just scared.
[31:07] But I think that SaaS will be one of the largest winners of like the whole AI game. And I want to
[31:14] like work really, really well with like all the biggest, most boring, most like in the know of
[31:19] user problem, SaaS companies, because I think that they are the best position to know what workflows
[31:26] can automate. What do people need? Like that's what their bread and butter is. And to spend the
[31:30] big, like, you know, software is always a CapEx investment, but like you spend it ahead of time
[31:35] in order to make this experience even better that gets, you know, like distributed to all of that
[31:40] massive users. Right. So I think that it's going to be, I'm not going to forecast anything about
[31:45] the financial markets, but I think for as far as like a capabilities games goes, it's going to be
[31:50] like an inverse SaaSpocalypse. And I am so jazzed about it. I should make a name.
[31:55] Yeah. I got that. Yeah. That, that, that, that should have a name.
[31:57] SaaSapalooza. Oh, that sounds a little too fun.
[32:03] Well, all the SaaS applications are going to all of a sudden get like dramatically more useful. And
[32:08] by the way, you know, the, the, the kind of capital investment, like so much of a, of a SaaS company's
[32:15] capital investment is like actually getting to all the customers. And so if you've gotten to all the
[32:19] customers and then you make, you know, not just put a chat bot on your SaaS product, but actually
[32:25] make the software like way, way better. That's a hell of a thing.
[32:29] I don't know if this is a realistic dream or not, but I think that there's a world where like the
[32:36] multi choice choice forms just disappear. You know, like I, I feel like they are like, they are
[32:42] always like something mapping natural language that usually the software already has into like
[32:47] a Jev like output. And I think.
[32:49] It's literally, it's literally from the eighties. It's like, it's called, we used to call it 4GLs.
[32:54] Do you remember fourth generation language?
[32:56] Yeah. Actually also, I think this is from the eighties. This might be an insult. I wasn't born
[33:01] then. Um, like I think that do what I mean is going to be like, be taken to the absolute next
[33:07] level. If I could shout out one Jev application, I don't know if it's reliable, so I can't promise
[33:12] anything, but it was so freaking cool. Someone was using like a voice to control your computer and it
[33:19] was basically
[33:19] constantly making decisions on like, is this a command or is it inserting text? Where's
[33:23] inserting text? Like, like that sounds so unbelievably cool. Like, like I feel like
[33:29] interfaces could like just completely change and maybe we're going to have to make it like cheaper
[33:33] and faster.
[33:33] Yeah. Then you're at Star Trek. Well, you know, there's just such a profound intuition here,
[33:41] which is, um, um, you know, if, if you use AI today to generate software, right. You're still creating
[33:49] the, the same software that you did before. But if you actually look at like the average PR for a
[33:53] large company, it's like 10 lines, right? Seriously. No, we actually did the study. So it's like 10
[33:59] lines. So like you're automating 10 lines. And by the way, those 10 lines are, are like, you know,
[34:03] part of a learning from a customer or something. So like you're, you've kind of optimized something
[34:07] that's actually pretty minimal. Um, but what it doesn't do is provide new capabilities to
[34:11] the software, right? It's kind of automating this thing, which in the limit ends up being
[34:15] relatively minor. And now like there's actually a new capability. And so, you know, it's, you know,
[34:18] so like, it could just be the case that just software just actually gets better. Yep. And,
[34:22] and by the, it didn't even before Jev, like just, it didn't even even occur to me that like,
[34:26] it doesn't matter how much, you know, AI coding agents you use, the software actually isn't
[34:31] getting better. Maybe you're writing it faster. It's like, aren't you really getting worse just
[34:35] because like there's less oversight. So I think this is well, and, and often in more insecure.
[34:39] Yeah, for sure. For sure. But you're, but you're actually now can make an argument. Like,
[34:43] like, like, like apps will have new functionalities as a result of this. Cause there is this new
[34:48] primitive that you're providing that, I mean, like in, in a way, like it speaks natural languages
[34:52] and it can reason, but it marries that to a state machine. I, if people take that as a takeaway,

### [34:58] New capabilities, not more code

[35:00] that would be like the greatest compliment ever to what we are doing. Like, I actually feel like
[35:04] it's almost two grand of a vision to expand beyond the three logic gates that we have into like,
[35:11] you know, our types are kind of like one of the same logic, but like one that's like a little
[35:16] brain in there. Yeah. Like that would be the
[35:18] greatest compliment to like the, the TypeSafe legacy. Cause like that is, that's a very huge,
[35:23] non-trivial huge thing for the world. Um, I'm not gonna like over promise under deliver that,
[35:29] but I will fight for that. Yeah. I mean, listen to that. I mean, there's, I think pretty open
[35:33] questions to what, like how deep can this get as far as like, like really serious stuff,
[35:37] like state consistency or durability or like real systems level stuff where you actually need to
[35:42] like provide strong guarantees. And so a hundred percent, this will change things like whatever,
[35:47] like analyzing logs, analyzing emails, providing a UI talking to the human, like that for sure. But
[35:52] like, you know, you could argue that over time, this becomes like a smart database, like, you know,
[35:56] and also air traffic control system, which we really need a little scary. Like I think automate
[36:04] the easy work before the hard work is always my philosophy. But I also think there's going to be
[36:09] like an entire era of probabilistic programming. That's opened up like my, by the way, you know,
[36:15] there's a huge history of problems. Oh,
[36:16] I basically died in like the 70s. I'm familiar with it. I, I actually think it's going to be
[36:22] like with the same, like you could also call Jev like, so your, your co-founder Eric came from that
[36:28] background. He was telling me, oh, cool. Oh yes, yes, yes. He did a lot of biology that goes up
[36:34] and down. Yeah. But like what I mean is a more, I'm not a fan. My brand is pragmatism, incredible
[36:42] pragmatism. I'm not a fan of like biologically inspired stuff. Um,
[36:46] at all ever worked. Have you ever noticed that? I mean, I, I think it's never worked. It's useful
[36:52] to motivate crazy people to work on things for decades until it works. And then they refine it
[36:57] into like the engineering, the story of AI neural nets for sure. Yes. Yeah. But like,
[37:02] you know, a lot of the stories about how it worked were not accurate. Right. So like the hierarchical
[37:07] features of applications really did end up working because like otherwise ResNets wouldn't have
[37:11] worked longer story. Yeah. Um, I, I do think that it opens up like, like from a systems,
[37:16] perspective, I'm not excited about this part cuz it's really, I'm excited for the world. Not about
[37:20] me programming this cuz it sounds like really complicated. Yeah. But I think that as we have
[37:24] like lots of intelligence at lots of, uh, like different cost and speed trade-offs, the super
[37:30] systems types will be making trade-offs at like, you know, like Jev's gonna be like a thousand
[37:35] times too smart for them. They just want like an approximate link to have an approximate guest to
[37:39] like optimistically route here and there. It's gonna be like so crazy. The, this type of stuff
[37:44] Right. That's available in the extreme
[37:46] systems. And and the good news is we get to like rebuild systems again, which is great. Right. We
[37:50] have a new, no, seriously, we have a new primitive. It's kind of a new way of thinking about doing
[37:53] software. Like, I mean, we did this and we did this for the internet. You know, we did this main
[37:57] frame to client server. I mean, we do this periodically. And, and by the way, just because
[38:02] of the, um, cybersecurity issues, we probably have to rebuild almost all the systems, uh, to,
[38:09] to just make them safe. I, I would think, I think it's pretty clear that they're like, there's not,
[38:16] at least the critical, the critical infrastructure for sure. Yeah. Yeah. Yeah. Do you, do you think
[38:21] about this more in terms of like apps, SaaS analytics, or more in terms of like systems,

### [38:27] Apps vs the guts of systems

[38:30] foundations or all the above for what I would think of, or how just general application for
[38:35] this. And when you think about like, like you're working on Jev and like, and you kind of envision
[38:39] that people are adapting it, like how, you know, like do you do, oh, maybe do you even have an
[38:44] opinion? I have a little bit,
[38:45] and it's so the way I think of it is a little like, uh, like deep into like the TCP guts,
[38:54] you know, like UDP TCP, you know, like it's unreliable, too reliable. Exactly. And like,
[38:59] I, I, so like when I think of AI, yeah, I mean like when I think of AI and this is why I care
[39:04] about intelligence per dollar, to be clear, when I think, and how I got to this conclusion, I work
[39:09] backwards from AI based economic revolution, AI everywhere, no sci-fi and everything, like all the
[39:15] over the place. And I ask myself the question, what percentage of the calls to AI, I imagine it's
[39:21] like a function, which, uh, what percentage are like for human consumption where you need that
[39:25] style and yeah. And it's going to be like many nines. And actually from that same question,
[39:30] how many will be at the first layer versus like deep in the guts. Right. And I think that it's
[39:35] going to be many nines in the guts, but it will start at the first layer. But like, we need to,
[39:38] if you don't aim for the guts, you don't aim for the guts. You it's, it's going to take you a while
[39:45] to get there. Right. I think people don't understand to what extent, like AI was kind
[39:49] of ships in the night with software. Like even if you try to embed AI in software, it kind of like
[39:53] didn't behave right. Because software doesn't really take natural languages and you like do
[39:57] all this weird stuff. Like you stick at the prompt, like here's the JSON output that you want and
[40:02] here's a schema and it would never listen to it. And so, and so what you ended up doing is just
[40:06] taking the output and giving it to a human. You're like the hell with it. Right. Or another LLM that
[40:09] is what a while loop is like the agent while loop. Right. So it's like, it's like from first
[40:12] principles, it needs to be human in the loop, which
[40:15] with chat or an agent, which is the while loop because like the natural language needs to be
[40:19] fed back into another. And I will say, I, I have watched this. There was almost like this kind of
[40:24] like five stages of grief, like, you know, people will pick up AI and like, I'm going to use this,
[40:29] you know, within my software. Right. And then, you know, and then it would like go with like,
[40:32] you know, whatever denial, like try to make it work and like anger that they go to acceptance,
[40:37] which is like, okay, nevermind. I'm just going to give this to another LLM to a human being. So
[40:41] it's been very ships in the night. And I think this is the first time I have seen when
[40:44] it's almost like, actually you can take an LLM, you can take AI and you can actually map it to
[40:49] like, like a state machine and you can do that productively. And I hope so. I will not want to
[40:56] over-promise under deliver as well. Like, I don't know if it's ready for all the applications that
[41:00] have been over-promised. I really, really want it to. And my team will fight for that. Obviously,
[41:06] like we really, really care about reliability. We could have released so much sooner. I don't think

### [41:08] Reliability and "do what I mean"

[41:14] Yeah.
[41:14] realize that. And I don't think that honestly, I don't think that they will based on what I see
[41:18] at the Twitter discussion. I think it's people will never get it, but like, it'll just have like
[41:23] that good vibe of like, oh, I can trust this. So it's the anti-frustration machine.
[41:28] It's it's a, I hope so. I hope do what I mean. Right. Like to me that is about like smoothness
[41:37] in the world, like having everything that just move more smoothly together and interlink like
[41:42] a gears. Yeah. I actually have my whole, like,
[41:44] AI utopia on like different axes that I really, really want. And like, do what I mean is a huge
[41:49] part of this. You know, like imagine if all technology just did what you mean that is like,
[41:55] that's not sci-fi look how smart AI is. Right. Yeah. No, it's a, it's amazing. And,
[42:00] and maybe that's the thought to close on. Oh, do what I mean. Yeah. I love it. Thank you,
[42:06] Diogo. This has been a great conversation. Hell yeah. Really enjoyed it.
[42:14] Thank you.
