# CoderBench labels (draft)

Schema: [`../labels.schema.json`](../labels.schema.json).

`drafts.jsonl` holds the first ten candidate rows. Every row has
`gradeable: false` and `owner_reviewed: false`. An agent wrote these drafts
from public issue numbers; the owner must review them before any row becomes
a dataset member.

`excluded.jsonl` is the start of the funnel: sessions considered and dropped,
with reasons.

Do not set `gradeable` to true in this tree without an owner review note.
