#!/usr/bin/env bash
# Firm reason2/5: actual changed host header with synthetic effects only.
set -euo pipefail
work="${1:?usage: check-panda-clock-host.sh NEW_DIRECTORY}"
[[ ! -e "$work" ]] || { echo 'new output directory required' >&2;exit 2; };mkdir -p "$work/case"
coax="${COAX:-coax/bin/coax}";cc="${CC:-cc}";python="${PYTHON:-python3}"
"$python" - "$work/case" <<'PY'
import pathlib,struct,sys
p=pathlib.Path(sys.argv[1]);(p/'scene.xml').write_text('<synthetic-test-only/>');(p/'init.f64').write_bytes(struct.pack('<45d',.25,*([0.]*44)))
PY
"$coax" c psionic/tests/robotics/panda-clock-session.coax --library --header "$work/panda-session.h" > "$work/debug.c"
"$coax" c psionic/tests/robotics/panda-clock-session.coax --library --release --header "$work/release.h" > "$work/release.c"
for mode in debug release sanitized;do
  source="$work/debug.c";flags=(-O0 -g)
  if [[ "$mode" == release ]];then source="$work/release.c";flags=(-O2);fi
  if [[ "$mode" == sanitized ]];then flags=(-O1 -g -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer);fi
  "$cc" -std=c11 "${flags[@]}" -ffp-contract=off -fno-fast-math -I"$work" -Ipsionic/platform -Ipsionic/tests/robotics "$source" psionic/tests/robotics/panda-clock-host-native.c -lm -o "$work/$mode"
  for scenario in 0 2;do
    mkdir "$work/$mode-scenario$scenario";"$work/$mode" "$work/$mode-scenario$scenario" "$work/case" "$scenario" 0 > "$work/$mode-scenario$scenario.out"
  done
  for fault in 1 2 3 4 5 6 7;do
    mkdir "$work/$mode-fault$fault"
    if "$work/$mode" "$work/$mode-fault$fault" "$work/case" 0 "$fault" > "$work/$mode-fault$fault.out" 2>&1;then echo "unexpected successful fault$fault" >&2;exit 1;else status="$?";fi
    [[ "$status" == 1 && ! -e "$work/$mode-fault$fault/case-fault0/record.json" ]] || { echo "incorrect fault$fault lifecycle" >&2;exit 1; }
    rg -q 'Panda evaluation failed' "$work/$mode-fault$fault.out"
    if rg -q 'AddressSanitizer|UndefinedBehaviorSanitizer|runtime error:' "$work/$mode-fault$fault.out";then echo "sanitizer fault$fault" >&2;exit 1;fi
  done
done
"$python" - "$work" <<'PY'
import pathlib,json,sys
p=pathlib.Path(sys.argv[1])
for mode in ['debug','release','sanitized']:
 for scenario in [0,2]:
  r=json.loads((p/f'{mode}-scenario{scenario}/case-fault{scenario}/record.json').read_text());assert r['ticks']==10 and r['physics_steps']==250 and r['physics_replay_exact'];assert r['model_calls']==(3 if scenario else 2) and r['fallbacks']==int(scenario==2) and r['refusals']==int(scenario==2)
print('Actual host-header seam: normal/delayed+7faults in debug/release/ASan; synthetic effects only; no completed fault ledger.')
PY
