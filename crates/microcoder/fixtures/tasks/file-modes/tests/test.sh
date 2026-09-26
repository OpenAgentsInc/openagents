#!/bin/bash
# Passes when every file is 644 and every directory 755: the modes umask 022
# gives. /app/loop.txt and /app/loop-dir come from the Rust test's command.
touch /tmp/verifier.txt && mkdir -p /tmp/verifier-dir
ok=1
for f in /app/loop.txt /app/solution.txt /app/collected.txt /tmp/verifier.txt; do
  mode=$(stat -c %a "$f" 2>/dev/null || echo missing)
  echo "$f $mode"
  [ "$mode" = 644 ] || ok=0
done
for f in /app/loop-dir /app/solution-dir /app/collected-dir /tmp/verifier-dir; do
  mode=$(stat -c %a "$f" 2>/dev/null || echo missing)
  echo "$f $mode"
  [ "$mode" = 755 ] || ok=0
done
echo $ok > /logs/verifier/reward.txt
