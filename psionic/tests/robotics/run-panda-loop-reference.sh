#!/usr/bin/env bash
# Firm reason2/5: isolated installed-framework oracle, never a product runtime.
set -euo pipefail
python="${GROOT_REFERENCE_PYTHON:-$HOME/.cache/psionic/groot-reference-env/bin/python}"
sdk="${MUJOCO_ROOT:-$("$python" -c 'import importlib.util,pathlib;print(pathlib.Path(importlib.util.find_spec("mujoco").origin).parent)')}"
export DYLD_LIBRARY_PATH="$sdk${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
export PYTORCH_ENABLE_MPS_FALLBACK=0
export PYTORCH_MPS_FAST_MATH=0
exec "$python" psionic/tests/robotics/panda_reference_policy.py "$@"
