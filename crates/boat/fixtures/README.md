# Boat contract fixtures

| Directory | Contents | Written by |
| --- | --- | --- |
| `spec/` | One file per operation in `../schema/operations.json` (69), plus `index.json`: the request target, parameters and body, the success schema sample, `oneOf` variant samples, published examples, and a body for each failure status | `python3 crates/boat/schema/fixtures.py` (offline; also rewrites `tests/fixtures.rs`) |
| `observed/` | 96 redacted vendor captures, ported from the private Coder repository's `crates/coder-box/fixtures/observed/` (c8821c72eb, captured 2026-09-05/06 against the Box API) | Ported once on 2026-10-02 |
| `recorded/` | Current read-only responses (`/me`, `/orgs`, `/limits`, `/sandboxes`, `/environments`, `/named-snapshots`, `/api-keys`), redacted | `schema/capture.py` with `BOAT_API_KEY` |

## The port

Each observed file keeps its original `source` (Box base URL, the Box spec
SHA-256 and SDK commit it was captured with) and adds a `ported` block naming
the renames applied: `/boxes/{boxId}` paths became `/sandboxes/{sandboxId}`;
operation ids `boxes`, `deleteBox`, `listBoxSnapshots` and
`getLatestBoxSnapshot` became `sandboxes`, `deleteSandbox`,
`listSandboxSnapshots` and `getLatestSandboxSnapshot`; keys such as `box`,
`boxes`, `activeBoxes`, `maxActiveBoxes`, `boxCount`, `boxPlan*`,
`passBoxCredentials` and `sourceBoxId` took their `sandbox` names; and
`box.*` type and event values became `sandbox.*`. Vendor message text is kept
as captured.

A capture marked `known_deviation` must still fail to decode, so the test
notices when the schema or the server changes:

- `apiKeys`: captured before `GET /api-keys` returned the required `catalog`
  (the current shape is `recorded/apiKeys.json`).
- `interrupt.archived`: a 500 without the error envelope.

No key appears here; `tests/fixture_tree.rs` checks for key-shaped strings.
