# Frame and affine-operator admission policy before source integration

The [new producer](../../coax/lib/compiler/spatial-policy.coax) checks nominal
frame/unit/representation descriptors and the proposed affine point/vector
operator table. Frame metadata admits declared handedness, convention,
continuity and minimum precision. Point descriptors require physical length;
vector descriptors accept any admitted unit, with no automatic angular,
pseudovector, transform or temporal interpretation. Matching IDs remain
nominal. Refusal payloads are zero and precedence is explicit.

Eight authored families cover 1,404 new cases plus six loaded unchanged unit-policy
checks. Changed-root mutation kills 158/158 with zero survivors or equivalent
waivers. An independent explicit table supplies 98,064 results/588,384 logical
words in each debug/release/fail-fast ASan+UBSan native mode: all exact, empty
stderr. The [receipt](../../coax/tests/spatial-policy/qualification.json)
binds source, oracle, transport, private 0.43 compiler and actual outputs.

This standalone producer is registered without rerunning its old passing
proofs or the full compiler suite. It does not implement @frame parsing,
Point3/Vector3 source types, coordinate arithmetic, generated aggregate ABI,
reflections/transforms, temporal realizations or robot/Verse migration.
Those are the next R12 compiler slice; full R12 remains broader still. Current
robotics book references are 189–191; dated historical records keep their names.
