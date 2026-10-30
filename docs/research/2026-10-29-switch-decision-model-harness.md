# Switch: an agent loop for decision models, and what we should take from it

- Source: Onur Solmaz, [`osolmaz/switch`](https://github.com/osolmaz/switch) (MIT,
  TypeScript, npm `@osolmaz/switch`), and his launch post of 2026-10-29.
- Read at commit `bd26521` (2026-10-30 UTC): the README, `docs/MATH.md`,
  `docs/DECISION_MODEL_API.md`, `docs/DOMAINS.md`, the first judge run
  record, the three examples and `src/core/pick.ts`. About 2,100 lines of
  core TypeScript.
- Nothing here was run. Numbers are the author's, from his repository.

## The idea in one paragraph

A decision model (Jev, Clef, Elephant, Mammoth) reads a JSON state and answers
typed questions (yes/no, choice, score) with a probability per answer. It writes
no text, so it can't emit tool calls and can't drive a normal coding-agent
harness. Switch turns that around. Your code lists the tool calls that make
sense right now, and the model only picks one. A **domain** gives four
functions:

- `initial`: the first state of a case;
- `questions(state)`: the questions to ask now (none means the run is over);
- `transition(state, answers)`: the next state;
- `view(state, budget)`: what the model reads, cut to fit its window.

A run is a Markov decision process with the model as the policy. Every step is
recorded, so a run can be replayed with no model calls, or forked at step *n*
and continued by a different model.

## What is in the repository

- **The loop** (`src/core/loop.ts`) runs a domain until `questions` returns
  nothing, a step limit (a safety net the domain sets), or a failed request.
- **The pick** (`src/core/pick.ts`) is the core helper. A choice function
  returns the candidate actions, each with a key, a text and an optional
  priority. The pick keeps at most `limit` of them (lowest priority value
  first, in the order given), sends them under short labels (`o1`, `o2`, …)
  so long keys like chess moves don't bloat the request, and records how
  many were cut. Duplicate keys or texts are errors. A "stop" choice is just
  another choice, usually with a priority that keeps it from being cut.
- **The view budget** is a fixed share of the window (90% by default) minus
  the questions. When the server still says a request is too long, Switch asks
  the view for a smaller one by a fixed factor and retries. The trace records
  what was left out.
- **Models** go through pi-ai's classifier interface, which covers TypeSafe's
  System One API (`POST /v1/systemone`, also served by llama.cpp and
  Cloudflare), OpenAI's `POST /v1/decisions`, and chat models used as
  classifiers through next-token probabilities.
- **A harness CLI** (`run`, `replay`, `replay --fork N`) and a browser viewer
  that plays a run back step by step.
- **Three domains:**
  - **Rubric judge.** Grades a saved answer against a rubric by reading parts
    of files, looking at page images, searching for terms, and stopping when
    it can grade each item.
  - **Chess.** Plays against random moves, Stockfish or itself. The pick is
    the list of legal moves.
  - **WikiRace.** TypeSafe's Jev launch demo: reach a target article by
    following links. A page with more than 200 links is split into sections
    first, because Jev takes at most 255 answers per question.

## The result worth noting

On 100 saved answers to legal tasks (1,288 rubric items, labels from strong
models plus a review), the rubric judge on Jev agreed with the labels on 94.6%
and 95.0% of items in two runs. Chat-model judges in an agentic harness on the
same items got 94.6% (DeepSeek-V4.1-Flash) and 93.5% (GPT-6 Luna). One run
was 319 calls at a median of 0.29 s, 3.2 million input tokens, about $0.13 at a
listed Jev price. Jev stopped on its own on 97 of 100 answers, after 1.2 steps
on average.

Caveats from the author:

- These answers had been seen in earlier judge work, so this isn't a held-out
  result.
- In his post he says Jev still isn't good enough to replace GPT 6.1 Sol as
  the judge in his domain.
- Jev is weak at chess.

His bet is that traces from a stronger model can fine-tune an open decision
model for one domain. That model would then be trained inside the same harness
it runs in.

## How it maps onto what we already have

- **The Lab is a one-step Switch domain.** The desktop Lab
  (`apps/openagents-desktop/zeron/lab.coax`) asks Elephant five questions
  once per message (which agent, how hard, how much reasoning, which model,
  whether it needs this Mac) and then hands the message to the chosen engine.
  Switch's shape would let the decision model keep going after that: read a
  file, run a search, look at a page, stop. Each step's choices would come
  from our code, not from generated text.
- **We serve the same wire format.** Psionic serves `POST /v1/systemone` for
  Elephant (Metal on the Mac, CUDA on the RTX 4080), and our client is
  `coax/lib/systemone`. pi-ai's System One adapter should be able to point at
  our door. That's unverified.
- **The house rule still applies.** Product decisions go through our Decisions
  API, Clef first, with Elephant and Mammoth behind it. New code shouldn't call
  Jev directly. A Switch-style loop in our product would call our door, not
  TypeSafe's.
- **Calibration is still our job.** `psionic/docs/decisions.md` already says a
  probability is never permission: thresholds come from our own labelled data,
  and destructive actions need a high bar. Switch passes the probabilities
  through but doesn't decide what they allow. A loop over our tools needs
  those thresholds on any choice with side effects, plus a recorded outcome
  for each decision so the thresholds can be refitted.

## What we should take

1. **The pick, as a Coax library.** A small `coax/lib/decide` (or an addition to
   `coax/lib/systemone`) with Switch's rules: a capped choice list with
   priorities, short labels with a key map in the trace, a cut count, a stop
   choice that is never cut, and errors on duplicate keys or texts. That's
   about 150 lines in Switch. We should port the idea in Coax rather than
   depend on the TypeScript package, as with other reference code.
2. **Traces with replay and fork.** Record every state, question, answer and
   distribution of a decision loop in one JSONL file. Replay needs no model,
   and fork lets a second model take over at step *n*. That gives regression
   tests that need no GPU, and the comparisons (Elephant against Mammoth
   against Clef on the same states) that the calibration loop in
   `psionic/docs/decisions.md` asks for.
3. **A judge for our own evals.** The rubric-judge pattern fits the checks we
   now run with chat models or by hand: whether a Coder run met its issue's
   acceptance criteria, whether a screenshot matches its Zeron reference. It
   reads only the evidence it needs, step by step, at decision-model prices.
   We should measure it against our own labels before trusting it, as the
   author did.
4. **A view budget that shrinks and retries.** Elephant's questions hold 512
   tokens today (`LAB_MESSAGE_MAX` in `lab.coax`). A view function with a
   budget, and a smaller retry when a request is refused, is the general form
   of the truncation the Lab does by hand.
5. **Domain traces as training data.** Psionic serves and trains Elephant-class
   models. Traces from a stronger chooser on one narrow domain, such as
   routing or judging Coder runs, are the data for a fine-tune that runs in
   the same loop.

## What not to take

- **A general decision-model coding agent.** The author's point: a decision
  model can only pick from what code offers, so there is no general pure
  harness, only domain-specific ones. Coding stays with Claude, Codex and
  gpt-oss.
- **Jev as a dependency.** Our doors are Clef, Elephant and Mammoth.

## Next step if we pursue it

Port the pick and the trace format into Coax. Then extend the Lab from one
routing step to a short loop where Elephant can read a file or search the repo
before routing. Score it against the current one-step router on a labelled set
of messages, using replays so the comparison needs no new model calls.
