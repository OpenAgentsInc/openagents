**[00:00]**
**[Visual: A dark screen titled "RLM VISUALIZER" with a code breakdown on the left and a video of the speaker in the bottom left corner. The screen quickly switches to a PDF of a research paper titled "Recursive Language Models".]**

All right, we're going to talk about recursive language models. We're going to step through the paper that just came out. We'll talk towards the end about why this is relevant for our model. The one sentence of why this is relevant to what we discussed yesterday—yesterday we talked about how to connect all these devices into a swarm compute network.

**[00:14]**
**[Visual: A brief flash of a video thumbnail titled "Fracking Apple Silicon" with a bar chart, then returning to the speaker.]**

But now we have a reason to sort of attach value to that compute because it is absolutely perfect for this RLM model we're about to explain. So we're going to read the abstract and the key points here.

**[00:36]**
**[Visual: The speaker is looking at the paper PDF.]**

Part of what makes me as a non-academic pay attention to papers like this is when I see people on my timeline saying that they applied it and it's having all these major improvements for them.

**[00:41]**
**[Visual: A Twitter/X post by user "Yasir" is displayed. It reads: "This is a major unlock for programmable AI for all domains... I wrote a codex skill using the RLM paper... and already see a massive gain in output performance AND 66% reduced in token usage."]**

So just some Codex user here, "Yeah, I wrote a Codex skill using the RLM paper and already see a massive gain in output performance and 66% reduced in token usage. Optimize RLM programs on smaller models will bring compute scalability to both personal..." Okay. This seems like it's worth looking into. And the authors are from MIT CSAIL and Omar created DSPy. So these are serious folks and everyone seems to be talking excitedly about this. So let's step through this and then we'll tie it into why we're using this and tying this into our launch tomorrow.

**[01:28]**
**[Visual: The speaker briefly navigates to an X profile for "Christopher David" showing a pinned post about "Fracking Apple Silicon", then returns to the paper.]**

Drumroll please... one second. Yeah, so our software Pylon launches tomorrow featuring integration with this model. Okay, let's step through this.

**[01:40]**
**[Visual: The speaker zooms in on the Abstract of the "Recursive Language Models" paper.]**

"We study allowing large language models (LLMs) to process arbitrarily long prompts through the lens of inference-time scaling. We propose Recursive Language Models (RLMs), a general inference strategy that treats long prompts as part of an external environment and allows the LLM to programmatically examine, decompose, and recursively call itself over snippets of the prompt. We find that RLMs successfully handle inputs up to two orders of magnitude beyond model context windows and, even for shorter prompts, dramatically outperform the quality of base LLMs and common long-context scaffolds across four diverse long-context tasks, while having comparable (or cheaper) cost per query."

**[02:20]**
**[Visual: The speaker scrolls down to the Introduction section.]**

This is striking at just major limitations of agents and models getting towards super massive context while being cheaper. So we'll go a little bit more into the background here.

**[02:37]**
**[Visual: The speaker reads from the Introduction.]**

"Despite rapid progress in reasoning and tool use, modern language models still have limited context lengths and, even within these limits, appear to inevitably exhibit context rot."

Da-da-da-da-da. "Though context lengths will keep improving from training, we are interested in whether it's possible to dramatically scale the context size of general-purpose LLMs by orders of magnitude. This is increasingly urgent as LLMs begin to be widely adopted for long-horizon tasks, in which they must routinely process tens if not hundreds of millions of tokens."

**[03:07]**
**[Visual: The speaker highlights the text regarding "scaling inference-time compute".]**

"We study this question through the lens of scaling inference-time compute. We draw broad inspiration from out-of-core algorithms, in which data-processing systems with a small but fast main memory can process far larger datasets by cleverly managing how data is fetched into memory. Inference-time methods for dealing with what are in essence long-context problems are very common though typically task-specific."

Context compaction, blah, blah, blah. Okay.

**[03:31]**
**[Visual: The speaker scrolls to Figure 2, titled "A Recursive Language Model (RLM)". It shows a diagram of a prompt entering a loop involving a Language Model and a Python REPL.]**

