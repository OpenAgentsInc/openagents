#!/bin/bash
# Passes when the collect command's copy of the agent's result arrived and
# the package the image installed imports.
python3 - <<'PY'
import json, pathlib
import six  # installed when the verifier image was built
ok = pathlib.Path("/app/out/collected.txt").is_file() and pathlib.Path("/app/out/collected.txt").read_text().strip() == "42"
pathlib.Path("/logs/verifier/reward.json").write_text(json.dumps({"reward": 1.0 if ok else 0.0, "checks": 1}))
print("collected ok" if ok else "collected.txt missing or wrong")
PY
