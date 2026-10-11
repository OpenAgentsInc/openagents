# filefind ranker promotions (#11312)

Every change to the active ranker (`scripts/filefind/model.json`) that the
learning loop made, newest first. A promotion happens only through the gate
(`scripts/filefind/ranker_gate.py`): a candidate retrained with the accepted
briefed runs' rows (`scripts/work/learn.py`) is measured against the active
model on the held-out window, paired per issue, and is activated only when
recall@50 rises by at least 2 SE and recall@20, recall@100 and calibration do
not fall by more than 2 SE, and only when top-8 recall (what a briefing lists)
on the briefed-agent bench's tasks, the goldens, does not fall by more than
2 SE either. The weekly run is `scripts/work/learn-job/` on `oa-learn-1`
(docs/inference/briefed-agent-ab.md, "Learning loop"). The receipt of the latest promotion is
`scripts/filefind/model.receipt.json`; every week's report, passing or not,
is in `gs://openagentsgemini-filefind-learn/reports/` and on #11313.

<!-- promotions: newest first -->
