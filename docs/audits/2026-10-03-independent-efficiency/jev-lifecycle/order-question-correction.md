# Correcting an overconstrained review question

An independent review of round two found an extra implementation requirement in
the `equal_rank` question: it required a content-based tie breaker. The public
Gym task requires directory-order independence. A stable source-path order can
satisfy that requirement. The original probabilities therefore cannot all be
interpreted as review mistakes against the actual task.

Retain the original question, outputs, and flag counts. The third development
round permits six additional calls, one for each Gym case. It keeps the exact
applied-source state and the `copy_marks` question. The revised `equal_rank`
question permits either a stable path or content order and asks about the
observable directory-order result. The `conflicting_marks` question explicitly
allows a canonical ordering in the caller before first/last-value map updates.
Neither question requires a particular algorithm. The output remains one Noul
per property; the descriptive flag rule remains any value below 0.5.

This is an evaluator correction on exposed development cases after seeing both
labels and model answers. It does not turn these cases into unseen tests or
establish that the repaired questions generalize. The exact old and new questions
are retained; compare their requests and responses rather than replacing the
earlier result.
