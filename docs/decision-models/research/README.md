# Research

Leads worth following, and what would have to be true for each to matter.

A page belongs here when it is a question rather than a finding. When a lead
is settled it moves: into an implementation's directory if it changes what we
build, into [`../others/`](../others/) if it is a reading of someone else's
work, or into a measurement record if it produced numbers.

Each page should open by saying what would change if the lead holds. A lead
that would change nothing is not worth the reading time, and saying so is a
result.

| Lead | Question | State |
| --- | --- | --- |
| [`2026-09-19-inference-side-scoring.md`](2026-09-19-inference-side-scoring.md) | Does an inference engine's scoring endpoint turn any open model into a decision model, without training a readout? | **settled**: mechanism real, quality claim contradicted by a public paired measurement |
| [`2026-09-19-question-text-optimization.md`](2026-09-19-question-text-optimization.md) | Is the question text a tunable parameter, and does a gain found on one door transfer to the others? | lead open, **tool settled**: its own authors' benchmark shows the technique lowering Jev's accuracy |
| [`2026-09-19-specialist-classifiers.md`](2026-09-19-specialist-classifiers.md) | Does an hour of fine-tuning on one task make a general decision model pointless? | **settled**: direction right, claims do not hold, and it corrected a number of ours |
