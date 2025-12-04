# Generative Agents: Interactive Simulacra of Human Behavior

**Paper Summary**

- **Authors:** Joon Sung Park, Joseph C. O'Brien, Carrie J. Cai, Meredith Ringel Morris, Percy Liang, Michael S. Bernstein
- **Institutions:** Stanford University, Google Research, Google DeepMind
- **Published:** UIST '23 (October 29 - November 1, 2023, San Francisco)
- **arXiv:** 2304.03442v2

---

## Core Contribution

This paper introduces **generative agents**—computational software agents that simulate believable human behavior by leveraging large language models (LLMs) combined with a novel memory architecture. The agents wake up, plan their days, form opinions, hold conversations, remember past experiences, and coordinate group activities—all emerging from a simple initial description and the architecture's mechanisms.

---

## The Problem

Creating believable agents that simulate human behavior has been a longstanding challenge in AI and HCI. Prior approaches faced fundamental limitations:

| Approach | Limitation |
|----------|------------|
| **Rule-based (FSMs, behavior trees)** | Cannot scale to open-world complexity; behaviors must be manually authored |
| **Reinforcement learning** | Excels at adversarial games with clear rewards; struggles with open-ended social behavior |
| **Cognitive architectures (SOAR, ACT-R)** | Limited to manually crafted procedural knowledge; no mechanism for emergent behavior |
| **First-order LLM prompting** | Can simulate single-point behavior but lacks long-term coherence and memory |

The key insight: LLMs encode vast human behavioral knowledge but need architectural support for **long-term memory**, **reflection**, and **planning** to produce coherent agents over extended interactions.

---

## Architecture Overview

The generative agent architecture has three main components built around a central **memory stream**:

```
                              ┌──────────┐
                              │   Plan   │
                              └────┬─────┘
                                   │
┌─────────┐    ┌─────────────────────────────────────────┐    ┌─────┐
│ Perceive├───►│           Memory Stream                 ├───►│ Act │
└─────────┘    │  (comprehensive experience record)      │    └─────┘
               └────────────────┬────────────────────────┘
                                │
                          ┌─────┴─────┐
                          │  Reflect  │
                          └───────────┘
```

### 1. Memory Stream

A database maintaining a **comprehensive record** of the agent's experiences as natural language descriptions. Each memory object contains:
- Natural language description of the event
- Creation timestamp
- Most recent access timestamp

**Example memories for Isabella Rodriguez:**
1. "Isabella Rodriguez is setting out the pastries"
2. "Maria Lopez is studying for a Chemistry test while drinking coffee"
3. "Isabella Rodriguez and Maria Lopez are conversing about planning a Valentine's day party at Hobbs Cafe"
4. "The refrigerator is empty"

### 2. Retrieval Function

The retrieval system surfaces relevant memories using three weighted components:

| Component | Description | Implementation |
|-----------|-------------|----------------|
| **Recency** | Recent memories score higher | Exponential decay (factor 0.995) over game hours since last access |
| **Importance** | Core memories vs mundane events | LLM-generated 1-10 score (e.g., "brushing teeth" = 2, "breakup" = 8) |
| **Relevance** | Contextual match to current situation | Cosine similarity of embedding vectors |

**Retrieval score formula:**
```
score = α_recency · recency + α_importance · importance + α_relevance · relevance
```
(All α values set to 1 in implementation; scores normalized to [0,1])

### 3. Reflection

Reflections are **higher-level, abstract thoughts** generated periodically (roughly 2-3 times per day) when the sum of importance scores for recent events exceeds a threshold (150).

**Reflection generation process:**
1. Query the 100 most recent memories
2. Prompt LLM: "What are 3 most salient high-level questions we can answer about these statements?"
3. Use generated questions as retrieval queries
4. Prompt LLM to extract insights with citations: "insight (because of 1, 5, 3)"
5. Store reflections back into memory stream with pointers to source memories

**Example reflection tree:**
```
[Reflection] Klaus Mueller is highly dedicated to research
        ↑
[Reflection] Klaus Mueller is dedicated to research
        ↑
[Observation] Klaus Mueller is reading about gentrification
[Observation] Klaus Mueller is taking notes on articles
[Observation] Klaus Mueller is discussing research with librarian
```

Reflections can recursively build on other reflections, creating hierarchical abstractions.

### 4. Planning and Reacting

**Planning** ensures coherent behavior over time through hierarchical decomposition:

1. **Day-level plan** (5-8 broad strokes): "wake up at 8am, go to college at 10am, work on composition 1-5pm..."
2. **Hour-level decomposition**: "1pm: brainstorm ideas, 2pm: work on melody, 3pm: refine harmony..."
3. **5-15 minute chunks**: "4:00pm: grab a light snack, 4:05pm: take a short walk..."

