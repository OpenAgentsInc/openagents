#!/usr/bin/env bash
# Explicit bounded original qualification; routine gates replay retained arrays.
set -euo pipefail
cd "$(dirname "$0")/../.."
if [[ $# -lt 2 || $# -gt 3 || ( $# == 3 && $3 != --pair ) ]];then
    echo 'usage: qualify_deepseek_expert.sh ORIGINAL_SHARD_3 NEW_OUTPUT_DIRECTORY [--pair]' >&2;exit 2
fi
original=$(realpath "$1")
out=$2
[[ ! -e $out ]] || { echo 'output directory already exists' >&2;exit 2; }
python3 -c 'import numpy'
python3 -I psionic/tests/deepseek_expert_artifacts.py --original "$original"
mkdir -p "$out"
out=$(realpath "$out")
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
flags=(--mixture)
policy=psionic/tests/expert-mixture-policy.json
if [[ ${3:-} == --pair ]];then flags=();policy=psionic/tests/expert-qualification-policy.json;fi
"$coax" c psionic/tests/deepseek-expert-runtime.coax --library --release --header "$out/deepseek-expert.h" > "$out/deepseek-expert.c"
"$cc" -std=c11 -O3 -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror \
    -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas -Wno-misleading-indentation \
    -I"$out" -Ipsionic/platform "$out/deepseek-expert.c" psionic/platform/deepseek-expert-memory.c \
    psionic/tests/deepseek_expert_original.c -lm -o "$out/original"
python3 - "$out" "$coax" "$cc" "$policy" <<'PY'
import hashlib,json,subprocess,sys
from pathlib import Path
p=Path(sys.argv[1]);files=[*Path('psionic/lib').rglob('*.coax'),*Path('psionic/fixtures/deepseek').glob('expert-*'),Path('psionic/tests/deepseek-expert-runtime.coax'),Path('psionic/tests/deepseek_expert_original.c'),Path('psionic/tests/deepseek_expert_reference.py'),Path('psionic/tests/deepseek_expert_oracle.py'),Path('psionic/platform/deepseek-expert-memory.c'),Path('psionic/platform/deepseek-expert-memory.h'),Path('psionic/platform/spans.h'),Path(sys.argv[4]),Path(sys.argv[2]),p/'deepseek-expert.c',p/'deepseek-expert.h',p/'original']
j=dict(schema=1,head=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),dirty=bool(subprocess.check_output(['git','status','--porcelain'])),compiler=subprocess.check_output([sys.argv[3],'--version'],text=True).splitlines()[0],flags='Coax release C; -std=c11 -O3 -g -ffp-contract=off -fno-fast-math',files={str(f):hashlib.sha256(f.read_bytes()).hexdigest() for f in files if f.is_file()});(p/'build-identity.json').write_text(json.dumps(j,indent=2)+'\n')
PY
"$out/original" "$original" "$out" "${flags[@]}" > "$out/stdout.log" 2> "$out/run.log"
OPENBLAS_NUM_THREADS=4 python3 psionic/tests/deepseek_expert_reference.py "$original" "$out" "${flags[@]}" > "$out/reference.log" 2>&1
