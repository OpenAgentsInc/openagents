#!/usr/bin/env bash
# Explicit, expensive first qualification on original weights. Never a routine gate.
set -euo pipefail
cd "$(dirname "$0")/../.."
if [[ $# != 2 ]];then echo 'usage: qualify_deepseek_v2.sh ORIGINAL_DIRECTORY NEW_OUTPUT_DIRECTORY' >&2;exit 2;fi
original=$(realpath "$1")
out=$2
[[ ! -e $out ]] || { echo 'output directory already exists' >&2;exit 2; }
mkdir -p "$out"
out=$(realpath "$out")
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
# NumPy is oracle tooling only; it is never linked into the model runtime.
python3 -c 'import numpy'
python3 -I psionic/tests/deepseek_v2_artifacts.py --original "$original"
python3 -I psionic/tests/deepseek_v2_text_artifacts.py --original "$original"
"$coax" c psionic/tests/deepseek-v2-runtime.coax --library --release --header "$out/deepseek-v2.h" > "$out/deepseek-v2.c"
"$cc" -std=c11 -O3 -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror \
    -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas -Wno-misleading-indentation \
    -I"$out" -Ipsionic/platform "$out/deepseek-v2.c" psionic/platform/deepseek-v2-memory.c \
    psionic/tests/deepseek_v2_original.c -lm -o "$out/original"
python3 - "$out" "$coax" "$cc" <<'PY'
import hashlib,json,subprocess,sys
from pathlib import Path
p=Path(sys.argv[1]);files=[*Path('psionic/lib').rglob('*.coax'),*Path('psionic/fixtures/deepseek/v2-original').glob('*'),Path('psionic/tests/deepseek-v2-runtime.coax'),Path('psionic/tests/deepseek_v2_original.c'),Path('psionic/platform/deepseek-v2-memory.c'),Path('psionic/platform/deepseek-v2-memory.h'),Path('psionic/tests/deepseek_v2_reference.py'),Path('psionic/tests/v2-qualification-policy.json'),Path(sys.argv[2]),p/'deepseek-v2.c',p/'deepseek-v2.h',p/'original']
data=dict(schema=1,head=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),dirty=bool(subprocess.check_output(['git','status','--porcelain'])),compiler=subprocess.check_output([sys.argv[3],'--version'],text=True).splitlines()[0],flags='-std=c11 -O3 -g -ffp-contract=off -fno-fast-math',files={str(f):hashlib.sha256(f.read_bytes()).hexdigest() for f in files if f.is_file()})
(p/'build-identity.json').write_text(json.dumps(data,indent=2)+'\n')
PY
"$out/original" "$original" psionic/fixtures/deepseek/v2-original "$out" > "$out/stdout.log" 2> "$out/run.log"
OPENBLAS_NUM_THREADS=4 python3 psionic/tests/deepseek_v2_reference.py --original "$original" --capture "$out" > "$out/reference.log" 2>&1
