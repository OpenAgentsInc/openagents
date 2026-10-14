# Aether AI: causal world models, and what we take from them

2026-10-14. Research note for the owner, prompted by a shared post that argued
robots can't rely on autoregressive sampling alone and need causality and
discrete reasoning back in the loop, crediting "Biwei and the team at Aether
AI." Claims are marked **verified** (we read the primary source: the paper,
repo, or company page) or **reported** (press, sponsored content, or company
statements we could not check). Nothing here changes Robot Lab code.

## Summary

- **Who.** Aether AI (aetherlabs.ai) is a San Diego company founded in 2026 by
  Biwei Huang, an assistant professor at UC San Diego's Halıcıoğlu Data Science
  Institute. She did her PhD at CMU (2022) under Kun Zhang and Clark Glymour,
  worked earlier at the Max Planck Institute for Intelligent Systems, and is
  a lead author of the open-source `causal-learn` library. **Verified** (the
  HDSI page, the paper author lists, and the causal-learn citation).
- **Money and size.** A $20M seed round announced in June 2026, led by MPCi,
  with Inno Angel Fund, SWC Global, and Unity Ventures. About 20 employees.
  Judea Pearl and Bernhard Schölkopf are named as supporters. **Reported**
  (press release coverage, TNW, Humanoids Daily). No valuation, revenue,
  customers, or paid deployments are public.
