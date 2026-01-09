https://www.youtube.com/watch?v=I77yLzAGmnc&t=1s

**00:00**
**[Applause]**

**Omar Khattab:**
Thanks everyone for coming and thanks Amplify for hosting us and [partners] for organizing. I want to start with a confession. I assumed that I... so I made an assumption, and it’s not correct. I assumed I was going to go first to set up things for the much more exciting talks after me.

So what I'm going to do right now is try to contextualize the content they discussed in the broader idea of what DSPy is about. And I used the slides that were meant to introduce some of this stuff, but what I will do is actually try to reflect, like philosophically, why does DSPy exist? And why do we think DSPy made sense when we started it in 2022? Why do we think it makes sense in 2025? And why, AGI or not, we think it makes sense in 2030? And we think like this is something essentially fundamental here.

And the two previous talks were incredibly sort of deep and thorough on prompt optimization. But something that I like to repeat, partially due to the surprise of people because it helps drive something home: DSPy is actually not definitionally about prompt optimization at all. We are in fact, and we have been as Chris discussed, leading sort of the space of In-Context Learning and prompt optimization for a few years. But that's just one of the things that implements the larger paradigm, which is declarative self-improvement or declarative AI programming. So let me tell you about that.

**01:23**
All of this work, and everything that Chris and Lakshya discussed, is—as they said—worked on by a lot of amazing people, many of whom are in the room. We have Isaac Miller going next as well. We have a lot of other folks here that you probably recognize. So let me... let me just jump right in and give you a little bit of context here.

**01:45**
So the context of DSPy and of this talk is: I think something super remarkable has happened over the past like, you know, three to eight years. Which is, we seem to have figured out how to train incredibly general and remarkably powerful AI assistants. And that's no small feat.

But something that is taking a lot of people a little bit of time to realize is: as amazing as these models are, it's actually something of a... of a... of a cheat that sort of makes them work as well as they do. Which is: they are interacting directly with you. They are virtually "human-in-the-loop" for every single interaction. Which means that when they make mistakes, or when they seem to be a little bit inconsistent, or when they don't exactly adhere to your requirements—you're sitting right there, sort of from an HCI angle, ready to correct them or to slightly steer them here and there. And you know, as a human, you're incredibly flexible and versatile anyway, so you're able to sort of smooth over a lot of what they do.

**02:43**
But what remains still hard—and the reason like we're doing all of this—is that we as a field are still fundamentally guessing how to engineer reliable AI *software*. And the thing about AI software is that now you have this really powerful and sort of super unpredictable language model inside a program. It's not talking to one of us; it's talking to dumb Python variables and like, control flow. And those things are much less versatile than we are.

And in fact, they are this way for a reason. They are this way because we want to be able to reason about the programs. We want to be able to say, "Oh yeah, it's doing the thing semantically it's supposed to achieve." We're supposed to be able to say, "This module and that module do not pass information with each other because that would be a privacy concern," right? So there is a reason that software is structured as it is, and that means though that building these types of AI software systems is really difficult still.

**03:38**
This is—as Chris's talk sort of alluded to—not something that improvements to language models is going to solve anytime soon. And the reason for this is maybe more apparent if we reflect for a second on how these models exist. These models are not engineered. We do not, in fact, understand exactly how they do what they do. What happens is: we figured out a recipe—much like cooking or maybe alchemy—where basically if we prepare data of enough scale, and we iterate on it enough times, and cover sort of all of the obvious cases, and we sort of run it with a whole lot of gradient descent in various different algorithms and objectives and shapes... you get models that, sort of when you put them under audit and you test them for a while, seem to be incredibly good and impressive. And we can sort of iterate from there.

**04:26**
But for the same reason that we get this amazing generalization—where the models extrapolate and interpolate between the points they see—for that exact same reason, we do not know precisely what they will do the farther out you go from these planes and sort of in this vastly high-dimensional space. And for that reason, you get models that are better and better and better all the time, and yet they seem to be susceptible to all kinds of like weird sensitivities. Sometimes very weird prompting tricks make a lot of difference, but other times their capabilities—as high as they go—the models just remain wildly inconsistent in other ways.

