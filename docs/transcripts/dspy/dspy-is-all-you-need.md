Here is the transcript for the presentation **"DSPy is (really) All You Need"** by Kevin Madura at the AI Engineer Code Summit.

**00:00**
**[Music and Event Graphics]**

**00:21**
**Kevin Madura:**
Thanks everybody for uh, for joining. I'm here to talk to you today about DSPy. Um, and feel free to jump in with questions or anything throughout the talk. It's, you know, I don't plan on spending the full hour and a half or so. I know it's the last session of the day. So, um, keep it casual. Feel free to jump in.

I'll start with a little bit of background. Don't want to go through too many slides. I'm technically a consultant, so I have to do some slides. But we will dive into the code for the latter half. And there's a GitHub repo that you can download to follow along and play around with it on your own.

**00:56**
Um, so how many people here have heard of DSPy? Almost everyone. That's awesome. How many people have actually used it kind of day-to-day in production or anything like that? Three. Okay, good. So hopefully we can convert some more of you today.

**01:15**
Um, so high level, DSPy—this is straight from the website—um, it's a declarative framework for how you can build modular software. And most important, for someone like myself... I'm not necessarily an engineer that is writing code all day, every day. As I mentioned before, I'm a more of a technical consultant, so I run across a variety of different problems. Could be an investigation for a law firm. It could be helping a company understand how to improve their processes, how to deploy AI internally. Maybe we need to look through 10,000 contracts to identify a particular clause, um, or paragraph.

**01:50**
And so DSPy has been a really nice way for me personally, and my team, to iterate really, really quickly on building these applications. Most importantly, building *programs*. It's not... it's not kind of iterating with prompts and tweaking things back and forth. It is building a proper Python program. And DSPy is a really good way for you to do that.

**02:13**
So, I mentioned before there's a repo online. If you want to download it now and kind of just get everything set up, I'll put this on the screen later on. Um, but if you want to go here, just kind of download some of the code. It, uh, it's been put together over the past couple of days, so it's not going to be perfect production-level code. It's much more of utilities and little things here and there to just kind of demonstrate the usefulness, demonstrate the point of what we're talking about today.

In that, and we'll walk through all of these different use cases. So, um, sentiment classifier, going through a PDF, some multimodal work, uh, a very, very simple web research agent, detecting boundaries of a PDF document. You'll see how to summarize basically arbitrary length text and then go into an optimizer, uh, with GEPA.

**03:00**
But before I do that, just again, kind of level set. The biggest thing for me personally, DSPy is a really nice way to decompose your logic into a *program* that treats LLMs as a first-class citizen. So at the end of the day, you're fundamentally just calling a function that under the hood just happens to be an LLM. And DSPy gives you a really nice, intuitive, easy way to do that with some guarantees about the input and output types.

**03:28**
So of course there are structured outputs. Of course there are other ways to do this—Pydantic and others. Um, but DSPy has a set of primitives that when you put it all together, allows you to build a cohesive, modular piece of software that you then happen to be able to optimize. And we'll get into that, uh, in a minute.

**03:51**
So, just a few reasons of why I'm such an advocate. It sits at this really nice level of abstraction. So it's... I would say it doesn't get in your way as much as a LangChain, and that's not a knock on LangChain, it's just a different kind of paradigm in the way that DSPy is structured. Um, and allows you to focus on things that actually matter. So you're not writing `.choices[0].messages.content`. You're not doing string parsing. You're not doing a bunch of stuff under the hood. You're just declaring your intent of how you want the program to operate, what you want your inputs and outputs to be.

**04:26**
Because of this, it allows you to create computer programs, as I mentioned before, not just tweaking strings and sending them back and forth. You are building a program first. It just happens to also use LLMs. And really the most kind of important part of this—and Omar Khattab, the founder of this, or the original developer of it—had this really good podcast with a16z, I think it came out just like two or three days ago. But he put it a really nice way. He said it's built with a *systems mindset*. And it's really about how you're encoding or expressing your intent of what you want to do.

**05:03**
Most importantly, in a way that's *transferable*. So the design of your system, I would imagine, or your program, isn't going to move necessarily as quickly as maybe the model capabilities are under the hood. Where we see new releases almost every single day, different capabilities, better models. And so DSPy allows you to structure it in a way that retains the control flow, retains the intent of your system, your program, um, while allowing you to bounce from model to model to the extent that you want to or need to.

**05:36**
Convenience comes for free. There's no parsing JSON, things like that. Again, it sits at a nice level of abstraction where you can still understand what's going on under the hood if you want to. You can go in and tweak things. But it allows you to kind of focus on just what you want to do while retaining the level of precision that you... that I think most of us would like to have in kind of building your programs. Um, as mentioned, it's robust to kind of model and paradigm shifts. So you can again, keep the logic of your program, um, but keep those LLMs infused in, uh, basically inline.

**06:07**
Now that being said, you know, there are absolutely other great libraries out there. PydanticAI, LangChain, I mean there's many, many others that allow you to do similar things. Agno is another one. Um, this is just one perspective. And, um, it might not be perfect for your use case.

For me, it took me a little bit to kind of grok *how* DSPy works, and you'll see why that is in a minute. Um, so I would just recommend just kind of have an open mind, play with it. Um, run the code, tweak the code, do whatever you need to do, um, and just see how it might work, might work for you.

And really this talk is more about ways that *I've* found it useful. It's not a dissertation on the ins and outs of every nook and cranny of DSPy. It's more of, you know, I run into these problems myself, now I naturally run to DSPy to solve them. Uh, and this is kind of why. And the hope is that you can extrapolate some of this to your own use cases.

**07:05**
So we'll go through everything uh, fairly quickly here. But the Core Concepts of DSPy really comes down to arguably five or these six that you see on the screen here. So we'll go into each of these in more detail. But high level: **Signatures** specify what you want your function call to do. This is when you specify your inputs, your outputs. Inputs and outputs can both be typed. Um, and you defer the rest of the, basically the *how*, the implementation of it, to the LLM. And we'll see how that all kind of comes together uh, in a minute.

**07:44**
**Modules** themselves are ways to logically structure your program. They're based off of Signatures. So a Module can have one or more Signatures embedded within it, in addition to uh, additional logic. And it's based off of um, PyTorch in terms of the methodology for how it's structured. You'll see how that comes to be in a minute.

