# Open Responses spec examples

Examples copied from the Open Responses specification, version 2026-04-24
(`github.com/openresponses/openresponses`):

| File | Source |
| --- | --- |
| `response-resource.json`, `response-param.json` | `src/examples/` |
| `items.json`, `events.json`, `error.json` | the examples in `src/specifications/2026-04-24.mdx` |
| `requests.json`, `response-phase.json` | the acceptance suite's request bodies and mock response, `src/lib/compliance-tests.ts` |

The image in `image-input` is a smaller PNG than the suite's, and one long
`output_text` in `events.json` is shortened; nothing else is changed. The
recorded upstream streams the tests also read live in
`crates/coder/fixtures/gateway/`.
