# VOYAGER: An Open-Ended Embodied Agent with Large Language Models

**Paper Summary**

**Authors:** Guanzhi Wang, Yuqi Xie, Yunfan Jiang, Ajay Mandlekar, Chaowei Xiao, Yuke Zhu, Linxi "Jim" Fan, Anima Anandkumar (NVIDIA, Caltech, UT Austin, Stanford, UW Madison)

**Published:** arXiv:2305.16291v2, October 2023

**Source:** https://voyager.minedojo.org

---

## Executive Summary

VOYAGER is the **first LLM-powered embodied lifelong learning agent** in Minecraft that continuously explores the world, acquires diverse skills, and makes novel discoveries without human intervention. It interacts with GPT-4 via blackbox queries (no fine-tuning required) and demonstrates strong in-context lifelong learning capabilities.

**Key Results:**
- **3.3×** more unique items discovered vs. prior SOTA
- **2.3×** longer distances traveled
- **Up to 15.3×** faster tech tree milestone unlocking
- **Only agent** to unlock diamond-level tools
- Successfully generalizes learned skills to new Minecraft worlds

---

## Core Architecture

VOYAGER consists of three key components working in a closed loop:

```
┌─────────────────────────────────────────────────────────────────┐
│                        VOYAGER LOOP                              │
│                                                                  │
│  ┌──────────────────┐    ┌────────────────────┐    ┌───────────┐│
│  │    Automatic     │───▶│     Iterative      │───▶│   Skill   ││
│  │   Curriculum     │    │     Prompting      │    │  Library  ││
│  │                  │◀───│    Mechanism       │◀───│           ││
│  └──────────────────┘    └────────────────────┘    └───────────┘│
│         │                        │                       │       │
│         ▼                        ▼                       ▼       │
│   [Propose Task]          [Generate Code]         [Store/Retrieve│
│                           [Execute]                Skills]       │
│                           [Self-Verify]                          │
└─────────────────────────────────────────────────────────────────┘
```

### 1. Automatic Curriculum

The automatic curriculum is essentially an **in-context novelty search** that proposes progressively harder tasks based on:

**Input Components:**
- **Directives:** "Discover as many diverse things as possible" with constraints to keep tasks achievable
- **Agent State:** Inventory, equipment, nearby blocks/entities, biome, time, health, hunger, position
- **Exploration Progress:** Previously completed and failed tasks
- **Additional Context:** GPT-3.5 self-asks and self-answers questions for supplementary information

**Key Design Decisions:**
- Uses **temperature=0.1** for task diversity (vs. temperature=0 elsewhere)
- Implements a **warm-up schedule** that gradually incorporates more state information as the agent completes more tasks
- Tasks follow concise formats: "Mine [quantity] [block]", "Craft [quantity] [item]", etc.
- Explicitly avoids building/placing tasks that require visual confirmation

**Example Task Proposals:**
| Agent State | Proposed Task |
|-------------|---------------|
| Wooden pickaxe + stone in inventory | "Craft 1 stone pickaxe" |
| Fishing rod + near river biome | "Catch 1 fish" |
| Hunger at 0 + pigs nearby | "Kill 1 pig" |
| Raw iron + coal + furnace | "Smelt 4 raw iron" |
| Night + zombie nearby + sword equipped | "Kill 1 zombie" |

### 2. Skill Library

The skill library stores **executable code** as skills, enabling compositional learning and preventing catastrophic forgetting.

**Architecture:**
- **Storage:** Vector database with embedding-indexed skills
- **Key:** GPT-3.5 text embedding of program description
- **Value:** The executable JavaScript program itself

**Skill Retrieval Process:**
1. Given a new task, GPT-3.5 generates a general suggestion for solving it
2. Combine suggestion with environment feedback as query context
3. Retrieve **top-5 relevant skills** via embedding similarity
4. Include retrieved skills in the code generation prompt

**Skill Characteristics:**
- **Temporally Extended:** Skills can perform multi-step sequences
- **Interpretable:** Readable JavaScript code with comments
- **Compositional:** Complex skills can call simpler skills
- **Reusable:** Generic functions work across different contexts

**Example Skills Generated:**
```javascript
// Simple skill: craftWoodenPlanks
async function craftWoodenPlanks(bot) {
  const logNames = ["oak_log", "birch_log", "spruce_log", ...];
  const logInInventory = logNames.find(logName =>
    bot.inventory.count(mcData.itemsByName[logName].id) > 0);
  if (!logInInventory) {
    await mineWoodLog(bot);
  }
  // ... craft planks from available log type
}

// Complex skill: catchFiveFishSafely
async function catchFiveFishSafely(bot) {
  // Check/craft fishing rod
  // Find water block with exploration
  // Move adjacent to water
  // Equip rod and fish 5 times with error handling
}
```