**08:07**
**Tools**. We're all familiar with tools. MCP and others. And really Tools fundamentally, as DSPy looks at them, are just Python functions. So it's just a way for you to very easily expose Python functions to the LLM within the DSPy kind of ecosystem, if you will.

**08:27**
**Adapters** live in between your Signature and the LLM call itself. I mean, as we all know, prompts are ultimately just strings of text that are sent to the LLM. Signatures are a way for you to express your intent at a higher level. And so Adapters are the things that sit in between those two. So it's how you translate your inputs and outputs into a format—basically explodes out from your initial Signature into a format that is ultimately the prompt that is sent to the LLM.

And so, you know, there's some debate or some research on if certain models perform better with XML as an example, or BAML, or JSON, or others. And so Adapters give you a nice easy abstraction to basically mix and match those at will, as you want.

**09:20**
**Optimizers** are the most interesting and for whatever reason, the most controversial part of DSPy. It's kind of the first thing that people think of, or at least when they hear of DSPy they think optimizers. We'll see a quote in a minute: It's not optimizers first. It is just a nice added benefit and a nice capability that DSPy offers *in addition* to the ability to structure your program with the Signatures and Modules and everything else.

**09:49**
And **Metrics** are used in tandem with optimizers. That basically defines *how* you measure success in your DSPy program. So the optimizers use the Metrics to determine if it's finding the right path, if you will.

**10:05**
So Signatures, I mentioned before, it's how you express your intent, your declarative intent. Can be super simple strings. And this is the weirdest part for me initially, but is one of the most powerful parts of it now. Or it can be more complicated class-based objects. If you've used Pydantic, that's basically what it runs on under the hood.

**10:25**
So this is an example of one of the class-based Signatures. Again, it's basically just a Pydantic object. What's super interesting about this is that the names of the fields themselves act almost as like mini-prompts. It's part of the prompt itself. And you'll see how this comes to life in a minute. But what's ultimately passed to the model from something like this is... it will say, "Okay, your inputs are going to be a parameter called 'text', and it's based off of the name of that particular parameter in this class."

**11:05**
And so these things are actually passed through. And so it's very important uh, to be able to name your parameters in a way that is intuitive for the model to be able to pick it up. Um, and you can add some additional context or what have you in the description field here. So most of this, if not all of this—yes it is proper, you know, typed Python code—but it's also, it also serves almost as a prompt ultimately that feeds into the model. Um, and that's basically translated through the use of Adapters.

**11:37**
Um, so just to highlight here, like these... the ones that are a little bit darker in bold, those are the things that are effectively part of the prompt uh, that's been sent in. And you'll see kind of how DSPy works with all this and formats it in a way that again, allows you to just worry about *what* you want—constructing your Signature—instead of figuring out how best to word something in the prompt.

**12:00**
**Audience Member:**
Can I just jump in with one thing? Because a lot of people get hung up on like, "But I have a really good prompt." Then I don't want this thing messing it up. For that, you literally just put your really good prompt in the doc string.

**Kevin Madura:**
Yeah. That's exactly right.

**Audience Member:**
A lot of people are averse to DSPy because they think that they're losing control. So it's like, okay, keep your prompt, put it in the doc string.

**Kevin Madura:**
So the question for folks online is: What if I already have a great prompt? I've done all this work. I'm an amazing prompt engineer. I don't want my job to go away or whatever.

Yes. So you can absolutely start with a custom prompt or something that you have demonstrated works really well. And you're exactly right. That can be done in the doc string itself. There's some other methods in order for you to inject basically system instructions or add additional things at certain parts of the ultimate prompt. And/or of course you can just inject it in the final string anyway. I mean, it's just, you know, a string that is constructed by DSPy.

So, um, absolutely. This doesn't necessarily prevent you... it does *not* prevent you from adding in some super prompt that you already have. Absolutely. Um, and to your point, it serves as a nice starting point from which to build the rest of the system.

**13:23**
Here's a shorthand version of the same exact thing. Which to me, the first time I saw this, this was like baffling to me. Um, but that's exactly how it works. Is that you're basically, again, kind of deferring the implementation or the logic or what have you to DSPy and the model to basically figure out what you want to do.

So in this case, if I want a super, super simple sentiment classifier, this is basically all you need. You're just saying, "Okay, I'm going to give you text as an input. I want the sentiment as an integer as the output." Now you probably want to specify some additional instructions to say, "Okay, your sentiment... a lower number means negative, a higher number is more positive sentiment," etc. But it just gives you a nice kind of easy way to scaffold these things out in a way that you don't have to worry about like, you know, creating this whole prompt from hand. It's like, okay, I just want to see how this works. And then if it works, then I can add the additional instructions. Then I can create a module out of it or whatever it might be. It's this shorthand that makes experimentation and iteration incredibly quick.

**14:32**
So Modules. It's the base abstraction layer for DSPy programs. There are a bunch of modules that are built-in. And these are a collection of kind of prompting techniques, if you will. And you can always create your own Module. So to the question before, if you have something that you know works really well, sure, yeah, put it in a Module. That's now kind of the base assumption, the base Module that others can build off of.

And all of DSPy is meant to be composable, optimizable. And when you deconstruct your business logic or whatever you're trying to achieve by using these different primitives, it's intended to kind of fit together and flow together.

**15:13**
Um, and we'll get to Optimizers in a minute. But at least for me and my team's experience, just being able to logically separate the different components of a program, but basically inlining LLM calls, has been incredibly powerful for us. And it's just an added benefit that at the end of the day, because we're just kind of in the DSPy paradigm, we happen to also be able to optimize it at the end of the day.

**15:37**
Uh, so it comes with a bunch of standard ones built-in. I don't use some of these bottom ones as much, although they're super interesting. Um, the base one at the top there is just `dspy.Predict`. That's literally just, you know, an LLM call. That's just a vanilla call.

`ChainOfThought` probably isn't as relevant anymore these days because models have kind of ironed those out. But, um, it is a good example of the types of kind of prompting techniques that can be built into some of these modules. Um, and basically all this does is add, um, some strings from literature to say, "Okay, let's think step by step" or whatever that might be.

