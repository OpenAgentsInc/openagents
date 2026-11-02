# Measurement clarification

Written before offline NLL/confidence reporting, after main generation began.
This clarification changes no candidate, question, selection or finite goal.

The frozen protocol says “teacher-forced log loss and confident wrong
first-channel choices.” The recorded `answer/` prefixes already force
`assistant` / `final` / message marker and capture the next-token logits.
The implemented report therefore measures **first answer-token** NLL at
temperature one and **confident wrong first answer-token**, not a channel
choice and not full-sequence language-model loss. The report and chapter
use the actual measurement name. This does not satisfy a full-sequence
loss or channel-confidence experiment. No such result is claimed.

The main generation failures still use their actual ordinary-prefix traces.
The forced-final loss proxy cannot substitute for their answer accuracy,
completion, token cost or the independent quality gate.