### 3. Iterative Prompting Mechanism

This mechanism addresses LLMs' inability to consistently produce correct code in one shot through **three types of feedback:**

#### A. Environment Feedback
- Runtime chat logs showing intermediate progress
- Example: `"I cannot make iron chestplate because I need: 7 more iron ingots"`
- Generated via `bot.chat()` calls in control primitives

#### B. Execution Errors
- Syntax errors and runtime exceptions from the JavaScript interpreter
- Example: `"No item named acacia_axe at line 18"`
- Enables GPT-4 to fix bugs (e.g., realizing "acacia_axe" doesn't exist, should use "wooden_axe")

#### C. Self-Verification
- A **separate GPT-4 agent** acts as a critic
- Checks task completion based on agent state
- Provides **critique** if task fails (not just success/failure boolean)
- More comprehensive than simple self-reflection

**Iteration Loop:**
```
For up to 4 rounds:
  1. Retrieve relevant skills from library
  2. Generate/refine code with GPT-4
  3. Execute code in Minecraft
  4. Collect environment feedback + execution errors
  5. Self-verify task completion
  If success:
    Add skill to library
    Request next task from curriculum
  If stuck after 4 rounds:
    Request different task from curriculum
```

---

## Technical Implementation

### Control Primitives (JavaScript/Mineflayer)

VOYAGER uses **code as the action space** rather than low-level motor commands. Key primitives provided:

| Function | Purpose |
|----------|---------|
| `exploreUntil(bot, direction, maxTime, callback)` | Explore until condition met |
| `mineBlock(bot, name, count)` | Mine and collect blocks |
| `craftItem(bot, name, count)` | Craft items at crafting table |
| `placeItem(bot, name, position)` | Place blocks at position |
| `smeltItem(bot, itemName, fuelName, count)` | Smelt items in furnace |
| `killMob(bot, mobName, timeout)` | Attack and collect drops |
| `getItemFromChest(bot, position, items)` | Retrieve from chest |
| `depositItemIntoChest(bot, position, items)` | Store in chest |

### Models Used

| Component | Model | Temperature |
|-----------|-------|-------------|
| Code Generation | GPT-4 (gpt-4-0314) | 0 |
| Automatic Curriculum | GPT-4 | 0.1 |
| Self-Verification | GPT-4 | 0 |
| Question Answering | GPT-3.5 (gpt-3.5-turbo-0301) | 0 |
| Text Embeddings | text-embedding-ada-002 | N/A |

### Environment
- Built on **MineDojo** framework
- Uses **Mineflayer** JavaScript APIs for motor control
- Agent respawns on death with preserved inventory
- Crafting table and furnace recycled after each program execution

---

## Experimental Results

### Baselines Compared
- **ReAct:** Chain-of-thought prompting with reasoning + actions
- **Reflexion:** ReAct + self-reflection for improved future actions
- **AutoGPT:** Task decomposition into subgoals with ReAct-style execution

### Exploration Performance

Over 160 prompting iterations:

| Method | Unique Items | Progress Pattern |
|--------|-------------|------------------|
| **VOYAGER** | **63** | Continuous growth |
| AutoGPT | 25 | Plateau at ~40 iterations |
| Reflexion | 5 | Minimal progress |
| ReAct | 4 | Minimal progress |

### Tech Tree Mastery

Prompting iterations to unlock each tool tier (lower = better):

| Method | Wooden | Stone | Iron | Diamond |
|--------|--------|-------|------|---------|
| **VOYAGER** | **6±2** | **11±2** | **21±7** | **102** (1/3 trials) |
| VOYAGER w/o Skill Library | 7±2 | 9±4 | 29±11 | N/A |
| AutoGPT | 92±72 | 94±72 | 135±103 | N/A |
| Reflexion | N/A | N/A | N/A | N/A |
| ReAct | N/A | N/A | N/A | N/A |

**VOYAGER is the only method to unlock diamond tools.**

### Map Coverage

VOYAGER traverses **2.3× longer distances** while crossing diverse terrains (meadows, deserts, rivers, forests, jungles, snowy areas, oceans). Baselines remain confined to local areas.

### Zero-Shot Generalization

Testing on unseen tasks in a new world (max 50 iterations):

| Method | Diamond Pickaxe | Golden Sword | Lava Bucket | Compass |
|--------|----------------|--------------|-------------|---------|
| **VOYAGER** | **19±3 (3/3)** | **18±7 (3/3)** | **21±5 (3/3)** | **18±2 (3/3)** |
| VOYAGER w/o Skill Library | 36 (2/3) | 30±9 (3/3) | 27±9 (3/3) | 26±3 (3/3) |
| AutoGPT w/ Skill Library | 39 (1/3) | 30 (1/3) | N/A (0/3) | 30 (2/3) |
| AutoGPT | N/A | N/A | N/A | N/A |
| Reflexion | N/A | N/A | N/A | N/A |
| ReAct | N/A | N/A | N/A | N/A |

**Key Finding:** The skill library is transferable—it also improves AutoGPT's performance.

---

## Ablation Studies

### Impact of Each Component

| Ablation | Effect on Discovered Items |
|----------|---------------------------|
| Random Curriculum | **-93%** (most critical) |
| Remove Self-Verification | **-73%** |
| Use GPT-3.5 instead of GPT-4 | **-82%** (GPT-4 gets 5.7× more items) |
| Remove Skill Library | Plateau in later stages |
| Manual Curriculum | Worse than automatic (less adaptive) |
| Remove Environment Feedback | Moderate degradation |
| Remove Execution Errors | Moderate degradation |

### Key Insights

1. **Automatic Curriculum is crucial** - Random task ordering makes many tasks impossible (e.g., crafting iron tools before obtaining iron)

2. **Self-Verification is the most important feedback type** - Determines when to move on vs. retry tasks

3. **GPT-4 significantly outperforms GPT-3.5** - The "quantum leap" in coding ability is essential

4. **Skill Library prevents plateauing** - Enables building complex skills on top of simpler ones

---

## Multimodal Feedback from Humans

VOYAGER can incorporate human feedback for tasks requiring visual perception (not available in text-only GPT-4 at time of writing):

1. **Human as Critic:** Provide visual critique for spatial details (e.g., 3D structure alignment)
2. **Human as Curriculum:** Break down complex building tasks into incremental steps

**Demonstrated Tasks:** Building a Nether Portal and a house with progressive refinement.

---

## Limitations and Future Work

### Current Limitations

1. **Cost:** GPT-4 API is 15× more expensive than GPT-3.5, but necessary for quality

2. **Inaccuracies:** Agent sometimes gets stuck despite iterative prompting; self-verification occasionally misses success signals

3. **Hallucinations:**
   - Curriculum proposes non-existent items (e.g., "copper sword")
   - GPT-4 uses invalid fuel sources (e.g., cobblestone)
   - Calls non-existent API functions

### Future Directions

- Integration with multimodal perception models for visual tasks
- Improvements in GPT API models and open-source LLM fine-tuning
- Application to other domains (e.g., robotics with safety constraints)

---

## Key Contributions

1. **First LLM-powered embodied lifelong learning agent** - Continuous exploration without human intervention

2. **Automatic curriculum via in-context learning** - GPT-4 proposes appropriately challenging tasks based on agent state

3. **Skill library as executable code** - Enables knowledge accumulation, transfer, and composition

4. **Iterative prompting with three feedback types** - Environment feedback, execution errors, and self-verification

5. **No gradient-based training required** - Purely prompt-based interaction with blackbox LLM

---

## Relevance to Autonomous Coding Agents

VOYAGER's architecture offers valuable lessons for building autonomous coding agents:

### Applicable Patterns

1. **Automatic Curriculum / Task Selection:**
   - Agent proposes next task based on current capabilities and state
   - Maintains lists of completed and failed tasks
   - Adapts difficulty based on progress

2. **Skill Library / Code Reuse:**
   - Store verified working code indexed by semantic embeddings
   - Retrieve relevant past solutions for new tasks
   - Compose complex operations from simpler building blocks

3. **Iterative Refinement with Multiple Feedback Types:**
   - Environment feedback (execution results, logs)
   - Execution errors (compiler/runtime errors)
   - Self-verification (does the output meet the goal?)

4. **Code as Action Space:**
   - Generate executable programs rather than low-level actions
   - Naturally handles temporally extended, compositional tasks
   - Interpretable and modifiable outputs

### Differences from Coding Agents

| VOYAGER (Game Environment) | Coding Agents (Software Development) |
|---------------------------|-------------------------------------|
| Minecraft simulation | Code repositories |
| JavaScript game APIs | File system, shell, git, etc. |
| Inventory/position state | Codebase state, test results |
| Self-verification via GPT-4 | Test suites, type checking, linting |
| Exploration-focused curriculum | Task-focused curriculum (user requests) |

---

## Citation

```bibtex
@article{wang2023voyager,
  title={Voyager: An open-ended embodied agent with large language models},
  author={Wang, Guanzhi and Xie, Yuqi and Jiang, Yunfan and Mandlekar, Ajay and Xiao, Chaowei and Zhu, Yuke and Fan, Linxi and Anandkumar, Anima},
  journal={arXiv preprint arXiv:2305.16291},
  year={2023}
}
```