**16:21**
Same thing for `ReAct` and `CodeAct`. `ReAct` is basically the way that you expose the tools to the model. So it's wrapping and doing some things under the hood with um, basically taking your Signatures and um, it's injecting the Python functions that you've given it as tools. And basically `ReAct` is how you do tool calling in DSPy.

`ProgramOfThought` is uh, is pretty cool. It kind of forces the model to think in code and then will return the result. Um, and you can give it a... it comes with a Python interpreter built-in, but you can give it some type of custom harness if you wanted to. Um, I haven't played with that one too, too much, but it is super interesting if you have like a highly technical problem or workflow or something like that where you want the model to inject reasoning in code at certain parts of your pipeline. That's a really easy way to do it.

**17:15**
And then some of these other ones are basically just different methodologies for comparing outputs or running things in parallel.

**17:21**
So here's what one looks like. Again, it's fairly simple. It's, you know, it is a Python class at the end of the day. Um, and so you do some initial initialization up top. In this case, you're seeing the uh, uh, the shorthand Signature up there. This Module—just to give you some context—is an excerpt from um, one of the Python files that's in the repo. It is basically taking in a bunch of time entries and making sure that they adhere to certain standards. Making sure that they're capitalized properly or that there are periods at the end of the sentences or whatever it might be. That's from a real client use case where they had hundreds of thousands of time entries and they needed to make sure that they all adhere to the same format.

**18:11**
This was one way to kind of do that very elegantly, at least in my opinion. Was taking... up top you can define the Signature. It's adding some additional instructions that were defined elsewhere. And then saying: for this Module, the "change tense" call is going to be just a vanilla `Predict` call.

And then when you actually *call* the Module, you enter into the `forward` function, which you can intersperse the LLM call—which would be the first one—and then do some hard-coded business logic beneath it.

**18:45**
Tools. As I mentioned before, these are just vanilla Python functions. It's the DSPy Tool interface. So under the hood, DSPy uses `LiteLLM`. And so there needs to be some kind of coupling between the two, but fundamentally um, any type of tool that you would use elsewhere, you can also use in DSPy.

**19:09**
And this is probably obvious to most of you, but here's just an example. You have two functions: `get_weather`, `search_web`. You include that with a Signature. So in this case I'm saying the Signature is "I'm going to give you a question, please give me an answer." I'm not even specifying the types; it's going to infer what that means. Uh, I'm giving it the `get_weather` and the `search_web` tools. And I'm saying, "Okay, do your thing but only go five rounds," just so it doesn't spin off and do something crazy. And then a call here is literally just calling the ReAct agent that I created above with the question "What's the weather like in Tokyo?" We'll see an example of this in the code session. But basically what this would do is give the model the prompt, the tools, and let it do its thing.

**19:55**
So Adapters... before I cover this a little bit, they're basically prompt formatters if you will. So the description from the docs probably says it best: it takes your Signature, the inputs, other attributes, and it converts them into some type of message format that you have specified or that the Adapter has specified.

And so as an example, the JSON Adapter... Taking say a Pydantic object that we defined before, this is the actual prompt that's sent into the LLM. And so you can see the input fields... so this would have been defined as "clinical note type string", "patient info" as a Patient Details object which would have been defined elsewhere. And then *this* is the definition of the patient info. It's basically a JSON dump of that Pydantic object.

**20:47**
**Audience Member:**
So the idea is there's like a base adapter or default that's good for most cases, and this is if you want to tweak that to do something more specific?

**Kevin Madura:**
That's right. Yeah. The question was if there's a base adapter and would this be an example of where you want to do something specific. Answer is yes.

So, um, there's a guy Prashanth who is um—I have his Twitter at the end of this presentation, but he's been great. He did some testing comparing the JSON Adapter with the BAML Adapter. Um, and you can see just intuitively, even for us humans, the way that this is formatted is a little bit more intuitive. It's probably more token efficient too, just considering like if you look at the messy JSON that's here versus the slightly better formatted BAML that's here. Um, can actually improve performance by, you know, 5 to 10% depending on your use case.

So this is a good example of how you can format things differently. The rest of the program wouldn't have changed at all. You just specify the BAML Adapter and it totally changes how the information is presented under the hood to the LLM.

**21:48**
Multimodality. I mean this obviously is more at the model level, but DSPy supports multiple modalities by default. So images, audio, some others. Um, and the same type of thing, you kind of just feed it in as part of your Signature and then you can get some very nice clean output. This allows you to work with them very, very easily, very quickly.

And for those uh, eagle-eyed participants, you can see the first line up there is "Attachments". It's probably a lesser known library. Another guy on Twitter who's awesome, uh, Maxime I think it is. Uh, he created this library that just is basically a catch-all for working with different types of files and converting them into a format that's super easy to use with LLMs. Um, and he's a big DSPy fan as well, so he made basically an adapter that's specific to this. But that's all it takes to pull in images, PDFs, whatever it might be. You'll see some examples of that. Uh, it just makes, at least has made my life super, super easy.

**22:50**
Here's another example of the same sort of thing. So this is a PDF of a Form 4... some, you know, public SEC form from NVIDIA. Um, up top I'm just giving it the link. I'm saying "Okay, attachments do your thing, pull it down, create images, whatever you're going to do." I don't need to worry about it, I don't care about it.

This is super simple RAG, but basically: "Okay, I want to do RAG over this document. I'm going to give you a question, I'm going to give you the document, and I want the answer." Um, and you can see how simple that is. Literally just feeding in the document: "How many shares were sold in total?" Interestingly here, I'm not sure if it's super easy to see, but you actually have *two* transactions here. So it's going to have to do some math likely under the hood. And you can see here the thinking and the ultimate answer.

**23:42**
**Audience Member:**
Is it... on the RAG step, is it creating a vector store of some kind or creating embeddings and then searching over those? Is there a bunch going on in the background there or... what's happening?

**Kevin Madura:**
This is "poor man's RAG." I should have clarified. This is literally just pulling in the document images and... I think Attachments will do some basic OCR under the hood. Um, but it doesn't do anything other than that. That's it. All we're feeding in here... the actual document object that's being fed in, yeah, is literally just the text that's been OCR'd, the images. The model does the rest.

