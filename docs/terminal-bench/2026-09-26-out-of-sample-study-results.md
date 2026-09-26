# Out-of-sample study: results

Results for the [pre-registered study](2026-09-26-out-of-sample-study.md)
(#9683). Only outcome lines are read for held-out runs; see the
pre-registration's rules.

## Round 1 (paused)

- **Configuration:** Microcoder `f2cc194b57` (binary `microcoder-study-r1` on
  coderos-4080), GPT-6 Luna at medium effort through the Codex login,
  `--kb candidates`, entries synced only from `wss://relay.openagents.com`
  into an empty local folder.
- **Snapshot:** relay cache digest
  `b368bb4b13e4dbd224895913a54d0f20e1433a9a89c9eab35198acba11422f76`
  (58 entries, 16 evidence reports).
- **Retrieval:** in practice word-only. Query embeddings failed because
  OpenRouter was out of credit, and this build falls back silently.
- **Started:** 2026-09-26 14:24 UTC.

**Paused** at about 15:00 UTC. The Codex (ChatGPT Pro) weekly usage limit was
reached: HTTP 429 `usage_limit_reached`, resetting 2026-10-01 07:22 UTC. The
screen driver was stopped, and the runs in flight were interrupted. Runs that
ended on the limit are provider faults, not results. Their tasks keep their
place in the queue.

| Task | Result | Steps | Time | Cost (list price) | How it ended |
| --- | --- | --- | --- | --- | --- |
| `biped-contact-dynamics` | Fail | 15 | 2:14 | $0.0228 | Luna's own tests held |
| `formal-crypto` | Fail | 38 | 4:15 | $0.0676 | The model finished |
| `atrx-vep-crispr` | Fail | 60 | 18:54 | $0.2548 | Step limit |
| `layout-config-recreation2` | Fail | 59 | 15:47 | $0.2087 | Luna's own tests held |
| `retro-console-soc` | Fail | 44 | 7:30 | $0.0575 | Luna's own tests held |
| `react-lead-form` | Fail | 60 | 13:42 | $0.1357 | Step limit |
| `freecad-spring-clip` | Grade unknown | 19 | 4:59 | $0.0251 | Harness fault: the verifier image didn't build |
| `freecad-platform-drawing` | Grade unknown | 40 | 9:47 | $0.0896 | Harness fault in grading |
| `ks-solver-cpp` | Grade unknown | 13 | 2:12 | $0.0138 | Harness fault in grading. **Burned** (see the amendments) |
| `photonic-waveguide-routing` | Provider fault | 44 | 20:59 | $0.0875 | 429 usage limit |
| `rs-archive-clone` | Provider fault | 12 | 6:34 | $0.0188 | 429 usage limit |
| `satb-audio-transcription`, `pretrain-shard-corruption`, `uefi-bootkit`, `takens-embedding-lean` | Interrupted | — | — | — | Stopped when the limit hit |

**Not supported by this build:** 9 held-out tasks ship a Compose file, and
Microcoder refused them: `ctr-optimization`, `cumulative-layout-shift`,
`heat-pump-warranty`, `intrastat-meldung`, `kv-live-surgery`,
`legacy-utility-triage`, `live-database-cutover`, `nextjs-performance`,
`payments-pipeline-fix`. Compose support and separate-verifier grading are a
harness fix (#9688), allowed by the rules because they aren't task-specific.

**Not yet run:** `vpp-loss-divergence`, `wdm-design`, and the 23 Fable-fails
tasks.

**So far:** 0 passes in 6 graded held-out runs. The commonest ending is the
one already seen out of sample: Luna's frozen tests pass before the grader's
requirements are met, within minutes and with most of the budget unspent.
Round 2 targets that with loop changes developed only on the excluded tasks
(#9683).

## Retrospective

- `react-lead-form-1790398585` (2026-09-25, before this study): **pass**,
  7:06, $0.110 billed. Fable 5.1 low's cheapest winning run there cost $2.46
  and took 6:18. It used general seed entries only. This is a retrospective
  out-of-sample cost win; see the pre-registration's amendments.
