# Plugin: `plugins.cron_matcher`

Implement `CronSpec` in `plugins/cron_matcher/__init__.py`:
1. `CronSpec.parse(expr: str) -> CronSpec`: parses standard 5-field cron `minute hour dom month dow`:
   - Ranges: minute `0-59`, hour `0-23`, dom `1-31`, month `1-12`, dow `0-6` (accept `7` as alias for `0` = Sunday; Python `datetime.weekday()` has Mon=0..Sun=6 so convert carefully!).
   - Supports `*`, single ints `5`, ranges `1-5`, steps `*/15` and `10-30/5`, and comma lists `1,3,5-7`.
   - Raise `ValueError` on wrong field count, out-of-range numbers, reversed range (`5-2`), or zero/negative step.
2. `matches(dt: datetime) -> bool`:
   - Minute, hour, month must match.
   - Standard Vixie cron DOM/DOW semantics: if BOTH `dom` and `dow` are restricted (neither is `*`), match if EITHER `dom` OR `dow` matches; otherwise both must match.
3. `next_after(dt: datetime) -> datetime`:
   - Return the earliest `datetime` strictly after `dt` (truncated to minute precision, seconds=0, microseconds=0) where `matches(candidate)` is `True`.
   - Search up to 4 years ahead; raise `ValueError` if impossible (e.g. Feb 30).