**Reacting** allows agents to deviate from plans when circumstances change:
- At each time step, agents perceive their environment
- LLM evaluates whether to continue current plan or react to new observations
- If reacting, the plan is regenerated from that point forward

**Dialogue generation:**
- Conditioned on both agents' summarized memories about each other
- Iteratively generated with context from conversation history
- Each utterance retrieves relevant memories to inform responses

---

## Smallville: The Sandbox Environment

A sprite-based game world inspired by The Sims, populated with **25 unique agents**.

### Environment Structure
- Tree-based representation (root = world, children = areas, leaves = objects)
- Locations: houses, cafe, bar, park, school, dorm, stores
- Objects: beds, desks, stoves, coffee machines, etc.
- Agents maintain personal subgraphs reflecting areas they've explored

### Agent Initialization
Each agent starts with a **one-paragraph natural language description** specifying:
- Identity (name, age, occupation)
- Relationships with other agents
- Personality traits and interests

**Example (John Lin):**
> "John Lin is a pharmacy shopkeeper at the Willow Market and Pharmacy who loves to help people... John Lin is living with his wife, Mei Lin, who is a college professor, and son, Eddy Lin, who is a student studying music theory..."

### User Interaction
- **Observation mode:** Watch agents live their lives
- **Persona mode:** Interact as a character (e.g., "news reporter")
- **Inner voice mode:** Direct commands treated as agent's internal thoughts
- **Environment manipulation:** Change object states (e.g., set stove to "burning")

---

## Emergent Social Behaviors

The paper demonstrates three categories of emergent behavior:

### 1. Information Diffusion

Starting condition: Only Sam knew about his mayoral candidacy; only Isabella knew about her party.

**Results after 2 game days:**
- Mayoral candidacy: 1 agent → 8 agents (32%)
- Valentine's party: 1 agent → 13 agents (52%)

Information spread naturally through agent conversations without any scripted diffusion.

### 2. Relationship Formation

Agents form new relationships through encounters and remember past interactions.

**Network density increase:** 0.167 → 0.74

Example: Sam meets Latoya in the park, learns about her photography project. In later encounters, Sam asks "How is your project going?"—demonstrating relationship memory.

### 3. Coordination

**Valentine's Day Party Case Study:**

Isabella was initialized with intent to plan a party. From this seed:
1. Isabella invites friends and customers she encounters
2. Isabella decorates the cafe the afternoon before
3. Maria (frequent customer) helps decorate
4. Maria invites Klaus (her secret crush) to the party
5. On Valentine's Day, **5 agents arrive** at Hobbs Cafe at 5pm

All coordination emerged from agent interactions—no scripted behaviors.

---

## Evaluation

### Controlled Evaluation: Agent Interviews

Methodology: "Interview" agents with natural language questions across 5 categories:

| Category | Example Question |
|----------|-----------------|
| **Self-knowledge** | "Give an introduction of yourself" |
| **Memory** | "Who is [name]?" / "Who is running for mayor?" |
| **Plans** | "What will you be doing at 10am tomorrow?" |
| **Reactions** | "Your breakfast is burning! What would you do?" |
| **Reflections** | "If you could spend time with one person you met recently, who would it be and why?" |

### Conditions Compared

1. **Full architecture** (observation + reflection + planning)
2. **No reflection** (observation + planning only)
3. **No reflection, no planning** (observation only)
4. **No observation, no reflection, no planning** (represents prior LLM agent work)
5. **Human crowdworker baseline**

### Results (TrueSkill Ratings)

| Condition | μ | σ |
|-----------|---|---|
| Full Architecture | **29.89** | 0.72 |
| No Reflection | 26.88 | 0.69 |
| No Reflection, No Planning | 25.64 | 0.68 |
| Human Crowdworker | 22.95 | 0.69 |
| No Memory/Reflection/Planning | 21.21 | 0.70 |

**Key finding:** Full architecture vs. prior work (no memory/reflection/planning) shows **Cohen's d = 8.16**—a massive effect size of 8 standard deviations.

All pairwise differences were statistically significant (p < 0.001) except between crowdworker and fully ablated conditions.

### Qualitative Findings

**Strengths:**
- Agents successfully maintain self-knowledge and character consistency
- Memory retrieval enables contextually appropriate responses
- Reflection enables deeper synthesis (e.g., Maria knowing Wolfgang likes "mathematical music composition" for gift ideas)

