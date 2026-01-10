# OpenAPI Spec Parts

The full OpenAPI spec is split into line-chunked parts under `openapi/parts/` to keep
individual files small. To reassemble the spec into a single file:

```bash
cat crates/opencode-sdk/openapi/parts/part-*.json > crates/opencode-sdk/openapi.json
```

You can also run `crates/opencode-sdk/scripts/assemble-openapi.sh`.
