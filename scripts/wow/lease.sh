#!/usr/bin/env bash
# Hold one private realm lease; usable as an SSH forced command.
set -euo pipefail
umask 077
if [ "${1:-}" = --ssh ]; then
  case "${SSH_ORIGINAL_COMMAND:-}" in
    '$HOME/wow-gym/lease.sh '*) key="${SSH_ORIGINAL_COMMAND#* }" ;;
    *) echo 'unsupported lease request' >&2; exit 2 ;;
  esac
else
  key="${1:-}"
fi
[[ "$key" =~ ^(capacity-[12]|account-GYM([1-9]|1[0-9]|20|SETUP)|character-[a-z]{2,12})$ ]] || { echo 'invalid lease key' >&2; exit 2; }
if [[ "$key" == account-* && -n "${WOW_LEASE_ACCOUNTS:-}" ]]; then
  case ",${WOW_LEASE_ACCOUNTS}," in
    *",${key#account-},"*) ;;
    *) echo 'account outside key scope' >&2; exit 2 ;;
  esac
fi
root="${WOW_GYM_ROOT:-$HOME/wow-gym}"
mkdir -p "$root/leases"
exec flock -n "$root/leases/$key.lock" sh -c 'echo acquired; cat >/dev/null'
