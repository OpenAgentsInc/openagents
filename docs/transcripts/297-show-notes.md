# Episode 297 show notes: Toward Open Robotics

Drafted 2026-11-15 for Episode 297, after [Episode 296, *Coup*](296.md).
This is a talk outline, not a script. The lines are written to be said out
loud, in the voice of Episodes 293 to 296. Read them, change the words, and
say them your own way.

Episode 296 ended on "truly open AI will win, one way or another." This
episode is the first concrete answer: what we are doing about it in
robotics. Nothing here is a robot you can buy or a policy we trained. The
honest title is in the first word: *toward*.

The demo is the [Microduck workbench](../../metaverse/microduck/README.md):
source-bound articulation, an illustrative servo and portable experiment
replay. The native viewer and live servo are already in TestFlight; the
saved-run delivery is tracked in the [1301 verification record](../verification/microduck-replay-13641/README.md).
The [18-second assembly film](../verification/microduck-film-13643/README.md)
is published; its walking is labeled illustrative animation.
[Section 6](#6-the-tour) is the tour. Check the delivery record and
[recording checklist](#check-before-recording) before filming.

## The short version

- **The idea:** NVIDIA built a broad, connected robotics stack, with
  different kinds of openness. Its future is uncertain. The loop it supports
  (design, simulate, teach, deploy, repair) has to exist somewhere nobody can close.
- **The line:** we don't need NVIDIA's whole catalog. We need one robot,
  one task, and the whole loop, on code we own.
- **Two robots:** a small walking duck we train ourselves, and a tabletop
  arm you can print parts for and build.
- **What exists:** a robot's control maths, a real GR00T model running on a
  Mac, a simulated control loop, and the first pieces of the duck pipeline,
  landed today: an experiment record, an illustrative motor model and shared
  observation statistics. The collector is next.
- **What doesn't:** our own contact simulator, a policy we trained that
  walks, and any motor moving at all.
- **The ask:** preserve the permitted inputs for a reproducible project,
  check our work, and help prove someone else can build and repair it.

## 1. Cold open: from the coup to the workbench

> Last episode was about what happened to Jensen. This episode is about
> what we do next.
>
> I said truly open AI will win. Fine. Where? Start with the hardest
> place. Robots.

On screen: the GTC Berlin clip, four small ducks sitting, standing and
lying down on cue. Then cut to our viewer with the same duck, still.

> That was three and a half weeks ago. Four little robots on a stage.
> Whoever was meant to carry that forward, it's now also us.

The fact behind the line: the keynote was October 21, 2026. The robots are
Microducks from Pollen Robotics and Hugging Face.

## 2. What NVIDIA built, in one minute

Talk to someone who has heard "Isaac" and "GR00T" and nothing else.

> NVIDIA doesn't make robots. It makes everything around the robot. Three
> computers: one to train the brain, one to simulate the world, one that
> rides on the robot.

The pieces, in plain words:

- **Simulation:** Isaac Sim and Isaac Lab, on physics engines called PhysX
  and Newton.
- **Brains:** GR00T, a model that takes camera pictures and an instruction
  and puts out motor commands. Cosmos, a model that predicts video of the
  physical world.
- **Data:** tools that turn a few human demonstrations into thousands.
- **On the robot:** Jetson computers and the software that runs on them.

> And a lot of it is open. You can download the model. You can read the
> simulator.

Show the [report's](../nvidia/robotics-report-2026-11-15.md) map of the
portfolio for a few seconds.

## 3. Why "open" isn't safe

> Here's the catch. Open isn't one thing.

Three examples, each true today, from the report's licence table:

- The motion planner, cuMotion, may only be used on machines with NVIDIA
  GPUs.
- GR00T's code is Apache-licensed. Its weights are under a separate NVIDIA
  licence.
- Newton and Warp are open source, and their GPU path runs on NVIDIA's
  stack only.

> So nobody has to delete anything for this to close. The next release
> just doesn't ship. Or it ships with different terms. Or the place you
> download it from changes hands.

The things to name as open questions, not predictions:

- GR00T 2's code was promised for December.
- The Hugging Face acquisition is signed and not closed.
- Projects like Newton are open and are mostly staffed by one employer.

> I don't know which of those goes which way. Neither does anybody else.
> So we plan for the bad case and we're happy if it doesn't come.

> This should still be worth doing if NVIDIA keeps every open promise it
> made. Your robot should keep working when a download server is down.
> And you should be able to repair it without waiting for us.

The [official transition announcement](https://nvidianews.nvidia.com/news/nvidia-announces-ceo-transition)
says the board expects the roadmap and customer commitments to continue.
The closure scenario is our planning assumption, not an announced policy.

## 4. What we already have

> We've been writing our own machine learning library for weeks. It's
> called Psionic. It's in Coax, like everything else. And it already does
> robot things.

Say each as a plain fact, with its limit in the same breath:

- **We can command a robot arm, in maths.** Where the hand is, how to get
  it somewhere else, how hard each joint has to push. One arm, the Franka
  Panda, in simulation.
- **We run a real GR00T model.** NVIDIA's original weights, our code, on a
  Mac's GPU. No Python, no CUDA. One version of the model, one robot body.
- **We closed a control loop.** That model drove a simulated arm through a
  real benchmark. Six test cases, three successes.
- **We have the learning maths.** The same algorithm the duck was trained
  with, PPO, in its smallest form.

> Three out of six. I'm telling you the number because that's the point of
> doing it this way. You get the number.

On screen: [Psionic chapters 167 to 171](../../psionic/book/SUMMARY.md),
one title each.

## 5. The plan: two robots

> Two robots. One walks, one works.

The duck tests whether we can own the learning loop. The arm tests whether
we can deliver a useful, repairable kit. Share the work underneath; neither
robot has to wait for the other to exist.

### The duck

> The duck is the capstone. The whole pipeline from that keynote, on our
> stack. Describe a course. Build the robot in simulation. Train a brain
> that has never existed before. Check it. Put it on a real duck. Then do
> it with several ducks at once.

- Playing somebody else's trained policy doesn't count. We train our own.
- The first walking brain is small. It is not a language model, and the
  agent that helps design the course is not what moves the motors.
- Six gates, D0 to D5. We are at the start of the first.

### The arm

> The arm is the product. A robot arm on a table, a gripper, a camera, and
> parts you can print. First job: sort blocks into a tray.

- Start from an open design. The SO-101 is the candidate, not a decision.
- A boring controller first, then a learned one, and the learned one has
  to beat the boring one or we keep the boring one.
- What we'd sell: kits, spare parts, assembly, calibration, support. You
  can always print your own.

> The real test is the second builder. A finger wears out. They print or
> buy the replacement, fit it, recalibrate, and run the same check. If only
> we can make the repair work, we haven't finished the product.

> And one rule for both. A model can suggest a movement. An agent can
> suggest a movement. Only a small checked program sitting next to the
> motors decides whether the motor moves.

## 6. The tour

Record full screen. Use **Home → Microduck** in the OpenAgents app or
build the browser viewer from its [README](../../metaverse/microduck/README.md).
Verify the saved-run build's delivery status before recording steps 6–7.

| Step | What you show | What to say |
| --- | --- | --- |
| 1. The keynote | A short credited excerpt of the Berlin ducks | "This is what we're working toward." |
| 2. Our viewer | Native Metal on the phone, or Coax/WebGPU in the browser | "Same source assembly, in our engine. About 800,000 triangles, streamed rather than bundled." |
| 3. Articulation | Tap a part, choose a joint and pose it inside its source limits | "Those are the source joints and limits. This is visual articulation." |
| 4. The film | Smooth explosion, reassembly, then the labeled walk | "An authored animation. It shows the assembly; it isn't a walking policy we trained." |
| 5. The servo | Baseline, Delayed and Limited; show actual response curves | "One joint, a fixed base, teaching coefficients. The plots come from the servo calculation." |
| 6. A saved experiment | Save, Open latest, scrub and replay | "A recipe, complete queue and trace. The same file is verified on the phone and in the browser." |
| 7. A branch | Pause, move the cursor, Branch + continue | "It keeps the original run and carries the delayed targets into a new one." |
| 8. A refusal | Negative inertia or a corrupted saved file | "It refuses the change and keeps the admitted experiment. That's a checked boundary." |
| 9. Source and rights | Pinned revision, source hash and CC BY-NC-SA credit | "Noncommercial research geometry, credited to Pollen Robotics and NVIDIA DLI." |
| 10. What landed | Chapters 257–259 and the implementation roadmap | "The experiment, illustrative actuator and statistics are here. Training and the broader collector still have their own work." |
| 11. The arm | Chapter 167's checked kinematics | "And this is the arm, in Coax, checked." |

The native cached-download/replay path and a warm browser with blocked
network are automated checks. Record a physical phone airplane-mode visit
only after completing the owner check. The scoped agent comparison journey
is #13642; don't present it as delivered before its verification record exists.

If you cut it down, keep 2, 5 and 6.

For a still or a clip without opening a window:

```sh
coax/web/tool.sh shot http://127.0.0.1:9123/demos/microduck/ OUT.png 1920x1080 3000 --keep
```

## 7. Beyond robots

Keep this to thirty seconds. It is the bridge to later episodes.

> Robots are one branch. Underneath, it's the same problem everywhere:
> a lot of robot learning depends on one company's GPU libraries, with
> Python holding the pieces together.
>
> We're building another path. Some of our models already run on a CPU,
> an NVIDIA card or a Mac, in our own code. Each target still has to earn
> its own correctness and speed numbers. Next we want more GPUs to qualify.

One sentence each, if there's time:

- **Any GPU.** Vulkan and WebGPU as compute targets, so "4,096 robots in
  parallel" stops being an NVIDIA-only sentence. Proposed, not built.
- **Keep the weights.** A way to fetch a model by its hash from anyone who
  has it, so no single hub is the only copy. Proposed, not built.
- **Record the answers now.** Run today's reference code while it still
  runs and save what it outputs, so our rewrite can be checked against it
  for years.

The longer version is [After the open era](../nvidia/after-the-open-era-2026-11-15.md).
It is marked speculation; present it that way.

## 8. What's missing

Say this part plainly, as in 294 to 296.

> Here's what we don't have.

- **No robot has moved.** Not a motor, not a servo. Everything so far is
  maths and simulation.
- **Our own simulator isn't finished.** Joints and contact, the part where
  a foot hits the floor, is still ahead. The closed loop in section 4
  borrowed that from MuJoCo.
- **We haven't trained a walking brain.** The learning code works on a
  textbook problem. A neural network that keeps a duck upright is the
  work.
- **It's slow.** One test ran 0.45 seconds of robot time in about 10
  seconds of real time. A real robot can't wait for that.
- **No hardware is chosen.** No arm bought, no parts list approved.
- **The duck's shape isn't ours to sell.**
- **GPUs are still NVIDIA's.** Training big models needs them. We use them
  where we have them and make sure nothing depends on them.

> So: toward. Not there.

## 9. How to help

> If you're a developer, or you've got agents: point them at this.

- **Preserve one working route.** Keep the permitted model, source, licence
  text, build inputs and a small recorded example together. Tell us what it
  takes to run it with the original download service unavailable. A useful
  reproducible copy beats an untested pile of files.
- **Check our robot maths.** Every chapter lists what it's tested against.
  Find a case we don't cover.
- **If you know robots,** read the two plans and tell us what will bite
  us. Servos, backlash, falling over: we'd rather hear it now.
- **If you have a printer,** say so. The first things we make will be
  trays, mounts and fixtures, and we'll want a second person to build
  from the files, measure what fits and later try a documented repair.
- **If you have a GPU that isn't NVIDIA's,** you're who the portable path
  is for.

> The repo's open. PRs welcome.

## 10. Close

> Money, identity, a language, a brain. Now a body. We're not there yet,
> and I'll tell you each time how far we got. Open robots, built in the
> open, that you can own and fix. Stay tuned.

End card: `github.com/OpenAgentsInc/openagents`. No product link: there is
nothing to open yet.

## What to say by way of introduction

If the talk is one take of about 90 seconds, say these five things in this
order:

1. "Last time, what happened. This time, what we do: open robotics."
2. "NVIDIA built an enormous robot toolkit, and a lot of it is open,
   and open can stop."
3. "We already run their robot model on our own code, and we closed a
   simulated control loop: three of six."
4. "Two robots: a duck we train ourselves, and an arm you can print parts
   for."
5. "Nothing has moved yet. Mirror the open stuff, check our work, stay
   tuned."

Footage for each line: the Episode 296 end frame (1), the report's
portfolio map (2), chapters 169 and 170 (3), the viewer and a picture of an
SO-101 from its own repository (4), the roadmap (5).

## Check before recording

- **The Jensen story.** The [company announcement](https://nvidianews.nvidia.com/news/nvidia-announces-ceo-transition)
  on November 12 confirms his resignation and Colette Kress as interim CEO;
  it also states an expectation of continuity. Motives and the future of
  open releases are separate claims. This episode's engineering argument
  works without speculating about either.
- **"A lot of it is open."** True, with the limits in section 3. Don't say
  NVIDIA has closed anything. As of today it hasn't.
- **"We run a real GR00T model."** One pinned N1.7 checkpoint with a LIBERO
  head, on Metal, for one simulated Panda. Don't say "we run GR00T" without
  the "one".
- **"Three of six."** Six frozen LIBERO cases, three successes, with
  MuJoCo supplying the physics. It is not a success rate for anything else.
- **The walking is authored animation.** Keep its illustrative label visible.
  The servo is a separate fixed-base single-joint calculation. Neither is
  evidence of trained locomotion, contact balance or physical actuation.
- **The duck's licence.** Geometry is CC BY-NC-SA 4.0, credit Pollen
  Robotics and NVIDIA DLI. Put the credit on screen at step 2 and in the
  description. The streamed research model retains those terms independently
  of the application's software license.
- **The keynote clip.** It is NVIDIA's footage. Keep it short and credit
  it, or describe it and show the viewer.
- **"Landed today."** Chapters 257 and 258 were implemented on 2026-11-15;
  chapter 259 has shared statistics/normalization, with collection still
  unimplemented. Recount if you record later; the roadmap says what is next.
- **The refusal example.** [Source](../../psionic/examples/robot-actuator.coax)
  and [exact transcript](../../psionic/fixtures/robotics/actuator/example.out)
  show a numerical query refusing negative inertia. It does not exercise a
  motor, physical stop or hardware fault.
- **The SO-101** is a candidate. Don't announce it as our arm.
- **Don't promise dates or prices.** Neither plan has any.
- **Section 7** is proposals. Say "we want", not "we have".
- **The viewer build** needs the commands in its README, run on a build
  host. Do it before you sit down to record.

## Sources

- [NVIDIA's robotics efforts: full report](../nvidia/robotics-report-2026-11-15.md)
  and its [PDF edition](../nvidia/NVIDIA_Robotics_Report_2026-11-15.pdf)
- [An open robotics platform OpenAgents can carry forward](../nvidia/open-robotics-platform-2026-11-15.md)
- [Microduck: the complete robot-learning pipeline](../nvidia/microduck-pipeline-2026-11-15.md)
- [After the open era](../nvidia/after-the-open-era-2026-11-15.md) (speculation)
- [The Microduck assembly viewer](../../metaverse/microduck/README.md)
- [The small-robot implementation roadmap](../../psionic/docs/robotics/microduck-implementation-roadmap.md)
- Psionic chapters [167](../../psionic/book/ch167-command-a-robot.md),
  [169](../../psionic/book/ch169-execute-a-groot-policy.md),
  [170](../../psionic/book/ch170-close-the-simulated-control-loop.md),
  [171](../../psionic/book/ch171-learn-a-gait-in-many-worlds.md),
  [256](../../psionic/book/ch256-build-and-teach-a-small-robot.md),
  [257](../../psionic/book/ch257-a-robot-description-becomes-an-experiment.md) and
  [258](../../psionic/book/ch258-a-servo-that-behaves-like-the-motor.md), plus
  [259](../../psionic/book/ch259-rollouts-from-independent-worlds.md)
  (statistics component only)
- [NVIDIA's November 12 leadership announcement](https://nvidianews.nvidia.com/news/nvidia-announces-ceo-transition)
- [Episode 296, *Coup*](296.md)
- [GTC Berlin keynote](https://www.nvidia.com/en-us/on-demand/session/gtcberlin26-sb61112/),
  the development film from about 1:20:44
