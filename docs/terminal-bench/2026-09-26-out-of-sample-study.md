# Out-of-sample knowledge and Microcoder study (pre-registration)

Pre-registered September 26, 2026, before any run it covers. Tracking:
[#9683](https://github.com/OpenAgentsInc/openagents/issues/9683), under the
[Beat Fable together plan](../coder/beat-fable-together.md) (#9680).

The three wins recorded so far (`embedding-drift-monitor`, `gsea-proteomics`,
`fin-saccr-rwa`) are in-sample: the deciding knowledge entry was written from
the task it helped. This study asks the question those can't answer: **on
Terminal-Bench 4 tasks that Microcoder and the knowledge base have never
touched, does Microcoder pass for less than Fable 5.1 low's cheapest winning
run?** It also measures whether the knowledge base helps there.

## What "out of sample" means here

A task is out of sample for this study when all of these hold at the time of
its run:

1. Microcoder has never run on it. Microcoder's loop and prompts were
   developed on 14 tasks; they are excluded.
2. No knowledge entry names it in `provenance.written_from`, and no entry was
   written or harvested after reading anything about it.
3. No change to Microcoder, Jev questions, or entries was made after reading
   its held-out transcripts (see [the rules between rounds](#rules-between-rounds)).

**Disclosure.** Earlier harnesses (Coder One, Microluna, Luna baselines) ran
the whole suite, and suite-wide analyses (check recall, strategy
fingerprints, truthful checks) read their records. `react-lead-form`,
`nextjs-performance`, and `photonic-waveguide-routing` also have stronger
agents' winning traces retained under
`bench/terminal-bench/reference/task-win-traces/` from a Coder One study.
None of that fitted Microcoder or wrote an entry, but reports mark those
three tasks, and the tasks in `tuned-lexicon.json`, as *studied by earlier
harnesses*, so a reader can drop them.

## The held-out tasks

This pool is every installed TB4 task that needs no GPU, that Microcoder
never ran, and that Fable 5.1 low passed at least once in the public replays:
26 tasks. The SHA-256 of the newline-joined names is `46824ed5bd0776b540480f68309443245b843d9c68abd339a47fee1fd6136f31`.

| Task | Fable 5.1 low passes | Its cheapest winning run | Its fastest winning run (min) | Category | Image |
| --- | --- | --- | --- | --- | --- |
| `atrx-vep-crispr` | 1/5 | $1.95 | — | Science/Biology | kept |
| `biped-contact-dynamics` | 3/5 | $15.24 | 45.4 | Science/Robotics | builds on first use |
| `ctr-optimization` | 1/5 | $38.44 | 289.4 | Operations/Marketing | builds on first use |
| `cumulative-layout-shift` | 5/5 | $7.21 | 35.9 | Software/Frontend | kept |
| `formal-crypto` | 3/5 | $4.09 | 13.9 | Security/Cryptography | builds on first use |
| `freecad-platform-drawing` | 1/5 | $0.53 | 1.8 | Hardware/CAD | builds on first use |
| `freecad-spring-clip` | 2/5 | $4.62 | — | Hardware/CAD | kept |
| `heat-pump-warranty` | 2/5 | $3.09 | 5.0 | Operations/Claims | kept |
| `intrastat-meldung` | 4/5 | $3.17 | 7.1 | Operations/Compliance | kept |
| `ks-solver-cpp` | 1/5 | $2.82 | 10.5 | Science/Physics | kept |
| `kv-live-surgery` | 2/5 | $2.67 | 13.7 | Software/Systems | builds on first use |
| `layout-config-recreation2` | 3/5 | $3.54 | 27.3 | Media/Design | builds on first use |
| `legacy-utility-triage` | 3/5 | $2.60 | 17.3 | Operations/Claims | builds on first use |
| `live-database-cutover` | 4/5 | $6.21 | 21.3 | Software/Databases | builds on first use |
| `nextjs-performance` | 3/5 | $1.21 | 3.9 | Software/Frontend | builds on first use |
| `payments-pipeline-fix` | 5/5 | $4.63 | 16.9 | Software/Systems | kept |
| `photonic-waveguide-routing` | 4/5 | $8.66 | 38.3 | Software/Algorithms | kept |
| `pretrain-shard-corruption` | 5/5 | $7.14 | 39.6 | ML/Training | builds on first use |
| `react-lead-form` | 2/5 | $2.46 | 6.3 | Software/Frontend | builds on first use |
| `retro-console-soc` | 5/5 | $8.14 | 32.5 | Hardware/RTL | kept |
| `rs-archive-clone` | 3/5 | $14.50 | 43.7 | Software/Algorithms | kept |
| `satb-audio-transcription` | 1/5 | $13.64 | 52.0 | Media/Music | builds on first use |
| `takens-embedding-lean` | 3/5 | $34.15 | 61.1 | Science/Math | builds on first use |
| `uefi-bootkit` | 2/5 | $15.90 | 85.5 | Security/Forensics | builds on first use |
| `vpp-loss-divergence` | 1/5 | $4.65 | 14.9 | ML/Training | builds on first use |
| `wdm-design` | 5/5 | $5.39 | 184.8 | Science/Physics | kept |

A second pool holds the 23 tasks Fable 5.1 low never passed (the first version of this file said 24; the digest was always over these 23) (SHA-256
`625fc34c5d27cb87806d0cab539aaf5183aa7a7b1e2696e44c7928a4f390f93b`): `bun-sourcemap-leak`, `cad-model`, `cargo-flight-dispatch`, `data-anonymization`, `foodstuff-beta-activity`, `freecad-impeller`, `freight-dispatch-shift`, `glycan-ms2-elucidation`, `html-js-filter`, `lake-temp-glm`, `layout-config-recreation`, `medical-claims-processing`, `music-harmony`, `mvcc-lsm-compaction`, `ontology-kg-querying`, `protein-autointerp-disulfide`, `roy-polymorph-cn`, `session-window-debug`, `sglang-qwen-burst`, `vba-userform-port`, `vf2-speedup-networkx`, `vllm-deepseek-streaming`, `wal-recovery-ordering`. A pass there beats Fable outright. They get one screening run
each and are reported separately.

Excluded and never counted:

- the 14 tasks Microcoder ran: `batched-eval-parity`, `coq-block-bound`,
  `distributed-dedup`, `embedding-drift-monitor`, `fin-saccr-rwa`,
  `gsea-proteomics`, `hof-topology-interpenetration`, `interleaved-vigenere`,
  `mp-checkpoint-consolidation`, `production-planning`, `risk-scorer-replay`,
  `shadow-relay`, `sound-change-cascade`, `telecom-entity-resolution`;
- the GPU tasks.

## Arms and configuration

- **Harness:** Microcoder at the commit recorded in the study record when
  Round 1 starts, running GPT-6 Luna at medium effort through the operator's
  Codex login, with Jev as today. Caps: 60 steps, $1.00 list price, and
  60 minutes. If twice Fable's fastest winning time is longer, the time cap is
  that, up to 120 minutes.
- **Knowledge on:** entries synced only from `wss://relay.openagents.com`
  into an empty local folder, frozen as the snapshot recorded at round start
  (IDs, versions, digests).
- **Knowledge off:** `--kb off`, everything else identical.
- **Cost basis:** list price for reported tokens (`cost_basis: list_price`).
  Fable's costs are the ones its public replays report. Unknown cost stays
  unknown, and such a run can't count as a cost win.

## Procedure

1. **Screen.** One knowledge-on run per held-out task and one per Fable-fails
   task, at most 4 runs at a time.
2. **Confirm.** Every held-out task that passed its screen gets 2 more
   knowledge-on runs and 3 knowledge-off runs, interleaved.
3. **Report** every run, including faults, aborts, and runs cut short, with
   its reason.

## What counts as a win

- **Cost win:** a pass on a held-out task whose list-price cost is below
  Fable 5.1 low's cheapest winning run on that task.
- **Time win:** a cost win that also finishes faster than Fable 5.1 low's
  fastest winning run.
- **Confirmed out-of-sample win:** a held-out task where at least 2 of its 3
  knowledge-on runs are cost wins. Report the knowledge-off arm next to it:
  if knowledge-off also wins, the loop gets the credit, not the knowledge
  base.
- **Beats Fable outright:** a pass on a Fable-fails task, confirmed by a
  second pass.

The study succeeds on its own terms if it reports honestly. The plan's goal
is at least two confirmed out-of-sample wins.

## Rules between rounds

Improvements between rounds are allowed, but only from sources outside the
held-out pool:

- **Knowledge sources.** Entries may be harvested or written only from the 14
  excluded Microcoder tasks, from Terminal-Bench 2.1 (different tasks), or
  from general references. Each entry's `written_from` names its source.
- **Loop changes.** Loop and question changes may be developed and debugged
  only on excluded tasks.
- **Held-out runs.** Operators and agents read only the outcome line until
  the study closes: pass or fail, how it ended, tests passed, time, and cost.
  **Reading a held-out transcript, verifier output, or task file to change
  anything burns that task.** It moves to the excluded list with the reason,
  and its later runs are reported as in-sample.
- **Harness faults.** Faults that aren't task-specific (image builds,
  container start, network policy, host timeouts) may be fixed after reading
  the fault's log. The fix is recorded, and the task isn't burned.

Each round records the Microcoder commit, the knowledge snapshot digest, and
the task list. A new round reruns the screen on held-out tasks that haven't
been burned or confirmed.

## Records

Run records stay under `~/.openagents/microcoder/runs/` on the execution host
and are copied into the retained set the Gym reads (#9681). The study record
and results go in `2026-09-26-out-of-sample-study-results.md`, next to this
file. The summary goes in [tb4-results.md](tb4-results.md).

The knowledge added for each round is recorded next to this file: Round 2 in [2026-09-26-round2-knowledge.md](2026-09-26-round2-knowledge.md) (sources, entries added and rejected, cost, and the relay-sync note). Round 3 in [2026-09-26-round3-knowledge.md](2026-09-26-round3-knowledge.md) (50 more entries, 154 on the relay).

## Amendments

**2026-09-26, during Round 1.** The Gym's ingestion (#9681) found a Microcoder run on `react-lead-form` made on the owner's Mac the night before this pre-registration (`react-lead-form-1790398585`, 2026-09-25). It passed all 11 tests in 7:06 for $0.110 (model $0.093, Jev $0.017, embeddings $0.001, billed through OpenRouter). The only entries shown were general seed entries, none written from this task. That contradicts criterion 1 for this task: Microcoder *had* run on it once. Nothing was changed because of that run, and no entry was written from it. So the task stays in the held-out pool, its Round 1 screen run is the prospective test, and the earlier run is reported separately as a **retrospective** out-of-sample pass. It is the first recorded one.

**2026-09-26, during Round 1.** While debugging a grading fault, the study operator read a few lines of model reasoning from the `ks-solver-cpp` screen run. By the rules above, `ks-solver-cpp` is **burned**: its results are reported as in-sample from now on.

**2026-09-26, Round 2 declared before any Round 2 held-out run.** Round 1 is closed as partial. See the results file: its harness couldn't run Compose tasks or grade some separate verifiers, and the Codex limit cut it short. Round 2 screens every held-out task that isn't burned (25; `ks-solver-cpp` is burned) and all 23 Fable-fails tasks again, with this configuration:

- Microcoder `e799ac020d` (binary `microcoder-study-r2`), which adds Compose tasks and separate verifiers like Harbor (#9688), complete relay sync (#9689), and honest cost fields (#9682).
- GPT-6 Luna at medium effort through **OpenRouter** (`--provider openrouter --model openai/gpt-6-luna`). The cost basis is `billed`. This change of route from the Codex login is declared here. The model and effort are unchanged.
- `--kb candidates`, entries synced only from `wss://relay.openagents.com`: 104 entries, the 58 from Round 1 plus 46 [Round 2 entries](2026-09-26-round2-knowledge.md) from excluded tasks and Terminal-Bench 2.1 only. The relay cache digest is `797cbd4242f921ec029a2b0ee0d6aa02ba8c65cf328e82a980d76b1822e445f0`.
- Retrieval uses embeddings, which work again through OpenRouter.
- No loop changes. Loop mechanisms still being developed on excluded tasks go to a later round.
- Same caps and same win rules.

**2026-09-26, Round 2 procedure notes.** The Round 2 screen runs 8 runs at a time, not the 4 the procedure named. Each run has its own containers and caps, so this changes wall time, not what a run sees. The report tool (`bench/terminal-bench/studies/2026-09-26-out-of-sample/`) takes Fable 5.1 low's fastest winning times from the replays file. That file disagrees with this page's table for five tasks (`atrx-vep-crispr` 9.0 min, `freecad-spring-clip` 14.6, `intrastat-meldung` 6.7, `retro-console-soc` 23.5, `wdm-design` 104.2), because this table was built from a parse that dropped some timestamps. Time wins use the replays values. Cost bars are unchanged. `freecad-platform-drawing` can't be graded: its verifier image no longer builds upstream (`pip` `uninstall-distutils-installed-package`), so it's reported as ungradeable, not as a failure. Confirmation runs are queued automatically by `confirm.py`.

**2026-09-26, dev set extended to Terminal-Bench 2.1.** TB2.1's 89 tasks have no names in common with TB4 ([dev set](tb21-dev-set.md)), so loop and configuration tuning may use them as well as the 14 excluded TB4 tasks. The rule already allowed them as a knowledge source. Also, a harness fault that isn't task-specific: on coderos-4080, commands run through `docker exec` got umask `0000`. The fix lands between rounds, and Round 3 records it.

**2026-09-26, Round 3 declared before any Round 3 held-out run.** Round 2 found 0 of 24 held-out passes, and 15 of those runs ended at the 60-step limit well inside their time and money caps. On the Terminal-Bench 2.1 dev set, an interim sweep (docs/terminal-bench/2026-09-26-tb21-sweep.md, 20 of 45 runs) had Luna at 200 steps passing 5 of 7 against 3 of 7 at 60 steps. GPT-6 Sol passed 3 of 6 at about 2.3× the cost. Round 3 changes only these things:

- Microcoder `ea9c973576` (binary `microcoder-study-r3`), which adds umask `022` for every container command (`79f97b89ee`).
- `--max-steps 200 --max-usd 2.00`. Time caps are the pre-registered ones with a floor of 90 minutes, still at most 120.
- Knowledge snapshot: 154 entries (Rounds 1–3 knowledge; [Round 3 record](2026-09-26-round3-knowledge.md)), relay cache digest `68c1d4e456dd2de6adfc16be271f7b3b4f39e40601b7436dfccbb8f5fdcabf03`.
- Still GPT-6 Luna at medium effort through OpenRouter (billed), `--kb candidates`, with no loop gates.

Same held-out tasks (25) and Fable-fails pool (23), same win rules, and confirmation runs queued automatically.

**2026-09-26, Round 3 restarted on the Codex login.** The first Round 3 attempt used OpenRouter, which ran out of credit mid-round (HTTP 402). Several Compose tasks also couldn't start their verifier because Docker's predefined address pools ran out at 8 runs at a time alongside the dev sweeps. Those runs are kept in `r3-aborted-openrouter` as provider or harness faults, and none counts as a result. Round 3 was restarted from scratch with the same binary, snapshot, and flags, except that it uses `--provider codex` (GPT-6 Luna through the Codex login, cost basis `list_price`) and runs 5 at a time.
