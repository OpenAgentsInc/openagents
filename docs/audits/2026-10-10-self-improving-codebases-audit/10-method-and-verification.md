# Method, coverage and verification

This audit asks whether the codebase can deliver and improve an accepted coding service, beginning with OpenAgents itself. It follows the complete path from issue selection to customer acceptance and learned-model promotion, rather than treating individual crates or issue closure as the product.

## Recorded scope

| Input | Coverage |
| --- | --- |
| Source | `07805e6a7c3513a057d226b488cb2d40fd974a64`, fetched `origin/main` on October 10, 2026 |
| Product proposal | `docs/product/self-improving-codebases.md`, complete |
| Sales | All nine files under `docs/sales/`, including JSON qualification and delivery records |
| Codebase health audit | All 37 files under `docs/audits/2026-10-10-codebase-health-audit/`, including all area/cross-cutting sections, action register and remediation |
| Training audit | Both files under `docs/audits/2026-10-10-training-system-audit/` |
| Total requested files | 49 files; 1,960,069 bytes; 243,217 whitespace-delimited words |
| Open issues | All 47 in the collected snapshot |
| Recently closed issues | All 917 returned by the explicit `closed:>=2026-10-03` search; seven-day window through collection on October 10 |
| Additional evidence | Relevant older studies, selected issue comments, manifests, trace/corpus artifacts and targeted live-path source inspection |

The original checkout was behind fetched main and contained unrelated staged work. The audit used a separate managed worktree from the pinned source revision. No product source or unrelated checkout change was included. The final documentation commit may be rebased onto a later main; findings and hashes continue to describe the recorded source revision rather than silently changing their date.

The requested files were divided among product/data, runtime/operations, learning/evaluation and repository/synthesis reviewers. Every area and cross-cutting report was assigned. The action register was indexed in full and checked against the expanded reports; its 709 rows are inherited evidence rather than a second independent count of defects. Reviewers then inspected the implementation paths behind material product claims and checked each other's synthesis.

Issue bodies were screened across all 964 records. The open issues were read for dependencies and conflicts. Recent closed records were divided among reviewers for full-text relevance screening, followed by deeper reading of relevant bodies and selected closing discussions. One learning partition was read in full. The audit does not claim to have retrieved every comment on all 917 closed issues. The [issue inventory](evidence/issues.csv) records exactly which bodies and state snapshots the screening used, through hashes and acquisition metadata.

## Evidence hierarchy

| Label used in this audit | What it means | Limitation |
| --- | --- | --- |
| Source-confirmed | A cited current implementation directly exhibits the stated behavior or boundary | Static inspection is not an exploit demonstration or proof about every caller |
| Locally recomputed | A count, digest, join or offline test was evaluated during this audit | Only the named artifacts and test scope were checked |
| Retained measurement | A repository study or issue discussion records an experiment and its artifacts | This audit did not repeat the model/hardware run; cohorts retain their original limitations |
| Contract/proposal | The code or prose describes an intended rule or future acceptance standard | Not evidence that the rule is integrated, enforced or measured in production |
| Integration gap | Owners exist, but the reviewed path does not establish their complete connection | A negative search is scoped evidence, not proof that no private or unreviewed implementation exists |
| Historical finding | A previous audit reported the defect at its own revision | Check later source and remediation before treating it as current |

The product's `measured`, `authored`, `sample` and `contract_only` classes remain useful. They describe how evidence arose; they do not make a target label correct, prove an independent authority, or authorize data use. A measured changed-file label can still be the wrong label for a relevance question.

Numeric claims are carried with denominators and cohort boundaries. The audit does not pool file recall, microtask pass rates, trace counts, customer acceptance or revenue. A model judge's agreement is an observation about that judge, not executable proof of correctness. A signed or digested receipt binds a record; it is not remote attestation.

## Prior health findings and fixes

The health audit used revision `3168c986aa11e18a8bd30f52c270f609e49b3815`. It reported 709 overlapping findings: one critical, 57 high, 332 medium and 319 low. Those numbers describe that audit and are not this audit's present-defect count.

The [remediation record](../2026-10-10-codebase-health-audit/remediation.md) includes these significant changes before this source snapshot:

