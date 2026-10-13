#!/usr/bin/env bash
# Installs the Clef TPU lane as two systemd units on a TPU VM (#11459):
#   clef-tpu-worker  the sealed JAX worker on 127.0.0.1:8091 (python -m clef_tpu.server)
#   clef-tpu-door    psionic-openai-server on :18096, /v1/systemone, --decision-worker
# Expects ~/venv (vm.sh setup), ~/clef-flash (BF16 safetensors), ~/oa/clef-tpu (this directory),
# ~/gguf/Clef-Flash-Q4_K_M.gguf (tokenizer and encoder only) and the door binary at $DOOR.
set -euo pipefail
U=${SUDO_USER:-$USER}
H=$(getent passwd "$U" | cut -d: -f6)
DOOR=${DOOR:-$H/src/crates/psionic/target/release/psionic-openai-server}
sudo tee /etc/systemd/system/clef-tpu-worker.service >/dev/null <<UNIT
[Unit]
Description=Clef TPU worker (JAX, localhost only)
After=network-online.target
[Service]
User=$U
WorkingDirectory=$H/oa/clef-tpu
ExecStart=$H/venv/bin/python -m clef_tpu.server $H/clef-flash --port 8091 --chunk 2048 --precision ${PRECISION:-bf16}
# libtpu pins its buffers; systemd's default memlock limit fails TPU init
LimitMEMLOCK=infinity
Restart=always
RestartSec=5
[Install]
WantedBy=multi-user.target
UNIT
sudo tee /etc/systemd/system/clef-tpu-door.service >/dev/null <<UNIT
[Unit]
Description=Clef TPU door (psionic-openai-server /v1/systemone)
After=clef-tpu-worker.service
Requires=clef-tpu-worker.service
[Service]
User=$U
ExecStartPre=/bin/sh -c 'until curl -sf http://127.0.0.1:8091/v1/identity >/dev/null; do sleep 5; done'
ExecStart=$DOOR --decision-model $H/gguf/Clef-Flash-Q4_K_M.gguf --decision-device cpu --decision-worker http://127.0.0.1:8091 --decision-max-tokens 16384 --host 0.0.0.0 --port 18096
TimeoutStartSec=900
Restart=always
RestartSec=5
[Install]
WantedBy=multi-user.target
UNIT
sudo systemctl daemon-reload
sudo systemctl enable clef-tpu-worker clef-tpu-door
sudo systemctl restart --no-block clef-tpu-worker clef-tpu-door
