#!/bin/sh
# Startup script for a DeepSeek-V4.1 stage machine (#12402). `pylon cloud up`
# passes it as the machine's startup-script with these metadata values:
#   oa-weights     the checkpoint's gs:// prefix (never Hugging Face)
#   oa-layers      this machine's stages' layer ranges, such as 0-10+10-20
#   oa-embeddings  1 when a stage here holds the embeddings
#   oa-head        1 when a stage here holds the final norm and head
#   oa-worker      the stage worker binary's gs:// URL (W11), as
#                  scripts/pylon/v41-stage-build.sh uploads it:
#                  .../psionic-v41-stage/COMMIT/sm_NN/psionic-v41-stage,
#                  with the group manifests beside it in manifests/ and
#                  the CUDA 13 libraries it opens in lib/
#   oa-manifest    the group manifest's SHA-256
# and, when the group head runs in the zone (#12399, `--head stage0|vm`):
#   oa-role        `head` on a machine that only heads the group
#   oa-group-head  1 on the machine the head runs on
#   oa-pylon       the pylon binary's gs:// URL, for that machine
#   oa-stage-ssh   scripts/pylon/v41-stage-ssh.sh, for that machine
# It copies only the shards those stages read (W9's stage selection), the
# tokenizer files, and the group manifest into the weights folder, installs
# the worker as /opt/openagents/stage-worker after checking it was built for
# this machine's GPU, and installs /opt/openagents/stage-run, which the group
# head starts over ssh for each stage, so the head owns every worker's
# lifetime. On the head's machine it also installs /opt/openagents/head-run
# and stage-ssh.
#
# Local mode, for tests: OA_STAGE_METADATA=DIR reads each metadata value from
# the file DIR/KEY instead of the metadata server; OA_STAGE_ROOT and
# OA_STAGE_OPT replace /mnt/stage and /opt/openagents; OA_GCLOUD and
# OA_NVIDIA_SMI replace gcloud and nvidia-smi.
set -eu
root=${OA_STAGE_ROOT:-/mnt/stage}
opt=${OA_STAGE_OPT:-/opt/openagents}
gcloud=${OA_GCLOUD:-gcloud}
mkdir -p "$root/weights" "$opt"
exec >>"$root/startup.log" 2>&1
md() {
  if [ -n "${OA_STAGE_METADATA:-}" ]; then
    cat "$OA_STAGE_METADATA/$1"
    return
  fi
  curl -fsS -H 'Metadata-Flavor: Google' \
    "http://metadata.google.internal/computeMetadata/v1/instance/attributes/$1"
}
optional() { md "$1" 2>/dev/null || true; }
if [ "$(optional oa-group-head)" = 1 ]; then
  "$gcloud" storage cp "$(md oa-pylon)" "$opt/pylon"
  chmod 755 "$opt/pylon"
  md oa-stage-ssh >"$opt/stage-ssh"
  chmod 755 "$opt/stage-ssh"
  cat >"$opt/head-run" <<RUN
#!/bin/sh
# Head the group in the foreground of the budget host's ssh session; the
# launch arrives on standard input, and the session ending stops the head.
exec $opt/pylon cloud head --home "\$HOME/.openagents/v41-head"
RUN
  chmod 755 "$opt/head-run"
  echo "oa-stage: the group head can start here"
fi
if [ "$(optional oa-role)" = head ]; then
  touch "$root/ready"
  exit 0
fi
weights=$(md oa-weights)
layers=$(md oa-layers)
embeddings=$(md oa-embeddings)
head=$(md oa-head)
worker=$(md oa-worker)
manifest=$(md oa-manifest)
echo "oa-stage: layers $layers from $weights"
# The worker is built for one GPU architecture; refuse another before
# copying any weights.
case $worker in
  */sm_*/*)
    want=${worker%/*}
    want=${want##*/sm_}
    have=$("${OA_NVIDIA_SMI:-nvidia-smi}" --query-gpu=compute_cap --format=csv,noheader |
      head -n 1 | tr -d '. ')
    if [ "$want" != "$have" ]; then
      echo "oa-stage: the worker is built for sm_$want, but this GPU is sm_$have"
      exit 1
    fi
    ;;
esac
"$gcloud" storage cp "${weights}model.safetensors.index.json" "${weights}config.json" \
  "${weights}tokenizer.json" "${weights}tokenizer_config.json" "$root/weights/"
python3 - "$root/weights/model.safetensors.index.json" "$layers" "$embeddings" "$head" \
  >"$root/shards" <<'PY'
import json, sys
index, layers, embeddings, head = sys.argv[1], sys.argv[2], sys.argv[3] == "1", sys.argv[4] == "1"
spans = [tuple(int(n) for n in span.split("-")) for span in layers.split("+")]
want = set()
for name, shard in json.load(open(index))["weight_map"].items():
    part = name.split(".")
    if part[0] == "layers" and any(a <= int(part[1]) < b for a, b in spans):
        want.add(shard)
    elif (name == "embed.weight" and embeddings) or (name in ("norm.weight", "head.weight") and head):
        want.add(shard)
print("\n".join(sorted(want)))
PY
sed "s|^|$weights|" "$root/shards" | "$gcloud" storage cp -I "$root/weights/"
# The group manifest, which each stage names by its SHA-256.
mkdir -p "$root/weights/manifests"
"$gcloud" storage cp "${worker%/*}/manifests/$manifest.json" "$root/weights/manifests/"
got=$(sha256sum "$root/weights/manifests/$manifest.json" | cut -d ' ' -f 1)
if [ "$got" != "$manifest" ]; then
  echo "oa-stage: the group manifest hashes to $got, not $manifest"
  exit 1
fi
"$gcloud" storage cp "$worker" "$opt/stage-worker"
chmod 755 "$opt/stage-worker"
mkdir -p "$opt/lib"
"$gcloud" storage cp "${worker%/*}/lib/*" "$opt/lib/"
cat >"$opt/stage-run" <<RUN
#!/bin/sh
# Wait for this machine's weights, then run one stage worker in the
# foreground of the head's ssh session; the session ending ends it.
i=0
while [ ! -f $root/ready ]; do
  i=\$((i + 1))
  [ "\$i" -le 1800 ] || { echo "the stage weights are not ready" >&2; exit 1; }
  sleep 1
done
export LD_LIBRARY_PATH=$opt/lib\${LD_LIBRARY_PATH:+:\$LD_LIBRARY_PATH}
exec $opt/stage-worker --weights $root/weights "\$@"
RUN
chmod 755 "$opt/stage-run"
touch "$root/ready"
echo "oa-stage: ready with $(wc -l <"$root/shards") shards"
