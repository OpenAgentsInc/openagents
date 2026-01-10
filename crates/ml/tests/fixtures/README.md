# ML Test Fixtures

The tokenizer fixture is split into line-chunked parts under `tokenizer/part-*.json`
to keep individual files small. The tests will assemble these parts into a temporary
`tokenizer.json` when needed.

To assemble the full tokenizer file manually:

```bash
cat crates/ml/tests/fixtures/tokenizer/part-*.json > crates/ml/tests/fixtures/tokenizer.json
```
