*FAKE! THIS NEVER HAPPENED*

*This debate is a creative synthesis based on the actual blog posts and writing styles of Armin Ronacher and Mario Zechner. While the specific conversations are imagined, the technical positions are grounded in their real arguments.*

# The Great LLM API Debate: Armin vs Mario

**A Long-Form Technical Conversation**

---

## Round 1: Opening Positions

**Armin**: So, Mario. I saw your post. "Armin is wrong and here's why." *[laughs]* I appreciate the directness, but I think you're missing the forest for the trees here.

**Mario**: *[grinning]* Look, I had to get your attention somehow. But seriously, I think you're over-engineering the problem. The message abstraction works. It's literally baked into the model weights.

**Armin**: That's exactly the kind of thinking that gets us into trouble. Just because the models were trained on chat templates doesn't mean the API should slavishly follow that format. We're building distributed systems here, not just wrapping prompt templates.

**Mario**: But that's my point—you're NOT building a distributed system in the CRDT sense. The server is still essentially stateless from your perspective. You send messages, you get responses, you append them. The opaque blobs the server sends back? They're just cache keys. Like HTTP cookies.

**Armin**: *[shakes head]* No, that's where I disagree fundamentally. Once you have cache points, thinking traces, server-side tool execution, VM state—you HAVE a distributed system whether you acknowledge it or not. And we're managing it with the wrong abstractions.

**Mario**: Okay, let's unpack that. Because I think we're talking past each other.

---

## Round 2: The Nature of State

**Mario**: Let me start with a concrete example. When I send a message to Claude, and it responds with a thinking blob, what state am I actually managing?

**Armin**: You're managing a partial view of a larger state machine. The full state includes: your message history, Claude's internal reasoning, the KV cache on their GPU, cache markers, and potentially server-side execution contexts. But you only see a projection of that—the messages and some opaque identifiers.

**Mario**: Right, but here's the thing: I never HAD access to the GPU state or the full reasoning. That's not a change from statefulness to distributed state. It's just... the provider protecting their IP. The contract has always been: you give me messages, I give you completions. Now they're just optimizing with caches.

**Armin**: But that's exactly why it's a synchronization problem! You have state on your side (messages), they have state on their side (caches, KV, reasoning), and there's no protocol for reconciling them when things go wrong. What happens when their cache expires? When there's a network partition? When you want to replay from a checkpoint?

**Mario**: *[pauses]* Okay, I'll grant you that the Responses API is a mess for exactly those reasons. But the completion API? You just... resend everything. Yes, it's quadratic. Yes, it's wasteful. But it's also completely transparent. You have all the state you need to replay.

**Armin**: Except you don't! What about the thinking traces? What about the cache state? If I checkpoint my conversation at turn 50, and Claude has built up a cache of 100K tokens, and then their cache expires, I can't reconstruct that state. I have to pay to rebuild it. That's lossy state synchronization.

**Mario**: But you're treating the cache as canonical state when it's not. It's derived state. An optimization. Under a local-first lens, the cache is server-side scratchpad stuff that doesn't matter for correctness.

**Armin**: *[leans forward]* That's where I think you're wrong. The cache DOES matter for correctness in an economic sense. If I'm running a production agent that processes 1000 requests a day, and my cache invalidation strategy is wrong, I'm paying 10x more than I should. That's a correctness problem for my business.

**Mario**: *[considers this]* Okay, that's actually a fair point. I was thinking about correctness as "can I reconstruct the conversation" but you're thinking about it as "can I maintain the performance characteristics I expect."

---

## Round 3: The Abstraction Question

**Armin**: Let me try a different angle. You say the message abstraction is baked into the weights. Fine. But does that mean our API should also be message-based? Consider: HTTP is built on TCP, but we don't expose socket APIs to web developers. We abstract it.

**Mario**: But in that analogy, TCP is the lower-level primitive, and HTTP is the higher-level convenience. What you're proposing is the opposite—you want to go BELOW messages to... what, tokens? State vectors?

**Armin**: Not necessarily tokens. I'm thinking more about explicit state management. Imagine an API where you have:
- A `state` object representing the current conversation
- Operations to mutate that state (`append_user_message`, `run_inference`, etc.)
- Explicit checkpointing and replay semantics
- Clear contracts about what state is local vs remote

**Mario**: *[skeptical]* But how is that different from just keeping an array of messages and appending to it?

**Armin**: Because messages don't capture the full state! They don't tell you about cache points, they don't tell you about the KV cache, they don't tell you about thinking traces. A proper state object would make all of that explicit.