So the key insight of RLMs is that long prompts should not be fed into the neural network directly but should instead be treated as part of the environment that the LLM can symbolically interact with.

**[03:52]**
**[Visual: The speaker scrolls quickly past sections on "Scaling Long Context Tasks" and "Tasks".]**

Benchmarks, blah, blah, blah.

**[04:00]**
**[Visual: The speaker stops at "3. Results and Discussion" and highlights Observation 1.]**

Observation 1: RLMs can scale to the 10 million plus token regime and can outperform base LLMs and existing task-agnostic agent scaffolds on long context tasks. Cool.

**[04:11]**
**[Visual: Highlights Observation 2.]**

Observation 2: The REPL environment is necessary for handling long inputs, while the recursive sub-calling of RLMs provides strong benefits on information-dense outputs.

**[04:22]**
**[Visual: Highlights Observation 3.]**

Observation 3: LLM performance degrades as a function of input length and problem complexity while RLM performance scales better.

**[04:30]**
**[Visual: Highlights Observation 4.]**

Observation 4: The inference cost of RLMs remains comparable to a base model call but are high variance due to differences in trajectory lengths.

**[04:39]**
**[Visual: Highlights Observation 5.]**

Observation 5: RLMs are a model-agnostic inference strategy but different models exhibit different overall decisions on context management. Okay.

**[04:52]**
**[Visual: The speaker scrolls to section "5 Limitations and Future Work" and highlights the text.]**

Okay, so here's kind of the key section for our purposes. "While RLMs show strong performance on tasks beyond the context window limitations of existing LLMs at reasonable inference costs, the optimal mechanism for implementing RLMs remains unexplored. We focused on synchronous sub-calls inside of a Python REPL environment, but we note that alternative strategies involving asynchronous sub-calls and sandboxed REPLs can potentially significantly reduce the runtime and inference cost of RLMs."

**[05:30]**
**[Visual: Continuing to read the highlighted section.]**

"Furthermore, we chose to use a max recursion depth of one... while we found strong performance on existing long-context benchmarks, we believe that future work should investigate deeper layers of recursion." Okay.

**[05:42]**
**[Visual: The speaker addresses the camera directly while the paper remains on screen.]**

So taking a task, decomposing it into a bunch of subtasks, and then wanting to throw that out to small models—you're basically having one smart model throw it out to a bunch of small models. To be able to throw out subtasks to like a hundred different models at the same time, different tasks, have that come back within a few seconds, use that to throw out more... there's a lot, a lot, a lot that can be done.

**[06:26]**
**[Visual: The screen briefly switches to the "RLM Visualizer" again, then to the "Christopher David" Twitter profile, and finally to a video titled "Power at Scale" showing a bar chart of compute power.]**

I think that this model we've been talking about of, when you have millions of Macs connected into a swarm compute network where you're not needing to care about one or a few cloud providers rate limiting you or forcing you to do things sequentially—a swarm compute network *can* let you, and how we've architected our system with Nostr, you can literally send out jobs, like a thousand of them, to a thousand different providers. They're just signed JSON blobs over websockets that a thousand providers can pick up.

**[07:03]**
**[Visual: The speaker continues to talk over the "Power at Scale" chart.]**

So all of that is going to be enabled by the software that we launch tomorrow. To be clear, for the first week, we're going to be using only testnet funds for Bitcoin. The live money version will go live the following Wednesday, January 14th.

**[07:19]**
**[Visual: The screen switches to a blog post by "Prime Intellect" titled "Recursive Language Models: the paradigm of 2026".]**

Okay, we'll pause there. Aside from just to kind of emphasize another lab doing decentralized OpenAI stuff says they're super excited also about RLMs. "The paradigm of 2026."

So I think this is exciting. I think maybe there's a bunch of researchers who would appreciate having access to a swarm network of nodes running the models to enable these jobs to be thrown out to thousands of people at a time. And tomorrow we're going to release the software letting you become one of those nodes on the network and we'll start testing this in practice. See you soon.
