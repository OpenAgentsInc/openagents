> Write a report on the idea of "general agents" like it was popular in what, 2024 or something, people talked about general agents but then talk shifted to "vertical agents" , specific to coding or legal or whatever. I want to know the full history of "general agents" as a concept and why the shift

# General agents: the long history, the vertical-agent turn, and what actually changed

**You are remembering a real change in the conversation—but the modern general-agent boom started mainly in spring 2023, not 2024.** AutoGPT and BabyAGI made open-ended autonomous agents a major topic in 2023. By late 2024, a much more commercially focused argument had become prominent: instead of building an agent that could “do anything,” build one that reliably performs a particular kind of work. Y Combinator’s *“Vertical AI Agents Could Be 10X Bigger Than SaaS”* was a particularly visible expression of that shift. [Sequoia Capital](https://sequoiacap.com/article/ai-agents-perspective)

But the familiar account—*general agents failed, so everyone switched to vertical agents*—is misleading. General-purpose agent research continued throughout that period, and general-purpose products kept appearing. In November 2024, for example, Microsoft introduced Magentic-One specifically as a **generalist** agent system, using specialized agents inside a broader architecture. [Microsoft](https://www.microsoft.com/en-us/research/publication/magentic-one-a-generalist-multi-agent-system-for-solving-complex-tasks/?utm_source=chatgpt.com)

**My central conclusion is that the industry did not abandon generality. It became more realistic about everything required to turn general capability into dependable work.** Specialization offered a way to define the task, provide the missing context, measure success, control risk, and find a buyer. That is different from proving that general-purpose agents are impossible—or that every specialization needs its own standalone company.

## 1. First, “general agent” has meant several different things

Much of the confusion comes from using one term for several ambitions. For this report, it helps to separate them:

| Concept | What is supposed to be general? |
|---|---|
| **General intelligence** | The ability to learn and solve problems across a very broad range of environments and domains. |
| **Generalist model** | One learned model can perform many different kinds of tasks, rather than being trained separately for each. |
| **General-purpose agent** | One system can pursue different user goals by choosing actions, using tools, observing results, and adapting. |
| **General agent infrastructure** | The same execution framework can support many different applications, even when each application is specialized. |
| **Vertical agent** | A product is designed around a particular profession, business function, industry, or workflow. |

These are not mutually exclusive. DeepMind’s Gato explored generality at the **model** level. A universal-agent theory such as Marcus Hutter’s work addresses a different, more abstract question. A reusable agent framework can support both general assistants and tightly specialized applications. [DeepMind](https://deepmind.google/blog/a-generalist-agent/?utm_source=chatgpt.com)

There is also some imprecision in “vertical.” Legal services are a professional domain; coding and customer support are functions used across many industries. The startup discussion commonly groups all of these under the same label. Here, I will use **specialized agents** when that distinction matters.

The essential distinction is this:

**Breadth, autonomy, and reliability are different properties.** An assistant can discuss almost anything but execute very little. A narrowly scoped agent can act autonomously. A general-purpose agent can remain useful while requiring approval for consequential actions. None of those arrangements is inherently contradictory.

## 2. The idea is much older than the current agent boom

### 1950s–1980s: general problem-solving meets domain knowledge

A clear ancestor is the **General Problem Solver**, developed by Allen Newell, J. C. Shaw, and Herbert Simon in the late 1950s. Their 1958–59 report described an effort to separate the content of a problem from the techniques used to solve it. The program used subgoals, planning, and means–ends analysis: identify the difference between the current situation and the desired situation, then choose operations that reduce that difference. Its demonstrations involved restricted domains such as symbolic logic and elementary algebra. [Bitsavers](https://bitsavers.trailing-edge.com/pdf/rand/ipl/P-1584_Report_On_A_General_Problem-Solving_Program_Feb59.pdf)

That is already recognizably close to the contemporary ambition: **give a system a goal rather than specifying every action**.

SRI’s **Shakey**, developed between 1966 and 1972, extended this lineage into an embodied system combining perception, reasoning, planning, and action. It could plan activities and manipulate objects, but within a carefully constrained environment—not the unrestricted everyday world. [SRI](https://www.sri.com/hoi/shakey-the-robot/?utm_source=chatgpt.com)

Meanwhile, expert systems advanced a different emphasis. **DENDRAL**, begun in 1965, helped establish the importance of substantial domain knowledge. Edward Feigenbaum’s associated argument was that useful performance depended heavily on what a system knew about its subject, not simply on having an elegant general inference mechanism. [CHM](https://computerhistory.org/profile/edward-feigenbaum/?utm_source=chatgpt.com)

This was an early version of today’s dispute:

> How much useful intelligence comes from a general problem-solving engine, and how much comes from supplying the knowledge and structure of a particular domain?

The two approaches did not simply replace each other. General cognitive architectures continued; **Soar**, in use since 1983, explicitly pursued a reusable architecture for a broad range of intelligent behavior. [Soar](https://soar.eecs.umich.edu/home/About/?utm_source=chatgpt.com)

The historical parallel matters, but it should not be overstated. Contemporary foundation models already contain broad learned capabilities that older expert systems generally lacked. The recurring issue is not identical technology; it is the gap between **general reasoning machinery** and **competence in a concrete environment**.

### 1990s: software agents, computer-using assistants, and agent networks

By the 1990s, “intelligent agents” were an established research field. Wooldridge and Jennings’s 1995 survey discussed agents in terms of autonomy, responsiveness to their environments, proactive behavior, and interaction with other agents. The environment could be software, not necessarily a physical world. [Oxford Computer Science](https://www.cs.ox.ac.uk/people/michael.wooldridge/pubs/ker95/ker95-html.html?utm_source=chatgpt.com)

One particularly striking predecessor was **Rodney**, described in *Building Softbots for UNIX*, presented in 1994 following an earlier 1992 report. The authors explicitly called it a general-purpose UNIX softbot. It accepted high-level goals, planned actions, executed commands, maintained information about the environment, and monitored execution. Its work included activities involving files, permissions, and event monitoring. [AAAI](https://cdn.aaai.org/Symposia/Spring/1994/SS-94-03/SS94-03-002.pdf)

In other words, the concept of an agent operating a computer on someone’s behalf predates today’s LLM agents by roughly three decades.

Another important branch was SRI’s **Open Agent Architecture**. Rather than putting every capability into one monolithic program, it connected different agents through a common communication framework and a facilitator that helped route requests to appropriate capabilities. This is an ancestor of the idea that a general assistant might obtain its breadth by coordinating specialists. [SRI](https://www.sri.com/press/story/75-years-of-innovation-open-agent-architecture-software-oaa/?utm_source=chatgpt.com)

That gives us three distinct historical visions, not just two: a broadly capable individual agent, a specialized agent, and a general system composed of specialized agents.

### 2000s–2010s: personal assistants and general learning systems

During the 2000s, broad assistance became a major research objective. SRI’s **CALO—Cognitive Assistant that Learns and Organizes—**began in 2003 as a five-year project. It combined capabilities involving information organization, scheduling, task management, and learning from users. The work contributed to the lineage that produced Siri. [SRI](https://www.sri.com/75-years-of-innovation/75-years-of-innovation-calo-cognitive-assistant-that-learns-and-organizes/?utm_source=chatgpt.com)

This branch emphasized the **personal assistant**: a system that understands the user’s circumstances and helps across multiple activities. That is not quite the same as an unrestricted autonomous worker, but it is an important part of the history.

A separate branch pursued generality through learning across environments. OpenAI’s **Universe**, announced in December 2016, proposed a platform for training and evaluating agents across games, websites, and other software. The agent would interact through screen observations and keyboard-and-mouse actions. [OpenAI](https://openai.com/index/universe/?utm_source=chatgpt.com)

So the present-day idea of “give the AI a computer and let it use software” did not originate with GPT-4. What changed later was the capability of the system controlling that computer.

### 2022: general models make the old vision newly plausible

Several developments converged in 2022.

DeepMind’s **Gato**, introduced that May as *“A Generalist Agent,”* used the same network and weights across very different activities, including game playing, image captioning, dialogue, and robotic control. This was an important demonstration of shared-model generality, although not a demonstration of a dependable all-purpose assistant. [DeepMind](https://deepmind.google/blog/a-generalist-agent/?utm_source=chatgpt.com)

In September, **Adept introduced ACT-1**, an action model for interacting with software. Its stated ambition was a system capable of doing what people can do on computers. Its early demonstrations included operating web applications through a browser interface. [Adept](https://www.adept.ai/blog/act-1/)

Research also made language-model-driven action more concrete. **ReAct**, introduced in 2022, interleaved reasoning with actions and observations. **Toolformer**, introduced in early 2023, investigated models learning when and how to call external tools. These approaches helped connect language-model capabilities to operations outside text generation. [arXiv](https://arxiv.org/html/2210.03629v2?utm_source=chatgpt.com)

The conceptual breakthrough was not the invention of planning or tool use. It was the availability of a broadly knowledgeable, language-accessible controller that could be reused across many tasks.

## 3. Spring 2023: the modern general-agent explosion

GPT-4’s release on **March 14, 2023**, supplied a more capable model for developers to build around. Projects such as **AutoGPT** and **BabyAGI** then made autonomous task execution an unusually accessible experiment. BabyAGI’s original task-planning system dates to March 2023. [OpenAI](https://openai.com/index/gpt-4/?utm_source=chatgpt.com)

The excitement was intense. Sequoia’s May 1, 2023 essay *“Agents on the Brain”* noted that AutoGPT had passed 100,000 GitHub stars by April 21, less than a month after launch. It described the appeal of lightweight applications that could break goals into subtasks and act on users’ behalf. [Sequoia Capital](https://sequoiacap.com/article/ai-agents-perspective)

The implied product was enormously ambitious: a user would describe an objective, and the agent would work out the steps, select tools, execute them, inspect results, and continue.

That created an understandable leap in expectations. If a model could write, reason about code, search for information, and describe a sensible plan, perhaps a loop around that model could turn it into a broadly useful autonomous worker.

By November 2023, Bill Gates was articulating a sweeping version of this vision: personalized agents operating across applications, understanding users’ circumstances, and changing the way people interact with software. His essay is a useful record of the period’s ambition—but it was a forecast, not a description of a fully realized product. [Gates Notes](https://www.gatesnotes.com/AI-agents?utm_source=chatgpt.com)

Importantly, skepticism was present from the beginning. Sequoia’s May essay already identified poor execution, repetitive loops, inadequate feedback, and compute costs. The shortcomings were not discoveries that suddenly arrived in 2024. [Sequoia Capital](https://sequoiacap.com/article/ai-agents-perspective)

**What changed over the following year was the weight given to those shortcomings when deciding what to build and sell.**

## 4. 2023–2024: specialization was growing alongside general agents

The vertical-agent story did not begin after the general-agent story ended.

**Casetext launched CoCounsel on March 1, 2023**, before GPT-4’s public release. The product focused on legal work such as research, document review, deposition preparation, and contract analysis. Its launch announcement emphasized domain-specific testing and involvement from practicing attorneys. This was specialized commercialization developing at almost exactly the same time as the general-agent boom. [PR Newswire](https://www.prnewswire.com/news-releases/casetext-unveils-cocounsel-the-groundbreaking-ai-legal-assistant-powered-by-openai-technology-301759255.html?utm_source=chatgpt.com)

In February 2024, **Sierra** launched around customer-service agents, emphasizing company-specific knowledge, integrations, policies, and the ability to take actions for customers. In March, **Cognition introduced Devin**, organized around software engineering and equipped with a development environment, shell, editor, and browser. [Sierra](https://sierra.ai/blog/introducing-sierra?utm_source=chatgpt.com)

These products changed the pitch from *“look at this general autonomous system”* to *“delegate this recognizable category of work.”*

By late 2024, Y Combinator was explicitly promoting vertical agents as a major business category. Its argument was not merely that specialization improved technical performance. It was that AI companies could address spending on both software and the human work performed around that software. The claim that this could create hundreds of large companies was an investment thesis—not an established consequence of the technology. [YouTube](https://www.youtube.com/watch?v=ASABxNenD_U\&utm_source=chatgpt.com)

The engineering conversation moved in a similarly pragmatic direction. Anthropic’s December 2024 *“Building Effective Agents”* recommended starting with simple approaches and adding complexity when justified. It distinguished predefined workflows from systems in which the model dynamically directs execution, rather than treating maximum autonomy as the goal of every application. [Anthropic](https://www.anthropic.com/engineering/building-effective-agents)

This is the turn you are remembering: **a change in the preferred unit of commercialization, from open-ended intelligence to a defined job or workflow.**

## 5. Why the shift happened

There was no single cause. My reading of the contemporary engineering reports, benchmarks, and business arguments is that six pressures reinforced one another.

### A. Describing a plan was much easier than completing it

The early excitement often treated competent language and plausible planning as evidence of reliable execution. Agent benchmarks exposed the distance between them.

The original **GAIA** benchmark, introduced in November 2023, tested general-assistant questions requiring combinations of reasoning, browsing, tool use, and other capabilities. Its authors reported roughly **92% human success versus 15% for GPT-4 with plugins**. [arXiv](https://arxiv.org/abs/2311.12983)

The original **OSWorld** study in 2024 evaluated agents on real computer tasks. It reported **72.36% human success**, compared with **12.24% for the best evaluated agent**, identifying problems with understanding interfaces and carrying out operations. These are historical results, not estimates of current performance. [arXiv](https://arxiv.org/abs/2404.07972)

Long workflows make reliability particularly demanding. As a simplified illustration, if a task requires 100 essential steps, each succeeds independently with 99% probability, and there is no recovery, overall success is:

**0.99¹⁰⁰ ≈ 37%.**

Real errors are not necessarily independent, and recovery can greatly improve performance. The illustration simply shows why impressive individual actions do not automatically produce dependable end-to-end work.

Specialization helps by constraining decisions, providing known recovery paths, and making intermediate states easier to check. It does not make failure disappear; it makes failure more tractable.

### B. Real work requires local knowledge, not just general intelligence

“Handle this customer’s problem” depends on facts that a generally intelligent model cannot infer from public training data: the customer’s account, what the company promised, the relevant policy, available exceptions, previous interactions, and the actions the agent is authorized to take.

Sierra’s original product description emphasized precisely this combination of company context, business-system integration, policy adherence, and controls. Those are substantive parts of the product, not merely a specialized personality prompt. [Sierra](https://sierra.ai/blog/introducing-sierra?utm_source=chatgpt.com)

Likewise, CoCounsel’s launch described extensive legal testing and involvement from attorneys. The specialization was partly a process for determining what acceptable performance looked like in that profession. [PR Newswire](https://www.prnewswire.com/news-releases/casetext-unveils-cocounsel-the-groundbreaking-ai-legal-assistant-powered-by-openai-technology-301759255.html?utm_source=chatgpt.com)

A useful way to express the distinction is:

**Knowing how a profession works in general is not the same as knowing how this organization should handle this case.**

A vertical product can package that missing context. A general agent can also acquire it—but somebody still has to supply, maintain, and validate it.

### C. Narrower work makes evaluation and feedback more practical

A broadly scoped assistant is difficult to evaluate comprehensively. A specific workflow allows developers to construct representative cases, identify recurring failures, and define acceptance criteria.

Coding had a particular advantage here. A coding agent can often run a program, inspect an error, execute tests, and revise its work. Anthropic’s 2024 guidance explicitly highlighted software development as a promising application because of its feedback mechanisms and opportunities for human review. Tests do not establish complete correctness, but they provide an unusually useful signal. [Anthropic](https://www.anthropic.com/engineering/building-effective-agents)

Legal work illustrates a different situation. It has recognizable tasks and strong incentives to improve productivity, but verification is not automatically easy. A preregistered Stanford-led evaluation published in 2024 found substantial hallucination problems in specialized legal research tools, even though those tools reduced hallucinations relative to the general-purpose chatbot comparison. [RegLab](https://reglab.stanford.edu/publications/hallucination-free-assessing-the-reliability-of-leading-ai-legal-research-tools/?utm_source=chatgpt.com)

That matters to the history: **vertical agents became attractive partly because their quality could be worked on systematically—not because attaching an industry label made them trustworthy.**

### D. Open-ended execution made costs unpredictable

The relevant cost is not simply the price of one model response. It includes repeated calls, searches, failed attempts, tool operations, waiting time, human supervision, and repair.

The 2024 paper **“AI Agents That Matter”** criticized agent research for emphasizing accuracy while neglecting cost. Its experiments showed that some elaborate agent architectures did not outperform much simpler approaches once the accuracy–cost trade-off was examined. [arXiv](https://arxiv.org/html/2407.01502v1)

The commercial implication is straightforward. “Keep working until you achieve this vaguely defined goal” is difficult to price and support. “Process this class of request under these conditions” permits a much clearer estimate of cost per successful outcome.

Specialization also allows reusable preparation: known tools, prepared context, standard checks, and established fallback procedures. The system does not have to rediscover the entire workflow every time.

### E. A defined job provides a buyer, a budget, and a reason to purchase

A general agent creates a customer question: **“What should I use this for?”**

A focused product can begin with an existing problem: a queue of support requests, a software maintenance backlog, or a document-review process. That makes it easier to identify the person responsible for the problem and the metric they want to improve.

This was central to the vertical-agent business thesis. YC’s discussions emphasized products that could perform work previously requiring both software and people, potentially making their economic scope larger than conventional software subscriptions. [Y Combinator](https://www.ycombinator.com/library/Lt-vertical-ai-agents-could-be-10x-bigger-than-saas?utm_source=chatgpt.com)

But task automation, labor savings, and elimination of an entire job are not interchangeable. A product may save substantial time while still requiring supervision, handling only some cases, or changing how a team works rather than removing the team.

The narrative was commercially powerful because it connected AI to recognizable spending. Its strongest claims about future market size remained predictions.

### F. Specialization makes authority and accountability easier to define

The broader the assignment, the harder it becomes to specify acceptable actions. An assistant that researches options is different from one that sends messages, modifies production systems, or commits an organization to a purchase.

OpenAI’s 2025 ChatGPT agent launch illustrates the issue: a broad tool-using system was accompanied by controls around consequential actions and explicit discussion of risks such as malicious instructions embedded in outside content. General capability did not eliminate the need for permissions and safeguards. [OpenAI](https://openai.com/index/introducing-chatgpt-agent/?utm_source=chatgpt.com)

A bounded workflow offers a clearer mandate: what the agent can access, what it can change, what requires approval, and when it must stop.

This is another reason “more autonomous” and “more useful” are not synonyms. Sometimes the commercially valuable design is a powerful agent operating within a narrow, well-specified grant of authority.

## 6. Why coding became so prominent—and why it complicates the story

Coding is not just another specialization.

It offers a relatively accessible digital environment and useful feedback, but it is also a **general-purpose means of acting on digital information**. An agent that can write and execute code can manipulate files, analyze datasets, call services, generate artifacts, and automate other software.

Anthropic made this connection explicit in September 2025 when describing the **Claude Agent SDK**. It reported using the machinery behind Claude Code for non-coding work including research, video creation, and note-taking, and described that machinery as a basis for general-purpose agents with computer access. [Claude](https://claude.com/blog/building-agents-with-the-claude-agent-sdk?utm_source=chatgpt.com)

Cognition’s original Devin announcement also presented software engineering as a starting point for broader ambitions, rather than a declaration that only coding mattered. [Cognition](https://cognition.com/blog/introducing-devin?utm_source=chatgpt.com)

So coding complicates a simple general-to-vertical history. A team can specialize initially because coding provides an effective development and evaluation environment, while building components that later support broader work.

**A narrow initial product can be a route toward generality, not a retreat from it.**

## 7. General agents never went away

Several developments make that clear.

In **October 2024**, Anthropic introduced general computer-use capabilities, explicitly allowing a model to interact with software through interfaces rather than requiring a separate bespoke integration for every application. It also acknowledged that the capability was experimental and error-prone. [Anthropic](https://www.anthropic.com/news/3-5-models-and-computer-use?utm_source=chatgpt.com)

In **November 2024**, Microsoft introduced Magentic-One: a generalist system with an orchestrator coordinating agents for browsing, files, and code. Its architecture made specialization an internal mechanism for broader competence. [Microsoft](https://www.microsoft.com/en-us/research/publication/magentic-one-a-generalist-multi-agent-system-for-solving-complex-tasks/?utm_source=chatgpt.com)

In **July 2025**, OpenAI introduced ChatGPT agent, combining capabilities associated with browsing, research, and computer-based action. Meanwhile, Manus’s engineering account described the substantial work needed to build a broadly useful agent around existing models: managing context, tools, memory, and execution—not merely writing a clever initial prompt. [OpenAI](https://openai.com/index/introducing-chatgpt-agent/?utm_source=chatgpt.com)

There was also evidence that execution capability was improving. METR’s March 2025 study measured progress in the length of software and research tasks agents could complete at a specified success rate. Its historical trend supported increasing task horizons, while explicitly leaving questions about generalization beyond the evaluated tasks. The early failures therefore should not be treated as permanent capability ceilings. [arXiv](https://arxiv.org/html/2503.14499v1?utm_source=chatgpt.com)

The continuing general-agent effort became more engineered, not less general in ambition.

## 8. The emerging synthesis: general systems, specialized capabilities

By 2025–2026, product developments increasingly made the supposed opposition look like a question of system architecture.

Anthropic’s **Agent Skills**, introduced in October 2025, packaged instructions, scripts, and resources that a general agent could load when needed. The explicit objective was to equip general-purpose agents with procedural knowledge and context without requiring an entirely separate custom agent for every use case. [Anthropic](https://www.anthropic.com/engineering/equipping-agents-for-the-real-world-with-agent-skills)

Interoperability efforts addressed related pieces of the problem. **MCP**, introduced in November 2024, standardized connections between AI applications and external tools or data. Google’s **Agent2Agent initiative**, announced in April 2025, addressed communication and cooperation between agents. Neither protocol by itself establishes competence or trustworthy delegation, but both support architectures that do not require all capabilities to live in one closed application. [Anthropic](https://www.anthropic.com/research/model-context-protocol?utm_source=chatgpt.com)

A particularly revealing example came in **February 2026**. Anthropic described enterprise plugins that specialize Claude for different roles and departments, while also showcasing Thomson Reuters CoCounsel Legal as a purpose-built legal agent using the Claude Agent SDK. General infrastructure, portable specialization, and a dedicated vertical product appeared together—not as mutually exclusive alternatives. [Claude](https://claude.com/blog/cowork-plugins-across-enterprise?utm_source=chatgpt.com)

That suggests a more useful model:

**A general execution system, supplied with the appropriate domain knowledge, tools, permissions, and verification procedures.**

The specialization might be packaged as a standalone product, an internal workflow, a skill bundle, a tool, or another agent. Those are different commercial and architectural choices.

## 9. What the history does—and does not—tell us

The evidence supports the proposition that **valuable agent applications need situated competence**: knowledge of the relevant environment, clear authority, effective feedback, and a way to establish that the work succeeded. The launch and engineering accounts repeatedly converge on those requirements. [PR Newswire](https://www.prnewswire.com/news-releases/casetext-unveils-cocounsel-the-groundbreaking-ai-legal-assistant-powered-by-openai-technology-301759255.html?utm_source=chatgpt.com)

It does **not** follow that every domain needs a permanently separate agent company. The history of cooperative agent architectures, and the newer development of portable skills and common agent frameworks, demonstrates alternative ways to package specialization. [SRI](https://www.sri.com/press/story/75-years-of-innovation-open-agent-architecture-software-oaa/?utm_source=chatgpt.com)

Conversely, it does not follow that a general platform will automatically absorb every specialist. The existence of a reusable runtime does not establish that domain content, customer relationships, integrations, quality assurance, or responsibility for outcomes have become valueless. The CoCounsel–Claude example is evidence of coexistence, not proof of either side’s eventual dominance. [Claude](https://claude.com/blog/cowork-plugins-across-enterprise?utm_source=chatgpt.com)

My assessment is that three questions were repeatedly collapsed into one:

**Can the underlying AI handle many domains? Can a deployed system reliably complete this particular work? And where will the resulting economic value accumulate?**

A “yes” to the first does not settle the second or third.

## Bottom line

The history is not a straight line from general agents to vertical agents. It is a recurring attempt to reconcile broad problem-solving machinery with the detailed requirements of actual work—from general symbolic problem solving and expert systems, through software agents and personal assistants, to today’s foundation-model-based systems.

The distinctive overreach of the 2023 boom was the expectation that a broadly capable model, placed inside a relatively generic autonomous loop, would quickly become a dependable worker across many situations. The vertical-agent turn responded to the practical gaps: execution, context, evaluation, cost, permissions, and commercial focus. The warnings were already visible in contemporary accounts. [Sequoia Capital](https://sequoiacap.com/article/ai-agents-perspective)

**The strongest lesson is not “general agents were the wrong idea.” It is “generality does not remove the need for specialization somewhere in the system.”**

The unresolved question is where that specialization should live: inside separate companies, inside general platforms, in portable capabilities, or across networks of cooperating agents. The shift toward vertical agents was one commercially compelling answer to that question—not its final resolution.
