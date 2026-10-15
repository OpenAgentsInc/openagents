#!/usr/bin/env bash
# Activate an already checked candidate, preserving policy and requiring a real post-restart payout.
set -euo pipefail
[[ $EUID == 0 && $# == 2 ]] || { echo 'Usage: sudo activate-payouts.sh COMMIT SETTINGS.json' >&2; exit 2; }
revision=$1
settings=$2
[[ $revision =~ ^[a-f0-9]{40}$ && $settings =~ ^/[A-Za-z0-9_./-]+$ ]] || exit 2
binary="/opt/openagents-pay/lnd-candidates/$revision/openagents"
cd "$(dirname "$binary")"
sha256sum -c openagents.sha256
python3 - "$settings" <<'PY'
import json,pathlib,sys
s=json.loads(pathlib.Path(sys.argv[1]).read_text())
p=pathlib.Path(s['policy']); assert p.is_absolute()
p=json.loads(p.read_text()); assert p['version']==1
assert p['mode'] in ('mdk','shadow','canary','lnd')
if p['mode'] in ('canary','lnd'):
    assert p['mainnet_approved'] and p['account_limit_sats']>0 and p['account_id']
PY
unit=openagents-pay-payouts.service
dropin=/etc/systemd/system/$unit.d/50-lnd.conf
since=$(date +%s)
record=/var/lib/openagents-pay/deploy/payout-$since-$$
install -d -m 0700 "$record"
had_previous=false
if [[ -f $dropin ]]; then cp "$dropin" "$record/previous.conf"; had_previous=true; fi
restore() {
  trap - ERR INT TERM
  if $had_previous; then cp "$record/previous.conf" "$dropin"; else rm -f "$dropin"; fi
  systemctl daemon-reload
  systemctl restart "$unit"
  echo 'Payout check failed; restored the previous worker. Spending policy was preserved.' >&2
  exit 1
}
trap restore ERR INT TERM
install -d -m 0755 "$(dirname "$dropin")"
cat > "$dropin" <<UNIT
[Service]
ExecStart=
ExecStart=$binary --json pay payouts --ledger /var/lib/openagents-pay/ledger/ledger.sqlite --spark-home /var/lib/openagents-pay/spark --interval 60 --lnd $settings
UNIT
systemctl daemon-reload
systemctl stop "$unit"
since=$(( $(date +%s) + 1 ))
sleep 1
systemctl start "$unit"
confirmed=false
for attempt in 1 2 3 4 5 6; do
  if systemd-run --quiet --wait --pipe --collect --uid=openagents-pay --gid=openagents-pay \
      --property=EnvironmentFile=/etc/openagents-pay/openagents-pay.env \
      "$binary" --json pay payouts --check --ledger /var/lib/openagents-pay/ledger/ledger.sqlite \
      --lnd "$settings" --require-sent-after "$since"; then
    confirmed=true
    break
  fi
  sleep 20
done
$confirmed || restore
trap - ERR INT TERM
printf '%s\n' "$revision" > "$record/confirmed-commit"
echo 'Payout confirmed after restart; the new worker is active.'
