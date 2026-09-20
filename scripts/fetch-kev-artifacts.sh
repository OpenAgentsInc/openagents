#!/usr/bin/env bash
# Fetch one kev variant's weights and its base checkpoint from the Hugging
# Face Hub into the artifact layout crates/kev loads, and verify every file
# against the digests the variant's fixture manifest pins.
#
# Usage: scripts/fetch-kev-artifacts.sh [<variant>...]
#
#   <variant>   kev-0.5b (default), kev-0.6b, kev-4b, or kev-8b.
#
# Environment:
#
#   KEV_ARTIFACTS   Artifact root. Defaults to ../kev-artifacts beside the
#                   workspace, which is where the kev tests look.
#   HF_TOKEN        Sent as a bearer token when set; the checkpoints are
#                   public and none is needed.
#
# The Hub repo ships the pointer head as `head.pt`, a torch pickle, and the
# port loads `head.safetensors` plus `head_meta.json`. The conversion is
# the one gen_fixtures.py performs, run here in a private venv with a CPU
# torch, so the resulting digests match the manifest byte for byte. Python
# is used only for that conversion; nothing here becomes product code.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
root="${KEV_ARTIFACTS:-$(dirname "$here")/kev-artifacts}"
variants=("$@")
if [ "${#variants[@]}" -eq 0 ]; then
  variants=(kev-0.5b)
fi

auth=()
if [ -n "${HF_TOKEN:-}" ]; then
  auth=(-H "Authorization: Bearer $HF_TOKEN")
fi

fetch() { # repo revision file dest
  local url="https://huggingface.co/$1/resolve/$2/$3"
  if [ -s "$4" ]; then
    return 0
  fi
  echo "  $3"
  curl -fsSL --retry 3 -C - "${auth[@]}" -o "$4.part" "$url"
  mv "$4.part" "$4"
}

sha() { sha256sum "$1" | cut -d' ' -f1; }

manifest_for() {
  case "$1" in
    kev-0.5b) echo "$here/crates/kev/fixtures/manifest.json" ;;
    *) echo "$here/crates/kev/fixtures/variants/$1/manifest.json" ;;
  esac
}

venv="$root/.convert-venv"
ensure_converter() {
  if [ -x "$venv/bin/python" ] && "$venv/bin/python" -c "import torch, safetensors, numpy" 2>/dev/null; then
    return 0
  fi
  echo "creating a CPU torch venv at $venv for the head.pt conversion"
  python3 -m venv "$venv"
  "$venv/bin/pip" install --quiet --upgrade pip
  "$venv/bin/pip" install --quiet torch --index-url https://download.pytorch.org/whl/cpu
  "$venv/bin/pip" install --quiet safetensors numpy
}

for variant in "${variants[@]}"; do
  manifest="$(manifest_for "$variant")"
  if [ ! -f "$manifest" ]; then
    echo "no fixture manifest for $variant at $manifest" >&2
    exit 2
  fi
  adapter="$root/$variant"
  mkdir -p "$adapter"
  echo "$variant -> $adapter"
  for file in adapter_config.json adapter_model.safetensors eval.json head.pt \
      merges.txt special_tokens_map.json tokenizer.json tokenizer_config.json vocab.json; do
    fetch "jaredpalmer/$variant" main "$file" "$adapter/$file"
  done

  ensure_converter
  "$venv/bin/python" - "$adapter" <<'EOF'
import json, os, sys, torch
from safetensors.torch import save_file
adapter = sys.argv[1]
meta = torch.load(os.path.join(adapter, "head.pt"), map_location="cpu", weights_only=False)
save_file(meta["head"], os.path.join(adapter, "head.safetensors"))
with open(os.path.join(adapter, "head_meta.json"), "w") as f:
    json.dump({k: v for k, v in meta.items() if k != "head"}, f, indent=2)
EOF

  # The base checkpoint is named by the head's `base`, the way the fixture
  # README states: `Qwen/Qwen3-0.6B-Base` becomes `qwen3-0.6b`.
  base_id="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["base"])' "$adapter/head_meta.json")"
  base_revision="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("base_revision") or "main")' "$adapter/head_meta.json")"
  base_dir="$root/$(echo "${base_id#*/}" | sed -e 's/-Base$//' | tr '[:upper:]' '[:lower:]')"
  mkdir -p "$base_dir"
  echo "$base_id@$base_revision -> $base_dir"
  fetch "$base_id" "$base_revision" config.json "$base_dir/config.json"
  shards="$(curl -fsSL "${auth[@]}" "https://huggingface.co/api/models/$base_id/revision/$base_revision" \
    | python3 -c 'import json,sys; print("\n".join(s["rfilename"] for s in json.load(sys.stdin)["siblings"] if s["rfilename"].endswith(".safetensors")))')"
  for shard in $shards; do
    fetch "$base_id" "$base_revision" "$shard" "$base_dir/$shard"
  done

  # Verify against the pinned digests. A file the manifest does not name
  # (head.pt, the base shards) is not checked here; the conformance tests
  # are the check on the base.
  bad=0
  while IFS=' ' read -r file expected; do
    actual="$(sha "$adapter/$file")"
    if [ "$actual" != "$expected" ]; then
      echo "  DIGEST MISMATCH $file: $actual != $expected" >&2
      bad=1
    fi
  done < <(python3 -c 'import json,sys
for name, file in json.load(open(sys.argv[1]))["files"].items():
    print(name, file["sha256"])' "$manifest")
  if [ "$bad" -ne 0 ]; then
    echo "$variant: artifacts do not match $manifest" >&2
    exit 1
  fi
  echo "$variant: every pinned digest matches"
done

cat <<EOF

Run the conformance suite:
  KEV_VARIANT=<variant> KEV_ARTIFACT_DIR=$root/<variant> KEV_BASE_DIR=$root/<base> \\
    cargo test -p kev --features serve --release
EOF