**05:05**
Why is that a problem? Well, if you're just chatting with the model and the companies start to treat their models as systems, they can sort of iterate over these sort of regions and try to essentially smooth out a lot of that behavior.

But when your goal is to take a model *now* and build a product or build an artifact or some kind of system, you're always trying to extract the *best possible* tradeoffs—the best quality, the best consistency, the best efficiency—out of *today's* generation of models.

And what that invites in practice... it invites hacks. A few years ago, those hacks were, you know, people sort of introducing funny strings into the prompts. But nowadays, it's people sort of prioritizing which things they repeat in the prompts, or sort of which examples they select for their models, or how exactly they break their system into multiple agents, or any other number of sort of ephemeral decisions that you *know* the next time there is a better generation of models, all of these things are going to be counterproductive.

**06:04**
Now this phenomenon is just one example of what is a much broader idea in AI, which is the idea of **The Bitter Lesson**. If folks are not aware of this, this is sort of an essay by Rich Sutton, the pioneer of Reinforcement Learning, who is the current year's Turing Award winner. Where he basically says that a lesson from—maybe the biggest lesson from—decades of doing AI is that we're really bad at engineering *around* AI.

Whenever we pick a specific problem and we start to encode our knowledge into things for our agents and our systems, what happens is: you do see local improvement. The engineers and the researchers do actually see a bunch of return that helps them celebrate on benchmarks or tasks that they're solving. But you give it a few more years, and hardware improves, and algorithms improve, and the sort of search spaces we can explore with gradient descent or with discrete search improves... and you find that simpler and more general approaches that rely on scale are able to sort of invalidate or wash over a lot of the hand-engineered tricks.

**07:07**
And you might think about this as something that's specific to AI or that's fairly peculiar. But I actually sort of implore people to realize that this is nothing but Donald Knuth's 1974: **"Premature optimization is the root of all evil."**

And it's sort of this idea that, you know, when engineers sort of misprioritize where to spend their time, and they sort of micro-optimize and micro-hand-engineer, we end up sort of making bad tradeoffs. Either in terms of productivity of the developers, or of maintainability or portability of the systems, or simply doing things that do not make sense given the rate of progress in computer science in general and in AI in particular.

**07:45**
So, what is DSPy about? DSPy to me is not about prompt optimization, although this is one of our best arms in solving the problem. DSPy is fundamentally about: How do we take AI software and make it an actual field? I think there's very few people outside the DSPy ecosystem and its many derivatives that actually think and take AI software as a *serious* thing. This is not something—you know, some wrapper around the model. We're actually thinking that this is a discipline, and an engineering discipline, and one where we really want to be systematic.

You can go back to the original DSPy papers and the question is not "How do we get the best quality out of today's models?" The question is "How do we turn this into a systematic discipline?"

**08:26**
And the answer starts from: well actually, in order for AI software to be a systematic discipline, the first thing you do is you actually say "What is the AI system in what you're doing?" The AI system is not about GPT-5. It's not about `text-davinci-001` or `002` when we started.

And you know, sort of like the first instantiation of DSPy was a homework assignment in the NLP class at Stanford, in this very course, in April of 2022. So this is before you had GPT-3.5. You had models that barely knew how to add up two numbers. But the core of the paradigm and the core insight is: actually inside *in* each of these systems is some kind of **AI Specification**. And that AI specification should be decoupled from all of the machine learning techniques that we are ourselves inventing and other people are inventing.

**09:09**
But by decoupling the AI from the Machine Learning, you're able to get people to express their systems—much like how we express things in a programming language—and you're able to get the graduate students, the researchers, the other engineers to build a lot of machine learning techniques. And we can then compose these seamless things.

If you have a better algorithm—you have GEPA [Genetic Prompt optimization] now, we had MIPRO before, we had BootstrapFewShot with random search before, we had SIMBA, we have a lot of these methods. There is GRPO with Reinforcement Learning. We had earlier sort of methods for offline Reinforcement Learning. These compose seamlessly over your programs. You get new programs and new applications, you can sort of run them with whatever the latest methods and algorithms are.

