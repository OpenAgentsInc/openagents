#!/bin/sh
# Pure Python, so the instrumentation test can run it wherever python3 is.
python3 - <<'PY'
checks = []
def check(x): checks.append(bool(x))
total = 0
passed = 0
def named(name, fn):
    global passed, total
    total += 1
    try: ok = bool(fn())
    except Exception: ok = False
    passed += ok
xs = [3, 1, 2]
check(sorted(xs) == [1, 2, 3])  # follows from the definition of sorting
check(len(xs) == 4); named('max', lambda: max(xs) == 3)
for x in xs: check(x > 1)
named('boom', lambda: 1 / 0 == 0)
if xs[0] == 3: passed += 1
print('SCORE %d %d' % (sum(checks) + passed, len(checks) + total + 1))
PY
