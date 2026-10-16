#!/usr/bin/env bash
# Confirm the no-inflation proof rejects a one-satoshi payout defect.
set -euo pipefail
cd "$(dirname "$0")/.."
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/work/openagents-target-agent2}"
if [[ -n "${LAYERTREE_PROOF_SCRATCH:-}" ]]; then
  proof_scratch="$LAYERTREE_PROOF_SCRATCH"
elif command -v openagents >/dev/null 2>&1; then
  proof_scratch="$(openagents scratch)"
else
  proof_scratch="$HOME/.cache/openagents/layertree-proofs"
fi
mkdir -p "$proof_scratch"
mutant="$(mktemp -d "$proof_scratch/layertree-mutant.XXXXXX")"
python3 - "$mutant" <<'PY'
import pathlib, shutil, sys
source = pathlib.Path('crates/layertree-tee-lab')
target = pathlib.Path(sys.argv[1])
shutil.copytree(source / 'src', target / 'src')
for name in ('Cargo.toml', 'Cargo.lock'):
    shutil.copy2(source / name, target / name)
p = target / 'src/predicate.rs'
s = p.read_text()
needle = 'amount: leaf.amount,'
if s.count(needle) != 1:
    raise SystemExit('The exit payout changed; update the mutation check.')
p.write_text(s.replace(needle, 'amount: leaf.amount + 1,'))
PY
set +e
cargo kani --manifest-path "$mutant/Cargo.toml" --lib -Z stubbing \
  -Z unstable-options --harness ExitK_no_inflation --output-format terse \
  --export-json "$mutant/result.json" > "$mutant/proof.log" 2>&1
result=$?
set -e
if [[ "$result" == 0 ]]; then
  echo "The one-satoshi defect passed the no-inflation proof." >&2
  exit 1
fi
python3 - "$mutant/result.json" <<'PY'
import json, pathlib, sys
p = pathlib.Path(sys.argv[1])
if not p.exists():
    raise SystemExit('Kani did not produce a verification result; inspect proof.log.')
data = json.loads(p.read_text())
def failed_assertion(value):
    if isinstance(value, dict):
        text = json.dumps(value)
        if str(value.get('status', '')).upper() in ('FAILURE', 'FAILED') and 'ExitK.no_inflation' in text:
            return True
        return any(failed_assertion(v) for v in value.values())
    if isinstance(value, list):
        return any(failed_assertion(v) for v in value)
    return False
if not failed_assertion(data):
    raise SystemExit('The mutation did not fail the named no-inflation assertion; inspect result.json.')
print('The one-satoshi defect fails ExitK.no_inflation.')
PY
printf 'Evidence: %s\n' "$mutant"
