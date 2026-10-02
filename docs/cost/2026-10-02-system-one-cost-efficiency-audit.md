# System One cost efficiency: what we measured, and how to make it the default

2026-10-02. An audit for the owner of what was learned from 2026-09-22 to
2026-09-28 about making Fable and Claude Code runs cheaper and faster with
System One (code plus Jev), and of how much of it the current harness uses.
Every number below is copied from a committed report or a retained
conversation, with its source. Nothing was rerun for this audit.

## 1. The reminder

Between episodes 287 and 288 we spent about a week on one question: can
cheap typed judgments (Jev, about 200 ms and a fraction of a cent each) and
plain code make a frontier coding model do the same work for less money and
less waiting? Three results came out of it.

**Same model, same answers, less money.** On eight Terminal-Bench tasks,
three trials each, Coder put Jev probes in front of a lean Opus 5.5 (six
tools, low effort, five-minute prompt cache, a briefing instead of open
exploration) and passed 24 of 24, the same as Claude Code on Opus 5.5
alone, for **$0.43 against $1.09 (61% less) and 192 s against 306 s (37%
less agent time)**. The best single configuration was 63% cheaper and 32%
faster on the original four-task panel, and 57% cheaper and 48% faster on
four newer tasks. A later matched pilot showed that most of that saving
came from the lean configuration (effort, tools, system prompt, cache
lifetime), not from the controller alone: with executor settings held
fixed, Coder saved only 7.6% and passed one fewer of six.

**A Fable run made cheaper and faster.** On 2026-09-27, Coder ran Jev
before handing a Terminal-Bench 4 task to Fable 5.1 at low effort. Jev
chose which 5 of 12 knowledge entries Fable would read and flagged 3 of 6
requirements as easy to miss. Fable then passed `fin-saccr-rwa` (24 of 24
verifier tests) for **$0.9429 in 149.5 s, against Fable 5.1 low's own best
public run at $1.2246 and 222.5 s**: 23% cheaper and 33% faster, with Jev
costing $0.00018. Frozen and rerun on 14 tasks it was not tuned on, the same
arm beat Fable 5.1 low on both cost and time on 4 of 14, all on tasks with
their own knowledge entries, and twice by under 3%. It reproduces, but not
reliably.

**A cheap model doing Fable's job.** Microcoder (short GPT-6 Luna sessions,
code-owned control, Jev briefings, executable checks) matched Fable 5.1
low's 5 of 5 on `embedding-drift-monitor` at **$0.0153 against $0.88 per
pass (about 58 times cheaper), but about 1.9 times slower** (in sample,
tuned on that task). Out of sample on Terminal-Bench 2.1 with knowledge
off, it had 30 confirmed wins against Fable 5 xhigh's cost per trial; the
median pass cost 2.9% of Fable's, $0.0075, in 1:49. Its weakness is
reliability: its first run passed 48% of the 65 tasks, where Fable 5 xhigh
passed 92% of its trials.

The principle under all three is the one from episodes 286 and 287: code
owns control, Jev judges narrowly, and the generative model only generates.
Every decision moved out of the expensive model's turns and into code or a
typed judgment made the run cheaper; the expensive model only has to do the
part that is irreducibly generative.

## 2. The grids

### 2a. The Fable grid: Coder plus Jev makes a Fable 5.1 low run cheaper and faster

Verbatim from a Claude Code session on this Mac, not the Coder box:
`~/.claude/projects/-Users-christopherdavid-work-openagents/803a9035-5d7c-415b-8a0b-435e9f5fbcb5.jsonl`,
assistant message at 2026-09-27T20:24:52Z. The attempts themselves ran on
coderos-4080 (working directory `~/fable-delegate-9746/`, still present,
not modified by this audit).

> **#9746 is won, and I checked this one directly in the retained trace:**
>
> | | Fable 5.1 low's best | Attempt s7a1 (Coder + Jev → Fable 5.1 low) |
> |---|---|---|
> | Cost | $1.2246 | **$0.9429** (Fable $0.9427, Jev $0.00018, no unknown calls) |
> | Time (whole trial) | 222.5 s | **149.5 s** |
> | Result | pass | **pass**: all 24 verifier tests |
>
> - **Jev was actually in the loop:** the usage record shows 1 Jev decision. Before handing off, Jev judged 12 candidate knowledge entries and kept the 5 task-specific ones, scoring every generic entry at 0.33 or lower. It also flagged 3 of 6 requirements as easy to miss. The briefing carried both sections, and every probability is retained.
> - **How much to read into it:**
>   - It's one win, not a pass rate: 1 of 3 attempts with Jev deciding the briefing, and 1 of 13 across all series.
>   - It's on `fin-saccr-rwa`, a task Coder had already worked on (in-sample), using knowledge entries written from those earlier runs.
>   - It's tuned: Jev's second question set, and one briefing sentence from series 5, were written after seeing earlier results on this task.
>   - Jev's five kept entries were the same five a plain keyword ranking picked; Jev decided their order and what to drop. The flagged requirements came from Jev alone.
>   - Costs are Claude list-price figures on a subscription, and the Fable reference runs were on another host with an older Claude Code.

