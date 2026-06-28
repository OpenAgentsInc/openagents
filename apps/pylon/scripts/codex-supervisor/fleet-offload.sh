#!/usr/bin/env bash
#
# fleet-offload.sh — move already-authenticated Pylon Codex profiles to
# secondary Tailnet Macs and print/launch the matching standing supervisors.
#
# This script never runs `codex login` or `pylon auth codex`. It copies existing
# isolated Pylon account homes from:
#   $PYLON_HOME/accounts/codex/<ref>
# to the remote:
#   $REMOTE_PYLON_HOME/accounts/codex/<ref>
#
# Default mode is dry-run. Pass --execute to run tar/scp/ssh.
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../../.." && pwd)"

PYLON_HOME="${PYLON_HOME:-$HOME/.pylon-fable}"
REMOTE_PYLON_HOME="${REMOTE_PYLON_HOME:-~/.pylon-fable}"
REMOTE_REPO="${REMOTE_REPO:-~/work/openagents}"
SUP_STATE_BASE="${SUP_STATE_BASE:-~/.codex-supervisor}"
SUP_PER_ACCOUNT="${SUP_PER_ACCOUNT:-2}"
SUP_MAX_SLOTS_PER_HOST="${SUP_MAX_SLOTS_PER_HOST:-4}"
SUP_REPO="${SUP_REPO:-OpenAgentsInc/openagents}"
SUP_VERIFY="${SUP_VERIFY:-bun run --cwd apps/openagents.com/workers/api test -- src/labor-earnings-routes.test.ts}"
REMOTE_AGENT_ENV="${REMOTE_AGENT_ENV:-~/.pylon-fable/openagents-agent.env}"
SSH_BIN="${SSH_BIN:-ssh}"
SCP_BIN="${SCP_BIN:-scp}"
TAR_BIN="${TAR_BIN:-tar}"
MKDIR_BIN="${MKDIR_BIN:-mkdir}"
DATE_BIN="${DATE_BIN:-date}"

DRY_RUN=1
HOSTS=()
ACCOUNTS=()

usage() {
  cat <<'USAGE'
Usage:
  fleet-offload.sh --hosts imac-pro-bertha,macbook-pro-m2 --accounts codex-4,codex-5,codex-6,codex-7 [--execute]

Options:
  --hosts       Comma or space separated Tailnet hosts.
  --accounts    Comma or space separated Codex account refs to move.
  --execute     Actually run tar/scp/ssh. Default is dry-run.
  --help        Show this help.

Environment:
  PYLON_HOME                 Local Pylon home. Default: ~/.pylon-fable
  REMOTE_PYLON_HOME          Remote Pylon home. Default: ~/.pylon-fable
  REMOTE_REPO                Remote clean openagents checkout. Default: ~/work/openagents
  REMOTE_AGENT_ENV           Remote env file exporting OPENAGENTS_AGENT_TOKEN.
                             Default: ~/.pylon-fable/openagents-agent.env
  SUP_PER_ACCOUNT            Supervisor parallelism per account. Default: 2
  SUP_MAX_SLOTS_PER_HOST     Host slot cap. Default: 4
  SUP_PYLON_REF              Required for --execute launch commands.
  OPENAGENTS_AGENT_TOKEN     Required on the remote for launched supervisors.
USAGE
}

die() {
  printf 'FATAL: %s\n' "$*" >&2
  exit 1
}

split_words() {
  printf '%s\n' "$1" | tr ',' ' ' | tr -s ' ' '\n' | sed '/^$/d'
}

shell_quote() {
  # Single-quote a shell word.
  printf "'%s'" "$(printf '%s' "$1" | sed "s/'/'\\\\''/g")"
}

remote_path_expr() {
  case "$1" in
    "~") printf '~' ;;
    "~/"*) printf '~/%s' "$(printf '%s' "${1#~/}" | sed "s/'/'\\\\''/g")" ;;
    "\$HOME") printf '$HOME' ;;
    "\$HOME/"*) printf '$HOME/%s' "$(printf '%s' "${1#\$HOME/}" | sed "s/'/'\\\\''/g")" ;;
    *) shell_quote "$1" ;;
  esac
}

run_cmd() {
  if [ "$DRY_RUN" -eq 1 ]; then
    printf 'DRY-RUN:'
    local arg
    for arg in "$@"; do printf ' %s' "$(shell_quote "$arg")"; done
    printf '\n'
    return 0
  fi
  "$@"
}

remote_sh() {
  local host="$1"
  shift
  if [ "$DRY_RUN" -eq 1 ]; then
    printf 'DRY-RUN: %s %s %s\n' "$(shell_quote "$SSH_BIN")" "$(shell_quote "$host")" "$(shell_quote "$*")"
    return 0
  fi
  "$SSH_BIN" "$host" "$*"
}

