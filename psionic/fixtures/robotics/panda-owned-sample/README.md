# A fresh owned controller sample

`pcs_forward` admits seven radian coordinates, two metre coordinates, seven
radian-per-second rates, two metre-per-second rates and one `PcSample` output.
The raw adapter accepts two packed nine-value views. All shapes precede values;
positions use the qualified FK order, then rates check number before range in
arm/finger order. Inclusive numerical boxes are 100 radians, 1 metre, 20 rad/s
and 2 m/s. Refusal preserves the complete caller record.

One fresh owned finger/FK result feeds the mass metric, grip Jacobian and
zero-acceleration inverse force. The source profile is locally constructed.
World site velocity is the typed Jacobian/rate product; angle units require
explicit conversion before the existing raw ABI. `PcSample` remains exactly
144 F64 words in its existing order. Its position and rotation belong to the
grip SITE, not the hand BODY. Moving finger rates can affect bias; zero-rate
gravity alone cannot replace it.

The constructor owns pose/profile admission. Its source-visible low-level
packing and arithmetic helpers are unsealed and trust their inputs; writable
PODs are not model or sensor provenance certificates. This query establishes
no clock stamp, observation generation or compiler runtime bound.

[The experiment](experiment.json) froze per-field absolute budgets before
owned comparison. Fifteen matched SDK capture contexts supply all 2,160 fields;
some q/v states repeat across acceleration patterns, so this is not fifteen
independent states. All three native profiles pass 9,811 assertions each,
including actual nonfinite values, inclusive rate/coordinate endpoints,
canaries and four overlapping-output success/refusal cases. Complete outputs
agree byte for byte under the named Darwin-arm64 scalar-libm profile.
The [unchanged first receipt](native-qualification.json) preserves executed
source, compiler, input, C/header and binary identities. Three separate
[logical-unit refusals](unit-refusals.json) qualify the typed boundary.

The first authored mutation run found three survivors in the constructor:
a shape guard duplicated delegated FK admission, and typed/raw agreement alone
did not catch missing public q/qd copies. The redundant guard was removed and
independent expected public q/qd checks were added. The
[changed-procedure scope](mutation-forward-scope.json) kills all 29 remaining
constructor mutants. Five unchanged product bodies retain their exact initial
zero-survivor evidence; all original checks remain and the dense check is
stronger. No survivor is waived and no aggregate current-site count is invented.
The initial source/log are retained as development evidence.

```shell
python3 psionic/tests/robotics/panda_owned_sample_mutation_scope.py --output /tmp/new-pcs-scope
coax/bin/coax mutate /tmp/new-pcs-scope/root.coax --library --root-only
```

The scope tool was bound by exact entry/dependency bytes to the passing private
scope; its publication did not repeat mutation execution. The ordinary focused
gate is `psionic/check.sh --only robotics/panda-owned-sample`.

The [example golden](example-reference.json) was written from SDK capture 13
before the runnable example. Display rounding does not change the F64 library
result. [Qualification](qualification.json) keeps the component scopes separate.
Paired controller output/state qualification is separate from field closeness;
neither is a new SDK-stepped or learned-policy episode, hardware qualification
or an owned contact simulator.
