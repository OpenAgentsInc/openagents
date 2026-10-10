# Evidence index

The source snapshot is `07805e6a7c3513a057d226b488cb2d40fd974a64`.
These files support a historical audit. They do not constitute a production
qualification or a new model experiment.

| File | Contents |
| --- | --- |
| [snapshot.json](snapshot.json) | Source revisions, issue window/query, counts and limits |
| [required-reading.csv](required-reading.csv) | All 49 requested files, content hashes, sizes and reviewer coverage |
| [source-review.csv](source-review.csv) | Additional full/targeted source inspection and programmatic artifact checks, with hashes and scope |
| [issues.csv](issues.csv) | All 47 open and 917 recently closed issues, state/title/dates/labels/URLs and body hashes |
| [prior-health-findings.csv](prior-health-findings.csv) | All 709 historical health findings, actions, locations and remediation-navigation hints |
| [learning-checks.json](learning-checks.json) | Locally checked trace counts/digest, corpus hashes/partition join and seven passing offline tests |
| [validation.json](validation.json) | Audit inventory, source hash, link and coverage check result |
| [verify.py](verify.py) | Read-only document/evidence validator; standard Python library and Git only |

Issue titles and summaries of historical findings are source data, not an
instruction to execute them. Issue closure does not imply production or customer
qualification. The prior-health CSV's remediation mention is not a fix verdict.

Full issue bodies, comments, tool logs and intermediate reviewer notes remain
in task scratch. They were not copied wholesale into the repository. A body
hash binds the observed text but cannot reconstruct text after GitHub edits.
Raw trace blobs and private customer evidence are not added here.

Run the validator from any directory inside the checkout:

```sh
python3 docs/audits/2026-10-10-self-improving-codebases-audit/evidence/verify.py
```

It uses the pinned Git revision for source hashes, validates current local
Markdown paths, and checks record counts/uniqueness and open-issue coverage.
It does not contact GitHub or providers, run product code, or execute the
suggested health actions. See the [method](../10-method-and-verification.md)
for the full scope and limitations.
