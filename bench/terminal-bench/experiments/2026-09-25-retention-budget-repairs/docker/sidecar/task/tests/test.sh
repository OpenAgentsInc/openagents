#!/bin/sh
set -eu
test "$(cat /app/out.txt)" = first
test "$(cat /logs/artifacts/note.txt)" = retained
test ! -e /app/absent.txt
test ! -e /app/absent-dir
test "$(cat /tmp/db.dump)" = first
printf 1 > /logs/verifier/reward.txt