- **What they ship.** Three real artifacts:
  1. **CausalWM** (arXiv [2609.23184](https://arxiv.org/abs/2609.23184),
     September 2026): a 16B video world model built on Lightricks' LTX-2. It
     writes a "causal chain of thought" (optical flow and 3D pointmaps)
     before it predicts future video. Code is on
     [GitHub](https://github.com/AetherLabsAI/CausalWM) (inference only, about
     55 stars) and weights are on Hugging Face under the **LTX-2 Community
     License**, with Gemma terms on the text encoder. It was tested on one
     H200. **Verified.** Its claimed first place on TriWorldBench (by about
     half a point) and its PAI-Bench robot score are **self-measured**.
  2. **CRIS-0** (the "Causal Robotic Intelligence System", demonstrated on
     2026-10-08): a robot agent plus the world model. The
     [technical article](https://aetherlabs.ai/articles/real-world-autonomous-robotic-system-with-causality-driven-agent-and-world-model)
     is **verified** as published, but it gives no success rates, baselines,
     or trial counts, and it doesn't name the robot. The demo numbers (0.2–0.5
     s obstruction reaction, 2 s replanning, 90% on ambiguous pick-and-place)
     are **reported** only. No code.
  3. Two research add-ons with public papers: **CD-LAM** (a latent-action
     model with visual confounders removed) and **RSIAgent** (a software agent
     that improves through verified, reusable memory, Apache-2.0).
     **Reported** through a secondary summary; we did not read those papers.
- **Real versus claimed.** The world model, the paper, and the repo are real.
  The "causal" label on CausalWM means structured intermediate predictions
  inside a diffusion transformer, not an explicit causal graph you can query.
  The part that matches the shared post is CRIS-0's agent loop: discrete,
  verifiable stages; executable checks after each stage; and the ability to
  regress to an earlier stage and replan. That loop is described, not measured.
  Aether itself lists friction, contact-rich manipulation, and generalisation
  as open problems.
- **The post.** We could not find who wrote it. Searches for its exact
  wording found nothing, and the only related X post we found is a different
  third-party post about the funding. Treat its author as **unknown**.

## The ideas, simply

**Causal discovery.** Given data, find which variables cause which, rather
than which merely move together. Methods like PC, FCI, and GES (all in
`causal-learn`) test conditional independence or score candidate graphs. Huang's
own work extends this to data whose mechanisms change over time or across
settings (CD-NOD), and to hidden variables.

**Causal world models.** A world model predicts the next state from the
current state and an action. A *causal* one keeps the variables that an
action actually changes, so it can answer an intervention ("if I pull now,
does the corner move?") rather than a correlation ("pulls usually come before
the corner moves"). Huang's ICML 2022 paper on
[action-sufficient state representations](https://proceedings.mlr.press/v162/huang22f.html)
makes this concrete: keep only the state dimensions with a causal path to the
reward, which yields smaller, more robust models for control.

**Why discrete reasoning in the loop.** A robot that samples one long motion
from a learned model compounds small errors, and physical mistakes can't be
undone. CRIS-0 instead cuts a task into stages with a few named variables
each (the tablecloth example uses `cloth_grasped`, `corner_offset_cm`, and
`slippage_detected`), runs a check after every stage, and on failure retries,
steps back, or patches the tool. Learned models do the continuous work inside
a stage; a typed, discrete state machine decides what happens between stages.
That is plan → act → check → repair, with the check grounded in observation.

## What we already have

| Piece | Where | State |
| --- | --- | --- |
| Robot documents (robot, scene, experiment, episode), versioned | `crates/robot-lab` ([#11603](https://github.com/OpenAgentsInc/openagents/issues/11603)) | Done |
| SO-101 import, URDF, forward kinematics, limits | `crates/robot-description` ([#11594](https://github.com/OpenAgentsInc/openagents/issues/11594)) | Done |
| LeRobot v2.1/v3.0 reader, pinned SO-101 fixtures | `crates/robot-data` ([#11605](https://github.com/OpenAgentsInc/openagents/issues/11605)) | Done |
| MuJoCo 3.3.7 runner: arm, table, cube, seeds, checkpoints, replay | `crates/robot-sim` ([#11606](https://github.com/OpenAgentsInc/openagents/issues/11606)) | Done (Linux x86-64) |
| Owned rigid-body physics: contacts, friction cone, hinge/slider servos | `crates/physics` ([#11607](https://github.com/OpenAgentsInc/openagents/issues/11607)) | Done |
| ACT and SmolVLA policies in Rust | [#11608](https://github.com/OpenAgentsInc/openagents/issues/11608), [#11609](https://github.com/OpenAgentsInc/openagents/issues/11609) | Open |
| Gym robotics adapter, cube scoring from physics state | [#11610](https://github.com/OpenAgentsInc/openagents/issues/11610) | Open |
| Lagrange 1 orbit and construction sandbox | `crates/verse-lagrange` | Done |
| Lagrange 2 spaceship factory with robot arms | [#11625](https://github.com/OpenAgentsInc/openagents/issues/11625) | Open |
| Typed decisions (Noul, Choice, Score) on our Clef GPUs, Jev as backup | [Decisions API](../../api/decisions.md), [decision models](../../decision-models/README.md) | Live |

Two features already fit Aether's thesis. First, the simulator, not a model,
decides success: the Lab plan scores the cube task from physics state, and a
kinematic replay never counts as a grasp. Second, the Decisions API returns
typed answers with probabilities rather than free text. One constraint from the
[Lab plan](../../lab/README.md#gym-experiments) stands: a robot policy does not
answer `POST /v1/systemone`. Clef picks between discrete options; it never
outputs joint commands.

## How we would use the ideas

Ranked by value for effort.

### 1. A stage planner with the simulator as the check

A small `robot-plan` layer over `robot-sim` would hold a typed task: stages,
the named variables each stage changes, and a success predicate per stage
computed from simulation state (for example, the gripper's contact force on
the cube is above a threshold, and the cube is lifted at least 2 cm). The
loop:

1. **Plan:** choose the next stage and the controller for it (scripted, then
   ACT or SmolVLA when #11608 or #11609 land).
2. **Simulate:** restore the checkpoint and run the stage.
3. **Check:** evaluate the predicate on physics state.
4. **Act or repair:** commit the step, or regress to an earlier stage, as
   CRIS-0 does.

`robot-sim` already has the pieces this needs: restorable checkpoints, fixed
seeds, stop at step boundaries, and saved partial attempts. Because
predicates are code over state, the result is the simulator's ground truth,
not a sampled guess. Stage predicates also give the Gym a finer failure
reason than pass or fail (the cube slipped in stage 3).

### 2. Typed stage decisions through Clef

Stage choices are discrete questions, so they go through Clef, never
through keyword matching: a `Choice` for which stage comes next or which part
to take, and a `Noul` for whether the grasp is done. The state is a small
JSON summary of the stage variables, plus the camera image once Clef reads
images. Clef proposes, and the predicate from idea 1 verifies. A wrong
proposal costs one simulated stage, not a dropped part. This follows the
workspace rule on routing and gives Clef a robotics workload whose ground
truth comes free from the simulator.

### 3. The Lagrange 2 assembly sequence as a checked discrete plan

#11625 already describes assembly as a plan (which part, which arm, which
port). Model it as a discrete plan whose preconditions are checked by
`physics`:

- the port is reachable within the arm's joint limits;
- the mass budget and centre of mass stay in bounds after the latch;
- the reaction impulse keeps the station's attitude inside the station-keeping
  envelope;
- no collision is swept along the path.

A plan search (topological order over a part dependency graph, then a
best-first search over arm assignments) proposes sequences; each step runs
in physics before it commits. In microgravity, a bad step drifts the station
off its halo orbit and cannot be undone, which is exactly the post's
argument. This is also the most visual way to show it: the station shows a
step being proposed, simulated, rejected, and replanned.

### 4. Learning causal structure from LeRobot episodes

`robot-data` produces typed episodes of states and actions. Running causal
discovery on per-stage summaries (gripper aperture, joint velocity, contact
events, object displacement) could suggest which variables a stage actually
changes, which would seed the stage variables in idea 1. This is research,
not product: demonstrations are observational and confounded by the operator,
and the pinned fixtures hold only four frames. Simulated interventions are
the better data source, because in `robot-sim` we can set one variable and
watch the rest. Do this after ideas 1–3. Treat discovered graphs as
hypotheses for a person to check, not as authority.

### 5. Verification in the Gym

When #11610 lands, add per-stage results: the stage, the predicate, pass or
fail, and the number of retries or regressions. Compare a plain scripted or
policy controller with the same controller under the stage planner, on the
same scene and seed list, with sample counts and uncertainty as the Lab plan
already requires. That comparison tests Aether's thesis on our own task.

### Not now

- **Running CausalWM.** It is 16B parameters, tested on an H200, produces
  video rather than state, and carries LTX-2 and Gemma licence terms. Video
  prediction adds nothing where our simulator already gives exact state. Revisit it
  only for real-camera robots, where no simulator matches the scene.
- **Training a causal world model in Psionic.** Our in-repo Psionic is the
  serving path; its training programs were not imported. Nothing here needs
  new training until the planner shows where a learned model would help.

## Partnering and reusing their work

| Item | Licence | Use |
| --- | --- | --- |
| [`causal-learn`](https://github.com/py-why/causal-learn) (Huang is a co-author) | MIT | Offline analysis for idea 4, in an isolated scratch environment. Port only the algorithms we need to Rust; do not add a Python dependency to the product. |
| [CausalWM code](https://github.com/AetherLabsAI/CausalWM) | LTX-2 Community License | Read only. Do not ship it, and do not train on its weights without a licence review. |
| CausalWM weights (Hugging Face `AetherLabs-AI/CausalWM`) | LTX-2 Community, plus Gemma terms | Not now. |
| CRIS-0 paper | Not on arXiv yet; no code | Read for the loop design (stage variables, verifiers, regression, tool repair). |
| [ASR (ICML 2022)](https://proceedings.mlr.press/v162/huang22f.html) | Paper | Background for choosing stage variables. |

**Should we reach out?** Yes, lightly, once idea 1 has a result we can show.
The natural opening is that our simulator, documents, and Gym could give them
a public, reproducible check of the stage-verification loop, which they
currently lack. We do not need anything from them to start. They have no API
or hosted product to buy, so there is nothing to integrate yet. Any
conversation stays at research exchange. No data sharing; Robot Lab stores
are private by design.

## First experiment (about 3–5 days for one agent)

**Question:** On the existing SO-101 cube scene, does a stage-checked
scripted controller beat the same script run open loop when disturbances are
added?

1. Split the existing scripted cube sequence into five stages (approach,
   pre-grasp, close, lift, place), each with a predicate on MuJoCo state:
   gripper pose error, contact on both fingers, cube height, cube in region
   and released. About 1 day, in a new crate or example that depends on
   `robot-sim` without editing it.
2. Add seeded disturbances between stages: shift the cube 1–3 cm, or add
   joint noise. About half a day.
3. Run 50 seeds both ways: open loop, and stage-checked with retry and
   regression. Record success, time, retries, and the stage of each failure.
   About 1 day.
4. Swap the stage choice for a Clef `Choice` over a JSON stage state, and
   measure how often it agrees with the rule-based choice. About 1 day.
5. Write the result here; feed it into #11610 when the Gym adapter exists.

This needs no new physics, no model training, and no hardware. MuJoCo runs on
a Linux x86-64 host (Robot Lab's current qualification). Coordinate with the
Robot Lab session before touching any of its crates.

## Open questions

- How are CRIS-0's stage variables and verifiers produced: by a language model
  reading demonstrations, by hand, or by causal discovery? The article doesn't
  say, and this matters most for whether we could automate it.
- Do the CausalWM benchmark wins hold under official evaluation, and does
  better video prediction improve a robot's task success at all?
- Can stage predicates be written once per task family (pick, place,
  insert, latch), shared by SO-101 and the Lagrange 2 arms?
- Should Clef's stage decisions be trained on simulator-labelled stage
  states, since the simulator supplies the label for free?
- When does our owned `physics` crate replace MuJoCo for planning checks? It
  runs on phone and web, but grasp contact still needs qualifying against MuJoCo.
- Who wrote the shared post, and what does Aether's team think of a
  simulator-verified public benchmark for their loop?

## Sources

- [Aether AI site](https://aetherlabs.ai) and the
  [CRIS-0 technical article](https://aetherlabs.ai/articles/real-world-autonomous-robotic-system-with-causality-driven-agent-and-world-model)
- [CausalWM paper](https://arxiv.org/abs/2609.23184) and
  [repository](https://github.com/AetherLabsAI/CausalWM)
- [Biwei Huang, UCSD HDSI](https://datascience.ucsd.edu/people/biwei-huang/)
- [causal-learn](https://github.com/py-why/causal-learn)
- [Action-sufficient state representation, ICML 2022](https://proceedings.mlr.press/v162/huang22f.html)
- Funding coverage: [TNW](https://thenextweb.com/news/aether-ai-causal-world-models-20m-seed-physical-ai),
  [RuntimeWire](https://runtimewire.com/article/aether-ai-biwei-huang-20m-causal-world-models-seed),
  [Startup Fortune](https://startupfortune.com/aether-ai-raises-20-million-to-bet-the-next-ai-ceiling-is-causality-not-compute/)
- CRIS-0 coverage: [Humanoids Daily](https://www.humanoidsdaily.com/news/aether-ai-cris0-causal-robot-intelligence),
  [TechCrunch sponsored post](https://techcrunch.com/sponsor/aether-ai/aether-ai-builds-causal-intelligence-for-the-real-world/)
  (paid content)
- Secondary summary of CausalWM, CD-LAM and RSIAgent:
  [aiidelist](https://aiidelist.com/blog/aether-ai-causal-world-models)
