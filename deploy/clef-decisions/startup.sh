#!/bin/bash
# Boot script of the decision GPUs (clef-decisions-h100, clef-decisions-a100),
# the backends of the decision gateway's hosted Clef pools
# (docs/inference/clef-decisions-h100.md). Runs as root on every boot
# (first boot and every start after the VM was stopped). It:
#   1. fetches the Psionic CUDA bundle (psionic-openai-server and its CUDA
#      13 libraries, scripts/bench/decision-throughput/h100/
#      cloudbuild-bundle.yaml) named by metadata `clef-bundle` (default
#      bin/latest, an sm_90 build; the A100 pins an sm_80 one), when the
#      one on disk is another;
#   2. fetches Clef-Flash and Clef 27B Q4_K_M from the GCS model cache onto
#      the boot disk when missing, and checks their SHA-256 once;
#   3. writes and starts two systemd services, clef-flash (:18096) and
#      clef-27b (:18098), restart=always, and a health timer that restarts
#      a service whose /health fails three times in a row.
# The ports are reachable only from the gateway's subnets
# (deploy/clef-decisions/provision.sh firewall). Nothing secret is here.
set -u
B=gs://openagentsgemini-decision-throughput
W=/opt/clef
M=/var/lib/clef/models
LOG=/var/log/clef-decisions-boot.log
exec > >(tee -a "$LOG") 2>&1
T0=$(date +%s)
say() { echo "[clef-decisions $(( $(date +%s) - T0 ))s] $*"; }
md() { curl -sf -H Metadata-Flavor:Google "http://metadata.google.internal/computeMetadata/v1/instance/$1"; }

say "boot on $(md name) ($(md zone | awk -F/ '{print $NF}'))"
for i in $(seq 60); do nvidia-smi >/dev/null 2>&1 && break; sleep 5; done
nvidia-smi --query-gpu=name,driver_version,memory.total --format=csv,noheader

# 1. The bundle.
TAG=$(md attributes/clef-bundle || true)
[ -n "$TAG" ] || TAG=$(gcloud storage cat "$B/bin/latest")
HAVE=$(cat "$W/tag.txt" 2>/dev/null || cat "$W/commit.txt" 2>/dev/null || true)
CHANGED=0
if [ "$HAVE" != "$TAG" ] || [ ! -x "$W/psionic-openai-server" ]; then
    say "bundle $TAG (had ${HAVE:-none})"
    rm -rf "$W.new"; mkdir -p "$W.new"
    if gcloud storage cp "$B/bin/$TAG/bundle.tar.gz" "$W.new/" &&
        tar -xzf "$W.new/bundle.tar.gz" -C "$W.new" &&
        (cd "$W.new" && sha256sum -c --quiet SHA256SUMS); then
        rm -f "$W.new/bundle.tar.gz"; echo "$TAG" > "$W.new/tag.txt"
        rm -rf "$W.old"; [ -d "$W" ] && mv "$W" "$W.old"; mv "$W.new" "$W"
        CHANGED=1
    else
        say "bundle $TAG failed to fetch or verify; keeping ${HAVE:-nothing}"
        rm -rf "$W.new"
    fi
fi
say "serving bundle $(cat "$W/commit.txt" 2>/dev/null) ($(cat "$W/image.txt" "$W/arch.txt" 2>/dev/null))"

# 2. The models (pinned digests; the cache was filled from Hugging Face at
# pinned revisions by the decision-throughput run).
mkdir -p "$M"
fetch() { # file sha256
    if [ -s "$M/$1" ] && [ "$(cat "$M/$1.ok" 2>/dev/null)" = "$2" ]; then return 0; fi
    say "fetching $1"
    gcloud storage cp "$B/models/$1" "$M/$1.part" || return 1
    got=$(sha256sum "$M/$1.part" | cut -d' ' -f1)
    if [ "$got" != "$2" ]; then say "$1: digest $got, wanted $2"; rm -f "$M/$1.part"; return 1; fi
    mv "$M/$1.part" "$M/$1"; echo "$2" > "$M/$1.ok"; CHANGED=1
}
fetch Clef-Flash-Q4_K_M.gguf fd3e90605e8103307dca37cb5a8cdb036267e2fe3cb2d908d80a8ceb9ec0638c &
F1=$!
fetch Clef-Q4_K_M.gguf bd9b8ae24c5752a2bb5e2733d5168340d5f04457e02b8bf0d1bc9285ec3d4f61 &
F2=$!
wait $F1 || say "Clef-Flash is missing"
wait $F2 || say "Clef 27B is missing"

# 3. The services.
unit() { # name model port
    cat > "/etc/systemd/system/$1.service" <<UNIT
[Unit]
Description=Psionic $2 decisions on :$3 (docs/inference/clef-decisions-h100.md)
After=network-online.target
Wants=network-online.target

[Service]
Environment=LD_LIBRARY_PATH=$W/lib
ExecStart=$W/psionic-openai-server -m $M/$2 --host 0.0.0.0 --port $3 --decision-device cuda --decision-chunk 2048 --decision-max-tokens 65536
Restart=always
RestartSec=5
LimitNOFILE=65536
StandardOutput=append:/var/log/$1.log
StandardError=append:/var/log/$1.log

[Install]
WantedBy=multi-user.target
UNIT
}
unit clef-flash Clef-Flash-Q4_K_M.gguf 18096
unit clef-27b Clef-Q4_K_M.gguf 18098

cat > /usr/local/bin/clef-health <<'HEALTH'
#!/bin/bash
# Restart a decision service whose /health failed three checks in a row
# (a server that is still loading answers nothing; 3 minutes covers it).
for pair in clef-flash:18096 clef-27b:18098; do
    name=${pair%%:*}; port=${pair##*:}; f=/run/$name.fails
    if curl -sf -m 5 "127.0.0.1:$port/health" >/dev/null; then echo 0 > "$f"; continue; fi
    n=$(( $(cat "$f" 2>/dev/null || echo 0) + 1 )); echo "$n" > "$f"
    if [ "$n" -ge 3 ]; then
        echo "clef-health: $name failed $n checks; restarting" | systemd-cat -t clef-health
        systemctl restart "$name"; echo 0 > "$f"
    fi
done
HEALTH
chmod 755 /usr/local/bin/clef-health
cat > /etc/systemd/system/clef-health.service <<'UNIT'
[Unit]
Description=Health check of the Clef decision services
[Service]
Type=oneshot
ExecStart=/usr/local/bin/clef-health
UNIT
cat > /etc/systemd/system/clef-health.timer <<'UNIT'
[Unit]
Description=Check the Clef decision services every minute
[Timer]
OnBootSec=3min
OnUnitActiveSec=1min
[Install]
WantedBy=timers.target
UNIT

systemctl daemon-reload
systemctl enable clef-flash clef-27b clef-health.timer
if [ "$CHANGED" = 1 ]; then
    systemctl restart clef-flash clef-27b
else
    systemctl start clef-flash clef-27b
fi
systemctl start clef-health.timer
for p in 18096 18098; do
    for i in $(seq 120); do curl -sf "127.0.0.1:$p/health" >/dev/null && break; sleep 2; done
    if curl -sf "127.0.0.1:$p/health" >/dev/null; then
        say ":$p up: $(curl -sf "127.0.0.1:$p/v1/models" | head -c 200)"
    else
        say ":$p did not come up"; tail -20 /var/log/clef-*.log
    fi
done
nvidia-smi --query-gpu=memory.used --format=csv,noheader
say "boot done"
