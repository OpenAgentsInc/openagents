# Plugin: `plugins.log_parser`

Implement `parse_and_summarize(lines: list[str]) -> dict` in `plugins/log_parser/__init__.py`:
1. Each log line has format: `TIMESTAMP LEVEL SERVICE key1=val1 key2="quoted val" ...`
   Example: `2026-11-26T10:00:01Z INFO api status=200 latency_ms=42 path="/v1/items"`
2. Ignore blank lines and lines starting with `#`.
3. Malformed lines (missing timestamp/level/service or unclosed quote) must be counted in `malformed_count` and skipped (do not raise).
4. Normalize `LEVEL` to uppercase (`DEBUG`, `INFO`, `WARN`, `ERROR`).
5. Parse `key=value` pairs supporting double-quoted values with spaces and `\"` escapes.
6. Return a dict with keys:
   - `"total_valid"`: int
   - `"malformed_count"`: int
   - `"by_level"`: dict[str, int] (only levels observed, count > 0)
   - `"by_service"`: dict[str, dict] where each service has `{"count": int, "errors": int, "avg_latency_ms": float}` (rounded to 2 decimals; `0.0` if no `latency_ms` fields for that service; `"errors"` counts `LEVEL == "ERROR"` or numeric `status >= 500`).