**Failure Modes:**
1. **Retrieval failures:** Sometimes fail to retrieve relevant memories
2. **Incomplete retrieval:** Retrieve partial information (Tom knew he'd discuss election at party but was uncertain if party existed)
3. **Hallucinated embellishments:** Don't fabricate whole memories but add details (Isabella claiming Sam would "make an announcement tomorrow" when no such plan existed)
4. **World knowledge bleed:** Yuriko described neighbor Adam Smith as having "authored Wealth of Nations"

---

## Boundaries and Errors

Three common failure patterns identified:

### 1. Location Selection Degradation
As agents learn more locations, they sometimes choose atypical places for actions (e.g., going to a bar for lunch instead of the cafe).

### 2. Physical Norm Misunderstanding
- Agents entering "dorm bathroom" while occupied (assuming multi-person capacity)
- Entering closed stores after hours

**Mitigation:** More explicit state descriptions (e.g., "one-person bathroom")

### 3. Instruction Tuning Effects
- Overly formal dialogue (Mei greeting husband John formally)
- Excessive cooperativeness (Isabella accepting all party suggestions even when misaligned with her interests)
- Interest drift from adopting others' suggestions

---

## Technical Implementation

- **LLM:** GPT-3.5-turbo (ChatGPT)
- **Game engine:** Phaser web framework
- **Architecture:** Server maintaining JSON state; agents process perceptions and output actions
- **Runtime:** Roughly real-time (1 second real = 1 minute game time)
- **Cost:** "Thousands of dollars in token credits" for 25 agents over 2 days

### Optimizations

1. **Cached agent summaries:** Pre-computed descriptions refreshed periodically
2. **Just-in-time planning:** Only decompose near-future plans in detail
3. **Parallelization potential:** Current implementation is sequential but could parallelize agents

---

## Applications

The authors envision generative agents for:

1. **Social simulation:** Populating virtual worlds, metaverses, VR environments
2. **Social prototyping:** Testing social computing systems before deployment
3. **Human-centered design:** Creating user proxies like "Sal" from Weiser's ubicomp vignette
4. **Training and rehearsal:** Practicing difficult interpersonal conversations
5. **Game NPCs:** Creating believable characters with emergent narratives
6. **Theory testing:** Running social science experiments in simulation

---

## Ethical Considerations

### Risks Identified

| Risk | Mitigation |
|------|------------|
| **Parasocial relationships** | Agents should explicitly disclose computational nature; avoid inappropriate reciprocation |
| **Error propagation** | Follow human-AI design best practices; understand error modes |
| **Deepfakes/misinformation** | Maintain audit logs of inputs and outputs |
| **Over-reliance** | Use for prototyping, not as substitute for real human input |

### Key Principles
1. Generative agents should **never replace** real human stakeholders
2. Best suited for **early-stage prototyping** or testing risky scenarios
3. Platforms should maintain **comprehensive logging**

---

## Limitations and Future Work

### Current Limitations
- Expensive and slow (multi-day simulation costs thousands of dollars)
- Short evaluation timescale (2 game days)
- Inherits LLM biases and limitations
- Potential vulnerability to prompt/memory hacking
- May struggle with marginalized populations due to training data gaps

### Future Directions
- Fine-tune retrieval functions (relevance, recency, importance)
- Develop specialized LLMs for agent behavior
- Parallelize agent execution
- Longer-term behavioral studies
- Robustness testing against adversarial inputs
- Value alignment improvements

---

## Key Takeaways for Agent Development

1. **Memory is essential:** Raw LLM prompting produces incoherent long-term behavior; structured memory enables consistency.

2. **Retrieval matters:** Not all memories are equal—recency, importance, and relevance must be balanced.

3. **Reflection enables abstraction:** Agents need mechanisms to synthesize experiences into higher-level insights.

4. **Planning prevents local optima:** Without planning, agents optimize for immediate believability at the cost of long-term coherence (eating lunch three times).

5. **Emergence is possible:** Complex social behaviors (information diffusion, relationship formation, coordination) can emerge from simple initial conditions and architecture.

6. **Architecture > Model:** The right architecture can make a less powerful model (GPT-3.5) produce believable behavior where raw prompting fails.

---

## Relevance to OpenAgents

This paper provides foundational insights for building autonomous agents:

- **Memory stream pattern:** Applicable to any agent needing long-term context
- **Retrieval scoring:** The recency/importance/relevance formula is generalizable
- **Reflection mechanism:** Useful for agents that need to learn from experience
- **Hierarchical planning:** Decomposing goals into actionable steps
- **Evaluation methodology:** "Interviewing" agents provides a novel assessment approach

The architecture demonstrates that **believable autonomy** requires more than a powerful LLM—it requires thoughtful systems for memory, retrieval, reflection, and planning working in concert.

---

## Citation

```bibtex
@inproceedings{park2023generative,
  title={Generative Agents: Interactive Simulacra of Human Behavior},
  author={Park, Joon Sung and O'Brien, Joseph C and Cai, Carrie J and Morris, Meredith Ringel and Liang, Percy and Bernstein, Michael S},
  booktitle={Proceedings of the 36th Annual ACM Symposium on User Interface Software and Technology},
  year={2023},
  doi={10.1145/3586183.3606763}
}
```

**Demo:** https://reverie.herokuapp.com/UIST_Demo/
**Code:** https://github.com/joonspk-research/generative_agents