**09:50**
Alright. So I have a snippet of code here. And I don't expect you to actually even see it, but it's a bit of C code that's very messy. And what I normally like to ask when I use this slide is like... Unless you've seen me use this slide before, or unless you're like, I don't know, deeply a nerd in that part of programming, it's a piece of code that's just doing a bunch of bit shifting and most people have no idea what this is doing.

**10:19**
So this is kind of a snippet of code from *Quake*, like a late 1990s game, where in order to implement something in their inner loops—which is a fast inverse square root—they figured out sort of a bunch of tricks where, for the floating point representation you're working with, shifting and adding and doing a bunch of these operations in a certain way gives you an approximation of an inverse square root.

And I would like folks to think about this. If you open a piece of code and they don't have *one* function of that—they have *a lot* of functions, and *all* their functions look like this—and maybe you cannot actually make out what the name of the function is, it's just a whole bunch of this stuff... Would you say that these people are engineering good systems?

Like maybe if there's like one part of the system that is super like "core" and nobody else is doing it well, maybe you get an excuse there, especially if it's well documented. But if the whole system is built like this, you can bet that's a team whose work is going to collapse. That's not a team that can iterate fast. Maybe the code is fast *today*, but I don't know if it's correct, and I don't know if they are going to be able to sort of stay in the game.

**11:21**
So the question is, what did we do? Because most of us are not writing code that looks like this. What we did though is we built **higher-level languages** that allow people to think at the level of abstraction closer to the problem domain. And then we give them, as researchers and as engineers, much faster compilers and much faster runtimes so they do not have to do a lot of this stuff by hand.

Notice that the argument here is not—as actually turns out to be true in practice—but notice that the argument here is not "If you use higher-level languages and compilers your code will be faster." Although that's what we see in practice due to things like the Bitter Lesson. But the argument is: You are an engineer. There are important things you need to be thinking about. Please do not do bit shifts unless you demonstrated that this is actually the bottleneck where we should be investing your time in.

**12:05**
So the answer to AI programming is exactly the same. Let's elevate ourselves above the level of hand-engineered assembly and bit shifts to the level of **structured programming**. Let's rediscover the 70s and 80s and 90s of programming, but this time with **natural language specification** as a first-class component and primitive in our programs.

**12:26**
So this is the idea of not hand-engineering long prompts that are iteratively evolved with a specific model in mind—because the model will change—but instead, declarative **Signatures** which structure your intent and actually allow you to express ambiguous natural language specifications, but sort of localize it so that as much structure that can be expressed formally and symbolically in code is expressed formally and symbolically in code.

**13:00**
So instead of a very long and messy prompt, we break down the task so that the task that is supposed to be executed is clearly specified, and any requirements are clearly isolated from the other components. So that the inputs to your prompt are not just some f-strings, but they are actually arguments that have **types** and that have descriptions that can sort of stand on their own feet. And that the outputs are also types that can be understood independently of—and in conjunction with—the other parts.

**13:33**
So **Signatures** is sort of like the core abstraction of DSPy. DSPy has fundamentally a single abstraction: it's just Signatures. And everything else falls out of that. If you understand what Signatures are, and how they localize ambiguity in a larger program, and how they open up the space after that to composition in a larger space of sort of functions that operate on Signatures—functions that operate on *intent*, essentially—you can sort of see how Signatures allow us to approach the stage of making AI engineering actually an engineering field.

Because now we can think about how we **abstract the details of inference**: You know, should I use Chain of Thought? Should my model be an agent? Should it do Monte Carlo Tree Search? I don't know—that's not the decision I'm trying to make. I'm trying to build an application for, I don't know, like "chat with your PDF" or whatever it is that people do.

It abstracts away the **learning**. So if I want to try Reinforcement Learning, that Signature doesn't say I can't do that. That Signature actually captures my task. And now the question is "How do I turn that into a model with Reinforcement Learning?" I want to use something like GEPA, genetic approaches for evolving the prompt? Well, great, because the Signature captures what the prompt is supposed to do, and now the prompt can be generated according to this specification.