**24:16**
All right, so Optimizers. Uh, let's see how we're doing. Okay. Um, Optimizers are super powerful, super interesting concept. It's been some research um, that argues I think that it's just as performant—if not in certain situations more performant—than fine-tuning would be for certain models for certain situations. There's all this research about In-Context Learning and such.

And so whether you want to go fine-tune and do all of that, nothing stops you. But I would recommend at least trying this *first* to see how far you can get without having to set up a bunch of infrastructure and you know, go through all of that. See how the Optimizers work.

**24:58**
Um, but fundamentally what it allows you to do is... DSPy gives you the primitives that you need and the organization you need to be able to measure and then quantitatively improve that performance. And I mentioned **transferability** before. This transferability is enabled arguably through the use of Optimizers.

Because if you can get... okay, I have a classification task, works really well with GPT-4, but maybe it's a little bit costly because I have to run it a million times a day. Can I try it with GPT-4o-mini? Okay, maybe it's at 70%, whatever it might be. But I run the Optimizer on GPT-4o-mini and I can get the performance back up to maybe 87%. And maybe that's okay for my use case. But I've now just dropped my cost profile by multiple orders of magnitude. And it's the Optimizer that allows you to do that type of model and kind of use case transferability, if you will.

**25:55**
But really all it does at the end of the day under the hood is iteratively prompt... uh, iteratively optimize or tweak that prompt, that string under the hood. And because you've constructed your program using the different Modules, DSPy kind of handles all of that for you under the hood. So if you compose a program with multiple Modules and you're optimizing against all that, it by itself—DSPy—will optimize the various components in order to improve the input and output performance.

**26:26**
And we'll take it from the man himself, Omar. You know, "DSPy is NOT an optimizer." I've said this multiple times. It's just a set of programming abstractions—or a way to program. You just happen to be able to optimize it. Um, so again, the value that I've gotten and my team has gotten is mostly because of the programming abstractions. It's just this incredible added benefit that you are also able to—should you choose—to optimize it afterwards.

**26:55**
And I was listening to this, uh, Dwarkesh and uh, Karpathy the other day. And this kind of... I was like prepping for this talk and this like hit home perfectly. I was thinking about the Optimizers. And someone smarter than me can please, you know, please correct me, but I think this makes sense because... he was basically talking about using LLM-as-a-Judge can be a bad thing because the model being judged can find adversarial examples and degrade the performance or basically create a situation where the judge is not scoring something properly.

**27:35**
Um, because he's saying that the model will find these little cracks. It'll find these little spurious things in the nooks and crannies of the giant model and find a way to cheat it. Basically saying that LLM-as-a-Judge can only go so far until the other model finds those adversarial examples.

If you kind of invert that and flip that on its head, it's *this property* that the Optimizers for DSPy are taking advantage of. To optimize, to find the nooks and crannies in the model—whether it's a bigger model or smaller model—to improve the performance against your dataset. So that's what the Optimizer is doing. It's finding these nooks and crannies in the model to optimize and improve that performance.

**28:15**
So typical flow, I'm not going to spend too much time on this but fairly logical: Construct your program, which is decomposing your logic into the Modules. Define your Metrics to define basically the contours of how the program works. And you optimize all that through to get your final result.

**28:36**
So another talk that this guy Chris Potts just had maybe two days ago... um, where he made the point—this is what I was mentioning before—where GEPA, which you probably saw some of the talks the other day... um, where the Optimizers are on par or exceed the performance of something like GRPO and other kind of fine-tuning methods. So pretty impressive. I think it's an active area of research. People a lot smarter than me like Omar and Chris and others are leading the way on this. But point being, I think prompt optimization is a pretty exciting place to be and if nothing else, is worth exploring.

**29:18**
And then finally Metrics. Again, these are kind of the building blocks that allow you to define *what success looks like* for the Optimizer. So this is what it's using. And you can have many of these and we'll see examples of this where again, at a high level, your program works on inputs, it works on outputs. The Optimizer is going to use the Metrics to understand: "Okay, my last tweak in the prompt, did it improve performance? Did it degrade performance?" And the way you define your Metrics provides that direct feedback for the Optimizers to work on.

**29:50**
Uh, so here's another example, a super simple one from that time entry example I mentioned before. Um, so they can be... the Metrics can either be like fairly rigorous in terms of like "Does this equal 1?" or you know, some type of equality check. Or a little bit more subjective where you're using LLM-as-a-Judge to say "Whatever, was this generated string... does it adhere to these various criteria?" But that itself can be a metric.

**30:20**
And so all of this is to say—it's a very long-winded way of saying—in my opinion, this is probably *most*, if not *all* of what you need to construct arbitrarily complex workflows, data processing pipelines, business logic, whatever that might be. Different ways to work with LLMs. If nothing else, DSPy gives you the primitives that you need in order to build these modular, composable systems.

**30:48**
So if you're interested, some people online... um, there's many, many more. There's a Discord community as well. Um, but usually these people are on top of the latest and greatest. And so would recommend giving them a follow. You don't need to follow me, I don't really do much, but the others on there are really pretty good.

**31:08**
Okay. So the fun part. We'll actually get to some code. So if you haven't had a chance, now's your last chance to get the repo. Um, but I'll just kind of go through a few different examples here of what we talked about.

**[Kevin transitions to VS Code / Jupyter Notebooks]**

**31:28**
Okay. So I'll set up Phoenix, which is from Arize, uh, which is basically an observability platform. Uh, I just did this today so I don't know if it's going to work or not but we'll give it a shot. Uh, but basically what this allows you to do is have a bunch of observability and tracing for all the calls that are happening under the hood. We'll see if this works, we'll give it like another five seconds. Um, but it should I think automatically do all this stuff for me. Yeah, let's see. Yeah, alright, so something's up. Okay, cool.

**32:05**
So I'll just... I'm just going to run through the notebook which is a collection of different use cases basically putting into practice a lot of what we just saw. Feel free to jump in any questions, anything like that. We'll start with this notebook. There's a couple of other more proper Python programs that we'll walk through afterwards. Uh, but really the intent is a rapid-fire review of different ways that DSPy has been useful to me and others.

