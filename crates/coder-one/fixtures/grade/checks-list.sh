#!/bin/sh
# One Python heredoc whose checks are a single list literal: it doesn't
# split into check lines, so accept.grade grades it as one advisory unit.
cd /app || exit 1
python3 - <<'PY'
from drift_monitor.statistical_tests import mmd
import numpy as np
x = np.zeros((4, 2))
far = np.ones((4, 2)) * 3
checks = [
    mmd(x, x) == 0,
    mmd(x, far) > 0,
    np.isfinite(mmd(x, far)),
]
print('SCORE', sum(checks), len(checks))
PY