**14:41**
**Plumbing**: You know, is XML the right way to use your model? There's this thing called... what is it called, like "Tun" or something that came out a few days ago? Or maybe it's YAML? Or maybe it's CSVs? Or what, I don't know. But that's clearly not part of my task. That's some kind of lower-level optimization that should sort of be composable on top of my system.

**15:03**
So what do we actually do in practice? What we do in practice in DSPy is we give you the tools to take your Signatures inside a larger program, and then we do any number of the things that Lakshya described today. We allow you to evolve the prompts. We can actually reinforce the weights—we've always actually been able to do this ever since February of 2023. And we also can optimize the pipeline. So I'll just give you a few highlights of those really quickly.

**15:29**
I will not discuss GEPA because, you know, I cannot do as good of a job as Lakshya did. I'm also gonna skip this one [MIPROv2] because I wanted to tell you that prompt optimizers can be composed sort of with RL in DSPy and then the two are better together—but actually *both* Chris and Lakshya told you about that, so I'll move faster here. Although I want to highlight that the tool that enables this is the team's **Arbor** library, which is sort of this integration with DSPy that handles all of the fickleness of Reinforcement Learning so that you basically take your exact same DSPy program—that's why it's declarative—is that it composes with different techniques, and simply run Reinforcement Learning on top.

**16:09**
The one thing I will discuss really briefly is the following. So far we've thought about language models as a bit of a black box, right? It's just this thing that we pass this context to and that we maybe fine-tune with Reinforcement Learning. But today's language models—and for the foreseeable future that might be the case as well—are **Transformers**. And there is actually an increasing, growing literature that shows that Transformers are actually a very limited type of object. Maybe because they are really highly parallelizable.

For example, we are all familiar with the **Context Rot** phenomenon. And there is actually kind of theoretical reasons to believe that a lot of that is going to be very difficult to remove from the models. Like, we can increase their context length, but at some point—and fairly consistently—they sort of just seem to struggle for difficult tasks if the context is long enough.

**17:00**
But that kind of causes a problem for the vision that we described: a vision of declarative modules. You want to be able to show up to your system and actually break it down in accordance to your engineering sort of design. And your engineering design might have you in some cases design modules that *want* to accept very long input, or that do very long processing, or that aim to produce very long output.

And the problem there is: in practice, at any given model generation, if the task is large enough, maybe quality is best if you actually decompose or break down the task into multiple calls. You're not breaking things down here because your specification calls for it; you're breaking things down because your model is just struggling.

**17:46**
So what do we do? And this is like... well, for the longest time we told you like, "Well, don't mess with the weights, we can handle that. Don't mess with the prompts, we can handle that." But the answer for this one was like, "Yeah... you gotta do this one, sorry."

So we're starting to think about that. And one answer is evolving the prompt itself like Lakshya described. But maybe a slightly more general answer is what we think could be a **new paradigm for inference scaling** in general. Which is: what if we actually go to the language model and say, "Hey, you're very good at code generation. And you know what kind of code we really like to write that's very powerful? It's language model programs. It's essentially DSPy programs."

**18:23**
So what if we come to the model and we say: "Actually, I want to put you in a setting where you can generate code, and that code can actually be language model programs. One of your tools can be to call a language model."

Well, if you just do that, the problem is the prompt is still very long. And so like the model is still failing at reading that whole prompt. So you might say, "Well, okay, maybe in order to make this happen... let's actually take that whole prompt and put it in a variable." If you put it in a variable in Python and you tell the model, "Hey, there's a variable named, it's called X, and it includes the prompt and it has the following number of tokens." Well, now the model is not gonna struggle to sort of process the task; it just cannot *see* what's in there.

**19:03**
So maybe what we can do is we go to the model and we say, "Hey, you are in a Jupyter Notebook. You are in a persistent REPL environment. You can execute code, see what happens, and then write more code and see what happens." And you can kind of persist that state as you go.