**Mario**: Okay, but providers will NEVER expose the KV cache to you. It's proprietary. It's tied to their specific hardware and implementation. How do you build an abstraction around something you can't access?

**Armin**: That's the crux of it, isn't it? Maybe we shouldn't be trying to build APIs around closed providers. Maybe we need to look at open-weights models where we CAN control the full state.

**Mario**: *[laughs]* Now we're getting somewhere. So your real position is that closed SaaS LLMs are fundamentally incompatible with proper state management?

**Armin**: I wouldn't go that far, but... yeah, there's tension there. With open weights, I can checkpoint the KV cache, I can store the full state, I can replay deterministically. With closed providers, I'm always working with partial information.

**Mario**: Right. So maybe the real issue isn't the message abstraction—it's the closed nature of SaaS LLMs. The messages API is just doing the best it can given those constraints.

---

## Round 4: Quadratic Growth and Responses API

**Armin**: Let's talk about the quadratic growth problem. You said it's "manageable" but I think you're underestimating how painful it gets at scale.

**Mario**: I said it's manageable for TEXT. Images and files are a different story, but providers have upload APIs for those. The actual message JSON is usually small.

**Armin**: True for short conversations. But I'm working with agents that might have 200+ turns. Each turn is maybe 1KB of JSON. By turn 200, I'm sending 200KB per request. By turn 1000, it's 1MB. That's not nothing.

**Mario**: *[nods]* Fair. That's legitimately wasteful. But is it a sync problem or just an optimization problem?

**Armin**: It's both! The reason it's quadratic is because the server is stateless. If there was proper state management, I'd only send deltas. That's how CRDTs work—you send operations, not the full document.

**Mario**: But CRDTs assume all peers are equal and need to reconstruct the same state. In the LLM case, the server is authoritative for inference. You don't need to reconstruct the KV cache client-side. You just need the server to keep it around.

**Armin**: Which brings us to the Responses API. OpenAI's attempt at exactly that—keep state server-side. And it's a disaster because there's no clear contract about:
- How long state persists
- What happens on network failures
- How to query current state
- How to rollback or fork

**Mario**: Yeah, the Responses API is bad. I said that in my post. But I think it's bad because of poor API design, not because the fundamental idea is wrong.

**Armin**: *[interested]* So you think server-side state COULD work with a better design?

**Mario**: Maybe? But here's the thing: even with a perfect design, you still need to handle the case where server state is lost. Which means you need full state client-side anyway. So why bother with server-side state at all?

**Armin**: Because of the cache! If the server maintains state, it can maintain the cache. That's the whole value proposition.

**Mario**: Right, but the cache is an optimization, not canonical state. You're back to conflating the two.

**Armin**: *[frustrated]* The optimization IS part of the state when it affects correctness! When my agent times out because cache was lost and it has to reprocess 100K tokens, that's a correctness issue.

**Mario**: No, that's a performance issue that you're treating as correctness. Correctness is: does the conversation proceed correctly? Can you replay it? Can you switch providers? The cache doesn't affect any of that.

**Armin**: *[pauses]* I think we're using different definitions of correctness.

**Mario**: I think you're right. Let me try to articulate mine: a system is correct if, given the same inputs, it produces the same outputs. The cache doesn't change the outputs, just the latency.

**Armin**: But in a production system, latency IS part of the contract. If my agent promises results in 30 seconds, and cache invalidation makes it take 5 minutes, I've violated the contract. That's incorrect behavior from the user's perspective.

**Mario**: *[thinks]* Okay, that's... actually a really good point. You're defining correctness at the SLA level, not the pure computation level.

---

## Round 5: Local-First and What It Really Means

**Armin**: Let's talk about local-first principles. You said they only apply to the client side, not the provider.

**Mario**: Right. Because providers will never give you full state. So trying to apply local-first to the provider relationship is futile.

**Armin**: But that's exactly why we should be thinking about it! Local-first says: canonical state lives locally, servers are just for sync and optimization. If we applied that to LLMs, we'd have:
- Full conversation history local
- KV cache is optional server-side optimization
- Ability to replay from local state to any point
- Provider switching becomes trivial

**Mario**: Sure, in theory. But the KV cache isn't just an optimization—it's economically necessary. Without it, long conversations become prohibitively expensive. And you can't rebuild the KV cache without re-running inference, which means paying again.

**Armin**: Which is exactly why providers should expose checkpoint/restore for caches! Let me download my cache state, store it locally, upload it to a different provider. Make it part of the protocol.

**Mario**: *[laughs]* Armin, come on. Anthropic and OpenAI are competitors. They're never going to agree on a cache format. And even if they did, the cache is hardware-specific. An Anthropic cache built on their GPU cluster won't work on OpenAI's infrastructure.

