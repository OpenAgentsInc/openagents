# Classification v1 fixtures

`manifest.json` binds request filenames to expected results from the actual
gateway planner under the product limits. Run the corpus with:

```sh
cargo test --locked -p oak --test classification_contract
```

The corpus covers all four modes, Unicode records, overlapping independent
labels, empty input, duplicate IDs, unknown fields, conflicting content forms,
unsupported versions, the 1,000-input/100-label shape, 20 dimensions, and the
1,000-judgment bound. Maximum-shape fixtures establish envelope support, not
backend capacity or quality. Another test applies a tighter backend limit and
requires refusal. Planning leaves every judgment unattempted.

The [request JSON Schema](../../schemas/classify-request-v1.json) describes
structural fields and product array limits. The gateway also validates byte
limits, IDs, mode-dependent fields, policy compatibility, and total judgments.
Its checks remain authoritative. A schema-valid request can still be refused.
The schema's `$id` identifies the document; it is not a hosted discovery route.

Thresholds in these fixtures are synthetic selection inputs, not calibrated
recommendations. Fixture text does not establish multilingual model support.

## Runtime response fixtures

`responses/` contains reports produced by real gateway HTTP tests against
bounded synthetic backends: categorical, independent multi-label, binary with
refusal, score, full refusal, unavailable, mixed completion, corrected review,
invalid review with original retention, and opt-in fallback. The mixed
report includes queued work that never dispatched. Elapsed `latency_ms` values are normalized to zero. Run-specific attempt,
receipt, and usage references are replaced with stable placeholders while
preserving reference equality. These examples are neither latency evidence nor
valid receipt chains. Separate runtime tests verify actual receipt hashes and
response bindings. Native scores, usage completeness, input order, and model
identity must match exactly.

Run `cargo test --locked -p gateway --test serve` to compare the reports with
the executing gateway. Run the Oak corpus test above to decode every answered
primitive through the existing Jev SDK. After reviewing an intentional wire
change, regenerate the response files with:

```sh
UPDATE_CLASSIFY_FIXTURES=1 cargo test --locked -p gateway --test serve classify_
```

The [response JSON Schema](../../schemas/classify-response-v1.json) constrains
native item, unit, aggregate, selection, identity, and usage shapes. Distribution
mass, label membership, aggregate arithmetic, and policy semantics still need
runtime checks. Nested review records, attempts, fallback, and secondary usage
are covered structurally too; schema validation does not verify a receipt hash.
A disconnected caller receives no cancellation response; transport cancellation
and durable settlement need their own runtime evidence, not a fabricated JSON
report. This corpus does not yet establish that cancellation requirement.

To validate both JSON Schemas and reject malformed response mutations, install
`jsonschema==4.25.1` in an isolated Python environment and run
`python scripts/check-classification-schemas.py`. This additional schema check
is separate from the Rust manual gate; report both results when changing the
published contract.
