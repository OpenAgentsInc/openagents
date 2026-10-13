#!/usr/bin/env bash
# Runs ON the VM: mirror ~/out to GCS every 90 s so a preemption loses at most that much.
DEST=gs://openagentsgemini-decision-throughput/tpu/${DATE:-2026-10-13}/${1:?node name}
pgrep -f '[v]msync-loop' >/dev/null && exit 0
exec -a vmsync-loop bash -c "while true; do mkdir -p ~/out; cp ~/setup.json ~/out/ 2>/dev/null; gcloud storage rsync -r ~/out $DEST -q >/dev/null 2>&1; sleep 90; done"
