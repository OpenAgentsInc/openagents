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

Runtime refusals, unavailable results, cancellation, and mixed outcomes need
an executing service. The gateway HTTP tests in `crates/gateway/tests/serve.rs`
and caller subprocess tests in `crates/oak/tests/cli.rs` exercise those
boundaries. This request corpus does not replace them or assert that every
runtime outcome has a standalone response fixture.
