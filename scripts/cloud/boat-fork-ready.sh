#!/usr/bin/env bash
#
# Make a Boat sandbox started from an `oa-coder-main-<date>` template ready to
# build (issue #10251). Run it first, synchronously, as the sandbox's `user`.
#
# What a fresh fork needs, measured 2026-10-03:
#   1. Ownership. A fork comes back with a few dozen directories owned by root
#      (~/.cargo, ~/.openagents, slot directories, ~/.ascii/processes). Until
#      ~/.ascii is repaired Boat's detached commands fail with EACCES, and
#      until the rest is, cargo cannot write the slot. ~/.ascii is repaired
#      first, the rest after the restore (or at once with --no-wait).
#   2. The restore. Boat restores $HOME lazily: a FUSE mount (ascii-lazyfs)
#      serves files on demand while a background extract fills the real disk,
#      then the mount is "retired" (moved to /var/lib/ascii-lazy/retired/...)
#      and $HOME is the plain disk. Building on the FUSE mount is slow and can
#      stall for many minutes on a large file, and a process whose working
#      directory is inside the mount at retirement keeps the retired path (so
#      cargo there sees /var/lib/ascii-lazy/retired/home/...). So this waits
#      until $HOME is plain disk, and builds start from a new process after.
#      (Reading the warm slot ahead of the extract did not make it sooner.)
#
# Usage: boat-fork-ready.sh [--timeout SECS] [--no-wait] [--bookkeeping-only]
# (--bookkeeping-only repairs ~/.ascii and stops: run it as a plain,
# synchronous command before the first detached one.)
# Prints key=value lines: fork_ready_bookkeeping_seconds,
# fork_ready_restore_seconds, fork_ready_restored (true|false),
# fork_ready_chown_seconds. Exits 0 even when the restore is still
# running at the timeout (the build then runs on the FUSE mount, slower).
set -uo pipefail

timeout=900
wait_restore=true
bookkeeping_only=false
while [[ $# -gt 0 ]]; do
  case "$1" in
    --timeout) timeout="${2:?}"; shift 2 ;;
    --no-wait) wait_restore=false; shift ;;
    --bookkeeping-only) bookkeeping_only=true; shift ;;
    *) echo "boat-fork-ready: unknown argument: $1" >&2; exit 2 ;;
  esac
done

now() { date +%s.%N; }
since() { awk -v a="$1" -v b="$(now)" 'BEGIN { printf "%.1f", b - a }'; }
lazy() { grep -q "^ascii-lazyfs $HOME fuse" /proc/mounts; }

cd / || exit 1
owner="$(id -u):$(id -g)"
repair() { sudo -n find "$@" -xdev -user root -exec chown -h "$owner" {} + 2>/dev/null || true; }
t=$(now)
# Boat's own bookkeeping first (a few files): its detached commands need it.
repair "$HOME/.ascii"
sudo -n chown -h "$owner" "$HOME" 2>/dev/null || true
echo "fork_ready_bookkeeping_seconds=$(since "$t")"
[[ "$bookkeeping_only" == true ]] && exit 0

t=$(now)
if [[ "$wait_restore" == true ]] && lazy; then
  deadline=$(( $(date +%s) + timeout ))
  while lazy && (( $(date +%s) < deadline )); do sleep 1; done
fi
echo "fork_ready_restore_seconds=$(since "$t")"
if lazy; then echo "fork_ready_restored=false"; else echo "fork_ready_restored=true"; fi

# The whole of $HOME, once it is plain disk: walking it through the lazy
# mount took 26 to 100 s on some forks, against about 4 s on disk.
t=$(now)
repair "$HOME"
echo "fork_ready_chown_seconds=$(since "$t")"