**Armin**: But there could be a standardized intermediate format! Something like GGUF but for KV caches.

**Mario**: You're dreaming. The cache structure is intimately tied to model architecture. A cache for Claude 3.5 Sonnet won't even work for Claude 3.5 Opus, let alone a completely different model from a different provider.

**Armin**: *[deflating slightly]* Okay, fine. Maybe cache portability is unrealistic. But what about the principle? What about designing our CLIENT-side code with local-first principles?

**Mario**: Now THAT I agree with completely. That was my whole point in the blog post. Treat the provider as a black box, keep your canonical state local, treat server-side stuff as transient cache. That's exactly local-first thinking applied correctly.

**Armin**: But then why do you defend the message abstraction? If we're being local-first on the client, shouldn't we have richer state management than just an array of messages?

**Mario**: We can have richer state! But the messages are still the core. You can layer on top: checksums, vector embeddings, application state. But the messages themselves are the canonical log of what was said.

**Armin**: *[considering]* So you're saying: messages as the log, but with metadata and derived state around them?

**Mario**: Exactly. The messages are your append-only log in local-first terms. Everything else—summaries, embeddings, whatever—is derived state you compute from that log.

---

## Round 6: The Hidden State Problem

**Armin**: Let's dig into the hidden state thing. You keep saying "you never had access to it anyway" but I think that's missing the point.

**Mario**: Okay, explain.

**Armin**: The fact that thinking traces are hidden doesn't mean they're not part of the state. When I send a message to a reasoning model, it produces thinking tokens, then a response. Those thinking tokens INFLUENCE the response. They're part of the causal chain. But I can't see them, I can't store them, I can't replay them.

**Mario**: Right, but they're ephemeral. Once the response is generated, the thinking tokens are gone. They're not part of the conversation going forward.

**Armin**: Are they though? What if the model references its earlier reasoning in a later response? What if the cache includes thinking tokens from previous turns?

**Mario**: *[pauses]* That's... actually a good question. I don't know if providers cache thinking tokens or just the final outputs.

**Armin**: Exactly! We don't know. And that's the problem. There's hidden state that we can't introspect, can't control, and don't have contracts around.

**Mario**: Okay, but even if we HAD access to thinking tokens, what would you do with them? Store them? They're huge. A single thinking trace can be 10K+ tokens. Multiply that by 200 turns and you're storing megabytes of text that you can't even meaningfully replay because you don't have the model.

**Armin**: I'd want the OPTION to store them. Maybe I don't store all of them. Maybe I sample. Maybe I use them for debugging. But right now I have zero visibility.

**Mario**: Fair. But this circles back to closed vs open models. With open weights, you can capture everything. With SaaS, you're at the provider's mercy.

**Armin**: Which is why I keep saying: we need better abstractions that are designed for this reality. Not abstractions that pretend it's simpler than it is.

---

## Round 7: SDK Abstractions and Tool Use

**Mario**: Let's shift gears. You said the Vercel AI SDK breaks with tool use. Can you elaborate?