**32:32**
So load in the `.env` file. Usually I'll have some type of config object like this where I can very easily use these later on. So if I'm... call like "model mixing". So if I have like a super hairy problem or like some workload I know will need the power of a reasoning model like GPT-5 or something else like that, I'll define multiple LLMs. So like one will be 4o, one will be 5, maybe I'll do a 4o-mini, um, you know Gemini 2.5 Flash, stuff like that. And then I can kind of intermingle or intersperse them depending on what I think or what I'm reasonably sure the workload will be. And you'll see how that comes into play in terms of classification and others.

**33:15**
Um, I'll pull in a few others here. I'm using OpenRouter for this. So if you have an OpenRouter API key, would recommend plugging that in. So now I have three different LLMs I can work with. I have Codex, I have Gemini, I have 4o-mini.

**33:30**
And then I'll ask basically for each of them: "Who's best between Google, OpenAI, Open AI?" All of them are hedging a little bit. They say "Subjective, Subjective, Undefined". Alright, great. It's not very helpful.

But because DSPy works on Pydantic, I can define the answer as a **Literal**. So I'm basically forcing it to only give me those three options. And then I can go through each of those and you can see each of them of course chooses their own organization.

**33:58**
Um, the reason that those came back so fast is that DSPy has caching automated under the hood. So as long as nothing has changed in terms of your Signature definitions or basically if nothing has changed—this is super useful for testing—it will just load it from the cache. Um, so I ran this before, that's why those came back so quickly. But that's another kind of super useful um, piece here.

**34:31**
Okay. Make sure we're up and running. So if I change this to "Hello" with a space... you can see we're making a live call. Okay, great, we're still up.

**34:44**
So super simple sentiment classifier. Obviously this can be built into something arbitrarily complex. Make this a little bit bigger. Um, but I'm basically... I'm giving it the text, the sentiment that you saw before. And I'm adding that additional specification to say, "Okay, lower is more negative, higher number is more positive sentiment." I'm going to define that as my Signature. I'm going to pass this into just a super simple `Predict` object. And then I'm going to say, "Okay, well this hotel stinks." Okay, that's probably pretty negative. Now if I flip that to "I'm feeling pretty happy" ... oops ... Good thing I'm not in a hotel right now. Comes out to 8.

**35:30**
And this might not seem that impressive, and you know, it's not really. But the important part here is that it just demonstrates the use of the shorthand Signature. So I have the string, I have the integer, I pass in the custom instructions—which would be in the doc string if I used the class-based method. The other interesting part about DSPy comes with a bunch of usage information built in. So because it's cached it's going to be an empty object. But when I change it, you can see that I'm using Azure right now... but for each call you get this nice breakdown. I think it's from LiteLLM. But allows you to very easily track your usage, token usage, etc. for observability and optimization and everything like that. Just nice little tidbits uh, that are part of it here and there.

**36:23**
I need to make this smaller. We saw the example before in the slides, but I'm going to pull in that Form 4 off of online. I'm going to create this `doc` objects using Attachments. You can see some of the stuff it did under the hood. So it pulled out... PDFPlumber, created Markdown from it, pulled out the images, etc. Again, I don't have to worry about all that. Attachments makes that super easy.

**36:47**
I'm going to show you what we're working with here. In this case we have the Form 4. And then I'm going to do that poor man's RAG that I mentioned before. Okay great, "How many shares were sold in total?" It's going to go through that whole Chain of Thought and bring back the response.

**37:02**
That's all well and good. But the power in my mind of DSPy is that you can have these arbitrarily complex data structures. That's fairly obvious because it uses Pydantic and everything else, but you can get a little creative with it. So in this case I'm going to say, okay, a different type of Document Analyzer Signature. I'm just going to give it the document and then I'm just going to defer to the model on defining the structure of what it thinks is most important from the document.

So in this case I'm defining a dictionary object. And so it will hopefully return to me a series of key-value pairs that describe important information in the document in a structured way. And so you can see here—again this is probably cached—but I passed in... I did it all in one line in this case... but I'm saying I want to do Chain of Thought using the Document Analyzer Signature, and I'm going to pass in the input field which is just the document. I'm going to pass in the document that I got before. And you can see here it pulled out a bunch of great information in this super structured way.

And I didn't have to really think about it. I just kind of deferred all of this to the model, to DSPy, for how to do this.

**38:12**
Now of course you can do the inverse. And saying, okay, I have a very specific business use case, I have something specific in terms of the formatting or the content that I want to get out of the document. I define that as just kind of your typical Pydantic classes. So in this case I want to pull out if there's multiple transactions, the schema itself, important information like the filing date.

**38:40**
Going to define the Document Analyzer Schema Signature. Again, super simple input field which is just the document itself, which was parsed by Attachments... gives me the text and the images. And then I'm passing in the `document_schema` parameter, which has the `DocumentSchema` type which is defined above. And this is effectively what you would pass into structured outputs, um, but just doing it the DSPy way. Where it's going to give you um, basically the output in that specific format.

So you can see the different fields that are here. And it's nice because it exposes it in a way that you can use dot notation, so you can just very quickly access the resulting objects.

**39:27**
So looking at Adapters. Um, I'll use another little tidbit from DSPy which is `inspect_history`. So for those who want to know what's going on under the hood, `inspect_history` will give you the raw dump of what's actually going on. So you can see here the system message that was constructed under the hood was all of this. So you can see input fields or document, output fields are reasoning and the schema. It's going to pass these in.

And then you can see here the actual document content that was extracted and put into the text... into the prompt... with some metadata. This is all generated by Attachments. And then you get the response which follows this specific format. So you can see the different fields that are here. And it's this kind of relatively arbitrary response, um, basically format for the... for the names... which is then parsed by DSPy and passed back to you as the user. Um, so I can do, okay, `response.document_schema` and get the actual result.

**40:30**
To show you what the BAML adapter looks like, we can basically do two different calls. So this is an example from uh, my buddy Prashanth online again. So what we do here is define Pydantic model. Super simple one. Patient Address and then Patient Details. Patient Details has the Patient Address object within it.

And then we're going to say, we're going to create a super simple DSPy Signature to say: taking the clinical note which is a string, the patient info is the output type. And then note, so I'm going to run this two different ways. The first time with the "smart" LLM that I mentioned before. And just use the built-in adapter, so I don't specify anything there. And then the second one will be using the BAML adapter which is defined there.

