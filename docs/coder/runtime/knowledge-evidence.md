# Knowledge evidence intake

`knowledge::evidence` reads recorded Microcoder attempts for historical
screening. It does not infer an entry's causal benefit, turn an observed cohort
into a held-out study, or authorize admission. This corrects the intake and
admission defects tracked in [#9677](https://github.com/OpenAgentsInc/openagents/issues/9677).
The [frozen comparison runner](knowledge-studies.md) separately retains
prospective assignments; it does not authorize automatic entry admission.
[#9683](https://github.com/OpenAgentsInc/openagents/issues/9683) owns the
registered transfer cohort.

## Retained population

Every immediate run directory produces one intake record. A missing summary is
incomplete; malformed JSON, duplicate JSON fields, unreadable files, symlinks,
and nonregular summary files remain explicit failures. A directory-listing
failure produces a fault record rather than a successful empty scan. Ordinary
files beside run directories are not attempts. Summaries are bounded to 16 MiB.

A readable summary is retained as exact bytes under its SHA-256 digest, even
when its JSON cannot be parsed. Each report's `openagents.kb-intake.v2` artifact
contains all intake records, with source paths, problems, exact summary artifact
references, recorded knowledge pins, cost components, and comparison identities.
The report separately counts intake faults, unknown membership, changed entry
versions, missing entry pins, incomplete configuration identity, and unverified
prospective declarations. These counts overlap; they are diagnostic dimensions,
not mutually exclusive terminal outcomes.

A missing knowledge list cannot establish that an attempt ran without the
entry. It remains unassigned. A task named in `provenance.written_from` is
excluded. Source matching retains the exact source and its possible run-name
normalization, so numeric task suffixes cannot bypass exclusion. A known different entry digest or explicit version is excluded from
this version's screening groups. A missing digest remains unpinned. A source
exclusion or unreadable record stays in intake even when it cannot contribute
to a two-arm statistic.

## Costs and uncertainty

The required base components are `model_usd`, `jev_usd`, and `embedding_usd` in
`outcome`. A producer can name additional components through a top-level
`required_cost_components` array, but cannot remove the base components.
A required missing, negative, nonnumeric, or nonfinite amount is unknown.
An explicit zero is a recorded zero; a missing field is not zero.

`known_lower_bound_usd` sums valid known components, including spend on attempts
with unknown verifier outcomes. Newer producers retain `outcome.known_usd` for
known calls within an otherwise unknown component; intake preserves that lower
bound without double-counting complete components. Inconsistent lower bounds
or a nonempty producer `cost_unknown` list prevent a comparable total.
`total_usd` exists only when all declared
required components are known. This is the total of the declared cost scope,
not a claim that unrecorded infrastructure is free. Cost per run is null when
any attempt in the screening arm has an unknown outcome or cost. Reports retain
both the known lower bound and the missing-component list.

Unknown verifier outcomes do not become failures or disappear. Any unknown
outcome in a two-arm group makes that group's descriptive direction
inconclusive. A missing cost prevents a cost-based direction. Marginal 95%
Wilson intervals describe the observed pass fractions; they do not measure a
causal entry effect, correct for selection bias, or establish equivalence.

## Comparison identity

A producer can retain this top-level summary object:

```json
{
  "evidence_identity": {
    "schema": "openagents.kb-evidence-identity.v1",
    "harness_digest": "sha256:<64 lowercase hexadecimal characters>",
    "configuration_digest": "sha256:<64 lowercase hexadecimal characters>",
    "environment_digest": "sha256:<64 lowercase hexadecimal characters>",
    "workload_digest": "sha256:<64 lowercase hexadecimal characters>",
    "context_digest": "sha256:<64 lowercase hexadecimal characters>",
    "model": "the exact served model identity",
    "effort": "the effective effort",
    "budget": {"steps": 100, "tokens": 100000, "seconds": 600, "usd": 10.0},
    "partition": "held_out",
    "group": "the source-family identity"
  }
}
```

The placeholders illustrate the fields; they are not accepted digest values.
`configuration_digest` binds all other settings, including decision and
embedding models, question sets, tools, retrieval policy, seeds, and required
cost scope. `context_digest` binds the other entries and instructions shared by
both arms, excluding only the entry under study. `environment_digest` and
`workload_digest` bind exact environment and task artifacts. The producer must
retain those referenced manifests; an opaque digest alone is not independent
proof of what executed.

Every field is retained. Complete declared identities split screening groups;
changing effort, budget, environment, context, partition, or source group creates
a different group. Missing fields remain visible and prevent the reader from
claiming configuration comparability. `model` and a recorded top-level `effort`
must agree with the identity object. Entry exposure separately retains each
entry's `id`, `digest`, and optional explicit `version`; a digest identifies exact
bytes even when the historical producer omitted a version number.

An `assignment` field marks a **prospective declaration, unverified**. This
reader does not establish when that declaration was written, whether it matches
a pre-run frozen protocol, whether every assigned trial exists, or whether the
executor used it. A declaration cannot upgrade a historical report to a
prospective result. The study implementation must verify all those bindings
before exposing an admission path.

## Admission and historical reports

All reports from this reader use `meta.kb.kind: historical_screening`,
`evidence_revision: 2`, `promotion_eligible: false`, and an admission verdict of
`inconclusive`. Descriptive favorable and opposing groups remain visible.
Historical partitions are labeled observational; absence from an entry's
source-task list does not establish a held-out partition.

`kb admit --evidence` refuses these reports and legacy reports with a `pass`
verdict. It checks before promoting a waiting candidate or archiving the current
head. `kb review --apply` cannot automatically demote an entry from historical
correlations. Operator review and explicit withdrawal remain available.

Writing a replacement report preserves the previous exact bytes in
`history/<digest>.json`; referenced content-addressed artifacts stay retained.
Reading a legacy report still returns its original verdict and digest. This
preserves what the earlier software reported without carrying its admission
rule forward.

## Verification scope

Regression tests cover failed intake and exact corrupt bytes, missing and
invalid costs, additional required cost components, unknown outcomes and spend,
changed entry versions, changed comparison identities, unverified study claims,
legacy verdict retention, and refusal before candidate promotion. The
Microcoder relay fixture publishes and reads back the new inconclusive report.
These deterministic fixtures do not measure model quality or knowledge transfer.
