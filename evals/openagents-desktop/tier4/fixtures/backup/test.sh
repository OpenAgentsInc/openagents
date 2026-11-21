#!/bin/sh
# Firm reason: 5 (Tier 4 eval fixture: a tiny project the live agent edits in a temp repo)
set -e
rm -rf src dest
mkdir src
echo one > 'src/plain.txt'
echo two > 'src/with space.txt'
echo three > 'src/two  spaces.md'
sh backup.sh
test "$(cat dest/plain.txt.bak)" = one
test "$(cat 'dest/with space.txt.bak')" = two
test "$(cat 'dest/two  spaces.md.bak')" = three
test "$(ls dest | wc -l | tr -d ' ')" = 3
rm -rf src dest
echo ok
