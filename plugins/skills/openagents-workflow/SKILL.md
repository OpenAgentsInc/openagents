---
name: openagents-workflow
description: Write the script for a managed-agent workflow run that fans work across many agents. Use only when the person asks for a workflow, a fan-out, or multi-agent orchestration, or when a skill they invoked tells you to start a workflow run. A large task is not by itself a request.
---

# Workflow scripts

A workflow run (`wrun_`) fans work across many agents: to cover a large set, to check a result from more than one side, or to take on a job one context cannot hold. You write the script. The service runs it in the background. You can keep working, or end your turn.

This script is the plan for that run. It is not a plugin program, and it is not a file you add to the repository.

Start a run only when the person asked, in their own words, for a workflow, a fan-out, or multi-agent orchestration, or when a skill or command they invoked tells you to. Otherwise do the work yourself, or with one subagent. You can describe what a run would do and ask.

A job with more pieces than 25 is the usual shape for a run. Do a smaller job yourself. A run does not start another run. At most 10 runs are open in one session. After a run ends, read its ending notice, then start another on a later turn if the work still needs one.

Scout first when you do not yet know the list: find the files, the diff, or the sources, then run the script over that list.

## The script

Pass the script inline. Do not write it to a file first, and do not pass a saved name beside it. Call the `workflow` tool with the script, and pass `args` when the script reads them. The script is plain JavaScript, not TypeScript. Type annotations, interfaces, and generics fail to parse. The body is async, so use `await` directly. It has JavaScript built-ins and no filesystem or host APIs of its own.

Every script begins with `export const meta = { ... }`. That object is a literal: no variables, calls, spreads, or interpolation.

```js
export const meta = {
  name: 'find-flaky-tests',
  description: 'Find flaky tests and propose fixes',
  phases: [
    { title: 'Scan', detail: 'grep test logs for retries' },
    { title: 'Fix', detail: 'one agent per flaky test' },
  ],
}
```

`name` and `description` are required. `whenToUse` and `phases` are optional. Write one `phases` entry per `phase()` call, and use the same title in both places. A `phase()` call with no matching entry still gets its own progress group. A phase entry may set `model`. Names are cut to 64 characters and descriptions to 256.

The script does not read the clock and does not draw random numbers. `Date.now()`, `Math.random()`, and `new Date()` with no argument throw. Pass a timestamp in through `args`, or stamp a result after the script returns. Put the index in the prompt or the label when two calls must differ.

## Calls

`agent(prompt, opts)` starts one thread.

- With no `schema`, it returns that thread's final text.
- With `schema`, a JSON Schema, it returns the validated object. The root is `{ type: 'object', properties, required }`, and `required` is a subset of `properties`. An unsatisfiable schema throws at `agent()`.
- It returns `null` when the person skips that thread, or the thread dies after retries. Filter with `.filter(Boolean)` before you read the values.
- `label` is the display name.
- `phase` assigns the progress group. Set it on calls inside `pipeline` and `parallel`. The same title is the same group.
- `model` overrides the model. Omit it. The thread inherits the session model.
- `effort` is `low`, `medium`, `high`, `xhigh`, or `max`. Omit it to inherit the session effort. Use `low` for a mechanical stage.
- `agentType` names a predefined agent. Omit it and the thread uses the session agent's model and tools.
- `agent` is `{ name, description, system }` for an inline agent, when the session allows inline agents. Pass `agent` or `agentType`, not both.

Threads share the session environment and the session files.

`pipeline(items, ...stages)` runs each item through every stage on its own. There is no barrier. One item can be in a later stage while another is still in an earlier stage. This is the default for work with more than one stage. The wait is the slowest item, not the sum of the slowest stage. Each stage receives `(prev, item, index)`. A stage that throws drops that item to `null` and skips its remaining stages. A failed item does not fail the others.

`parallel(thunks)` runs the thunks together and waits until all of them finish. This is a barrier. A thunk that throws becomes `null`. The call itself does not reject. Use it only when the next step needs every result.

`phase(title)` groups the `agent()` calls that follow.

`log(message)` posts a progress line. When you choose to cover only part of a set, say in `log` what you left out.

`args` is the value passed with the script. Pass an array or an object as a JSON value. A JSON string is one string, and `.map` on it throws.