| Earlier finding family | Recorded change | Treatment here |
| --- | --- | --- |
| CLI MCP spend exposure and release version | Read-only/effect allowlist and version correction | Do not repeat the former unrestricted MCP finding as current |
| Semantic routing and accidental merge | Typed routing and explicit merge confirmation | Relevant policy improvement; decision quality and calibration still need their own evidence |
| Relay forwarded IP and NEG admission | Trusted-hop/auth/rate corrections | Historical finding with remediation, not a fresh open defect |
| Web local privilege and shutdown | Loopback peer check and shutdown handling | Credit the changed boundary; do not generalize to all servers |
| Verse asset replay and some error handling | Admission/cache repair | Partial family repair; inspect the specific caller before reusing the old finding |
| Pylon spend ceiling | Closed failure behavior and serialization | Useful budget foundation, not proof that every training or new runner cost is bounded |
| Child environment credentials | Shared policy adoption | Some provider-specific exceptions remain intentional; new check execution still needs its own boundary |
| Tenancy lock files | OS-released locks | Fixed stale-lock family; unrelated training trial read-count/append needs separate review |
| Ambiguous wallet outcome | Typed unknown result and stable retry identity | Credit CLI repair; avoid turning this into an assertion about every mobile path |
| Host process identity | Saved process identity before termination | Useful recovery protection |
| Eval ledger writer | Serialized writes and unique temporary files | Fixed specific race; not proof of a complete evaluator trust chain |
| Browser terminal input | Queue handling | Fixed interaction defect, relevant only if using that supervision surface |
| Gateway receipts | Blocking work moved off async workers; failures counted/logged | Current source corroborates the repair; do not repeat silent blocking as current |
| Build stamps | Stop watching the Git index | Credit reduced rebuild trigger without claiming a new measured speedup |
| Verification scope | Nested workspaces, direct consumers and lock-only changes | Current main scoper improved; experimental verifier selectors still differ |
| Tracked bytecode/build hygiene | Ignore/prune changes with retained transcript handling | Do not blindly delete remaining retained artifacts to meet the old count |

This table is a summary of the prior remediation evidence, not a fresh rerun of all those tests. The machine-readable register has a `remediation_mentions_id` column only as a navigation hint. A mention is neither a complete fix nor proof that every overlapping ID has been addressed.

Some inherited recommendations are inappropriate to apply literally. Conflicting dependency-version suggestions need compatibility checks. GitHub workflows and GitHub-billed automation are prohibited here. Full release gates are not prerequisites for ordinary changes. Old screen-grant requirements have been superseded by owner instructions. Preserve `docs/transcripts/`, artifact identities and source rights through repository cleanup.

## What ran for this audit

1. Git status, fetched source inspection, tracked-file and symbol searches, documentation reads, and GitHub issue queries.
2. SHA-256 inventory of requested files and reviewed source/artifact paths at the pinned source revision.
3. Parse and index all 709 historical health rows; check issue inventory counts and all 47 open issues' coverage in the issue map.
4. Recompute the trace manifest digest and count 45 receipts: 41 verified, four unverifiable, 22 verified passing and 19 verified failing checks across three verified issue groups.
5. Verify both N8 corpus artifact hashes and join verified trace issue groups to their corpus partitions: two calibration groups and one development group, while the trace generator assigns training.
6. Run the seven existing offline trace tests: `python3 -m unittest discover -s scripts/bench/traces -p test_traces.py`, with `TMPDIR` pointing to the task's `openagents scratch` directory. All seven passed in 2.185 seconds.
7. Validate local documentation links, JSON/CSV structure, evidence counts and pinned hashes; run `git diff --check` before commit.

The exact artifact values and normalized test command are in [learning-checks.json](evidence/learning-checks.json). The audit's [validator](evidence/verify.py) checks the retained inventory and local links without running Cargo, contacting a provider or mutating product data. Its output is retained in [validation.json](evidence/validation.json).

No heavy Cargo commands were needed for a documentation-only change. No new product tests, model calls, training runs, benchmarks, cloud workers, production deploys, customer messages, sales outreach, payment transactions or device sessions were performed. No live credentials were inspected. Historical test reports are attributed rather than presented as reruns.

## Reproduce the document checks

From the repository root:

```sh
python3 docs/audits/2026-10-10-self-improving-codebases-audit/evidence/verify.py
git diff --check
```

The validator checks source hashes with `git show` at the pinned revision so later main changes do not invalidate a historical audit. It checks current local Markdown link paths for navigation. A missing historical Git object is an explicit failure; it never substitutes current source. External issue pages and fragment anchors are not re-fetched by this validator.

The issue bodies and full comments remain in task scratch rather than being duplicated into public audit files. Their hashes permit comparison against the captured bodies, but cannot reconstruct edited/deleted GitHub text. This limitation is itself relevant to historical benchmark design. Future experiment intake should preserve authorized immutable input snapshots; this audit does not indiscriminately republish hundreds of issue bodies or comments.

## Limits on the conclusion

This is a comprehensive product and architecture audit informed by the requested full document corpus, issue screening and selected current-source paths. It is not a line-by-line audit of every Rust file, a penetration test, a legal opinion, a production release certification or a newly executed causal experiment.

The absence of a reviewed qualification packet means this audit cannot substantiate the claim; it does not establish that no private evidence exists. Likewise, a source-level missing check in one path does not prove all paths lack it. Chapters identify stronger existing owners and compensating controls, especially the independent A/B grader and environment verifier.

The most important conclusions are falsifiable: supply a complete S1/S2 packet, demonstrate the joined training/promotion path, repair the named source defects, qualify customer boundaries, and perform the protected two-cycle experiment. Those artifacts would change the assessment. More issue closures, larger corpora or more impressive model names without that evidence would not.
