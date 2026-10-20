#!/usr/bin/env bash
# Keep an existing CUDA build and add a host-optimized CPU backend for offload.
set -euo pipefail

source_dir=${1:?Usage: prepare-llama-runtime.sh LLAMA_CPP_SOURCE [CPU_BUILD] [OUTPUT]}
build_dir=${2:-$HOME/work/pylon-runtimes/llama-cpu-avx2}
output_dir=${3:-$HOME/work/pylon-runtimes/llama-cpp-avx2}
[[ $(uname -s) == Linux && $(uname -m) == x86_64 ]] || { echo 'This recipe requires an x86-64 Linux AVX2 host.' >&2; exit 1; }
for feature in avx2 fma f16c; do
  rg -q "\\b$feature\\b" /proc/cpuinfo || { echo "This CPU needs $feature support." >&2; exit 1; }
done
[[ ! -e $output_dir ]] || { echo "Choose a new output folder; $output_dir already exists." >&2; exit 1; }
vnni=OFF
if [[ -r /proc/cpuinfo ]] && rg -q '\bavx_vnni\b' /proc/cpuinfo; then vnni=ON; fi

# Nix may strip -march=native. Explicit switches produce the intended backend.
cmake -S "$source_dir" -B "$build_dir" -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DGGML_CUDA=OFF -DGGML_NATIVE=OFF \
  -DGGML_AVX=ON -DGGML_AVX2=ON -DGGML_FMA=ON -DGGML_F16C=ON \
  -DGGML_AVX_VNNI="$vnni" -DLLAMA_BUILD_TESTS=OFF \
  -DLLAMA_BUILD_EXAMPLES=OFF -DLLAMA_BUILD_SERVER=OFF
cmake --build "$build_dir" --target ggml-cpu -j4

mkdir -p "$(dirname "$output_dir")"
staging=$(mktemp -d "${output_dir}.XXXXXX")
trap 'rm -rf "$staging"' EXIT
mkdir "$staging/bin"
cp -a "$source_dir/build/bin/llama-server" "$staging/bin/"
cp -a "$source_dir"/build/bin/*.so* "$staging/bin/"
for file in "$build_dir"/bin/libggml-cpu.so.*.*.*; do
  cp "$file" "$staging/bin/$(basename "$file")"
done

LD_LIBRARY_PATH="$staging/bin:${LD_LIBRARY_PATH:-}" python3 - "$staging" "$source_dir" <<'PY'
import ctypes
import hashlib
import json
from pathlib import Path
import subprocess
import sys

runtime, source = map(Path, sys.argv[1:])
cpu = next((runtime/'bin').glob('libggml-cpu.so.*.*.*'))
library = ctypes.CDLL(str(cpu))
features = {name: bool(getattr(library, 'ggml_cpu_has_'+name)()) for name in ('avx', 'avx2', 'fma', 'f16c', 'avx_vnni')}
if not all(features[name] for name in ('avx', 'avx2', 'fma', 'f16c')):
    raise RuntimeError('The compiled CPU backend is missing required AVX2/FMA/F16C support')
record = {
    'source_commit': subprocess.check_output(['git', '-C', str(source), 'rev-parse', 'HEAD'], text=True).strip(),
    'cpu_features': features,
    'cpu_library_sha256': hashlib.file_digest(cpu.open('rb'), 'sha256').hexdigest(),
    'server_sha256': hashlib.file_digest((runtime/'bin/llama-server').open('rb'), 'sha256').hexdigest(),
    'cuda_backend': 'Unchanged from the existing build at the same source revision',
}
(runtime/'runtime.json').write_text(json.dumps(record, indent=2)+'\n')
print(json.dumps(record))
PY
mv "$staging" "$output_dir"
trap - EXIT
printf 'Runtime installed at %s/bin/llama-server\n' "$output_dir"