So this is exactly the paradigm sort of developed by my student at MIT, Alex Tan, which is called **Recursive Language Models**. It's incredibly simple. What we do is we basically throw long prompts into a Python variable. We put the agent essentially in a Jupyter Notebook. And we tell it, "Hey, please poke around. You can look at the first few tokens, you can look at the last few tokens. And once you understand the structure enough, you should start writing code." And when you write that code, one of the tools you have is you can actually pass context to a language model.

**19:48**
And what you see in practice is that the models are very good at discovering the structure inside these long strings, launching parallel jobs—or sort of like MapReduce style jobs where like they call smaller models on small pieces—and they say, "Hey, I'm trying to poke through this and understand what's in there. Could you summarize for me what's happening?" and sort of iterate based on the intermediate outputs they see in order to decompose very large problems.

**20:10**
And the interesting thing is: even before we bring our big tools like GEPA or GRPO to this kind of thing, we're able to see that the models are surprisingly good at this. It's one of the sort of reasons that we say, "Well, as the models get better, it can actually be harder to figure out the best ways to use them."

So what I have here is a plot [OOLONG] that shows on a really difficult task with 130,000 context length, that if you just call GPT-5 and you sort of try to prompt it to do this task, the original paper as well as our own experiments show that it's only able to handle around 30% of the instances. But if you go to **GPT-5-mini**—a far smaller model and a far cheaper model, maybe I think 20 times cheaper, I can't remember the exact number—it is more than twice as good if you put it in a recursive setting. And it's actually still cheaper in the recursive setting than using GPT-5 with a single call. It's also better than calling GPT-5 in a React agent.

**21:05**
And this structure... although here it's able to improve sort of the problem of Context Rot—so cases where the context *does* fit in the technical context window but the model is not performing very well—it is also the case that which allows us to scale to far longer contexts than the actual sort of advertised window of the model. So here to 10 million plus tokens.

**21:28**
So let me wrap up. Thanks to the contributions of a lot of the folks in this room and many others, the **DSPy community** is regularly putting out dozens of new applications every month. Someone actually from the community started a "DSPy Weekly" newsletter. I subscribed, maybe you also want to. And you can kind of track cool new things that he wants to highlight every week.

I have a number of these here that just kind of highlight a bunch of stuff. Like a Romanian hospital that built a sort of optimized DSPy program for their doctors to better communicate with the telemedicine patients, and then they actually found very large gains. Reports from Meta in using prompt optimization for their Llama models. Reports from Amazon AWS in their Nova optimizer sort of suite. Stuff from Databricks—people winning machine learning competitions—and many other types of things including many others that are not here that Lakshya highlighted and others highlighted.

**22:29**
But thanks to the work of this huge community, we're able to see what, you know... I'm thinking, I'm laughing because I'm thinking here of sort of the scaling laws of the models. We're able to see on a log scale what could sort of be conceived of as an **exponential growth** in the number of monthly downloads of DSPy—if that's a metric somebody cares about. And so like right now we're at **3 million downloads per month**. But you could sort of see a lot of the earlier stages sort of at like a few hundreds and few thousands for the first half of 2023.

So that's thanks to you guys and to the many other folks sort of in the wider community. So I just wanted to kind of like contextualize the prompt optimization discussion and to sort of bring back like the essence of prompts in DSPy.

The most fundamental role of prompts in DSPy is that we have this notion of **Signatures**. And Signatures is our attempt at sort of answering the question of: "How do we bring English, or other natural languages, inside structured programs?" And the answer is: actually you should write in English what can only—or can be best—expressed in English, but in a structured way.

And when you do that, what you actually are writing are **human-facing prompts**. Or developer-facing prompts. Or "what prompts wanted to be when they grow up." And the association with prompt optimization is: we can take these beautiful, organized, self-documenting human-facing prompts and turn them into whatever ugly mess or whatever highly structured intricate set of requirements—that maybe you did not think about because maybe they are very model-specific, or environment-specific, or dataset-specific, or composition-specific where you have multiple modules connected together—and turn them into *that* through the search and learning methods that scale, that allow you in your system to counteract the effects of the Bitter Lesson or the fact that premature optimization is maybe the... the square root, I guess... well, of all evil.

Alright, thanks everyone.

**24:30**
**[Applause]**
