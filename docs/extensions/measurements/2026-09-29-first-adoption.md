# The first adoption: Project map goes from claim to validated to admitted, 2026-09-29

What happened: one capability went the whole way through the loop the
[test-time capabilities essay](../../essays/2026-09-29-test-time-capabilities.md)
describes, on the production relay (`wss://relay.openagents.com`) and the
hosted runner on `coderos-4080`, in one afternoon
([#9961](https://github.com/OpenAgentsInc/openagents/issues/9961)). A
**Better** result on Project map was confirmed by three checks from
distinct trainer keys, externally validated by a **Better** result on a
second test set released under another key after the tool, listed by the
referee as a candidate, adopted by an operator with the `coder-defaults`
key, credited under `eval-adopt`, and admitted by the next hosted run in
both arms and by a Coder turn on a computer. What did not happen is
listed at the end.

The code that made it possible landed as
[`4467f24eb9`](https://github.com/OpenAgentsInc/openagents/commit/4467f24eb9):
runtimes consume `coder-defaults`
([policy](../../../packages/coder-defaults/policy.md#how-a-release-reaches-runtimes)),
the hosted wire carries `validates`, and `openagents ext eval release`
releases a suite without running it.

## The subject and the two test sets

| | Release | Signer | Created |
| --- | --- | --- | --- |
| Project map, the tool (`crates/plugin-repo-map`) | `7af8bd08bbff4440c2b038ffc45f422b67371daddfb9664750a768ee7175dbf4` | the hosted runner, `a7cff3ee…` | 1790667818 (the first release; reused unchanged by every restart since) |
| Starter test set, re-released (`<runner>:project-map-tests`, `suite-…`) | `7b09bd998129f11ebb37e1534833e7f038f16243a3e843c78426eac9b1dac7f1` | the hosted runner | 2026-09-29 18:14 UTC |
| Second test set (`<validator>:project-map-tests`, `crates/plugin-repo-map/evals-validation`) | `5efad14e69dd56ca31d13f5c8538617539d5f01f03d213344a6169847c1a3e2b` | the validator key, `63fe2f2178a4b2b9719ca4655d7db85036f0e156d197c1652ea13eb6b73bdabc` | 2026-09-29 18:03 UTC, suite `sha256:ad005657a335…` |

The second test set is six cases the starter set doesn't ask (a file
count and byte total, the busiest directory, the build manifests of a
two-part project, the test directories of a JavaScript library, and two
requests where a map has no place), with the sampling story in
[its README](../../../crates/plugin-repo-map/evals-validation/README.md).
It was released with `openagents ext eval release crates/plugin-repo-map
--eval-dir evals-validation --as validator`, and its 46 files were put in
the runner's public bucket by hand, since `relay.openagents.com` has no
Blossom store. Independence as the protocol checks it holds: a different
signer than the tool's release, created after it, no `distribution`
declared on either side (so both claims are about the tool's definition
ID). Independence as a fact about people does not: the validator key was
made on the same MacBook that sent every trainer request below, by the
same person's agent that wrote the tool's starter set's fixes earlier
in the day. The record shows two keys and a chronology, which is all it
can show.

## The runs

Every run below went through the hosted runner at commit `4467f24eb9`
(agent `sha256:1e01d9ca713a6d50602e8a4489fc241e09522a371a6cc8c0fccde5ed379e99c8`),
3 runs per arm, both arms, live Coder on the Gemini Flash lane and live
Jev, inside `bwrap`, with the hosted grant of read and sandbox write.
Each request came from a fresh trainer key made for the purpose
(`crates/eval-runner/examples/trainer.rs`); the subject-arm lock of every
result before the adoption is
`sha256:86e810f2e5c8d82a08bf9e1f3da5224aa8ecc6abc110727e0b345d1d42293ffc`.

| Role | Trainer key | Test set | Result (`3189`) | With / without | Verdict | Cites |
| --- | --- | --- | --- | --- | --- | --- |
| The result | `ebb5acbc…` | starter `7b09bd99…` | `9048973d47bb64383cf0de507f3ac8d5782ebd76532c5489cf5b52771a2f50d8` | 5 of 6 / 2 of 6 | **Better** | |
| Check 1 | `71ddbddd…` | starter | `1b009944357e246753215088664cbe9fa79ba224bd7c449bb07abdabff084804` | 5 of 6 / 2 of 6 | **Better**: confirms | `check` → the result |
| Check 2 | `0379c2cb…` | starter | `581b7933ff1178d84b04bb37cb26f1d54e3f72eac11d65ffb271ca176fae0d7d` | 5 of 6 / 2 of 6 | **Better**: confirms | `check` → the result |
| Check 3 | `9d829c2b…` | starter | `b37004767f3c5f8e34c3a8cfa43b9f29ac60ab4632eb0aa3186833433fdc3ec8` | 5 of 6 / 2 of 6 | **Better**: confirms | `check` → the result |
| The validation | `832669fb…` | second `5efad14e…` | `85391dc33526e311d88a1e4cb37dca5670ef995820bcff7769dafc199180a423` | 4 of 6 / 2 of 6 | **Better**: validates | `validates` → the result |

Per test, runs passed of 3, with and without the tool:

| Test set | Test | Kind | With | Without |
| --- | --- | --- | --- | --- |
| Starter (the result and all three checks agree) | `largest-file`, `languages`, `build-files` | should fire | 3, 3, 3 | 0, 0, 0 |
| | `where-tests` | should fire | 0 | 0 |
| | `greeting`, `explain-404` | should not fire | 3, 3 | 3, 3 |
| Second | `file-count`, `test-files` | should fire | 3, 3 | 0, 0 |
| | `busiest-directory`, `manifests` | should fire | 0, 0 | 0, 0 |
| | `rename-advice`, `commit-message` | should not fire | 3, 3 | 3, 3 |

The externally validated delta is smaller than the original, +2 of 6
against +3 of 6, which is the finding the essay said to expect. The two
second-set cases that failed in both arms failed the way `where-tests`
does on the starter set: Jev didn't choose the tool's program for that
wording (which directory is busiest; which manifests there are), so
neither arm looked. Time per run with the tool was 30 to 42 s against
68 to 91 s without, in every record; cost is unknown, as before.

## The referee, the queue, and the adoption

The XP referee at the old revision `3c0e8ea7` (deployed before the
validation rule and the candidate policy existed) signed nothing for
these checks and listed no candidate. Reinstalled at `4467f24eb9` with
`deploy/xp-referee/install.sh`, its next pass (18:20 UTC) published quest
`ext-eval.project-map.check@2` (`a7e6abdea777…`) and signed five awards:
50 XP to each of the three checkers, 25 to the result's trainer, and 25
to the test set's author (the runner's key), and printed:

```text
candidate for Coder's defaults: tool release 7af8bd08bbff (a7cff3ee…:project-map/project-map), 1 confirmed Better result(s)
```

`microcoder xp adopt --subject 7af8bd08bbff…` on `coderos-4080`, signed
with the `coder-defaults` key that lives there, then wrote and published:

| | |
| --- | --- |
| Admission (`openagents.eval-admission.v1`) | `sha256:be6127d5a191d34875959bd128580bdd4070e961125a27e3a2dda58656b50050`; `decision: admit`; cites report `sha256:145d5cba…` and validation `sha256:8bb8162a…`; `expires_at` 1822242039 (365 days: a content-addressed subject) |
| Manifest (`openagents.package.v1`, `coder-defaults` 1) | `sha256:8194883f9903fab244a22f950dba0aad264b1d198002a1a443f2df458558ef2d`; depends on `7af8bd08bbff…`; cites the admission |
| Release (`3184`) | `680720dd100f96b906a2fdf622e0c363a88539916103291b84e3ec23a8954261`, signed by the root `51428dd2…`, created 1790706039 |
| Locators (`1063`) | `6345b2bac173…` and `64c3567e4321…`, pointing at [`packages/coder-defaults/documents/`](../../../packages/coder-defaults/documents/) on `main` |

The referee's next pass published `ext-eval.adopt@1` (`68df39f6d3fe…`)
and signed two `eval-adopt` awards: 200 XP to the extension's author and
50 XP to the result's trainer (`ebb5acbc…`). The runner's key is both
the tool's author and the starter test set's author, and a key holding
two roles is paid once in the larger, so there was no separate
suite-author award. Awards, recomputed by `microcoder xp ledger` with the
adoption's documents: `511570ea8cbf…` (extension-author), `4abeafd1efe0…`
(evaluator).

## The admission reaching Coder

- **The hosted runner**, without a restart, read the release at the next
  admission and logged `defaults release 680720dd100f (version 1) admits
  Project map`. Trainer `391c1350…`'s run of the starter set right after
  (`581a1a9472e57c4092f09ae8528af97b430468cb2571dd2ddacc59601594bc69`)
  is the first marginal report: both arms held Project map, so it passed
  5 of 6 with the tool and 5 of 6 without, **No clear change**, with
  `meta.ext_eval.defaults` naming `680720dd…` and both locks changed
  (subject `sha256:0642e873…`, baseline `sha256:b4c8777b…`). That is the
  adopted tool doing its work in the baseline arm; it is also why the
  tool leaves the queue and why results from before the adoption can no
  longer be checked on the runner.
- **A computer**: `openagents ext defaults sync --catalog
  crates/plugin-repo-map` read the release and the admission (from the
  cache, since the locators serve `main` and this record's commit carries
  the documents), resolved the tool by the manifest digest its local bytes
  release as, and wrote the lock `sha256:81bc3a2a74366c21411888bd99c4d0ea6323e71eab03f66ef1d870cd062b4f97`
  with `programs/project-map.json`; `openagents ext defaults show` says
  `admits project-map`. A `coder -p` turn under `CODER_DEFAULTS` pointing
  there recorded in its trace, before the turn: `defaults: release
  680720dd100f (version 1) lock sha256:81bc3a2a… admits project-map`. The
  turn ran against the stub door (no key on this machine), so the reply is
  the stub's; the admission is what the trace shows.

## Jev-probe as a subject

The Jev-probe policy is packaged in
[`packages/jev-probe`](../../../packages/jev-probe/README.md): a NIP-PRG
program (probe with the evidence guests, select with Jev's
`evidence-relevance` question, delegate to `coder-one-ask` under the v3
directions as a skill) and a cost-primary test set naming
`ext-eval-cost-v1`. The test set was released under the validator key as
`a75c2b5e94b03ff3f0d5590c2deb54edb67b3851b3f520c903d8261e9c4de7f8`
(suite `sha256:89055d1e6737…`, the first release whose acceptance names
the cost gate). One local run on `coderos-4080` with the runner's Coder
and door, 2 runs per arm (report `sha256:1465ee6ab15d9fa0de06927fed9764104406669892c18a023b26fdc4f97dd22e`,
not published): **No clear change**, 2 of 6 with the program and 1 of 6
without, every should-fire case 0 of 2 in both arms, cost unknown in both
arms. Two things stopped it, both on the record: the child Coder refused
the program at selection (`this host would run none of the programs it
resolved`: its `probe` step is a child program and its `work` step a
delegate to an executor, neither of which the sandboxed child holds), and
several runs in both arms ended with `the model endpoint did not answer
… over 3 attempts`, which is the door, not the policy. The claim is now
citable and the suite runnable; the Terminal-Bench numbers stay in the
Terminal-Bench record, and a cost-primary verdict needs a priced door
before it can read anything but inconclusive.

## Found and fixed on the way

- **The starter test sets no longer reproduced.** The runner refuses a
  released suite it can't rebuild byte for byte, and the suite document
  gained the workload's sampling story (`65f53d6d15`) after the starter
  sets were released, so the first request was refused. The starter sets
  were released again under the runner's key (`eval-runner release …`);
  the referee opens a new quest version per test set release, and it did.
- **The referee was two days behind.** Its installed binary predated the
  candidate policy; nothing in its log said so beyond `0 candidates`.
  The runbook's install step fixed it. A deploy that changes the referee's
  rules should say its revision in the pass line.
- **A reader that follows an award's evidence never sees the
  validation.** `microcoder xp ledger` fetched the events the awards name
  (release, result, check) and refused both `eval-adopt` awards as citing
  no validation, because the validation is named only by its report
  digest in the admission. The ledger and Verse now also fetch the
  publications the held admissions cite, by `#x`. The awards were right;
  the reader was incomplete.
- **The runner logged nothing while no defaults existed.** Fixed to log
  the first read too.

## What did not happen

- Nobody's phone showed "Coder has this capability now": the tool's
  author is the runner's key, which no phone holds. `xp_ledger::eval::made`
  for that key lists the adoption; the screen wasn't exercised.
- The three checks, the validation, and the result all came from one
  machine and one person's agent, through one runner, one door, one Coder
  build, and one grader. The policy counts keys; this record counts one
  operator.
- No cost-primary result read Better, and none can until a door prices
  its lane.
- The runner's post-adoption report is marginal by construction, not by a
  new arm: with one default, "current defaults plus the candidate" and
  "current defaults" coincide when the candidate is the default.
