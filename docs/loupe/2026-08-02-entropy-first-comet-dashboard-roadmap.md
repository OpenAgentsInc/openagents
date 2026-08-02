# Entropy-first Comet repository dashboard roadmap

Status: **owner-directed short-term product list.** This roadmap defines a
small entropy-only repository analysis inside Omega. It does not authorize a
generic vulnerability scanner, a third-party disclosure, a product safety
rating, a live-key search, or publication.

Date: 2026-08-02

Parent program: [OpenAgents issue #9300](https://github.com/OpenAgentsInc/openagents/issues/9300)

## 1. Product outcome

The user opens a repository in Omega, edits one visible entropy-analysis
prompt, starts a run, and watches Omega traverse the repository file by file.
Coldcard is the first target. The dashboard shows what is queued, what is being
read, what completed, what failed, and which entropy candidates appeared.

The user can then change the prompt and rerun the same pinned repository. Omega
keeps both runs and shows which candidates were gained, lost, changed, or left
unchanged.

This is an exploratory repository tool. Its output is a source observation or
hypothesis. It is not a verified vulnerability or product-level verdict.

## 2. Separate implementation list

These issues live in the Omega repository because Omega owns the GPUI product,
repository context, prompt interaction, run presentation, and source
navigation.

| Order | Issue                                                                                                                  | Outcome                                                                                                            |
| ----- | ---------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| 1     | [OFR-ENT-01 — Traverse one repository for entropy risks](https://github.com/OpenAgentsInc/omega/issues/199)            | Create one read-only, pinned repository run with a deterministic file manifest and typed entropy observations.     |
| 2     | [OFR-ENT-02 — Make entropy prompts editable and reruns immutable](https://github.com/OpenAgentsInc/omega/issues/200)   | Let the user edit the prompt while freezing the exact prompt and lineage used by each run.                         |
| 3     | [OFR-ENT-03 — Show live entropy analysis in the Comet workbench](https://github.com/OpenAgentsInc/omega/issues/201)    | Show file-by-file progress, candidates, failures, cancellation, restore, and prompt A/B changes.                   |
| 4     | [OFR-ENT-04 — Compare one entropy prompt across the 15-project set](https://github.com/OpenAgentsInc/omega/issues/202) | Run one frozen prompt across a versioned, source-aware project catalog and compare results without product grades. |

OFR-ENT-01 through OFR-ENT-03 are the Coldcard vertical slice. OFR-ENT-04 is
the first multi-project campaign. Do not start with the 15-project grid before
one Coldcard run can be inspected and reproduced end to end.

## 3. First screen

Keep the existing Comet project sidebar, task tabs, history, and composer. Add
one workbench region:

```text
┌ Coldcard · bcc2c382… · Entropy scan                      Ready ┐
│ Prompt                                                       │
│ [ Find entropy sources, fallbacks, seeding paths…          ] │
│ [Reset] [Use prompt from prior run]                 [Run]    │
├──────────────────────────────┬───────────────────────────────┤
│ Files  142 / 388             │ Selected candidate            │
│ ✓ random.py                  │ Secret consumer               │
│ ! board_config.h             │ ngu.random.bytes()            │
│ → rng.c                      │ Fallback/provider path         │
│ ○ wallet.py                  │ Source refs                    │
│ × vendor.c                   │ Missing evidence + next check  │
├──────────────────────────────┴───────────────────────────────┤
│ 3 candidates · 1 failure · 245 queued · 02:14       [Cancel] │
└──────────────────────────────────────────────────────────────┘
```

The main interaction is a stable file list and a selected-result pane. Do not
start with a five-tab forensic console, card grid, model panel, publication
workflow, or target-level score.

## 4. Entropy-only analysis boundary

The default prompt and file selection focus on:

- operating-system, hardware, secure-element, and library entropy sources;
- initial seeding, reseeding, state construction, and state persistence;
- deterministic or weak fallback generators;
- compile-time and runtime guards that choose an entropy provider;
- missing, shadowed, or incorrectly linked provider symbols;
- dependency crossings between a randomness API and its implementation;
- secret consumers such as mnemonic, seed, key, nonce, and wallet creation;
- truncation, bias, repeated state, predictable inputs, and unsafe error
  fallback; and
- claims that cannot be settled because a dependency, generated file,
  configuration, artifact, or closed component is unavailable.

The run can search and read the selected pinned repository through Omega's
existing repository and task authority. It is read-only and uses the existing
agent/model route selected for the task. It does not create a new shell,
credential path, cloud authority, reporting destination, or write permission.

Every candidate records the file, symbols, suspected mechanism, secret
consumer, exact source references, confidence boundary, missing evidence, and
next check. Prose without a typed observation remains diagnostic output.

## 5. Run and prompt behavior

Before analysis, Omega freezes:

- repository and revision;
- dependency and file manifest;
- visible user prompt and prompt digest;
- parent prompt when the prompt was copied from a prior run;
- model route, parameters, and tool surface; and
- run start time and cancellation boundary.

The file list uses explicit states: `queued`, `reading`, `analyzed`,
`candidate`, `skipped`, `failed`, and `cancelled`. Missing source,
unsupported language, incomplete dependencies, request-schema failure, tool
incompatibility, and provider failure are limitations. They never become a
clean result or a zero.

Changing the editor affects only the next run. A running or completed run keeps
its immutable prompt. The user can:

1. rerun the exact prompt against the same source;
2. duplicate a prompt, edit it, and create a derived run; and
3. compare prompt A with prompt B by gained, lost, changed, and unchanged
   candidates, files completed, limitations, elapsed time, and usage exactness.

The first comparison is descriptive. It does not automatically select a
winner, optimize the prompt, or promote a prompt to a release.

## 6. Fifteen-project campaign

The supplied comparison contains these 15 product rows:

1. Coldcard MK4/Q1
2. Trezor Model One/Model T
3. SeedSigner
4. Sparrow
5. Trezor Safe 3/5/7
6. BitBox02
7. Opendime
8. Bitkey
9. BlueWallet
10. Phoenix
11. Blockstream Jade
12. Ledger
13. SpecterDIY
14. Electrum for Android
15. Samourai Wallet

The campaign catalog must bind each row to a public repository and pinned
revision when one is available. It must also record license/access state,
dependencies, and source availability. Closed, missing, ambiguous, partial, or
discontinued source remains visible as `source_unavailable` or
`input_incomplete`; it is not analyzed from marketing material and does not
receive a clean or unsafe grade.

Each eligible project receives an isolated run with the same prompt snapshot,
model route, tool surface, and file-selection policy. The campaign dashboard
shows:

- project and pinned revision;
- source eligibility and current run state;
- files queued, completed, skipped, and failed;
- entropy candidates and limitations;
- elapsed time and usage exactness; and
- prompt digest and prompt A/B change counts.

Selecting a project opens its ordinary file traversal and candidate details.
The overview does not reduce different entropy mechanisms to one red, yellow,
or green product rating.

## 7. Acceptance order

1. Coldcard fixture: manifest, traversal, cancellation, typed candidates, and
   honest incomplete/failed states.
2. Editable prompt: default, reset, copy, freeze, digest, lineage, and restart
   restore.
3. Live dashboard: stable file progress, result detail, filters, source
   navigation, bounded rendering, and visual states.
4. Prompt comparison: exact rerun plus derived prompt A/B comparison on the
   same pinned Coldcard source.
5. Fifteen-project catalog: source availability before campaign execution.
6. Multi-project campaign: pause, resume, partial completion, per-project
   drill-down, and prompt comparison.

After this list is usable, the broader OFR-UI evidence ladder, model-panel,
publication, cloud lifecycle, artifact, generator, and historical-replay work
can reuse its run and comparison interaction. They are not prerequisites for
the entropy dashboard.