**41:20**
Um, so a few things going on here. One is the ability to use Python's context, which is the lines starting with `with`. Which allow you to basically break out of what the global LLM um, has been defined as and use a specific one just for that call. So you can see in this case I'm using the same LLM, but if I wanted to change this to like `lm_openai` or something... I think that should work. Um, but basically what that's doing is just offloading that call to the other... whatever LLM that you're defining for that particular call. And something happened. And I'm on a VPN, so let's kill that. Sorry Alex's partners.

**42:07**
Okay. So we had two separate calls. One was to the smart LLM which is I think 4o. The other one was to OpenAI. Everything else is the exact same, the note's the exact same, etc. We got the same exact output. That's great.

But what I wanted to show here is the Adapters themselves. So in this case I'm doing `inspect_history(n=2)`, so I'm going to get both of the last two calls. And we're going to see how the prompts are going to be different.

**2:38**
And so you can see here the first one, this is the built-in JSON schema. This crazy long JSON string. Yeah, LLMs are good enough to handle that, but um, you know, probably not for super complicated ones. Um, and then you see here for the second one it used the BAML notation, which as we saw in the slides, is a little bit easier to comprehend. Um, and on super complicated use cases can actually have a measurable improvement.

**43:07**
Multimodal example. Same sort of thing as before. I'll pull in the image itself. Let's just see what we're working with. Okay great, we're looking at these various street signs. And I'm just going to ask it super simple question: "It's this time of day. Can I park here now? When should I leave?"

And you can see I'm just passing in again the super simple shorthand for defining a Signature. Which then I get out the Boolean in this case and a string of when I can leave.

**43:40**
So Modules themselves. It's again fairly simple. You know, it is a Python class at the end of the day. And so you do some initial initialization up top. In this case, you're seeing the shorthand Signature up there. This Module, just to give you some context, is an excerpt from one of the Python files that's in the repo. It is basically taking in a bunch of time entries and making sure that they adhere to certain standards. Making sure that they're capitalized properly or that there are periods at the end of the sentences or whatever it might be. That's from a real client use case where they had hundreds of thousands of time entries and they needed to make sure that they all adhere to the same format.

**44:37**
Um, so that's actually a good segue to the Modules. Um, so Modules basically just wrapping all of this into some type of replicable logic. Um, and so we're given it the Signature here. We're saying `self.predict`. In this case it's just a demonstration of how it's being used as a class. So I'll just add this module identifier and some sort of counter. But this can be any type of arbitrary business logic or control flow or any database actions, whatever it might be. When this Image Analyzer class is called, this function would run.

And then when you actually invoke it, this is when it's actually going to run the core logic. And so you can see I'm just passing in... so I'm instantiating it, the Analyzer of AIE 123, and then I'll call it. Great, it called that. And you can see the counter incrementing each time I actually make the call. So super simple example. Um, we don't have a ton of time but I'll show you some of the other Modules and how that kind of works out.

**45:35**
In terms of Tool Calling. Fairly straightforward. I'm going to define two different functions: `perplexity_search` and `get_url_content`. Creating a Bio Agent Module. So this is going to define Gemini 2.5 as this particular Module's LLM. It's going to create an Answer Generator object which is a `ReAct` call. So I'm going to basically do tool calling whenever this is called.

And then the `forward` function is literally just calling that answer generator with the parameters that are provided to it. And then I'm creating an async version of that function as well. So I can do that here. I'm going to say, okay, "Identify instances where a particular person has been at their company for more than 10 years." It needs to do tool calling to do this to get the most up-to-date information. And so what this is doing, I'm basically looping through um, and it's going to call that Bio Agent which is using the tool calls in the background. And it will make a determination as to whether their background is applicable per my criteria. In this case Satya is true. Brian should be false.

**46:42**
Um, but what's interesting here while that's going, similar to the reasoning object that you get back for Chain of Thought, you can get a trajectory back for things like ReAct. So you can see what tools it's calling, the arguments that are passed in, um, and the observations for each of those calls. Which is nice for debugging and other obviously other uses.

**47:05**
Um, I want to get to the other content so I'm going to speed through the rest of this. This is basically an async version of the same thing so you would run both of them in parallel. Same idea.

**47:16**
Um, I'm going to skip the GEPA example here just for a second. Um, I can show you what the output looks like, but basically what this is doing is creating a dataset. It is showing you what's in the dataset. It's creating a variety of Signatures. In this case it's going to create a system that categorizes and classifies different basically help messages um, that is part of the dataset. So "My sink is broken" or "My light is out" or whatever it is. It want to classify whether it's positive, neutral, or negative, and the uh, the urgency of the actual message.

It's going to categorize it. And then it's going to pack all this stuff, all those different Modules, into a single Support Analyzer Module. And then from there what it's going to do is define a bunch of Metrics which is based off of the dataset itself. So it's going to say, "Okay, how do we score the urgency?" This is a very simple one where it's okay, it either matches or it doesn't. Um, and there's other ones where it can be a little bit more subjective. And then you can run it. This is going to take too long, probably takes 20 minutes or so.

**48:28**
Um, but uh, what it will do is basically evaluate the performance of the base model and then apply those metrics uh, and iteratively come up with new prompts to uh, create that.

Now I want to pause here just for a second because there's different types of Metrics. And in particular for GEPA, it uses feedback from the teacher model in this case. So it can work with the same level of model, but in particular when you're trying to use say a smaller model... it can actually provide textual feedback so it says not only did you get this classification wrong, but it's going to give you some additional um, information or feedback as you can see here for *why* it got it wrong and what the answer should have been. Which allows it... you should read the paper... but it basically allows it to um, iteratively find that kind of pareto frontier of *how* it should uh, tweak the prompt to optimize it based off that feedback. It basically just tightens that iteration loop.

**49:29**
Um, you can see there's a bunch here. Um, and then you can run it and see how it works.

**49:38**
Um, but kind of just give you a concrete example of how it all comes together. So we took a bunch of those examples from before. And we're basically basically going to do a bit of um, categorization. So I have things like contracts, I have images, I have different things that one DSPy program can comprehend and do some type of processing with. So this is something that we see fairly regularly in terms of... we might run into a client situation where they have just a big dump of files, they don't really know what's in it. They want to find something... they want to maybe find SEC filings and process them a certain way. They want to find contracts and process those a certain way. Maybe there's some images in there and they want to process those a certain way.

