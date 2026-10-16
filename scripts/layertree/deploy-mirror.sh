#!/usr/bin/env bash
# Install one research mirror and verify its running binary hash.
set -euo pipefail
index=${1:?use INDEX BINARY SOURCE_COMMIT}
binary=${2:?use INDEX BINARY SOURCE_COMMIT}
commit=${3:?use INDEX BINARY SOURCE_COMMIT}
case $index in 0) zone=us-central1-a ;; 1) zone=us-central1-b ;; 2) zone=us-central1-c ;; *) echo 'Use mirror index 0, 1, or 2.' >&2; exit 2 ;; esac
[[ $commit =~ ^[a-f0-9]{40}$ ]] || { echo 'Use the full source commit.' >&2; exit 2; }
git merge-base --is-ancestor "$commit" origin/main
project=openagentsgemini
vm=oa-layertree-mirror-$index
label=$(gcloud compute instances describe "$vm" --project "$project" --zone "$zone" --format='value(labels.issue)')
[[ $label == 11789 ]] || { echo 'This instance is not a signet research mirror.' >&2; exit 2; }
expected=$(sha256sum "$binary" | cut -d' ' -f1)
gcloud compute scp "$binary" "$vm:~/layertree-mirror.incoming" --project "$project" --zone "$zone" --tunnel-through-iap --quiet
remote=$(cat <<'SH'
set -eu
umask 077
mkdir -p "$HOME/layertree-mirror/data"
install -m 0755 "$HOME/layertree-mirror.incoming" "$HOME/layertree-mirror/mirror.staged"
mv "$HOME/layertree-mirror/mirror.staged" "$HOME/layertree-mirror/mirror"
if [ ! -e "$HOME/layertree-mirror/receipt.key" ]; then
    "$HOME/layertree-mirror/mirror" keygen "$HOME/layertree-mirror/receipt.key" > "$HOME/layertree-mirror/receipt.pub"
fi
printf '%s\n' "$task_commit" > "$HOME/layertree-mirror/source-commit"
sudo systemctl stop layertree-mirror.service >/dev/null 2>&1 || true
sudo systemctl reset-failed layertree-mirror.service >/dev/null 2>&1 || true
sudo systemd-run --unit=layertree-mirror --collect \
    --property="User=$(id -un)" --property="Group=$(id -gn)" \
    --property=Restart=on-failure --property=RestartSec=2 \
    --property=NoNewPrivileges=yes --property=ProtectSystem=strict \
    --property=ProtectHome=read-only --property="ReadWritePaths=$HOME/layertree-mirror/data" \
    "$HOME/layertree-mirror/mirror" 0.0.0.0:18440 "$HOME/layertree-mirror/data" "$HOME/layertree-mirror/receipt.key"
task_pid=$(sudo systemctl show layertree-mirror.service --property=MainPID --value)
actual=$(sudo sha256sum "/proc/$task_pid/exe" | cut -d' ' -f1)
test "$actual" = "$task_expected"
sudo systemctl is-active layertree-mirror.service
cat "$HOME/layertree-mirror/receipt.pub"
printf '%s\n' "$actual"
SH
)
gcloud compute ssh "$vm" --project "$project" --zone "$zone" --tunnel-through-iap \
    --command "task_commit=$commit task_expected=$expected bash -c $(printf '%q' "$remote")"
gcloud compute instances add-labels "$vm" --project "$project" --zone "$zone" --labels "source-commit=$commit" --quiet
