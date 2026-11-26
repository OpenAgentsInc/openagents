# Plugin: `plugins.json_patch`

Implement `apply_patch(doc, ops: list[dict])` in `plugins/json_patch/__init__.py` implementing RFC 6902 JSON Patch (deep-copying `doc`, never mutating the caller's input):
1. Support JSON Pointer (RFC 6901) with `~1` -> `/` and `~0` -> `~` unescaping in that exact order, plus `-` array append token.
2. Support all 6 operations: `add`, `remove`, `replace`, `move`, `copy`, `test`.
3. `add` on an object inserts or replaces key; on an array inserts at index `0..len` or appends for `-` (reject out-of-range or leading-zero indices like `"01"` with `ValueError`).
4. `remove` deletes existing object key or array index (raise `ValueError` if missing).
5. `replace` replaces existing target (raise `ValueError` if target does not exist).
6. `move` removes `from` and adds at `path` (raise `ValueError` if `path` is a proper child of `from`).
7. `copy` deep-copies `from` to `path`.
8. `test` checks exact structural & type equality (`True != 1`, `False != 0`!) and raises `ValueError` on mismatch.
9. Unknown op or missing parent path raises `ValueError`.
