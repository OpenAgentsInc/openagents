# cron

`next_run(expr, after)` in `cron.py` returns the first time a cron expression matches that is
strictly later than `after` (a naive `datetime`). The result has seconds and microseconds of 0.

An expression has five fields separated by spaces: minute (0-59), hour (0-23), day of month
(1-31), month (1-12) and day of week (0-7, where both 0 and 7 are Sunday, 1 is Monday).

Each field is a comma-separated list of items. An item is `*`, a number, a range `a-b`, or any of
those three followed by a step `/n` (`*/15`, `10-30/5`, and `5/20`, which means from 5 to the
field's largest value in steps of 20).

Day of month and day of week follow the classic cron rule: when both fields are restricted
(neither is `*`), a day matches when either field matches. When only one is restricted, that
one decides.

`next_run` raises `ValueError` for an expression that is not valid: the wrong number of fields, a
value out of range, a range that runs backwards, a step of 0, or anything that is not a number.
An expression that can never match (`0 0 31 2 *`) also raises `ValueError`.
