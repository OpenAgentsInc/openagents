#!/bin/sh
# Startup script for a DeepSeek-V4.1 stage machine (#12402). `pylon cloud up`
# passes it as the machine's startup-script with these metadata values:
#   oa-weights     the checkpoint's gs:// prefix (never Hugging Face)
#   oa-layers      this machine's stages' layer ranges, such as 0-10+10-20
#   oa-embeddings  1 when a stage here holds the embeddings
#   oa-head        1 when a stage here holds the final norm and head
#   oa-worker      the stage worker binary's gs:// URL (W11)
# It copies only the shards those stages read (W9's stage selection), then
# installs /opt/openagents/stage-run, which the group head starts over ssh
# for each stage, so the head owns every worker's lifetime.
set -eu
root=/mnt/stage
mkdir -p "$root/weights" /opt/openagents
exec >>"$root/startup.log" 2>&1
md() {
  curl -fsS -H 'Metadata-Flavor: Google' \
    "http://metadata.google.internal/computeMetadata/v1/instance/attributes/$1"
}
weights=$(md oa-weights)
layers=$(md oa-layers)
embeddings=$(md oa-embeddings)
head=$(md oa-head)
worker=$(md oa-worker)
echo "oa-stage: layers $layers from $weights"
gcloud storage cp "${weights}model.safetensors.index.json" "${weights}config.json" \
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
sed "s|^|$weights|" "$root/shards" | gcloud storage cp -I "$root/weights/"
gcloud storage cp "$worker" /opt/openagents/stage-worker
chmod 755 /opt/openagents/stage-worker
cat >/opt/openagents/stage-run <<'RUN'
#!/bin/sh
# Wait for this machine's weights, then run one stage worker in the
# foreground of the head's ssh session; the session ending ends it.
i=0
while [ ! -f /mnt/stage/ready ]; do
  i=$((i + 1))
  [ "$i" -le 1800 ] || { echo "the stage weights are not ready" >&2; exit 1; }
  sleep 1
done
exec /opt/openagents/stage-worker --weights /mnt/stage/weights "$@"
RUN
chmod 755 /opt/openagents/stage-run
touch "$root/ready"
echo "oa-stage: ready with $(wc -l <"$root/shards") shards"
