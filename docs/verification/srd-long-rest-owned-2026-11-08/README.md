# Clock-bound rest admission and activity ownership

Base: `9b218b4d7f`. Source: pinned SRD 5.2.1 p185;
PDF identity is in `docs/dnd/long-rest-implementation.md`.

`long-rest-owned.coax` binds actor operations to the battle's validated
microsecond rest clock. Starting/resuming sleep retains actual Unconscious
ownership before committing the journal; full effect storage refuses that
activity without evicting records. Starting/resuming watch wakes natural sleep
while preserving independent Unconscious sources. Invalid actors, dead/zero-HP
actors, borrowed Reaction frames, poisoned clocks and mismatched journal units
refuse. `settled` is an authoritative world observation, not a client flag;
world controls must establish it before calling start/resume.

Activity changes first account the old mode. Their result separates the
observed boundary from admission of the requested mode: even a refused sleep
transition cannot discard the true earlier light progress or an earlier
completion/interruption. Exact two-hour watch permits switching to sleep at
that timestamp. Reporting continued light activity *after* the limit abandons
the invalid period without crediting the gap as sleep or creating a listed
interruption/extra-hour charge. A new rest starts from its actual new timestamp.
An earlier valid completion wins over a later light limit or activity request.

Exertion stops at its first one-hour boundary and reports Short Rest eligibility
once; resumption uses the current clock and excludes the paused gap. The caller
must consume that boundary before permitting resumption or later causal events.
This layer reports eligibility, not automatic healing or optional choices.

## Verification

- Final composed `long-rest-resources-checks.coax --library`: 296 checks pass.
  Six new owned-operation checks were written before implementation; initial
  storage fixtures were corrected to retain effects beyond their tick rather
  than relying on already-expired records. Checks were sharpened against
  surviving admission/resumption mutants.
- Owned operation unit: 143 mutants killed, zero survivors/exemptions.
- Journal declaration gained the additive `abandoned` observation flag; its
  procedure policy is unchanged. With the composed owner consumers, the changed
  unit kills all283 mutants with zero survivors/exemptions. Total across the
  two changed authority units:426 killed, zero survivors.
- Updated compiled `rest-clock-native.coax`:
  `PASS REST-CLOCK-OWNED (1843200 rest frames, precise admission and late-watch rejection)`.
  It uses owned start/sync operations across all 1,843,200 actual 64 Hz rest
  ticks, verifies no early completion, applies the pending boundary, and checks
  sixteen-hour readmission. A second watch exceeds the limit by one second;
  attempting sleep abandons it and a new admitted rest finishes eight hours
  after its new start. It also retains the compiled overflow/carry witness.
- `coax/web/tool.sh tools-check`: 1897 tracked tools, 35 vendored, no missing
  reasons. `git diff --check` passes.
- Full checked Lab Wasm composition passes in41.2s, 2,540,042 bytes:

```
coax/web/wasm.sh apps/openagents-web/fivelab.coax apps/openagents-web/fivelab-face.c /tmp/srd-long-rest-owned.wasm apps/openagents-web/everglade-step.coax aethyr/sticks.coax
```

Wasm SHA256:
`7727d086852fd5d6d83acdbc14dc76956fdd7920380e7117ed91295b9ae7d4d2`.
Existing compiler source-version and near-limit work/value advisories remain
(value slots86%); no budget was raised. No shipping size budget was added for
the temporary artifact name. Checked composition does not prove an exposed
world rest flow; uncalled engine operations may be eliminated from Wasm.

Mutation uses `coax/bin/coax mutate
 aethyr/rules/srd/long-rest-resources-checks.coax --library --unit UNIT --jobs 4`.
Source SHA256:

- owned operations: `13dc2d6b0cb9986f6e469e94f40b41da7278a4f4a5580ad7f51a6320b6512437`
- journal: `688a661dc206414788d0f896c0f7b805588d4211c2e1e8e7d784040e6702db84`
- native entry: `5c5c80a282d309011c8f93302d3585f83322c581eeff61631173ffc1270442ab`

## Required remaining integration

Automatic actor-journal scheduling inside the world's step and causal
Initiative/damage/casting producers are still required. In particular, effect
expiry and completion benefits must be ordered at their actual timestamps,
not applied after a later event merely because an owner queried late. Pending
completion intentionally reports a boundary; it does not heal during the
activity query. The native witness consumes that boundary explicitly.

The world must supply actual activity and settled-downtime observations,
consume Short Rest fallback/optional choices, and preserve timeline/actor
ownership through rejoin. Source loss/nutrition producers, general inventory
dropping and rest controls remain separate work. No raw client journal or
permission admission is exposed. #13040 remains open.