**Armin**: Sure. The Vercel SDK tries to present a unified interface across providers. But tool calling works differently everywhere:
- OpenAI has function calling with strict schemas
- Anthropic has tool use with a different format
- Some models have parallel tool calls, others don't
- Some providers have server-side tools (like Claude's web search)

The SDK tries to paper over these differences, but it leaks. Badly.

**Mario**: Can you give a concrete example?

**Armin**: Sure. Anthropic's web search tool. When you use it through the Anthropic SDK directly, it injects search results into the conversation in a specific way. When you use it through Vercel SDK, the message history gets corrupted because the SDK doesn't know how to handle the search result format.

**Mario**: Okay, that's a legitimate bug in the Vercel SDK. But is that a fundamental problem with abstraction, or just an implementation bug?

**Armin**: I think it's fundamental. You CAN'T unify these things without loss. Each provider has proprietary features that don't map to other providers. Any SDK that tries to hide that is lying to you.

**Mario**: *[nods slowly]* I think you're right about that. The lowest common denominator approach doesn't work when features are so heterogeneous.

**Armin**: Exactly. Which is why I'm using the raw SDKs now. Yes, it means vendor lock-in. Yes, it means more code. But at least I KNOW what's happening.

**Mario**: But doesn't that contradict your local-first principles? If you're locked into Anthropic's SDK, you can't easily switch providers.

**Armin**: That's the tension I'm living with. I want portability, but I also want features. Right now I'm choosing features.

**Mario**: What if there was a way to have both? What if... *[thinking out loud]* what if instead of trying to unify at the API level, we unified at the capability level?

**Armin**: What do you mean?

**Mario**: Like, instead of "here's a message API that works everywhere", it's "here's a capability registry that tells you what each provider supports, and here's provider-specific code for each capability."

**Armin**: So more like a router than an abstraction?

**Mario**: Yeah! The system would know: Anthropic has web search, OpenAI has code interpreter, etc. And it would route to the appropriate provider based on what capabilities you need.

**Armin**: *[intrigued]* That's... actually interesting. Like a capability-based routing layer rather than a unified API.

**Mario**: Right. You'd still write provider-specific code, but the routing and composition logic would be abstracted.

**Armin**: I like this. This feels like it acknowledges reality instead of fighting it.

---

## Round 8: Cache Management Deep Dive

**Armin**: Let's get into the weeds on caching. You said it's just an optimization, but I want to push back on that.

**Mario**: Go ahead.

**Armin**: Anthropic's prompt caching gives you explicit control: you mark cache points with special tags. OpenAI does it automatically. Google does something in between. These are fundamentally different programming models.

**Mario**: Sure, but they're all optimizations. The OUTPUT is the same regardless of whether cache hits.

**Armin**: The output is the same, but the PROCESS is different. And in a long-running agent, the process matters. Let me give you a concrete example:

I'm running an agent that does code reviews. Each review has:
- System prompt (5K tokens)
- Tool definitions (3K tokens)
- Code context (50K tokens)
- Conversation history (variable)

With Anthropic's explicit caching, I can:
- Cache system prompt indefinitely
- Cache tool definitions indefinitely
- Cache code context for the session
- Let conversation history grow without breaking cache

**Mario**: Okay, I'm following.

**Armin**: With OpenAI's automatic caching, I have NO control. Maybe it caches the system prompt. Maybe it doesn't. Maybe it caches the first 20 messages. Maybe it caches the last 20. I don't know, and I can't influence it.

**Mario**: But does it matter if you can't influence it?

**Armin**: YES! Because I structure my prompts differently based on cache behavior. With explicit caching, I put static content first, dynamic content last. With automatic caching, I... well, I don't know what to do. So I just hope for the best.

**Mario**: *[considers]* Okay, I see your point. The explicitness lets you PROGRAM against the cache. Without it, you're just hoping the heuristics work in your favor.

**Armin**: Exactly. And it's not just about cost. It's about predictability. I can reason about my system's behavior when I control the cache. I can't when it's opaque.

**Mario**: Right. So in your "state API" idea, cache points would be first-class?

**Armin**: Absolutely. You'd have operations like:
- `create_checkpoint(state, label)`
- `restore_from_checkpoint(label)`
- `get_cache_stats()`

The cache becomes part of the observable system, not hidden magic.

**Mario**: But again, that only works if providers expose it. And OpenAI clearly doesn't want to.

**Armin**: Which is why I'm saying: we should DEMAND it. We should say "this is a requirement for building serious agents" and push providers to support it.

**Mario**: *[skeptical]* Good luck with that. These companies are optimizing for broad appeal, not power users.

**Armin**: Maybe. But I think as agents become more common, people will hit these issues and demand better tools.

---

## Round 9: The Replay Problem

**Mario**: You keep coming back to replay semantics. Why is that so important to you?

**Armin**: Because debugging. Because testing. Because understanding what the hell happened.

When an agent fails in production, I need to:
1. Capture the state at failure
2. Replay it locally
3. Introspect what went wrong
4. Fix it
5. Verify the fix works

Right now, I can do step 1 (sort of). But step 2 is impossible if there's hidden state I can't capture.

**Mario**: But you can replay the messages. Just send them to the API again.

**Armin**: And pay for inference again. And potentially get different results if cache state was different. And have no visibility into why it failed the first time.

**Mario**: *[nods]* The non-determinism is a pain. I grant you that.

**Armin**: It's not just a pain—it makes debugging nearly impossible. Imagine if your web app had non-deterministic bugs that you couldn't reproduce. That's where we are with agents.

**Mario**: But some non-determinism is inherent. Temperature, sampling, even just floating point variance on GPUs.

**Armin**: True, but we could minimize it. If I could set temperature=0, capture full state including KV cache, I should be able to get deterministic replays.

**Mario**: Assuming the provider doesn't change their model or infrastructure between captures.

**Armin**: Right, which is another argument for open weights. But even with closed providers, we could have BETTER replay than we do now.

**Mario**: What would that look like?

**Armin**: At minimum:
- Deterministic mode (temp=0, fixed seed)
- Full observable state (including cache tags)
- Version locking (pin to specific model version)
- Replay API that guarantees same result for same input

**Mario**: That's... actually reasonable. The version locking alone would be huge.

**Armin**: Right? But providers don't offer it because they want to silently update models.

**Mario**: Which is good for most users—they get improvements automatically.

**Armin**: But terrible for anyone who needs reproducibility. We need an opt-in for stability.

---

## Round 10: Where We Agree

**Mario**: Okay, I think we've been arguing long enough. Let's find common ground.

**Armin**: *[laughs]* Sure. What do we actually agree on?

**Mario**: First: the Responses API is bad.

**Armin**: Definitely.

**Mario**: Second: SDK abstractions that try to hide provider differences are problematic.

**Armin**: Yes.

**Mario**: Third: explicit cache management is better than automatic.

**Armin**: Absolutely.

**Mario**: Fourth: we need better replay semantics for debugging.

**Armin**: Yes!

**Mario**: Fifth: the quadratic growth problem is real, even if it's manageable.

**Armin**: Agreed.

**Mario**: So... we actually agree on most of the PROBLEMS. We just disagree on whether they constitute a "state synchronization problem" in the CRDT sense.

**Armin**: *[considers]* Maybe. Or maybe we're just using different terminology for the same issues.

**Mario**: Let me try to synthesize. You're saying: there's hidden state, it's not properly managed, we need better abstractions. I'm saying: that hidden state is inherently uncontrollable with closed providers, so we should build our systems to be resilient to it.

**Armin**: Right. I'm advocating for better APIs. You're advocating for better client architecture.

**Mario**: Can we do both?

**Armin**: *[grins]* I think we have to.

---

## Round 11: Toward Synthesis

**Mario**: Let me propose something. What if the right model is:
- Accept that providers will have opaque state
- Design client systems with local-first principles
- Push for standardization on the edges (cache control, replay, versioning)
- Use capability-based routing instead of unified APIs

**Armin**: I like most of that. But I'd add:
- Push for open protocols where possible
- Build escape hatches for power users (direct SDK access)
- Document the hidden state even if we can't control it
- Create debugging tools that work despite the opacity

**Mario**: Okay, so here's a concrete proposal. What if there was a tool that:
- Wraps provider APIs with instrumentation
- Captures all visible state (messages, cache tags, timing)
- Provides replay with best-effort determinism
- Warns when hidden state might affect results
- Suggests cache strategies based on usage patterns

**Armin**: That's... actually really good. Like an observability layer for LLM APIs.

**Mario**: Right. It wouldn't solve the fundamental problem, but it would make it manageable.

**Armin**: And it could work across providers. Each provider adapter would know what state is observable and what's hidden.

**Mario**: Exactly. So you'd still write provider-specific code, but you'd get consistent observability.

**Armin**: I'd use that. Hell, I'd build that.

**Mario**: *[laughs]* We should build it.

---

## Round 12: The Open Weights Angle

**Armin**: Okay, but let's talk about the elephant in the room: open weights models.

**Mario**: Where you have full control.

**Armin**: Exactly. With Llama or Mixtral running locally, all the state synchronization problems disappear. I have the weights, I have the KV cache, I can checkpoint and restore at will.

**Mario**: Sure, but you also have operational complexity. You need GPUs, you need to manage infrastructure, you need to deal with versioning yourself.

**Armin**: True. But for serious production use, that might be worth it.

**Mario**: Maybe. But most people aren't going to run their own inference. The economics don't work.

**Armin**: YET. GPU prices are dropping. Open models are getting better. Give it two years.

**Mario**: *[skeptical]* I don't know. The frontier models are still way ahead. And providers can use their scale advantage.

**Armin**: But for specific tasks, fine-tuned open models are already competitive. And they give you all the control we've been talking about.

**Mario**: Okay, so here's a question: if you're designing a system today, what do you choose?

**Armin**: Honestly? A hybrid:
- Open weights for tasks where I need control
- SaaS for tasks where I need frontier performance
- Clear boundaries between them
- Capability router that picks the right backend

**Mario**: That's... actually sensible. Use the right tool for each job.

**Armin**: And as open models improve, migrate more tasks to open weights.

**Mario**: What about the message abstraction in that world?

**Armin**: Still useful! Messages are a good log format. But I'd layer on:
- Explicit state snapshots for open models
- Cache annotations for SaaS models
- Provider-specific extensions
- Clear semantics for replay and forking

**Mario**: So messages plus metadata?

**Armin**: Exactly. The messages are the log. Everything else is operational detail.

**Mario**: *[pauses]* I think we just agreed on something important.

---

## Round 13: What About MCP?

**Mario**: We should talk about MCP. Model Context Protocol.

**Armin**: *[groans]* Yes, I saw the hype. What do you think?

**Mario**: I wrote about it actually. I think it's over-engineered for most use cases.

**Armin**: How so?

**Mario**: Most MCP servers are just wrapping CLI tools or APIs. You could get the same result with Bash and simple tool definitions. Why add another protocol layer?

**Armin**: But the promise is standardization, right? One protocol for all tools.

**Mario**: Sure, but standardization only helps if everyone actually uses it. And if the standard is complex, people won't.

**Armin**: Fair. What's your alternative?

**Mario**: Simple, composable tools. Give the agent Bash access and well-documented CLIs. Let it figure out composition.

**Armin**: *[considers]* That works for code execution agents. But what about more structured environments?

**Mario**: Then maybe you want something richer. But not a whole new protocol. Just good API design.

**Armin**: The thing is, MCP is trying to solve the state problem too. It has resources, which are kind of like shared state between tools.

**Mario**: Which brings us back to the file system approach.

**Armin**: Exactly! The file system is already a protocol for shared state. MCP resources are reinventing it.

**Mario**: So your file system insight from the agent blog post applies here too.

**Armin**: Yeah. Tools need a common place to put data. File system is the obvious choice.

**Mario**: Okay, but here's where MCP might be useful: discovery. How does the agent know what tools are available?

**Armin**: Good point. That's something MCP solves. But you could also just... list files in a directory.

**Mario**: *[laughs]* True. Convention over protocol.

**Armin**: We keep coming back to: simple solutions beat complex protocols.

**Mario**: Unless the simple solution doesn't work, then you need the protocol.

**Armin**: Right. So MCP might be useful if you have a large ecosystem of tools that need discovery and capability negotiation. But for most cases, it's overkill.

**Mario**: Agreed.

---

## Round 14: The Economics of State

**Armin**: Let's talk about money. Because I think that's actually the root of a lot of these issues.

**Mario**: How so?

**Armin**: Providers charge by the token. So they're incentivized to make you send fewer tokens. Hence: caching. But caching introduces state. Which introduces all the problems we've discussed.

**Mario**: Right. If they charged by the request instead of the token, there'd be no pressure to cache.

**Armin**: Exactly. But then long conversations would be prohibitively expensive for providers.

**Mario**: So the token pricing model creates the state management problem.

**Armin**: In a sense, yes. It's an economic problem masquerading as a technical problem.

**Mario**: *[interested]* So what's the alternative?

**Armin**: Subscription model? You pay $X per month for unlimited inference within rate limits.

**Mario**: But then providers have to overprovision for peak load. Doesn't scale well.

**Armin**: True. Or maybe... graduated pricing based on session length?

**Mario**: Like, first 10 turns are cheap, then it gets more expensive?

**Armin**: Yeah. Aligns incentives: provider wants you to finish fast, you want to keep costs down.

**Mario**: But then you're incentivized to make multiple short sessions instead of one long one.

**Armin**: Which might actually be good! Forces you to structure your agent better.

**Mario**: *[laughs]* I don't know if perverse incentives are the answer to good architecture.

**Armin**: Fair. But my point is: pricing model affects system design in weird ways.

**Mario**: Totally. And it's something we don't talk about enough.

---

## Round 15: Testing and Evals

**Armin**: You know what's funny? We've been talking about state management for an hour, but I think the REAL problem is testing.

**Mario**: How do you mean?

**Armin**: All these state issues make testing nearly impossible. How do you write a test for an agent when:
- Results are non-deterministic
- State is partially hidden
- Costs grow quadratically
- Providers can change behavior

**Mario**: *[nods]* Yeah, this is brutal. We've been struggling with it too.

**Armin**: What are you doing?

**Mario**: Honestly? Manual testing mostly. Some smoke tests. But comprehensive evals are really hard.

**Armin**: Same. I want to do evals based on observability data, but the data is incomplete.

**Mario**: Right. You can't assert on things you can't observe.

**Armin**: Which circles back to the state problem! If I had full observable state, I could write assertions against it.

**Mario**: Okay, here's a thought: what if we separated functional testing from performance testing?

**Armin**: Explain.

**Mario**: Functional: does the agent produce the right output? Performance: does it do so efficiently?

For functional, you could use temperature=0, fixed inputs, snapshot testing. You don't care about cache.

For performance, you explicitly test cache behavior: does the system maintain cache correctly? Does it invalidate at the right times?

**Armin**: That's... actually really good. Separate concerns.

**Mario**: Right. And for functional tests, you could even use a different (smaller, cheaper) model. You're just testing the logic.

**Armin**: And for performance tests, you'd want production conditions. Real model, real cache.

**Mario**: Exactly. And you'd have different SLAs for each.

**Armin**: I like this framework. It makes the testing problem feel more tractable.

---

## Round 16: What Should We Build?

**Mario**: Okay, we've been talking for over an hour. What should we actually BUILD to make this better?

**Armin**: Great question. Let me think...

First: an observability layer. Something that instruments provider APIs and gives you visibility into:
- Actual tokens sent/received
- Cache hit rates
- Latency breakdown
- State at each turn

**Mario**: Like a proxy that sits between your code and the provider?

**Armin**: Exactly. With replay capabilities and debugging tools.

**Mario**: I'd use that. What else?

**Armin**: Second: a capability router. You define what capabilities you need (web search, code execution, etc.) and it routes to the best provider for each.

**Mario**: With fallbacks?

**Armin**: Yes. And cost optimization. Maybe Anthropic is best for code but Google is cheaper for search.

**Mario**: Okay, that's cool. What else?

**Armin**: Third: a testing framework designed for agents. With:
- Snapshot testing for deterministic modes
- Performance assertions for cache behavior
- Cost tracking across test runs
- Replay from production captures

**Mario**: Yes! And maybe synthetic data generation for test cases?

**Armin**: Good addition. What would you build?

**Mario**: I think the instrumentation layer is most important. If we had good observability, a lot of other problems would be easier.

**Armin**: Agreed. And it's provider-agnostic. Works with any API.

**Mario**: Should we build it?

**Armin**: *[pauses]* Maybe. But it's a lot of work.

**Mario**: What if we just spec it out? Write down what it should do, what API it should have. See if anyone wants to build it.

**Armin**: I like that. Open spec, multiple implementations.

**Mario**: Like a standard but not too standard.

**Armin**: *[laughs]* The XKCD comic about standards.

**Mario**: Exactly. But seriously, I think a well-designed observability layer could really help.

---

## Round 17: Open Protocols vs Closed Providers

**Armin**: Here's something I keep wrestling with: should we be building on closed providers at all?

**Mario**: What do you mean?

**Armin**: Like, we're trying to build serious infrastructure on top of APIs that could change at any time, that have hidden state, that we can't debug. Is that sustainable?

**Mario**: It's what we have.

**Armin**: But open weights are getting better. At some point, does it make sense to just... abandon the SaaS providers?

**Mario**: *[skeptical]* For frontier performance? No way. The closed models are still way ahead.

**Armin**: But for how long? Llama 4 is supposedly close to GPT-4 level. Qwen is amazing. The gap is closing.

**Mario**: Maybe. But even if open models catch up in quality, the providers have operational advantages: scale, latency, reliability.

**Armin**: True. But the control advantage of open weights might outweigh that.

**Mario**: For some use cases, sure. But most people don't want to run inference.

**Armin**: What if there was a middle path? Open weights hosted by third parties, with proper state management guarantees?

**Mario**: Like Hugging Face Inference or Replicate?

**Armin**: Sort of, but with explicit contracts about state, caching, replay, etc.

**Mario**: That would require standardization across providers.

**Armin**: Which is why I think we need open protocols! Not for the closed providers—they'll never comply. But for the open weights ecosystem.

**Mario**: *[interested]* So you'd have:
- Closed SaaS for frontier (Anthropic, OpenAI)
- Open standard for open weights (Llama, etc.)
- Capability router that picks which to use

**Armin**: Exactly! And over time, more stuff migrates to open as the quality gap closes.

**Mario**: That's actually a pretty compelling vision.

---

## Round 18: The Message Format Future

**Mario**: Okay, let's revisit the original question: are messages the right abstraction?

**Armin**: *[sighs]* I've been thinking about this. I think I was too harsh on messages.

**Mario**: Oh?

**Armin**: Messages are a good log format. They're human-readable, they compose well, they're easy to debug. The problem isn't messages per se.

**Mario**: It's the metadata and operational details.

**Armin**: Right. We need messages PLUS:
- Cache annotations
- State snapshots
- Replay markers
- Provider-specific extensions

**Mario**: So messages are the core, but with a richer envelope?

**Armin**: Yes. Think of it like HTTP: the request/response model is fine, but we needed headers for metadata.

**Mario**: And messages are the body, cache tags are headers?

**Armin**: Something like that. The key is making the metadata standardized and observable.

**Mario**: Okay, so if you were designing this from scratch, what would the format be?

**Armin**: Probably something like:
```json
{
  "message": {
    "role": "user",
    "content": "Hello"
  },
  "metadata": {
    "cache": "hit",
    "checkpoint": "cp_abc123",
    "provider": "anthropic",
    "model": "claude-3-5-sonnet-20250101"
  }
}
```

**Mario**: And that metadata would be provider-specific but documented?

**Armin**: Yes. Each provider would document what metadata they support, and you could use it or ignore it.

**Mario**: That's actually pretty clean. Keeps the message abstraction but adds observability.

**Armin**: Right. And it would enable:
- Better debugging (you can see cache state)
- Better testing (you can assert on metadata)
- Better optimization (you can tune cache strategy)
- Better portability (metadata helps with translation)

**Mario**: I like it. It's evolutionary, not revolutionary.

**Armin**: Which is probably the right approach.

---

## Round 19: Convergence

**Mario**: You know what's funny? I think we started at opposite ends and met in the middle.

**Armin**: How so?

**Mario**: You started from "messages are wrong, we need state management." I started from "messages are fine, stop overcomplicating."

**Armin**: And now?

**Mario**: Now I think: messages are fine, but we need better metadata. And you think?

**Armin**: Messages are fine, but we need better state management AROUND them.

**Mario**: Which is basically the same thing.

**Armin**: *[laughs]* Yeah, I guess it is.

**Mario**: So we agree:
- Messages are the core abstraction
- Metadata needs to be richer and standardized
- Providers will have hidden state, we need to work around it
- Observability is critical
- Testing needs better tools
- Open weights are important for control
- Economics drive behavior

**Armin**: I think that's a pretty good summary.

**Mario**: Should we write this up?

**Armin**: As a joint post?

**Mario**: Why not? "The Great LLM API Debate: Synthesis Edition"

**Armin**: *[grins]* I like it. Show people that disagreement can lead to better understanding.

**Mario**: And maybe inspire someone to build the tools we talked about.

**Armin**: That too.

---

## Final Round: Action Items

**Mario**: Okay, let's get concrete. What should people actually DO with this?

**Armin**: If you're building an agent:
1. Keep full state locally (messages + metadata)
2. Use explicit cache management when available
3. Instrument your API calls for observability
4. Separate functional and performance testing
5. Plan for provider switching from day one

**Mario**: Good list. I'd add:
6. Use the file system for shared state between tools
7. Don't over-abstract—stay close to provider APIs
8. Document what state is observable vs hidden
9. Build replay capabilities early
10. Consider open weights for control-critical paths

**Armin**: Perfect. And for the ecosystem:
- Spec out an observability layer (we should do this)
- Push providers for better replay semantics
- Standardize metadata formats where possible
- Build better testing tools
- Make open weights easier to run

**Mario**: And for providers:
- Expose cache management controls
- Document state lifetimes and guarantees
- Provide replay/determinism modes
- Version lock options
- Better debugging tools

**Armin**: Think they'll listen?

**Mario**: Probably not. *[laughs]* But we can try.

**Armin**: That's the spirit.

---

## Epilogue

**Armin**: This was fun. We should do it again.

**Mario**: Agreed. Maybe we can debate something else next time.

**Armin**: Like what?

**Mario**: How about: "Are ORMs good actually?"

**Armin**: *[groans]* Oh god, not that.

**Mario**: *[laughs]* Too soon?

**Armin**: Way too soon. I still have PTSD from SQLAlchemy.

**Mario**: Fair enough. Hey, thanks for the robust discussion. I learned a lot.

**Armin**: Same here. I think I was being too idealistic about state management. You brought me back to reality.

**Mario**: And you made me realize that "just append messages" isn't a complete answer. We need better tools.

**Armin**: Deal. Let's build them.

**Mario**: Deal.

---

## Postscript: What They Built

*Six months later...*

Armin and Mario collaborated on [LLM Observatory](https://github.com/llm-observatory/llm-obs), an open-source instrumentation layer for LLM APIs. It provides:

- **Unified observability** across providers (OpenAI, Anthropic, Google, etc.)
- **Cache analytics** showing hit rates and recommendations
- **Replay capabilities** with best-effort determinism
- **Testing framework** with snapshot and performance modes
- **Cost tracking** per session, turn, and provider
- **State export/import** for debugging and migration

The project was adopted by several agent frameworks and became the de facto standard for LLM API observability. More importantly, the collaboration showed that technical disagreements, when approached constructively, can lead to better outcomes than either person could have achieved alone.

Armin still complains about hidden state. Mario still defends the message abstraction. But now they do it while shipping code together.

---

**End of Transcript**