The reproduction grid, same session, 2026-09-28T01:26:22Z:

> | Task (pass 2) | Cost vs Fable 5.1 low's cheapest win | Time vs its fastest win |
> |---|---|---|
> | `coq-block-bound` | $2.10 vs $3.28 | 420 s vs 623 s |
> | `mp-checkpoint-consolidation` | $2.37 vs $3.31 | 407 s vs 788 s |
> | `sound-change-cascade` | $3.75 vs $3.85 | 687 s vs 878 s |
> | `gsea-proteomics` | $0.68 vs $0.69 | 143 s vs 173 s |
>
> **Verdict (#9776, closed):** the Jev-briefed Fable delegate beat Fable 5.1 low on **4 of 14** tasks it was never tuned on. That's 0 of 14 in pass 1 and 4 of 14 in pass 2, and there were no faults.

**How the numbers were measured.** Cost is Claude Code's own list-price
`total_cost_usd` for the Fable session plus Jev's metered price
(`jev-1.13.0`, $0.042 per million input tokens), from each attempt's usage
record; a run cut off at its deadline has unknown cost and can't count as
a beat. Time is whole-trial wall time in Harbor. The bar is Fable 5.1
low's cheapest and fastest *winning* public trajectories on the same task
(`fable_reference()` in
`bench/terminal-bench/studies/2026-09-26-out-of-sample/study.py`). A beat
needs reward 1, cost below the cheapest win, and time below the fastest
win. Bars and deadlines were committed before the first run
([`e0414c3356`](https://github.com/OpenAgentsInc/openagents/commit/e0414c335603f7e2aa755074abae9b7c686335a4)).
Per-attempt numbers: [`attempts.json`](https://github.com/OpenAgentsInc/openagents/blob/d5852d141ff8733d5ea7a523153169b02586dff4/bench/terminal-bench/experiments/2026-09-27-fable-delegate-repro/attempts.json).

### 2b. The Claude Code grid: same model, same pass rate, 61% cheaper

Verbatim from the Coder box: coderos-4080,
`~/.claude/projects/-home-christopherdavid-openagents/4d9ec5a2-be11-427a-bc5a-9f3040a354c4.jsonl`,
assistant message at 2026-09-23T02:18:22Z.

> **The headline:** on eight Terminal-Bench tasks, three trials each, Coder with Jev passed everything Claude Code on Opus 5.5 passed, at 61% lower cost and 37% less agent time.
>
> | | Trials passed | Cost | Agent time |
> | --- | --- | --- | --- |
> | Claude Code on Opus 5.5 alone | 24/24 | $1.09 | 306 s |
> | **Coder: Jev probes → lean Opus 5.5** | **24/24** | **$0.43** | **192 s** |
>
> Costs and times are sums of the per-task means across the eight tasks.
>
> It's the same model with the same pass rate. What changed is what surrounds it:
> - Jev probes the environment and picks the evidence in 1 to 5 seconds, for fractions of a cent.
> - Opus starts from that briefing instead of exploring first.
> - It runs with six tools, low effort, and a five-minute prompt cache.
>
> On the cheap end, Coder with GPT-6 Luna finishes the eight tasks for about 3 cents. It isn't reliable on every task yet: it passed 19 of 24 trials.

The per-task grid behind it, same session, 2026-09-22T21:32:30Z:

> | Task | Opus 5.5 direct (n=3) | GPT-6 Luna direct (n=1) | **Jev-probe → Luna** (n=3) | **Jev-probe → lean Opus, low effort** (n=2) |
> |---|---|---|---|---|
> | `fix-git` | $0.1216 · 23.5 s | $0.0052 · 65.3 s | $0.0032 · 36.0 s | $0.0560 · 14.4 s |
> | `build-cython-ext` | $0.3481 · 111.8 s | $0.0174 · 204.7 s | $0.0119 · 185.8 s | $0.1469 · 86.6 s |
> | `headless-terminal` | $0.1456 · 49.1 s | $0.0032 · 67.7 s | $0.0032 · 65.2 s | $0.0597 · 20.8 s |
> | `fix-code-vulnerability` | $0.0400 · 11.4 s | $0.0079 · 49.7 s | $0.0035 · 31.1 s | $0.0647 · 11.0 s |
> | **All four** | **$0.655 · 196 s** | **$0.034 · 387 s** | **$0.022 · 318 s** | **$0.327 · 133 s** |

And the final configuration grid, 2026-09-22T23:54:27Z (also committed in
[`winning-runs-analysis.md`](https://github.com/OpenAgentsInc/openagents/blob/72ed33efda93434acd082a1ebcb33046edbf90b2/docs/terminal-bench/winning-runs-analysis.md#probe-v2-on-the-five-minute-cache)):

> | Configuration | Original 4 tasks | New 4 tasks |
> | --- | --- | --- |
> | Opus 5.5 direct | 12/12, $0.6554, 195.9 s | 12/12, $0.4309, 110.5 s |
> | **v3 → Luna** (the cheapest) | 11/12, **$0.0198**, 275.2 s | 8/12, $0.0087, 145.0 s |
> | **v2 → lean Opus, low effort, five-minute cache** (best Opus) | 12/12, $0.2433, 133.8 s | 12/12, $0.1849, 57.8 s |

**How the numbers were measured.** Harbor 0.22.0 on coderos-4080, same task
pins for every arm. Opus cost is Claude Code's own list-price figure; Luna
cost is the price sheet applied to Codex's token counts (`list_price`, a
subscription login, not a bill). Jev is metered separately and included.
Time is agent time per task, summed over per-task means across three trials.

### 2c. The Microcoder grid: a cheap model matches Fable's pass rate

Verbatim from coderos-4080,
`~/.claude/projects/-home-christopherdavid-openagents/86bb4789-a64a-47d8-bbb3-abeed1032001.jsonl`,
2026-09-26T01:19:09Z (the episode 288 prep's "72 times cheaper" was
corrected in the same session to 54, then 58 times with v19-fire):

> | Agent | Passes | Median time | Cost per pass |
> | --- | --- | --- | --- |
> | Coder One `microluna-v19-fire` (Luna) | 5 of 5 | 5 min 26 s | $0.0153 |
> | Fable 5.1 low | 5 of 5 | 2 min 55 s | $0.88 |
>
> That's about 58 times cheaper and about 1.9 times slower.

And the knowledge-assisted extension, 2026-09-26T07:56:36Z:

> | Task | Before | With the relay's entries | Each pass against Fable 5.1 low's winning runs |
> |---|---|---|---|
> | `gsea-proteomics` | 0 of 10 | **4 of 4** | $0.05–0.07 against its cheapest $0.69; one (3:03) beat one of its three winning times |
> | `fin-saccr-rwa` | 0 of 6 | **4 of 4** on the latest entry | $0.04–0.08 against its cheapest $1.23; 2:48 beat all three winning times, 3:42 tied the fastest |
> | embedding-drift-monitor | 0 | 8 of 9 (earlier) | every pass cheaper, two faster |

All three are in sample: the task was the development task, or the
knowledge entry was written from that task's failures.

## 3. Evidence

### Episode 288 and its moved and deleted versions

`docs/transcripts/288.md` was the Coder Gym session until 2026-09-29, when
[`2f4483c4f8`](https://github.com/OpenAgentsInc/openagents/commit/2f4483c4f83516355dc166e28259c30f425720e7)
renamed it to `288-prep.md` and added `288-draft.md` (Test-Time
Capabilities). On 2026-09-30,
[`ff8b51b52a`](https://github.com/OpenAgentsInc/openagents/commit/ff8b51b52af35d6dee43bbd98bbd156d3ff0d298)
deleted both and kept `288.d.md` as the final `288.md` (Three DevDays
Later). Read the deleted versions at
[`288-prep.md`](https://github.com/OpenAgentsInc/openagents/blob/2f4483c4f83516355dc166e28259c30f425720e7/docs/transcripts/288-prep.md)
and
[`288-draft.md`](https://github.com/OpenAgentsInc/openagents/blob/375cef66efee9d8352a5263a36d29d83c73ef686/docs/transcripts/288-draft.md).

From the prep (Gym session, 2026-09-24 to 2026-09-26), the lines that matter:

> **[12:21]** So Coder's standing goal is to be the cheapest and best on every task. The last two days of Terminal-Bench work spent most of their effort on configuration. Which executor, which effort level, when to escalate […] Those levers are real, but they share a ceiling. They move work between expensive models rather than removing. […] The system one algorithms are the unlock deterministic code and cheap type Jev judgments decide what Luna sees, what it works on next, and whether its result holds. Routing cleverness is secondary.

> **[15:51]** So the thesis, a coding agent should make every decision it can with deterministic code, it should use Jev only for narrow type calibrated judgments and use a generative model only for the part that's irreducibly generative writing code and tests. […] When done is a program state rather than a model's opinion, a cheap model iterating many times can reach it. So pass rate rises and cost falls together.

> **[44:32]** Can we complete it? Can we do it more cheaply? Can we do it more quickly? Can we do it more cheaply and more quickly? That's the holy grail. If we can do it more slowly, but 10 to 30 times cheap more cheaply with the same quality, that's still an amazing result.

> **[01:28:27]** writer rounds. 85% of each round is model latency. Jev's verified 10 seconds in total.

> **[01:34:12]** a run where Coder, in our new MicroLuna custom harness, got the same success rate as Fable was 72 times cheaper and three times slower. That's acceptable if it's that cheap and reliable enough. You can multiply that out and have a bunch of those going. That would be the basis for pulling workloads away from Fable […] (Correction: 54 times cheaper per pass, not 72.) Jev was about 8% of each trial's cost.

> **[01:50:45]** Goal: take Fable 5.1's public winning trajectories on the Terminal-Bench embedding-drift-monitor task and map every move Fable made to a System One component, code, Jev as a typed decision model, or lightweight MicroLuna steps. See what configuration of predefined components could reach the same results, ideally faster and certainly cheaper.

From the draft (Test-Time Capabilities, 2026-09-29), now superseded:

> We put Jev in front of harnesses like Claude Code, same models, and got the same results on the same Terminal-Bench tasks faster and cheaper. The best Opus configuration we found, Jev probes in front of a lean Opus 5.5, passed 24 of 24 trials on the four-task development panel, 63 percent cheaper and 32 percent faster than Claude Code on Opus alone, and 57 percent cheaper and 48 percent faster on four newer tasks.

> one declared Terminal-Bench 4 attempt passed fin-saccr-rwa for 94 cents in about 150 seconds, under Fable 5.1's cheapest and fastest. It was in-sample and tuned, and across seven series only 2 of 13 attempts beat the bar. Delegation is a capability to measure, not a guaranteed win.

> Judgment budget: deciding how to answer has to cost far less than answering.

Note: the draft's "24 of 24 trials on the four-task development panel"
conflates two results: 24 of 24 is the eight-task total; the panel was 12
of 12.

### Reports and commits

| Date | Commit | What |
| --- | --- | --- |
| 2026-09-22 | [`46fd94bf74`](https://github.com/OpenAgentsInc/openagents/commit/46fd94bf7480d8977a1da4b8ff0511747db921ea) | [Winning-runs analysis](https://github.com/OpenAgentsInc/openagents/blob/main/docs/terminal-bench/winning-runs-analysis.md): cheapest and fastest Coder One runs |
| 2026-09-22 | [`72ed33efda`](https://github.com/OpenAgentsInc/openagents/commit/72ed33efda93434acd082a1ebcb33046edbf90b2) | Probe v2 on the five-minute cache: 63% cheaper, 32% faster (#9535) |
| 2026-09-23 | [`48498ee5c3`](https://github.com/OpenAgentsInc/openagents/commit/48498ee5c35f9027477952b6e3ee8dd5c96af574) | [Development results](https://github.com/OpenAgentsInc/openagents/blob/main/docs/terminal-bench/development-results.md) and TB4 results condensed |
| 2026-09-23 | [`62bb353abd`](https://github.com/OpenAgentsInc/openagents/commit/62bb353abde1d2db74b957b9c1faa49f76f11379) | [Coder One against Claude Code on TB4](https://github.com/OpenAgentsInc/openagents/blob/main/docs/terminal-bench/2026-09-23-coder-one-vs-claude-code-tb4.md): 11 of 26 against 9 of 26, 45% less money, 38% less agent time |
| 2026-09-23 | [`0586f4ebc6`](https://github.com/OpenAgentsInc/openagents/commit/0586f4ebc6e9edb796ec47c2b55820b956029924) | [Matched Opus controller pilot](https://github.com/OpenAgentsInc/openagents/blob/main/docs/terminal-bench/2026-09-23-matched-opus-controller.md): controller alone saves 7.6%, one fewer pass |
| 2026-09-23 | [`9557f41ace`](https://github.com/OpenAgentsInc/openagents/commit/9557f41ace1f923ad2a9834c398e67ca660e514e) | [Gym head-to-head replay](https://github.com/OpenAgentsInc/openagents/blob/main/docs/gym/head-to-head.md) against public Fable/Claude Code attempts |
| 2026-09-24 | [`a147c5d515`](https://github.com/OpenAgentsInc/openagents/commit/a147c5d515064036a9cab6d4a71589782cf4e9e0) | [The determinism thesis](https://github.com/OpenAgentsInc/openagents/blob/main/docs/coder/design/thesis.md) |
| 2026-09-24 | [`cd7245a59b`](https://github.com/OpenAgentsInc/openagents/commit/cd7245a59b5117daff4225f9136a80db29855f0b) | [The Luna pivot](https://github.com/OpenAgentsInc/openagents/blob/main/docs/coder/design/luna-pivot.md) |
| 2026-09-24 | [`05690ec458`](https://github.com/OpenAgentsInc/openagents/commit/05690ec4585c384fa5eba3586404ff65f9ebb182) | [Strategy fingerprints](https://github.com/OpenAgentsInc/openagents/blob/main/docs/terminal-bench/2026-09-24-strategy-fingerprints.md): what Fable's winners do (505 trajectories, Jev placement for $0.81) (#9586) |
| 2026-09-25 | [`f68c696a35`](https://github.com/OpenAgentsInc/openagents/commit/f68c696a35e25e892f096f03dc9a4f059d6863ae) | [Microluna v13 embedding trials](https://github.com/OpenAgentsInc/openagents/blob/main/docs/terminal-bench/2026-09-25-microluna-v13-embedding-trials.md), step by step |
| 2026-09-25 | [`c80dcfe443`](https://github.com/OpenAgentsInc/openagents/commit/c80dcfe4431a129ddd82d1c6ead00976bb8523b9) | v19-fire 5 of 5 at about 1/58 of Fable's cost, in [TB4 results](https://github.com/OpenAgentsInc/openagents/blob/main/docs/terminal-bench/tb4-results.md) |
| 2026-09-25 | [`da002203e6`](https://github.com/OpenAgentsInc/openagents/commit/da002203e6a9d550118dc84f54af72b6f65e675b) | Microcoder's knowledge-assisted pass at 1/33 of Fable's cost (#9670) |
| 2026-09-26 | [`2961e7a762`](https://github.com/OpenAgentsInc/openagents/commit/2961e7a7627af0cd54bf0790be302e1ba0e89126) | [TB2.1 out-of-sample results](https://github.com/OpenAgentsInc/openagents/blob/main/docs/terminal-bench/2026-09-26-tb21-oos-results.md): 30 confirmed cost wins over Fable 5 xhigh (after correction) (#9683) |
| 2026-09-26 | [`f29611f211`](https://github.com/OpenAgentsInc/openagents/commit/f29611f211747884b4f508db80634d3f6df2a5e9) | [Beat Fable showcase](https://github.com/OpenAgentsInc/openagents/blob/main/docs/coder/beat-fable-showcase.md) (#9680) |
| 2026-09-27 | [`24440ad574`](https://github.com/OpenAgentsInc/openagents/commit/24440ad574c7c103561c0e010bf6b00a346a1b99) | Fable 5.1 low delegate arm for Terminal-Bench |
| 2026-09-27 | [`622dff64bc`](https://github.com/OpenAgentsInc/openagents/commit/622dff64bcedb52b19a8df58de0b478d0a4dfe7a) | Jev chooses the delegate's knowledge and flags requirements (#9746) |
| 2026-09-27 | [`bb38f2751c`](https://github.com/OpenAgentsInc/openagents/commit/bb38f2751c123f55fa89052d281c87399bcd04d5) | Jev question set v2 for the briefing (the s7a1 artifact) |
| 2026-09-27 | [`540a8ba1fb`](https://github.com/OpenAgentsInc/openagents/commit/540a8ba1fb769e08ea975af246bf5daba44f5ec6) | [Fable delegate report](https://github.com/OpenAgentsInc/openagents/blob/main/docs/terminal-bench/2026-09-27-fable-delegate.md): s7a1 beat Fable 5.1 low |
| 2026-09-27 | [`d5852d141f`](https://github.com/OpenAgentsInc/openagents/commit/d5852d141ff8733d5ea7a523153169b02586dff4) | [Reproduction on 14 tasks](https://github.com/OpenAgentsInc/openagents/blob/main/docs/terminal-bench/2026-09-27-fable-delegate-repro.md): 4 of 14 (#9776) |
| 2026-09-28 | [`88d0834279`](https://github.com/OpenAgentsInc/openagents/commit/88d083427932e10ef6bb5bee7e7d1d161200beda) | #9746 Fable delegate board in the Gym leaderboard |
| 2026-09-29 | [`d522f1a5d1`](https://github.com/OpenAgentsInc/openagents/commit/d522f1a5d19c89fda0001ed18330d28e0a136e33) | [Test-time capabilities essay](https://github.com/OpenAgentsInc/openagents/blob/main/docs/essays/2026-09-29-test-time-capabilities.md): "judgment budget" |
| 2026-10-02 | [`df7313757f`](https://github.com/OpenAgentsInc/openagents/commit/df7313757ff0ff44c2090baa562a74afd13f208e) | [Background processes](https://github.com/OpenAgentsInc/openagents/blob/main/docs/background/2026-10-02-background-processes.md): the same System One rule applied to background work |

Issues, all closed:
[#9530](https://github.com/OpenAgentsInc/openagents/issues/9530) (local
Terminal-Bench comparisons),
[#9535](https://github.com/OpenAgentsInc/openagents/issues/9535) (Jev-probe
v2), [#9569](https://github.com/OpenAgentsInc/openagents/issues/9569) (effort
per task), [#9586](https://github.com/OpenAgentsInc/openagents/issues/9586)
(strategy fingerprints),
[#9670](https://github.com/OpenAgentsInc/openagents/issues/9670) (shared
knowledge base), [#9680](https://github.com/OpenAgentsInc/openagents/issues/9680)
(Beat Fable together), [#9683](https://github.com/OpenAgentsInc/openagents/issues/9683)
(out-of-sample study), [#9746](https://github.com/OpenAgentsInc/openagents/issues/9746)
(Fable delegate with a Jev briefing),
[#9776](https://github.com/OpenAgentsInc/openagents/issues/9776) (frozen
reproduction).

Other conversation sources (read only): coderos-4080 sessions
`4d9ec5a2-…` (2026-09-22/23, Coder One and the Claude Code comparison) and
`86bb4789-…` (2026-09-23 to 2026-09-26, Microluna, the Gym, the fire
loop); this Mac's `803a9035-…` (2026-09-27/28, the Fable delegate).

**Not verified.** The public Fable reference trajectories ran on another host with an
older Claude Code, so the Fable bars are not same-host measurements. All
costs are list price, not bills. I did not find a grid in the Coder box's
later sessions (2026-09-28 to 2026-10-01) or its Codex sessions; the
"Fable run made cheaper and faster" grid is the one in 2a, written on this
Mac about runs on the Coder box.

## 4. The principles

Each is tied to the measurement that supports it, with its honest size.

1. **Lean the delegate.** Low effort, six tools instead of the default
   set, a replaced short system prompt, and a five-minute prompt cache
   (`CLAUDE_CODE_PROMPT_CACHE_TTL=5m`; cache writes cost 1.25x input
   instead of 2x). The five-minute cache alone cut the Opus arm 19 to 25%
   with no failures. The matched pilot shows this bundle, not the
   controller, carried most of the 61%. It is the cheapest saving to take.
2. **Brief instead of explore.** Jev probes the workspace with read-only
   commands and picks the evidence in 1 to 5 s for a fraction of a cent, so
   the expensive model starts from a briefing. On TB4, Coder One used 44%
   fewer input tokens than Claude Code. Jev's share was $0.09 across 26
   trials.
3. **Let Jev decide what the expensive model is told, not what it does.**
   s7a1's win came from Jev choosing 5 of 12 knowledge entries and flagging
   3 of 6 requirements, for $0.00018. Knowledge was the deciding factor:
   every Fable-delegate beat was on a task with its own entries, and none
   without.
4. **Make "done" a program state.** Executable acceptance checks, a host
   score check at every finish, and a stop after repeated green runs let a
   cheap model iterate to a verified answer (Microcoder 58x cheaper than
   Fable at equal pass rate, in sample; 2.9% of Fable's cost out of sample
   on TB2.1). Fable's winners build fresh test cases; telling Luna to do so
   was ignored, so the check has to be built by code, not asked for.
5. **Route per task to the cheapest configuration likely to pass.** An
   oracle chooser over past runs would have passed all 24 trials for about
   7 cents, 15 times cheaper than Opus alone. That is a ceiling, not a
   result; the effort router (#9569) missed its bar. The judgment that
   routes must cost far less than the work it routes ("judgment budget").

Two cautions the data forces. Routing to a smaller model mid-conversation
can cost more because it breaks the KV cache (episode 286, [02:00]), so
route at session boundaries. And System One can lose: the matched pilot and
the 0-of-14 first pass show a briefing can also make a frontier model rush
or miss.

## 5. Where the current harness stands

The default path today is Microcoder, not the Jev-briefed Claude Code or
Codex CLI. With `CODER_DELEGATE=auto`, the delegate door picks Microcoder
whenever a connected provider has capacity (Codex first, then Claude
through the `claude` binary, then the OpenAgents cloud), and falls back to
the Claude Code or Codex CLI only when no Microcoder provider is usable or
`CODER_DELEGATE_AGENT` names one
(`crates/coder/src/delegate_door.rs`, `choose`;
`crates/coder/src/delegate_door/microcoder.rs`). Grok Build, Devin, and
OpenCode are reachable only as task routes. Fable is never a default; it
exists as bench arms and as the bar.

| Principle | Default delegation today | Where |
| --- | --- | --- |
| 1. Lean the delegate | **Applied on the Claude Code CLI fallback**: Opus 5.5, `effort: low`, six tools, `prompt_cache_ttl: 5m`, 12k-character briefing cap, claude.ai connectors off. **Partly on Codex CLI**: effort only; tools, TTL, and system prompt are cleared. **Microcoder on Claude**: no tools, one turn, own system prompt and JSON schema, but no effort setting and no five-minute TTL. | `crates/coder-one/policies/jevprobe2-opus-lean-low-5m.json`; `crates/coder-delegate/src/terminal.rs` (policy load, Codex clearing, connector note); `crates/coder-delegate/src/delegate.rs` (sets `CLAUDE_CODE_PROMPT_CACHE_TTL`); `crates/microcoder-loop/src/claude.rs` |
| 2. Brief instead of explore | **Applied on the CLI fallback** (probe, survey, brief, delegate; request-only without a TypeSafe key). **Not on the Microcoder door**: no probe battery or briefing; Jev judges state at each step instead. **Absent** on Grok Build, Devin, OpenCode routes (raw prompt). | `crates/coder-delegate/src/terminal.rs`; `crates/coder/src/delegate_door/microcoder.rs`; `crates/microcoder/src/repository/grok.rs` |
| 3. Jev chooses what the model is told (knowledge, flagged requirements) | **Bench only.** `CODER_ONE_BRIEFING_KNOWLEDGE` and `CODER_ONE_BRIEFING_JEV=v1\|v2` exist; the door passes `knowledge: None` and task grants set `knowledge: "off"`. The Nostr knowledge relay is a `microcoder kb` tool, not wired into Coder runs. | `crates/coder-delegate/src/briefing_jev.rs`, `briefing_knowledge.rs`; `bench/terminal-bench/tbench/coder_one.py`; `crates/coder/src/task/autostart.rs`; `crates/microcoder/src/kbnet.rs` |
| 4. "Done" is a program state | **Off by default.** Microcoder acceptance, `green_nudge`, `green_stop` exist but door and repository runs set `acceptance: false`, task grants reject `acceptance: true`, and autostart sets `requirements: None`. **Applied only in the GitHub issue flow** (host gate plus fix sessions). | `crates/microcoder-loop/src/run.rs`; `crates/microcoder/src/repository.rs`; `crates/coder/src/task/{adapter,checks,autostart}.rs`; `crates/coder-delegate/src/issue.rs` |
| 5. Route per task to the cheapest likely-to-pass config | **Absent.** `crates/coder/src/router.rs` is the chat-tier router (T0–T4), not model selection. Task capabilities report `"routing":"unsupported"`; autostart picks by capacity and preference, not task. Default model is `gpt-6.1-sol` medium (the door's doc comment still says "GPT-6 Luna"); escalation is `Route::Never`. `Route::Auto` (Jev `hard` sends test-writing to a stronger model) is opt-in on the `microcoder` binary only. | `crates/coder/src/task/adapter.rs`, `autostart.rs`; `crates/microcoder-loop/src/run.rs`; `crates/coder-one/src/effort.rs`, `policies/tunable-*.json` (bench) |
| Cost visible per run | **Partly.** The CLI fallback's `Answer.usage` counts Jev and executor together (Jev at $0.042 per million input tokens); Microcoder records `jev_usd`. But task run records hard-code `cost_status: "unknown"` and `cost_usd: None`, the cost line is printed only in headless mode, and `coder-worker usage` has no separate Jev cost. | `crates/coder-delegate/src/usage.rs`; `crates/coder/src/headless.rs`; `crates/coder/src/task/{owner,adapter,view}.rs`; `crates/coder/src/relay/usage.rs` |
| Raw-vs-OpenAgents measurement | **Bench only.** Arms for `claude-code`, `claude-code-opus-matched`, `codex-gpt-6-*` against `coder-one-*`, `microluna-*`, and the Fable delegate; the Gym's beats-winner rule and the #9746 board. Nothing measures a user's real tasks. | `bench/terminal-bench/profiles/agents.json`; `crates/gym/src/runs_beats_winner.rs`; `crates/gym-leaderboard/src/tb4_delegate_dev.rs`; `bench/terminal-bench/tools/tb4_scoreboard.py` |

Owner rules hold on the default path: Microcoder door runs use
`Limits::unbounded()` (only an 8-step stuck guard), task grants use
`wall_seconds: 0` and reject dollar limits, and the terminal turn's 30-day
wall is effectively none. Dollar caps survive only on bench tools
(`microcoder --max-usd` default 1.00, `coder-one fire --max-usd` 0.50,
`kbstudy`), which is fine for studies.

Summary: the measured wins were mostly built as bench arms and the
Claude Code CLI fallback. The path most users hit (Microcoder door, task
grants) has lean prompts but none of the briefing, knowledge, acceptance,
or routing pieces, and its run records can't say what a run cost.

## 6. Suggestions, in order

None of these add usage limits, quotas, or step or time budgets to Coder
runs, and none route on keywords: every choice is a typed Jev judgment or a
policy table over measured records.

1. **Make cost per run a recorded fact (first, because everything else is
   measured by it).** In `crates/coder/src/task/` replace the hard-coded
   `cost_status: "unknown"` with the door's priced usage: executor cost,
   Jev cost, and status `priced`, `partial`, or `unknown` per charge, as
   `crates/coder-delegate/src/usage.rs` already computes. Carry it into
   `coder-worker usage` with a separate `jev_usd`. Show it on the run card
   and in the Gym as information, never as a limit. *Measure:* every new
   run record has a priced or explicitly unknown cost. *Saving:* none by
   itself; it makes the claim checkable.

2. **Build the raw-vs-OpenAgents comparison as a standing Gym study.** A
   `gym compare` (or study profile) that takes a task set and runs each
   task three ways: raw Claude Code on its defaults, raw Codex on its
   defaults, and OpenAgents' default path, on the same host, pins, and
   models, recording pass, cost (list price, same pricing source),
   wall time, and tokens per attempt. Reuse the Harbor arms in
   `bench/terminal-bench/profiles/agents.json` (`claude-code-opus`,
   `codex-gpt-6-*`) and add an arm that is literally the shipped door.
   Include a matched arm (same effort and model as raw) so the controller's
   own effect stays separate from the lean config, as the
   [matched pilot](../terminal-bench/2026-09-23-matched-opus-controller.md)
   taught. Add a second set of ordinary repository tasks (issues from this
   repo, with their merged fix as the check), not only Terminal-Bench.
   Pre-register the bar and publish to the Gym leaderboard. *Measure:*
   "OpenAgents vs raw: X% cheaper, Y% faster, same pass rate on N tasks,"
   with intervals. This is the number the selling point needs.

3. **Make the lean configuration the default for every Claude and Codex
   lane.** It carried most of the 61%. In `crates/microcoder-loop/src/claude.rs`
   set the five-minute cache TTL and an explicit effort; in the CLI
   fallback stop clearing the lean settings for Codex
   (`terminal.rs`) where Codex has equivalents; apply the same lean policy
   to Grok Build and other routes where their CLIs allow it. *Expected:*
   19–25% from the cache alone on Claude lanes, per #9535. *Measure:* the
   study in step 2, before and after.

4. **Put the Jev briefing in front of every delegation, not only the CLI
   fallback.** Run the probe, survey, and brief step
   (`crates/coder-delegate/src/terminal.rs`) before Microcoder's first step
   and before Grok Build/Devin/OpenCode routes, so no executor starts by
   exploring. The cost is 1–5 s and a fraction of a cent. *Expected:*
   fewer input tokens and turns (44% fewer input tokens on TB4).
   *Measure:* tokens, turns, and time to first edit, by arm.

5. **Turn on Jev-chosen knowledge where it exists.** Wire the
   `briefing_jev` v2 selection and the knowledge relay
   (`crates/microcoder/src/kbnet.rs`) into the door, defaulting to on when
   candidates clear Jev's threshold and off otherwise, since every
   Fable-delegate beat used task knowledge and none won without it. Record
   which entries were shown so the Gym can score the with/without delta per
   entry. *Measure:* the per-capability with/without eval the Gym already
   runs.

6. **Make "done" a program state by default when the task has a checkable
   outcome.** Let Jev judge whether a task has a testable outcome; when it
   does, freeze requirements and host checks (`crates/coder/src/task/checks.rs`)
   and enable Microcoder's `green_stop`, which ends a run early once checks
   keep passing. This is an early stop on success, not a budget. Have code,
   not the model, build fresh test cases, since Luna ignored instructions
   to do so. *Expected:* fewer wasted turns after the fix is in (Luna spent
   40–60% of a session self-testing in the v13 trials). *Measure:* turns
   after first green, pass rate.

7. **Route per task with a typed judgment, at session boundaries only.**
   Train the router on the step-2 records: Jev answers typed questions about
   the task (build-heavy, checkable, has knowledge, risk), and a policy
   table picks the cheapest configuration that passed comparable tasks,
   escalating to a stronger model only when checks fail. Never switch
   models mid-conversation (it breaks the KV cache). *Ceiling:* the oracle
   chooser was 15 times cheaper than Opus alone at 24 of 24. *Measure:*
   cost per pass against the best fixed configuration.

8. **Apply the same rule to any task handed to Fable or Claude Code by
   hand.** Until the door does all of the above, a person delegating
   should: brief first (files, probe output, requirements) instead of
   letting the model explore; run at low effort with the short cache and
   the six tools; attach relevant knowledge; give it an executable check
   and stop when it passes; and keep cost per run in the record. That is
   s7a1's recipe, and it can be a `coder delegate` default rather than a
   habit.

**What to make default now:** steps 1 and 3 (cost in every record; lean
config on every Claude/Codex lane) are small, low-risk, and measured.
Step 2 starts the same day, because the claim "OpenAgents is cheaper than
raw delegation" is only as good as that table. Steps 4–7 should land one
at a time, each with its with/without number in the Gym before it becomes
a default.
