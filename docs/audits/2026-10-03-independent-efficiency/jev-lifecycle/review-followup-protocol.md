# Follow-up: ask about applied implementation

The first 12 review calls recommend verification for every patch and assign
all six demonstrated defective patches a material-defect probability below
0.5. Retain that failure in full.

This second development round permits 12 additional calls after the first
round. It uses the same 12 candidates. Code applies each patch to its pinned
base, then extracts the same family-specific production spans from every
variant. The model sees final implementation and public contracts. It does
not have to mentally apply a diff to an older source excerpt. Tests, recorded
test outcomes, checker text, and candidate labels remain outside this state.
The common extraction rules and omitted source remain recorded.

Each question asks whether one concrete behavior is implemented. The
question set in `bench/jev-lifecycle/focused_review.py` covers the public
task's contract: recovery and lifecycle enforcement, numeric and request-aware
validation, or order-independent record and manifest merging. A question's
answer is a probability of implementing that property. For descriptive
comparison, flag a patch when any answer is below 0.5. This is a development
operating point, not a production completion gate.

The audit already knows these defects and designs these questions after seeing
the first round's failures. This is diagnosis and adaptation on exposed cases,
not independent confirmation. It changes both source representation and
question granularity, so any improvement cannot isolate either cause. Passing
retained checks does not prove that a patch satisfies every new question.
Report all probabilities and flag rates without relabeling cases after inference.

No native executor is run or repaired in this round. Keep the first protocol,
inputs, outputs, costs, and timing separate. The same gateway identity and
single-attempt accounting rules apply.