parse_args() {
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --hosts)
        shift; [ "$#" -gt 0 ] || die "--hosts requires a value"
        while IFS= read -r item; do HOSTS+=("$item"); done < <(split_words "$1")
        ;;
      --accounts)
        shift; [ "$#" -gt 0 ] || die "--accounts requires a value"
        while IFS= read -r item; do ACCOUNTS+=("$item"); done < <(split_words "$1")
        ;;
      --execute) DRY_RUN=0 ;;
      --help|-h) usage; exit 0 ;;
      *) die "unknown argument: $1" ;;
    esac
    shift
  done
}

validate() {
  [ "${#HOSTS[@]}" -gt 0 ] || die "provide at least one --hosts entry"
  [ "${#ACCOUNTS[@]}" -gt 0 ] || die "provide at least one --accounts entry"
  if [ "$DRY_RUN" -eq 0 ]; then
    [ -n "${SUP_PYLON_REF:-}" ] || die "SUP_PYLON_REF is required with --execute"
  fi

  local account
  for account in "${ACCOUNTS[@]}"; do
    case "$account" in
      default|codex|codex-[A-Za-z0-9._-]*) ;;
      *) die "unsafe account ref '$account'; expected codex or codex-N" ;;
    esac
    [ -d "$PYLON_HOME/accounts/codex/$account" ] || die "missing local account profile: $PYLON_HOME/accounts/codex/$account"
  done
}

accounts_for_host() {
  local host_index="$1"
  local host_count="$2"
  local i
  for i in "${!ACCOUNTS[@]}"; do
    if [ $(( i % host_count )) -eq "$host_index" ]; then
      printf '%s\n' "${ACCOUNTS[$i]}"
    fi
  done
}

host_account_csv() {
  paste -sd, -
}

copy_account() {
  local host="$1"
  local account="$2"
  local parent="$PYLON_HOME/accounts/codex"
  local stamp
  stamp="$("$DATE_BIN" -u +%Y%m%dT%H%M%SZ)"
  local archive="/tmp/openagents-${account}-${stamp}.tgz"
  local remote_archive="/tmp/openagents-${account}-${stamp}.tgz"
  local remote_dir="$REMOTE_PYLON_HOME/accounts/codex"

  run_cmd "$TAR_BIN" -C "$parent" -czf "$archive" "$account"
  remote_sh "$host" "$MKDIR_BIN -p $(remote_path_expr "$remote_dir")"
  run_cmd "$SCP_BIN" "$archive" "$host:$remote_archive"
  remote_sh "$host" "$TAR_BIN -xzf $(shell_quote "$remote_archive") -C $(remote_path_expr "$remote_dir")"
  remote_sh "$host" "rm -f $(shell_quote "$remote_archive")"
  run_cmd rm -f "$archive"
}

launch_host() {
  local host="$1"
  local accounts_csv="$2"
  local state_dir="$SUP_STATE_BASE/$host"
  local max_slots="$SUP_MAX_SLOTS_PER_HOST"
  local launch
  launch="set -a; [ -f $(remote_path_expr "$REMOTE_AGENT_ENV") ] && . $(remote_path_expr "$REMOTE_AGENT_ENV"); set +a; cd $(remote_path_expr "$REMOTE_REPO") && PYLON_HOME=$(remote_path_expr "$REMOTE_PYLON_HOME") SUP_STATE_DIR=$(remote_path_expr "$state_dir") SUP_PYLON_REF=$(shell_quote "${SUP_PYLON_REF:-<live-pylon-ref>}") SUP_MAX_SLOTS=$(shell_quote "$max_slots") SUP_PER_ACCOUNT=$(shell_quote "$SUP_PER_ACCOUNT") SUP_ACCOUNT_REFS=$(shell_quote "$accounts_csv") SUP_REPO=$(shell_quote "$SUP_REPO") SUP_VERIFY=$(shell_quote "$SUP_VERIFY") bash apps/pylon/scripts/codex-supervisor/launch.sh start"
  remote_sh "$host" "$launch"
}

main() {
  parse_args "$@"
  validate

  printf 'mode=%s local_home=%s remote_home=%s remote_repo=%s\n' \
    "$([ "$DRY_RUN" -eq 1 ] && echo dry-run || echo execute)" \
    "$PYLON_HOME" "$REMOTE_PYLON_HOME" "$REMOTE_REPO"

  local host_count="${#HOSTS[@]}"
  local host_index
  for host_index in "${!HOSTS[@]}"; do
    local host="${HOSTS[$host_index]}"
    local assigned=()
    while IFS= read -r account; do assigned+=("$account"); done < <(accounts_for_host "$host_index" "$host_count")
    [ "${#assigned[@]}" -gt 0 ] || continue

    printf 'host=%s accounts=%s\n' "$host" "$(printf '%s\n' "${assigned[@]}" | host_account_csv)"
    local account
    for account in "${assigned[@]}"; do
      copy_account "$host" "$account"
    done
    launch_host "$host" "$(printf '%s\n' "${assigned[@]}" | host_account_csv)"
  done
}

main "$@"