**50:28**
Uh, and so this is an example of how you would do that. Where if I start at the bottom here, this is a regular Python file. Um, and it uses DSPy to do all those things I just mentioned. So we're pulling in the configurations. We're setting the regular LLM, the small, and one we use for an image. As an example, Gemini models might be better at image recognition than others, so I might want to defer or use a particular model for a particular workload. So if I detect an image, I will route the request to Gemini. If I detect something else, I'll route it to a 4o or whatever it might be.

**51:09**
So I'm going to process single file. And what it does is use our handy Attachments um, library to put it into a format that we can use. And then I'm going to classify it. And it's not super obvious here, but I'm getting a file type from this `classify_file` function call. And then I'm doing some different type of logic depending on what type of file it is.

So if it's an SEC filing, I do certain things. If it's a certain *type* of SEC filing, I do something else. Uh, if it's a contract, maybe I'll summarize it. If it's something that looks like city infrastructure—in this case the image that we saw before—I might do some more visual interpretation of it.

**51:56**
Um, so if I dive into `classify_file` super quick... It's running the Document Classifier. And all that is is basically doing a `Predict` on the image from the file and um, making sure it returns a type... where is this... returns a type which would be `DocumentType`. And so you can see here, at the end of the day, it's a fairly simple Signature. And so what we've done is basically take the PDF file in this case, take all of the images from it, and take the first image or first few images—in this case a list of images as the input field—and I'm saying "Okay, just give me the type. What is this?" And I'm giving it an option of these document types.

So obviously this is a fairly simple use case. But it's basically saying, given these three images—the first three pages of a document—is it an SEC filing? Is it a patent filing? Is it a contract? City infrastructure? Pretty different things so the model really shouldn't have an issue with any of those. And then we have a catch-all bucket for "other".

**53:05**
And then as I mentioned before, um, depending on the file type that you get back, you can process them differently. So I'm using the small model to do the same type of Form 4 extraction that we saw before. Um, and then asserting basically in this case that it is what we think it is.

**53:23**
Uh, a contract... in this case we're saying, uh, let's see... So city infrastructure, I'll do this one super quick just because it's pretty interesting on how it uses tool calls. And while this is running... I should use the right one, hold on...

**53:57**
So good question is the second part. Like how we generated the list of my documents from zero to six. Did we have like original document as an input or no?

**Kevin Madura:**
No. Uh, so let's let's just go to that uh super quick. So that should be Boundary Detector.

**54:30**
So there's a blog post on this that I published probably in August or so that goes into a little bit more detail. Code is actually pretty crappy in that one, it's going to be better here. Um, but basically what it does is... this is probably the main logic. So for each of the images in the PDF, we're going to call `classify_page`. We're going to gather the results. So it's doing all that asynchronously, pulling it all back. Saying okay all these you know all the different page classifications that there are.

And then I pass the output of that into a *new* Signature that says: Given tuple of page and classification, give me this—I don't know—relatively complicated output of a dictionary of a string, tuple, integer, integer. And I give it this set of instructions to say just detect the boundaries... like this is all very like non-production code obviously. But the point is that you can do these types of things super, super quickly. Like I'm not specifying much—not giving it much context—and it worked like pretty well. Like it works pretty well in most of my testing.

Now obviously there is a ton of low-hanging fruit in terms of ways to improve that, optimize it, etc. Um, but all this is doing is taking that Signature, these instructions, and then I call `ReAct`. And then all I give it is uh, the ability to basically self-reflect and call `get_page_images`.

So it says, "Okay, I'm going to look at this boundary. Well, let me get the the page images for these three pages to make sure basically that the boundary is correct." And then it uses that to construct the final answer. And so it's really... this is a perfect example of like the tight iteration loop that you can have both in um, building it. But then you can kind of take advantage of the model's introspective ability if you will to use function calls against the data itself, the data it generated itself, etc. to kind of keep that loop going.

**55:58**
**Audience Member:**
So under the hood, the beauty of DSPy then is that it enforces kind of structured output on a model?

**Kevin Madura:**
I mean, yes. I think that's probably reductive of of like its full potential, but generally that's that's correct. I mean yes you can use structured outputs, but you have to do a bunch of crap basically to coordinate like feeding all that into the rest of the program. Maybe you want to call a model differently or use XML here or use a different type of model or whatever it might be um, to to do that.

So absolutely. I'm not saying this is the only way obviously to kind of create these applications or that you shouldn't use Pydantic or shouldn't use structured outputs. You absolutely should. Um, it's just a way that once you kind of wrap your head around the the primitives that DSPy gives you, you can start to very quickly build these types of arguably uh—I mean these are like prototypes right now—but like if you want to take this to the next level to production scale, you have all the pieces in front of you to be able to do that.

**57:13**
Um, any other questions? I probably got about five minutes left. Go ahead.

**Audience Member:**
Can you talk about your experience using prompt optimization with DSPy? Is GEPA the primary one?

**Kevin Madura:**
Yeah. Yeah so GEPA... and actually I'll pull up... I ran one right before this. Um, this uses a different algorithm called **MIPRO**. But basically um, the Optimizers... as long as you have well-structured data. So for the machine learning folks in the room, which is probably everybody, obviously the quality of your data is very important. Um, you don't need thousands and thousands of examples necessarily. But as long as you have enough, maybe 10 to 100 of inputs and outputs... and if you're constructing your metrics in a way that is relatively intuitive and and that you know accurately describes what you're trying to achieve... the improvement can be pretty significant.

**57:57**
Um, and so that time entry corrector that I mentioned before... uh you can see the output of here. It's kind of iterating through. It's measuring the output metrics for each of these. And then you can see all the way at the bottom once it goes through all of its optimization stuff, you can see the actual performance on um, the basic versus the optimized model. In this case it went from 86 to 89.

And then interestingly—this is still in development this one in particular—but you can break it down by metrics so you can see where the model is optimizing better or performing better across certain metrics. And this can be really telling as to whether you need to tweak your metric, maybe you need to decompose your metric. Maybe there's other areas within your dataset or the basically the structure of your program that you can improve. Um, but it's a really nice way to understand what's going on under the hood.