`budget` is `{ total, spent(), remaining() }`. `total` is the session's `amount_sats` cap, or `null` when the session has no cap. `spent()` is the list-priced sats the session has spent, shared by every thread and every open run. `remaining()` is `max(0, total - spent())`, or `Infinity` when there is no cap. The cap is a hard stop. Once `spent()` reaches `total`, the run pauses and further `agent()` calls do not start. Guard a loop on the cap:

```js
while (budget.total && budget.remaining() > 1_000) {
  // one more pass
}
```

`return` the value the run finishes with. On your next turn you get an ending notice. `completed` means the script finished, not that the work passed.

## Pipeline is the default

Use `pipeline` for multi-stage work. A barrier belongs between stages only when the next stage needs the whole set:

- Merge duplicates before a more expensive stage.
- Stop when the count is zero.
- The next prompt has to mention the other findings.

Flatten, map, and filter inside a stage:

```js
pipeline(items, stageA, (row) => transform([row]).flat(), stageB)
```

Separate stages are still a pipeline. A barrier makes the fast items wait for the slowest one in that stage.

## Limits

64 threads work at once in one run. Further `agent()` calls wait. A call can take more items than 64. Over the run's life the cap is 1,000 threads, or `max_agents` when the session set a lower one. Past that cap the run ends with `thread_limit_error`. Threads already open can carry the count past the cap, so stop before you reach it. A run lasts 24 hours unless a shorter lifetime was set. A pause might not freeze that lifetime. When a step drops items to stay inside a limit, the ending notice names how many and why.

## Patterns

Review streams. Verification of one slice starts while another slice is still in review:

```js
export const meta = {
  name: 'review-changes',
  description: 'Review changed files, then verify each finding',
  phases: [{ title: 'Review' }, { title: 'Verify' }],
}
const VERDICT = {
  type: 'object',
  properties: { refuted: { type: 'boolean' } },
  required: ['refuted'],
}
const results = await pipeline(
  DIMENSIONS,
  (d) => agent(d.prompt, { label: 'review:' + d.key, phase: 'Review', schema: FINDINGS }),
  (review) => parallel(review.findings.map((f) => () =>
    agent('Try to refute: ' + f.title, { label: 'verify:' + f.file, phase: 'Verify', schema: VERDICT })
      .then((v) => ({ ...f, verdict: v }))
  ))
)
return { confirmed: results.flat().filter(Boolean).filter((f) => f.verdict && !f.verdict.refuted) }
```

A barrier is right when you merge before the expensive stage:

```js
const all = await parallel(DIMENSIONS.map((d) => () => agent(d.prompt, { schema: FINDINGS })))
const deduped = dedupe(all.filter(Boolean).flatMap((r) => r.findings))
const verified = await parallel(deduped.map((f) => () => agent(verifyPrompt(f), { schema: VERDICT })))
```

Loop until two rounds in a row add nothing. Deduplicate against everything you have already seen:

```js
const seen = new Set()
let dry = 0
while (dry < 2 && (!budget.total || budget.remaining() > 1_000)) {
  const found = (await parallel(FINDERS.map((f) => () =>
    agent(f.prompt, { phase: 'Find', schema: BUGS }))))
    .filter(Boolean).flatMap((r) => r.bugs)
  const fresh = found.filter((b) => !seen.has(key(b)))
  if (!fresh.length) { dry += 1; continue }
  dry = 0
  fresh.forEach((b) => seen.add(key(b)))
}
```

With no cap, `remaining()` is `Infinity`. Guard on `budget.total` or the loop runs until the thread cap.

Pick one check and name it in the session system. Skip the run when the job is too small for it.

- Cited research. Search several angles, fetch the sources, then try to refute each claim. Keep a claim that survives, and cite a source for every claim you keep.
- Code review. Read the tree in slices, read each slice again, then prove a candidate by writing a failing test. Drop a candidate the proof does not support.
- A list with a second pass. One agent per item, then one review of every report before the combined answer.

Several independent votes on one claim. Give each voter a different lens when the finding can fail in more than one way.

```js
const votes = await parallel([0, 1, 2].map(() => () =>
  agent('Try to refute: ' + claim + '. If you are unsure, mark it refuted.', { schema: VERDICT })))
const survives = votes.filter(Boolean).filter((v) => !v.refuted).length >= 2
```

A wide solution space can use a panel: several independent attempts, scored together, then one synthesis that keeps the strongest idea from the attempts that lost.

## Continue a run

The service keeps each step's result. Submit the same script to continue. A step whose prompt and options are unchanged is reused, and the run continues at the first step that differs. The same script and the same `args` reuse every step.