And if if you don't care about some of these and the Optimizer isn't doing as well on them, maybe you can maybe you can throw them out too. So it's it's a very kind of flexible system, flexible way of kind of doing all that.

**58:58**
**Audience Member:**
Like, what's the output of the optimization? Like what do you get out of it? And then how do you use that?

**Kevin Madura:**
Yeah, yeah. So the output of the Optimizers is basically just another um... it's almost like a compiled object if you will. So DSPy allows you to save and load programs as well. So the output of the Optimizer is basically just a Module that you can then serialize and save off somewhere, or you can call it later uh as you would any other Module.

**59:22**
**Audience Member:**
And it's just manipulating the phrasing of the prompts? Or like what is it actually like... you know, what's its solution space look like?

**Kevin Madura:**
Yeah, yeah. So under the hood it's literally just iterating on the actual prompt itself. Maybe it's adding additional instructions. It's saying, "Well I keep failing on this particular thing, like not capitalizing the names correctly. I need to add in my upfront criteria in the prompt an instruction to the model to say you must capitalize names properly."

And Chris uh, who I mentioned before, has a really good way of putting this and I'm going to butcher it now, but like... the Optimizer is basically finding **latent requirements** that you might not have specified initially upfront. But based off of the data—it's kind of like a poor man's deep learning I guess—but like it's learning from the data. It's learning what it's doing well, what what is doing not so well. And it's dynamically constructing a prompt that improves the performance based off of your metrics.

**60:11**
**Audience Member:**
Is that like LLM guided? Like is there a model thinking about like capitalization?

**Kevin Madura:**
Yeah, yeah. Question being is it all LLM guided? Yes. Particularly for GEPA, it's using LLMs to improve LLM's performance. So it's using the LLM to dynamically construct new prompts which are then fed into the system, measured, and then it kind of iterates. So it's using AI to build AI, if you will. Thank you.

**60:35**
**Audience Member:**
Go out on this question: Why is the solution object not just the optimized prompt?

**Kevin Madura:**
Why is the solution object not just the optimized prompt?

**Audience Member:**
Why are you using this [module/object]?

**Kevin Madura:**
Oh, absolutely yes. You can get it under the hood. I mean you can... the question was why don't you just get the optimized prompt? You *can*, absolutely.

**Audience Member:**
But what else is there besides the optimized prompt?

**Kevin Madura:**
The the... so what else is there other than the prompt? The DSPy object itself. So the Module, the way things... um... well we can probably look at one if we have time. Um... If I could see a dump of what gets... you know what is the optimized state that would be interesting. Yeah yeah sure let me see if I can find one quick.

**61:23**
Um, but fundamentally at the end of the day, yes, you get an optimized prompt string that you can dump somewhere if you want to. Actually...

**[Kevin navigates file system]**

There's a lot of pieces to the signature right? So it's like how you describe your fields in the doc string. Yes. This is a perfect segue and I'll I'll conclude right after this.

I was playing around with something I was... well I was playing around with this thing called **DSPy Hub** that I kind of created to create a repository of optimized programs. So basically like if you're an expert in whatever, you optimize an LLM against this dataset, or have a great classifier for city infrastructure images or whatever. Kind of like a Hugging Face. You can download something that has been pre-optimized.

**62:09**
And then what I have here... this is the actual loaded program. This would be the output of the optimized process. Or it it is. And then I can call it as I would any anything else. And so you can see here this is the output and I used the optimized program that I downloaded from from this hub. And if we inspect maybe the loaded program...

You can see under the hood it's a `Predict` object with a String Signature of Time and Reasoning. **Here is the optimized prompt ultimately**. This is the output of the optimization process. This long string here. Um, and then the various uh specifications and definitions of the inputs and outputs.

**62:56**
**Audience Member:**
Have you found specific uses of those? Like to his question, like what does it... what can you do with that?

**Kevin Madura:**
It's up to your use case. So if I if I have a... so a Document Classifier might be a good example. If in my business I come across whatever, documents of a certain type, I might optimize a classifier against those and then I can use that somewhere else on a different project or something like that. So out of 100,000 documents, I want to find *only* the pages that have an invoice on it as an example.

Now sure, 100% you can use a typical ML classifier to do that. That's great. This is just an example. But you can also theoretically train or optimize a model to do that type of classification or some type of generation of text or what have you. Which then you have the optimized state of, which then lives in your data processing pipeline. You know, and and you can use it for other types of purposes or give it to other teams or whatever it might be.

So it's just up to your particular use case. Um, something like this like Hub... maybe it's not useful because each individual's use case is so hyper specific, I don't really know. But um, yeah, you can do with it kind of whatever you want.

Probably last question. Yeah.

**64:12**
**Audience Member:**
I've heard that DSPy is can be kind of expensive because you're doing all of these LLM calls. So I was curious your experience with that and maybe relatedly like if you have any experience with like large contexts in your optimization dataset.

**Kevin Madura:**
Yeah so the question was can DSPy be expensive and then for large context kind of how have you seen that or how have you managed that?

The expensive part is totally up to you. If you call a function a million times asynchronously, you're going to generate a lot of costs. I don't think DSPy necessarily... maybe it makes it *easier* to call things, but it's not inherently expensive.

It might, to your point... add more content to the prompt? Like sure, the Signature is a string but the actual text that's sent to the model is much longer than that. That's totally true. I wouldn't say that it's a large cost driver. I mean it again, it's ultimately it's more more of a programming paradigm. So you can write your compressed adapter if you want that like you know reduces the amount that's sent to the to uh to the model.

Um, in terms of large context, I it's kind of the same answer I think in terms of... if you're worried about that, maybe you have some additional logic either in the program itself or in an adapter or part of the module that keeps track of that. Maybe you do some like context compression or something like that. There's some really good talks about that past few days obviously. I have a feeling that that will kind of go away at some point. Either context windows get bigger or context management is abstracted away somehow. I don't really have an answer, just that's more of an intuition.

Um, but DSPy again kind of gives you the tools, the primitives for you to do that should you choose... um, and kind of track that state, check that management over time.

I think that's it. We're going to get kicked out soon so thanks so much for your time. Really appreciate it.

**[Applause]**
